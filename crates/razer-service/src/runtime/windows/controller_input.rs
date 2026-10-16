//! Current host mapping_engine hardware-event callback ABI, verified against
//! the same official DLL/ASAR. The native producer owns its HID transformations;
//! only its callback JSON is passed to the host-neutral current MW parsers.
use super::{NativeRuntime, callback3, copy_string};
use anyhow::{Context as _, ensure};
use razer_device::controller_input::{Event, Parser, Throttle};
use serde_json::{Value, json};
use std::{
    ffi::{CString, c_char},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    time::{Duration, Instant},
};

type HardwareEvent = unsafe extern "C" fn(*const c_char, i32, *const c_char, u64);
type AddDevice = unsafe extern "C" fn(*const c_char, HardwareEvent, super::Callback3);
type Operation = unsafe extern "C" fn(*const c_char, super::Callback3);
type IntervalOperation = unsafe extern "C" fn(*const c_char, u32, super::Callback3);
type SetCallback = unsafe extern "C" fn(*const c_char, HardwareEvent, super::Callback3);
static EVENTS: Mutex<Option<SyncSender<RawEvent>>> = Mutex::new(None);
static OVERFLOW: AtomicBool = AtomicBool::new(false);
struct RawEvent {
    info: String,
    data: String,
    device_event: bool,
    tick: u64,
}
unsafe fn enqueue(info: *const c_char, data: *const c_char, device_event: bool, tick: u64) {
    let event = RawEvent {
        info: unsafe { copy_string(info) },
        data: unsafe { copy_string(data) },
        device_event,
        tick,
    };
    if let Ok(slot) = EVENTS.lock() {
        if let Some(sender) = slot.as_ref() {
            if sender.try_send(event).is_err() {
                OVERFLOW.store(true, Ordering::Release);
            }
        }
    }
}
unsafe extern "C" fn hardware_event(
    info: *const c_char,
    kind: i32,
    data: *const c_char,
    tick: u64,
) {
    if kind == 0 {
        unsafe { enqueue(info, data, false, tick) };
    }
}
unsafe extern "C" fn device_event(info: *const c_char, _kind: i32, data: *const c_char, tick: u64) {
    unsafe { enqueue(info, data, true, tick) };
}

struct Registration {
    product_id: u32,
    physical: Value,
    info: Value,
    register_attempted: bool,
    callback_ready: bool,
    parser: Parser,
    throttle: Throttle,
    clock: Instant,
    last_validation: Instant,
}
pub(super) struct State {
    registration: Option<Registration>,
    receiver: Receiver<RawEvent>,
}
impl State {
    pub(super) fn new() -> Self {
        let (sender, receiver) = mpsc::sync_channel(32);
        if let Ok(mut slot) = EVENTS.lock() {
            *slot = Some(sender);
        }
        OVERFLOW.store(false, Ordering::Release);
        Self {
            registration: None,
            receiver,
        }
    }
    pub(super) fn owner(&self) -> Option<String> {
        self.registration.as_ref()?.info["guid"]
            .as_str()
            .map(str::to_owned)
    }
    fn registration(&self, owner: &str) -> anyhow::Result<&Registration> {
        let registered = self
            .registration
            .as_ref()
            .context("Controller hardware events are not registered")?;
        ensure!(
            registered.info["guid"].as_str() == Some(owner),
            "Controller hardware-event owner changed"
        );
        Ok(registered)
    }
}

fn physical_target(product: u32, physical_pid: u32, container: &str) -> anyhow::Result<Value> {
    ensure!(
        super::receiver::valid_container(container),
        "Controller input requires an observed Windows ContainerId"
    );
    Parser::new(product, physical_pid)?;
    let enumeration = super::usb::enumerate()?;
    ensure!(
        enumeration["complete"] == true
            && enumeration["failures"]
                .as_array()
                .is_some_and(Vec::is_empty),
        "Controller USB identity enumeration is incomplete"
    );
    let candidates = enumeration["devices"]
        .as_array()
        .context("Controller USB identity list missing")?
        .iter()
        .filter(|device| {
            device["vendor_id"] == 0x1532
                && device["device_container_id"]
                    .as_str()
                    .is_some_and(|id| id.eq_ignore_ascii_case(container))
        })
        .collect::<Vec<_>>();
    let [device] = candidates.as_slice() else {
        anyhow::bail!("Controller physical USB identity is absent or ambiguous");
    };
    ensure!(
        device["product_id"] == physical_pid,
        "Controller physical product changed"
    );
    for key in ["path", "device_instance_id"] {
        ensure!(
            device[key].as_str().is_some_and(|text| !text.is_empty()),
            "Controller USB identity has no {key}"
        );
    }
    Ok((*device).clone())
}
fn matches(info: &Value, registered: &Value) -> bool {
    info["vendorId"] == registered["vendorId"]
        && info["productId"] == registered["productId"]
        && info["containerId"]
            .as_str()
            .zip(registered["containerId"].as_str())
            .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b))
        && info["usbInstanceId"] == registered["usbInstanceId"]
        && info
            .get("guid")
            .is_none_or(|guid| guid == &registered["guid"])
}

impl NativeRuntime {
    pub(super) fn start_controller_input(
        &mut self,
        product: u32,
        physical_pid: u32,
        container: &str,
        owner: &str,
    ) -> anyhow::Result<Value> {
        ensure!(
            !owner.is_empty() && !owner.contains('\0'),
            "Invalid controller input owner"
        );
        ensure!(
            self.controller_input.registration.is_none(),
            "Controller input worker already has an owner"
        );
        let physical = physical_target(product, physical_pid, container)?;
        let info = json!({"vendorId":0x1532,"productId":physical_pid,"containerId":physical["device_container_id"],
            "usbInstanceId":physical["device_instance_id"],"guid":owner});
        let parser = Parser::new(product, physical_pid)?;
        let interval = parser.interval_ms();
        self.mapping()?;
        while self.controller_input.receiver.try_recv().is_ok() {}
        OVERFLOW.store(false, Ordering::Release);
        self.controller_input.registration = Some(Registration {
            product_id: product,
            physical,
            info: info.clone(),
            register_attempted: false,
            callback_ready: false,
            parser,
            throttle: Throttle::default(),
            clock: Instant::now(),
            last_validation: Instant::now(),
        });
        let result = (|| {
            let text = CString::new(info.to_string())?;
            let add: AddDevice = unsafe { self.mapping_symbol("addUsbDeviceWithoutFilterDriver")? };
            let result = self.call(|| unsafe { add(text.as_ptr(), device_event, callback3) });
            if self.poisoned {
                std::mem::forget(text);
                return result.map(|_| ());
            }
            result?;
            self.controller_input
                .registration
                .as_mut()
                .unwrap()
                .register_attempted = true;
            let result = if let Some(interval) = interval {
                let register: IntervalOperation =
                    unsafe { self.mapping_symbol("registerIntervaledHardwareEvent")? };
                self.call(|| unsafe { register(text.as_ptr(), interval, callback3) })
            } else {
                let register: Operation = unsafe { self.mapping_symbol("registerHardwareEvent")? };
                self.call(|| unsafe { register(text.as_ptr(), callback3) })
            };
            if self.poisoned {
                std::mem::forget(text);
                return result.map(|_| ());
            }
            result?;
            let install: SetCallback = unsafe { self.mapping_symbol("setHardwareEventCallback")? };
            let result = self.call(|| unsafe { install(text.as_ptr(), hardware_event, callback3) });
            if self.poisoned {
                std::mem::forget(text);
                return result.map(|_| ());
            }
            result?;
            let current = physical_target(product, physical_pid, container)?;
            ensure!(
                current
                    == self
                        .controller_input
                        .registration
                        .as_ref()
                        .unwrap()
                        .physical,
                "Controller USB identity changed during registration"
            );
            self.controller_input
                .registration
                .as_mut()
                .unwrap()
                .callback_ready = true;
            Ok(())
        })();
        if let Err(error) = result {
            let cleanup = if !self.poisoned {
                self.stop_controller_input(owner)
                    .err()
                    .map(|error| format!("{error:#}"))
            } else {
                None
            };
            anyhow::bail!(
                "Controller hardware-event registration failed: {error:#}; cleanup: {cleanup:?}"
            );
        }
        Ok(
            json!({"started":true,"owner_guid":owner,"product_id":product,"device_info":info,"interval_ms":interval}),
        )
    }
    pub(super) fn controller_input_events(&mut self, owner: &str) -> anyhow::Result<Value> {
        let registered = self.controller_input.registration(owner)?;
        ensure!(
            registered.callback_ready,
            "Controller hardware callback was not installed"
        );
        ensure!(
            !OVERFLOW.load(Ordering::Acquire),
            "Controller native callback queue overflowed; observation is incomplete"
        );
        if registered.last_validation.elapsed() >= Duration::from_millis(250) {
            let current = physical_target(
                registered.product_id,
                registered.info["productId"].as_u64().unwrap() as u32,
                registered.info["containerId"].as_str().unwrap(),
            )?;
            ensure!(
                current == registered.physical,
                "Controller USB identity changed while observing input"
            );
            self.controller_input
                .registration
                .as_mut()
                .unwrap()
                .last_validation = Instant::now();
        }
        let registered = self.controller_input.registration.as_mut().unwrap();
        let mut samples = Vec::new();
        let mut notifications = Vec::new();
        let mut last_tick = None;
        let mut unsupported = 0u32;
        while let Ok(event) = self.controller_input.receiver.try_recv() {
            let info: Value = serde_json::from_str(&event.info)
                .context("Controller callback identity is malformed")?;
            if !matches(&info, &registered.info) {
                continue;
            }
            let data: Value = serde_json::from_str(&event.data)
                .context("Controller callback data is malformed")?;
            if event.device_event {
                ensure!(
                    data["type"] != "error",
                    "Controller input device reported error: {data}"
                );
                continue;
            }
            last_tick = Some(event.tick);
            for event in registered.parser.parse(&data)? {
                match event {
                    Event::Buttons { sample } => {
                        if let Some(sample) = registered
                            .throttle
                            .observe(sample, registered.clock.elapsed().as_millis() as u64)
                        {
                            samples.push(sample);
                        }
                    }
                    event @ Event::Notification { .. } => {
                        ensure!(
                            notifications.len() < 512,
                            "Controller notification consumer fell behind"
                        );
                        notifications.push(event);
                    }
                    Event::Unsupported { .. } => {
                        unsupported += 1;
                    }
                }
            }
        }
        if let Some(sample) = registered
            .throttle
            .flush(registered.clock.elapsed().as_millis() as u64)
        {
            samples.push(sample);
        }
        Ok(
            json!({"owner_guid":owner,"product_id":registered.product_id,"device_info":registered.info,
            "samples":samples,"notifications":notifications,"last_tick":last_tick,"unsupported_reports":unsupported}),
        )
    }
    pub(super) fn stop_controller_input(&mut self, owner: &str) -> anyhow::Result<Value> {
        let registered = self.controller_input.registration(owner)?;
        let info = registered.info.clone();
        let product = registered.product_id;
        let attempted = registered.register_attempted;
        let text = CString::new(info.to_string())?;
        let mut errors = Vec::new();
        if attempted {
            let result = (|| {
                let unregister: Operation =
                    unsafe { self.mapping_symbol("unregisterHardwareEvent")? };
                self.call(|| unsafe { unregister(text.as_ptr(), callback3) })
            })();
            match result {
                Ok(_) => {
                    let registered = self.controller_input.registration.as_mut().unwrap();
                    registered.register_attempted = false;
                    registered.callback_ready = false;
                }
                Err(error) => errors.push(format!("unregisterHardwareEvent: {error:#}")),
            }
        }
        if !self.poisoned {
            let result = (|| {
                let remove: Operation = unsafe { self.mapping_symbol("removeUsbDevice")? };
                self.call(|| unsafe { remove(text.as_ptr(), callback3) })
            })();
            match result {
                Ok(_) => self.controller_input.registration = None,
                Err(error) => errors.push(format!("removeUsbDevice: {error:#}")),
            }
        }
        if self.poisoned {
            std::mem::forget(text);
        }
        ensure!(
            errors.is_empty(),
            "Controller input cleanup was not confirmed: {}",
            errors.join("; ")
        );
        while self.controller_input.receiver.try_recv().is_ok() {}
        Ok(json!({"stopped":true,"owner_guid":owner,"product_id":product,"device_info":info}))
    }
}
