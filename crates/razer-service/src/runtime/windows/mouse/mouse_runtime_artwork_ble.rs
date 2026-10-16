//! Current packaged noble WinRT source: BLEManager::Connect/DiscoverServices/
//! DiscoverCharacteristics/Write/Notify/OnValueChanged and PeripheralWinrt.
//! Command responses use GATT; hardware events remain installed HID interrupts.
use super::{artwork, hid, receiver};
use anyhow::{Context as _, ensure};
use razer_device::mouse_runtime_artwork::{
    BleCommandStatus, BleResponse, Query, Transport, Update, WindowsBleTarget,
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    time::{Duration, Instant},
};
use windows::{
    Devices::Bluetooth::{
        BluetoothCacheMode, BluetoothConnectionStatus, BluetoothLEDevice,
        GenericAttributeProfile::{
            GattCharacteristic, GattCharacteristicProperties,
            GattClientCharacteristicConfigurationDescriptorValue as Descriptor,
            GattCommunicationStatus, GattDeviceService, GattValueChangedEventArgs, GattWriteOption,
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
pub(crate) fn retryable(error: &anyhow::Error) -> bool {
    error.downcast_ref::<ResponseTimeout>().is_some()
        || error
            .downcast_ref::<BleCommandStatus>()
            .is_some_and(BleCommandStatus::retryable)
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

/// All WinRT operations have a deadline and are canceled before returning a
/// failed owner check. This function runs only on the retained service worker.
fn wait<T: RuntimeType + 'static>(
    operation: IAsyncOperation<T>,
    deadline: Duration,
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<T> {
    let started = Instant::now();
    let result = (|| {
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
    })();
    if result.is_err() {
        let _ = operation.Cancel();
    }
    let _ = operation.Close();
    result
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
    device: BluetoothLEDevice,
    service: Option<GattDeviceService>,
    write: Option<GattCharacteristic>,
    notify: Option<GattCharacteristic>,
    connection_token: Option<i64>,
    value_token: Option<i64>,
    notification_enabled: bool,
    closed: bool,
    disconnected: Arc<AtomicBool>,
    overflow: Arc<AtomicBool>,
    responses: Receiver<Result<Vec<u8>, String>>,
    write_option: GattWriteOption,
}

impl Connection {
    pub(crate) fn connect(
        target: &WindowsBleTarget,
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Self> {
        validate()?;
        let device = wait(
            BluetoothLEDevice::FromBluetoothAddressAsync(target.mac_address)?,
            CONNECT_TIMEOUT,
            validate,
        )?;
        ensure!(
            device.BluetoothAddress()? == target.mac_address,
            "WinRT connected a different Bluetooth address"
        );
        let disconnected = Arc::new(AtomicBool::new(false));
        let overflow = Arc::new(AtomicBool::new(false));
        let (responses_tx, responses) = mpsc::sync_channel(64);
        let mut connection = Self {
            device,
            service: None,
            write: None,
            notify: None,
            connection_token: None,
            value_token: None,
            notification_enabled: false,
            closed: false,
            disconnected,
            overflow,
            responses,
            write_option: GattWriteOption::WriteWithResponse,
        };
        let signal = connection.disconnected.clone();
        let handler =
            TypedEventHandler::<BluetoothLEDevice, IInspectable>::new(move |device, _| {
                if let Some(device) = device.as_ref() {
                    if device.ConnectionStatus()? == BluetoothConnectionStatus::Disconnected {
                        signal.store(true, Ordering::Release);
                    }
                }
                Ok(())
            });
        connection.connection_token = Some(connection.device.ConnectionStatusChanged(&handler)?);
        let guard = || {
            validate()?;
            connection.validate()
        };
        let services = wait(
            connection
                .device
                .GetGattServicesWithCacheModeAsync(BluetoothCacheMode::Uncached)?,
            CONNECT_TIMEOUT,
            &guard,
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
        let guard = || {
            validate()?;
            connection.validate()
        };
        let characteristics = wait(
            connection
                .service
                .as_ref()
                .unwrap()
                .GetCharacteristicsWithCacheModeAsync(BluetoothCacheMode::Uncached)?,
            CONNECT_TIMEOUT,
            &guard,
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
            connection.validate()
        };
        let applied = wait(
            notify.WriteClientCharacteristicConfigurationDescriptorWithResultAsync(descriptor)?,
            CONNECT_TIMEOUT,
            &guard,
        )?;
        ensure!(
            applied.Status()? == GattCommunicationStatus::Success,
            "Enable BLE response notifications failed: {:?}",
            applied.Status()?
        );
        let overflow = connection.overflow.clone();
        let handler = TypedEventHandler::<GattCharacteristic, GattValueChangedEventArgs>::new(
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
        Ok(connection)
    }

    fn validate(&self) -> anyhow::Result<()> {
        ensure!(
            !self.disconnected.load(Ordering::Acquire),
            "BLE mouse disconnected during observation"
        );
        ensure!(
            !self.overflow.load(Ordering::Acquire),
            "BLE response queue overflowed or lost its consumer"
        );
        Ok(())
    }

    pub(crate) fn exchange(
        &mut self,
        frame: &[u8],
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Vec<u8>> {
        ensure!(
            frame.len() >= 8 && frame[0] < 31,
            "Protocol30 command header/transaction differs"
        );
        validate()?;
        self.validate()?;
        while self.responses.try_recv().is_ok() {}
        let writer = DataWriter::new()?;
        writer.WriteBytes(frame)?;
        let buffer = writer.DetachBuffer()?;
        writer.Close()?;
        let guard = || {
            validate()?;
            self.validate()
        };
        let sent = wait(
            self.write
                .as_ref()
                .context("BLE write characteristic was released")?
                .WriteValueWithResultAndOptionAsync(&buffer, self.write_option)?,
            RESPONSE_TIMEOUT,
            &guard,
        )?;
        ensure!(
            sent.Status()? == GattCommunicationStatus::Success,
            "BLE Protocol30 command write failed: {:?}",
            sent.Status()?
        );
        let deadline = Instant::now() + RESPONSE_TIMEOUT;
        let mut response = BleResponse::new(frame[0]);
        loop {
            guard()?;
            let remaining = deadline
                .checked_duration_since(Instant::now())
                .ok_or(ResponseTimeout)?;
            match self
                .responses
                .recv_timeout(remaining.min(Duration::from_millis(10)))
            {
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

    pub(crate) fn finish(&mut self) -> anyhow::Result<()> {
        if self.closed {
            return Ok(());
        }
        self.closed = true;
        let mut failures = Vec::new();
        if let (Some(notify), Some(token)) = (&self.notify, self.value_token.take()) {
            if let Err(error) = notify.RemoveValueChanged(token) {
                failures.push(format!("BLE ValueChanged removal: {error}"));
            }
        }
        if self.notification_enabled {
            self.notification_enabled = false;
            if let Some(notify) = &self.notify {
                let cleanup = (|| -> anyhow::Result<()> {
                    let applied = wait(
                        notify.WriteClientCharacteristicConfigurationDescriptorWithResultAsync(
                            Descriptor::None,
                        )?,
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
                }
            }
        }
        if let Some(token) = self.connection_token.take() {
            if let Err(error) = self.device.RemoveConnectionStatusChanged(token) {
                failures.push(format!("BLE connection callback removal: {error}"));
            }
        }
        self.write = None;
        self.notify = None;
        if let Some(service) = self.service.take() {
            if let Err(error) = service.Close() {
                failures.push(format!("BLE service close: {error}"));
            }
        }
        if let Err(error) = self.device.Close() {
            failures.push(format!("BLE device close: {error}"));
        }
        ensure!(failures.is_empty(), "{}", failures.join("; "));
        Ok(())
    }
}
impl Drop for Connection {
    fn drop(&mut self) {
        if let Err(error) = self.finish() {
            eprintln!("[mouse-artwork-ble] {error:#}");
        }
    }
}

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
        let _lock = receiver::ReceiverLock::acquire(&self.target.device_container_id)?;
        let result = (|| {
            if self.connection.is_none() {
                let mut last = None;
                // Current rzDevice30 constants w6=2, ah=150.
                for attempt in 0..2 {
                    if attempt > 0 {
                        interruptible_sleep(Duration::from_millis(150), validate)?;
                    }
                    match Connection::connect(self.target, validate) {
                        Ok(connection) => {
                            self.connection = Some(connection);
                            break;
                        }
                        Err(error) => {
                            validate()?;
                            last = Some(error);
                        }
                    }
                }
                if self.connection.is_none() {
                    return Err(last.context("BLE mouse connection did not run")?);
                }
                // rzDevice30 validates its connected command channel through
                // Get Edition Information before the first requested query.
                let transaction = self.sequence;
                self.sequence = if self.sequence == 30 { 0 } else { self.sequence + 1 };
                let frame = razer_device::mouse_current_power::ble::frame(
                    transaction,
                    razer_device::mouse_current_power::ble::EDITION,
                    &[],
                )?;
                let mut last = None;
                let mut verified = false;
                for attempt in 0..2 {
                    if attempt > 0 {
                        interruptible_sleep(Duration::from_millis(30), validate)?;
                    }
                    match self.connection.as_mut().unwrap().exchange(&frame, validate) {
                        Ok(data) => {
                            ensure!(
                                data.len() >= 3,
                                "BLE connection edition response is truncated"
                            );
                            verified = true;
                            break;
                        }
                        Err(error) => {
                            validate()?;
                            if !retryable(&error) {
                                return Err(error);
                            }
                            last = Some(error);
                        }
                    }
                }
                if !verified {
                    return Err(last.context("BLE edition validation did not run")?);
                }
            }
            let transaction = self.sequence;
            self.sequence = if self.sequence == 30 { 0 } else { self.sequence + 1 };
            let frame = artwork::ble_query_frame(transaction);
            let mut last = None;
            // Current rzDevice30 lI=2, lz=30. Retry retains the same transaction.
            for attempt in 0..2 {
                if attempt > 0 {
                    interruptible_sleep(Duration::from_millis(30), validate)?;
                }
                match self.connection.as_mut().unwrap().exchange(&frame, validate) {
                    Ok(payload) => {
                        return payload
                            .first()
                            .copied()
                            .context("BLE sidepad response has no actual status byte");
                    }
                    Err(error) => {
                        validate()?;
                        if !retryable(&error) {
                            return Err(error);
                        }
                        last = Some(error);
                    }
                }
            }
            Err(last.context("BLE sidepad query did not run")?)
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
