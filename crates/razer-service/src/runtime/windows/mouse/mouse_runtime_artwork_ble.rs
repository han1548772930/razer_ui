//! Current packaged noble WinRT source: BLEManager::Connect/DiscoverServices/
//! DiscoverCharacteristics/Write/Notify/OnValueChanged and PeripheralWinrt.
//! Command responses use GATT; hardware events remain installed HID interrupts.
use super::{artwork, hid, receiver};
use anyhow::{Context as _, ensure};
use razer_device::mouse_runtime_artwork::{
    BleCommandStatus, BleResponse, Query, Transport, Update, WindowsBleTarget, write_ble_command,
};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    time::{Duration, Instant},
};
use windows::{
    Devices::Bluetooth::{
        BluetoothCacheMode, BluetoothConnectionStatus, BluetoothLEDevice,
        GenericAttributeProfile::{
            GattCharacteristic, GattCharacteristicProperties, GattCharacteristicsResult,
            GattClientCharacteristicConfigurationDescriptorValue as Descriptor,
            GattCommunicationStatus, GattDeviceService, GattDeviceServicesResult,
            GattValueChangedEventArgs, GattWriteOption, GattWriteResult,
        },
    },
    Foundation::TypedEventHandler,
    Storage::Streams::{DataReader, DataWriter},
    Win32::System::WinRT::{RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize},
    core::{GUID, IInspectable, RuntimeType},
};
use windows_future::{AsyncStatus, IAsyncOperation};

const SERVICE: GUID = GUID::from_u128(0x52401523_f97c_7f90_0e7f_6c6f4e36db1c);
const WRITE: GUID = GUID::from_u128(0x52401524_f97c_7f90_0e7f_6c6f4e36db1c);
const NOTIFY: GUID = GUID::from_u128(0x52401525_f97c_7f90_0e7f_6c6f4e36db1c);
const CONNECT_TIMEOUT: Duration = Duration::from_millis(5000);
const RESPONSE_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Debug)]
struct ResponseTimeout;
impl std::fmt::Display for ResponseTimeout {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("BLE Protocol30 response timeout")
    }
}
impl std::error::Error for ResponseTimeout {}
#[derive(Debug)]
struct CleanupFailure(String);
impl std::fmt::Display for CleanupFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}
impl std::error::Error for CleanupFailure {}
pub(crate) fn retryable(error: &anyhow::Error) -> bool {
    error.downcast_ref::<CleanupFailure>().is_none()
        && (error.downcast_ref::<ResponseTimeout>().is_some()
            || error
                .downcast_ref::<BleCommandStatus>()
                .is_some_and(BleCommandStatus::retryable))
}
pub(crate) fn connection_retryable(error: &anyhow::Error) -> bool {
    error.downcast_ref::<CleanupFailure>().is_none()
}

trait NativeOperation {
    fn status(&self) -> anyhow::Result<AsyncStatus>;
    fn cancel(&self) -> anyhow::Result<()>;
    fn close(&self) -> anyhow::Result<()>;
}
impl<T: RuntimeType + 'static> NativeOperation for IAsyncOperation<T> {
    fn status(&self) -> anyhow::Result<AsyncStatus> {
        self.Status().map_err(Into::into)
    }
    fn cancel(&self) -> anyhow::Result<()> {
        self.Cancel().map_err(Into::into)
    }
    fn close(&self) -> anyhow::Result<()> {
        self.Close().map_err(Into::into)
    }
}
trait DeviceOperation: NativeOperation {
    type Device;
    fn results(&self) -> anyhow::Result<Self::Device>;
    fn error_code(&self) -> anyhow::Result<String>;
}
impl DeviceOperation for IAsyncOperation<BluetoothLEDevice> {
    type Device = BluetoothLEDevice;
    fn results(&self) -> anyhow::Result<Self::Device> {
        self.GetResults().map_err(Into::into)
    }
    fn error_code(&self) -> anyhow::Result<String> {
        self.ErrorCode()
            .map(|code| format!("{code:?}"))
            .map_err(Into::into)
    }
}

struct DeviceAcquisition<O: DeviceOperation> {
    operation: Option<PendingOperation<O>>,
    result: Option<O::Device>,
    result_observed: bool,
    cleanup: CleanupState,
}
impl<O: DeviceOperation> Default for DeviceAcquisition<O> {
    fn default() -> Self {
        Self {
            operation: None,
            result: None,
            result_observed: false,
            cleanup: CleanupState::default(),
        }
    }
}
impl<O: DeviceOperation> DeviceAcquisition<O> {
    fn start(&mut self, operation: O) -> anyhow::Result<()> {
        ensure!(self.is_empty(), "BLE device acquisition is already owned");
        self.operation = Some(PendingOperation {
            operation,
            cancel_requested: false,
        });
        self.result_observed = false;
        Ok(())
    }
    fn is_empty(&self) -> bool {
        self.operation.is_none() && self.result.is_none()
    }
    fn failure<T>(&mut self, phase: &str, error: anyhow::Error) -> anyhow::Result<T> {
        let failure = format!("BLE device acquisition {phase}: {error:#}");
        self.cleanup.record([failure.clone()]);
        Err(error.context(CleanupFailure(failure)))
    }
    fn status(&mut self) -> anyhow::Result<AsyncStatus> {
        match self
            .operation
            .as_ref()
            .context("BLE device acquisition was released")?
            .operation
            .status()
        {
            Ok(status) => Ok(status),
            Err(error) => self.failure("Status", error),
        }
    }
    fn read_result(&mut self) -> anyhow::Result<()> {
        if !self.result_observed {
            match self
                .operation
                .as_ref()
                .context("BLE device acquisition was released")?
                .operation
                .results()
            {
                Ok(device) => {
                    self.result = Some(device);
                    self.result_observed = true;
                }
                Err(error) => return self.failure("GetResults", error),
            }
        }
        Ok(())
    }
    fn reap(&mut self) -> anyhow::Result<()> {
        if self.operation.is_none() {
            return Ok(());
        }
        let status = self.status()?;
        if status == AsyncStatus::Started {
            return Ok(());
        }
        if status == AsyncStatus::Completed {
            self.read_result()?;
        } else if status != AsyncStatus::Error && status != AsyncStatus::Canceled {
            return self.failure("Status", anyhow::anyhow!("unknown status {}", status.0));
        }
        if let Err(error) = self.operation.as_ref().unwrap().operation.close() {
            return self.failure("Close", error);
        }
        self.operation = None;
        Ok(())
    }
    fn wait(
        &mut self,
        timeout: Duration,
        validate: &dyn Fn() -> anyhow::Result<()>,
        mut now: impl FnMut() -> Instant,
        mut pause: impl FnMut(Duration),
    ) -> anyhow::Result<()> {
        let deadline = now() + timeout;
        loop {
            validate()?;
            let status = self.status()?;
            if status == AsyncStatus::Completed {
                self.read_result()?;
                validate()?;
                self.reap()?;
                return self.cleanup.result();
            }
            if status == AsyncStatus::Error || status == AsyncStatus::Canceled {
                let code = match self.operation.as_ref().unwrap().operation.error_code() {
                    Ok(code) => code,
                    Err(error) => return self.failure("ErrorCode", error),
                };
                anyhow::bail!(
                    "BLE device acquisition ended with status {} / HRESULT {code}",
                    status.0
                );
            }
            if status != AsyncStatus::Started {
                return self.failure("Status", anyhow::anyhow!("unknown status {}", status.0));
            }
            let Some(remaining) = deadline
                .checked_duration_since(now())
                .filter(|remaining| !remaining.is_zero())
            else {
                return Err(ResponseTimeout.into());
            };
            pause(remaining.min(Duration::from_millis(5)));
        }
    }
    fn take_result(&mut self) -> anyhow::Result<O::Device> {
        self.cleanup.result()?;
        ensure!(
            self.operation.is_none(),
            "BLE device acquisition is not closed"
        );
        self.result
            .take()
            .context("BLE device acquisition has no observed result")
    }
    fn cleanup(
        &mut self,
        mut now: impl FnMut() -> Instant,
        mut pause: impl FnMut(Duration),
    ) -> anyhow::Result<()> {
        let deadline = now() + RESPONSE_TIMEOUT;
        if self.operation.is_some() {
            let status = self.status()?;
            let pending = self.operation.as_mut().unwrap();
            if status == AsyncStatus::Started && !pending.cancel_requested {
                if let Err(error) = pending.operation.cancel() {
                    return self.failure("Cancel", error);
                }
                pending.cancel_requested = true;
            }
        }
        while self.operation.is_some() {
            self.reap()?;
            if self.operation.is_none() {
                break;
            }
            let Some(remaining) = deadline
                .checked_duration_since(now())
                .filter(|remaining| !remaining.is_zero())
            else {
                return self.failure(
                    "cleanup timeout",
                    anyhow::anyhow!("operation has not reached a terminal status"),
                );
            };
            pause(remaining.min(Duration::from_millis(5)));
        }
        // Current native Connect retains an accepted device and releases its
        // local result. A late result is only released; it never initializes.
        self.result = None;
        self.cleanup.result()
    }
}

struct PendingOperation<W> {
    operation: W,
    cancel_requested: bool,
}
struct PendingOperations<W> {
    operations: Vec<PendingOperation<W>>,
}
impl<W> Default for PendingOperations<W> {
    fn default() -> Self {
        Self {
            operations: Vec::new(),
        }
    }
}
impl<W: NativeOperation> PendingOperations<W> {
    fn push(&mut self, operation: W) {
        self.operations.push(PendingOperation {
            operation,
            cancel_requested: false,
        });
    }
    fn dispatch_and_wait<T>(
        &mut self,
        dispatch: impl FnOnce() -> anyhow::Result<W>,
        wait: impl FnOnce(&W) -> anyhow::Result<T>,
    ) -> anyhow::Result<T> {
        self.push(dispatch()?);
        wait(&self.operations.last().unwrap().operation)
    }
    fn reap(&mut self) -> anyhow::Result<()> {
        let mut failures = Vec::new();
        self.operations.retain(|pending| {
            match pending.operation.status() {
                Ok(status)
                    if status == AsyncStatus::Completed
                        || status == AsyncStatus::Error
                        || status == AsyncStatus::Canceled =>
                {
                    match pending.operation.close() {
                        Ok(()) => return false,
                        Err(error) => failures.push(format!("BLE operation Close: {error:#}")),
                    }
                }
                Ok(status) if status == AsyncStatus::Started => {}
                Ok(status) => failures.push(format!("Unknown BLE operation status {}", status.0)),
                Err(error) => failures.push(format!("BLE operation Status: {error:#}")),
            }
            true
        });
        if failures.is_empty() {
            Ok(())
        } else {
            Err(CleanupFailure(failures.join("; ")).into())
        }
    }
    fn cleanup(
        &mut self,
        mut now: impl FnMut() -> Instant,
        mut pause: impl FnMut(Duration),
    ) -> anyhow::Result<()> {
        let deadline = now() + RESPONSE_TIMEOUT;
        let mut failures = Vec::new();
        for pending in &mut self.operations {
            match pending.operation.status() {
                Ok(status) if status == AsyncStatus::Started && !pending.cancel_requested => {
                    match pending.operation.cancel() {
                        Ok(()) => pending.cancel_requested = true,
                        Err(error) => failures.push(format!("BLE operation Cancel: {error:#}")),
                    }
                }
                Ok(_) => {}
                Err(error) => failures.push(format!("BLE operation Status: {error:#}")),
            }
        }
        loop {
            if let Err(error) = self.reap() {
                failures.push(format!("{error:#}"));
                break;
            }
            if self.operations.is_empty() {
                break;
            }
            let Some(remaining) = deadline
                .checked_duration_since(now())
                .filter(|remaining| !remaining.is_zero())
            else {
                failures.push(format!(
                    "{} BLE operations did not stop before cleanup timeout",
                    self.operations.len()
                ));
                break;
            };
            // Cancel only requests cancellation. Keep every operation and its
            // connection dependencies until the OS reports a terminal status.
            pause(remaining.min(Duration::from_millis(5)));
        }
        ensure!(failures.is_empty(), "{}", failures.join("; "));
        Ok(())
    }
}

struct ConnectionOperations<S, C, W> {
    services: PendingOperations<S>,
    characteristics: PendingOperations<C>,
    writes: PendingOperations<W>,
}
impl<S, C, W> Default for ConnectionOperations<S, C, W> {
    fn default() -> Self {
        Self {
            services: PendingOperations::default(),
            characteristics: PendingOperations::default(),
            writes: PendingOperations::default(),
        }
    }
}
impl<S: NativeOperation, C: NativeOperation, W: NativeOperation> ConnectionOperations<S, C, W> {
    fn is_empty(&self) -> bool {
        self.services.operations.is_empty()
            && self.characteristics.operations.is_empty()
            && self.writes.operations.is_empty()
    }
    fn cleanup(
        &mut self,
        mut now: impl FnMut() -> Instant,
        mut pause: impl FnMut(Duration),
    ) -> anyhow::Result<()> {
        let mut failures = Vec::new();
        for (phase, result) in [
            (
                "GATT service discovery",
                self.services.cleanup(&mut now, &mut pause),
            ),
            (
                "GATT characteristic discovery",
                self.characteristics.cleanup(&mut now, &mut pause),
            ),
            ("GATT write", self.writes.cleanup(&mut now, &mut pause)),
        ] {
            if let Err(error) = result {
                failures.push(format!("{phase}: {error:#}"));
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(CleanupFailure(failures.join("; ")).into())
        }
    }
}

#[derive(Default)]
struct CleanupState {
    failures: Vec<String>,
}
impl CleanupState {
    fn record_result(&mut self, result: anyhow::Result<()>) -> anyhow::Result<()> {
        if let Err(error) = result {
            self.record([format!("{error:#}")]);
        }
        self.result()
    }
    fn record(&mut self, failures: impl IntoIterator<Item = String>) {
        for failure in failures {
            if !self.failures.contains(&failure) {
                self.failures.push(failure);
            }
        }
    }
    fn result(&self) -> anyhow::Result<()> {
        if self.failures.is_empty() {
            Ok(())
        } else {
            Err(CleanupFailure(self.failures.join("; ")).into())
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct CleanupContainer(String);
impl CleanupContainer {
    fn new(container: &str) -> Self {
        Self(container.to_ascii_lowercase())
    }
}

trait RetainedOwner {
    fn finish(&mut self) -> anyhow::Result<()>;
    fn is_closed(&self) -> bool;
}
fn connect_owned<O: RetainedOwner>(
    cleanup_gate: impl FnOnce() -> anyhow::Result<()>,
    open: impl FnOnce() -> anyhow::Result<O>,
    initialize: impl FnOnce(&mut O) -> anyhow::Result<()>,
) -> anyhow::Result<O> {
    cleanup_gate().map_err(|error| {
        error.context(CleanupFailure("BLE connection cleanup gate failed".into()))
    })?;
    let mut owner = open()?;
    match initialize(&mut owner) {
        Ok(()) => Ok(owner),
        Err(error) => match owner.finish() {
            Ok(()) => Err(error),
            Err(cleanup) => Err(error.context(CleanupFailure(format!(
                "BLE connection initialization cleanup also failed: {cleanup:#}"
            )))),
        },
    }
}
struct RetainedSlot<O> {
    owners: Vec<O>,
    reaping: bool,
}
struct RetainedRegistry<O> {
    containers: HashMap<CleanupContainer, RetainedSlot<O>>,
    history: CleanupState,
}
impl<O> Default for RetainedRegistry<O> {
    fn default() -> Self {
        Self {
            containers: HashMap::new(),
            history: CleanupState::default(),
        }
    }
}
impl<O> RetainedRegistry<O> {
    fn retain(&mut self, container: CleanupContainer, owner: &mut Option<O>) {
        let slot = self
            .containers
            .entry(container)
            .or_insert_with(|| RetainedSlot {
                owners: Vec::new(),
                reaping: false,
            });
        // Keep the caller's owner intact if reserving the slot unwinds.
        slot.owners.reserve(1);
        slot.owners.push(owner.take().unwrap());
    }
    fn begin(&mut self, container: &CleanupContainer) -> anyhow::Result<Option<Vec<O>>> {
        let Some(slot) = self.containers.get_mut(container) else {
            return Ok(None);
        };
        ensure!(
            !slot.reaping,
            "BLE cleanup is already in progress for {}",
            container.0
        );
        if slot.owners.is_empty() {
            return Ok(None);
        }
        slot.reaping = true;
        Ok(Some(std::mem::take(&mut slot.owners)))
    }
    fn restore(
        &mut self,
        container: &CleanupContainer,
        owners: &mut Vec<O>,
        failures: Vec<String>,
    ) {
        let slot = self
            .containers
            .entry(container.clone())
            .or_insert_with(|| RetainedSlot {
                owners: Vec::new(),
                reaping: true,
            });
        slot.owners.append(owners);
        slot.reaping = false;
        if slot.owners.is_empty() {
            // This slot contains no owner whose Drop could reenter the registry.
            self.containers.remove(container);
        }
        self.history.record(
            failures
                .into_iter()
                .map(|failure| format!("{}: {failure}", container.0)),
        );
    }
}

fn retain_owner<O>(
    registry: &Mutex<RetainedRegistry<O>>,
    container: CleanupContainer,
    mut owner: Option<O>,
    failures: impl IntoIterator<Item = String>,
) {
    // owner precedes the lock guard so an unwind releases the lock before the
    // owner's Drop can reenter its registry.
    let mut retained = registry
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    retained.history.record(
        failures
            .into_iter()
            .map(|failure| format!("{}: {failure}", container.0)),
    );
    if owner.is_some() {
        retained.retain(container, &mut owner);
    }
}

struct RetainedCleanup<'a, O> {
    registry: &'a Mutex<RetainedRegistry<O>>,
    container: CleanupContainer,
    owners: Vec<O>,
    failures: Vec<String>,
}
impl<'a, O> RetainedCleanup<'a, O> {
    fn begin(
        registry: &'a Mutex<RetainedRegistry<O>>,
        container: CleanupContainer,
    ) -> anyhow::Result<Option<Self>> {
        let owners = registry
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .begin(&container)?;
        Ok(owners.map(|owners| Self {
            registry,
            container,
            owners,
            failures: Vec::new(),
        }))
    }
    fn result(&self) -> anyhow::Result<()> {
        let mut failures = self.failures.clone();
        let queued = self
            .registry
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .containers
            .get(&self.container)
            .map_or(0, |slot| slot.owners.len());
        let pending = self.owners.len() + queued;
        if pending != 0 {
            failures.push(format!(
                "{} retained BLE connections still own unreleased resources",
                pending
            ));
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(CleanupFailure(failures.join("; ")).into())
        }
    }
}
impl<O: RetainedOwner> RetainedCleanup<'_, O> {
    fn finish(&mut self) {
        self.owners.retain_mut(|owner| {
            match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| owner.finish())) {
                Ok(Ok(())) => {}
                Ok(Err(error)) => self.failures.push(format!("{error:#}")),
                Err(_) => self.failures.push("Retained BLE cleanup panicked".into()),
            }
            !owner.is_closed()
        });
    }
}
impl<O> Drop for RetainedCleanup<'_, O> {
    fn drop(&mut self) {
        // No registry lock spans native cleanup or dropping a closed owner.
        // The guard also restores unfinished owners on early return/unwind.
        let mut owners = std::mem::take(&mut self.owners);
        let failures = std::mem::take(&mut self.failures);
        self.registry
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .restore(&self.container, &mut owners, failures);
    }
}

fn retained_connections() -> &'static Mutex<RetainedRegistry<Connection>> {
    static RETAINED: OnceLock<Mutex<RetainedRegistry<Connection>>> = OnceLock::new();
    RETAINED.get_or_init(|| Mutex::new(RetainedRegistry::default()))
}

/// The caller holds this container's ReceiverLock on the current thread.
pub(in crate::runtime::native::device_reads) fn cleanup_container(
    container: &str,
) -> anyhow::Result<()> {
    let Some(mut retained) =
        RetainedCleanup::begin(retained_connections(), CleanupContainer::new(container))?
    else {
        return Ok(());
    };
    match Apartment::new() {
        Ok(_apartment) => retained.finish(),
        Err(error) => retained.failures.push(format!(
            "Initialize retained BLE cleanup apartment: {error:#}"
        )),
    }
    retained.result()
}

/// Called after portable workers stop. Acquiring each container lease invokes
/// cleanup_container before another transport can use that container.
pub(in crate::runtime::native::device_reads) fn shutdown_cleanup() -> anyhow::Result<()> {
    shutdown_retained(retained_connections(), |container| {
        receiver::ReceiverLock::acquire(&container.0, &|| Ok(())).map(drop)
    })
}

pub(in crate::runtime::native::device_reads) fn cleanup_pending_count() -> usize {
    retained_pending_count(retained_connections())
}

fn retained_pending_count<O>(registry: &Mutex<RetainedRegistry<O>>) -> usize {
    registry
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .containers
        .values()
        .map(|slot| slot.owners.len() + usize::from(slot.reaping))
        .sum()
}

fn shutdown_retained<O>(
    registry: &Mutex<RetainedRegistry<O>>,
    mut acquire: impl FnMut(&CleanupContainer) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let containers = {
        let retained = registry
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        retained.containers.keys().cloned().collect::<Vec<_>>()
    };
    let mut failures = Vec::new();
    for container in containers {
        match acquire(&container) {
            Ok(()) => {}
            Err(error) => failures.push(format!("{}: {error:#}", container.0)),
        }
    }
    {
        let mut retained = registry
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        retained.history.record(failures);
        let pending = retained
            .containers
            .values()
            .map(|slot| slot.owners.len() + usize::from(slot.reaping))
            .sum::<usize>();
        if pending != 0 {
            return Err(CleanupFailure(format!(
                "{pending} retained BLE cleanup owners remain; {}",
                retained.history.failures.join("; ")
            ))
            .into());
        }
        retained.history.result()
    }
}

fn exchange_command<W: NativeOperation>(
    frame: &[u8],
    responses: &Receiver<Result<Vec<u8>, String>>,
    validate: &dyn Fn() -> anyhow::Result<()>,
    writes: &mut PendingOperations<W>,
    mut dispatch: impl FnMut(&[u8]) -> anyhow::Result<W>,
    mut now: impl FnMut() -> Instant,
) -> anyhow::Result<Vec<u8>> {
    let result = (|| {
        validate()?;
        while responses.try_recv().is_ok() {}
        write_ble_command(frame, validate, |fragment| {
            writes.push(dispatch(fragment)?);
            Ok(())
        })?;
        // Current protocol30.js arms its one response timer after all native
        // write dispatches return. Native 0x44270 returns after setting the
        // WinRT Completed handler; GATT completion does not gate the response.
        let deadline = now() + RESPONSE_TIMEOUT;
        receive_response(
            frame[0],
            responses,
            validate,
            deadline,
            || writes.reap(),
            now,
        )
    })();
    match result {
        Ok(response) => Ok(response),
        Err(error) => match writes.cleanup(Instant::now, std::thread::sleep) {
            Ok(()) => Err(error),
            Err(cleanup) => Err(error.context(CleanupFailure(format!(
                "BLE command cleanup also failed: {cleanup:#}"
            )))),
        },
    }
}

fn receive_response(
    transaction: u8,
    responses: &Receiver<Result<Vec<u8>, String>>,
    validate: &dyn Fn() -> anyhow::Result<()>,
    deadline: Instant,
    mut poll_writes: impl FnMut() -> anyhow::Result<()>,
    mut now: impl FnMut() -> Instant,
) -> anyhow::Result<Vec<u8>> {
    let mut response = BleResponse::new(transaction);
    loop {
        validate()?;
        poll_writes()?;
        let remaining = deadline
            .checked_duration_since(now())
            .filter(|remaining| !remaining.is_zero())
            .ok_or(ResponseTimeout)?;
        match responses.recv_timeout(remaining.min(Duration::from_millis(10))) {
            Ok(Ok(packet)) => {
                if let Some(payload) = response.push(&packet)? {
                    return Ok(payload);
                }
            }
            Ok(Err(error)) => anyhow::bail!("BLE response notification failed: {error}"),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                anyhow::bail!("BLE response subscription was released")
            }
        }
    }
}

pub(crate) struct Apartment;
impl Apartment {
    pub(crate) fn new() -> anyhow::Result<Self> {
        unsafe { RoInitialize(RO_INIT_MULTITHREADED) }
            .context("Initialize BLE worker WinRT apartment")?;
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { RoUninitialize() };
    }
}

fn wait_for<T: RuntimeType + 'static>(
    operation: &IAsyncOperation<T>,
    deadline: Duration,
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<T> {
    let started = Instant::now();
    loop {
        validate()?;
        let status = operation.Status()?;
        if status == AsyncStatus::Completed {
            return operation.GetResults().map_err(Into::into);
        }
        if status == AsyncStatus::Error || status == AsyncStatus::Canceled {
            anyhow::bail!(
                "BLE WinRT operation ended with status {} / HRESULT {:?}",
                status.0,
                operation.ErrorCode()?
            );
        }
        if started.elapsed() >= deadline {
            return Err(ResponseTimeout.into());
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

pub(crate) fn current_target(target: &WindowsBleTarget) -> anyhow::Result<()> {
    ensure!(
        receiver::valid_container(&target.device_container_id),
        "BLE mouse ContainerId is invalid"
    );
    let observed = hid::enumerate_metadata()?;
    let current = WindowsBleTarget::from_hid(
        target.product_id,
        target.physical_product_id,
        &target.device_container_id,
        &observed,
    )?;
    ensure!(
        current == *target,
        "BLE mouse installed endpoint / parent / address changed"
    );
    Ok(())
}

pub(crate) struct Connection {
    container: CleanupContainer,
    acquisition: DeviceAcquisition<IAsyncOperation<BluetoothLEDevice>>,
    device: Option<BluetoothLEDevice>,
    service: Option<GattDeviceService>,
    write: Option<GattCharacteristic>,
    notify: Option<GattCharacteristic>,
    connection_token: Option<i64>,
    value_token: Option<i64>,
    notification_enabled: bool,
    closing: bool,
    closed: bool,
    device_closed: bool,
    operations: ConnectionOperations<
        IAsyncOperation<GattDeviceServicesResult>,
        IAsyncOperation<GattCharacteristicsResult>,
        IAsyncOperation<GattWriteResult>,
    >,
    cleanup: CleanupState,
    disconnected: Arc<AtomicBool>,
    overflow: Arc<AtomicBool>,
    responses: Receiver<Result<Vec<u8>, String>>,
    write_option: GattWriteOption,
}

impl Connection {
    pub(crate) fn connect_with_retry(
        target: &WindowsBleTarget,
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Self> {
        let mut last = None;
        // Current rzDevice30 w6=2 and ah=150, shared by its actual callers.
        for attempt in 0..2 {
            if attempt > 0 {
                interruptible_sleep(Duration::from_millis(150), validate)?;
            }
            match Self::connect(target, validate) {
                Ok(connection) => return Ok(connection),
                Err(error) => {
                    if !connection_retryable(&error) {
                        return Err(error);
                    }
                    validate()?;
                    last = Some(error);
                }
            }
        }
        Err(last.context("BLE connection did not run")?)
    }

    pub(crate) fn exchange_with_retry(
        &mut self,
        frame: &[u8],
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Vec<u8>> {
        let mut last = None;
        // Current rzDevice30 lI=2 and lz=30. Retries reuse the same frame/tx.
        for attempt in 0..2 {
            if attempt > 0 {
                interruptible_sleep(Duration::from_millis(30), validate)?;
            }
            match self.exchange(frame, validate) {
                Ok(data) => return Ok(data),
                Err(error) => {
                    validate()?;
                    if !retryable(&error) {
                        return Err(error);
                    }
                    last = Some(error);
                }
            }
        }
        Err(last.context("BLE command did not run")?)
    }

    pub(crate) fn connect(
        target: &WindowsBleTarget,
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Self> {
        validate()?;
        let (responses_tx, responses) = mpsc::sync_channel(64);
        connect_owned(
            || cleanup_container(&target.device_container_id),
            || {
                validate()?;
                let disconnected = Arc::new(AtomicBool::new(false));
                let overflow = Arc::new(AtomicBool::new(false));
                Ok(Self {
                    container: CleanupContainer::new(&target.device_container_id),
                    acquisition: DeviceAcquisition::default(),
                    device: None,
                    service: None,
                    write: None,
                    notify: None,
                    connection_token: None,
                    value_token: None,
                    notification_enabled: false,
                    closing: false,
                    closed: false,
                    device_closed: false,
                    operations: ConnectionOperations::default(),
                    cleanup: CleanupState::default(),
                    disconnected,
                    overflow,
                    responses,
                    write_option: GattWriteOption::WriteWithResponse,
                })
            },
            |connection| {
                connection
                    .acquisition
                    .start(BluetoothLEDevice::FromBluetoothAddressAsync(
                        target.mac_address,
                    )?)?;
                connection.acquisition.wait(
                    CONNECT_TIMEOUT,
                    validate,
                    Instant::now,
                    std::thread::sleep,
                )?;
                validate()?;
                connection.device = Some(connection.acquisition.take_result()?);
                let device = connection.device.as_ref().unwrap();
                ensure!(
                    device.BluetoothAddress()? == target.mac_address,
                    "WinRT connected a different Bluetooth address"
                );
                let signal = connection.disconnected.clone();
                let handler =
                    TypedEventHandler::<BluetoothLEDevice, IInspectable>::new(move |device, _| {
                        if let Some(device) = device.as_ref() {
                            if device.ConnectionStatus()? == BluetoothConnectionStatus::Disconnected
                            {
                                signal.store(true, Ordering::Release);
                            }
                        }
                        Ok(())
                    });
                connection.connection_token = Some(device.ConnectionStatusChanged(&handler)?);
                let guard = || {
                    validate()?;
                    Self::validate_signals(&connection.disconnected, &connection.overflow)
                };
                let services = connection.operations.services.dispatch_and_wait(
                    || {
                        device
                            .GetGattServicesWithCacheModeAsync(BluetoothCacheMode::Uncached)
                            .map_err(Into::into)
                    },
                    |operation| wait_for(operation, CONNECT_TIMEOUT, &guard),
                )?;
                ensure!(
                    services.Status()? == GattCommunicationStatus::Success,
                    "BLE GATT service discovery failed: {:?}",
                    services.Status()?
                );
                let mut matching = Vec::new();
                for service in services.Services()? {
                    if service.Uuid()? == SERVICE {
                        matching.push(service);
                    }
                }
                ensure!(
                    matching.len() == 1,
                    "BLE mouse Protocol30 service is absent or ambiguous"
                );
                connection.service = matching.pop();
                connection
                    .cleanup
                    .record_result(connection.operations.services.reap())?;
                let guard = || {
                    validate()?;
                    Self::validate_signals(&connection.disconnected, &connection.overflow)
                };
                let characteristics = connection.operations.characteristics.dispatch_and_wait(
                    || {
                        connection
                            .service
                            .as_ref()
                            .unwrap()
                            .GetCharacteristicsWithCacheModeAsync(BluetoothCacheMode::Uncached)
                            .map_err(Into::into)
                    },
                    |operation| wait_for(operation, CONNECT_TIMEOUT, &guard),
                )?;
                ensure!(
                    characteristics.Status()? == GattCommunicationStatus::Success,
                    "BLE GATT characteristic discovery failed: {:?}",
                    characteristics.Status()?
                );
                for characteristic in characteristics.Characteristics()? {
                    match characteristic.Uuid()? {
                        WRITE => {
                            ensure!(
                                connection.write.is_none(),
                                "Protocol30 write characteristic is ambiguous"
                            );
                            connection.write = Some(characteristic);
                        }
                        NOTIFY => {
                            ensure!(
                                connection.notify.is_none(),
                                "Protocol30 notify characteristic is ambiguous"
                            );
                            connection.notify = Some(characteristic);
                        }
                        _ => {}
                    }
                }
                connection
                    .cleanup
                    .record_result(connection.operations.characteristics.reap())?;
                let write = connection
                    .write
                    .as_ref()
                    .context("BLE mouse has no actual Protocol30 write characteristic")?;
                let properties = write.CharacteristicProperties()?;
                ensure!(
                    properties.contains(GattCharacteristicProperties::Write)
                        || properties.contains(GattCharacteristicProperties::WriteWithoutResponse),
                    "Protocol30 characteristic cannot write"
                );
                connection.write_option =
                    if properties.contains(GattCharacteristicProperties::WriteWithoutResponse) {
                        GattWriteOption::WriteWithoutResponse
                    } else {
                        GattWriteOption::WriteWithResponse
                    };
                let notify = connection
                    .notify
                    .as_ref()
                    .context("BLE mouse has no actual Protocol30 notify characteristic")?;
                let properties = notify.CharacteristicProperties()?;
                let descriptor = if properties.contains(GattCharacteristicProperties::Indicate) {
                    Descriptor::Indicate
                } else {
                    ensure!(
                        properties.contains(GattCharacteristicProperties::Notify),
                        "Protocol30 response characteristic cannot notify"
                    );
                    Descriptor::Notify
                };
                // Retain disable intent before dispatch, including a failed response
                // after the OS may have applied the CCCD value.
                connection.notification_enabled = true;
                let guard = || {
                    validate()?;
                    Self::validate_signals(&connection.disconnected, &connection.overflow)
                };
                let applied = connection.operations.writes.dispatch_and_wait(
                    || {
                        notify
                            .WriteClientCharacteristicConfigurationDescriptorWithResultAsync(
                                descriptor,
                            )
                            .map_err(Into::into)
                    },
                    |operation| wait_for(operation, CONNECT_TIMEOUT, &guard),
                )?;
                ensure!(
                    applied.Status()? == GattCommunicationStatus::Success,
                    "Enable BLE response notifications failed: {:?}",
                    applied.Status()?
                );
                connection
                    .cleanup
                    .record_result(connection.operations.writes.reap())?;
                let overflow = connection.overflow.clone();
                let handler =
                    TypedEventHandler::<GattCharacteristic, GattValueChangedEventArgs>::new(
                        move |_, event| {
                            let received = (|| -> windows::core::Result<Vec<u8>> {
                                let event = event.ok()?;
                                let reader = DataReader::FromBuffer(&event.CharacteristicValue()?)?;
                                let mut bytes = vec![0; reader.UnconsumedBufferLength()? as usize];
                                reader.ReadBytes(&mut bytes)?;
                                reader.Close()?;
                                Ok(bytes)
                            })()
                            .map_err(|error| error.to_string());
                            if responses_tx.try_send(received).is_err() {
                                overflow.store(true, Ordering::Release);
                            }
                            Ok(())
                        },
                    );
                connection.value_token = Some(notify.ValueChanged(&handler)?);
                validate()?;
                connection.validate()?;
                Ok(())
            },
        )
    }

    fn validate(&self) -> anyhow::Result<()> {
        Self::validate_signals(&self.disconnected, &self.overflow)
    }

    fn validate_signals(disconnected: &AtomicBool, overflow: &AtomicBool) -> anyhow::Result<()> {
        ensure!(
            !disconnected.load(Ordering::Acquire),
            "BLE mouse disconnected during observation"
        );
        ensure!(
            !overflow.load(Ordering::Acquire),
            "BLE response queue overflowed or lost its consumer"
        );
        Ok(())
    }

    pub(crate) fn exchange(
        &mut self,
        frame: &[u8],
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Vec<u8>> {
        self.cleanup.result()?;
        ensure!(!self.closing, "BLE connection is closing");
        let disconnected = &self.disconnected;
        let overflow = &self.overflow;
        let guard = || {
            validate()?;
            Self::validate_signals(disconnected, overflow)
        };
        let write = self
            .write
            .as_ref()
            .context("BLE write characteristic was released")?;
        let result = exchange_command(
            frame,
            &self.responses,
            &guard,
            &mut self.operations.writes,
            |fragment| {
                let writer = DataWriter::new()?;
                writer.WriteBytes(fragment)?;
                let buffer = writer.DetachBuffer()?;
                writer.Close()?;
                write
                    .WriteValueWithResultAndOptionAsync(&buffer, self.write_option)
                    .map_err(Into::into)
            },
            Instant::now,
        );
        if let Err(error) = &result {
            if error.downcast_ref::<CleanupFailure>().is_some() {
                self.cleanup.record([format!("{error:#}")]);
            }
        }
        result
    }

    pub(crate) fn finish(&mut self) -> anyhow::Result<()> {
        if self.closed {
            return self.cleanup.result();
        }
        self.closing = true;
        let mut failures = Vec::new();
        if let Err(error) = self.acquisition.cleanup(Instant::now, std::thread::sleep) {
            failures.push(format!("BLE device acquisition cleanup: {error:#}"));
        }
        if let Err(error) = self.operations.cleanup(Instant::now, std::thread::sleep) {
            failures.push(format!("BLE in-flight operation cleanup: {error:#}"));
        }
        if !self.acquisition.is_empty() || !self.operations.is_empty() {
            self.cleanup.record(failures);
            return self.cleanup.result();
        }
        if let (Some(notify), Some(token)) = (&self.notify, self.value_token) {
            if let Err(error) = notify.RemoveValueChanged(token) {
                failures.push(format!("BLE ValueChanged removal: {error}"));
            } else {
                self.value_token = None;
            }
        } else if self.value_token.is_some() {
            failures.push("BLE ValueChanged owner is absent".into());
        }
        if self.notification_enabled {
            if let Some(notify) = &self.notify {
                let cleanup = (|| -> anyhow::Result<()> {
                    self.operations.writes.push(
                        notify.WriteClientCharacteristicConfigurationDescriptorWithResultAsync(
                            Descriptor::None,
                        )?,
                    );
                    let applied = wait_for(
                        &self.operations.writes.operations.last().unwrap().operation,
                        RESPONSE_TIMEOUT,
                        &|| Ok(()),
                    )?;
                    ensure!(
                        applied.Status()? == GattCommunicationStatus::Success,
                        "Disable BLE notifications failed: {:?}",
                        applied.Status()?
                    );
                    Ok(())
                })();
                if let Err(error) = cleanup {
                    failures.push(format!("BLE CCCD cleanup: {error:#}"));
                } else {
                    self.notification_enabled = false;
                }
                if let Err(error) = self
                    .operations
                    .writes
                    .cleanup(Instant::now, std::thread::sleep)
                {
                    failures.push(format!("BLE CCCD operation cleanup: {error:#}"));
                }
            } else {
                failures.push("BLE CCCD cleanup owner is absent".into());
            }
        }
        // Failed callback removal or an in-flight CCCD write retains its
        // characteristic, service and device for another explicit finish.
        if self.value_token.is_some() || self.notification_enabled || !self.operations.is_empty() {
            self.cleanup.record(failures);
            return self.cleanup.result();
        }
        if let Some(token) = self.connection_token {
            if let Some(device) = &self.device {
                if let Err(error) = device.RemoveConnectionStatusChanged(token) {
                    failures.push(format!("BLE connection callback removal: {error}"));
                } else {
                    self.connection_token = None;
                }
            } else {
                failures.push("BLE connection callback owner is absent".into());
            }
        }
        if self.connection_token.is_some() {
            self.cleanup.record(failures);
            return self.cleanup.result();
        }
        self.write = None;
        self.notify = None;
        if let Some(service) = &self.service {
            if let Err(error) = service.Close() {
                failures.push(format!("BLE service close: {error}"));
            } else {
                self.service = None;
            }
        }
        if self.service.is_none() && !self.device_closed {
            if let Some(device) = &self.device {
                if let Err(error) = device.Close() {
                    failures.push(format!("BLE device close: {error}"));
                } else {
                    self.device_closed = true;
                }
            } else {
                self.device_closed = true;
            }
        }
        self.closed = self.service.is_none() && self.device_closed;
        self.cleanup.record(failures);
        self.cleanup.result()
    }
}
impl RetainedOwner for Connection {
    fn finish(&mut self) -> anyhow::Result<()> {
        Connection::finish(self)
    }
    fn is_closed(&self) -> bool {
        self.closed
    }
}
impl Drop for Connection {
    fn drop(&mut self) {
        let failure = self.finish().err().map(|error| format!("{error:#}"));
        let owner = if self.closed {
            None
        } else {
            // Retain the actual operations, callbacks and COM dependencies.
            // The Windows container lease remains owned by the caller thread.
            let owner = Self {
                container: self.container.clone(),
                acquisition: std::mem::take(&mut self.acquisition),
                device: self.device.take(),
                service: self.service.take(),
                write: self.write.take(),
                notify: self.notify.take(),
                connection_token: self.connection_token.take(),
                value_token: self.value_token.take(),
                notification_enabled: std::mem::replace(&mut self.notification_enabled, false),
                closing: true,
                closed: false,
                device_closed: self.device_closed,
                operations: std::mem::take(&mut self.operations),
                cleanup: std::mem::take(&mut self.cleanup),
                disconnected: self.disconnected.clone(),
                overflow: self.overflow.clone(),
                responses: std::mem::replace(&mut self.responses, mpsc::channel().1),
                write_option: self.write_option,
            };
            self.closed = true;
            self.device_closed = true;
            Some(owner)
        };
        if owner.is_some() || failure.is_some() {
            retain_owner(
                retained_connections(),
                self.container.clone(),
                owner,
                failure,
            );
        }
    }
}

#[cfg(test)]
#[path = "mouse_runtime_artwork_ble_tests.rs"]
mod tests;

struct Feature<'a> {
    target: &'a WindowsBleTarget,
    connection: Option<Connection>,
    sequence: u8,
}
impl Transport for Feature<'_> {
    fn query(
        &mut self,
        query: Query,
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<u8> {
        ensure!(
            query == Query::Sidepad,
            "Current mouse BLE source has no Hyperflux wireless query branch"
        );
        validate()?;
        let _lock = receiver::ReceiverLock::acquire(&self.target.device_container_id, validate)?;
        let result = (|| {
            if self.connection.is_none() {
                self.connection = Some(Connection::connect_with_retry(self.target, validate)?);
                // rzDevice30 validates its connected command channel through
                // Get Edition Information before the first requested query.
                let transaction = self.sequence;
                self.sequence = if self.sequence == 30 { 0 } else { self.sequence + 1 };
                let frame = razer_device::mouse_current_power::ble::frame(
                    transaction,
                    razer_device::mouse_current_power::ble::EDITION,
                    &[],
                )?;
                let data = self
                    .connection
                    .as_mut()
                    .unwrap()
                    .exchange_with_retry(&frame, validate)?;
                ensure!(
                    data.len() >= 3,
                    "BLE connection edition response is truncated"
                );
            }
            let transaction = self.sequence;
            self.sequence = if self.sequence == 30 { 0 } else { self.sequence + 1 };
            let frame = artwork::ble_query_frame(transaction);
            let payload = self
                .connection
                .as_mut()
                .unwrap()
                .exchange_with_retry(&frame, validate)?;
            payload
                .first()
                .copied()
                .context("BLE sidepad response has no actual status byte")
        })();
        // Power and artwork workers share this GATT characteristic. Release
        // this command's CCCD and callbacks before releasing its container
        // lock; installed HID interrupt observation remains continuously owned.
        let cleanup = self
            .connection
            .take()
            .map_or(Ok(()), |mut connection| connection.finish());
        match (result, cleanup) {
            (Ok(value), Ok(())) => Ok(value),
            (Ok(_), Err(cleanup)) => Err(cleanup),
            (Err(error), Ok(())) => Err(error),
            (Err(error), Err(cleanup)) => {
                Err(error.context(format!("BLE command cleanup also failed: {cleanup:#}")))
            }
        }
    }
}

fn interruptible_sleep(
    duration: Duration,
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let deadline = Instant::now() + duration;
    while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
        validate()?;
        std::thread::sleep(remaining.min(Duration::from_millis(5)));
    }
    validate()
}

pub(super) fn run(
    target: &WindowsBleTarget,
    canceled: &AtomicBool,
    publish: &mut impl FnMut(Update) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let _apartment = Apartment::new()?;
    let source =
        artwork::source(target.product_id).context("Current mouse BLE source is absent")?;
    ensure!(
        source.is_ble(target.physical_product_id),
        "BLE physical PID is not the source BLE branch"
    );
    let guard = || {
        ensure!(
            !canceled.load(Ordering::Acquire),
            "Mouse artwork owner canceled"
        );
        current_target(target)
    };
    guard()?;
    let mut feature = Feature {
        target,
        connection: None,
        sequence: 0,
    };
    let mut events = super::super::super::receiver_events::Events::new(
        u16::try_from(target.physical_product_id)?,
        &target.device_container_id,
    );
    // Current source uses registerHidDeviceEvent for Protocol30 events. GATT
    // response fragments must never be interpreted as hardware notification14.
    let result = artwork::monitor(
        source,
        target.physical_product_id,
        &mut feature,
        &mut events,
        &guard,
        publish,
    );
    let cleanup = feature
        .connection
        .as_mut()
        .map_or(Ok(()), Connection::finish);
    match (result, cleanup) {
        (Ok(()), cleanup) => cleanup,
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(cleanup)) => {
            Err(error.context(format!("BLE command cleanup also failed: {cleanup:#}")))
        }
    }
}
