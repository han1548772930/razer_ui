//! PID 70 hardware/input callbacks and calibration lifetime on the existing
//! serialized native worker. RemoveUsbDevice's success callback follows device
//! cleanup, callback-slot release and device-thread join in the current DLL.
use super::{Callback2, Callback3, NativeRuntime, callback2, callback3, copy_string};
use anyhow::{Context as _, ensure};
use razer_device::{device_reads::DeviceReadTarget, mouse_philips::measurement::*};
use razer_platform::global_shortcuts::{GlobalShortcuts, Shortcut};
use serde::Deserialize;
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

type EventCallback = unsafe extern "C" fn(*const c_char, i32, *const c_char, u64);
type Add = unsafe extern "C" fn(*const c_char, EventCallback, Callback3);
type Operation = unsafe extern "C" fn(*const c_char, Callback3);
type Install = unsafe extern "C" fn(*const c_char, EventCallback, Callback3);
type MoveCallback = unsafe extern "C" fn(*const c_char, i32, u8, u8, u64);
type InstallMove = unsafe extern "C" fn(*const c_char, MoveCallback, Callback3);
type Global = unsafe extern "C" fn(Callback2);
type QueryActiveCallback = unsafe extern "C" fn(bool, *const c_char, *const c_char, u8, u8);
type QueryActive = unsafe extern "C" fn(*const c_char, QueryActiveCallback);

enum RawEvent {
    Hardware { identity: String, data: String },
    Input { identity: String, data: String },
    Device { identity: String, data: String },
    Redirect { identity: String },
    MouseMove { identity: String },
}

#[cfg(test)]
mod tests {
    use super::*;
    fn physical() -> Physical {
        Physical {
            path: "usb-path".into(),
            device_instance_id: "USB\\VID_1532&PID_0046\\INSTANCE".into(),
            device_container_id: "{container}".into(),
            vendor_id: 0x1532,
            product_id: 70,
        }
    }
    fn identity() -> CallbackIdentity {
        CallbackIdentity {
            vendor_id: 0x1532,
            product_id: 70,
            container_id: "{CONTAINER}".into(),
            usb_instance_id: "usb\\vid_1532&pid_0046\\instance".into(),
            guid: Some("owner-a".into()),
        }
    }
    #[test]
    fn philips_callbacks_reject_replaced_owner_physical_endpoint_or_container() {
        assert!(matches(&identity(), &physical(), "owner-a"));
        assert!(!matches(&identity(), &physical(), "owner-b"));
        let mut changed = identity();
        changed.product_id = 71;
        assert!(!matches(&changed, &physical(), "owner-a"));
        let mut changed = identity();
        changed.vendor_id = 0;
        assert!(!matches(&changed, &physical(), "owner-a"));
        let mut changed = identity();
        changed.container_id = "{other}".into();
        assert!(!matches(&changed, &physical(), "owner-a"));
        let mut changed = identity();
        changed.usb_instance_id.push('2');
        assert!(!matches(&changed, &physical(), "owner-a"));
        assert!(
            serde_json::from_str::<CallbackIdentity>(
                r#"{"vendorId":5426,"productId":70,"containerId":"container"}"#
            )
            .is_err()
        );
    }
    #[test]
    fn philips_driver_ready_requires_the_source_info_event_and_preserves_failures() {
        assert!(driver_ready(r#"{"type":"info","info":"driver ready"}"#).unwrap());
        assert!(
            !driver_ready(r#"{"type":"info","info":"driver ready without filter driver"}"#)
                .unwrap()
        );
        assert!(
            driver_ready(r#"{"type":"error","error":"fail to probe filter driver"}"#)
                .unwrap_err()
                .to_string()
                .contains("fail to probe filter driver")
        );
        for data in [
            r#"{"type":"info"}"#,
            r#"{"type":"error"}"#,
            r#"{"type":"info","info":true}"#,
        ] {
            assert!(driver_ready(data).is_err());
        }
    }
}
static EVENTS: Mutex<Option<SyncSender<RawEvent>>> = Mutex::new(None);
static OVERFLOW: AtomicBool = AtomicBool::new(false);
fn enqueue(event: RawEvent) {
    if let Ok(slot) = EVENTS.lock() {
        if let Some(sender) = slot.as_ref() {
            if sender.try_send(event).is_err() {
                OVERFLOW.store(true, Ordering::Release);
            }
        }
    }
}
unsafe extern "C" fn hardware_event(info: *const c_char, kind: i32, data: *const c_char, _: u64) {
    if kind == 0 {
        // SAFETY: the verified callback supplies borrowed, NUL-terminated strings.
        // Copy while the DLL still owns them; never retain its pointers.
        enqueue(RawEvent::Hardware {
            identity: unsafe { copy_string(info) },
            data: unsafe { copy_string(data) },
        });
    }
}
unsafe extern "C" fn input_event(info: *const c_char, kind: i32, data: *const c_char, _: u64) {
    if kind == 1 {
        enqueue(RawEvent::Input {
            identity: unsafe { copy_string(info) },
            data: unsafe { copy_string(data) },
        });
    }
}
unsafe extern "C" fn device_event(info: *const c_char, kind: i32, data: *const c_char, _: u64) {
    if kind == 5 {
        enqueue(RawEvent::Device {
            identity: unsafe { copy_string(info) },
            data: unsafe { copy_string(data) },
        });
    }
}
unsafe extern "C" fn redirect_event(info: *const c_char, kind: i32, _: *const c_char, _: u64) {
    if kind == 3 {
        enqueue(RawEvent::Redirect {
            identity: unsafe { copy_string(info) },
        });
    }
}
// The current Philips manager collects Z/Q exclusively from protocol report90.
// Its host installs this callback but the manager has no mousemoveredirect
// listener. Counting it as another sample would duplicate an observed stream.
unsafe extern "C" fn mouse_move_event(info: *const c_char, kind: i32, _: u8, _: u8, _: u64) {
    if kind == 4 {
        enqueue(RawEvent::MouseMove {
            identity: unsafe { copy_string(info) },
        });
    }
}
unsafe extern "C" fn active_result(
    success: bool,
    reason: *const c_char,
    _: *const c_char,
    macros: u8,
    keys: u8,
) {
    super::complete(super::CallbackReply {
        success,
        reason: unsafe { copy_string(reason) },
        value: Some(json!({"macros":macros,"keys":keys}).to_string()),
    });
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct Physical {
    path: String,
    device_instance_id: String,
    device_container_id: String,
    vendor_id: u32,
    product_id: u32,
}
impl Physical {
    fn current(target: &DeviceReadTarget) -> anyhow::Result<Self> {
        ensure!(
            target.product_id == 70
                && target.physical_product_id == 70
                && target.peer_product_id.is_none(),
            "Philips measurement requires current direct PID70"
        );
        let (_, value) = super::native_target(&target.device_container_id, Some(70))?;
        Ok(serde_json::from_value(value)?)
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CallbackIdentity {
    vendor_id: u32,
    product_id: u32,
    container_id: String,
    usb_instance_id: String,
    guid: Option<String>,
}
fn matches(info: &CallbackIdentity, physical: &Physical, owner: &str) -> bool {
    info.vendor_id == physical.vendor_id
        && info.product_id == physical.product_id
        && info
            .container_id
            .eq_ignore_ascii_case(&physical.device_container_id)
        && info
            .usb_instance_id
            .eq_ignore_ascii_case(&physical.device_instance_id)
        && info.guid.as_deref().is_none_or(|guid| guid == owner)
}
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum DeviceEvent {
    Info { info: String },
    Error { error: String },
}
fn driver_ready(data: &str) -> anyhow::Result<bool> {
    match serde_json::from_str::<DeviceEvent>(data).context("Malformed Philips device event")? {
        DeviceEvent::Info { info } => Ok(info == "driver ready"),
        DeviceEvent::Error { error } => anyhow::bail!("Philips native driver reported: {error}"),
    }
}
struct Session {
    id: String,
    target: DeviceReadTarget,
    physical: Physical,
    info: String,
    core: Machine,
    commands: Vec<CommandReport>,
    initialization_error: Option<String>,
    driver_ready: bool,
    redirect_callbacks: u64,
    mouse_move_callbacks: u64,
    connection: Option<super::device_reads::MeasurementConnection>,
    added: bool,
    hardware: bool,
    input: bool,
    input_redirect: bool,
    mouse_redirect: bool,
    mappings_suspended: bool,
    escape: bool,
    began: Instant,
    last_validation: Instant,
}
pub(super) struct State {
    session: Option<Session>,
    receiver: Receiver<RawEvent>,
    shortcuts: GlobalShortcuts,
}
impl State {
    pub(super) fn new() -> Self {
        let (sender, receiver) = mpsc::sync_channel(8192);
        if let Ok(mut slot) = EVENTS.lock() {
            *slot = Some(sender);
        }
        Self {
            session: None,
            receiver,
            shortcuts: GlobalShortcuts::default(),
        }
    }
    pub(super) fn owner(&self) -> Option<String> {
        self.session.as_ref().map(|session| session.id.clone())
    }
    fn current(&self, id: &str) -> anyhow::Result<&Session> {
        let session = self
            .session
            .as_ref()
            .context("Philips measurement has no owner")?;
        ensure!(session.id == id, "Philips measurement owner changed");
        Ok(session)
    }
}
impl NativeRuntime {
    fn philips_operation(&mut self, symbol: &str) -> anyhow::Result<()> {
        let info = CString::new(
            self.philips_measurement
                .session
                .as_ref()
                .context("Philips owner absent")?
                .info
                .clone(),
        )?;
        let operation: Operation = unsafe { self.mapping_symbol(symbol)? };
        // SAFETY: info remains alive until the completion callback. A timed-out
        // call poisons this worker and retains the argument through process exit.
        let result = self.call(|| unsafe { operation(info.as_ptr(), callback3) });
        if self.poisoned {
            std::mem::forget(info);
        }
        result.map(|_| ())
    }
    fn philips_install(&mut self, symbol: &str, callback: EventCallback) -> anyhow::Result<()> {
        let info = CString::new(
            self.philips_measurement
                .session
                .as_ref()
                .context("Philips owner absent")?
                .info
                .clone(),
        )?;
        let install: Install = unsafe { self.mapping_symbol(symbol)? };
        let result = self.call(|| unsafe { install(info.as_ptr(), callback, callback3) });
        if self.poisoned {
            std::mem::forget(info);
        }
        result.map(|_| ())
    }
    fn philips_global(&mut self, symbol: &str) -> anyhow::Result<()> {
        let operation: Global = unsafe { self.mapping_symbol(symbol)? };
        self.call(|| unsafe { operation(callback2) })?;
        Ok(())
    }
    fn register_philips(&mut self) -> anyhow::Result<()> {
        self.mapping()?;
        let info = CString::new(
            self.philips_measurement
                .session
                .as_ref()
                .unwrap()
                .info
                .clone(),
        )?;
        let add: Add = unsafe { self.mapping_symbol("addUsbDevice")? };
        self.philips_measurement.session.as_mut().unwrap().added = true;
        let result = self.call(|| unsafe { add(info.as_ptr(), device_event, callback3) });
        if self.poisoned {
            std::mem::forget(info);
        }
        result?;
        // The current host awaits both add's completion and this correlated
        // device event. Add acknowledgement alone cannot prove filter readiness.
        let deadline = Instant::now() + super::CALLBACK_TIMEOUT;
        loop {
            ensure!(
                !OVERFLOW.load(Ordering::Acquire),
                "Philips registration callback queue overflowed"
            );
            let remaining = deadline.saturating_duration_since(Instant::now());
            let event = self
                .philips_measurement
                .receiver
                .recv_timeout(remaining)
                .context("Philips add callback completed without its driver-ready event")?;
            if let RawEvent::Device { identity, data } = event {
                let identity: CallbackIdentity = serde_json::from_str(&identity)?;
                let session = self.philips_measurement.session.as_ref().unwrap();
                if !matches(&identity, &session.physical, &session.id) {
                    continue;
                }
                if driver_ready(&data)? {
                    self.philips_measurement
                        .session
                        .as_mut()
                        .unwrap()
                        .driver_ready = true;
                    break;
                }
            }
        }
        self.philips_measurement.session.as_mut().unwrap().hardware = true;
        self.philips_operation("registerHardwareEvent")?;
        self.philips_install("setHardwareEventCallback", hardware_event)?;
        self.philips_measurement.session.as_mut().unwrap().input = true;
        self.philips_operation("registerInputNotification")?;
        self.philips_install("setInputNotificationCallback", input_event)?;
        let target = self
            .philips_measurement
            .session
            .as_ref()
            .unwrap()
            .target
            .clone();
        let (connection, initialization_error) =
            super::device_reads::MeasurementConnection::open(&target)?;
        let session = self.philips_measurement.session.as_mut().unwrap();
        session.connection = Some(connection);
        session.initialization_error = initialization_error;
        Ok(())
    }
    pub(super) fn start_philips_measurement(
        &mut self,
        id: String,
        target: DeviceReadTarget,
        kind: Kind,
    ) -> anyhow::Result<Value> {
        ensure!(
            !id.is_empty() && id.len() <= 128 && !id.contains('\0'),
            "Invalid Philips owner"
        );
        if let Some(previous) = self.philips_measurement.session.as_ref() {
            ensure!(
                previous.core.cleanup_confirmed()
                    && !previous.added
                    && !previous.escape
                    && !previous.hardware
                    && !previous.input
                    && !previous.input_redirect
                    && !previous.mouse_redirect
                    && !previous.mappings_suspended
                    && previous.connection.is_none(),
                "Previous Philips calibration cleanup is not confirmed"
            );
        }
        let physical = Physical::current(&target)?;
        let info = json!({"vendorId":physical.vendor_id,"productId":physical.product_id,
            "containerId":physical.device_container_id,"usbInstanceId":physical.device_instance_id,"guid":id}).to_string();
        while self.philips_measurement.receiver.try_recv().is_ok() {}
        OVERFLOW.store(false, Ordering::Release);
        self.philips_measurement.session = Some(Session {
            id: id.clone(),
            target,
            physical,
            info,
            core: Machine::new(kind),
            commands: vec![],
            initialization_error: None,
            driver_ready: false,
            redirect_callbacks: 0,
            mouse_move_callbacks: 0,
            connection: None,
            added: false,
            hardware: false,
            input: false,
            input_redirect: false,
            mouse_redirect: false,
            mappings_suspended: false,
            escape: false,
            began: Instant::now(),
            last_validation: Instant::now(),
        });
        if let Err(error) = self.register_philips() {
            self.philips_measurement
                .session
                .as_mut()
                .unwrap()
                .core
                .abort(format!("Register Philips callbacks: {error:#}"));
        }
        self.advance_philips()?;
        self.philips_measurement_progress(&id)
    }
    fn redirect_philips(&mut self, enabled: bool) -> anyhow::Result<()> {
        if enabled {
            let info = CString::new(
                self.philips_measurement
                    .session
                    .as_ref()
                    .unwrap()
                    .info
                    .clone(),
            )?;
            let query: QueryActive =
                unsafe { self.mapping_symbol("queryRunningMacrosAndKeysPressed")? };
            let result = self.call(|| unsafe { query(info.as_ptr(), active_result) });
            if self.poisoned {
                std::mem::forget(info);
            }
            #[derive(Deserialize)]
            struct Active {
                macros: u8,
                keys: u8,
            }
            let active: Active = serde_json::from_str(
                &result?
                    .value
                    .context("Philips active input query returned no counts")?,
            )?;
            ensure!(
                active.macros == 0 && active.keys == 0,
                "Calibration redirect has active macros or pressed keys"
            );
            self.philips_measurement
                .session
                .as_mut()
                .unwrap()
                .mappings_suspended = true;
            self.philips_global("disableMapping")?;
            self.philips_measurement
                .session
                .as_mut()
                .unwrap()
                .input_redirect = true;
            self.philips_operation("enableInputRedirect")?;
            self.philips_install("setInputRedirectCallback", redirect_event)?;
            // Current manager obtains clicks through input notification/report77;
            // input redirection itself still belongs to this restoration lease.
            self.philips_measurement
                .session
                .as_mut()
                .unwrap()
                .mouse_redirect = true;
            self.philips_operation("enableMouseMoveRedirect")?;
            let info = CString::new(
                self.philips_measurement
                    .session
                    .as_ref()
                    .unwrap()
                    .info
                    .clone(),
            )?;
            let install: InstallMove =
                unsafe { self.mapping_symbol("setMouseMoveRedirectCallback")? };
            let result =
                self.call(|| unsafe { install(info.as_ptr(), mouse_move_event, callback3) });
            if self.poisoned {
                std::mem::forget(info);
            }
            result?;
            Ok(())
        } else {
            let mut errors = Vec::new();
            if self
                .philips_measurement
                .session
                .as_ref()
                .unwrap()
                .mappings_suspended
            {
                match self.philips_global("enableMapping") {
                    Ok(()) => {
                        self.philips_measurement
                            .session
                            .as_mut()
                            .unwrap()
                            .mappings_suspended = false
                    }
                    Err(error) => errors.push(format!("enableMapping: {error:#}")),
                }
            }
            for (symbol, mouse) in [
                ("disableInputRedirect", false),
                ("disableMouseMoveRedirect", true),
            ] {
                let session = self.philips_measurement.session.as_ref().unwrap();
                if if mouse {
                    session.mouse_redirect
                } else {
                    session.input_redirect
                } {
                    match self.philips_operation(symbol) {
                        Ok(()) => {
                            let session = self.philips_measurement.session.as_mut().unwrap();
                            if mouse {
                                session.mouse_redirect = false;
                            } else {
                                session.input_redirect = false;
                            }
                        }
                        Err(error) => errors.push(format!("{symbol}: {error:#}")),
                    }
                }
            }
            ensure!(errors.is_empty(), "{}", errors.join("; "));
            Ok(())
        }
    }
    fn release_philips(&mut self) -> anyhow::Result<()> {
        let mut errors = Vec::new();
        // A failed earlier redirect stop is retried before removing the device.
        if let Err(error) = self.redirect_philips(false) {
            errors.push(format!("Restore redirects: {error:#}"));
        }
        if self.philips_measurement.session.as_ref().unwrap().escape {
            match self.philips_measurement.shortcuts.unregister(27, 0) {
                Ok(()) => self.philips_measurement.session.as_mut().unwrap().escape = false,
                Err(error) => errors.push(format!("Unregister calibration Escape: {error:#}")),
            }
        }
        for (symbol, hardware) in [
            ("unregisterInputNotification", false),
            ("unregisterHardwareEvent", true),
        ] {
            let session = self.philips_measurement.session.as_ref().unwrap();
            if if hardware { session.hardware } else { session.input } {
                match self.philips_operation(symbol) {
                    Ok(()) => {
                        let session = self.philips_measurement.session.as_mut().unwrap();
                        if hardware {
                            session.hardware = false;
                        } else {
                            session.input = false;
                        }
                    }
                    Err(error) => errors.push(format!("{symbol}: {error:#}")),
                }
            }
        }
        if self.philips_measurement.session.as_ref().unwrap().added {
            match self.philips_operation("removeUsbDevice") {
                Ok(()) => {
                    let session = self.philips_measurement.session.as_mut().unwrap();
                    session.added = false;
                    session.hardware = false;
                    session.input = false;
                    session.input_redirect = false;
                    session.mouse_redirect = false;
                    // Only the successful remove completion proves native
                    // callback context release. Keep the Feature lease until it.
                    session.connection = None;
                }
                Err(error) => errors.push(format!("removeUsbDevice: {error:#}")),
            }
        } else {
            self.philips_measurement
                .session
                .as_mut()
                .unwrap()
                .connection = None;
        }
        ensure!(
            errors.is_empty(),
            "Philips callback cleanup unconfirmed: {}",
            errors.join("; ")
        );
        while self.philips_measurement.receiver.try_recv().is_ok() {}
        Ok(())
    }
    fn advance_philips(&mut self) -> anyhow::Result<()> {
        while let Some(effect) = self
            .philips_measurement
            .session
            .as_mut()
            .unwrap()
            .core
            .next_effect()
        {
            let result = match effect {
                Effect::Device(command) => {
                    let mut sent = false;
                    let result = self
                        .philips_measurement
                        .session
                        .as_ref()
                        .unwrap()
                        .connection
                        .as_ref()
                        .context("Philips Feature connection is not open")
                        .and_then(|connection| connection.exchange(command, &mut || sent = true));
                    self.philips_measurement
                        .session
                        .as_mut()
                        .unwrap()
                        .commands
                        .push(CommandReport {
                            command,
                            acknowledged: result.as_ref().ok().cloned(),
                            command_may_have_reached_device: sent,
                            error: result.as_ref().err().map(|error| format!("{error:#}")),
                        });
                    result.map(Some)
                }
                Effect::Escape(enabled) => {
                    let session = self.philips_measurement.session.as_ref().unwrap();
                    let result = if enabled {
                        self.philips_measurement.shortcuts.register(Shortcut {
                            virtual_key: 27,
                            modifiers: 0,
                            argument: session.id.clone(),
                        })
                    } else if session.escape {
                        self.philips_measurement.shortcuts.unregister(27, 0)
                    } else {
                        Ok(())
                    };
                    if result.is_ok() {
                        self.philips_measurement.session.as_mut().unwrap().escape = enabled;
                    }
                    result.map(|_| None)
                }
                Effect::Redirect(enabled) => self.redirect_philips(enabled).map(|_| None),
                Effect::Release => self.release_philips().map(|_| None),
            };
            let session = self.philips_measurement.session.as_mut().unwrap();
            session
                .core
                .complete(effect, result, session.began.elapsed().as_millis() as u64)?;
        }
        Ok(())
    }
    fn philips_measurement_progress(&self, id: &str) -> anyhow::Result<Value> {
        let session = self.philips_measurement.current(id)?;
        let mut progress = session
            .core
            .progress(id, &session.target, session.commands.clone());
        progress.initialization_error = session.initialization_error.clone();
        progress.driver_ready = session.driver_ready;
        progress.redirect_callbacks = session.redirect_callbacks;
        progress.mouse_move_callbacks = session.mouse_move_callbacks;
        Ok(serde_json::to_value(progress)?)
    }
    pub(super) fn poll_philips_measurement(&mut self, id: &str) -> anyhow::Result<Value> {
        self.philips_measurement.current(id)?;
        let observation = (|| -> anyhow::Result<()> {
            ensure!(
                !OVERFLOW.load(Ordering::Acquire),
                "Philips callback queue overflowed; samples are incomplete"
            );
            let session = self.philips_measurement.session.as_mut().unwrap();
            if session.core.phase() == Phase::Finished {
                return Ok(());
            }
            if session.last_validation.elapsed() >= Duration::from_millis(250) {
                ensure!(
                    Physical::current(&session.target)? == session.physical,
                    "Philips physical USB identity changed"
                );
                session
                    .connection
                    .as_ref()
                    .context("Philips connection closed during measurement")?
                    .validate()?;
                session.last_validation = Instant::now();
            }
            let now = session.began.elapsed().as_millis() as u64;
            for event in self.philips_measurement.shortcuts.drain()? {
                if event.shortcut().virtual_key == 27 && event.shortcut().argument == session.id {
                    session.core.cancel();
                }
            }
            while let Ok(raw) = self.philips_measurement.receiver.try_recv() {
                let identity = match &raw {
                    RawEvent::Hardware { identity, .. }
                    | RawEvent::Input { identity, .. }
                    | RawEvent::Device { identity, .. }
                    | RawEvent::Redirect { identity }
                    | RawEvent::MouseMove { identity } => identity,
                };
                let identity: CallbackIdentity = serde_json::from_str(identity)
                    .context("Malformed Philips callback identity")?;
                if !matches(&identity, &session.physical, &session.id) {
                    continue;
                }
                let input = match raw {
                    RawEvent::Hardware { data, .. } => parse_report(
                        &serde_json::from_str::<Vec<u8>>(&data)
                            .context("Malformed Philips hardware bytes")?,
                    )?,
                    RawEvent::Input { data, .. } => {
                        #[derive(Deserialize)]
                        struct InputData {
                            id: u32,
                            #[serde(rename = "type")]
                            input_type: String,
                        }
                        let data: InputData = serde_json::from_str(&data)
                            .context("Malformed Philips input notification")?;
                        (data.id == 1 && data.input_type == "mouse").then_some(Input::LeftDown)
                    }
                    RawEvent::Device { data, .. } => {
                        driver_ready(&data)?;
                        None
                    }
                    RawEvent::Redirect { .. } => {
                        session.redirect_callbacks += 1;
                        None
                    }
                    RawEvent::MouseMove { .. } => {
                        session.mouse_move_callbacks += 1;
                        None
                    }
                };
                if let Some(input) = input {
                    session.core.observe(input, now);
                }
            }
            session.core.tick(now);
            Ok(())
        })();
        if let Err(error) = observation {
            self.philips_measurement
                .session
                .as_mut()
                .unwrap()
                .core
                .abort(format!("{error:#}"));
        }
        self.advance_philips()?;
        self.philips_measurement_progress(id)
    }
    pub(super) fn cancel_philips_measurement(&mut self, id: &str) -> anyhow::Result<Value> {
        self.philips_measurement.current(id)?;
        self.philips_measurement
            .session
            .as_mut()
            .unwrap()
            .core
            .cancel();
        self.advance_philips()?;
        self.philips_measurement_progress(id)
    }
    pub(super) fn shutdown_philips_measurement(&mut self) -> anyhow::Result<Option<Value>> {
        let Some(id) = self.philips_measurement.owner() else {
            return Ok(None);
        };
        self.cancel_philips_measurement(&id)?;
        self.philips_measurement
            .session
            .as_mut()
            .unwrap()
            .core
            .retry_cleanup();
        self.advance_philips()?;
        self.philips_measurement_progress(&id).map(Some)
    }
}
