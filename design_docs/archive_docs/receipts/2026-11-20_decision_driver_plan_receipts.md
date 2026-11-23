# Decision-driver qualification receipts

## 2026-11-23. Public evidence-brief trial

The native comparison executable was built from Distillery
`10e6dc2e8679d37eabcac94d09a94fde3c7edea8` in release mode with Rust 1.100.0.
Its SHA-256 was `ac1c12def696a5a3a14d5359530f7e4f013a712ede205760ad46e3206f1b51f9`.
The target was O-PC, Windows 11 build 26320, Ryzen 9 7940HS, 32 GB system RAM,
NVIDIA RTX 4060 Laptop GPU with 8188 MiB VRAM, Vulkan, NVIDIA driver 610.88.
The scored models were run serially. The host also ran builds. An independent
0.6B workshop verification used the GPU for about 15 seconds during the long
streamed 4B run. Timings and sampled board memory describe that host and load.
The separate first-turn fit probe preceded that overlap.

### Inputs and acceptance

Six authored public briefs each had three or four eligible passages. Both the
original and reversed order went through Athanor's normal host driver, worker
and Servitor reducer with a real monotonic clock. Each case created a fresh
workshop store and worker. A model and its GPU device were reused across cases
within each process. A score required a confirmed proposal awaiting review.
The review packet hid the producer, verified each quoted passage and citation
in order, and collapsed identical proposals for a case. The frozen rubric
counted missing required sources, irrelevant inclusions and violated precedence
edges. Ties were allowed where specified. Both presentations of the same brief
remain paired observations. This is a synthetic benchmark with a rule-based
rubric and a producer-blind inspection, without a human preference panel.

| Input | Identity |
|---|---|
| Corpus SHA-256 | `501f1374f6f8511896abd91a5d6e1c75e48b7b5977c6655802bf7b8ffafaaa0f` |
| Rubric SHA-256 | `6cc8d5a09b5a426b25630b31e51e87ede2ee17a32156d925b039ea2abb530655` |
| Qwen/Qwen3-0.6B revision | `c1899de289a04d12100db370d81485cdf75e47ca` |
| Qwen/Qwen3-4B revision | `1cfa9a7208912126459214e8b04321603b3df60c` |
| Template source BLAKE3, both checkpoints | `72a1a5e64f5ff443c86b9332e82c85afd838c9e398b8e8f8bb38579d0e76d892` |

All four 0.6B checkpoint files and all seven 4B files were verified against
upstream content identifiers. The model arms used constrained greedy generation,
`enable_thinking=false`, 32 tokens per attempt, up to 64 reserved tokens per
run and the workshop's 30-second budget. Each returned decision carries the
bound session and template. The raw attempt, accepted journal turn, reserved
usage, actual generated tokens and reopened proposal are distinct receipt fields.

### Measured arms

Times cover the run through confirmation or refusal. Loading is shown separately.
Peak GPU memory is the sampled usage of the whole board. Exact briefs count
against all 12 presentations. Omission, inclusion and order counts apply to
reviewable briefs. A dash means there was no reviewable brief to score.

| Arm | Load (s) | Reached review | Exact briefs | Omitted | Irrelevant | Order violations | Median / max run (s) | Peak board (MiB) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| deterministic | — | 12/12 | 3/12 | 0 | 10 | 8 | 0.004 / 0.006 | 1139 |
| qwen06-f32 | 4.746 | 12/12 | 3/12 | 2 | 6 | 6 | 1.137 / 2.281 | 5016 |
| qwen4-f32 | 9.870 | 0/12 | 0/12 | — | — | — | 71.569 / 184.862 | 7667 |
| qwen06-q8 | 9.791 | 10/12 | 2/12 | 2 | 6 | 5 | 1.195 / 4.766 | 7720 |
| qwen4-q8 | 41.216 | 10/12 | 10/12 | 0 | 0 | 0 | 1.964 / 115.734 | 7607 |

| Arm | Raw attempts returned | Generated tokens observed | Journal retries | SHA-256 of raw JSONL |
|---|---:|---:|---:|---|
| deterministic | 0 | 0 | 0 | `42103865946fa4ccd4a90fc5ab01c08e17df5ee9c24faaa1f6cd197a0b944c30` |
| qwen06-f32 | 12 | 152 | 0 | `bdd86dc2a73be1c3efe3d24b19582d62b6d03f2e40f88ad53b999cd5a85b4331` |
| qwen4-f32 | 5 | 63 | 0 | `5f92b0f257f8e530549608aaa346c4e108b0a3fd839d1ef2d6916631bbbd57ca` |
| qwen06-q8 | 10 | 126 | 0 | `d33086464be1e9c2bef2217ec304b1e76dba56f24afb2a0812b01228ddd04ea2` |
| qwen4-q8 | 12 | 144 | 0 | `cc5ed4fb8fb00440740d7d63c522b5fc67755d6dceff0bb1751b2c746a28e567` |

### Observations

- The deterministic body always kept all sources in input order. It preserved
  required evidence in all cases. Distractors and reversed precedence account
  for its 10 irrelevant inclusions and eight ordering errors.
- Resident 0.6B F32 completed every run and reduced irrelevant inclusions and
  inversions. It omitted required evidence twice. Its exact-brief count tied
  deterministic assembly. Resident 0.6B Q8 recorded GPU buffer allocation
  failures in the last two cases. Neither reached review or journalled a
  successful model turn. The host reported a worker join failure.
- The first streamed 4B F32 fit probe loaded in 13.086 s and returned an
  11-token decision in 58.164 s. The owner exhausted the 30-second budget.
  In the full run five presentations exhausted budget and seven stopped on a
  worker join error. WGPU reported allocation failures. Returning a raw
  decision after the deadline produced no reviewable brief.
- Resident 4B Q8 supplied all and only the required passages in the requested
  order on its first ten presentations. For example, both closure orders
  selected the keeper's log before the corroborating detour, and both repair
  orders selected isolation, cable replacement, then restoration. The two
  water presentations yielded raw choices `[0,1,2]` and `[2,1,0]`.
  Both were too late to reach review (98.618 and 115.734 s). A new process
  running just those two inputs reached review in 3.462 and 2.073 s. The
  scored primary run retains both failures.

### Replay, host checks and reproduction

The bound Qwen3 0.6B standalone GPU test passed with a repeatable 11-token
decision. The separate real-weight Athanor test passed with a 13-token
proposal and verified its bound session and template through workshop reopen.
One-byte template-source refusal and reopen without another driver call were
also exercised in the M4 binding suite. Mere keeps the schema-2 and D20
consumer receipts in
`mere/design_docs/archive_docs/receipts/2026-10-28_alembic_implementation_plan_receipts.md`.

Run the corpus generator at `crates/athanor/tests/fixtures/brief_comparison/corpus.py`.
Build the `workshop_compare` example from the Athanor workspace with
`workshop-model-gpu`. Pass the generated corpus, a fresh JSONL destination,
and either `deterministic` or the model ID, checkpoint directory and placement.
The 0.6B F32 arm used `resident-f32`, 4B F32 used `streamed-f32`, and both
Q8 arms used `resident-q8`. The `blind` and `score` commands in the sibling
`score.py` reconstruct the review packet and marks. Run directories and
receipts are retained in `Code/output/alembic-p3/20261123/`,
`Code/output/distillery-decision-comparison/` and
`Code/output/distillery-decision-binding/`. The model revisions and receipt
digests above identify the inputs and returned rows independently of file paths.
