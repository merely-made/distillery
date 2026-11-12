use burn_backend::{DTypeUsageSet, tensor::Device};
use burn_ir::BackendIr;
use burn_router::{CustomOpRegistry, TensorInterpreter};
use std::{
    collections::HashMap,
    sync::{Arc, Once},
};
use tokio::sync::{Mutex, mpsc, oneshot, watch};

use crate::metrics::{MetricSide, logger_task};
use crate::server::DeviceCaptureLock;
use crate::server::DeviceQueries;
use crate::server::local_comm::LocalCommService;
use crate::server::service::{SessionChannels, SessionService};
use crate::server::spawn::spawn_detached;
use crate::server::transfer::TensorTransfer;
use crate::server::worker::{SessionHandler, SessionServices};
use crate::shared::{HANDSHAKE_DTYPES, SessionId, Task};
use crate::telemetry::{TelemetryEvent, TelemetryProbe};

/// Capacity for the per-session response queue.
///
/// Sized larger than the typical in-flight read/sync count so that request processing
/// doesn't block on backpressure during a burst, but small enough that a stuck response
/// writer surfaces as a backpressure stall rather than memory growth.
const RESPONSE_CHANNEL_CAPACITY: usize = 64;

/// Coordinates per-session state.
///
/// Each [`Session`] owns a dedicated [`SessionHandler`] that holds the session's
/// [`TensorInterpreter`] with its own [`HandleContainer`](burn_ir::HandleContainer) — different
/// sessions never share tensor handles, so concurrent sessions can't race on each other's backend
/// state. Cross-session tensor transfers go through `external_comm` (cross-server) or `local_comm`
/// (same-host), each of which has its own rendezvous.
///
/// Tasks run on the handler's worker threads, not the submit handler: the latter only decodes the
/// incoming batch and forwards each [`Task`] to the session over a bounded channel. Inside the
/// session a dispatcher routes each task to a per-stream worker thread, so per-stream ordering is
/// preserved while independent streams — and other sessions — keep making progress even when one
/// stream is parked on a blocking op (a same-host transfer rendezvous or an all-reduce barrier).
pub struct SessionManager<B, T>
where
    B: BackendIr,
    T: TensorTransfer<B>,
{
    /// All devices this server hosts, indexed by the device index the client selects at
    /// session init. `devices[0]` is the default device (`DeviceIndex::Default`).
    devices: Vec<HostedDevice<B>>,
    pub(crate) transfer: Arc<T>,
    /// Rendezvous registry for same-host tensor transfers between this server's sessions.
    pub(crate) local_comm: Arc<LocalCommService<B>>,
    /// Custom-op handlers shared (read-only) with every session's interpreter.
    custom_ops: CustomOpRegistry<B>,
    /// Device-query handlers shared (read-only) with every session.
    queries: DeviceQueries<B>,
    sessions: Mutex<HashMap<SessionId, Session>>,
    probe: TelemetryProbe,
    /// Spawns the telemetry logger once, on the first session.
    logger: Once,
}

/// A device this server hosts, with the lock its sessions' graph captures share.
struct HostedDevice<B: BackendIr> {
    device: Device<B>,
    capture_lock: DeviceCaptureLock,
}

/// One reserved or bound session (Mere: reserved before authorization, so an owner can see and
/// close it from admission on, and kept registered until its teardown is known to be clean).
struct Session {
    /// Inbound channel to the session's dispatcher thread, held from binding until teardown so a
    /// worker cannot disappear while the pump is active.
    task_sender: Option<mpsc::Sender<Task>>,
    device_index: u32,
    authorization: Arc<[u8]>,
    close: watch::Sender<bool>,
    done: watch::Sender<SessionCompletion>,
    worker_done: Option<oneshot::Receiver<Result<(), String>>>,
    finishing: bool,
}

#[derive(Clone, Debug)]
enum SessionCompletion {
    Running,
    Clean,
    Failed(Arc<str>),
}

/// One session currently served by an Iroh remote protocol.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServedSession {
    /// Client-chosen process-local session id.
    pub id: SessionId,
    /// Hosted compute device selected at admission.
    pub device_index: u32,
    /// Opaque application credential accepted for this session.
    pub credential: Arc<[u8]>,
}

impl<B, T> SessionManager<B, T>
where
    B: BackendIr,
    T: TensorTransfer<B>,
{
    pub fn new(devices: Vec<Device<B>>, transfer: Arc<T>) -> Self {
        assert!(
            !devices.is_empty(),
            "A remote server must host at least one device"
        );
        Self {
            local_comm: Arc::new(LocalCommService::new(devices.len())),
            devices: devices
                .into_iter()
                .map(|device| HostedDevice {
                    device,
                    capture_lock: DeviceCaptureLock::default(),
                })
                .collect(),
            transfer,
            custom_ops: CustomOpRegistry::default(),
            queries: DeviceQueries::default(),
            sessions: Mutex::new(HashMap::new()),
            probe: TelemetryProbe::disabled(),
            logger: Once::new(),
        }
    }

    /// Register custom-op handlers, shared read-only with every session's interpreter.
    pub fn with_custom_ops(mut self, custom_ops: CustomOpRegistry<B>) -> Self {
        self.custom_ops = custom_ops;
        self
    }

    /// Register device-query handlers, shared read-only with every session.
    pub fn with_device_queries(mut self, queries: DeviceQueries<B>) -> Self {
        self.queries = queries;
        self
    }

    /// Emit telemetry into `probe`.
    pub fn with_telemetry(mut self, probe: TelemetryProbe) -> Self {
        self.probe = probe;
        self
    }

    fn ensure_logger(&self) {
        self.logger.call_once(|| {
            if let Some(task) = logger_task(&self.probe, MetricSide::Server) {
                spawn_detached(task);
            }
        });
    }

    /// Resolve the device at `device_index`.
    ///
    /// The index is validated against the server's device count on the client init handshake, so
    /// an out-of-range index here is a protocol/configuration error (e.g. a client enumerating
    /// more devices than this server hosts). Fail loudly rather than silently collapsing onto
    /// device 0 — for a collective that would reduce a device against itself and silently corrupt
    /// the result instead of producing a clear failure.
    pub(crate) fn device(&self, device_index: u32) -> Device<B> {
        self.hosted(device_index).device.clone()
    }

    /// The hosted device at `device_index`, failing as [`device`](Self::device) does.
    fn hosted(&self, device_index: u32) -> &HostedDevice<B> {
        self.devices.get(device_index as usize).unwrap_or_else(|| {
            panic!(
                "Requested device index {device_index} but server hosts only {} device(s)",
                self.devices.len()
            )
        })
    }

    /// Snapshot reserved and active sessions in deterministic id order.
    pub async fn sessions(&self) -> Vec<ServedSession> {
        let sessions = self.sessions.lock().await;
        let mut served: Vec<_> = sessions
            .iter()
            .map(|(id, session)| ServedSession {
                id: *id,
                device_index: session.device_index,
                credential: session.authorization.clone(),
            })
            .collect();
        served.sort_by_key(|session| session.id);
        served
    }

    /// Ask one session's pump to close, and wait until its worker has released backend state.
    /// `Ok(false)` if no such session is registered.
    pub async fn close_session(&self, session_id: SessionId) -> Result<bool, String> {
        let (close, done) = {
            let sessions = self.sessions.lock().await;
            let Some(session) = sessions.get(&session_id) else {
                return Ok(false);
            };
            (session.close.clone(), session.done.subscribe())
        };

        close.send_replace(true);
        wait_for_completion(session_id, done).await.map(|()| true)
    }
}

/// Wait for a session's teardown to finish: clean, or the failure it reported.
async fn wait_for_completion(
    session_id: SessionId,
    mut done: watch::Receiver<SessionCompletion>,
) -> Result<(), String> {
    loop {
        match done.borrow().clone() {
            SessionCompletion::Running => {}
            SessionCompletion::Clean => return Ok(()),
            SessionCompletion::Failed(error) => return Err(error.to_string()),
        }
        done.changed()
            .await
            .map_err(|_| format!("Session {session_id} teardown signal was dropped"))?;
    }
}

impl<B, T> SessionService for SessionManager<B, T>
where
    B: BackendIr,
    T: TensorTransfer<B>,
{
    async fn reserve_session(
        &self,
        session_id: SessionId,
        device_index: u32,
        authorization: Arc<[u8]>,
    ) -> Result<(), String> {
        self.ensure_logger();
        let mut sessions = self.sessions.lock().await;
        if sessions.contains_key(&session_id) {
            return Err(format!(
                "Session {session_id} is already bound to another stream"
            ));
        }

        let (close, _) = watch::channel(false);
        let (done, _) = watch::channel(SessionCompletion::Running);
        sessions.insert(
            session_id,
            Session {
                task_sender: None,
                device_index,
                authorization,
                close,
                done,
                worker_done: None,
                finishing: false,
            },
        );
        Ok(())
    }

    /// One lock for the whole bind, so a `close_session` from another task cannot land inside it.
    async fn bind_session(&self, session_id: SessionId) -> Result<SessionChannels, String> {
        let mut sessions = self.sessions.lock().await;
        let session = sessions
            .get_mut(&session_id)
            .ok_or_else(|| format!("Session {session_id} was not reserved"))?;
        if *session.close.borrow() {
            return Err(format!("Session {session_id} was closed during admission"));
        }
        if session.task_sender.is_some() {
            return Err(format!(
                "Session {session_id} is already bound to another stream"
            ));
        }

        let device_index = session.device_index;
        let (response_sender, responses) = mpsc::channel(RESPONSE_CHANNEL_CAPACITY);
        // The session is pinned to its device for its whole lifetime. This runs inside the tokio
        // runtime, so the handler can capture the runtime handle its worker threads need for the
        // async parts of a task.
        let runner =
            TensorInterpreter::with_custom_ops(self.device(device_index), self.custom_ops.clone());
        let (task_sender, worker_done) = SessionHandler::spawn(
            session_id,
            runner,
            response_sender,
            SessionServices {
                transfer: self.transfer.clone(),
                local_comm: self.local_comm.clone(),
                queries: self.queries.clone(),
                probe: self.probe.clone(),
            },
            self.hosted(device_index).capture_lock.clone(),
        );
        session.task_sender = Some(task_sender.clone());
        session.worker_done = Some(worker_done);
        let close = session.close.subscribe();
        self.probe.emit(|| TelemetryEvent::SessionOpened {
            session: session_id,
            device: device_index,
        });
        Ok(SessionChannels {
            tasks: task_sender,
            responses,
            close,
        })
    }

    /// The device settings for `device_index`, used by the handshake before any session-specific
    /// runner is needed.
    fn device_settings(&self, device_index: u32) -> burn_std::DeviceSettings {
        use burn_backend::backend::DeviceOps;
        self.device(device_index).defaults()
    }

    /// The total number of devices this server hosts. Sent to the client on the init handshake so
    /// it can enumerate every device behind the address (see [`RemoteDevice::enumerate`]).
    fn device_count(&self) -> u32 {
        self.devices.len() as u32
    }

    fn dtype_usage(&self, device_index: u32) -> Vec<(burn_std::DType, DTypeUsageSet)> {
        let device = self.device(device_index);
        HANDSHAKE_DTYPES
            .into_iter()
            .map(|dtype| (dtype, B::dtype_usage(&device, dtype)))
            .collect()
    }

    /// Tear the session down and acknowledge it only once its worker has synced, dropped its
    /// interpreter, run backend memory cleanup and waited for that release on the device (Mere).
    /// Dropping the manager's task sender, once the pump has dropped its own, closes the
    /// dispatcher's channel, so the worker drains and exits. A failed teardown is reported to every
    /// closer and leaves the session registered, never acknowledged clean.
    async fn finish_session(&self, session_id: SessionId) -> Result<(), String> {
        let (worker_done, was_bound) = {
            let mut sessions = self.sessions.lock().await;
            let Some(session) = sessions.get_mut(&session_id) else {
                return Ok(());
            };
            if session.finishing {
                let done = session.done.subscribe();
                drop(sessions);
                return wait_for_completion(session_id, done).await;
            }
            session.finishing = true;
            session.close.send_replace(true);
            let was_bound = session.task_sender.take().is_some();
            (session.worker_done.take(), was_bound)
        };
        // Release a barrier or rendezvous the worker may be parked on, so it can drain.
        self.local_comm.abandon_session(session_id).await;

        let cleanup = match worker_done {
            Some(worker_done) => worker_done
                .await
                .map_err(|_| {
                    format!("Session {session_id} worker stopped before backend cleanup completed")
                })
                .and_then(|teardown| teardown),
            None => Ok(()),
        };

        let mut sessions = self.sessions.lock().await;
        let Some(session) = sessions.get_mut(&session_id) else {
            return Err(format!(
                "Session {session_id} disappeared before teardown completed"
            ));
        };
        match cleanup {
            Ok(()) => {
                let done = session.done.clone();
                sessions.remove(&session_id);
                done.send_replace(SessionCompletion::Clean);
                if was_bound {
                    self.probe.emit(|| TelemetryEvent::SessionClosed {
                        session: session_id,
                    });
                }
                Ok(())
            }
            Err(error) => {
                session
                    .done
                    .send_replace(SessionCompletion::Failed(Arc::from(error.as_str())));
                Err(error)
            }
        }
    }
}

#[cfg(test)]
mod teardown_tests {
    //! Mere: close reports a failed teardown instead of acknowledging it as clean.

    use super::*;
    use crate::server::worker::teardown_fault;
    use crate::shared::TransferCapability;
    use crate::{PeerAddr, PeerId};
    use burn_backend::TensorData;
    use burn_flex::Flex;
    use std::future::Future;

    struct NoTransfer;

    impl<B: BackendIr> TensorTransfer<B> for NoTransfer {
        fn expose_data(
            &self,
            _data: TensorData,
            _max_downloads: u32,
            _capability: TransferCapability,
            _target: PeerId,
        ) -> impl Future<Output = ()> + Send {
            async {}
        }

        fn download_tensor(
            &self,
            _remote: PeerAddr,
            _capability: TransferCapability,
        ) -> impl Future<Output = Option<TensorData>> + Send {
            async { None }
        }

        fn fail(
            &self,
            _capability: TransferCapability,
            _target: PeerId,
            _reason: String,
        ) -> impl Future<Output = ()> + Send {
            async {}
        }
    }

    async fn bound(manager: &SessionManager<Flex, NoTransfer>) -> SessionId {
        let session_id = SessionId::new();
        manager
            .reserve_session(session_id, 0, Arc::from(&b"teardown-test"[..]))
            .await
            .expect("reserve");
        // Dropping the binding drops its task sender and response queue, as a closed pump does.
        drop(manager.bind_session(session_id).await.expect("bind"));
        session_id
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_failed_teardown_sync_is_reported_and_never_acknowledged_clean() {
        let manager =
            SessionManager::<Flex, NoTransfer>::new(vec![Default::default()], Arc::new(NoTransfer));

        // Success path in the same run: an unarmed session closes clean and leaves the registry.
        let clean = bound(&manager).await;
        assert_eq!(manager.finish_session(clean).await, Ok(()));
        assert!(manager.sessions().await.iter().all(|s| s.id != clean));
        assert_eq!(manager.close_session(clean).await, Ok(false));

        // Injected failure of the post-release completion wait.
        let faulted = bound(&manager).await;
        teardown_fault::arm(faulted);
        let closed = manager.finish_session(faulted).await;
        assert!(
            closed
                .as_ref()
                .is_err_and(|error| error.contains("injected teardown sync failure")),
            "a failed teardown must be reported: {closed:?}"
        );
        assert!(
            manager.sessions().await.iter().any(|s| s.id == faulted),
            "a failed teardown must not be acknowledged by removing the session"
        );
        let again = manager.close_session(faulted).await;
        assert!(
            again
                .as_ref()
                .is_err_and(|error| error.contains("injected teardown sync failure")),
            "a later close must report the same failure: {again:?}"
        );
    }
}
