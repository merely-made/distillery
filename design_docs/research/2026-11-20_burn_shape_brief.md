# Burn shape: how esp, Distillery and the workshop sit on Burn

**Date:** 2026-11-20
**Status:** direction brief, endorsed by Mark the same day (D1). Nothing here
is built; the two plans it feeds carry the work.

**Extends** the [Burn utilization brief](2026-07-04_burn_utilization_brief.md),
whose commitment 2 ("Burn stays behind the seams") this brief keeps and makes
concrete for a second decoder architecture, outside weight formats and an
agent. **Feeds** the
[decision-driver plan](../implementation_strategy/2026-11-20_decision_driver_plan.md)
and the [Inkling body plan](../implementation_strategy/2026-11-20_inkling_body_plan.md).

## Decisions

- **D1. The shape (Mark, 2026-11-20):** asked "What do you think is the best
  way to shape esp, distillery, alembic, etc. to use burn?", given the five
  statements below, and asked whether to write them up: "Yes".

  Links: draws on mere:house-style D12 · plans decision-driver, inkling-body · summary Only esp touches Burn; bodies split from generation services; one executed weight layout with importers; Distillery places work and splits by stages; no custom Burn kernels.

## The five statements

1. **Only esp touches Burn.** Outside esp, Burn is named in one file,
   `crates/distillery/src/remote.rs`, the lease-bound Burn Remote resource.
   Athanor and Alembic see neither Burn nor esp; they see a decision-driver
   trait (decision-driver D5).

2. **Inside esp, a body is separate from the generation services.** A body is
   one architecture: its config, its forward pass, its cache, the tensors it
   reads. The services are sampling, the output grammar, checkpoints,
   streaming and the provider. Today sampling, the grammar and streaming do
   not name the llama body at all; `generate`, `provider`, `stage`,
   `checkpoint`, `lora` and `train` name its types 54 times between them
   (F1). A body trait goes in before the Inkling body is written, so a second
   architecture does not copy the provider.

3. **One weight layout is executed; outside formats are imported.** What
   runs is Burn's quantized tensor scheme (inkling-body F6). NVFP4, MXFP8 and
   GGUF are readers at the edge (inkling-body D9). The same rule holds for
   every other seam: Hugging Face inputs, JSON Schema for operations
   (decision-driver D7), the model's chat template where one is selected
   (decision-driver D8). Standards at the edges, own formats inside only
   where replay, durability or Burn's kernels require them.

4. **Distillery decides where and when, never how.** It owns the GPU gate
   (`gpu.rs`), the leases and layer-stage work across the mesh
   (`inference_stage.rs`, esp's `StageExecutor`). A model is split across
   devices by stages, which hand work over at layer boundaries. Burn Remote
   is for running a whole model on another machine, since it carries every
   tensor operation over the network.

5. **Stay close to Burn's releases; write no custom kernels.** The tree is on
   0.23.0-pre.2 with exact pins and a patched `burn-remote`, and each Burn
   move has been costly. A weight format Burn cannot execute is decoded at
   the edge rather than given a kernel of ours. For Inkling's NVFP4 experts
   this means decoding each expert to half floats as it streams, which is the
   proposed answer to inkling-body fork 1; it is unmeasured.

## What Burn buys and costs

Buys: one codebase for browser, desktop and mesh, and training in the same
stack. Costs: each new architecture is ported by hand, and quantization
trails llama.cpp. Speed against llama.cpp is unmeasured here. The cost is why
statement 2 matters: a body has to be cheap to add.

The browser half of "buys" held on the cohort the tree is on only after a
fork: CubeCL 0.12.0-pre.2 does not compile for wasm (F3), and nobody saw it
for eight days. Statement 5 has a price when the tree sits on a prerelease,
and a browser build belongs among the checks run at every Burn move.

## Outside reference: lorasmith

`RegaLabs/lorasmith` (MIT OR Apache-2.0; first commit 2026-11-09, one author,
44 commits, version 0.1.1; read 2026-11-20) fine-tunes Llama, Mistral, Qwen2
and Qwen3 models with LoRA and QLoRA on Burn 0.23.0-pre.2, the cohort esp is
on. It is a training and export tool with no inference provider, no
mixture-of-experts and no GGUF reader. What it shows that bears on this
repository:

- **The 4-bit layout.** Its QLoRA uses the scheme esp uses: `QuantValue::Q4S`
  in blocks of 32 with an f32 scale. Its source states the reason for 32 by
  measurement (one scale per projection takes Qwen2.5-0.5B's loss from 5.1 to
  19.9; blocks of 32 keep it at 5.1) and calls it "the layout of GGUF's
  Q4_0". If that holds, GGUF's Q4_0 and Q8_0 import into Burn's scheme
  nearly exactly, and the K-quant files do not. Not checked against the GGUF
  specification here.
- **Two Burn changes it depends on.** A factored LoRA forward
  (tracel-ai/burn#6465) and a quantized matmul that autodiff keeps packed
  (tracel-ai/burn#6503); it pins a personal fork of Burn carrying both.
  Whether esp's autodiff trainer over a 4-bit base needs either is unchecked.
- **Chat templates through minijinja**, with `minijinja-contrib`'s Python
  compatibility, rendering a model's own template as transformers does. The
  renderer decision-driver D8 names.
- **A GGUF writer** (Q8_0, vocabulary and pretokenizer tables, SentencePiece),
  the mirror of the reader inkling-body P1g needs.
- **Parity by tiny fixtures**: small checkpoints per family with expected
  outputs generated by the reference implementation.

It is a reference to read, not a dependency: it is a binary, it overlaps esp,
and it is eleven days old. Whether any of its code is taken, with
attribution, is Mark's to rule.

## Findings

- **F1 (2026-11-20).** References to the llama body's types
  (`DecoderModel`, `DecoderLayer`, `DecoderConfig`, `LayerKvCache`,
  `DecoderAttention`) under `crates/esp/src/infer/decoder/`: `stage.rs` 15,
  `provider.rs` 12, `generate.rs` 10, `lora.rs` 7, `train.rs` 7,
  `checkpoint.rs` 3; `constraint.rs`, `streaming.rs` and `sample.rs` 0.
- **F2 (2026-11-20).** `crates/distillery/src/inference.rs:141` and
  `crates/distillery/src/inference_stage/lane.rs:106` label a flex device
  "burn-ndarray". Burn 0.23 has no ndarray backend, so the recorded backend
  name is wrong.

- **F3 (2026-11-20).** `cubecl-runtime` 0.12.0-pre.2 fails to compile for
  `wasm32-unknown-unknown` (`tune/tuner.rs:25` imports a function from a
  module `tune/mod.rs` compiles only off wasm). Carried by the
  `mark-ik/cubecl` fork; with it the browser probe passes headed
  (2026-11-21). Detail is in the models plan's Findings of both dates.

## Progress

- **2026-11-20:** written after Mark's endorsement. No code changed.
- **2026-11-20:** F3 added after the browser probe failed to compile.
- **2026-11-22:** statement 1 holds as built: with `workshop-model`, Athanor
  names Distillery in one file (`crates/athanor/src/decision_model.rs`) and
  names neither esp nor Burn; Distillery knows nothing of Athanor
  (decision-driver D10). Statement 2's body trait is not started.
