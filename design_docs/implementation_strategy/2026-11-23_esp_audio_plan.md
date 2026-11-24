# ESP audio plan

**Date:** 2026-11-23
**Status:** A1 landed 2026-11-23 on CPU fixtures; A1 GPU parity, A2 and A3 open.
**Source:** `mere/design_docs/2026-11-22_burn_ecosystem_survey_brief.md`, §8.
Mark ruled there that ESP gets an `audio` namespace beside `infer` and `embed`
(later `image` and `label` too), that third-party weights are fetched by the
person and never bundled, and the lift order below.

## Rules carried from the brief

- Port on the plain-Burn path. Watershed's CubeCL kernels and hand-tuned matmul
  table stay out until the plain path is measured on the family's machines.
- Harvest bunsen by kit; do not depend on it.
- ESP takes the caller's `Device` and bundles no checkpoints. A row records the
  checkpoint's origin, hash and license, or that it declares none.

## A1. BS-RoFormer separation (`esp::audio::separation`)

Landed: Watershed's `crates/roformer` at `1046832`, with its STFT and chunked
overlap-add, behind the `audio-separation` feature (`audio-separation-wgpu`
adds the wgpu backend). Its kernel module is replaced by `plain.rs`, which keeps
the names the layers call and declines every accelerated path, so the layers
take the fallback Watershed already tested. The `MAX_SCORES` attention slicing
stays. Ported from Burn 0.22.1 to this workspace's 0.23.0-pre.2; the only API
change met was safetensors 0.4's `serialize_to_file`. Provenance and the MIT
notices are in `crates/esp/src/audio/separation/NOTICE.md`.

Evidence, 2026-11-23, `esp-audio` build lane: `separation_parity` passes 7
CPU cases on `Device::flex()` (tiny random-weight model against PyTorch per
layer, STFT, one chunk, whole songs against `demix`, cancellation, f16 on quiet
input, converted weights bit-exact). The library's 17 `audio` unit tests pass.
Clippy with `audio-separation-wgpu` is clean for the new files; the default
`esp` build is unchanged.

Open:

1. Real-model parity on a GPU: `real_model_matches_pytorch` with the checkpoint,
   its `.yaml` and a PyTorch chunk dump in `ESP_ROFORMER_DIR`.
2. Timing and memory on the family's machines, as the baseline for deciding
   whether Watershed's kernels come back.
3. A catalog row for BS-RoFormer SW: commit-pinned URL and sha256 as Watershed
   records them, license "none declared".
4. A host: Hocket imports a song as stereo tracks (brief §8).

## A2. Parakeet TDT from bunsen

Timed words (`start`, `end` in seconds) for `timed-text`. bunsen is already on
the device API, so the port is about its weight readers and Rekey, not
signatures.

## A3. HTDemucs from demucs-rs

The CPU-capable fallback. Needs the move from `B: Backend` to the device API,
from Burn 0.20.1, and the parity fixtures demucs-rs lacks.
