# Inkling body: Inkling 2 Mini as a second decoder body in esp

**Status (2026-11-20):** plan. Nothing here is built. The gap below was read
from the model's `config.json` and tensor names, not from the reference
forward code; P0 closes that before anything is written.

**For:** running `thinkingmachines/Inkling-2-Mini` (Apache-2.0, 26.4B
parameters, released 2026-11-20) in esp on Burn, through the existing
provider seam, so the decision driver
([2026-11-20_decision_driver_plan.md](2026-11-20_decision_driver_plan.md))
can select it.

**Not here:** the agent itself (decision-driver plan); fine-tuning Inkling;
any change to the llama-family body.

## Origin

Mark, 2026-11-20: the goal is the model running in the stack, not only a
harness test against an outside runtime.

## Decisions

- **D1. The model in the stack (Mark, 2026-11-20):** asked whether the goal
  was testing a harness with this model or getting the model running in the
  stack: "The goal is the latter".

  Links: summary Inkling 2 Mini is to run in esp on Burn, not only behind an outside runtime.

- **D2. Weights may be fetched (Mark, 2026-11-20):** "we can download and use
  the model freely".

  Links: summary The Apache-2.0 checkpoint and its quantized derivatives may be downloaded and used.

- **D3. The whole checkpoint (Mark, 2026-11-20):** "Everything in the
  checkpoint", over text only and over text plus vision.

  Links: summary The port covers the text decoder, the vision encoder and projector, and the multi-token-prediction head.

- **D4. The laptop, and the mesh (Mark, 2026-11-20):** asked which machine it
  runs on, with the laptop as option 1 and the mesh as option 3: "1, but also
  3!"

  Links: summary The model must run on the Windows laptop and also split across devices through the mesh host.

D5–D8 answer four recommendations put on 2026-11-20, in one answer: "I accept
your recommendations".

- **D5. Port from transformers (Mark, 2026-11-20):** accepted as recommended:
  the transformers implementation is the source and the exact-weights parity
  reference, run on CPU; llama.cpp is a sanity check only.

  Links: summary The transformers implementation is the port source and parity reference; llama.cpp GGUFs are a sanity check.

- **D6. Read NVFP4 and MXFP8 directly (Mark, 2026-11-20):** accepted as
  recommended, over converting at import into esp's block formats. F6 found
  afterwards bears on this; see open fork 1.

  Links: summary esp reads the checkpoint's own quantization rather than requantizing it at import.

- **D7. Text, laptop GPU, vision, prediction head, mesh (Mark, 2026-11-20):**
  accepted as recommended; the order of the phases below.

  Links: summary The port lands in the order text decoder, laptop GPU, vision, MTP head, mesh split.

- **D8. Stream experts first, mesh second (Mark, 2026-11-20):** accepted as
  recommended: attention, norms and the router on the GPU, experts in RAM.

  Links: summary The 8 GB fit is tried first by streaming experts from RAM, then by a device split.

- **D9. NVFP4/MXFP8 and GGUF readers, both now (Mark, 2026-11-20):** "Both
  now", over building the NVFP4/MXFP8 reader first and measuring a GGUF
  import before deciding. Asked as part of aligning esp with outside
  standards; the GGUF reader serves every family esp runs, not only Inkling.

  Links: summary esp gains import readers for NVFP4/MXFP8 and for GGUF together; Burn's scheme stays the executed layout.

No `doctrine.md` exists in this repository yet, so these are recorded as
cases without a statement to file them under (DOC_POLICY §11).

## Open forks

1. **What "directly" costs (reopens D6, from F6).** esp's 4-bit format is
   Burn's own integer scheme, which Burn's kernels execute. NVFP4 is not that
   scheme, so reading it without requantizing needs one of: decoding to half
   floats at load (about 53 GB for the whole model, so only per expert as it
   streams); a new quantized kernel in Burn's backend; or accepting a second
   quantization into Burn's scheme after all. P0 settles the format's exact
   layout before this is put back to Mark.
2. **The transformers licence.** Unconfirmed; to be stated before a lane is
   briefed to read it.

## Phases

### P0. Read the reference, fix the gap list
Read the chosen reference's forward pass and replace every "inferred from
names" row in F2 with what the code does.
- Done when: each part in F2 has its exact computation written in Findings
  with a reference location, and anything F2 missed is added.
- Needs: fork 2.

### P1. Config, tokenizer and weights load
The nested config parses into its own config type; the tokenizer loads; every
tensor in the checkpoint is read or refused by name.
- Done when: the checkpoint loads with no unread tensor, and a tokenizer
  round trip matches the reference on a fixed text set.
- Needs: P0, fork 1.

### P1g. GGUF reader (D9)
A GGUF import path for the families esp already runs, beside the safetensors
one. Independent of the Inkling body; it shares P1's quantized-import seam.
- Done when: a published Qwen3 GGUF loads and generates, its tensors are read
  or refused by name, and its output is compared with the same model through
  esp's own conversion, the difference recorded.
- Needs: fork 1, since both readers land on the same question of what Burn
  executes.

### P2. Text decoder, CPU parity
The looped text decoder on the CPU backend: relative-position attention with
its sink, gate, clips and convolutions; the residual attention blocks; the
128-expert router; three passes over layers 6–18; the final soft-cap.
- Done when: next-token logits match the reference within a stated tolerance
  on fixed prompts, and greedy continuations match token for token.
- Needs: P1.

### P3. Text decoder on the laptop GPU, as a provider
The same body on wgpu within 8 GB of VRAM, wired to `InferenceProvider`.
- Done when: the P2 parity checks pass on the GPU, peak VRAM is recorded, a
  durable request replays, and the decision driver can select the provider.
- Needs: P2.

### P4. Vision input
The 23-layer vision encoder and projector, and an image input path in esp.
- Done when: image-and-text prompts match the reference's next-token logits
  within the P2 tolerance.
- Needs: P3.

### P5. The multi-token-prediction head
`mtp.safetensors` loaded and used as the reference uses it.
- Done when: its outputs match the reference, and what it buys (tokens per
  second, or nothing at inference) is measured and recorded.
- Needs: P3.

### P6. Split across devices
The model run through Distillery's mesh host across two devices.
- Done when: a two-device run produces the same tokens as the single-device
  run, receipted.
- Needs: P3.

## Findings

- **F1 (2026-11-20).** The checkpoint is one 19,526,019,168-byte safetensors
  file plus a 635,801,416-byte `mtp.safetensors`, with `tokenizer.json`, a
  tiktoken model and a chat template. The card states Apache-2.0, a 1M-token
  context window, and recommends 32 GB or more of VRAM.
- **F2 (2026-11-20).** Gap against esp's llama-family body, from
  `config.json` and tensor names (inferred where marked):
  - Config: nested `text_config`, `vision_config`, `mtp_config`. esp refuses
    a nested `text_config` and any `model_type` outside its list
    (`crates/esp/src/infer/decoder/config.rs`, `refuse_foreign`).
  - Positions: no RoPE in the text decoder. Learned relative position,
    `d_rel` 16, `rel_extent` 1024 (`wr_du`, `rel_logits_proj`). Inferred.
  - Attention: 24 heads, 8 key-value heads, head width 128; 20 of 24 layers
    are local with a 1024-token window. Adds L2 clips on queries and
    relative terms, an attention sink projection, an attention gate, value
    normalization, and kernel-4 convolutions on keys, values and the
    attention and MLP inputs (`k_sconv`, `v_sconv`, `attn_sconv`,
    `mlp_sconv`). Inferred.
  - Residual: `attn_res` holds learned queries per layer and an output
    query, in blocks of 6 layers. Inferred.
  - Experts: 128 routed, 8 per token, no shared expert, width 1152, gate
    bias, route scale 8, soft-caps on the gate and up projections, fused
    `w13` and `w2` tensors. esp's `moe.rs` implements Qwen2 routed plus
    shared experts.
  - Loop: layers 6–18 run 3 passes with a rank-64 gate; 48 layer
    applications per token.
  - Logits: final soft-cap 30. esp refuses `final_logit_softcapping`.
  - Vision: 23 layers, width 1024, patch 20, temporal patch 2, projector
    width 4096. esp has no image path.
  - MTP: 3 layers, block size 8, Markov rank 256, reading layers 5, 23, 41
    and 48. Its role at inference is unread.
- **F3 (2026-11-20).** Weights are NVFP4 and MXFP8 (`quantization_config`,
  `hf_quant_config.json`), with `_scale`, `_amax` and
  `_outer_scale_denominator` tensors beside expert weights. esp's
  `WeightFormat` is F32, F16, Bf16, Q8Block32 and Q4Block32
  (`crates/esp/src/infer/decoder/weight_format.rs`).
- **F4 (2026-11-20).** What carries over: the safetensors loader, streamed
  expert reads, the KV cache, sampling, constrained decoding and the
  provider seam. Constrained decoding needs a ByteLevel decoder
  (`crates/esp/src/infer/decoder/constraint.rs:123`); Inkling's tokenizer
  type is unchecked.
- **F5 (2026-11-20).** The laptop has an RTX 4060 Laptop GPU with 8188 MiB
  of VRAM and 31.2 GB of RAM. GGUF derivatives exist from 7.8 GB (IQ1_S) to
  22.9 GB (Q8_K_XL), 16.6 GB at Q4_K_M (`unsloth/Inkling-2-Mini-GGUF`);
  esp does not read GGUF.

- **F6 (2026-11-20).** esp's `Q4Block32` and `Q8Block32` are Burn's
  `QuantScheme` (`QuantValue::Q4S` / `Q8S`, `QuantStore::PackedU32`, blocks
  of 32 with an f32 scale), made at import by per-block absolute-maximum
  rounding with no calibration data, and stored as U8 tensors inside
  safetensors (`crates/esp/src/infer/decoder/weight_format.rs`, `scheme`,
  `convert_artifact`). No design document records why this was chosen over
  an outside format; the commit that added it (`68aebeb`) has no body.

## Progress

- **2026-11-20:** plan written from the model's config, tensor index and
  card, the tree at `0e2c7ff`, and Mark's answers. No code changed.
- **2026-11-20:** D5–D8 recorded. D6 reopened as fork 1 by F6.
- **2026-11-20:** D9 recorded; P1g added for the GGUF reader.
