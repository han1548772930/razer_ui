//! Host 4.0.896 Protocol30 `write` retains a 500ms timeout until a matching
//! failure or a complete response; unrelated and partial notifications do not
//! cancel it. The current source is in
//! `.analysis/inputs/host-current/asar/e7a524aca177/electron/modules/noble/`.
use super::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

struct WriteState {
    status: Cell<AsyncStatus>,
    cancel_finishes: Cell<bool>,
    cancel_fails: Cell<bool>,
    close_fails: Cell<bool>,
    status_fails: Cell<bool>,
    cancels: Cell<usize>,
    closes: Cell<usize>,
    drops: Cell<usize>,
}
impl WriteState {
    fn new(status: AsyncStatus) -> Rc<Self> {
        Rc::new(Self {
            status: Cell::new(status),
            cancel_finishes: Cell::new(true),
            cancel_fails: Cell::new(false),
            close_fails: Cell::new(false),
            status_fails: Cell::new(false),
            cancels: Cell::new(0),
            closes: Cell::new(0),
            drops: Cell::new(0),
        })
    }
}
struct MockWrite(Rc<WriteState>);
impl Drop for MockWrite {
    fn drop(&mut self) {
        self.0.drops.set(self.0.drops.get() + 1);
    }
}
impl NativeOperation for MockWrite {
    fn status(&self) -> anyhow::Result<AsyncStatus> {
        ensure!(!self.0.status_fails.get(), "simulated Status failure");
        Ok(self.0.status.get())
    }
    fn cancel(&self) -> anyhow::Result<()> {
        self.0.cancels.set(self.0.cancels.get() + 1);
        ensure!(!self.0.cancel_fails.get(), "simulated Cancel failure");
        if self.0.cancel_finishes.get() {
            self.0.status.set(AsyncStatus::Canceled);
        }
        Ok(())
    }
    fn close(&self) -> anyhow::Result<()> {
        self.0.closes.set(self.0.closes.get() + 1);
        ensure!(
            self.0.status.get() != AsyncStatus::Started,
            "Close called before terminal status"
        );
        ensure!(!self.0.close_fails.get(), "simulated Close failure");
        Ok(())
    }
}

fn completed_write() -> MockWrite {
    MockWrite(WriteState::new(AsyncStatus::Completed))
}

struct AcquiredDeviceState {
    get_results: Cell<usize>,
    get_fails: Cell<bool>,
    error_code_fails: Cell<bool>,
    live_results: Cell<usize>,
    releases: Cell<usize>,
    activations: Cell<usize>,
    current: Cell<bool>,
    invalidate_on_result: Cell<bool>,
}
impl AcquiredDeviceState {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            get_results: Cell::new(0),
            get_fails: Cell::new(false),
            error_code_fails: Cell::new(false),
            live_results: Cell::new(0),
            releases: Cell::new(0),
            activations: Cell::new(0),
            current: Cell::new(true),
            invalidate_on_result: Cell::new(false),
        })
    }
}
struct MockAcquiredDevice(Rc<AcquiredDeviceState>);
impl Drop for MockAcquiredDevice {
    fn drop(&mut self) {
        self.0.live_results.set(self.0.live_results.get() - 1);
        self.0.releases.set(self.0.releases.get() + 1);
    }
}
struct MockDeviceOperation {
    operation: MockWrite,
    state: Rc<AcquiredDeviceState>,
}
impl NativeOperation for MockDeviceOperation {
    fn status(&self) -> anyhow::Result<AsyncStatus> {
        self.operation.status()
    }
    fn cancel(&self) -> anyhow::Result<()> {
        self.operation.cancel()
    }
    fn close(&self) -> anyhow::Result<()> {
        self.operation.close()
    }
}
impl DeviceOperation for MockDeviceOperation {
    type Device = MockAcquiredDevice;
    fn results(&self) -> anyhow::Result<Self::Device> {
        assert_eq!(self.operation.0.status.get(), AsyncStatus::Completed);
        self.state.get_results.set(self.state.get_results.get() + 1);
        ensure!(!self.state.get_fails.get(), "simulated GetResults failure");
        self.state
            .live_results
            .set(self.state.live_results.get() + 1);
        if self.state.invalidate_on_result.get() {
            self.state.current.set(false);
        }
        Ok(MockAcquiredDevice(self.state.clone()))
    }
    fn error_code(&self) -> anyhow::Result<String> {
        ensure!(
            !self.state.error_code_fails.get(),
            "simulated ErrorCode failure"
        );
        Ok("simulated HRESULT".into())
    }
}

struct RetainedResourceState {
    callbacks_live: Cell<bool>,
    service_live: Cell<bool>,
    device_live: Cell<bool>,
    owner_drops: Cell<usize>,
    dropped_live: Cell<bool>,
    panic_on_finish: Cell<bool>,
    notifications_enabled: Cell<bool>,
    disable_dispatches: Cell<usize>,
}
impl RetainedResourceState {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            callbacks_live: Cell::new(true),
            service_live: Cell::new(true),
            device_live: Cell::new(true),
            owner_drops: Cell::new(0),
            dropped_live: Cell::new(false),
            panic_on_finish: Cell::new(false),
            notifications_enabled: Cell::new(false),
            disable_dispatches: Cell::new(0),
        })
    }
}
struct MockRetainedConnection {
    state: Rc<RetainedResourceState>,
    acquisition: DeviceAcquisition<MockDeviceOperation>,
    acquired_device: Option<MockAcquiredDevice>,
    operations: ConnectionOperations<MockWrite, MockWrite, MockWrite>,
    cleanup: CleanupState,
    on_finish: Option<Box<dyn Fn()>>,
    on_drop: Option<Box<dyn Fn()>>,
}
impl MockRetainedConnection {
    fn new(state: Rc<RetainedResourceState>, writes: impl IntoIterator<Item = MockWrite>) -> Self {
        let mut pending = PendingOperations::default();
        for write in writes {
            pending.push(write);
        }
        Self {
            state,
            acquisition: DeviceAcquisition::default(),
            acquired_device: None,
            operations: ConnectionOperations {
                writes: pending,
                ..ConnectionOperations::default()
            },
            cleanup: CleanupState::default(),
            on_finish: None,
            on_drop: None,
        }
    }
}
impl RetainedOwner for MockRetainedConnection {
    fn finish(&mut self) -> anyhow::Result<()> {
        if let Some(on_finish) = &self.on_finish {
            on_finish();
        }
        assert!(!self.state.panic_on_finish.get(), "simulated cleanup panic");
        let started = Instant::now();
        let elapsed = Cell::new(Duration::ZERO);
        if let Err(error) = self.acquisition.cleanup(
            || started + elapsed.get(),
            |duration| elapsed.set(elapsed.get() + duration),
        ) {
            self.cleanup.record([format!("{error:#}")]);
        }
        if let Err(error) = self.operations.cleanup(
            || started + elapsed.get(),
            |duration| elapsed.set(elapsed.get() + duration),
        ) {
            self.cleanup.record([format!("{error:#}")]);
        }
        if self.acquisition.is_empty() && self.operations.is_empty() {
            if self.state.notifications_enabled.replace(false) {
                self.state
                    .disable_dispatches
                    .set(self.state.disable_dispatches.get() + 1);
            }
            self.state.callbacks_live.set(false);
            self.state.service_live.set(false);
            self.state.device_live.set(false);
            self.acquired_device = None;
        }
        self.cleanup.result()
    }
    fn is_closed(&self) -> bool {
        !self.state.device_live.get() && self.acquisition.is_empty() && self.operations.is_empty()
    }
}
impl Drop for MockRetainedConnection {
    fn drop(&mut self) {
        if let Some(on_drop) = &self.on_drop {
            on_drop();
        }
        self.state.owner_drops.set(self.state.owner_drops.get() + 1);
        if self.state.callbacks_live.get()
            || self.state.service_live.get()
            || self.state.device_live.get()
            || !self.acquisition.is_empty()
        {
            self.state.dropped_live.set(true);
        }
    }
}
struct ScopedRetainedConnection<'a> {
    registry: &'a Mutex<RetainedRegistry<MockRetainedConnection>>,
    container: CleanupContainer,
    owner: Option<MockRetainedConnection>,
}
impl RetainedOwner for ScopedRetainedConnection<'_> {
    fn finish(&mut self) -> anyhow::Result<()> {
        self.owner.as_mut().unwrap().finish()
    }
    fn is_closed(&self) -> bool {
        self.owner.as_ref().unwrap().is_closed()
    }
}
impl Drop for ScopedRetainedConnection<'_> {
    fn drop(&mut self) {
        let failure = self
            .owner
            .as_mut()
            .unwrap()
            .finish()
            .err()
            .map(|error| format!("{error:#}"));
        let owner = self.owner.take().filter(|owner| !owner.is_closed());
        retain_owner(self.registry, self.container.clone(), owner, failure);
    }
}

fn reap_retained(
    registry: &Mutex<RetainedRegistry<MockRetainedConnection>>,
    container: CleanupContainer,
) -> anyhow::Result<()> {
    if let Some(mut retained) = RetainedCleanup::begin(registry, container)? {
        retained.finish();
        retained.result()?;
    }
    Ok(())
}

fn predevice_owner(state: Rc<RetainedResourceState>) -> MockRetainedConnection {
    state.callbacks_live.set(false);
    state.service_live.set(false);
    state.device_live.set(false);
    MockRetainedConnection::new(state, [])
}

fn acquire_mock_device(
    owner: &mut MockRetainedConnection,
    operation: Rc<WriteState>,
    device: Rc<AcquiredDeviceState>,
    validate: &dyn Fn() -> anyhow::Result<()>,
    advance: impl FnMut(Duration),
) -> anyhow::Result<()> {
    owner.acquisition.start(MockDeviceOperation {
        operation: MockWrite(operation),
        state: device.clone(),
    })?;
    let started = Instant::now();
    let elapsed = Cell::new(Duration::ZERO);
    let mut advance = advance;
    owner.acquisition.wait(
        CONNECT_TIMEOUT,
        validate,
        || started + elapsed.get(),
        |duration| {
            elapsed.set(elapsed.get() + duration);
            advance(duration);
        },
    )?;
    validate()?;
    owner.acquired_device = Some(owner.acquisition.take_result()?);
    device.activations.set(device.activations.get() + 1);
    owner.state.device_live.set(true);
    owner.state.callbacks_live.set(true);
    Ok(())
}

#[test]
fn protocol30_device_acquisition_accepts_only_a_current_result_after_the_operation_is_closed() {
    for invalidate in [false, true] {
        let state = RetainedResourceState::new();
        let operation = WriteState::new(AsyncStatus::Completed);
        let device = AcquiredDeviceState::new();
        device.invalidate_on_result.set(invalidate);
        let result = connect_owned(
            || Ok(()),
            || Ok(predevice_owner(state.clone())),
            |owner| {
                acquire_mock_device(
                    owner,
                    operation.clone(),
                    device.clone(),
                    &|| {
                        ensure!(device.current.get(), "device generation changed");
                        Ok(())
                    },
                    |_| unreachable!(),
                )
            },
        );
        if invalidate {
            let error = result.err().unwrap();
            assert!(error.to_string().contains("generation changed"));
            assert_eq!(device.activations.get(), 0);
        } else {
            let mut connection = result.ok().unwrap();
            assert_eq!(device.activations.get(), 1);
            assert_eq!(operation.closes.get(), 1);
            assert_eq!(device.live_results.get(), 1);
            assert_eq!(device.releases.get(), 0);
            connection.finish().unwrap();
            drop(connection);
        }
        assert_eq!(operation.cancels.get(), 0);
        assert_eq!(operation.closes.get(), 1);
        assert_eq!(operation.drops.get(), 1);
        assert_eq!(device.get_results.get(), 1);
        assert_eq!(device.live_results.get(), 0);
        assert_eq!(device.releases.get(), 1);
        assert_eq!(state.owner_drops.get(), 1);
        assert!(!state.dropped_live.get());
    }
}

#[test]
fn protocol30_device_acquisition_timeout_and_cancellation_retain_late_results_and_block_reconnect()
{
    for canceled in [false, true] {
        let registry = Mutex::new(RetainedRegistry::default());
        let container = CleanupContainer::new("container");
        let state = RetainedResourceState::new();
        let operation = WriteState::new(AsyncStatus::Started);
        operation.cancel_finishes.set(false);
        let device = AcquiredDeviceState::new();
        let error = connect_owned(
            || Ok(()),
            || {
                Ok(ScopedRetainedConnection {
                    registry: &registry,
                    container: container.clone(),
                    owner: Some(predevice_owner(state.clone())),
                })
            },
            |connection| {
                acquire_mock_device(
                    connection.owner.as_mut().unwrap(),
                    operation.clone(),
                    device.clone(),
                    &|| {
                        ensure!(!canceled, "device acquisition owner canceled");
                        Ok(())
                    },
                    |_| {},
                )
            },
        )
        .err()
        .unwrap();
        assert!(!connection_retryable(&error));
        if canceled {
            assert!(format!("{error:#}").contains("owner canceled"));
        } else {
            assert!(error.downcast_ref::<ResponseTimeout>().is_some());
        }
        assert_eq!(operation.cancels.get(), 1);
        assert_eq!(operation.closes.get(), 0);
        assert_eq!(operation.drops.get(), 0);
        assert_eq!(device.get_results.get(), 0);
        assert_eq!(state.owner_drops.get(), 0);
        let opens = Cell::new(0);
        let error = connect_owned(
            || reap_retained(&registry, container.clone()),
            || {
                opens.set(opens.get() + 1);
                Ok(predevice_owner(RetainedResourceState::new()))
            },
            |_| Ok(()),
        )
        .err()
        .unwrap();
        assert!(!connection_retryable(&error));
        assert_eq!(opens.get(), 0);
        operation.status.set(AsyncStatus::Completed);
        assert!(reap_retained(&registry, container.clone()).is_err());
        assert_eq!(operation.cancels.get(), 1);
        assert_eq!(operation.closes.get(), 1);
        assert_eq!(operation.drops.get(), 1);
        assert_eq!(device.get_results.get(), 1);
        assert_eq!(device.activations.get(), 0);
        assert_eq!(device.live_results.get(), 0);
        assert_eq!(device.releases.get(), 1);
        assert_eq!(state.owner_drops.get(), 1);
        assert!(!state.dropped_live.get());
        let next = AcquiredDeviceState::new();
        let next_operation = WriteState::new(AsyncStatus::Completed);
        let mut connection = connect_owned(
            || reap_retained(&registry, container),
            || Ok(predevice_owner(RetainedResourceState::new())),
            |owner| {
                acquire_mock_device(
                    owner,
                    next_operation,
                    next.clone(),
                    &|| Ok(()),
                    |_| unreachable!(),
                )
            },
        )
        .ok()
        .unwrap();
        assert_eq!(next.activations.get(), 1);
        assert_eq!(device.activations.get(), 0);
        connection.finish().unwrap();
        drop(connection);
        assert!(registry.lock().unwrap().history.result().is_err());
    }
}

#[test]
fn protocol30_device_acquisition_close_failure_retains_the_already_observed_device_without_rereading()
 {
    let registry = Mutex::new(RetainedRegistry::default());
    let container = CleanupContainer::new("container");
    let state = RetainedResourceState::new();
    let operation = WriteState::new(AsyncStatus::Completed);
    operation.close_fails.set(true);
    let device = AcquiredDeviceState::new();
    let error = connect_owned(
        || Ok(()),
        || {
            Ok(ScopedRetainedConnection {
                registry: &registry,
                container: container.clone(),
                owner: Some(predevice_owner(state.clone())),
            })
        },
        |connection| {
            acquire_mock_device(
                connection.owner.as_mut().unwrap(),
                operation.clone(),
                device.clone(),
                &|| Ok(()),
                |_| unreachable!(),
            )
        },
    )
    .err()
    .unwrap();
    assert!(!connection_retryable(&error));
    assert!(format!("{error:#}").contains("Close"));
    assert_eq!(device.get_results.get(), 1);
    assert_eq!(device.live_results.get(), 1);
    assert_eq!(device.releases.get(), 0);
    assert_eq!(device.activations.get(), 0);
    assert_eq!(operation.drops.get(), 0);
    assert_eq!(state.owner_drops.get(), 0);
    let error = reap_retained(&registry, container.clone()).unwrap_err();
    assert!(!connection_retryable(&error));
    assert_eq!(device.get_results.get(), 1);
    assert_eq!(device.live_results.get(), 1);
    operation.close_fails.set(false);
    assert!(reap_retained(&registry, container).is_err());
    assert_eq!(device.get_results.get(), 1);
    assert_eq!(device.live_results.get(), 0);
    assert_eq!(device.releases.get(), 1);
    assert_eq!(device.activations.get(), 0);
    assert_eq!(operation.cancels.get(), 0);
    assert_eq!(operation.drops.get(), 1);
    assert_eq!(state.owner_drops.get(), 1);
    assert!(!state.dropped_live.get());
    assert!(registry.lock().unwrap().history.result().is_err());
}

#[test]
fn protocol30_device_acquisition_status_cancel_and_get_results_failures_keep_the_retained_owner() {
    for failure in ["Status", "Cancel", "GetResults"] {
        let registry = Mutex::new(RetainedRegistry::default());
        let container = CleanupContainer::new("container");
        let state = RetainedResourceState::new();
        let operation = WriteState::new(if failure == "GetResults" {
            AsyncStatus::Completed
        } else {
            AsyncStatus::Started
        });
        operation.status_fails.set(failure == "Status");
        operation.cancel_fails.set(failure == "Cancel");
        let device = AcquiredDeviceState::new();
        device.get_fails.set(failure == "GetResults");
        let error = connect_owned(
            || Ok(()),
            || {
                Ok(ScopedRetainedConnection {
                    registry: &registry,
                    container: container.clone(),
                    owner: Some(predevice_owner(state.clone())),
                })
            },
            |connection| {
                acquire_mock_device(
                    connection.owner.as_mut().unwrap(),
                    operation.clone(),
                    device.clone(),
                    &|| {
                        ensure!(failure != "Cancel", "device acquisition owner canceled");
                        Ok(())
                    },
                    |_| {},
                )
            },
        )
        .err()
        .unwrap();
        assert!(!connection_retryable(&error), "{failure}");
        assert!(format!("{error:#}").contains(failure));
        assert_eq!(operation.closes.get(), 0);
        assert_eq!(operation.drops.get(), 0);
        assert_eq!(state.owner_drops.get(), 0);
        assert_eq!(device.activations.get(), 0);
        operation.status_fails.set(false);
        operation.cancel_fails.set(false);
        device.get_fails.set(false);
        operation.status.set(AsyncStatus::Completed);
        assert!(shutdown_retained(&registry, |key| reap_retained(&registry, key.clone())).is_err());
        assert_eq!(operation.closes.get(), 1);
        assert_eq!(operation.drops.get(), 1);
        assert_eq!(device.live_results.get(), 0);
        assert_eq!(device.releases.get(), 1);
        assert_eq!(device.activations.get(), 0);
        assert_eq!(state.owner_drops.get(), 1);
        assert!(!state.dropped_live.get());
        let retained = registry.lock().unwrap();
        assert!(retained.containers.is_empty());
        assert!(retained.history.result().is_err());
    }
}

#[test]
fn protocol30_device_acquisition_terminal_errors_do_not_read_a_device_result() {
    for status in [AsyncStatus::Error, AsyncStatus::Canceled] {
        for error_code_fails in [false, true] {
            let state = RetainedResourceState::new();
            let operation = WriteState::new(status);
            let device = AcquiredDeviceState::new();
            device.error_code_fails.set(error_code_fails);
            let error = connect_owned(
                || Ok(()),
                || Ok(predevice_owner(state.clone())),
                |owner| {
                    acquire_mock_device(
                        owner,
                        operation.clone(),
                        device.clone(),
                        &|| Ok(()),
                        |_| unreachable!(),
                    )
                },
            )
            .err()
            .unwrap();
            assert_eq!(connection_retryable(&error), !error_code_fails);
            assert_eq!(device.get_results.get(), 0);
            assert_eq!(device.activations.get(), 0);
            assert_eq!(operation.cancels.get(), 0);
            assert_eq!(operation.closes.get(), 1);
            assert_eq!(operation.drops.get(), 1);
            assert_eq!(state.owner_drops.get(), 1);
            assert!(!state.dropped_live.get());
        }
    }
}

#[test]
fn protocol30_device_acquisition_shutdown_keeps_started_operations_then_releases_late_devices() {
    let registry = Mutex::new(RetainedRegistry::default());
    let container = CleanupContainer::new("CONTAINER");
    let state = RetainedResourceState::new();
    let operation = WriteState::new(AsyncStatus::Started);
    operation.cancel_finishes.set(false);
    let device = AcquiredDeviceState::new();
    {
        let mut owner = predevice_owner(state.clone());
        owner
            .acquisition
            .start(MockDeviceOperation {
                operation: MockWrite(operation.clone()),
                state: device.clone(),
            })
            .unwrap();
        let _connection = ScopedRetainedConnection {
            registry: &registry,
            container,
            owner: Some(owner),
        };
    }
    assert_eq!(operation.drops.get(), 0);
    assert_eq!(state.owner_drops.get(), 0);
    let leases = Cell::new(0);
    assert_eq!(retained_pending_count(&registry), 1);
    let result = shutdown_retained(&registry, |key| {
        assert_eq!(key.0, "container");
        leases.set(leases.get() + 1);
        assert_ne!(retained_pending_count(&registry), 0);
        reap_retained(&registry, key.clone())
    });
    assert!(result.is_err());
    assert_eq!(leases.get(), 1);
    assert_eq!(operation.cancels.get(), 1);
    assert_eq!(operation.closes.get(), 0);
    assert_eq!(operation.drops.get(), 0);
    assert_eq!(retained_pending_count(&registry), 1);
    operation.status.set(AsyncStatus::Completed);
    let result = shutdown_retained(&registry, |key| {
        leases.set(leases.get() + 1);
        reap_retained(&registry, key.clone())
    });
    assert!(result.is_err());
    assert_eq!(leases.get(), 2);
    assert_eq!(operation.closes.get(), 1);
    assert_eq!(operation.drops.get(), 1);
    assert_eq!(device.get_results.get(), 1);
    assert_eq!(device.activations.get(), 0);
    assert_eq!(device.releases.get(), 1);
    assert_eq!(state.owner_drops.get(), 1);
    assert!(!state.dropped_live.get());
    assert!(shutdown_retained(&registry, |_| panic!("closed owner was reacquired")).is_err());
    assert!(registry.lock().unwrap().containers.is_empty());
    assert_eq!(retained_pending_count(&registry), 0);
}

#[derive(Clone, Copy, Debug)]
enum DiscoveryPhase {
    Services,
    Characteristics,
}
impl DiscoveryPhase {
    fn pending(
        self,
        operations: &mut ConnectionOperations<MockWrite, MockWrite, MockWrite>,
    ) -> &mut PendingOperations<MockWrite> {
        match self {
            Self::Services => &mut operations.services,
            Self::Characteristics => &mut operations.characteristics,
        }
    }
    fn has_service(self) -> bool {
        matches!(self, Self::Characteristics)
    }
}

#[test]
fn protocol30_discovery_cancellation_keeps_parent_resources_and_blocks_reconnect_until_terminal() {
    for phase in [DiscoveryPhase::Services, DiscoveryPhase::Characteristics] {
        for canceled in [false, true] {
            let registry = Mutex::new(RetainedRegistry::default());
            let container = CleanupContainer::new("container");
            let state = RetainedResourceState::new();
            state.service_live.set(phase.has_service());
            let operation = WriteState::new(AsyncStatus::Started);
            operation.cancel_finishes.set(false);
            let error = connect_owned(
                || Ok(()),
                || {
                    Ok(ScopedRetainedConnection {
                        registry: &registry,
                        container: container.clone(),
                        owner: Some(MockRetainedConnection::new(state.clone(), [])),
                    })
                },
                |connection| {
                    phase
                        .pending(&mut connection.owner.as_mut().unwrap().operations)
                        .dispatch_and_wait(
                            || Ok(MockWrite(operation.clone())),
                            |_| {
                                assert!(state.device_live.get());
                                assert_eq!(state.service_live.get(), phase.has_service());
                                if canceled {
                                    Err(anyhow::anyhow!("discovery owner canceled"))
                                } else {
                                    Err(ResponseTimeout.into())
                                }
                            },
                        )
                },
            )
            .err()
            .unwrap();
            assert!(!connection_retryable(&error), "{phase:?}");
            assert_eq!(operation.cancels.get(), 1);
            assert_eq!(operation.closes.get(), 0);
            assert_eq!(operation.drops.get(), 0);
            assert!(state.callbacks_live.get());
            assert!(state.device_live.get());
            assert_eq!(state.service_live.get(), phase.has_service());
            let opens = Cell::new(0);
            let error = connect_owned(
                || reap_retained(&registry, container.clone()),
                || {
                    opens.set(opens.get() + 1);
                    Ok(MockRetainedConnection::new(
                        RetainedResourceState::new(),
                        [],
                    ))
                },
                |_| Ok(()),
            )
            .err()
            .unwrap();
            assert!(!connection_retryable(&error));
            assert_eq!(opens.get(), 0);
            assert_eq!(operation.closes.get(), 0);
            assert_eq!(state.owner_drops.get(), 0);
            operation.status.set(AsyncStatus::Canceled);
            assert!(reap_retained(&registry, container.clone()).is_err());
            assert_eq!(operation.cancels.get(), 1);
            assert_eq!(operation.closes.get(), 1);
            assert_eq!(operation.drops.get(), 1);
            assert_eq!(state.owner_drops.get(), 1);
            assert!(!state.dropped_live.get());
            let fresh = RetainedResourceState::new();
            let mut connection = connect_owned(
                || reap_retained(&registry, container),
                || Ok(MockRetainedConnection::new(fresh.clone(), [])),
                |_| Ok(()),
            )
            .ok()
            .unwrap();
            connection.finish().unwrap();
            drop(connection);
            assert!(!fresh.dropped_live.get());
            assert!(registry.lock().unwrap().history.result().is_err());
        }
    }
}

#[test]
fn protocol30_discovery_cancel_failure_retains_the_operation_and_preserves_the_original_error() {
    for phase in [DiscoveryPhase::Services, DiscoveryPhase::Characteristics] {
        let registry = Mutex::new(RetainedRegistry::default());
        let container = CleanupContainer::new("container");
        let state = RetainedResourceState::new();
        state.service_live.set(phase.has_service());
        let operation = WriteState::new(AsyncStatus::Started);
        operation.cancel_fails.set(true);
        let error = connect_owned(
            || Ok(()),
            || {
                Ok(ScopedRetainedConnection {
                    registry: &registry,
                    container: container.clone(),
                    owner: Some(MockRetainedConnection::new(state.clone(), [])),
                })
            },
            |connection| {
                phase
                    .pending(&mut connection.owner.as_mut().unwrap().operations)
                    .dispatch_and_wait(
                        || Ok(MockWrite(operation.clone())),
                        |_| Err(anyhow::anyhow!("discovery owner canceled")),
                    )
            },
        )
        .err()
        .unwrap();
        assert!(!connection_retryable(&error));
        assert!(format!("{error:#}").contains("discovery owner canceled"));
        assert!(format!("{error:#}").contains("Cancel"));
        assert!(operation.cancels.get() > 0);
        assert_eq!(operation.closes.get(), 0);
        assert_eq!(operation.drops.get(), 0);
        assert_eq!(state.owner_drops.get(), 0);
        assert!(state.device_live.get());
        assert_eq!(state.service_live.get(), phase.has_service());
        operation.cancel_fails.set(false);
        operation.status.set(AsyncStatus::Canceled);
        assert!(reap_retained(&registry, container).is_err());
        assert_eq!(operation.closes.get(), 1);
        assert_eq!(state.owner_drops.get(), 1);
        assert!(!state.dropped_live.get());
        assert!(registry.lock().unwrap().history.result().is_err());
    }
}

#[test]
fn protocol30_discovery_close_failure_keeps_terminal_operations_and_dependencies_owned() {
    for phase in [DiscoveryPhase::Services, DiscoveryPhase::Characteristics] {
        let registry = Mutex::new(RetainedRegistry::default());
        let container = CleanupContainer::new("container");
        let state = RetainedResourceState::new();
        state.service_live.set(phase.has_service());
        let operation = WriteState::new(AsyncStatus::Started);
        operation.close_fails.set(true);
        let error = connect_owned(
            || Ok(()),
            || {
                Ok(ScopedRetainedConnection {
                    registry: &registry,
                    container: container.clone(),
                    owner: Some(MockRetainedConnection::new(state.clone(), [])),
                })
            },
            |connection| {
                phase
                    .pending(&mut connection.owner.as_mut().unwrap().operations)
                    .dispatch_and_wait(
                        || Ok(MockWrite(operation.clone())),
                        |_| {
                            operation.status.set(AsyncStatus::Completed);
                            Err(anyhow::anyhow!("discovery GATT response failed"))
                        },
                    )
            },
        )
        .err()
        .unwrap();
        assert!(!connection_retryable(&error));
        assert!(format!("{error:#}").contains("discovery GATT response failed"));
        assert!(format!("{error:#}").contains("Close"));
        assert_eq!(operation.cancels.get(), 0);
        assert!(operation.closes.get() > 0);
        assert_eq!(operation.drops.get(), 0);
        assert!(state.device_live.get());
        assert_eq!(state.service_live.get(), phase.has_service());
        let opens = Cell::new(0);
        let error = connect_owned(
            || reap_retained(&registry, container.clone()),
            || {
                opens.set(opens.get() + 1);
                Ok(MockRetainedConnection::new(
                    RetainedResourceState::new(),
                    [],
                ))
            },
            |_| Ok(()),
        )
        .err()
        .unwrap();
        assert!(!connection_retryable(&error));
        assert_eq!(opens.get(), 0);
        operation.close_fails.set(false);
        assert!(reap_retained(&registry, container).is_err());
        assert_eq!(operation.drops.get(), 1);
        assert_eq!(state.owner_drops.get(), 1);
        assert!(!state.dropped_live.get());
        assert!(registry.lock().unwrap().history.result().is_err());
    }
}

#[test]
fn protocol30_discovery_consumes_typed_results_before_closing_completed_operations() {
    for phase in [DiscoveryPhase::Services, DiscoveryPhase::Characteristics] {
        let state = RetainedResourceState::new();
        state.service_live.set(phase.has_service());
        let operation = WriteState::new(AsyncStatus::Started);
        let expected = match phase {
            DiscoveryPhase::Services => vec![SERVICE],
            DiscoveryPhase::Characteristics => vec![WRITE, NOTIFY],
        };
        let mut connection = connect_owned(
            || Ok(()),
            || Ok(MockRetainedConnection::new(state.clone(), [])),
            |connection| {
                let values = phase
                    .pending(&mut connection.operations)
                    .dispatch_and_wait(
                        || Ok(MockWrite(operation.clone())),
                        |_| {
                            assert_eq!(operation.closes.get(), 0);
                            operation.status.set(AsyncStatus::Completed);
                            Ok(expected.clone())
                        },
                    )?;
                assert_eq!(values, expected);
                assert_eq!(operation.closes.get(), 0);
                connection
                    .cleanup
                    .record_result(phase.pending(&mut connection.operations).reap())?;
                assert_eq!(operation.closes.get(), 1);
                assert_eq!(operation.drops.get(), 1);
                assert!(state.device_live.get());
                assert_eq!(state.service_live.get(), phase.has_service());
                Ok(())
            },
        )
        .ok()
        .unwrap();
        assert_eq!(operation.cancels.get(), 0);
        connection.finish().unwrap();
        drop(connection);
        assert_eq!(operation.closes.get(), 1);
        assert_eq!(state.owner_drops.get(), 1);
        assert!(!state.dropped_live.get());
    }
}

#[test]
fn protocol30_discovery_reap_error_remains_fatal_after_all_resources_are_released() {
    for phase in [DiscoveryPhase::Services, DiscoveryPhase::Characteristics] {
        let registry = Mutex::new(RetainedRegistry::default());
        let container = CleanupContainer::new("container");
        let state = RetainedResourceState::new();
        state.service_live.set(phase.has_service());
        let operation = WriteState::new(AsyncStatus::Started);
        let error = connect_owned(
            || Ok(()),
            || {
                Ok(ScopedRetainedConnection {
                    registry: &registry,
                    container: container.clone(),
                    owner: Some(MockRetainedConnection::new(state.clone(), [])),
                })
            },
            |connection| {
                let owner = connection.owner.as_mut().unwrap();
                phase.pending(&mut owner.operations).dispatch_and_wait(
                    || Ok(MockWrite(operation.clone())),
                    |_| {
                        operation.status.set(AsyncStatus::Completed);
                        Ok(())
                    },
                )?;
                operation.close_fails.set(true);
                let result = owner
                    .cleanup
                    .record_result(phase.pending(&mut owner.operations).reap());
                operation.close_fails.set(false);
                result
            },
        )
        .err()
        .unwrap();
        assert!(!connection_retryable(&error));
        assert!(format!("{error:#}").contains("Close"));
        assert_eq!(operation.cancels.get(), 0);
        assert_eq!(operation.closes.get(), 2);
        assert_eq!(operation.drops.get(), 1);
        assert_eq!(state.owner_drops.get(), 1);
        assert!(!state.dropped_live.get());
        reap_retained(&registry, container).unwrap();
        let retained = registry.lock().unwrap();
        assert!(retained.containers.is_empty());
        assert!(retained.history.result().is_err());
    }
}

#[test]
fn protocol30_connection_gate_rejects_reconnect_while_old_operations_remain() {
    let registry = Mutex::new(RetainedRegistry::default());
    let container = CleanupContainer::new("container");
    let old = RetainedResourceState::new();
    let write = WriteState::new(AsyncStatus::Started);
    write.cancel_finishes.set(false);
    retain_owner(
        &registry,
        container.clone(),
        Some(MockRetainedConnection::new(
            old.clone(),
            [MockWrite(write.clone())],
        )),
        [],
    );
    let opens = Cell::new(0);
    let error = connect_owned(
        || reap_retained(&registry, container.clone()),
        || {
            opens.set(opens.get() + 1);
            Ok(MockRetainedConnection::new(
                RetainedResourceState::new(),
                [],
            ))
        },
        |_| Ok(()),
    )
    .err()
    .unwrap();
    assert!(!connection_retryable(&error));
    assert_eq!(opens.get(), 0);
    assert_eq!(write.closes.get(), 0);
    assert_eq!(write.drops.get(), 0);
    assert_eq!(old.owner_drops.get(), 0);
    assert!(old.device_live.get());
    write.status.set(AsyncStatus::Canceled);
    assert!(reap_retained(&registry, container).is_err());
    assert_eq!(write.closes.get(), 1);
    assert_eq!(old.owner_drops.get(), 1);
    assert!(!old.dropped_live.get());
}

#[test]
fn protocol30_connection_gate_releases_old_dependencies_before_opening_a_new_connection() {
    let registry = Mutex::new(RetainedRegistry::default());
    let container = CleanupContainer::new("container");
    let old = RetainedResourceState::new();
    let write = WriteState::new(AsyncStatus::Completed);
    retain_owner(
        &registry,
        container.clone(),
        Some(MockRetainedConnection::new(
            old.clone(),
            [MockWrite(write.clone())],
        )),
        [],
    );
    let new = RetainedResourceState::new();
    let mut connection = connect_owned(
        || reap_retained(&registry, container.clone()),
        || {
            assert_eq!(write.closes.get(), 1);
            assert_eq!(old.owner_drops.get(), 1);
            assert!(!old.device_live.get());
            Ok(MockRetainedConnection::new(new.clone(), []))
        },
        |_| Ok(()),
    )
    .ok()
    .unwrap();
    connection.finish().unwrap();
    drop(connection);
    assert!(!old.dropped_live.get());
    assert!(!new.dropped_live.get());
    assert!(registry.lock().unwrap().containers.is_empty());
}

#[test]
fn protocol30_failed_initialization_explicitly_finishes_and_preserves_a_sticky_cleanup_failure() {
    for sticky_failure in [false, true] {
        let registry = Mutex::new(RetainedRegistry::default());
        let container = CleanupContainer::new("container");
        let state = RetainedResourceState::new();
        let finishes = Rc::new(Cell::new(0));
        let mut owner = MockRetainedConnection::new(state.clone(), []);
        let calls = finishes.clone();
        owner.on_finish = Some(Box::new(move || calls.set(calls.get() + 1)));
        if sticky_failure {
            owner.cleanup.record(["earlier Close failure".into()]);
        }
        let error = connect_owned(
            || Ok(()),
            || {
                Ok(ScopedRetainedConnection {
                    registry: &registry,
                    container: container.clone(),
                    owner: Some(owner),
                })
            },
            |_| Err(anyhow::anyhow!("notification registration failed")),
        )
        .err()
        .unwrap();
        // One explicit finish precedes the scoped owner's Drop safety net.
        assert_eq!(finishes.get(), 2);
        assert_eq!(state.owner_drops.get(), 1);
        assert!(!state.dropped_live.get());
        assert_eq!(connection_retryable(&error), !sticky_failure);
        assert!(format!("{error:#}").contains("notification registration failed"));
        // A clean later gate cannot turn this attempt's cleanup error into success.
        reap_retained(&registry, container).unwrap();
        let retained = registry.lock().unwrap();
        assert!(retained.containers.is_empty());
        assert_eq!(retained.history.result().is_err(), sticky_failure);
        if sticky_failure {
            assert!(format!("{error:#}").contains("earlier Close failure"));
            assert!(!retryable(&error));
        }
    }
}

#[test]
fn protocol30_cccd_enable_wait_failures_retain_the_operation_before_disabling_notifications() {
    for canceled in [false, true] {
        let registry = Mutex::new(RetainedRegistry::default());
        let container = CleanupContainer::new("container");
        let state = RetainedResourceState::new();
        let enable = WriteState::new(AsyncStatus::Started);
        enable.cancel_finishes.set(false);
        let error = connect_owned(
            || Ok(()),
            || {
                Ok(ScopedRetainedConnection {
                    registry: &registry,
                    container: container.clone(),
                    owner: Some(MockRetainedConnection::new(state.clone(), [])),
                })
            },
            |connection| {
                state.notifications_enabled.set(true);
                connection
                    .owner
                    .as_mut()
                    .unwrap()
                    .operations
                    .writes
                    .dispatch_and_wait(
                        || Ok(MockWrite(enable.clone())),
                        |_| {
                            assert_eq!(enable.closes.get(), 0);
                            assert_eq!(enable.drops.get(), 0);
                            if canceled {
                                Err(anyhow::anyhow!("connection owner canceled"))
                            } else {
                                Err(ResponseTimeout.into())
                            }
                        },
                    )
            },
        )
        .err()
        .unwrap();
        assert!(!connection_retryable(&error));
        assert_eq!(enable.cancels.get(), 1);
        assert_eq!(enable.closes.get(), 0);
        assert_eq!(enable.drops.get(), 0);
        assert_eq!(state.disable_dispatches.get(), 0);
        assert!(state.notifications_enabled.get());
        assert!(state.callbacks_live.get());
        assert!(state.service_live.get());
        assert!(state.device_live.get());
        assert_eq!(state.owner_drops.get(), 0);
        enable.status.set(AsyncStatus::Canceled);
        assert!(reap_retained(&registry, container).is_err());
        assert_eq!(enable.closes.get(), 1);
        assert_eq!(enable.drops.get(), 1);
        assert_eq!(state.disable_dispatches.get(), 1);
        assert!(!state.notifications_enabled.get());
        assert_eq!(state.owner_drops.get(), 1);
        assert!(!state.dropped_live.get());
    }
}

#[test]
fn protocol30_completed_cccd_enable_is_closed_and_restored_when_subscription_setup_fails() {
    let state = RetainedResourceState::new();
    let enable = WriteState::new(AsyncStatus::Started);
    let error = connect_owned(
        || Ok(()),
        || Ok(MockRetainedConnection::new(state.clone(), [])),
        |connection| {
            state.notifications_enabled.set(true);
            connection.operations.writes.dispatch_and_wait(
                || Ok(MockWrite(enable.clone())),
                |_| {
                    enable.status.set(AsyncStatus::Completed);
                    Ok(())
                },
            )?;
            connection.operations.writes.reap()?;
            assert_eq!(enable.closes.get(), 1);
            assert_eq!(enable.cancels.get(), 0);
            assert_eq!(state.disable_dispatches.get(), 0);
            Err(anyhow::anyhow!("notification registration failed"))
        },
    )
    .err()
    .unwrap();
    assert!(connection_retryable(&error));
    assert_eq!(enable.closes.get(), 1);
    assert_eq!(enable.drops.get(), 1);
    assert_eq!(enable.cancels.get(), 0);
    assert_eq!(state.disable_dispatches.get(), 1);
    assert_eq!(state.owner_drops.get(), 1);
    assert!(!state.dropped_live.get());
}

#[test]
fn protocol30_retained_cleanup_keeps_operations_and_dependencies_after_scope_exit() {
    let registry = Mutex::new(RetainedRegistry::default());
    let container = CleanupContainer::new("{01234567-89AB-CDEF-0123-456789ABCDEF}");
    let state = RetainedResourceState::new();
    let write = WriteState::new(AsyncStatus::Started);
    write.cancel_finishes.set(false);
    {
        let _connection = ScopedRetainedConnection {
            registry: &registry,
            container: container.clone(),
            owner: Some(MockRetainedConnection::new(
                state.clone(),
                [MockWrite(write.clone())],
            )),
        };
    }
    assert_eq!(write.cancels.get(), 1);
    assert_eq!(write.closes.get(), 0);
    assert_eq!(write.drops.get(), 0);
    assert_eq!(state.owner_drops.get(), 0);
    assert!(state.callbacks_live.get());
    assert!(state.service_live.get());
    assert!(state.device_live.get());
    let mut retained = RetainedCleanup::begin(
        &registry,
        CleanupContainer::new("{01234567-89ab-cdef-0123-456789abcdef}"),
    )
    .unwrap()
    .unwrap();
    retained.finish();
    assert!(retained.result().is_err());
    drop(retained);
    assert_eq!(write.drops.get(), 0);
    assert_eq!(state.owner_drops.get(), 0);
    write.status.set(AsyncStatus::Canceled);
    let mut retained = RetainedCleanup::begin(&registry, container.clone())
        .unwrap()
        .unwrap();
    retained.finish();
    // Resource release does not erase the earlier cleanup timeout.
    assert!(retained.result().is_err());
    drop(retained);
    assert_eq!(write.cancels.get(), 1);
    assert_eq!(write.closes.get(), 1);
    assert_eq!(write.drops.get(), 1);
    assert_eq!(state.owner_drops.get(), 1);
    assert!(!state.dropped_live.get());
    assert!(
        RetainedCleanup::begin(&registry, container)
            .unwrap()
            .is_none()
    );
    let retained = registry.lock().unwrap();
    assert!(retained.containers.is_empty());
    assert!(retained.history.result().is_err());
    assert!(retained.history.result().is_err());
}

#[test]
fn protocol30_retained_cleanup_blocks_reentry_and_restores_an_unfinished_batch() {
    let registry = Mutex::new(RetainedRegistry::default());
    let container = CleanupContainer::new("container");
    let state = RetainedResourceState::new();
    retain_owner(
        &registry,
        container.clone(),
        Some(MockRetainedConnection::new(state.clone(), [])),
        [],
    );
    {
        let _retained = RetainedCleanup::begin(&registry, container.clone())
            .unwrap()
            .unwrap();
        assert!(RetainedCleanup::begin(&registry, container.clone()).is_err());
        assert_eq!(state.owner_drops.get(), 0);
    }
    assert_eq!(state.owner_drops.get(), 0);
    let mut retained = RetainedCleanup::begin(&registry, container)
        .unwrap()
        .unwrap();
    retained.finish();
    retained.result().unwrap();
    drop(retained);
    assert_eq!(state.owner_drops.get(), 1);
    assert!(!state.dropped_live.get());
    assert!(registry.lock().unwrap().containers.is_empty());
}

#[test]
fn protocol30_retained_cleanup_restores_owners_during_unwind() {
    let registry = Mutex::new(RetainedRegistry::default());
    let container = CleanupContainer::new("container");
    let state = RetainedResourceState::new();
    retain_owner(
        &registry,
        container.clone(),
        Some(MockRetainedConnection::new(state.clone(), [])),
        [],
    );
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _retained = RetainedCleanup::begin(&registry, container.clone())
            .unwrap()
            .unwrap();
        panic!("simulated caller unwind");
    }));
    assert!(result.is_err());
    assert_eq!(state.owner_drops.get(), 0);
    let mut retained = RetainedCleanup::begin(&registry, container)
        .unwrap()
        .unwrap();
    retained.finish();
    retained.result().unwrap();
    drop(retained);
    assert_eq!(state.owner_drops.get(), 1);
    assert!(!state.dropped_live.get());
}

#[test]
fn protocol30_retained_cleanup_recovers_a_poisoned_registry_without_losing_owners_or_history() {
    let registry = Mutex::new(RetainedRegistry::default());
    let container = CleanupContainer::new("container");
    let state = RetainedResourceState::new();
    retain_owner(
        &registry,
        container.clone(),
        Some(MockRetainedConnection::new(state.clone(), [])),
        ["initial cleanup error".into()],
    );
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _retained = registry.lock().unwrap();
        panic!("simulated registry poison");
    }));
    assert!(result.is_err());
    assert!(registry.is_poisoned());
    let other = RetainedResourceState::new();
    retain_owner(
        &registry,
        container.clone(),
        Some(MockRetainedConnection::new(other.clone(), [])),
        [],
    );
    let mut retained = RetainedCleanup::begin(&registry, container)
        .unwrap()
        .unwrap();
    retained.finish();
    retained.result().unwrap();
    drop(retained);
    assert_eq!(state.owner_drops.get(), 1);
    assert_eq!(other.owner_drops.get(), 1);
    assert!(!state.dropped_live.get());
    assert!(!other.dropped_live.get());
    let retained = registry
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    assert!(retained.containers.is_empty());
    assert_eq!(
        retained.history.result().unwrap_err().to_string(),
        "container: initial cleanup error"
    );
}

#[test]
fn protocol30_retained_cleanup_continues_after_a_panicking_owner_and_preserves_the_error() {
    let registry = Mutex::new(RetainedRegistry::default());
    let container = CleanupContainer::new("container");
    let failing = RetainedResourceState::new();
    failing.panic_on_finish.set(true);
    let other = RetainedResourceState::new();
    for state in [&failing, &other] {
        retain_owner(
            &registry,
            container.clone(),
            Some(MockRetainedConnection::new(state.clone(), [])),
            [],
        );
    }
    let mut retained = RetainedCleanup::begin(&registry, container.clone())
        .unwrap()
        .unwrap();
    retained.finish();
    assert!(
        retained
            .result()
            .unwrap_err()
            .to_string()
            .contains("panicked")
    );
    drop(retained);
    assert_eq!(failing.owner_drops.get(), 0);
    assert_eq!(other.owner_drops.get(), 1);
    failing.panic_on_finish.set(false);
    let mut retained = RetainedCleanup::begin(&registry, container)
        .unwrap()
        .unwrap();
    retained.finish();
    retained.result().unwrap();
    drop(retained);
    assert_eq!(failing.owner_drops.get(), 1);
    assert!(!failing.dropped_live.get());
    assert!(!other.dropped_live.get());
    let retained = registry.lock().unwrap();
    assert!(retained.containers.is_empty());
    assert!(retained.history.result().is_err());
}

#[test]
fn protocol30_retained_cleanup_does_not_hold_the_registry_lock_during_finish_or_drop() {
    let registry = Rc::new(Mutex::new(RetainedRegistry::default()));
    let container = CleanupContainer::new("container");
    let state = RetainedResourceState::new();
    let mut owner = MockRetainedConnection::new(state.clone(), []);
    let finish_registry = registry.clone();
    owner.on_finish = Some(Box::new(move || {
        assert!(finish_registry.try_lock().is_ok());
    }));
    let drop_registry = registry.clone();
    owner.on_drop = Some(Box::new(move || {
        assert!(drop_registry.try_lock().is_ok());
    }));
    retain_owner(&registry, container.clone(), Some(owner), []);
    let mut retained = RetainedCleanup::begin(&registry, container)
        .unwrap()
        .unwrap();
    retained.finish();
    retained.result().unwrap();
    drop(retained);
    assert_eq!(state.owner_drops.get(), 1);
    assert!(!state.dropped_live.get());
    assert_eq!(Rc::strong_count(&registry), 1);
}

#[test]
fn protocol30_retained_cleanup_preserves_an_owner_handed_off_while_reaping() {
    let registry = Mutex::new(RetainedRegistry::default());
    let container = CleanupContainer::new("container");
    let first = RetainedResourceState::new();
    retain_owner(
        &registry,
        container.clone(),
        Some(MockRetainedConnection::new(first.clone(), [])),
        [],
    );
    let mut retained = RetainedCleanup::begin(&registry, container.clone())
        .unwrap()
        .unwrap();
    let next = RetainedResourceState::new();
    retain_owner(
        &registry,
        container.clone(),
        Some(MockRetainedConnection::new(next.clone(), [])),
        [],
    );
    retained.finish();
    assert!(retained.result().is_err());
    drop(retained);
    assert_eq!(first.owner_drops.get(), 1);
    assert_eq!(next.owner_drops.get(), 0);
    let mut retained = RetainedCleanup::begin(&registry, container)
        .unwrap()
        .unwrap();
    retained.finish();
    retained.result().unwrap();
    drop(retained);
    assert_eq!(next.owner_drops.get(), 1);
    assert!(!first.dropped_live.get());
    assert!(!next.dropped_live.get());
    assert!(registry.lock().unwrap().containers.is_empty());
}

fn responses(packets: impl IntoIterator<Item = Vec<u8>>) -> Receiver<Result<Vec<u8>, String>> {
    let (sender, receiver) = mpsc::channel();
    for packet in packets {
        sender.send(Ok(packet)).unwrap();
    }
    receiver
}

#[test]
fn protocol30_command_dispatches_all_fragments_before_consuming_an_early_response() {
    let frame = [
        6, 21, 0, 0, 11, 4, 1, 0, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140,
        141, 142, 143, 144, 145, 146, 147, 148,
    ];
    let (sender, receiver) = mpsc::channel();
    sender.send(Ok(vec![6, 0, 0, 0, 11, 4, 1, 3])).unwrap();
    let sent = RefCell::new(Vec::new());
    let started = Instant::now();
    let mut pending = PendingOperations::default();
    let final_write = WriteState::new(AsyncStatus::Started);
    let data = exchange_command(
        &frame,
        &receiver,
        &|| Ok(()),
        &mut pending,
        |fragment| {
            sent.borrow_mut().push(fragment.to_vec());
            if sent.borrow().len() == 1 {
                sender.send(Ok(vec![6, 0, 0, 0, 11, 4, 1, 2]))?;
            }
            Ok(if sent.borrow().len() == 3 {
                MockWrite(final_write.clone())
            } else {
                completed_write()
            })
        },
        || {
            assert_eq!(sent.borrow().len(), 3);
            started
        },
    )
    .unwrap();
    assert!(data.is_empty());
    assert_eq!(pending.operations.len(), 1);
    assert_eq!(final_write.cancels.get(), 0);
    assert_eq!(final_write.closes.get(), 0);
    pending.cleanup(|| started, |_| unreachable!()).unwrap();
    assert_eq!(final_write.cancels.get(), 1);
    assert_eq!(final_write.closes.get(), 1);
    assert_eq!(
        sent.into_inner(),
        [
            vec![6, 21, 0, 0, 11, 4, 1, 0],
            vec![
                128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140, 141, 142, 143,
                144, 145, 146, 147,
            ],
            vec![148],
        ]
    );
}

#[test]
fn protocol30_command_write_failure_does_not_start_response_waiting() {
    let frame = [
        6, 21, 0, 0, 11, 4, 1, 0, 128, 129, 130, 131, 132, 133, 134, 135, 136, 137, 138, 139, 140,
        141, 142, 143, 144, 145, 146, 147, 148,
    ];
    for failure_at in 0..3 {
        let receiver = responses([]);
        let mut calls = 0;
        let mut states = Vec::new();
        let mut pending = PendingOperations::default();
        let error = exchange_command(
            &frame,
            &receiver,
            &|| Ok(()),
            &mut pending,
            |_| {
                let position = calls;
                calls += 1;
                ensure!(position != failure_at, "simulated GATT write failure");
                let state = WriteState::new(AsyncStatus::Started);
                states.push(state.clone());
                Ok(MockWrite(state))
            },
            || panic!("response deadline started after a failed write"),
        )
        .unwrap_err();
        assert_eq!(error.to_string(), "simulated GATT write failure");
        assert_eq!(calls, failure_at + 1);
        assert!(pending.operations.is_empty());
        for state in states {
            assert_eq!(state.cancels.get(), 1);
            assert_eq!(state.closes.get(), 1);
        }
    }
}

#[test]
fn protocol30_command_cancellation_stops_between_header_and_data() {
    let receiver = responses([]);
    let writes = Cell::new(0);
    let error = exchange_command(
        &[7, 2, 0, 0, 5, 4, 0, 0, 44, 1],
        &receiver,
        &|| {
            ensure!(writes.get() == 0, "connection owner replaced");
            Ok(())
        },
        &mut PendingOperations::default(),
        |_| {
            writes.set(writes.get() + 1);
            Ok(completed_write())
        },
        || panic!("response deadline started after cancellation"),
    )
    .unwrap_err();
    assert_eq!(error.to_string(), "connection owner replaced");
    assert_eq!(writes.get(), 1);
}

#[test]
fn protocol30_response_keeps_one_deadline_across_unrelated_and_incomplete_packets() {
    for first in [
        vec![2, 0, 0, 0, 1, 145, 0, 2],
        vec![3, 1, 0, 0, 1, 145, 0, 2],
    ] {
        let receiver = responses([first, vec![3, 0, 0, 0, 1, 145, 0, 2]]);
        let started = Instant::now();
        let mut ticks = [490, 501].into_iter();
        let error = receive_response(
            3,
            &receiver,
            &|| Ok(()),
            started + RESPONSE_TIMEOUT,
            || Ok(()),
            || started + Duration::from_millis(ticks.next().expect("unexpected clock query")),
        )
        .unwrap_err();
        assert!(error.downcast_ref::<ResponseTimeout>().is_some());
        assert!(retryable(&error));
        assert_eq!(
            receiver.try_recv().unwrap().unwrap(),
            vec![3, 0, 0, 0, 1, 145, 0, 2]
        );
        assert!(ticks.next().is_none());
    }
}

#[test]
fn protocol30_response_accepts_only_the_matching_complete_payload_before_deadline() {
    let receiver = responses([
        vec![2, 0, 0, 0, 1, 145, 0, 2],
        vec![3, 1, 0, 0, 1, 145, 0, 2],
        vec![103],
    ]);
    let started = Instant::now();
    let mut ticks = [10, 50, 450].into_iter();
    assert_eq!(
        receive_response(
            3,
            &receiver,
            &|| Ok(()),
            started + RESPONSE_TIMEOUT,
            || Ok(()),
            || { started + Duration::from_millis(ticks.next().expect("unexpected clock query")) }
        )
        .unwrap(),
        vec![103]
    );
    assert!(ticks.next().is_none());
}

#[test]
fn protocol30_response_cancellation_stops_before_consuming_another_fragment() {
    let receiver = responses([vec![3, 1, 0, 0, 1, 145, 0, 2], vec![103]]);
    let checks = Cell::new(0);
    let started = Instant::now();
    let error = receive_response(
        3,
        &receiver,
        &|| {
            checks.set(checks.get() + 1);
            ensure!(checks.get() == 1, "connection owner replaced");
            Ok(())
        },
        started + RESPONSE_TIMEOUT,
        || Ok(()),
        || started,
    )
    .unwrap_err();
    assert_eq!(error.to_string(), "connection owner replaced");
    assert!(!retryable(&error));
    assert_eq!(receiver.try_recv().unwrap().unwrap(), vec![103]);
}

#[test]
fn protocol30_response_matching_status_failure_does_not_wait_for_payload() {
    for (status, expected_retry) in [(1, true), (3, true), (4, false), (255, false)] {
        let receiver = responses([vec![3, 1, 0, 0, 1, 145, 0, status], vec![103]]);
        let started = Instant::now();
        let error = receive_response(
            3,
            &receiver,
            &|| Ok(()),
            started + RESPONSE_TIMEOUT,
            || Ok(()),
            || started,
        )
        .unwrap_err();
        assert_eq!(error.downcast_ref::<BleCommandStatus>().unwrap().0, status);
        assert_eq!(retryable(&error), expected_retry);
        assert_eq!(receiver.try_recv().unwrap().unwrap(), vec![103]);
    }
}

#[test]
fn protocol30_response_preserves_notification_and_subscription_failures() {
    let (sender, receiver) = mpsc::channel();
    sender
        .send(Err("notification decode failed".into()))
        .unwrap();
    drop(sender);
    let started = Instant::now();
    let error = receive_response(
        3,
        &receiver,
        &|| Ok(()),
        started + RESPONSE_TIMEOUT,
        || Ok(()),
        || started,
    )
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "BLE response notification failed: notification decode failed"
    );
    assert!(!retryable(&error));
    let error = receive_response(
        3,
        &receiver,
        &|| Ok(()),
        started + RESPONSE_TIMEOUT,
        || Ok(()),
        || started,
    )
    .unwrap_err();
    assert_eq!(error.to_string(), "BLE response subscription was released");
    assert!(!retryable(&error));
}

#[test]
fn protocol30_deadline_starts_after_the_final_dispatch_without_waiting_for_gatt() {
    for timely in [true, false] {
        let (sender, receiver) = mpsc::channel();
        let started = Instant::now();
        let dispatched = Cell::new(0);
        let mut pending = PendingOperations::default();
        let state = WriteState::new(AsyncStatus::Started);
        let mut ticks = [600, if timely { 1099 } else { 1100 }].into_iter();
        let result = exchange_command(
            &[7, 1, 0, 0, 5, 4, 0, 0, 77],
            &receiver,
            &|| Ok(()),
            &mut pending,
            |_| {
                dispatched.set(dispatched.get() + 1);
                if timely && dispatched.get() == 2 {
                    sender.send(Ok(vec![7, 0, 0, 0, 5, 4, 0, 2]))?;
                }
                Ok(MockWrite(state.clone()))
            },
            || {
                assert_eq!(dispatched.get(), 2);
                started + Duration::from_millis(ticks.next().unwrap())
            },
        );
        if timely {
            assert!(result.unwrap().is_empty());
            assert_eq!(state.cancels.get(), 0);
            assert_eq!(state.closes.get(), 0);
            assert_eq!(pending.operations.len(), 2);
            pending.cleanup(|| started, |_| unreachable!()).unwrap();
        } else {
            assert!(
                result
                    .unwrap_err()
                    .downcast_ref::<ResponseTimeout>()
                    .is_some()
            );
            assert!(pending.operations.is_empty());
        }
        assert!(ticks.next().is_none());
    }
}

#[test]
fn protocol30_write_completion_status_does_not_replace_a_confirmed_protocol_response() {
    // Current native OnWrite publishes its event on completion and on error;
    // protocol30.js consumes the matching read response, not that write event.
    for status in [
        AsyncStatus::Completed,
        AsyncStatus::Error,
        AsyncStatus::Canceled,
    ] {
        let (sender, receiver) = mpsc::channel();
        let state = WriteState::new(status);
        let mut pending = PendingOperations::default();
        let started = Instant::now();
        let result = exchange_command(
            &[7, 0, 0, 0, 5, 4, 0, 0],
            &receiver,
            &|| Ok(()),
            &mut pending,
            |_| {
                sender.send(Ok(vec![7, 0, 0, 0, 5, 4, 0, 2]))?;
                Ok(MockWrite(state.clone()))
            },
            || started,
        )
        .unwrap();
        assert!(result.is_empty());
        assert_eq!(state.cancels.get(), 0);
        assert_eq!(state.closes.get(), 1);
        assert!(pending.operations.is_empty());
    }
}

#[test]
fn protocol30_command_failure_paths_cancel_and_close_every_dispatched_operation() {
    for failure in ["canceled", "notification", "timeout"] {
        let (sender, receiver) = mpsc::channel();
        let checks = Cell::new(0);
        let mut pending = PendingOperations::default();
        let states = [
            WriteState::new(AsyncStatus::Started),
            WriteState::new(AsyncStatus::Started),
        ];
        let mut next = 0;
        let started = Instant::now();
        let mut clocks = [started, started + RESPONSE_TIMEOUT].into_iter();
        let error = exchange_command(
            &[7, 1, 0, 0, 5, 4, 0, 0, 77],
            &receiver,
            &|| {
                checks.set(checks.get() + 1);
                ensure!(failure != "canceled" || checks.get() < 4, "owner canceled");
                Ok(())
            },
            &mut pending,
            |_| {
                let state = states[next].clone();
                next += 1;
                if failure == "notification" && next == 2 {
                    sender.send(Err("decode failed".into()))?;
                }
                Ok(MockWrite(state))
            },
            || {
                if failure == "notification" {
                    started
                } else {
                    clocks.next().unwrap()
                }
            },
        )
        .unwrap_err();
        assert_eq!(next, 2);
        assert!(pending.operations.is_empty());
        for state in states {
            assert_eq!(state.cancels.get(), 1);
            assert_eq!(state.closes.get(), 1);
        }
        match failure {
            "canceled" => assert_eq!(error.to_string(), "owner canceled"),
            "notification" => assert!(error.to_string().contains("decode failed")),
            _ => assert!(error.downcast_ref::<ResponseTimeout>().is_some()),
        }
    }
}

#[test]
fn protocol30_cleanup_waits_for_terminal_status_after_a_successful_cancel_request() {
    let state = WriteState::new(AsyncStatus::Started);
    state.cancel_finishes.set(false);
    let mut pending = PendingOperations::default();
    pending.push(MockWrite(state.clone()));
    let started = Instant::now();
    let elapsed = Cell::new(Duration::ZERO);
    pending
        .cleanup(
            || started + elapsed.get(),
            |duration| {
                assert_eq!(state.closes.get(), 0);
                elapsed.set(elapsed.get() + duration);
                if elapsed.get() >= Duration::from_millis(20) {
                    state.status.set(AsyncStatus::Canceled);
                }
            },
        )
        .unwrap();
    assert_eq!(elapsed.get(), Duration::from_millis(20));
    assert_eq!(state.cancels.get(), 1);
    assert_eq!(state.closes.get(), 1);
    assert!(pending.operations.is_empty());
}

#[test]
fn protocol30_cleanup_timeout_retains_the_operation_for_its_connection_owner() {
    let state = WriteState::new(AsyncStatus::Started);
    state.cancel_finishes.set(false);
    let mut pending = PendingOperations::default();
    pending.push(MockWrite(state.clone()));
    let started = Instant::now();
    let elapsed = Cell::new(Duration::ZERO);
    let error = pending
        .cleanup(
            || started + elapsed.get(),
            |duration| elapsed.set(elapsed.get() + duration),
        )
        .unwrap_err();
    assert!(error.to_string().contains("cleanup timeout"));
    assert_eq!(state.cancels.get(), 1);
    assert_eq!(state.closes.get(), 0);
    assert_eq!(pending.operations.len(), 1);
    let mut cleanup = CleanupState::default();
    cleanup.record([error.to_string()]);
    state.status.set(AsyncStatus::Canceled);
    pending.cleanup(|| started, |_| unreachable!()).unwrap();
    assert!(pending.operations.is_empty());
    assert_eq!(state.cancels.get(), 1);
    assert_eq!(state.closes.get(), 1);
    assert!(cleanup.result().is_err());
    assert!(cleanup.result().is_err());
}

#[test]
fn protocol30_cleanup_failures_do_not_skip_other_operations_or_become_retryable() {
    for cancel_fails in [true, false] {
        let state = WriteState::new(if cancel_fails {
            AsyncStatus::Started
        } else {
            AsyncStatus::Completed
        });
        state.cancel_fails.set(cancel_fails);
        state.close_fails.set(!cancel_fails);
        let other = WriteState::new(AsyncStatus::Completed);
        let mut pending = PendingOperations::default();
        pending.push(MockWrite(state.clone()));
        pending.push(MockWrite(other.clone()));
        let started = Instant::now();
        let elapsed = Cell::new(Duration::ZERO);
        let error = pending
            .cleanup(
                || started + elapsed.get(),
                |duration| elapsed.set(elapsed.get() + duration),
            )
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains(if cancel_fails { "Cancel" } else { "Close" })
        );
        assert_eq!(pending.operations.len(), 1);
        assert_eq!(other.closes.get(), 1);
        let error = anyhow::Error::new(ResponseTimeout).context(CleanupFailure(error.to_string()));
        assert!(!retryable(&error));
        state.cancel_fails.set(false);
        state.close_fails.set(false);
        state.status.set(AsyncStatus::Completed);
        pending.cleanup(|| started, |_| unreachable!()).unwrap();
        assert!(pending.operations.is_empty());
    }
}
