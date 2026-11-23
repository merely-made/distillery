// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! Decision-driver P2: a driver at the proposing step, run through the host
//! driver and its worker. Every driver here is deterministic.

use super::*;
use crate::alembic::workshop::ModelTurn;
use crate::decision::{Decision, DecisionDriver, DecisionError, Scripted, Turn};
use crate::workshop_driver::Driver;
use muniment::MemoryBackend;
use serde_json::{Value, json};
use servitor::{Cap, Mode, Subject};
use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, AtomicUsize};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

mod profile;

const ID: [u8; 16] = [5; 16];
const SUBJECT: [u8; 32] = [7; 32];
const WAIT: Duration = Duration::from_secs(5);

struct Granted;
impl AuthorityProvider for Granted {
    fn covers(&self, _: Subject, _: &Cap, _: Mode) -> bool {
        true
    }
}

fn owner() -> Owner {
    Owner {
        host: "decision-driver".into(),
        workspace: "public-fixture".into(),
        session: "p2".into(),
    }
}

fn passage(resource: &str, version: &str, title: &str, text: &str) -> Passage {
    capture(
        Source {
            owner: owner(),
            resource: resource.into(),
            version: version.into(),
            content_digest: [0; 32],
            start: 0,
            end: text.len() as u64,
            title: title.into(),
        },
        text,
        true,
    )
    .unwrap()
}

fn inputs() -> Vec<Passage> {
    PUBLIC_EXAMPLE
        .iter()
        .map(|(resource, version, title, text)| passage(resource, version, title, text))
        .collect()
}

/// A scripted driver that counts how often it is asked and reports usage.
struct Counted {
    script: Mutex<VecDeque<Result<Value, DecisionError>>>,
    calls: AtomicUsize,
}
impl Counted {
    const TOKENS: u64 = 9;
    fn session() -> crate::decision::DecisionSession {
        crate::decision::DecisionSession {
            session_id: "counted-session".into(),
            prompt_template_hash: "counted-template".into(),
        }
    }
    fn new(script: impl IntoIterator<Item = Result<Value, DecisionError>>) -> Arc<Self> {
        Arc::new(Self {
            script: Mutex::new(script.into_iter().collect()),
            calls: AtomicUsize::new(0),
        })
    }
    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}
impl DecisionDriver for Counted {
    fn decide(&self, _: &Turn<'_>) -> Result<Decision, DecisionError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let next = self.script.lock().unwrap().pop_front();
        next.unwrap_or_else(|| Err(DecisionError::Failed("script exhausted".into())))
            .map(|value| Decision {
                value,
                provider: "test.counted".into(),
                session: Some(Self::session()),
                tokens: Self::TOKENS,
            })
    }
}

/// A turn that runs until it is cancelled, as a model mid-generation does.
struct Blocking {
    entered: Mutex<mpsc::Sender<()>>,
    saw_cancel: AtomicBool,
}
impl DecisionDriver for Blocking {
    fn decide(&self, turn: &Turn<'_>) -> Result<Decision, DecisionError> {
        let _ = self.entered.lock().unwrap().send(());
        let start = Instant::now();
        while start.elapsed() < WAIT {
            if (turn.cancelled)() {
                self.saw_cancel.store(true, Ordering::SeqCst);
                return Err(DecisionError::Failed("stopped at a token boundary".into()));
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        Err(DecisionError::Failed("never cancelled".into()))
    }
}

fn brief(order: &[usize]) -> Value {
    json!({"op": "brief", "order": order})
}

fn rig(
    store: &MemoryBackend,
    driver: Arc<dyn DecisionDriver>,
    max_tokens: usize,
) -> (Driver<MemoryBackend>, Receiver<()>) {
    let (send, receive) = mpsc::channel();
    let wake = Arc::new(move || {
        let _ = send.send(());
    });
    let host = Driver::new(store.clone(), owner(), ID, wake)
        .with_decider(Decider::new(driver, max_tokens));
    (host, receive)
}

/// One observation through the host driver.
async fn next(
    host: &mut Driver<MemoryBackend>,
    wake: &Receiver<()>,
    clock: &impl Fn() -> u64,
) -> Result<WorkshopView, Error> {
    loop {
        if let Some(view) = host.poll(&Granted, clock, &|_| true).await? {
            return Ok(view);
        }
        wake.recv_timeout(WAIT).expect("the worker answers");
    }
}

/// Poll until the host driver stops: a stop state, a pause or an error.
async fn settle(
    host: &mut Driver<MemoryBackend>,
    wake: &Receiver<()>,
    clock: &impl Fn() -> u64,
) -> Result<WorkshopView, Error> {
    while host.busy() {
        next(host, wake, clock).await?;
    }
    host.refresh().await
}

fn ticking(time: &AtomicU64) -> impl Fn() -> u64 + '_ {
    move || time.fetch_add(1, Ordering::SeqCst)
}

fn proposing(attempt: u64) -> Correlation {
    Correlation {
        run: 1,
        step: 1,
        attempt,
        generation: 1,
    }
}

/// Rewrite the stored run, as a corrupted or forged journal would.
async fn tamper(
    shop: &Workshop<MemoryBackend>,
    store: &MemoryBackend,
    edit: impl FnOnce(&mut Run),
) {
    let (_, mut record) = shop.load().await.unwrap();
    edit(&mut record.runs[0]);
    store
        .put(&shop.key, &encoded(&record).unwrap())
        .await
        .unwrap();
}

#[test]
fn a_scripted_driver_runs_to_review_through_the_host_driver_and_the_same_reducer() {
    pollster::block_on(async {
        let store = MemoryBackend::new();
        let time = AtomicU64::new(1);
        let clock = ticking(&time);
        let (mut host, wake) = rig(&store, Arc::new(Scripted::new([brief(&[2, 0])])), 64);
        host.open(SUBJECT).await.unwrap();
        host.begin(inputs(), &Granted, &clock, &|_| true)
            .await
            .unwrap();
        let view = settle(&mut host, &wake, &clock).await.unwrap();
        assert_eq!(view.phase, RunStatus::Completed);
        assert_eq!(view.review, Some(Review::AwaitingReview));
        // A host with a decider begins a model run: limit twice the cap.
        assert_eq!(view.limits, model_limits(64));
        let all = inputs();
        assert_eq!(
            view.proposal.unwrap().sources,
            vec![all[2].clone(), all[0].clone()]
        );
        assert_eq!(
            (
                view.usage.decisions,
                view.usage.tool_calls,
                view.usage.tokens
            ),
            (3, 2, 64)
        );
        assert_eq!(
            view.turns,
            vec![ModelTurn {
                correlation: proposing(0),
                reserved: 64,
                provider: Some("athanor.scripted".into()),
                session: None,
                tokens: 0,
                failure: None,
            }]
        );
    });
}

#[test]
fn a_recorded_proposing_step_is_never_asked_again_after_reopen_or_interruption() {
    pollster::block_on(async {
        let store = MemoryBackend::new();
        let time = AtomicU64::new(1);
        let clock = ticking(&time);
        let counted = Counted::new([Ok(brief(&[1, 2]))]);
        let (mut host, wake) = rig(&store, counted.clone(), 32);
        host.open(SUBJECT).await.unwrap();
        host.begin(inputs(), &Granted, &clock, &|_| true)
            .await
            .unwrap();
        next(&mut host, &wake, &clock).await.unwrap();
        // Step 1 is recorded, then a source is withdrawn before step 2's
        // intent (three passages: the gate's 7th call): the run pauses.
        let gate_calls = AtomicUsize::new(0);
        let gate = |_: &Passage| gate_calls.fetch_add(1, Ordering::SeqCst) < 6;
        let stopped = loop {
            match host.poll(&Granted, &clock, &gate).await {
                Ok(None) => wake.recv_timeout(WAIT).expect("the worker answers"),
                other => break other,
            }
        };
        assert_eq!(stopped, Err(Error::Unauthorized));
        let view = host.refresh().await.unwrap();
        assert_eq!(
            view.phase,
            RunStatus::Paused(Correlation {
                step: 2,
                ..proposing(0)
            })
        );
        assert_eq!(counted.calls(), 1);
        drop(host);

        // Reopened and resumed, the run goes on from step 2.
        let (mut reopened, wake) = rig(&store, counted.clone(), 32);
        reopened.open(SUBJECT).await.unwrap();
        reopened
            .control(Control::Resume, &Granted, &clock, &|_| true)
            .await
            .unwrap();
        let view = settle(&mut reopened, &wake, &clock).await.unwrap();
        assert_eq!(view.phase, RunStatus::Completed);
        assert_eq!(counted.calls(), 1, "the recorded step was asked again");

        // A cold reopen shows the model's identity and actual usage (D12).
        let cold_shop = Workshop::new(store.clone(), owner(), ID);
        let cold = cold_shop.view(Some(1)).await.unwrap();
        assert_eq!(cold.usage.tokens, 32);
        assert_eq!(
            cold.turns,
            vec![ModelTurn {
                correlation: proposing(0),
                reserved: 32,
                provider: Some("test.counted".into()),
                session: Some(Counted::session()),
                tokens: Counted::TOKENS,
                failure: None,
            }]
        );

        // A schema-2 model result recorded before binding has no session field.
        tamper(&cold_shop, &store, |run| {
            for entry in &mut run.entries {
                if let Entry::Result {
                    model: Some(model), ..
                } = entry
                {
                    model.session = None;
                }
            }
        })
        .await;
        let legacy = Workshop::new(store, owner(), ID)
            .view(Some(1))
            .await
            .unwrap();
        assert_eq!(legacy.turns[0].session, None);
        assert_eq!(legacy.turns[0].tokens, Counted::TOKENS);
        assert_eq!(legacy.proposal, cold.proposal);
        assert_eq!(counted.calls(), 1);

        // Interrupted: step 2 is in flight when the host goes away.
        let store = MemoryBackend::new();
        let counted = Counted::new([Ok(brief(&[1, 2]))]);
        let (mut host, wake) = rig(&store, counted.clone(), 32);
        host.open(SUBJECT).await.unwrap();
        host.begin(inputs(), &Granted, &clock, &|_| true)
            .await
            .unwrap();
        next(&mut host, &wake, &clock).await.unwrap();
        let view = next(&mut host, &wake, &clock).await.unwrap();
        assert_eq!(
            view.phase,
            RunStatus::Awaiting(Correlation {
                step: 2,
                ..proposing(0)
            })
        );
        drop(host);
        let (mut reopened, _wake) = rig(&store, counted.clone(), 32);
        reopened.open(SUBJECT).await.unwrap();
        let recovered = reopened.recover(&Granted, &clock).await.unwrap();
        assert_eq!(
            recovered.phase,
            RunStatus::Paused(Correlation {
                step: 2,
                attempt: 1,
                ..proposing(0)
            })
        );
        // Retrying step 2 would be its third tool call, over the run's two:
        // Servitor refuses the intent. The driver is still not asked again.
        assert_eq!(
            reopened
                .control(Control::Resume, &Granted, &clock, &|_| true)
                .await,
            Err(Error::Budget)
        );
        assert_eq!(
            reopened.refresh().await.unwrap().phase,
            RunStatus::BudgetExhausted
        );
        assert_eq!(counted.calls(), 1, "the recorded step was asked again");
    });
}

#[test]
fn a_failed_turn_is_journalled_and_retried_once() {
    pollster::block_on(async {
        let store = MemoryBackend::new();
        let time = AtomicU64::new(1);
        let clock = ticking(&time);
        let counted = Counted::new([
            Err(DecisionError::Failed("no answer".into())),
            Ok(brief(&[1, 2])),
        ]);
        let (mut host, wake) = rig(&store, counted.clone(), 64);
        host.open(SUBJECT).await.unwrap();
        host.begin(inputs(), &Granted, &clock, &|_| true)
            .await
            .unwrap();
        let view = settle(&mut host, &wake, &clock).await.unwrap();
        assert_eq!(view.phase, RunStatus::Completed);
        assert_eq!(counted.calls(), 2);
        // Each attempt counts against the decision budget and reserves.
        assert_eq!(
            (
                view.usage.decisions,
                view.usage.tokens,
                view.usage.consecutive_failures
            ),
            (4, 128, 0)
        );
        assert_eq!(
            view.turns,
            vec![
                ModelTurn {
                    correlation: proposing(0),
                    reserved: 64,
                    provider: None,
                    session: None,
                    tokens: 0,
                    failure: Some(r#"Failed("no answer")"#.into()),
                },
                ModelTurn {
                    correlation: proposing(1),
                    reserved: 64,
                    provider: Some("test.counted".into()),
                    session: Some(Counted::session()),
                    tokens: Counted::TOKENS,
                    failure: None,
                },
            ]
        );
        let all = inputs();
        assert_eq!(
            view.proposal.unwrap().sources,
            vec![all[1].clone(), all[2].clone()]
        );
    });
}

#[test]
fn a_refused_decision_and_a_second_failure_end_the_run_as_failed() {
    pollster::block_on(async {
        let store = MemoryBackend::new();
        let time = AtomicU64::new(1);
        let clock = ticking(&time);
        // A repeated passage is outside the offered choice: a refused turn.
        let counted = Counted::new([
            Ok(brief(&[0, 0])),
            Err(DecisionError::Failed("no answer".into())),
            Ok(brief(&[0, 1])),
        ]);
        let (mut host, wake) = rig(&store, counted.clone(), 64);
        host.open(SUBJECT).await.unwrap();
        host.begin(inputs(), &Granted, &clock, &|_| true)
            .await
            .unwrap();
        let view = settle(&mut host, &wake, &clock).await.unwrap();
        assert_eq!(view.phase, RunStatus::Failed);
        assert_eq!(counted.calls(), 2);
        assert!(!host.busy());
        assert!(view.proposal.is_none());
        assert_eq!(view.review, Some(Review::Generating));
        assert_eq!(view.usage.decisions, 3);
        let failures: Vec<_> = view.turns.iter().map(|t| t.failure.is_some()).collect();
        assert_eq!(failures, [true, true]);
        assert!(
            view.turns[0]
                .failure
                .as_ref()
                .unwrap()
                .starts_with("Invalid(")
        );
        assert!(view.controls.contains(&Control::Run));
        // The journal survives a cold reopen with the same disposition.
        let cold = Workshop::new(store, owner(), ID)
            .view(Some(1))
            .await
            .unwrap();
        assert_eq!((cold.phase, cold.turns), (RunStatus::Failed, view.turns));
    });
}

#[test]
fn cancelling_the_run_stops_an_in_flight_turn() {
    pollster::block_on(async {
        let store = MemoryBackend::new();
        let time = AtomicU64::new(1);
        let clock = ticking(&time);
        let (entered, turn_started) = mpsc::channel();
        let blocking = Arc::new(Blocking {
            entered: Mutex::new(entered),
            saw_cancel: AtomicBool::new(false),
        });
        let (mut host, wake) = rig(&store, blocking.clone(), 64);
        host.open(SUBJECT).await.unwrap();
        host.begin(inputs(), &Granted, &clock, &|_| true)
            .await
            .unwrap();
        next(&mut host, &wake, &clock).await.unwrap();
        turn_started.recv_timeout(WAIT).expect("the turn starts");

        let started = Instant::now();
        let view = host
            .control(Control::Revoke, &Granted, &clock, &|_| true)
            .await
            .unwrap();
        assert!(started.elapsed() < WAIT, "the turn ran to its own timeout");
        assert!(blocking.saw_cancel.load(Ordering::SeqCst));
        assert_eq!(view.phase, RunStatus::Cancelled);
        assert!(!host.busy());
        // The stopped turn is not an observation: nothing is journalled.
        assert!(view.turns.is_empty());
        assert!(view.proposal.is_none());
    });
}

#[test]
fn a_brief_not_rebuildable_from_the_owners_passages_is_refused() {
    pollster::block_on(async {
        let store = MemoryBackend::new();
        let time = AtomicU64::new(1);
        let clock = ticking(&time);
        let counted = Counted::new([Ok(brief(&[0, 1]))]);
        let (mut host, wake) = rig(&store, counted.clone(), 64);
        host.open(SUBJECT).await.unwrap();
        host.begin(inputs(), &Granted, &clock, &|_| true)
            .await
            .unwrap();
        next(&mut host, &wake, &clock).await.unwrap();
        // While step 1 is in flight the stored selection no longer holds the
        // passage the brief will cite.
        tamper(&host.workshop, &store, |run| {
            run.inputs[1] = passage("D", "1", "Elsewhere", "A passage the brief never saw.");
        })
        .await;
        assert_eq!(
            settle(&mut host, &wake, &clock).await,
            Err(Error::Invalid("worker proposal"))
        );
        let view = host.refresh().await.unwrap();
        assert!(matches!(view.phase, RunStatus::Paused(c) if c.step == 1));
        assert!(view.proposal.is_none() && view.turns.is_empty());
        assert_eq!(counted.calls(), 1);
    });
}

#[test]
fn a_confirmed_brief_that_differs_from_the_recorded_candidate_is_refused() {
    pollster::block_on(async {
        let store = MemoryBackend::new();
        let time = AtomicU64::new(1);
        let clock = ticking(&time);
        let (mut host, wake) = rig(&store, Arc::new(Scripted::new([brief(&[2, 0])])), 64);
        host.open(SUBJECT).await.unwrap();
        host.begin(inputs(), &Granted, &clock, &|_| true)
            .await
            .unwrap();
        next(&mut host, &wake, &clock).await.unwrap();
        next(&mut host, &wake, &clock).await.unwrap();
        // Step 2 is confirming the recorded [2, 0]; the record now says [0, 1].
        tamper(&host.workshop, &store, |run| {
            run.candidate = Some(proposal(ID, 1, &run.inputs[..2]).unwrap());
        })
        .await;
        assert_eq!(
            settle(&mut host, &wake, &clock).await,
            Err(Error::Invalid("worker proposal"))
        );
        let view = host.refresh().await.unwrap();
        assert!(matches!(view.phase, RunStatus::Paused(c) if c.step == 2));
        assert!(view.proposal.is_none());
        assert_eq!(view.review, Some(Review::Generating));
    });
}
