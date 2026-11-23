# athanor

**Athanor** is the distillation furnace of Distillery,
the Mere platform's works.

An athanor is the furnace that holds a constant low heat for as long as the
work takes. This crate is the authority half of the
Alembic workshop (folded in as `athanor::alembic`, features `workshop`, `workshop-surface`, `workshop-native`): what a furnace pass
is, what it may propose — distillation, cleanup, facet proposals — and the
grants, runs, pending petitions, refusals, costs, pause, revoke, retry, and
dissolve over the bounded actors the workshop admits, of which the furnace
was always the first.

Two rules it keeps:

- **It proposes; it never owns truth.** A furnace pass emits proposals. The
  authority that owns the affected store decides. Athanor is not, and does
  not become, a graph-truth authority.
- **It is scheduled, not resident.** Djinn contains the scheduler and runs
  Athanor as one resident service, composing this crate's authority the way
  it composes Distillery's works: putting owners together and inventing
  nothing. Lifetime is Djinn's; the domain is Distillery's.

Ruled 2026-09-02: Athanor's authority lives with Distillery, the domain,
rather than with Djinn, the resident, by the same precedent Djinn set for the
works themselves. It sits beside Alembic as `ports/distillery/athanor`; the
package is `mere-athanor` and the library keeps the name.

Lives in the [mere](https://github.com/merely-made/mere) workspace at
`ports/distillery/athanor`.

**Status (2026-10-25):** the default `furnace` feature retains forgetting,
consolidation, image-reference cleanup and retirement proposal logic. The
`workshop` feature adds the public retained-passage evidence-brief domain;
`workshop-worker` adds native Armillary execution. The two product adapters,
headed controls and destination mutation receipts remain open. The [Alembic plan](../../../design_docs/archive_docs/2026-10-28/2026-06-24_alembic_implementation_plan.md#5-workshop-reconciliation-and-first-workflow-2026-10-24)
owns their integration and the first retained-passage evidence-brief workflow.
Athanor persists domain decisions and drives Servitor's existing reducer;
an Armillary adapter performs admitted work and reports execution completion.
Each destination owner separately acknowledges its accepted effects.

The workshop can be selected with defaults disabled. It uses Servitor’s
`admit`, `revalidate` and recorded-event `RunReducer`; its core has no Armillary,
Tokio, ESP, GPU or UI dependency. The native worker receives captured bytes
and correlations, with no store, authority provider or destination handle.
Its cancellation flag and observed join concern execution only.

`Workshop::new` names one installation in one owner/session namespace. Supply
a transactional Muniment backend, an explicit installed subject, scoped
current authority and owner timestamps. `begin` records the header/inputs;
`dispatch` persists each intent; `observe` revalidates and records each result.
The three steps read context, assemble literal Djot quotations and emit one
immutable proposal. Limits are three decisions, two tool calls, zero tokens
and 30,000 ms for a run begun with `begin`. `citation_source` resolves only captured immutable references.
Private inputs refuse until a qualified custody adapter exists.

**Decision drivers (2026-11-22).** The `decision` feature, which `workshop`
includes, adds `athanor::decision`: `DecisionDriver` is handed context and the
operations it may choose among, as one JSON Schema, and returns one decision
with its provider identity and token count. It holds no authority. `Scripted`
is the deterministic driver for scenarios and tests.

At the proposing step a driver chooses which of the owner's selected passages
a brief cites, at least two, and in what order (`propose_with`); the brief is
still built deterministically, and steps 0 and 2 are unchanged. The owner
accepts a brief only if it can be rebuilt from a duplicate-free subset of the
run's passages, and the confirming step must match the recorded candidate. A
brief that fails those checks pauses the run and writes nothing. Private
dispatches are never shown to a driver.

`Driver::with_decider` runs a driver through the host driver and its worker.
Such a run is begun with `begin_model` and carries its own limits
(`model_limits`): four decisions, one retry, and a token limit of twice the
driver's per-turn cap. The proposing step reserves its cap when it starts.
The journal keeps the provider and actual token count. A successful model
turn also carries the bound session ID and template hash in
`WorkshopView::turns`. Earlier schema-2 journals open with that field absent.
A failed or refused turn is journalled and retried once before the run ends
as failed, and cancelling the run stops an in-flight turn. Load accepts only
the two limit shapes.

`workshop-model` is the one place this crate reaches for Distillery: it
implements `DecisionDriver` for `distillery::decision::ModelDecider`. Athanor
still names neither esp nor Burn. `workshop-model-gpu` adds Distillery's GPU
decoder for the ignored real-model test, which needs `ESP_QWEN3_DIR`.

`accept` fixes one destination, accepting human and operation ID before an
effect. Concurrent callers may receive `Conflict`; reloading returns the same
winner’s assignment. Revalidate the returned acceptance after every await
with the current human, grants and deadline, and pass it through the target’s
actual gate. The target must atomically associate its operation ledger with
the note and graph mutation. `uncertain` retains reconciliation; only an
authenticated durable target receipt permits `acknowledge` to show `Added`.
The generation reducer remains terminal throughout review and acceptance.
A receipt DTO is a host contract, not a proof minted by this library.

The v1 store retains at most 32 runs in a 1 MiB transactional record. It
reserves room for pending artifacts and acceptance resolution before admitting
more work. It refuses at that bound without dropping history or unresolved
operations. This is a bounded first workflow, without a retention service,
scheduler, model calls, standing grants or automatic replay of effects.

## License

MPL-2.0
