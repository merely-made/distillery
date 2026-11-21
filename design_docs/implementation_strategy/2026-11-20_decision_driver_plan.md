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

No `doctrine.md` exists in this repository yet, so these are recorded as
cases without a statement to file them under (DOC_POLICY §11).

## Open forks

None open. The shape these decisions sit in is the
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
- Done when: a multi-step run reaches a stop state through the same reducer
  the deterministic body uses, an interrupted run reopens cold without
  redispatching a consequential operation, and a claimed success that the
  observations contradict is caught.
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

## Progress

- **2026-11-20:** plan written from the harness brief, the tree at `0e2c7ff`
  and Mark's answers. No code changed.
- **2026-11-20:** D3–D6 recorded; the four first forks closed and two that
  follow from them opened.
