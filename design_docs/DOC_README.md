# Distillery documentation

Network-session receiving (2026-11-03): mesh and durable inference use
`murm-sync` for live sessions and Stickleback 0.2 for storage and admission.
The shared extraction scope and evidence live in
`murm/design_docs/2026-10-28_transport_consolidation_plan.md`, T7.

Preserve host-owned authority, existing formats and independent crate feature boundaries. Read [documentation policy](DOC_POLICY.md) before editing.

Canonical migration and receipts: `mere/design_docs/archive_docs/2026-10-28/2026-10-19_family_promotion_plan.md` (archived 2026-10-28). Its open items continue in `mere/design_docs/mere_docs/implementation_strategy/2026-10-28_family_promotion_plan.md`.

Execution/shutdown receiving (2026-10-19): Armillary now exposes fallible actor completion; ESP keeps model execution and validates clean actor shutdown in its existing provider tests. Canonical source and consumer receipts remain in `mere/design_docs/archive_docs/2026-10-28/2026-10-19_family_promotion_plan.md`.

Pool lifetime receiving (2026-10-21): the workspace and generated lock now pin
Armillary `92d5b15cf39aaac40a3f2d863ffee69c9be11bfc` for Q112 admission and
Q113 retirement. `cargo +1.99.1 test --workspace --locked -j2` passes 94 cases
with one existing benchmark ignored; `cargo +1.99.1 test -p esp --features actor
--lib --locked -j2` passes 88 cases, including all six inference actor cases;
the same actor library sweep passes on the supported Rust 1.97.1 minimum.
ESP continues to use dedicated `spawn_named` actors. Its actor normal dependency
tree resolves one canonical Armillary source without Tokio; its default normal
tree omits Armillary. Pool behavior belongs to the supplier and standalone
consumer qualification; model, GPU, native and product acceptance remain their
existing gates. Coordinated ESP → Knot → Mere/product receipt:
`mere/support/family-promotion/pool-lifetime-20261021.json`.

No plan is active. Archived under `archive_docs/2026-10-31/`:

- [Embedding compatibility](archive_docs/2026-10-31/2026-10-20_embedding_compatibility_plan.md) — landed and resident-qualified; provider identities and guarded dense/sparse persistence, with Eidetic P4 and resident I10 as the acceptance owners.
- [Burn 0.22.1 qualification](archive_docs/2026-10-31/2026-10-24_burn_0221_plan.md) — landed 2026-10-25: released dependency cohort, numerical/native/durable-generation checks and receiving composition. Mere owns vendor patches and browser/remote acceptance; deferred items are carried by the [models plan](implementation_strategy/2026-10-28_distillery_models_plan.md), CU1.

Registry storage receiving (2026-11-03): the Eidetic and Muniment dependency
requirements follow the published storage API, qualified with the default and
actor suites. Canonical coordination and receipts are in
`mere/design_docs/mere_docs/implementation_strategy/2026-10-28_workspace_hygiene_plan.md`.

Distillery receiving resumes 2026-11-03 under the
[extraction plan](implementation_strategy/2026-10-31_distillery_extraction_plan.md) (then in Mere),
D1-D18 and the current instruction to move the developed inference, mesh and
port subsystems out of Mere. The existing `distillery-receive` history is merged
into main in a collision worktree; the primary ESP checkout stays untouched.
Required receipts are donor/source equivalence, manifest exceptions, standalone
build/test gates, package-source identities, consumer checks and final commits.
Keep all run evidence at `Code/testing/esp/distillery-extraction-20261103`,
with a total retained-byte cap of 100 MB, using the stable ESP Cargo target.

Receiving qualification: 278 default tests pass with one existing ignored
benchmark; ESP's actor suite passes 88 tests. The complete workspace checks
all features and targets. Native, session and remote probes compile in their
own workspaces; the model probe compiles for wasm32 with locked sources.
These are software build/test receipts, not new model, headed-browser or
physical-network acceptance. Current run records live at the lane path above.

The receiving cohort now follows Eidetic's renamed-source intake; the moved
runtime and frozen receipts retain their Git blobs. Mesh provenance explicitly
names this repository and resolves exact direct or delegated Cargo selections,
with rejection controls for missing, disconnected and ambiguous dependencies.
The separate browser reproducer manifests also compile for wasm32.

The lexical cross-target probe also moves from Mere into `crates/mesh/probe`.
It compiles the owning codec by path and consumes ESP through this workspace;
its original source and donor revision are recorded in the lane receipts.
The wasm32 module runs under Node and agrees with the native mesh fixture:
801 canonical bytes, with the fixture's pinned BLAKE3 digest. This does not
extend inference-model or physical-device acceptance.

ESP receives the goal-model adapter (2026-11-06, Mere
`design_docs/mere_docs/implementation_strategy/2026-11-06_cycle_breaks_plan.md`
P4, decision D2): `esp::embed::goal_model` (`EspGoalEmbedder` and
`recommended_match_similarity_milli`, behind the `goal-model` feature) implements
Eidetic's `GoalEmbedder` trait, which `eidetic-search` keeps. The MiniLM
calibration test (`tests/goal_model_minilm.rs`, with its corpus under
`tests/fixtures/goal_calibration/`), the lexical n-gram recall receipt
(`tests/lexical_ngram_recall.rs`) and the `eidetic-recall` example (feature
`recall-example`) moved with it; Eidetic no longer depends on ESP. Calibration
rationale and the 220-thousandths threshold stay recorded in Eidetic's
`design_docs/eidetic_docs/implementation_strategy/2026-11-01_memory_gates_plan.md`.

## Documents received from Mere (2026-11-11)

Ten documents moved here from `merely-made/mere` with their history, as written there; the rows are Mere's, with the link repointed. Decisions are indexed in [decisions.toml](decisions.toml). A relevance review is pending; this log's own shape is not yet the index `DOC_POLICY.md` §6 describes.

### Plans

- [distillery_models_plan](implementation_strategy/2026-10-28_distillery_models_plan.md) — **cut 2026-10-28**: local models, the mesh host and compute-stack upkeep after the Burn migration.
- [distillery_extraction_plan](implementation_strategy/2026-10-31_distillery_extraction_plan.md) — **plan, 2026-10-31; rulings D1–D12**: Distillery, `mere-mesh` and the workshop halves of Alembic and Athanor leave Mere for the ESP repository, renamed `distillery`; Distillery becomes a producer over the published contract crates; the graph halves become `pandect` modules; every `esp.git` consumer moves in the same pass. Records the leaf/branch test (D8, refining stack.3) and opens Djinn's store-lifetime question (D11).

### Technical architecture

- [lease_bound_remote_sessions](technical_architecture/2026-08-10_lease_bound_remote_sessions.md) — **implemented through the supported plain-WGPU Distillery row**: Burn Remote mounts on Mere's endpoint, accepts a signed live-lease projection rather than a bearer token, interrupts live requests on close, orders stop before reclaim, drains sessions to zero, and recovers under a fresh lease. Fusion/autotune remains an upstream compatibility lane; portable remote checkpoints remain open.
- [alembic_memory_and_engrams](technical_architecture/2026-06-09_alembic_memory_and_engrams.md) — the alembic memory model + engrams; the armillary distillation daemon.

### Research

- [local_models_harness_brief](research/2026-06-24_local_models_harness_brief.md) — the **runtime + harness direction**, refreshed after ESP: `InferenceProvider`, the deterministic stub, own Burn decoder, Eidetic loading, cancellation, Armillary actor, and native host receipt are landed in `esp::infer`; `esp::embed` holds the adjacent embedding seam. The wasm execution claim is routed to the headed D2 browser probe, while immutable model sessions and `AdapterLoader` remain gated on one real adapter. Training, marketplace, and governance stay owned by their Geist/resource-coordination lanes.
- [burn_utilization_brief](research/2026-07-04_burn_utilization_brief.md) — **direction brief, all five lanes endorsed**: how far burn goes and what "shape the app around it" commits to. Code-verified footprint (embed's full BERT + aether's field lowering, all ndarray-CPU today; gyre burn-free by design), then five lanes in leverage order: L1 turn burn-wgpu on (+ the D1 shared-device-with-netrender decision), L2 burn-remote-over-iroh as the fleet compute protocol (gated on the post-0.21 release), L3 burn-first `InferenceProvider` (the browser-reaching backend), L4 on-device training/LoRA (burn's moat; adapters-as-engrams), L5 orrery graph intelligence (semantic arrangement, similarity edges, tensorized force pass via aether). Three commitments: columnar hot data, burn only behind the provider/field seams, one early device policy. Orthogonal to the 60fps frame-budget work except D1's queue contention.
- [geist_models_brief](research/2026-05-10_geist_models_brief.md) — personal + moot-trained LoRA adapters as engrams; LoRA stacking keystone; tessera-as-compute-credit (tier-3+).
- [local_intelligence_integration_research](research/2026-05-08_local_intelligence_integration_research.md) — Burn-first statistical intelligence; tier ladder; embeddings shipped at `intel/embed`, generative/agentic deferred. *(banner.)*
- [communal_compute_tiers_brief](research/2026-06-10_communal_compute_tiers_brief.md) — authority for the communal-compute/model-hosting lane after the 2026-06-30 split: volunteer-computing lessons, moot → moothold → coalition compute commons, time-bank as a constitution preset, LCZero-style data/eval loops, and late Petals-shaped async hosting.
- [agent_harness_brief](research/2026-07-03_agent_harness_brief.md) — **model-driver and peer-topology rationale reconciled, 2026-10-28**: Alembic/Athanor own workshop composition, Servitor supplies runtime-free admission/run semantics from Eidetic, Armillary supplies execution, and products acknowledge their own effects. Replaces the retired action-registry assumption with owner-defined semantic operations and separates proposal completion from accepted application. The Alembic plan owns the first deterministic two-host workflow; model guidance, protocol adapters and R2/R3 remain later work.
