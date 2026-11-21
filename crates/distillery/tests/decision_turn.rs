// Copyright 2026 Mark Alan Boykin
// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
// SPDX-License-Identifier: MPL-2.0

//! One real decision turn (decision-driver plan, P1): Qwen3-0.6B on Burn
//! chooses among operations given as a JSON Schema, and the recorded tokens
//! replay to the same decision.
//!
//! Ignored: the checkpoint is a 1.5 GB local artifact. On the discrete GPU:
//!
//! `ESP_QWEN3_DIR=/path/to/Qwen3-0.6B cargo test -p distillery --features decision,inference-gpu --test decision_turn -- --ignored --nocapture`
//!
//! `ESP_DECISION_CPU=1` runs the same turn on the CPU backend. The test
//! asserts the turn's contract, not the quality of the model's choice.
#![cfg(all(feature = "decision", feature = "inference-gpu"))]

use distillery::decision::{decide, replay};
use esp::infer::decoder::{DecoderDevice, DecoderProvider};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::time::Instant;

const MODEL_ID: &str = "Qwen/Qwen3-0.6B";
const ARTIFACTS: &[(&str, &str)] = &[
    ("config.json", "660db3b73d788119c04535e48cf9be5f55bc3100841a718637ae695b442f27dd"),
    ("tokenizer.json", "aeb13307a71acd8fe81861d94ad54ab689df773318809eed3cbe794b4492dae4"),
    ("model.safetensors", "f47f71177f32bcd101b7573ec9171e6a57f4f4d31148d38e382306f42996874b"),
];

fn artifact(dir: &std::path::Path, name: &str, expected: &str) -> Vec<u8> {
    let bytes = std::fs::read(dir.join(name)).unwrap_or_else(|error| panic!("{name}: {error}"));
    let actual: String = Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(actual, expected, "{name} is not the pinned artifact");
    bytes
}

/// Qwen's ChatML turn with thinking closed, rendered above the seam.
fn chatml(system: &str, user: &str) -> String {
    format!(
        "<|im_start|>system\n{system}<|im_end|>\n<|im_start|>user\n{user}<|im_end|>\n\
         <|im_start|>assistant\n<think>\n\n</think>\n\n"
    )
}

#[test]
#[ignore = "needs the pinned Qwen3-0.6B checkpoint (ESP_QWEN3_DIR)"]
fn qwen3_makes_one_schema_bound_decision_that_replays() {
    let dir = PathBuf::from(std::env::var_os("ESP_QWEN3_DIR").expect("ESP_QWEN3_DIR"));
    let [config, tokenizer, weights] = [0, 1, 2].map(|at| {
        let (name, digest) = ARTIFACTS[at];
        artifact(&dir, name, digest)
    });
    let (device, placement) = if std::env::var_os("ESP_DECISION_CPU").is_some() {
        (DecoderDevice::flex(), "burn-flex".to_string())
    } else {
        let (device, facts) = distillery::inference::discrete_gpu_inference_device().unwrap();
        (device, format!("burn-wgpu on {facts}"))
    };
    let loaded = Instant::now();
    let provider =
        DecoderProvider::from_bytes(&config, &tokenizer, &weights, MODEL_ID, &placement, &device)
            .unwrap()
            .with_literal_prompt()
            .with_template_eos("<|im_end|>");
    println!("loaded on {placement} in {:.1?}", loaded.elapsed());

    let operations = json!({
        "type": "object", "additionalProperties": false, "required": ["op", "order"],
        "properties": {
            "op": {"const": "brief"},
            "order": {"type": "array", "minItems": 2, "maxItems": 3,
                      "items": {"type": "integer", "minimum": 0, "maximum": 2}}}});
    let prompt = chatml(
        "You choose which passages an evidence brief cites. Reply with one JSON object.",
        "Passage 0: Field notes\nA retained copy remains readable when its origin is offline.\n\n\
         Passage 1: Recipe card\nWhisk two eggs with a pinch of salt.\n\n\
         Passage 2: Local research note\nAcceptance finishes after the destination confirms storage.\n\n\
         Which passages are about keeping and storing documents? Give their numbers, most \
         relevant first, as {\"op\":\"brief\",\"order\":[...]}.",
    );

    let asked = Instant::now();
    let decision = decide(&provider, &prompt, &operations, 32).unwrap();
    println!(
        "decision {} from {} in {:.1?}, {} tokens",
        decision.checkpoint.text,
        decision.provider,
        asked.elapsed(),
        decision.tokens
    );
    assert_eq!(decision.value["op"], "brief");
    let order = decision.value["order"].as_array().unwrap();
    assert!((2..=3).contains(&order.len()));
    assert!(order.iter().all(|index| index.as_u64().is_some_and(|n| n <= 2)));
    assert_eq!(decision.tokens, decision.checkpoint.token_ids.len() as u64);
    assert!(decision.provider.starts_with(MODEL_ID));

    // The recorded tokens reproduce the decision without a model choice.
    assert_eq!(replay(&provider, &operations, &decision.checkpoint).unwrap(), decision);
    // Greedy generation is repeatable.
    let again = decide(&provider, &prompt, &operations, 32).unwrap();
    assert_eq!(again.checkpoint.token_ids, decision.checkpoint.token_ids);

    // A record that is cut short, or offered under other operations, is refused.
    let mut cut = decision.checkpoint.clone();
    cut.token_ids.pop();
    assert!(replay(&provider, &operations, &cut).is_err());
    let narrower = json!({"type": "object", "additionalProperties": false, "required": ["op"],
                          "properties": {"op": {"const": "stop"}}});
    assert!(replay(&provider, &narrower, &decision.checkpoint).is_err());
    // A turn too short for any decision fails rather than returning a fragment.
    assert!(decide(&provider, &prompt, &operations, 3).is_err());
}
