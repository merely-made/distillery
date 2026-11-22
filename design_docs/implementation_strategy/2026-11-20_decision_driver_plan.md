# Decision driver: a model-guided turn for the workshop, on Burn

**Status (2026-11-20):** plan. Nothing here is built. Awaiting Mark's read and the open forks below.

**For:** the "stack agent". The agent harness brief
([research/2026-07-03_agent_harness_brief.md](../research/2026-07-03_agent_harness_brief.md))
already designs it: Alembic is the workshop, Athanor owns the domain logic,
Servitor reduces runs, Armillary runs actors, ESP supplies the model. Its §2,
"A model decision turn", is marked "design for a later driver". This plan
builds that driver.

**Not here:** the workshop workflow and its acceptance (W0–W3), which the
Alembic plan owns
(`mere/design_docs/mere_docs/implementation_strategy/2026-10-28_alembic_implementation_plan.md`);
Servitor's admission and run semantics (Eidetic); the Inkling 2 Mini decoder
body ([2026-11-20_inkling_body_plan.md](2026-11-20_inkling_body_plan.md)),
which arrives later through the same provider seam.

## Origin

Mark, 2026-11-20, after asking about Inkling 2 Mini for harness testing:
"should we not make a stack agent? Via burn? Possibly leveraging the stack?"
Checked against the tree the same day: the composition exists, the reducer
has receipts, and the brief's own status says "model-guided decisions remain
open".

## Decisions

- **D1. A stack agent, on Burn (Mark, 2026-11-20):** "should we not make a
  stack agent? Via burn? Possibly leveraging the stack? I only mentioned the
  nous agent as a reference".

  Links: draws on mere:house-style D12 · summary The agent is built from the stack's own parts and runs its model on Burn; Hermes Agent is a design reference only.

- **D2. Agent first, on a model esp already runs (Mark, 2026-11-20):** "Agent
  first, on a model esp runs", over writing the Inkling body first and over
  driving the agent with Inkling through an outside runtime.

  Links: plans inkling-body · summary The first decision driver uses a Qwen-family model esp runs natively today; Inkling 2 Mini follows as its own plan.

D3–D6 answer four recommendations put on 2026-11-20, in one answer: "I accept
your recommendations".

- **D3. Qwen3, sized by measurement (Mark, 2026-11-20):** accepted as
  recommended: Qwen3 0.6B to wire the turn, Qwen3 4B as the working driver,
  8B measured for fit on 8 GB.

  Links: summary The first driver model is Qwen3: 0.6B for wiring, 4B for work, 8B if it measures to fit.

- **D4. Decisions are forced by constrained decoding (Mark, 2026-11-20):**
  accepted as recommended, over parsing the model's own tool-call format.

  Links: summary A decision is produced under `OutputGrammar` and still validated against the operation schema.

- **D5. Trait in Athanor, model in Distillery (Mark, 2026-11-20):** accepted
  as recommended: the driver trait sits in Athanor with no esp dependency;
  the model implementation sits in Distillery behind `inference`.

  Links: summary Athanor owns the decision-driver trait; Distillery implements it over esp.

- **D6. A small fixture first, then the evidence brief (Mark, 2026-11-20):**
  accepted as recommended.

  Links: summary P1 and P2 run on a fixture; the Alembic evidence brief is P3's task.

- **D7. JSON Schema in front of the grammar (Mark, 2026-11-20):** "JSON
  Schema in front", over growing `OutputGrammar` alone and over adopting
  GBNF. Asked as part of aligning esp with outside standards.

  Links: summary Operations are declared as JSON Schema and compiled into `OutputGrammar`, which grows numbers, booleans and lists; replay and private-text clearing are unchanged.

- **D8. Inert templates by default, the model's own through minijinja (Mark,
  2026-11-20):** first asked "There's no rusty equivalent? What's jinja,
  python?"; told that minijinja is a pure-Rust renderer by Jinja's author and
  asked whether templates are inert by default with minijinja per model:
  "yes".

  Links: summary esp's inert template stays the default; a model's own chat template may be selected per model and is rendered by minijinja, recorded in the session identity.

- **D9. Both layers now (Mark, 2026-11-21):** shown that Athanor's only body
  is `evaluate(Dispatch) -> Observation`, shaped around the evidence brief,
  and asked what the driver trait should look like: "Both layers now", over
  the general decision turn alone and over the workshop's own types alone.

  Links: summary Athanor gets a general decision-turn trait (context and operation schemas in, one validated decision out) and, in the same slice, a workshop adapter that turns a decision into an Observation.

- **D10. Athanor reaches for Distillery (Mark, 2026-11-21):** shown that D5
  as written needs Distillery to depend on Athanor, which engine-split D117
  avoided by giving Athanor its own workspace root: "Athanor reaches for
  Distillery", over a shared leaf crate, over Distillery depending on
  Athanor, and over the trait in Servitor.

  Links: amends decision-driver D5 · draws on mere:engine-split D117 · summary The trait stays in Athanor with no esp dependency by default; Distillery exposes a plain model-decision function and knows nothing of Athanor; an optional Athanor feature depends on Distillery and holds the trait implementation.

- **D11. A driver chooses and orders passages (Mark, 2026-11-21):** shown
  that the workshop accepted a worker's proposal only when identical to the
  brief built from the run's passages in their original order: "Choose and
  order passages", over an adapter that leaves the workshop's checks alone
  and over deferring the adapter to P3.

  Links: summary At the proposing step a driver picks at least two of the owner's selected passages and their order; the brief text stays deterministic; the workshop accepts a duplicate-free subset of the run's passages and confirms the recorded candidate.

D12–D14 answer three questions put on 2026-11-22 after the workshop and
Servitor's reducer were read for P2.

- **D12. Reserve at start, record actual (Mark, 2026-11-22):** shown that
  Servitor checks a token limit only when a step starts and that the
  workshop's limit is 0: "Reserve at start, record actual", over recording
  without a cap and over charging actual tokens with a workshop-side check.

  Links: summary The proposing step reserves its max_tokens against a real limit at intent; the actual count and the model's identity are stored in the journal and shown in the view. Stored runs under the old limits are not carried over.

- **D13. Record a failed turn, retry once, then fail (Mark, 2026-11-22):**
  shown that a failed or refused model turn writes nothing and the run
  pauses: "Record it, retry once, then fail", over failing at once and over
  pausing as today.

  Links: summary A failed or refused model turn is journalled as a retryable failure; one retry is allowed and a second failure ends the run as failed; each attempt counts against the decision budget.

- **D14. The claim condition restated (Mark, 2026-11-22):** shown that a
  worker cannot claim success in the workshop, since the owner decides
  completion and rebuilds every brief from its own passages: "Restate it as
  what is checked", over adding a claim to check and over deferring.

  Links: summary P2's "a claimed success that the observations contradict is caught" becomes: a brief must be rebuildable from the owner's own passages, and the confirmed brief must equal the recorded candidate.
D15–D17 answer three forks the P2 lane returned on 2026-11-22.

- **D15. Separate limits for model runs (Mark, 2026-11-22):** shown that
  the lane had raised the limits for every run (4 decisions, 256 tokens, 2
  consecutive failures), which lets a model-free run retry an interrupted
  proposing step: "Separate limits for model runs", over one raised limit
  for all and over one limit with the token cap taken from the driver.

  Links: refines decision-driver D12, decision-driver D13 · summary A run without a decision driver keeps the limits it had (3 decisions, no tokens, no retry). A run begun with a driver gets 4 decisions, one retry and a token limit of twice the driver's per-turn cap; the limits are stored with the run.

- **D16. An integrity refusal pauses and writes nothing (Mark,
  2026-11-22):** asked whether a brief that fails the owner's checks should
  count as a failed model turn: "Yes, pause and write nothing", over
  journalling it as a failed turn with or without a retry.

  Links: refines decision-driver D13, decision-driver D14 · summary A brief not rebuildable from the owner's passages, or a confirmed brief that differs from the candidate, is refused without a journal entry and outside the retry count; the run pauses for a person.

- **D17. An interrupted confirming step stays unfinishable (Mark,
  2026-11-22):** shown that retrying step 2 would be a third tool call
  against a limit of 2, as before P2: "Leave it", over allowing one retry.

  Links: summary The tool-call limit stays 2; a run whose confirming step was interrupted ends as budget-exhausted.
No `doctrine.md` exists in this repository yet, so these are recorded as
cases without a statement to file them under (DOC_POLICY §11).

Standing for this plan (Mark, 2026-11-21): "Commit and push each green
slice": when a slice's tests and the repository's hooks pass, it is committed
to main and pushed.

## Open forks

1. **Where a model's identity and usage are kept.** `propose_with` returns
   the decision's provider identity and token count to the host, but the
   workshop's run record has no place for them and its limits carry
   `tokens: 0`. The brief asks that both be retained.
2. **The Alembic plan has not seen D11.** It is the single acceptance
   specification for the evidence brief and lives in Mere; D11 widens what
   the workshop accepts and is recorded only here so far. The shape these decisions sit in is the
[Burn shape brief](../research/2026-11-20_burn_shape_brief.md).

## Phases

### P1. One decision from a real model
A single turn of brief §2 steps 1–3 against the chosen model on Burn:
authorized context in, one decision out, validated against an operation
schema, with provider and session identity and observed usage retained.
- Done when: a test drives one turn end to end with real weights on the
  local GPU, an invalid decision is refused by name, and the same turn
  replays from its recorded token IDs.
- Needs: the schema front end of D7; the template selection of D8.

### P2. The turn loop through Servitor
Steps 3–4 repeated: dispatch, record the observed result, apply recorded
events through Servitor, advance or stop. Interruption and cancellation use
the existing actor controls.
- Done when: with a decision driver at the proposing step, run through the
  host driver and its worker: a run reaches a stop state through the same
  reducer the deterministic body uses; an interrupted or reopened run does
  not ask the driver again for a step already recorded; the step reserves
  its token cap at intent and the journal keeps the actual count and the
  model's identity (D12); a failed or refused turn is journalled, retried
  once and then ends the run as failed (D13); an in-flight turn stops at a
  token boundary when the run is cancelled; and a brief that is not
  rebuildable from the owner's passages, or a confirmed brief that differs
  from the recorded candidate, is refused (D14).
- Needs: P1.

### P3. The workshop task with a model body
The model driver takes the place of the deterministic body in the chosen
first task.
- Done when: the task completes under the model driver with the same
  authority checks as the deterministic body, and its failures are recorded
  as findings with the decisions that caused them.
- Needs: P2.

### P4. A second provider through the same seam
The Inkling 2 Mini body, once it exists, drives the same turn with no change
to the driver.
- Done when: the P1 test passes with the Inkling provider selected.
- Needs: inkling-body P3.

## Findings

- **F1 (2026-11-20).** The laptop has an NVIDIA RTX 4060 Laptop GPU with
  8188 MiB of VRAM, an AMD Radeon 780M, and 31.2 GB of RAM (`nvidia-smi`,
  `Win32_VideoController`).
- **F2 (2026-11-20).** esp's decoder accepts `model_type` llama, mistral,
  qwen2, qwen3 and qwen2_moe and refuses others by name
  (`crates/esp/src/infer/decoder/config.rs`, `SUPPORTED_MODEL_TYPES`).
- **F3 (2026-11-20).** Athanor's features are `workshop`, `workshop-surface`,
  `workshop-native`, `workshop-worker`, `workshop-sealed` and
  `workshop-destination`; none depends on esp (`crates/athanor/Cargo.toml`).
  The host driver is `crates/athanor/src/workshop_driver.rs` (`Driver<B>`).
- **F4 (2026-11-20).** Hermes Agent's README states an MIT licence. The
  code is Python; the Rust it ships is the bundled `uv` package manager.
  Read from README copies, not from the repository's LICENSE file.

- **F5 (2026-11-20).** esp's prompt templates are declared, inert literals:
  "Strings are literal data, never programs", one system instruction,
  optional context and one ask (`crates/esp/src/infer/prompt.rs`). Nothing in
  `crates/esp/src/infer` mentions tool calls, a chat template or a JSON
  schema.

- **F6 (2026-11-21).** D7 needed no change to the grammar engine. Numbers,
  booleans and lists compile into the four existing `OutputGrammar` variants
  (`crates/esp/src/infer/schema.rs`), so replay, forced spans and
  private-text clearing are as they were. The subset: `const`, `enum`,
  `anyOf`, `boolean`, `null`, `string` with `maxLength`, `integer` with
  `minimum` and `maximum`, `array` with `maxItems`, and closed objects whose
  properties are all required. Integer ranges are exact. Limits: a string is
  nonempty and `maxLength` bounds its UTF-8 bytes; `number` (floats),
  `oneOf`, `$ref`, `pattern` and every other keyword are refused by name;
  nesting is capped by the engine's depth of 24 (about eight list items);
  members are emitted in sorted name order.

## Progress

- **2026-11-20:** plan written from the harness brief, the tree at `0e2c7ff`
  and Mark's answers. No code changed.
- **2026-11-20:** D3–D6 recorded; the four first forks closed and two that
  follow from them opened.
- **2026-11-21:** P1, first slice: the JSON Schema front end and its
  validator landed in esp behind `output-schema`
  (`grammar_from_schema`, `validate_against_schema`), with tests for exact
  integer ranges, array bounds, refusal by keyword, refusal of a decision by
  path, and the constraint engine driven through a compiled schema. No model
  has run a turn yet.
- **2026-11-21:** P1, second slice (D9, D11): `athanor::decision` (the
  `DecisionDriver` trait, `Turn`, `Decision`, a `Scripted` driver) behind a
  light `decision` feature, and `workshop::propose_with`, which lets a
  driver choose and order passages at step 1. The workshop's load check,
  `observe` and `evaluate`'s confirming step accept a duplicate-free subset
  of the run's passages and confirm the recorded candidate; acceptance
  integrity reads the reviewed brief's digest. Athanor's 35 library tests
  pass with `workshop-worker` (33 as before, 2 new). Its `workshop-sealed`
  and `workshop-destination` features were not rebuilt. Still no model.
- **2026-11-21:** P1, third slice (D10): `distillery::decision` behind a
  light `decision` feature (`decide`, `replay`), and the first real turn.
  `tests/decision_turn.rs` (ignored without `ESP_QWEN3_DIR`; artifacts
  pinned by SHA-256) passed on Burn wgpu, RTX 4060 Laptop GPU, Vulkan:
  Qwen3-0.6B was offered three passages, one of them a recipe, and decided
  `{"op":"brief","order":[0,2]}` in 11 tokens; the record replayed to the
  same decision, a second greedy run produced the same tokens, and a
  cut-short record, a record under other operations and a three-token turn
  were refused. Debug build: 65 s to load, 9.1 s for the turn; not a speed
  claim. One prompt is not a measure of the model's judgement. Not yet
  done for P1: the Athanor feature that implements `DecisionDriver` with
  `decide` (D10), and template selection (D8); the test renders Qwen's chat
  format by hand.
- **2026-11-21:** P1, fourth slice (D10): Athanor's `workshop-model` feature
  depends on Distillery and implements `DecisionDriver` for
  `distillery::decision::ModelDecider`, whose surface names no esp type
  (`crates/athanor/src/decision_model.rs`, the whole of the glue).
  `discrete_gpu_decider` builds one from the artifact triple. Athanor's root
  resolves Distillery's graph with no unused patch and no family, Burn or
  wgpu crate at two sources; the 18 third-party crates at two versions are
  the same 18 the main root shows for `distillery --features decision`.
  End to end, ignored without `ESP_QWEN3_DIR`
  (`a_model_proposes_and_the_workshop_holds_it_to_the_owners_selection`,
  feature `workshop-model-gpu`): the workshop's own fixture run with
  Qwen3-0.6B at the proposing step on the RTX 4060. It decided
  `{"op":"brief","order":[0,1,2]}` in 13 tokens; the owner accepted it, step
  2 confirmed it, and the reopened run awaits review with that brief. The
  model kept all three passages here, so this run did not exercise a real
  subset; the scripted test does. P1's done-condition is met except that
  the prompt is still rendered by hand (D8).
- **2026-11-22:** P1, fifth slice (D8): `esp::infer::chat` behind
  `chat-template` renders a model's own Jinja chat template with minijinja
  3.1 (Apache-2.0): no loader, filesystem, network or clock, and a fuel limit.
  Its source bytes are what `BoundModelSession::prepare` already binds by
  hash, so the session needed no change. Qwen3's real template renders the
  prompt the earlier tests wrote by hand. `distillery::decision::chat_prompt`
  builds the renderer from a `tokenizer_config.json`; both real-model tests
  use it and pass on the RTX 4060 with the same decisions as before
  (`[0,2]` in Distillery, `[0,1,2]` in the workshop run; Athanor 36 of 36).
  The decision path does not yet run inside a bound session, so a decision
  does not carry the template's hash; that moves to P2. P1 is done.
- **2026-11-22:** D12–D14 recorded and P2's done-condition restated after
  reading the workshop and Servitor's reducer: a token limit is checked only
  at intent (`servitor` `run/reducer.rs`, `would_exceed`), the worker actor
  calls `evaluate` directly, and no journal entry carries tokens, a model
  identity or a failure.
- **2026-11-22:** P2 landed from a lane in its own worktree (`8d80ef2`,
  `01f1699`), verified there before merging: 55 Athanor library tests pass
  with `workshop-model-gpu`, `workshop-sealed` and `workshop-destination`,
  the real-model workshop run among them (Qwen3-0.6B, `[0,1,2]`, 13 tokens,
  32 reserved against a limit of 64). The worker and the host driver take a
  decision driver (`Decider`, `Driver::with_decider`,
  `Workshop::begin_model`); a model run carries its own limits
  (`model_limits`) and a model-free run keeps the old ones, its
  interruption test unchanged (D15). The proposing step reserves its cap at
  intent and the journal keeps provider and actual tokens (D12); a failed or
  refused turn is journalled and retried once (D13); a cancelled run stops
  an in-flight turn through `Turn::cancelled` and esp's stop flag;
  integrity refusals pause and write nothing (D14, D16). The record schema
  is 2; the brief's schema stays 1. Lane choices kept: a refused decision
  and a driver error are journalled alike; the view's `usage.tokens` is the
  sum of reservations and `turns` carries the actual counts; a failed turn
  records no provider or tokens; a private dispatch with a driver fails the
  step; resuming under a mismatched driver is refused before anything is
  written. Not covered: no real model was cancelled mid-turn (the stop path
  is tested with a fake provider), and the decision path still does not run
  inside a bound session, so a decision carries no template hash.
