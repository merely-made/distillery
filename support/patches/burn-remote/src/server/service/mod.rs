//! The session-layer service trait, plus the init-handshake helper shared by the session pump.
//!
//! The pump ([`drive_session`](crate::server::pump::drive_session)) is written against
//! [`SessionService`] rather than a concrete type, so the submit-loop teardown logic stays testable
//! against a fake service with no backend and no live socket.

mod base;

pub(crate) use base::parse_init_handshake;

use std::{future::Future, sync::Arc};

use burn_backend::DTypeUsageSet;
use burn_std::{DType, DeviceSettings};
use tokio::sync::{mpsc, watch};

use crate::shared::{SessionId, Task, TaskResponse};

/// A bound session stream's ends: tasks go to the session's worker, responses come back.
pub(crate) struct SessionChannels {
    pub(crate) tasks: mpsc::Sender<Task>,
    pub(crate) responses: mpsc::Receiver<TaskResponse>,
    /// Set when the owner closes the session (Mere targeted close).
    pub(crate) close: watch::Receiver<bool>,
}

/// What the session pump needs from the session layer: bind a session stream to its worker, read
/// the device metadata for the handshake, and tear a session down.
///
/// Async methods return `impl Future + Send` so a session future built on them stays `Send` and can
/// be spawned by the server. The production implementation is
/// [`SessionManager`](super::session::SessionManager).
pub(crate) trait SessionService: Send + Sync + 'static {
    /// Reserve one session before application authorization begins (Mere).
    ///
    /// A duplicate id is rejected. Reserving first makes server-requested
    /// closure race-free: the host can see and close a session throughout the
    /// interval between authorization and worker binding.
    fn reserve_session(
        &self,
        session_id: SessionId,
        device_index: u32,
        authorization: Arc<[u8]>,
    ) -> impl Future<Output = Result<(), String>> + Send;

    /// Bind one reserved and admitted session to its stream and spawn its worker: the channel
    /// forwarding tasks to its worker, its response receiver and its close signal. Errors if the
    /// session was not reserved, was closed during admission or is already bound: the protocol
    /// allows one stream per session.
    fn bind_session(
        &self,
        session_id: SessionId,
    ) -> impl Future<Output = Result<SessionChannels, String>> + Send;

    /// The default settings of the device at `device_index`, returned on the handshake so the
    /// client can resolve op dtypes without an extra round-trip.
    fn device_settings(&self, device_index: u32) -> DeviceSettings;

    /// The number of devices this server hosts, returned on the handshake so the client can
    /// enumerate every device behind the address.
    fn device_count(&self) -> u32;

    /// What the device at `device_index` can do with each of
    /// [`HANDSHAKE_DTYPES`](crate::shared::HANDSHAKE_DTYPES), returned on the handshake so the
    /// client answers dtype queries without a round-trip.
    fn dtype_usage(&self, device_index: u32) -> Vec<(DType, DTypeUsageSet)>;

    /// Remove a finished session and acknowledge closure only after its worker has released
    /// backend state (Mere). A teardown failure is returned and the session stays registered. A
    /// `finish_session` for an unknown session is a no-op.
    fn finish_session(
        &self,
        session_id: SessionId,
    ) -> impl Future<Output = Result<(), String>> + Send;
}
