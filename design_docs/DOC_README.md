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
- [Burn 0.22.1 qualification](archive_docs/2026-10-31/2026-10-24_burn_0221_plan.md) — landed 2026-10-25: released dependency cohort, numerical/native/durable-generation checks and receiving composition. Mere owns vendor patches and browser/remote acceptance; deferred items are carried by Mere's `2026-10-28_distillery_models_plan.md`, CU1.

Registry storage receiving (2026-11-03): the Eidetic and Muniment dependency
requirements follow the published storage API, qualified with the default and
actor suites. Canonical coordination and receipts are in
`mere/design_docs/mere_docs/implementation_strategy/2026-10-28_workspace_hygiene_plan.md`.

Distillery receiving resumes 2026-11-03 under Mere's
`design_docs/mere_docs/implementation_strategy/2026-10-31_distillery_extraction_plan.md`,
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
