// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

//! Compare public evidence briefs through the workshop host driver.
//! See the Athanor README for the corpus format and receipt fields.

use athanor::{
    alembic::workshop::{Owner, Passage, Review, Source},
    decision::{Decision, DecisionDriver, DecisionError, Turn},
    workshop::{Decider, capture, proposal_cap, read_cap, validate_proposal},
    workshop_driver::Driver,
};
use distillery::{
    decision::{ModelDecider, chat_prompt},
    decision_checkpoint::{Placement, load_checkpoint},
};
use muniment::MemoryBackend;
use serde::Deserialize;
use serde_json::{Value, json};
use servitor::{Grant, GrantTable, Mode, Subject};
use std::{
    collections::HashSet,
    error::Error,
    fs::{File, OpenOptions},
    io::Write,
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const CAP: usize = 32;
const SUBJECT: [u8; 32] = [7; 32];
const INSTALLATION: [u8; 16] = [3; 16];
const SYSTEM: &str =
    "You choose which passages an evidence brief cites. Reply with one JSON object.";
const FORMAT: &str = "Choose the passages the question asks for. Return their numbers in the requested order as {\"op\":\"brief\",\"order\":[...]}.";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    cases: Vec<Case>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Case {
    id: String,
    ask: String,
    passages: Vec<Input>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    id: String,
    title: String,
    text: String,
}

// Preserve every raw decision and wall latency, even if owner validation or
// the run deadline subsequently refuses it. Only the host journal determines
// actual retries, and only a proposal awaiting review is eligible for scoring.
struct Trial {
    model: Arc<ModelDecider>,
    ask: String,
    attempts: Mutex<Vec<Value>>,
}
impl DecisionDriver for Trial {
    fn decide(&self, turn: &Turn<'_>) -> std::result::Result<Decision, DecisionError> {
        let context = format!("{}\nQuestion: {}", turn.context, self.ask);
        let start = Instant::now();
        let result = DecisionDriver::decide(
            &*self.model,
            &Turn {
                context: &context,
                ..*turn
            },
        );
        let elapsed = start.elapsed().as_millis();
        let observed = match &result {
            Ok(d) => {
                json!({"decision":d.value,"provider":d.provider,"session":d.session,"tokens":d.tokens,"elapsed_ms":elapsed})
            }
            Err(e) => json!({"error":e.to_string(),"elapsed_ms":elapsed}),
        };
        self.attempts
            .lock()
            .map_err(|_| DecisionError::Failed("trial observations poisoned".into()))?
            .push(observed);
        result
    }
}

fn capture_inputs(case: &Case, owner: &Owner) -> Result<Vec<Passage>> {
    let mut ids = HashSet::new();
    if case.ask.trim().is_empty() || !(2..=4).contains(&case.passages.len()) {
        return Err(format!("{} needs a question and two to four passages", case.id).into());
    }
    case.passages
        .iter()
        .map(|p| {
            if p.id.is_empty() || !ids.insert(&p.id) {
                return Err(format!("duplicate or empty passage id for {}", case.id).into());
            }
            Ok(capture(
                Source {
                    owner: owner.clone(),
                    resource: p.id.clone(),
                    version: "1".into(),
                    content_digest: [0; 32],
                    start: 0,
                    end: p.text.len() as u64,
                    title: p.title.clone(),
                },
                &p.text,
                true,
            )?)
        })
        .collect()
}

async fn run_case(case: &Case, model: Option<&Arc<ModelDecider>>) -> Result<Value> {
    let owner = Owner {
        host: "workshop-compare".into(),
        workspace: "public-fixtures".into(),
        session: case.id.clone(),
    };
    let inputs = capture_inputs(case, &owner)?;
    let granted = inputs
        .iter()
        .fold(GrantTable::new(), |table, passage| {
            table.with_grant(Grant::new(
                Subject(SUBJECT),
                read_cap(&passage.source),
                Mode::Read,
            ))
        })
        .with_grant(Grant::new(
            Subject(SUBJECT),
            proposal_cap(&owner, INSTALLATION),
            Mode::Write,
        ));
    let source_gate = |passage: &Passage| inputs.contains(passage);
    let trial = model.map(|model| {
        Arc::new(Trial {
            model: model.clone(),
            ask: case.ask.clone(),
            attempts: Mutex::default(),
        })
    });
    let store = MemoryBackend::new();
    let mut driver = Driver::new(store.clone(), owner.clone(), INSTALLATION, Arc::new(|| {}));
    if let Some(trial) = &trial {
        driver = driver.with_decider(Decider::new(trial.clone(), CAP));
    }
    driver.open(SUBJECT).await?;
    let start = Instant::now();
    let clock = || start.elapsed().as_millis() as u64;
    let mut error = None;
    driver
        .begin(inputs.clone(), &granted, &clock, &source_gate)
        .await?;
    while driver.busy() {
        match driver.poll(&granted, &clock, &source_gate).await {
            Ok(None) => std::thread::sleep(Duration::from_millis(1)),
            Ok(Some(_)) => {}
            Err(e) => {
                error = Some(e.to_string());
                break;
            }
        }
    }
    let elapsed_ms = clock();
    drop(driver);
    // Reopen the same store through a fresh workshop without sending work.
    let view = athanor::workshop::Workshop::new(store, owner, INSTALLATION)
        .view(Some(1))
        .await?;
    let (selected, brief) = if let Some(p) = &view.proposal {
        validate_proposal(p)?;
        if p.sources.iter().any(|passage| !inputs.contains(passage)) {
            return Err("confirmed brief includes a passage outside the selected inputs".into());
        }
        (
            Some(
                p.sources
                    .iter()
                    .map(|s| &s.source.resource)
                    .collect::<Vec<_>>(),
            ),
            Some(&p.djot),
        )
    } else {
        (None, None)
    };
    let attempts = match trial {
        Some(t) => t
            .attempts
            .lock()
            .map_err(|_| "trial observations poisoned")?
            .clone(),
        None => vec![],
    };
    Ok(json!({
        "case":case.id,"inputs":case.passages.iter().map(|p| &p.id).collect::<Vec<_>>(),
        "elapsed_ms":elapsed_ms,"ready_for_review":view.review == Some(Review::AwaitingReview) && view.proposal.is_some(),
        "error":error,"phase":view.phase,"limits":view.limits,"usage":view.usage,
        "turns":view.turns,"attempts":attempts,"selected":selected,"brief":brief,
    }))
}

fn line(out: &mut File, value: &Value) -> Result<()> {
    serde_json::to_writer(&mut *out, value)?;
    out.write_all(b"\n")?;
    out.flush()?;
    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(3..=5).contains(&args.len()) {
        return Err("usage: workshop_compare CORPUS OUTPUT deterministic | MODEL HF_DIRECTORY [resident-f32|streamed-f32|resident-q8]".into());
    }
    let bytes = std::fs::read(&args[0])?;
    let corpus: Corpus = serde_json::from_slice(&bytes)?;
    let mut ids = HashSet::new();
    if corpus.cases.is_empty()
        || corpus
            .cases
            .iter()
            .any(|c| c.id.is_empty() || !ids.insert(&c.id))
    {
        return Err("empty corpus, empty case id or duplicate case id".into());
    }
    let mut out = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[1])?;
    let model_id = &args[2];
    let mut header = json!({
        "event": "start", "model": model_id,
        "corpus_blake3": blake3::hash(&bytes).to_hex().as_str(), "cases": ids.len(),
    });
    if model_id != "deterministic" {
        header["system"] = SYSTEM.into();
        header["format"] = FORMAT.into();
        header["variables"] = json!({"enable_thinking": false});
        header["max_tokens"] = CAP.into();
    }
    line(&mut out, &header)?;
    let model = match args.get(3) {
        None if model_id == "deterministic" => None,
        Some(dir) if model_id != "deterministic" => {
            let placement = match args.get(4).map(String::as_str).unwrap_or("resident-f32") {
                "resident-f32" => Placement::ResidentF32,
                "streamed-f32" => Placement::StreamedF32,
                "resident-q8" => Placement::ResidentQ8,
                other => return Err(format!("unknown placement {other}").into()),
            };
            let dir = Path::new(dir);
            let start = Instant::now();
            let prompt = chat_prompt(
                &std::fs::read(dir.join("tokenizer_config.json"))?,
                SYSTEM.into(),
                FORMAT.into(),
                json!({"enable_thinking":false}),
            )
            .map_err(|e| format!("{e:?}"))?;
            let (model, identity) = match load_checkpoint(dir, model_id, placement, prompt) {
                Ok(m) => m,
                Err(e) => {
                    line(
                        &mut out,
                        &json!({"event":"load_failure","elapsed_ms":start.elapsed().as_millis(),"error":format!("{e:?}")}),
                    )?;
                    return Err(format!("checkpoint load: {e:?}").into());
                }
            };
            line(
                &mut out,
                &json!({"event":"loaded","elapsed_ms":start.elapsed().as_millis(),"identity":identity}),
            )?;
            Some(Arc::new(model))
        }
        _ => return Err("supply a checkpoint directory for a model only".into()),
    };
    for case in &corpus.cases {
        let row = pollster::block_on(run_case(case, model.as_ref()))?;
        line(&mut out, &row)?;
        eprintln!("{} {} {}", model_id, case.id, row["phase"]);
    }
    line(
        &mut out,
        &json!({"event":"complete","cases":corpus.cases.len()}),
    )?;
    Ok(())
}
