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

fn request(prompt: &str, operations: &Value, max_tokens: usize) -> Result<GenerationRequest, InferError> {
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
    let request = request(prompt, operations, max_tokens)?;
    let checkpoint =
        provider.generate_checkpointed(&request, None, &mut |_| Ok(()), &mut || false)?;
    finish(provider, operations, checkpoint)
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
    let request = request(&recorded.request.prompt, operations, recorded.request.max_tokens)?;
    let checkpoint =
        provider.generate_checkpointed(&request, Some(recorded), &mut |_| Ok(()), &mut || false)?;
    finish(provider, operations, checkpoint)
}
