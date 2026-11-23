// Copyright 2026 Mark Alan Boykin
// SPDX-License-Identifier: MPL-2.0

//! Local, single-file or indexed Hugging Face checkpoints for decision trials.
//! Each admitted layer is immutable for the lifetime of the session. A session
//! identifies the exact config and ordered tensor payloads consumed by ESP.

use crate::decision::{ChatPrompt, DecideError, ModelDecider};
use eidetic::ManifestId;
use esp::infer::InferError;
use esp::infer::decoder::{
    DecoderConfig, DecoderProvider, WeightFormat,
    checkpoint::CheckpointReader,
    streaming::{LayerSource, StreamingOptions},
    weight_format::convert_artifact,
};
use serde::Serialize;
use std::{path::Path, sync::Arc};

struct Layers(Vec<Arc<Vec<u8>>>);

impl LayerSource for Layers {
    fn read_layer(&self, index: usize, limit: usize) -> Result<Vec<u8>, InferError> {
        self.read_layer_shared(index, limit)
            .map(|bytes| (*bytes).clone())
    }

    fn read_layer_shared(&self, index: usize, limit: usize) -> Result<Arc<Vec<u8>>, InferError> {
        self.0
            .get(index)
            .filter(|b| b.len() <= limit)
            .cloned()
            .ok_or_else(|| {
                InferError::InvalidWeights("absent or oversized checkpoint layer".into())
            })
    }
}

/// Device placement requested for an offline comparison.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Placement {
    /// Expand the checkpoint to F32 and retain all layers on the device.
    ResidentF32,
    /// Keep verified host layers and up to 2 GiB of cached F32 device layers.
    StreamedF32,
    /// Convert to ESP's block-32 Q8 weights and retain all layers on the device.
    ResidentQ8,
}

/// Immutable identities and actual placement used for a measured model load.
#[derive(Serialize)]
pub struct CheckpointReceipt {
    /// Content identity of the original HF model configuration.
    pub config: ManifestId,
    /// Content identity of the tokenizer used by the decoder.
    pub tokenizer: ManifestId,
    /// Content identity of the shared tensors in the selected format.
    pub shared: ManifestId,
    /// Content identities of the consumed layer artifacts in execution order.
    pub layers: Vec<ManifestId>,
    /// Precision and residency selected for this process.
    pub placement: Placement,
    /// Session's base-model reference derived from config, shared weights and layers.
    pub model_ref: ManifestId,
    /// Name, graphics backend and class of the bound discrete device.
    pub device: String,
}

/// Assemble one decoder on a discrete GPU and bind its content identity to the
/// prompt. Timing this operation measures loading separately from generation.
///
/// # Errors
/// Returns an error if checkpoint files, execution format, GPU placement or
/// session binding cannot serve the requested model.
pub fn load_checkpoint(
    directory: &Path,
    model_id: &str,
    placement: Placement,
    prompt: ChatPrompt,
) -> Result<(ModelDecider, CheckpointReceipt), DecideError> {
    let read = |name: &str| {
        std::fs::read(directory.join(name)).map_err(|e| DecideError::Failed(format!("{name}: {e}")))
    };
    let config_bytes = read("config.json")?;
    let tokenizer_bytes = read("tokenizer.json")?;
    let cfg = DecoderConfig::from_json_bytes(&config_bytes)?;
    if cfg.moe.is_some() {
        return Err(DecideError::Unsupported(
            "decision comparison accepts dense checkpoints".into(),
        ));
    }
    let (device, facts) =
        crate::inference::discrete_gpu_inference_device().map_err(DecideError::Unsupported)?;
    let format = match placement {
        Placement::ResidentQ8 => WeightFormat::Q8Block32,
        _ => WeightFormat::F32,
    };
    format.validate_device(&device)?;
    let prepare = |bytes: Vec<u8>| match placement {
        Placement::ResidentQ8 => convert_artifact(&bytes, format),
        _ => Ok(bytes),
    };
    let options = StreamingOptions::default();
    let mut reader = CheckpointReader::open(directory, &cfg)?;
    let shared_bytes = prepare(reader.shared_bytes(options.max_shared_bytes)?)?;
    let shared = ManifestId::of_blob(&shared_bytes);
    let mut layers = Vec::with_capacity(cfg.num_hidden_layers);
    let mut layer_ids = Vec::with_capacity(cfg.num_hidden_layers);
    for index in 0..cfg.num_hidden_layers {
        let bytes = prepare(reader.layer_bytes(index, options.max_layer_bytes)?)?;
        layer_ids.push(ManifestId::of_blob(&bytes));
        layers.push(Arc::new(bytes));
    }
    let config = ManifestId::of_blob(&config_bytes);
    let tokenizer = ManifestId::of_blob(&tokenizer_bytes);
    let manifest = serde_json::to_vec(&("decision-checkpoint/v1", config, shared, &layer_ids))
        .map_err(|e| DecideError::Failed(e.to_string()))?;
    let model_ref = ManifestId::of_blob(&manifest);
    let provider = DecoderProvider::from_layer_source_with_format(
        &config_bytes,
        &tokenizer_bytes,
        &shared_bytes,
        Box::new(Layers(layers)),
        options,
        model_id,
        format!("burn-wgpu {placement:?} on {facts}"),
        &device,
        format,
    )?;
    let provider = match placement {
        Placement::StreamedF32 => provider.with_layer_cache(2 * 1024 * 1024 * 1024)?,
        _ => provider.with_resident_layers()?,
    };
    let provider = provider
        .with_literal_prompt()
        .with_template_eos(prompt.eos());
    let decider = ModelDecider::bind(provider, model_ref, tokenizer, prompt)?;
    Ok((
        decider,
        CheckpointReceipt {
            config,
            tokenizer,
            shared,
            layers: layer_ids,
            placement,
            model_ref,
            device: facts.to_string(),
        },
    ))
}
