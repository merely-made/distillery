# Mere burn-remote patch

Source: crates.io `burn-remote 0.23.0-pre.2`, upstream Burn commit
`cc3a1d4052fc72ee96dfb9664530c837cca7a88b` (rebased 2026-11-12; see the last
section). Earlier cuts: `0.22.1`, commit `907ab111a02c2dea47bc41633754f095c6b2220e`.
License: MIT OR Apache-2.0, unchanged from upstream.

The released server authorizes an Iroh session only at admission. Removing a
session from its manager does not stop the live duplex pump, whose task sender
can still accept work. Mere requires targeted close before owner revocation.

Each session is reserved before application authorization and carries a close
signal observed by the pump. This fences the authorization-to-worker-binding
interval. `IrohRemoteProtocol::sessions` exposes reserved or active session IDs
and opaque credentials; `close_session` targets one pump. Client close uses the
same teardown. The public exports include `ServedSession` and `SessionId`.

The earlier 0.22.0 rebase on 2026-10-06 used pristine stable source. Stable connection errors,
credential types, public server builder, hosted-device ID translation, FIFO
worker admission, response cancellation and global shutdown are preserved.
The patch adds a public `IrohRemoteProtocol::from_endpoint` constructor for
application-owned endpoints and keeps the protocol alive across owner clones.
Stable `HostSpec` and `IrohHost` handle client connections.

The session registry reserves admission before the application authorizer,
keeps failed teardown registered, and reports completion to every closer.
The duplex pump watches owner close, server shutdown and response-writer exit.
Worker completion waits for released device allocations and propagates errors.
The normalized manifest adds an empty workspace, self patch and exact stable
test dependencies; both manifests enable `tokio/macros` for pump selection.
Regression tests cover targeted close, in-flight authorization fencing and
failed cleanup acknowledgements alongside stable upstream's tests.

This rebase alone is not lifecycle acceptance. Full two-peer reclaim/recovery
and headed checks remain gates in the migration plan. Remove the patch when an
upstream release exposes equivalent session control and passes those receipts.

## Teardown completion (2026-10-03, Isometry ruling 508)

Upstream's worker teardown syncs, drops the session's interpreter and calls
`memory_cleanup`, logging sync failures and signalling completion either way.
On pre.4's WGPU backend the frees it issues complete only when their completion
callbacks run, so close was acknowledged while the session's allocations were
still live: the two-peer gate held 10 allocations against a zero baseline.

`server/worker.rs` now waits for the device (`B::sync`) after the release, then
runs one more `memory_cleanup` for the released pages. Every teardown sync
failure is kept, and the worker's completion carries `Result<(), String>`.
`server/session.rs` turns a teardown error into `SessionCompletion::Failed`.
The session stays registered and is never acknowledged clean, so
`close_session` returns the error, and Distillery's `close_run` propagates it.
The wait is device-wide; it may also wait on other sessions' queued work.

A `cfg(test)`-only fault hook (`worker::teardown_fault`) fails that wait for a
chosen session. The unit test
`server::session::teardown_tests::a_failed_teardown_sync_is_reported_and_never_acknowledged_clean`
uses it to check that a clean session closes `Ok` while a faulted one reports
the failure. It exists only in this crate's own test build; the public API is
unchanged. Remove this part with the rest of the patch, or earlier if an
upstream release waits for teardown completion and reports its failures.

## 0.22.1 cohort rebase (2026-10-24)

Released archives were checksum-verified before a three-way rebase. Existing
local source and regression deltas are preserved, including changes added
after the original stable-release receipt. New upstream source changes are
retained. Selected execution gates pass in the Burn migration plan, section 13.48.
The bounded macOS receipt and remaining platform/model limits are recorded
there; the source rebase alone is not a wider acceptance claim.

## 0.23.0-pre.2 rebase (2026-11-12)

Cut from the crates.io `burn-remote 0.23.0-pre.2` archive in the local cargo
registry (commit `cc3a1d40`). Pristine `0.22.1` was not on the machine, so the
local delta was taken from this file and the patched 0.22.1 tree, not from a
mechanical three-way diff. Upstream 0.23 has no session registry, no targeted
close, no public `IrohRemoteProtocol` and no teardown that reports failure, so
nothing was dropped as upstreamed. `RemoteNode::from_endpoint` and
`ServerSettings::sessions_of` are upstream and are reused.

Carried, onto upstream's 0.23 code (capture locks, device queries, dtype usage,
`MessageSink`/`MessageSource` framing and `MAX_UNAUTHORIZED_FRAME_SIZE` kept):

- `server/service/mod.rs`: `bind`/`close` become `reserve_session`,
  `bind_session` and `finish_session -> Result`; `SessionChannels` gains the
  owner's `close` signal.
- `server/session.rs`: the registry (`ServedSession`, `sessions`,
  `close_session`, completion watch, failed teardown stays registered).
  `finish_session` also calls upstream's `local_comm.abandon_session` before
  waiting, as upstream's `close` did, so a worker parked on a barrier drains.
  Teardown regression test carried.
- `server/pump.rs`: reserve before authorization, finish on every refusal or
  bind failure, close signal interrupts reader and writer, sink closed however
  the writer ends; upstream pump tests moved onto the new trait.
- `server/worker.rs`: `spawn` returns the teardown completion; teardown keeps
  every failure, waits for the device after the release, runs a second
  `memory_cleanup`; `teardown_fault` test hook.
- `transport/iroh/protocol.rs`: public, `Clone` with an owner-only lifetime
  guard (replaces upstream's `Drop` on the handler), `from_endpoint`,
  `sessions`, `close_session`.
- `server/mod.rs`, `shared/mod.rs`: export `SessionId`, `IrohRemoteProtocol`,
  `ServedSession` (0.23 made `shared::*` crate-private, so `SessionId` gets
  its own `pub use`).
- `lib.rs`: `lease_tests`; the in-flight test now sends `Init` with upstream's
  `FrameSink`/`Encode` instead of the removed `send_frame`.
- Manifests: `tokio/macros` in `Cargo.toml.orig`; empty `[workspace]`, self
  patch and exact `=0.23.0-pre.2` dev-dependencies (`burn-dispatch`,
  `burn-flex`, `burn-tensor` with upstream's features) in `Cargo.toml`.

Upstream also removed the 0.22 hosted-device id translation
(`HostedDeviceIds`); collectives are now local all-reduce tasks, so it is not
carried.

Verified: Distillery `--features remote` checks and its tests pass (both
remote-session tests included). The crate's own tests could not run as
shipped: `serial_test 4.0.1` and `burn-communication 0.23.0-pre.2` are not in
the local registry. A scratch copy with the websocket transport and its tests
removed ran offline: 75 unit tests (pump, teardown and both lease tests
included) and 31 `tests/iroh.rs` tests pass. Not run: the websocket tests, the
fusion feature, wasm, two-peer reclaim/recovery and headed gates.
