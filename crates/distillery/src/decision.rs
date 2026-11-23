// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! One model decision over an esp provider (agent harness brief, section 2;
//! decision-driver D4, D7, D10).
//!
//! The offered operations are one JSON Schema. It is compiled into the
//! provider's output grammar, so the model can only produce a decision the
//! schema admits; the text is then parsed and checked against the schema
//! again before it is returned. Generation is greedy, and the recorded token
//! IDs replay to the same decision. Chat templating happens above this seam:
//! `prompt` is already rendered. This module knows nothing of who asks or of
//! what a decision authorizes.

use esp::infer::provider::GenerationCheckpoint;
use esp::infer::schema::{grammar_from_schema, validate_against_schema};
use esp::infer::{GenerationRequest, InferError, InferenceProvider};
use serde_json::Value;

/// A decision and the record that reproduces it.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelDecision {
    /// The decision, parsed and checked against the offered operations.
    pub value: Value,
    /// The provider's model and loader, as its capability names them.
    pub provider: String,
    /// Tokens the model generated, forced syntax included.
    pub tokens: u64,
    /// Request, prompt IDs and generated IDs; enough to replay the turn.
    pub checkpoint: GenerationCheckpoint,
}

fn request(
    prompt: &str,
    operations: &Value,
    max_tokens: usize,
) -> Result<GenerationRequest, InferError> {
    Ok(GenerationRequest {
        prompt: prompt.into(),
        context: String::new(),
        output_grammar: Some(grammar_from_schema(operations)?),
        max_tokens,
        temperature: 0.0,
        top_p: None,
        seed: None,
        stop: Vec::new(),
    })
}

fn finish(
    provider: &dyn InferenceProvider,
    operations: &Value,
    checkpoint: GenerationCheckpoint,
) -> Result<ModelDecision, InferError> {
    if !checkpoint.complete {
        return Err(InferError::InvalidRequest(
            "the decision did not finish within max_tokens".into(),
        ));
    }
    let value: Value = serde_json::from_str(&checkpoint.text)
        .map_err(|error| InferError::InvalidRequest(format!("decision is not JSON: {error}")))?;
    validate_against_schema(operations, &value)?;
    let capability = provider.capability();
    Ok(ModelDecision {
        value,
        provider: format!("{} via {}", capability.model_id, capability.loader),
        tokens: checkpoint.token_ids.len() as u64,
        checkpoint,
    })
}

/// Ask `provider` for one decision among `operations`.
///
/// # Errors
/// When the schema is outside the supported subset, the provider refuses
/// constrained or checkpointed generation, generation fails, or the decision
/// does not finish within `max_tokens`.
pub fn decide(
    provider: &dyn InferenceProvider,
    prompt: &str,
    operations: &Value,
    max_tokens: usize,
) -> Result<ModelDecision, InferError> {
    decide_until(provider, prompt, operations, max_tokens, &|| false)
}

/// [`decide`], stopping at a token boundary once `stop` returns true. A
/// stopped turn is [`InferError::Interrupted`], never a fragment.
///
/// # Errors
/// As [`decide`], and when `stop` ends generation before the decision is
/// complete.
pub fn decide_until(
    provider: &dyn InferenceProvider,
    prompt: &str,
    operations: &Value,
    max_tokens: usize,
    stop: &dyn Fn() -> bool,
) -> Result<ModelDecision, InferError> {
    let request = request(prompt, operations, max_tokens)?;
    let mut stopped = false;
    let checkpoint =
        provider.generate_checkpointed(&request, None, &mut |_| Ok(()), &mut || {
            stopped = stopped || stop();
            stopped
        })?;
    if stopped && !checkpoint.complete {
        return Err(InferError::Interrupted);
    }
    finish(provider, operations, checkpoint)
}

/// Why a [`ModelDecider`] produced no decision, in terms a caller that does
/// not name esp can act on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecideError {
    /// The operations or the request are outside what this model path serves.
    Unsupported(String),
    /// The model's output is not a decision among the offered operations.
    Invalid(String),
    /// Generation failed or did not finish.
    Failed(String),
}

impl From<InferError> for DecideError {
    fn from(error: InferError) -> Self {
        match error {
            InferError::Unsupported { .. } => Self::Unsupported(error.to_string()),
            InferError::InvalidRequest(_) => Self::Invalid(error.to_string()),
            _ => Self::Failed(error.to_string()),
        }
    }
}

mod binding;
pub use binding::{ChatPrompt, ModelDecider, chat_prompt};

/// Load a decoder from its config, tokenizer and safetensors bytes onto the
/// first discrete GPU and bind it to `render`. The rendered prompt is taken
/// literally; `template_eos` names an end-of-turn token the base config
/// lacks. The placement the runtime bound is part of the provider identity.
///
/// # Errors
/// When no discrete adapter would be bound or the decoder does not load.
#[cfg(feature = "inference-gpu")]
pub fn discrete_gpu_decider(
    config: &[u8],
    tokenizer: &[u8],
    weights: &[u8],
    model_id: &str,
    template_eos: Option<&str>,
    render: ChatPrompt,
) -> Result<ModelDecider, DecideError> {
    use esp::infer::decoder::DecoderProvider;
    let (device, facts) =
        crate::inference::discrete_gpu_inference_device().map_err(DecideError::Unsupported)?;
    let placement = format!("burn-wgpu on {facts}");
    let provider =
        DecoderProvider::from_bytes(config, tokenizer, weights, model_id, placement, &device)?
            .with_literal_prompt();
    let provider = match template_eos {
        Some(eos) => provider.with_template_eos(eos),
        None => provider,
    };
    let manifest = serde_json::to_vec(&[
        eidetic::ManifestId::of_blob(config),
        eidetic::ManifestId::of_blob(weights),
    ])
    .map_err(|error| DecideError::Failed(error.to_string()))?;
    ModelDecider::bind(
        provider,
        eidetic::ManifestId::of_blob(&manifest),
        eidetic::ManifestId::of_blob(tokenizer),
        render,
    )
}

/// Re-derive a decision from its recorded checkpoint. The provider prefills
/// the recorded prompt and token IDs and checks them against the grammar; no
/// model choice is made again.
///
/// # Errors
/// When the checkpoint does not belong to `operations`, is incomplete, or its
/// tokens violate the grammar or decode to different text.
pub fn replay(
    provider: &dyn InferenceProvider,
    operations: &Value,
    recorded: &GenerationCheckpoint,
) -> Result<ModelDecision, InferError> {
    if recorded.session != provider.session_identity() {
        return Err(InferError::InvalidRequest(
            "checkpoint belongs to a different exact session".into(),
        ));
    }
    if !recorded.complete {
        return Err(InferError::InvalidRequest(
            "incomplete decision checkpoint".into(),
        ));
    }
    let request = request(
        &recorded.request.prompt,
        operations,
        recorded.request.max_tokens,
    )?;
    let checkpoint =
        provider.generate_checkpointed(&request, Some(recorded), &mut |_| Ok(()), &mut || false)?;
    finish(provider, operations, checkpoint)
}

#[cfg(test)]
mod tests {
    use super::*;
    use esp::infer::ModelCapability;
    use esp::infer::provider::GenerationCheckpoint;
    use serde_json::json;
    use std::cell::Cell;
    use std::ops::ControlFlow;
    use std::sync::Mutex;

    /// Emits a fixed decision one byte per token and consults `stop` before
    /// each token, as a checkpointing provider does.
    struct Typist {
        capability: ModelCapability,
        emitted: Mutex<usize>,
    }
    impl Typist {
        const TEXT: &str = r#"{"op":"brief"}"#;
        fn new() -> Self {
            Self {
                capability: ModelCapability {
                    model_id: "typist".into(),
                    context_window: 64,
                    quantization: None,
                    loader: "test".into(),
                    streaming: false,
                },
                emitted: Mutex::new(0),
            }
        }
    }
    impl InferenceProvider for Typist {
        fn capability(&self) -> &ModelCapability {
            &self.capability
        }
        fn generate_streaming(
            &self,
            _: &GenerationRequest,
            _: &mut dyn FnMut(&str) -> ControlFlow<()>,
        ) -> Result<String, InferError> {
            Err(InferError::ModelNotLoaded)
        }
        fn generate_checkpointed(
            &self,
            request: &GenerationRequest,
            resume: Option<&GenerationCheckpoint>,
            commit: &mut dyn FnMut(&GenerationCheckpoint) -> Result<(), InferError>,
            stop: &mut dyn FnMut() -> bool,
        ) -> Result<GenerationCheckpoint, InferError> {
            if let Some(checkpoint) = resume {
                assert_eq!(checkpoint.request, *request);
                return Ok(checkpoint.clone());
            }
            let mut checkpoint = GenerationCheckpoint {
                placements: Vec::new(),
                session: None,
                request: request.clone(),
                prompt_ids: vec![0],
                token_ids: Vec::new(),
                draws: 0,
                text: String::new(),
                complete: false,
            };
            for byte in Self::TEXT.bytes() {
                if stop() {
                    break;
                }
                checkpoint.token_ids.push(u32::from(byte));
                checkpoint.text.push(char::from(byte));
            }
            *self.emitted.lock().unwrap() = checkpoint.token_ids.len();
            checkpoint.complete = checkpoint.text == Self::TEXT;
            commit(&checkpoint)?;
            Ok(checkpoint)
        }
    }

    fn operations() -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["op"],
            "properties": {"op": {"const": "brief"}},
        })
    }

    #[test]
    fn replay_binds_source_bytes_even_when_a_template_edit_renders_identically() {
        use eidetic::{Hash, ManifestId};
        let source = "{#A#}{{ messages[1].content }}";
        let edited = "{#B#}{{ messages[1].content }}";
        let make = |source: &str| {
            let config = serde_json::to_vec(&json!({"chat_template":source})).unwrap();
            let prompt = chat_prompt(&config, "system".into(), "ask".into(), json!({})).unwrap();
            assert_eq!(prompt.render("context").unwrap(), "context\nask");
            ModelDecider::bind(
                Typist::new(),
                ManifestId::of_blob(b"base"),
                ManifestId::of_blob(b"tokenizer"),
                prompt,
            )
            .unwrap()
        };
        let original = make(source);
        let decision = original.decide("context", &operations(), 32).unwrap();
        let session = decision.checkpoint.session.as_ref().unwrap();
        assert_eq!(
            session.prompt_template_hash,
            Hash::of(source.as_bytes()).to_string()
        );
        assert_eq!(
            make(source)
                .replay(&operations(), &decision.checkpoint)
                .unwrap(),
            decision
        );
        let changed = make(edited);
        assert!(
            matches!(changed.replay(&operations(), &decision.checkpoint),
            Err(DecideError::Invalid(message)) if message.contains("session"))
        );
        let mut altered = decision.checkpoint.clone();
        altered.session.as_mut().unwrap().prompt_template_hash =
            Hash::of(edited.as_bytes()).to_string();
        assert!(original.replay(&operations(), &altered).is_err());
        altered.session = None;
        assert!(original.replay(&operations(), &altered).is_err());
    }

    #[test]
    fn raw_replay_refuses_a_checkpoint_from_a_bound_session() {
        let provider = Typist::new();
        let mut recorded = decide(&provider, "p", &operations(), 32)
            .unwrap()
            .checkpoint;
        recorded.session = Some(esp::infer::provider::SessionIdentity {
            session_id: "other session".into(),
            prompt_template_hash: "other template".into(),
        });
        assert!(matches!(replay(&provider, &operations(), &recorded),
            Err(InferError::InvalidRequest(message)) if message.contains("session")));
    }

    #[test]
    fn a_stopped_turn_fails_at_a_token_boundary_instead_of_returning_a_fragment() {
        let typist = Typist::new();
        let decided = decide(&typist, "p", &operations(), 32).unwrap();
        assert_eq!(decided.value, json!({"op": "brief"}));
        assert_eq!(decided.tokens, Typist::TEXT.len() as u64);

        let asked = Cell::new(0);
        let stop = || {
            asked.set(asked.get() + 1);
            asked.get() > 3
        };
        assert_eq!(
            decide_until(&typist, "p", &operations(), 32, &stop),
            Err(InferError::Interrupted)
        );
        assert_eq!(*typist.emitted.lock().unwrap(), 3);
    }
}
