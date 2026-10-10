//! Current RzAudioUtil 1.0.3.1 local capture-to-render WASAPI routing.
//! IDA evidence: docs/re/evidence/audio-router-ida.json. No target execution.
use crate::audio_util::windows::Com;
use anyhow::{Context, Result, ensure};
use razer_device::{
    audio_router::{AudioFifo, StreamFormat},
    audio_util::{AudioEndpoint, AudioFlow},
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    ffi::c_void,
    ptr::{null, null_mut},
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use windows_sys::{
    Win32::{
        Foundation::{CloseHandle, GetLastError, HANDLE},
        System::{
            Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize},
            Threading::{CreateEventW, SetEvent, WaitForMultipleObjects},
        },
    },
    core::GUID,
};

const ENUMERATOR_CLASS: GUID = GUID::from_u128(0xbcde0395_e52f_467c_8e3d_c4579291692e);
const ENUMERATOR_INTERFACE: GUID = GUID::from_u128(0xa95664d2_9614_4f35_a746_de8db63617e6);
const ENDPOINT: GUID = GUID::from_u128(0x1be09788_6894_4089_8586_9a2a6c265ac5);
const DEVICE: GUID = GUID::from_u128(0xd666063f_1587_4e43_81f1_b948e807363f);
const AUDIO_CLIENT: GUID = GUID::from_u128(0x1cb9ad4c_dbfa_4c32_b178_c2f568a703b2);
const CAPTURE_CLIENT: GUID = GUID::from_u128(0xc8adbd64_e71e_48a0_a4de_185c395cd317);
const RENDER_CLIENT: GUID = GUID::from_u128(0xf294acfc_3146_4483_a7bf_addca7c260e2);
const AUDIO_CLOCK: GUID = GUID::from_u128(0xcd63314f_3fba_4a1b_812c_ef96358728e7);

#[link(name = "ole32")]
unsafe extern "system" {
    fn CoMarshalInterThreadInterfaceInStream(
        iid: *const GUID,
        unknown: *mut c_void,
        stream: *mut *mut c_void,
    ) -> i32;
    fn CoGetInterfaceAndReleaseStream(
        stream: *mut c_void,
        iid: *const GUID,
        output: *mut *mut c_void,
    ) -> i32;
    fn CoReleaseMarshalData(stream: *mut c_void) -> i32;
}

#[link(name = "avrt")]
unsafe extern "system" {
    fn AvSetMmThreadCharacteristicsW(task: *const u16, index: *mut u32) -> HANDLE;
    fn AvRevertMmThreadCharacteristics(handle: HANDLE) -> i32;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetSystemTimeAdjustment(
        adjustment: *mut u32,
        increment: *mut u32,
        disabled: *mut i32,
    ) -> i32;
}

fn exact(hr: i32, operation: &str) -> Result<()> {
    ensure!(hr == 0, "{operation}: 0x{:08x}", hr as u32);
    Ok(())
}

struct Apartment;
impl Apartment {
    fn open() -> Result<Self> {
        let hr = unsafe { CoInitializeEx(null(), 0) };
        ensure!(hr >= 0, "AudioRouter CoInitializeEx: 0x{:08x}", hr as u32);
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

fn enumerator() -> Result<Com> {
    let mut output = null_mut();
    let hr = unsafe {
        CoCreateInstance(
            &ENUMERATOR_CLASS,
            null_mut(),
            0x17,
            &ENUMERATOR_INTERFACE,
            &mut output,
        )
    };
    Com::from_hresult(hr, output, "AudioRouter MMDeviceEnumerator")
}

fn device(enumerator: &Com, id: &str) -> Result<Com> {
    ensure!(!id.contains('\0'), "AudioRouter endpoint ID contains NUL");
    let id = id.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    type GetDevice = unsafe extern "system" fn(*mut c_void, *const u16, *mut *mut c_void) -> i32;
    let call: GetDevice = unsafe { enumerator.method(5) };
    let mut output = null_mut();
    let hr = unsafe { call(enumerator.raw(), id.as_ptr(), &mut output) };
    Com::from_hresult(hr, output, "AudioRouter GetDevice")
}

fn flow(device: &Com) -> Result<i32> {
    type Query = unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32;
    let call: Query = unsafe { device.method(0) };
    let mut output = null_mut();
    let hr = unsafe { call(device.raw(), &ENDPOINT, &mut output) };
    let endpoint = Com::from_hresult(hr, output, "AudioRouter IMMEndpoint")?;
    type GetFlow = unsafe extern "system" fn(*mut c_void, *mut i32) -> i32;
    let call: GetFlow = unsafe { endpoint.method(3) };
    let mut result = -1;
    exact(unsafe { call(endpoint.raw(), &mut result) }, "GetDataFlow")?;
    Ok(result)
}

pub(super) enum Command {
    Enable(i32),
    Route {
        primary: String,
        routed: String,
        product_id: u32,
    },
    Drain,
}
type Request = (Command, Sender<Result<Value>>);

pub(super) struct Owner {
    sender: Option<Sender<Request>>,
    thread: Option<JoinHandle<()>>,
}
impl Owner {
    pub(super) fn open() -> Result<Self> {
        let (sender, receiver) = mpsc::channel();
        let (ready_sender, ready) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("razer-audio-router".into())
            .spawn(move || match Router::open() {
                Ok(mut router) => {
                    let _ = ready_sender.send(Ok(()));
                    router.run(receiver);
                }
                Err(error) => {
                    let _ = ready_sender.send(Err(error));
                }
            })?;
        match ready
            .recv()
            .context("AudioRouter owner exited during initialization")?
        {
            Ok(()) => Ok(Self {
                sender: Some(sender),
                thread: Some(thread),
            }),
            Err(error) => {
                let _ = thread.join();
                Err(error)
            }
        }
    }
    pub(super) fn call(&mut self, command: Command) -> Result<Value> {
        let (sender, receiver) = mpsc::channel();
        self.sender
            .as_ref()
            .context("AudioRouter is closed")?
            .send((command, sender))
            .map_err(|_| anyhow::anyhow!("AudioRouter owner exited"))?;
        receiver
            .recv()
            .context("AudioRouter response channel closed")?
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        self.sender.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

struct Route {
    primary: String,
    routed: String,
    product_id: u32,
    capture: Option<AudioEndpoint>,
    render: Option<AudioEndpoint>,
    bound: Option<(Com, Com)>,
    state: u32,
    dirty: bool,
    stream: Option<Stream>,
    error: Option<String>,
}
impl Route {
    fn matches(&self, friendly: &str) -> bool {
        friendly.contains(&self.primary) || friendly.contains(&self.routed)
    }
    fn initialize(&mut self) -> Result<()> {
        // Native 0xd1b0 uses the same OR substring predicate for both flows,
        // selecting the first item in the actual collection order.
        self.capture = Some(
            crate::audio_util::enumerate(AudioFlow::Record)?
                .endpoints
                .into_iter()
                .find(|endpoint| self.matches(&endpoint.friendly_name))
                .context("AudioRouter matched no active capture endpoint")?,
        );
        self.render = Some(
            crate::audio_util::enumerate(AudioFlow::Playback)?
                .endpoints
                .into_iter()
                .find(|endpoint| self.matches(&endpoint.friendly_name))
                .context("AudioRouter matched no active render endpoint")?,
        );
        let enumerator = enumerator()?;
        let capture = device(&enumerator, &self.capture.as_ref().unwrap().id)?;
        let render = device(&enumerator, &self.render.as_ref().unwrap().id)?;
        ensure!(
            flow(&capture)? == 1 && flow(&render)? == 0,
            "AudioRouter endpoint flow mismatch"
        );
        // Keep the actual IMMDevice objects like native 0x460e0. Marshal them
        // into the PCM owner instead of reopening IDs or sharing raw COM.
        self.bound = Some((capture, render));
        self.state = 1;
        Ok(())
    }
    fn start(&mut self) -> Result<()> {
        self.stream.take();
        let (capture, render) = self
            .bound
            .as_ref()
            .context("AudioRouter has no initialized endpoints")?;
        self.stream = Some(Stream::open(
            Marshalled::open(capture)?,
            Marshalled::open(render)?,
        )?);
        self.state = 2;
        Ok(())
    }
    fn stop(&mut self) {
        self.stream.take();
        self.state = 0;
    }
    fn active(&self) -> Result<bool> {
        if self.state == 0 {
            return Ok(false);
        }
        let enumerator = enumerator()?;
        type State = unsafe extern "system" fn(*mut c_void, *mut u32) -> i32;
        for endpoint in [&self.capture, &self.render] {
            let endpoint = endpoint
                .as_ref()
                .context("AudioRouter missing bound endpoint")?;
            let device = device(&enumerator, &endpoint.id)?;
            let call: State = unsafe { device.method(6) };
            let mut state = 4;
            let _ = unsafe { call(device.raw(), &mut state) };
            if state != 1 {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

impl Drop for Route {
    fn drop(&mut self) {
        self.stop();
    }
}

struct Router {
    // Field order releases all COM owners before the apartment.
    routes: BTreeMap<Vec<u16>, Route>,
    notifications: crate::audio_notification::AudioNotifications,
    enabled: i32,
    due: Option<Instant>,
    events: Vec<Value>,
    _apartment: Apartment,
}
impl Router {
    fn open() -> Result<Self> {
        let apartment = Apartment::open()?;
        let mut notifications = crate::audio_notification::AudioNotifications::default();
        notifications.enable(true)?;
        Ok(Self {
            routes: BTreeMap::new(),
            notifications,
            enabled: 1,
            due: None,
            events: Vec::new(),
            _apartment: apartment,
        })
    }
    fn run(&mut self, receiver: Receiver<Request>) {
        loop {
            self.observe();
            match receiver.recv_timeout(Duration::from_millis(25)) {
                Ok((command, response)) => {
                    let _ = response.send(self.command(command));
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        }
        self.routes.clear();
        let _ = self.notifications.enable(false);
    }
    fn command(&mut self, command: Command) -> Result<Value> {
        match command {
            Command::Enable(enable) => {
                let mut diagnostics = Vec::new();
                if enable != self.enabled {
                    self.enabled = enable;
                    for route in self.routes.values_mut() {
                        let result = if enable != 0 {
                            route.start()
                        } else {
                            route.stop();
                            Ok(())
                        };
                        if let Err(error) = result {
                            route.error = Some(format!("{error:#}"));
                            diagnostics
                                .push(json!({"primaryDevice":route.primary,"error":route.error}));
                        }
                    }
                }
                // 0xba30 deliberately discards individual route HRESULTs.
                Ok(json!({"source_response":{"response":{"enabled":enable}},
                    "diagnostics":diagnostics,"vendor_dll_loaded":false}))
            }
            Command::Route {
                primary,
                routed,
                product_id,
            } => {
                let key = primary.encode_utf16().collect::<Vec<_>>();
                self.routes.remove(&key); // Source destroys old route first.
                if routed.is_empty() {
                    return Ok(json!({"source_response":{}}));
                }
                let mut route = Route {
                    primary: primary.clone(),
                    routed,
                    product_id,
                    capture: None,
                    render: None,
                    bound: None,
                    state: 0,
                    dirty: false,
                    stream: None,
                    error: None,
                };
                let result = route.initialize().and_then(|()| {
                    if self.enabled != 0 {
                        route.start()
                    } else {
                        Ok(())
                    }
                });
                if let Err(error) = &result {
                    route.error = Some(format!("{error:#}"));
                }
                // Failure retains the newly inserted node; no rollback.
                self.routes.insert(key, route);
                result?;
                Ok(json!({"source_response":{},"vendor_dll_loaded":false}))
            }
            Command::Drain => {
                let observations = self
                    .routes
                    .values()
                    .map(|route| {
                        json!({
                            "primaryDevice":route.primary,"routedDevice":route.routed,"state":route.state,
                            "capture":route.capture,"render":route.render,"error":route.error,
                            "pump_error":route.stream.as_ref().and_then(Stream::error)
                        })
                    })
                    .collect::<Vec<_>>();
                Ok(json!({"events":std::mem::take(&mut self.events),"routes":observations}))
            }
        }
    }
    fn observe(&mut self) {
        if let Ok(events) = self.notifications.drain() {
            for event in events {
                // 0xd830 resolves the changed ID before searching, and returns
                // only the first route in the source UTF-16 map order.
                let friendly = (|| {
                    let enumerator = enumerator()?;
                    let endpoint = device(&enumerator, &event.endpoint_id)?;
                    crate::audio_util::windows::property(&endpoint, 14)
                })();
                let Ok(friendly) = friendly else {
                    continue;
                };
                for route in self.routes.values_mut() {
                    let matched = if route.state == 0 {
                        route.matches(&friendly)
                    } else {
                        route
                            .capture
                            .as_ref()
                            .is_some_and(|e| e.id == event.endpoint_id)
                            || route
                                .render
                                .as_ref()
                                .is_some_and(|e| e.id == event.endpoint_id)
                    };
                    if matched {
                        route.dirty = true;
                        self.due = Some(Instant::now() + Duration::from_secs(3));
                        break;
                    }
                }
            }
        }
        if !self.due.is_some_and(|due| Instant::now() >= due) {
            return;
        }
        self.due = None;
        for route in self.routes.values_mut().filter(|route| route.dirty) {
            if route.state == 2 {
                route.stop();
            } else if route.state == 0 {
                if let Err(error) = route.initialize() {
                    route.error = Some(format!("{error:#}"));
                }
            }
            let active = route.active().unwrap_or(false);
            if active && self.enabled != 0 {
                if let Err(error) = route.start() {
                    route.error = Some(format!("{error:#}"));
                }
            }
            self.events.push(json!({"event":"RzAudioUtilEvent",
                "eventType":"AudioRouter_StatusChange","deviceId":route.product_id,
                "status":u32::from(route.state == 2),"deviceStatus":u32::from(active)}));
        }
    }
}

struct Event(HANDLE);
// Kernel events are cross-thread synchronization objects; COM objects are not.
unsafe impl Send for Event {}
unsafe impl Sync for Event {}
impl Event {
    fn open() -> Result<Self> {
        let handle = unsafe { CreateEventW(null(), 0, 0, null()) };
        ensure!(!handle.is_null(), "AudioRouter CreateEventW failed");
        Ok(Self(handle))
    }
}
impl Drop for Event {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}
struct Stream {
    stop: Arc<Event>,
    thread: Option<JoinHandle<Result<()>>>,
    diagnostics: Arc<Mutex<PumpDiagnostics>>,
}
struct Marshalled(*mut c_void);
// This pointer is an IStream expressly created for interthread COM transfer.
unsafe impl Send for Marshalled {}
impl Marshalled {
    fn open(device: &Com) -> Result<Self> {
        let mut stream = null_mut();
        let hr =
            unsafe { CoMarshalInterThreadInterfaceInStream(&DEVICE, device.raw(), &mut stream) };
        let result = Self(stream);
        ensure!(
            hr >= 0 && !result.0.is_null(),
            "AudioRouter COM marshal: 0x{:08x}",
            hr as u32
        );
        Ok(result)
    }
    fn consume(mut self) -> Result<Com> {
        let stream = std::mem::replace(&mut self.0, null_mut());
        let mut device = null_mut();
        let hr = unsafe { CoGetInterfaceAndReleaseStream(stream, &DEVICE, &mut device) };
        Com::from_hresult(hr, device, "AudioRouter COM unmarshal")
    }
}
impl Drop for Marshalled {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { CoReleaseMarshalData(self.0) };
            // Release the IStream if it was never consumed by the PCM thread.
            let _ = Com::from_hresult(0, self.0, "AudioRouter marshal stream cleanup");
        }
    }
}
#[derive(Default)]
struct PumpDiagnostics {
    capture_errors: u64,
    render_errors: u64,
    last_capture_error: Option<String>,
    last_render_error: Option<String>,
    terminal_error: Option<String>,
}
impl Stream {
    fn open(capture: Marshalled, render: Marshalled) -> Result<Self> {
        let stop = Arc::new(Event::open()?);
        let signal = stop.clone();
        let diagnostics = Arc::new(Mutex::new(PumpDiagnostics::default()));
        let pump_diagnostics = diagnostics.clone();
        let (sender, ready) = mpsc::channel();
        let thread = thread::Builder::new()
            .name("razer-wasapi-route".into())
            .spawn(move || {
                let apartment = Apartment::open();
                let pump = apartment
                    .as_ref()
                    .map_err(|error| anyhow::anyhow!("{error:#}"))
                    .and_then(|_| Pump::open(capture.consume()?, render.consume()?));
                let mut pump = match pump {
                    Ok(pump) => {
                        let _ = sender.send(Ok(()));
                        pump
                    }
                    Err(error) => {
                        let message = format!("{error:#}");
                        let _ = sender.send(Err(anyhow::anyhow!(message)));
                        return Err(error);
                    }
                };
                let result = pump.run(signal.0, &pump_diagnostics);
                if let Err(error) = &result {
                    if let Ok(mut diagnostics) = pump_diagnostics.lock() {
                        diagnostics.terminal_error = Some(format!("{error:#}"));
                    }
                }
                result
            })?;
        match ready
            .recv()
            .context("AudioRouter pump exited during initialization")?
        {
            Ok(()) => Ok(Self {
                stop,
                thread: Some(thread),
                diagnostics,
            }),
            Err(error) => {
                let _ = thread.join();
                Err(error)
            }
        }
    }
    fn error(&self) -> Option<Value> {
        let diagnostics = self.diagnostics.lock().ok()?;
        Some(json!({
            "capture_errors":diagnostics.capture_errors,
            "render_errors":diagnostics.render_errors,
            "last_capture_error":diagnostics.last_capture_error,
            "last_render_error":diagnostics.last_render_error,
            "terminal_error":diagnostics.terminal_error,
            "thread_exited":self.thread.as_ref().is_some_and(|thread| thread.is_finished())
        }))
    }
}
impl Drop for Stream {
    fn drop(&mut self) {
        unsafe { SetEvent(self.stop.0) };
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

struct Pump {
    capture_client: Com,
    render_client: Com,
    capture_audio: Com,
    render_audio: Com,
    _capture_clock: Com,
    _render_clock: Com,
    capture_event: Event,
    render_event: Event,
    render_frames: u32,
    alignment: usize,
    fifo: AudioFifo,
    capture_started: bool,
    render_started: bool,
}

fn activate(device: &Com) -> Result<Com> {
    type Activate = unsafe extern "system" fn(
        *mut c_void,
        *const GUID,
        u32,
        *const c_void,
        *mut *mut c_void,
    ) -> i32;
    let call: Activate = unsafe { device.method(3) };
    let mut output = null_mut();
    let hr = unsafe { call(device.raw(), &AUDIO_CLIENT, 0x17, null(), &mut output) };
    let result = Com::from_hresult(hr, output, "IMMDevice Activate IAudioClient");
    exact(hr, "IMMDevice Activate IAudioClient")?;
    result
}
fn mix_format(client: &Com) -> Result<[u8; 40]> {
    type Format = unsafe extern "system" fn(*mut c_void, *mut *mut u8) -> i32;
    let call: Format = unsafe { client.method(8) };
    let mut output = null_mut();
    let hr = unsafe { call(client.raw(), &mut output) };
    struct Alloc(*mut u8);
    impl Drop for Alloc {
        fn drop(&mut self) {
            unsafe { CoTaskMemFree(self.0.cast()) };
        }
    }
    let allocation = Alloc(output);
    exact(hr, "IAudioClient GetMixFormat")?;
    ensure!(!allocation.0.is_null(), "GetMixFormat returned null");
    let length = 18 + unsafe { u16::from_le_bytes([*output.add(16), *output.add(17)]) } as usize;
    ensure!(
        length <= 40,
        "AudioRouter native truncated mix format has extensions beyond 40 bytes"
    );
    let mut format = [0u8; 40];
    let count = length.min(40);
    format[..count].copy_from_slice(unsafe { std::slice::from_raw_parts(output, count) });
    Ok(format)
}
fn period(client: &Com) -> Result<u64> {
    type Period = unsafe extern "system" fn(*mut c_void, *mut i64, *mut i64) -> i32;
    let call: Period = unsafe { client.method(9) };
    let mut period = 0;
    exact(
        unsafe { call(client.raw(), &mut period, null_mut()) },
        "GetDevicePeriod",
    )?;
    ensure!(period > 0, "AudioRouter device period is not positive");
    Ok(period as u64)
}
fn initialize(client: &Com, format: &[u8; 40], event: &Event) -> Result<u32> {
    type Initialize =
        unsafe extern "system" fn(*mut c_void, i32, u32, i64, i64, *const u8, *const GUID) -> i32;
    let call: Initialize = unsafe { client.method(3) };
    // Exact 0x47640 flags/duration, both endpoints use render mix format.
    exact(
        unsafe {
            call(
                client.raw(),
                0,
                0x880c0000,
                200000,
                0,
                format.as_ptr(),
                null(),
            )
        },
        "IAudioClient Initialize",
    )?;
    type SetHandle = unsafe extern "system" fn(*mut c_void, HANDLE) -> i32;
    let call: SetHandle = unsafe { client.method(13) };
    exact(
        unsafe { call(client.raw(), event.0) },
        "IAudioClient SetEventHandle",
    )?;
    type Size = unsafe extern "system" fn(*mut c_void, *mut u32) -> i32;
    let call: Size = unsafe { client.method(4) };
    let mut frames = 0;
    exact(
        unsafe { call(client.raw(), &mut frames) },
        "IAudioClient GetBufferSize",
    )?;
    Ok(frames)
}
fn service(client: &Com, iid: &GUID) -> Result<Com> {
    type Service = unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32;
    let call: Service = unsafe { client.method(14) };
    let mut output = null_mut();
    let hr = unsafe { call(client.raw(), iid, &mut output) };
    let result = Com::from_hresult(hr, output, "IAudioClient GetService");
    exact(hr, "IAudioClient GetService")?;
    result
}
fn client_call(client: &Com, slot: usize, operation: &str) -> Result<()> {
    type Call = unsafe extern "system" fn(*mut c_void) -> i32;
    let call: Call = unsafe { client.method(slot) };
    exact(unsafe { call(client.raw()) }, operation)
}
impl Pump {
    fn open(capture: Com, render: Com) -> Result<Self> {
        ensure!(
            flow(&capture)? == 1 && flow(&render)? == 0,
            "AudioRouter endpoint flow mismatch"
        );
        let capture_audio = activate(&capture)?;
        let render_audio = activate(&render)?;
        let _capture_format = mix_format(&capture_audio)?;
        let format = mix_format(&render_audio)?;
        let sample_rate = u32::from_le_bytes(format[4..8].try_into().unwrap());
        let block_align = u16::from_le_bytes(format[12..14].try_into().unwrap());
        let fifo = AudioFifo::new(
            StreamFormat {
                sample_rate,
                block_align,
            },
            period(&capture_audio)?,
            period(&render_audio)?,
        )?;
        let capture_event = Event::open()?;
        let render_event = Event::open()?;
        let _capture_frames = initialize(&capture_audio, &format, &capture_event)?;
        let render_frames = initialize(&render_audio, &format, &render_event)?;
        let capture_client = service(&capture_audio, &CAPTURE_CLIENT)?;
        let render_client = service(&render_audio, &RENDER_CLIENT)?;
        let capture_clock = service(&capture_audio, &AUDIO_CLOCK)?;
        let render_clock = service(&render_audio, &AUDIO_CLOCK)?;
        Ok(Self {
            capture_client,
            render_client,
            capture_audio,
            render_audio,
            _capture_clock: capture_clock,
            _render_clock: render_clock,
            capture_event,
            render_event,
            render_frames,
            alignment: block_align as usize,
            fifo,
            capture_started: false,
            render_started: false,
        })
    }
    fn run(&mut self, stop: HANDLE, diagnostics: &Mutex<PumpDiagnostics>) -> Result<()> {
        client_call(&self.capture_audio, 10, "capture Start")?;
        self.capture_started = true;
        client_call(&self.render_audio, 10, "render Start")?;
        self.render_started = true;
        let task = "Pro Audio"
            .encode_utf16()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let mut index = 0;
        struct Priority(HANDLE);
        impl Drop for Priority {
            fn drop(&mut self) {
                if !self.0.is_null() {
                    unsafe { AvRevertMmThreadCharacteristics(self.0) };
                }
            }
        }
        let _priority =
            Priority(unsafe { AvSetMmThreadCharacteristicsW(task.as_ptr(), &mut index) });
        let events = [stop, self.capture_event.0, self.render_event.0];
        let started = Instant::now();
        loop {
            match unsafe { WaitForMultipleObjects(3, events.as_ptr(), 0, u32::MAX) } {
                0 => break,
                1 => {
                    if let Err(error) = self.capture() {
                        let mut diagnostics = diagnostics
                            .lock()
                            .map_err(|_| anyhow::anyhow!("AudioRouter diagnostics poisoned"))?;
                        diagnostics.capture_errors += 1;
                        diagnostics.last_capture_error = Some(format!("{error:#}"));
                    }
                }
                2 => {
                    if let Err(error) = self.render(started.elapsed().as_millis() as u64) {
                        let mut diagnostics = diagnostics
                            .lock()
                            .map_err(|_| anyhow::anyhow!("AudioRouter diagnostics poisoned"))?;
                        diagnostics.render_errors += 1;
                        diagnostics.last_render_error = Some(format!("{error:#}"));
                    }
                }
                258 => break,
                u32::MAX if matches!(unsafe { GetLastError() }, 0 | 3992977411) => break,
                _ => {
                    // 0x48300 retries failed waits after rounding 20 ms to
                    // the system clock increment, rather than killing audio.
                    let (mut adjustment, mut increment, mut disabled) = (0, 0, 0);
                    let ok = unsafe {
                        GetSystemTimeAdjustment(&mut adjustment, &mut increment, &mut disabled)
                    };
                    let unit = if ok != 0 {
                        increment.saturating_add(9999) / 10000
                    } else {
                        0
                    };
                    let milliseconds = if unit == 0 {
                        20
                    } else {
                        unit - (unit + 19) % unit + 19
                    };
                    thread::sleep(Duration::from_millis(u64::from(milliseconds)));
                }
            }
        }
        Ok(())
    }
    fn capture(&mut self) -> Result<()> {
        type Next = unsafe extern "system" fn(*mut c_void, *mut u32) -> i32;
        type Get = unsafe extern "system" fn(
            *mut c_void,
            *mut *mut u8,
            *mut u32,
            *mut u32,
            *mut u64,
            *mut u64,
        ) -> i32;
        type Release = unsafe extern "system" fn(*mut c_void, u32) -> i32;
        let next: Next = unsafe { self.capture_client.method(5) };
        let get: Get = unsafe { self.capture_client.method(3) };
        let release: Release = unsafe { self.capture_client.method(4) };
        loop {
            let mut pending = 0;
            exact(
                unsafe { next(self.capture_client.raw(), &mut pending) },
                "GetNextPacketSize",
            )?;
            if pending == 0 {
                break;
            }
            let (mut data, mut frames, mut flags) = (null_mut(), 0, 0);
            let hr = unsafe {
                get(
                    self.capture_client.raw(),
                    &mut data,
                    &mut frames,
                    &mut flags,
                    null_mut(),
                    null_mut(),
                )
            };
            // Native accepts these three results. A buffer-empty result owns
            // no packet, and must not be fabricated into a successful read.
            if hr == 143196161 {
                break;
            }
            ensure!(
                hr == 0 || hr == -2004287464,
                "capture GetBuffer: 0x{:08x}",
                hr as u32
            );
            let copied = if data.is_null() && frames != 0 {
                // 0x48570 ignores flags and dereferences the data pointer.
                // Reject that unsafe native branch rather than invent PCM.
                Err(anyhow::anyhow!(
                    "AudioRouter capture has null PCM pointer (flags 0x{flags:x})"
                ))
            } else {
                let bytes = frames as usize * self.alignment;
                if bytes == 0 {
                    Ok(())
                } else {
                    self.fifo
                        .capture(unsafe { std::slice::from_raw_parts(data, bytes) })
                }
            };
            let released = exact(
                unsafe { release(self.capture_client.raw(), frames) },
                "capture ReleaseBuffer",
            );
            copied?;
            released?;
        }
        Ok(())
    }
    fn render(&mut self, elapsed_ms: u64) -> Result<()> {
        type Padding = unsafe extern "system" fn(*mut c_void, *mut u32) -> i32;
        let call: Padding = unsafe { self.render_audio.method(6) };
        let mut padding = 0;
        exact(
            unsafe { call(self.render_audio.raw(), &mut padding) },
            "GetCurrentPadding",
        )?;
        ensure!(
            padding <= self.render_frames,
            "render padding exceeds actual buffer"
        );
        let frames = self.render_frames - padding;
        if frames == 0 {
            return Ok(());
        }
        type Get = unsafe extern "system" fn(*mut c_void, u32, *mut *mut u8) -> i32;
        type Release = unsafe extern "system" fn(*mut c_void, u32, u32) -> i32;
        let get: Get = unsafe { self.render_client.method(3) };
        let release: Release = unsafe { self.render_client.method(4) };
        let mut output = null_mut();
        exact(
            unsafe { get(self.render_client.raw(), frames, &mut output) },
            "render GetBuffer",
        )?;
        let copied = (|| {
            ensure!(
                !output.is_null(),
                "render GetBuffer returned null PCM pointer"
            );
            let pcm = self.fifo.render(frames, elapsed_ms)?;
            unsafe { std::ptr::copy_nonoverlapping(pcm.as_ptr(), output, pcm.len()) };
            Ok(())
        })();
        let released = exact(
            unsafe {
                release(
                    self.render_client.raw(),
                    frames,
                    if copied.is_ok() { 0 } else { 2 },
                )
            },
            "render ReleaseBuffer",
        );
        copied?;
        released
    }
}
impl Drop for Pump {
    fn drop(&mut self) {
        if self.render_started {
            let _ = client_call(&self.render_audio, 11, "render Stop");
        }
        if self.capture_started {
            let _ = client_call(&self.capture_audio, 11, "capture Stop");
        }
    }
}
