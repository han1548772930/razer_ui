//! OS-owned PCM for the current Seiren EQ6007 Web Audio input hook.
//! The worker owns every COM interface, capture packet and apartment. Native
//! firmware peak meters are a separate chain and are never inferred from PCM.
use crate::{
    audio_util::windows::{Com, router_friendly_name},
    runtime::seiren_audio_capture_controller::Canceled,
};
use anyhow::ensure;
use razer_device::seiren_audio_capture::{CaptureFrame, PcmFormat};
use razer_device::seiren_microphone::{CaptureOptions, PcmPlaybackFile};
use std::{
    ffi::c_void,
    ptr::{null, null_mut},
    sync::atomic::{AtomicBool, Ordering},
    thread,
    time::Duration,
};
use windows_sys::{
    Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize},
    core::GUID,
};

const ENUMERATOR_CLASS: GUID = GUID::from_u128(0xbcde0395_e52f_467c_8e3d_c4579291692e);
const ENUMERATOR_INTERFACE: GUID = GUID::from_u128(0xa95664d2_9614_4f35_a746_de8db63617e6);
const AUDIO_CLIENT: GUID = GUID::from_u128(0x1cb9ad4c_dbfa_4c32_b178_c2f568a703b2);
const AUDIO_CLIENT2: GUID = GUID::from_u128(0x726778cd_f60a_4eda_82de_e47610cd78aa);
const CAPTURE_CLIENT: GUID = GUID::from_u128(0xc8adbd64_e71e_48a0_a4de_185c395cd317);
const BUFFER_EMPTY: i32 = 0x0889_0001;
const RAW_MODE_UNSUPPORTED: i32 = 0x8889_0027u32 as i32;
const MAX_PACKET_BYTES: usize = 16 * 1024 * 1024;

fn check(canceled: &AtomicBool) -> anyhow::Result<()> {
    if canceled.load(Ordering::Acquire) {
        return Err(Canceled.into());
    }
    Ok(())
}

fn ok(hr: i32, operation: &str) -> anyhow::Result<()> {
    ensure!(hr >= 0, "{operation}: 0x{:08x}", hr as u32);
    Ok(())
}

struct Apartment;
impl Apartment {
    fn open() -> anyhow::Result<Self> {
        ok(
            unsafe { CoInitializeEx(null(), 0) },
            "Seiren capture CoInitializeEx",
        )?;
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

struct FormatMemory(*mut u8);
impl FormatMemory {
    fn read(client: &Com) -> anyhow::Result<Self> {
        type Get = unsafe extern "system" fn(*mut c_void, *mut *mut u8) -> i32;
        let get: Get = unsafe { client.method(8) };
        let mut raw = null_mut();
        let hr = unsafe { get(client.raw(), &mut raw) };
        let format = Self(raw);
        ok(hr, "Seiren capture IAudioClient::GetMixFormat")?;
        ensure!(!format.0.is_null(), "Seiren capture mix format is absent");
        Ok(format)
    }
    fn bytes(&self) -> &[u8] {
        // GetMixFormat allocates a complete WAVEFORMATEX plus cbSize bytes.
        // Read packed fields as bytes, avoiding an aligned struct reference.
        let base = unsafe { std::slice::from_raw_parts(self.0, 18) };
        let extra = usize::from(u16::from_le_bytes([base[16], base[17]]));
        unsafe { std::slice::from_raw_parts(self.0, 18 + extra) }
    }
}
impl Drop for FormatMemory {
    fn drop(&mut self) {
        unsafe { CoTaskMemFree(self.0.cast()) };
    }
}

fn enumerator() -> anyhow::Result<Com> {
    let mut raw = null_mut();
    let hr = unsafe {
        CoCreateInstance(
            &ENUMERATOR_CLASS,
            null_mut(),
            0x17,
            &ENUMERATOR_INTERFACE,
            &mut raw,
        )
    };
    Com::from_hresult(hr, raw, "Seiren capture IMMDeviceEnumerator")
}

fn default_device(enumerator: &Com, flow: i32) -> anyhow::Result<Com> {
    type Get = unsafe extern "system" fn(*mut c_void, i32, i32, *mut *mut c_void) -> i32;
    let get: Get = unsafe { enumerator.method(4) };
    let mut raw = null_mut();
    // The source selects deviceId "default", rather than "communications".
    let hr = unsafe { get(enumerator.raw(), flow, 0, &mut raw) };
    Com::from_hresult(hr, raw, "Seiren capture default console endpoint")
}

fn activate(device: &Com) -> anyhow::Result<Com> {
    type Activate = unsafe extern "system" fn(
        *mut c_void,
        *const GUID,
        u32,
        *const c_void,
        *mut *mut c_void,
    ) -> i32;
    let call: Activate = unsafe { device.method(3) };
    let mut raw = null_mut();
    let hr = unsafe { call(device.raw(), &AUDIO_CLIENT, 0x17, null(), &mut raw) };
    Com::from_hresult(hr, raw, "Seiren capture IMMDevice::Activate(IAudioClient)")
}

fn select_input(
    enumerator: &Com,
    canceled: &AtomicBool,
    diagnostics: &mut Vec<String>,
    options: &CaptureOptions,
) -> anyhow::Result<(Com, String, bool)> {
    type Enumerate = unsafe extern "system" fn(*mut c_void, i32, u32, *mut *mut c_void) -> i32;
    let enumerate: Enumerate = unsafe { enumerator.method(3) };
    let mut raw = null_mut();
    let hr = unsafe { enumerate(enumerator.raw(), 1, 1, &mut raw) };
    let collection = Com::from_hresult(hr, raw, "Seiren active capture endpoints")?;
    type Count = unsafe extern "system" fn(*mut c_void, *mut u32) -> i32;
    let count: Count = unsafe { collection.method(3) };
    let mut total = 0;
    ok(
        unsafe { count(collection.raw(), &mut total) },
        "Seiren capture endpoint count",
    )?;
    ensure!(total <= 4096, "Seiren capture endpoint count exceeds limit");
    type Item = unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> i32;
    let item: Item = unsafe { collection.method(4) };
    let mut inputs = Vec::new();
    for index in 0..total {
        check(canceled)?;
        let observed = (|| -> anyhow::Result<(Com, String)> {
            let mut raw = null_mut();
            let hr = unsafe { item(collection.raw(), index, &mut raw) };
            let device = Com::from_hresult(hr, raw, "Seiren capture endpoint")?;
            let label = router_friendly_name(&device)?;
            Ok((device, label))
        })();
        match observed {
            Ok(input) => inputs.push(input),
            Err(error) => diagnostics.push(format!(
                "Seiren capture input {index} could not be observed: {error:#}"
            )),
        }
    }
    let labels = inputs
        .iter()
        .map(|(_, label)| label.as_str())
        .collect::<Vec<_>>();
    let default = default_device(enumerator, 1).ok();
    let default_id = default.as_ref().and_then(|device| device.id_hresult().ok());
    let default_index = default_id.as_ref().and_then(|id| {
        inputs
            .iter()
            .position(|(device, _)| device.id_hresult().is_ok_and(|value| value == *id))
    });
    if let Some(index) = options.select_endpoint(&labels, default_index) {
        let (device, label) = inputs.remove(index);
        return Ok((device, label, false));
    }
    check(canceled)?;
    let device = default_device(enumerator, 1)?;
    let label = router_friendly_name(&device)?;
    Ok((device, label, true))
}

fn output_sample_rate(enumerator: &Com) -> anyhow::Result<u32> {
    // A Web Audio AudioContext without a sampleRate option uses the default
    // render endpoint. Query that format without starting a playback stream.
    let device = default_device(enumerator, 0)?;
    let client = activate(&device)?;
    let format = FormatMemory::read(&client)?;
    let rate = u32::from_le_bytes(format.bytes()[4..8].try_into().unwrap());
    ensure!(rate > 0, "Seiren default output sample rate is zero");
    Ok(rate)
}

fn request_unprocessed(client: &Com) -> anyhow::Result<()> {
    type Query = unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32;
    let query: Query = unsafe { client.method(0) };
    let mut raw = null_mut();
    let hr = unsafe { query(client.raw(), &AUDIO_CLIENT2, &mut raw) };
    let client2 = Com::from_hresult(hr, raw, "Seiren capture IAudioClient2")?;
    // SDK AudioClientProperties: BOOL and both enum values are 32-bit. RAW
    // requests endpoint capture without OS signal processing; this adapter
    // also performs no software echo/noise suppression or automatic gain.
    #[repr(C)]
    struct Properties {
        size: u32,
        offload: i32,
        category: i32,
        options: i32,
    }
    type Set = unsafe extern "system" fn(*mut c_void, *const Properties) -> i32;
    let set: Set = unsafe { client2.method(16) };
    let properties = Properties {
        size: std::mem::size_of::<Properties>() as u32,
        offload: 0,
        category: 0,
        options: 1,
    };
    ok(
        unsafe { set(client2.raw(), &properties) },
        "Seiren capture SetClientProperties(RAW)",
    )
}

fn initialize_bytes(client: &Com, format: &[u8], flags: u32) -> i32 {
    type Initialize =
        unsafe extern "system" fn(*mut c_void, i32, u32, i64, i64, *const u8, *const GUID) -> i32;
    let initialize: Initialize = unsafe { client.method(3) };
    // Shared input, no loopback/playback, 100ms packet capacity. The worker
    // drains every 5ms; this is a local buffer limit, not a Razer constant.
    unsafe {
        initialize(
            client.raw(),
            0,
            flags,
            1_000_000,
            0,
            format.as_ptr(),
            null(),
        )
    }
}

fn input_client(
    device: &Com,
    diagnostics: &mut Vec<String>,
    options: &CaptureOptions,
) -> anyhow::Result<(Com, PcmFormat)> {
    let mut client = activate(device)?;
    let raw_requested = match request_unprocessed(&client) {
        Ok(()) => true,
        Err(error) => {
            diagnostics.push(format!(
                "Seiren unprocessed OS input request was unavailable: {error:#}"
            ));
            // A failed property call cannot leave an unknown configuration in
            // the retained client. Keep any actual endpoint processing visible
            // as a diagnostic, rather than claim that RAW was accepted.
            client = activate(device)?;
            false
        }
    };
    let mut memory = FormatMemory::read(&client)?;
    let mut format = PcmFormat::parse(memory.bytes())?;
    if let Some(rate) = options.sample_rate() {
        let mut preferred = [0u8; 18];
        preferred[..2].copy_from_slice(&3u16.to_le_bytes());
        preferred[2..4].copy_from_slice(&1u16.to_le_bytes());
        preferred[4..8].copy_from_slice(&rate.to_le_bytes());
        preferred[8..12].copy_from_slice(&(rate * 4).to_le_bytes());
        preferred[12..14].copy_from_slice(&4u16.to_le_bytes());
        preferred[14..16].copy_from_slice(&32u16.to_le_bytes());
        let hr = initialize_bytes(&client, &preferred, 0x8800_0000);
        if hr >= 0 {
            return Ok((client, PcmFormat::parse(&preferred)?));
        }
        diagnostics.push(format!("Seiren ideal mono {rate}Hz capture was declined (0x{:08x}); observing actual shared PCM",hr as u32));
        client = activate(device)?;
        if raw_requested {
            if let Err(error) = request_unprocessed(&client) {
                diagnostics.push(format!("Seiren RAW retry was unavailable: {error:#}"));
                client = activate(device)?;
            }
        }
        memory = FormatMemory::read(&client)?;
        format = PcmFormat::parse(memory.bytes())?;
    }
    let mut hr = initialize_bytes(&client, memory.bytes(), 0);
    if raw_requested && hr == RAW_MODE_UNSUPPORTED {
        diagnostics.push("Seiren endpoint rejected RAW initialization; using its shared input format without software capture effects".into());
        client = activate(device)?;
        memory = FormatMemory::read(&client)?;
        format = PcmFormat::parse(memory.bytes())?;
        hr = initialize_bytes(&client, memory.bytes(), 0);
    }
    ok(hr, "Seiren capture IAudioClient::Initialize")?;
    Ok((client, format))
}

fn client_call(client: &Com, slot: usize, operation: &str) -> anyhow::Result<()> {
    type Call = unsafe extern "system" fn(*mut c_void) -> i32;
    let call: Call = unsafe { client.method(slot) };
    ok(unsafe { call(client.raw()) }, operation)
}

struct Capture {
    capture: Com,
    client: Com,
    _device: Com,
    format: PcmFormat,
    buffer_frames: u32,
    started: bool,
}
impl Capture {
    fn open(
        device: Com,
        diagnostics: &mut Vec<String>,
        options: &CaptureOptions,
    ) -> anyhow::Result<Self> {
        let (client, format) = input_client(&device, diagnostics, options)?;
        type Size = unsafe extern "system" fn(*mut c_void, *mut u32) -> i32;
        let size: Size = unsafe { client.method(4) };
        let mut buffer_frames = 0;
        ok(
            unsafe { size(client.raw(), &mut buffer_frames) },
            "Seiren capture GetBufferSize",
        )?;
        ensure!(
            buffer_frames > 0,
            "Seiren capture buffer contains no frames"
        );
        ensure!(
            format.bytes_for_frames(buffer_frames as usize)? <= MAX_PACKET_BYTES,
            "Seiren capture buffer exceeds the PCM safety limit"
        );
        type Service = unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32;
        let service: Service = unsafe { client.method(14) };
        let mut raw = null_mut();
        let hr = unsafe { service(client.raw(), &CAPTURE_CLIENT, &mut raw) };
        let capture = Com::from_hresult(hr, raw, "Seiren IAudioCaptureClient")?;
        Ok(Self {
            capture,
            client,
            _device: device,
            format,
            buffer_frames,
            started: false,
        })
    }
    fn start(&mut self) -> anyhow::Result<()> {
        client_call(&self.client, 10, "Seiren capture Start")?;
        self.started = true;
        Ok(())
    }
    fn packet(&self) -> anyhow::Result<Option<(Vec<f32>, Vec<String>)>> {
        type Next = unsafe extern "system" fn(*mut c_void, *mut u32) -> i32;
        let next: Next = unsafe { self.capture.method(5) };
        let mut available = 0;
        ok(
            unsafe { next(self.capture.raw(), &mut available) },
            "Seiren capture GetNextPacketSize",
        )?;
        if available == 0 {
            return Ok(None);
        }
        type Get = unsafe extern "system" fn(
            *mut c_void,
            *mut *mut u8,
            *mut u32,
            *mut u32,
            *mut u64,
            *mut u64,
        ) -> i32;
        let get: Get = unsafe { self.capture.method(3) };
        let mut data = null_mut();
        let mut frames = 0;
        let mut flags = 0;
        let hr = unsafe {
            get(
                self.capture.raw(),
                &mut data,
                &mut frames,
                &mut flags,
                null_mut(),
                null_mut(),
            )
        };
        ok(hr, "Seiren capture GetBuffer")?;
        if hr == BUFFER_EMPTY {
            return Ok(None);
        }
        // Every successful GetBuffer owns one packet, even when decoding fails.
        let decoded = (|| -> anyhow::Result<(Vec<f32>, Vec<String>)> {
            ensure!(
                frames <= self.buffer_frames,
                "Seiren capture packet exceeds its buffer"
            );
            let length = self.format.bytes_for_frames(frames as usize)?;
            ensure!(
                length <= MAX_PACKET_BYTES,
                "Seiren PCM packet exceeds safety limit"
            );
            let silent = flags & 2 != 0;
            let bytes = if silent || length == 0 {
                &[][..]
            } else {
                ensure!(!data.is_null(), "Seiren capture PCM pointer is null");
                unsafe { std::slice::from_raw_parts(data, length) }
            };
            let pcm = self.format.mono(bytes, frames as usize, silent)?;
            let mut diagnostics = Vec::new();
            if flags & 1 != 0 {
                diagnostics.push("Seiren OS capture reported a PCM discontinuity".into());
            }
            if flags & 4 != 0 {
                diagnostics.push("Seiren OS capture reported an invalid packet timestamp".into());
            }
            Ok((pcm, diagnostics))
        })();
        type Release = unsafe extern "system" fn(*mut c_void, u32) -> i32;
        let release: Release = unsafe { self.capture.method(4) };
        let released = ok(
            unsafe { release(self.capture.raw(), frames) },
            "Seiren capture ReleaseBuffer",
        );
        match (decoded, released) {
            (Ok(packet), Ok(())) => Ok(Some(packet)),
            (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
            (Err(error), Err(cleanup)) => {
                Err(error.context(format!("Seiren packet cleanup also failed: {cleanup:#}")))
            }
        }
    }
    fn stop(&mut self) -> anyhow::Result<()> {
        if self.started {
            client_call(&self.client, 11, "Seiren capture Stop")?;
            self.started = false;
        }
        Ok(())
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        if let Err(error) = self.stop() {
            eprintln!("Seiren audio capture cleanup failed: {error:#}");
        }
    }
}

pub(super) fn run(
    canceled: &AtomicBool,
    options: &CaptureOptions,
    publish: &mut dyn FnMut(CaptureFrame) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    check(canceled)?;
    // This guard is declared before every COM owner and is dropped last.
    let _apartment = Apartment::open()?;
    let enumerator = enumerator()?;
    let mut diagnostics = Vec::new();
    let output_sample_rate = match output_sample_rate(&enumerator) {
        Ok(rate) => Some(rate),
        Err(error) => {
            diagnostics.push(format!(
                "Seiren default output format was not observed: {error:#}"
            ));
            None
        }
    };
    check(canceled)?;
    options.validate()?;
    let (device, label, default_fallback) =
        select_input(&enumerator, canceled, &mut diagnostics, options)?;
    let endpoint_id = device.id_hresult()?;
    check(canceled)?;
    let mut capture = Capture::open(device, &mut diagnostics, options)?;
    if let Some(requested) = options
        .sample_rate()
        .filter(|rate| *rate != capture.format.sample_rate)
    {
        diagnostics.push(format!(
            "Source requested {requested}Hz capture; endpoint supplied {}Hz actual PCM",
            capture.format.sample_rate
        ));
    }
    check(canceled)?;
    capture.start()?;
    let mut metadata = CaptureFrame {
        output_sample_rate,
        capture_sample_rate: capture.format.sample_rate,
        mono_pcm: Vec::new(),
        microphone_levels: None,
        headphone_levels: None,
        endpoint_id,
        label,
        default_fallback,
        diagnostics,
    };
    let result = (|| -> anyhow::Result<()> {
        check(canceled)?;
        // Initialization is observed only after the real stream Start succeeds.
        publish(metadata.clone())?;
        metadata.diagnostics.clear();
        loop {
            check(canceled)?;
            loop {
                check(canceled)?;
                let Some((mono_pcm, diagnostics)) = capture.packet()? else {
                    break;
                };
                let mut frame = metadata.clone();
                frame.mono_pcm = mono_pcm;
                frame.diagnostics = diagnostics;
                publish(frame)?;
            }
            // Cancellation never waits for an audio packet or a UI poll.
            thread::park_timeout(Duration::from_millis(5));
        }
    })();
    let stopped = capture.stop().and_then(|()| {
        if !matches!(
            options.purpose(),
            razer_device::seiren_microphone::CapturePurpose::Recording
                | razer_device::seiren_microphone::CapturePurpose::Speech
                | razer_device::seiren_microphone::CapturePurpose::Environment
        ) {
            return Ok(());
        }
        // Stop freezes the endpoint buffer. Deliver its remaining actual PCM
        // before the controller joins and acknowledges normal Record Stop.
        // Lifecycle-canceled page owners reject this final drain by generation.
        let mut drained = 0usize;
        for _ in 0..1024 {
            let Some((mono_pcm, diagnostics)) = capture.packet()? else {
                return Ok(());
            };
            drained = drained.saturating_add(mono_pcm.len());
            ensure!(
                drained <= capture.buffer_frames as usize,
                "Seiren stopped capture drain exceeded its owned buffer"
            );
            let mut frame = metadata.clone();
            frame.mono_pcm = mono_pcm;
            frame.diagnostics = diagnostics;
            publish(frame)?;
        }
        anyhow::bail!("Seiren stopped capture did not finish its buffered PCM drain")
    });
    match (result, stopped) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(error), Err(cleanup)) => {
            // Preserve cleanup errors even if the primary result is the normal
            // typed cancellation marker consumed by the portable controller.
            anyhow::bail!("{error:#}; Seiren capture cleanup also failed: {cleanup:#}")
        }
    }
}

const RENDER_CLIENT: GUID = GUID::from_u128(0xf294acfc_3146_4483_a7bf_addca7c260e2);
fn render_device(enumerator: &Com, canceled: &AtomicBool) -> anyhow::Result<Com> {
    type Enumerate = unsafe extern "system" fn(*mut c_void, i32, u32, *mut *mut c_void) -> i32;
    let enumerate: Enumerate = unsafe { enumerator.method(3) };
    let mut raw = null_mut();
    let hr = unsafe { enumerate(enumerator.raw(), 0, 1, &mut raw) };
    let collection = Com::from_hresult(hr, raw, "Seiren active playback endpoints")?;
    type Count = unsafe extern "system" fn(*mut c_void, *mut u32) -> i32;
    let count: Count = unsafe { collection.method(3) };
    let mut total = 0;
    ok(
        unsafe { count(collection.raw(), &mut total) },
        "Seiren render endpoint count",
    )?;
    ensure!(total <= 4096, "Seiren render endpoint count exceeds limit");
    type Item = unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> i32;
    let item: Item = unsafe { collection.method(4) };
    for ix in 0..total {
        check(canceled)?;
        let mut raw = null_mut();
        let device = Com::from_hresult(
            unsafe { item(collection.raw(), ix, &mut raw) },
            raw,
            "Seiren render endpoint",
        )?;
        if router_friendly_name(&device)?.contains("Service (Do not use)") {
            return Ok(device);
        }
    }
    default_device(enumerator, 0)
}

struct Playback {
    client: Com,
    render: Com,
    _device: Com,
    frames: u32,
    started: bool,
}
impl Playback {
    fn open(device: Com, rate: u32) -> anyhow::Result<Self> {
        let client = activate(&device)?;
        // Browser Audio converts its real decoded mono PCM into the selected
        // sink's mix format. WASAPI shared AUTOCONVERTPCM performs that same OS
        // channel/rate conversion; no vendor success is inferred from it.
        let mut format = [0u8; 18];
        format[..2].copy_from_slice(&3u16.to_le_bytes());
        format[2..4].copy_from_slice(&1u16.to_le_bytes());
        format[4..8].copy_from_slice(&rate.to_le_bytes());
        format[8..12].copy_from_slice(
            &rate
                .checked_mul(4)
                .ok_or_else(|| anyhow::anyhow!("Playback byte rate overflow"))?
                .to_le_bytes(),
        );
        format[12..14].copy_from_slice(&4u16.to_le_bytes());
        format[14..16].copy_from_slice(&32u16.to_le_bytes());
        type Initialize = unsafe extern "system" fn(
            *mut c_void,
            i32,
            u32,
            i64,
            i64,
            *const u8,
            *const GUID,
        ) -> i32;
        let initialize: Initialize = unsafe { client.method(3) };
        ok(
            unsafe {
                initialize(
                    client.raw(),
                    0,
                    0x8800_0000,
                    1_000_000,
                    0,
                    format.as_ptr(),
                    null(),
                )
            },
            "Seiren playback Initialize",
        )?;
        type Size = unsafe extern "system" fn(*mut c_void, *mut u32) -> i32;
        let size: Size = unsafe { client.method(4) };
        let mut frames = 0;
        ok(
            unsafe { size(client.raw(), &mut frames) },
            "Seiren playback GetBufferSize",
        )?;
        ensure!(
            frames > 0 && frames <= 4_194_304,
            "Seiren playback buffer exceeds limits"
        );
        type Service = unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32;
        let service: Service = unsafe { client.method(14) };
        let mut raw = null_mut();
        let hr = unsafe { service(client.raw(), &RENDER_CLIENT, &mut raw) };
        let render = Com::from_hresult(hr, raw, "Seiren IAudioRenderClient")?;
        Ok(Self {
            client,
            render,
            _device: device,
            frames,
            started: false,
        })
    }
    fn padding(&self) -> anyhow::Result<u32> {
        type Get = unsafe extern "system" fn(*mut c_void, *mut u32) -> i32;
        let get: Get = unsafe { self.client.method(6) };
        let mut padding = 0;
        ok(
            unsafe { get(self.client.raw(), &mut padding) },
            "Seiren playback GetCurrentPadding",
        )?;
        ensure!(
            padding <= self.frames,
            "Seiren playback padding exceeds buffer"
        );
        Ok(padding)
    }
    fn write(&self, samples: &[f32]) -> anyhow::Result<()> {
        type Get = unsafe extern "system" fn(*mut c_void, u32, *mut *mut u8) -> i32;
        type Release = unsafe extern "system" fn(*mut c_void, u32, u32) -> i32;
        let get: Get = unsafe { self.render.method(3) };
        let release: Release = unsafe { self.render.method(4) };
        let mut raw = null_mut();
        ok(
            unsafe { get(self.render.raw(), samples.len() as u32, &mut raw) },
            "Seiren playback GetBuffer",
        )?;
        // Every acquired render buffer is released even if its pointer is invalid.
        let result = if raw.is_null() {
            Err(anyhow::anyhow!("Seiren render buffer is null"))
        } else {
            for (ix, sample) in samples.iter().enumerate() {
                unsafe { std::ptr::write_unaligned(raw.add(ix * 4).cast::<f32>(), *sample) }
            }
            Ok(())
        };
        let cleanup = ok(
            unsafe {
                release(
                    self.render.raw(),
                    samples.len() as u32,
                    if result.is_ok() { 0 } else { 2 },
                )
            },
            "Seiren playback ReleaseBuffer",
        );
        result.and(cleanup)
    }
    fn stop(&mut self) -> anyhow::Result<()> {
        if self.started {
            client_call(&self.client, 11, "Seiren playback Stop")?;
            self.started = false;
        }
        Ok(())
    }
}
impl Drop for Playback {
    fn drop(&mut self) {
        if let Err(error) = self.stop() {
            eprintln!("Seiren render cleanup failed: {error:#}");
        }
    }
}

pub(super) fn playback(
    canceled: &AtomicBool,
    clip: &PcmPlaybackFile,
    started: &mut dyn FnMut() -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    use std::io::Read;
    check(canceled)?;
    let _apartment = Apartment::open()?;
    let mut source = std::io::BufReader::new(std::fs::File::open(clip.path())?);
    ensure!(
        source.get_ref().metadata()?.len()
            == clip
                .sample_count()
                .checked_mul(4)
                .ok_or_else(|| anyhow::anyhow!("Playback size overflow"))?,
        "Local microphone recording byte count changed"
    );
    let enumeration = enumerator()?;
    let device = render_device(&enumeration, canceled)?;
    let mut playback = Playback::open(device, clip.sample_rate())?;
    let mut remaining = clip.sample_count();
    let result = (|| -> anyhow::Result<()> {
        loop {
            check(canceled)?;
            let padding = playback.padding()?;
            let available = playback.frames - padding;
            if remaining > 0 && available > 0 {
                let count = remaining.min(u64::from(available)) as usize;
                let mut bytes = vec![0; count * 4];
                source.read_exact(&mut bytes)?;
                let samples = bytes
                    .chunks_exact(4)
                    .map(|bytes| f32::from_le_bytes(bytes.try_into().unwrap()))
                    .collect::<Vec<_>>();
                ensure!(
                    samples.iter().all(|sample| sample.is_finite()),
                    "Recorded microphone PCM is non-finite"
                );
                playback.write(&samples)?;
                remaining -= count as u64;
                if !playback.started {
                    check(canceled)?;
                    client_call(&playback.client, 10, "Seiren playback Start")?;
                    playback.started = true;
                    started()?;
                }
            } else if remaining == 0 && padding == 0 {
                return Ok(());
            }
            thread::park_timeout(Duration::from_millis(5));
        }
    })();
    let stopped = playback.stop();
    match (result, stopped) {
        (Err(error), Err(cleanup)) => {
            Err(error.context(format!("Playback cleanup failed: {cleanup:#}")))
        }
        (Err(error), _) | (_, Err(error)) => Err(error),
        _ => Ok(()),
    }
}
