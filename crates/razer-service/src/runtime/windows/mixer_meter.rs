//! Current RzNative 0x376c0 capture initialization, 0x32a70 timers and 0x35b60
//! peak publication. Capture is local WASAPI; samples are discarded like 0x37e10.
use super::mixer_driver;
use crate::audio_util::windows::{Com, endpoint_container, router_friendly_name};
use anyhow::{Context as _, ensure};
use razer_device::{
    audio_mixer::{MixerChannel, MixerControl, MixerSession, MixerTarget, MixerValue},
    backend::HidNode,
};
use razer_hid::with_backend;
use serde_json::{Value, json};
use std::{
    ffi::c_void,
    ptr::{null, null_mut},
    time::{Duration, Instant},
};
use windows_sys::{
    Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize},
    core::GUID,
};

const ENUMERATOR_CLASS: GUID = GUID::from_u128(0xbcde0395_e52f_467c_8e3d_c4579291692e);
const ENUMERATOR_INTERFACE: GUID = GUID::from_u128(0xa95664d2_9614_4f35_a746_de8db63617e6);
const AUDIO_CLIENT: GUID = GUID::from_u128(0x1cb9ad4c_dbfa_4c32_b178_c2f568a703b2);
const CAPTURE_CLIENT: GUID = GUID::from_u128(0xc8adbd64_e71e_48a0_a4de_185c395cd317);
fn ok(hr: i32, operation: &str) -> anyhow::Result<()> {
    ensure!(hr >= 0, "{operation}: 0x{:08x}", hr as u32);
    Ok(())
}
pub(super) struct Apartment;
impl Apartment {
    pub(super) fn open() -> anyhow::Result<Self> {
        ok(
            unsafe { CoInitializeEx(null(), 0) },
            "Mixer capture apartment",
        )?;
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

fn select(container: &str, name_filter: &str) -> anyhow::Result<Com> {
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
    let enumerator = Com::from_hresult(hr, raw, "Mixer capture enumerator")?;
    type Enumerate = unsafe extern "system" fn(*mut c_void, i32, u32, *mut *mut c_void) -> i32;
    let enumerate: Enumerate = unsafe { enumerator.method(3) };
    let mut raw = null_mut();
    let collection = Com::from_hresult(
        unsafe { enumerate(enumerator.raw(), 1, 1, &mut raw) },
        raw,
        "Mixer active capture endpoints",
    )?;
    type Count = unsafe extern "system" fn(*mut c_void, *mut u32) -> i32;
    let count: Count = unsafe { collection.method(3) };
    let mut total = 0;
    ok(
        unsafe { count(collection.raw(), &mut total) },
        "Mixer capture count",
    )?;
    type Item = unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> i32;
    let item: Item = unsafe { collection.method(4) };
    let mut selected = Vec::new();
    for index in 0..total {
        let mut raw = null_mut();
        let device = Com::from_hresult(
            unsafe { item(collection.raw(), index, &mut raw) },
            raw,
            "Mixer capture endpoint",
        )?;
        let name = router_friendly_name(&device)?;
        if name.contains("Razer Audio Mixer")
            && name.contains(name_filter)
            && endpoint_container(&device)?.eq_ignore_ascii_case(container)
        {
            selected.push(device);
        }
    }
    ensure!(
        selected.len() == 1,
        "Mic meter requires one observed Microphone endpoint belonging to the Mixer container"
    );
    Ok(selected.remove(0))
}

struct Capture {
    node: HidNode,
    container: String,
    endpoint: String,
    capture: Com,
    client: Com,
    device: Com,
    started: bool,
    // Drop apartment last, after all COM objects.
    _apartment: Apartment,
}
fn client_call(client: &Com, slot: usize, name: &str) -> anyhow::Result<()> {
    type Call = unsafe extern "system" fn(*mut c_void) -> i32;
    let call: Call = unsafe { client.method(slot) };
    ok(unsafe { call(client.raw()) }, name)
}
impl Capture {
    fn start(node: HidNode, name: &str, enabled: bool) -> anyhow::Result<Self> {
        let proof = mixer_driver::observe_node(&node, 1342)?;
        let container = proof["device_container_id"]
            .as_str()
            .context("Mixer capture container absent")?
            .to_owned();
        let apartment = Apartment::open()?;
        let device = select(&container, name)?;
        let endpoint = device.id_hresult()?;
        type Activate = unsafe extern "system" fn(
            *mut c_void,
            *const GUID,
            u32,
            *const c_void,
            *mut *mut c_void,
        ) -> i32;
        let activate: Activate = unsafe { device.method(3) };
        let mut raw = null_mut();
        let client = Com::from_hresult(
            unsafe { activate(device.raw(), &AUDIO_CLIENT, 0x17, null(), &mut raw) },
            raw,
            "Mixer IAudioClient",
        )?;
        type Format = unsafe extern "system" fn(*mut c_void, *mut *mut u8) -> i32;
        let format: Format = unsafe { client.method(8) };
        let mut raw_format = null_mut();
        struct FormatMemory(*mut u8);
        impl Drop for FormatMemory {
            fn drop(&mut self) {
                unsafe { CoTaskMemFree(self.0.cast()) };
            }
        }
        let hr = unsafe { format(client.raw(), &mut raw_format) };
        let format = FormatMemory(raw_format);
        ok(hr, "Mixer GetMixFormat")?;
        ensure!(!format.0.is_null(), "Mixer mix format absent");
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
        // Original initializes shared mode, no event/loopback flags, duration1.
        ok(
            unsafe { initialize(client.raw(), 0, 0, 1, 0, format.0, null()) },
            "Mixer capture Initialize",
        )?;
        type Size = unsafe extern "system" fn(*mut c_void, *mut u32) -> i32;
        let size: Size = unsafe { client.method(4) };
        let mut frames = 0;
        ok(
            unsafe { size(client.raw(), &mut frames) },
            "Mixer capture GetBufferSize",
        )?;
        type Service = unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32;
        let service: Service = unsafe { client.method(14) };
        let mut raw = null_mut();
        let capture = Com::from_hresult(
            unsafe { service(client.raw(), &CAPTURE_CLIENT, &mut raw) },
            raw,
            "Mixer IAudioCaptureClient",
        )?;
        let mut meter = Self {
            node,
            container,
            endpoint,
            capture,
            client,
            device,
            started: false,
            _apartment: apartment,
        };
        if enabled {
            client_call(&meter.client, 10, "Mixer capture Start")?;
            meter.started = true;
        }
        Ok(meter)
    }
    pub(super) fn identity(&self) -> Value {
        json!({"node":self.node,"product_id":1342,"endpoint_id":self.endpoint,"device_container_id":self.container,"capture_started":self.started,"capture_interval_ms":80})
    }
    fn drain(&self) -> anyhow::Result<()> {
        type Get = unsafe extern "system" fn(
            *mut c_void,
            *mut *mut u8,
            *mut u32,
            *mut u32,
            *mut u64,
            *mut u64,
        ) -> i32;
        type Release = unsafe extern "system" fn(*mut c_void, u32) -> i32;
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
        ok(hr, "Mixer microphone capture GetBuffer")?;
        if hr == 0 {
            let release: Release = unsafe { self.capture.method(4) };
            ok(
                unsafe { release(self.capture.raw(), frames) },
                "Mixer microphone capture ReleaseBuffer",
            )?;
        }
        Ok(())
    }
    pub(super) fn stop(&mut self) -> anyhow::Result<()> {
        if self.started {
            client_call(&self.client, 11, "Mixer microphone capture Stop")?;
            self.started = false;
        }
        Ok(())
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        if let Err(error) = self.stop() {
            eprintln!("Mixer microphone capture cleanup failed: {error:#}");
        }
    }
}

pub(super) struct Meter {
    node: HidNode,
    container: String,
    mixer: bool,
    captures: Vec<Capture>,
}
impl Meter {
    pub(super) fn start(node: HidNode, mixer: bool) -> anyhow::Result<Self> {
        let proof = mixer_driver::observe_node(&node, 1342)?;
        let container = proof["device_container_id"]
            .as_str()
            .context("Mixer meter container absent")?
            .to_owned();
        let mut captures = Vec::new();
        for (name, enabled) in [
            ("Microphone", !mixer),
            ("Stream Mix", mixer),
            ("Voice Chat", mixer),
        ] {
            captures.push(Capture::start(node.clone(), name, enabled)?);
        }
        Ok(Self {
            node,
            container,
            mixer,
            captures,
        })
    }
    pub(super) fn identity(&self) -> Value {
        json!({"node":self.node,"product_id":1342,"device_container_id":self.container,"capture_started":self.captures.iter().any(|c|c.started),"mixer":self.mixer,"capture_interval_ms":80,"endpoints":self.captures.iter().map(Capture::identity).collect::<Vec<_>>()})
    }
    pub(super) fn read(&self, node: &HidNode) -> anyhow::Result<Value> {
        ensure!(*node == self.node, "Mixer meter owner changed");
        for capture in &self.captures {
            ensure!(
                endpoint_container(&capture.device)?.eq_ignore_ascii_case(&self.container),
                "Mixer capture endpoint container changed"
            );
            if capture.started {
                capture.drain()?;
            }
        }
        let start = Instant::now();
        let validate = || {
            ensure!(
                start.elapsed() < Duration::from_secs(10),
                "Mixer meter sample deadline exceeded"
            );
            let proof = mixer_driver::observe_node(node, 1342)?;
            ensure!(
                proof["device_container_id"]
                    .as_str()
                    .is_some_and(|c| c.eq_ignore_ascii_case(&self.container)),
                "Mixer meter container changed"
            );
            Ok(())
        };
        validate()?;
        let mut result = self.identity();
        let mut raw = serde_json::Map::new();
        {
            let device =
                with_backend(|backend| backend.open_validated(node, std::rc::Rc::new(&validate)))?;
            let session = MixerSession::new(device.as_ref(), &validate)?;
            for (name, control) in [
                ("mic", MixerControl::MicrophonePeak),
                ("playBackMix", MixerControl::HeadphonesPeak),
                ("lineOut", MixerControl::LineOutPeak),
            ] {
                let value = session.read(&MixerTarget {
                    control,
                    band: None,
                    channel: MixerChannel::Both,
                })?;
                let MixerValue::Peak { left, right } = value else {
                    anyhow::bail!("Mixer peak response has wrong source type");
                };
                result[name] = json!({"left":razer_device::audio_mixer_local::meter_percent(left)?,"right":razer_device::audio_mixer_local::meter_percent(right)?});
                raw.insert(name.into(), json!({"left":left,"right":right}));
            }
        }
        for (name, jack) in [("streamMix", 13), ("voiceChat", 10)] {
            let peak = super::mixer_endpoint::peak(&self.container, jack)?;
            let percent = razer_device::audio_mixer_local::meter_percent(peak)?;
            result[name] = json!({"left":percent,"right":percent});
            raw.insert(name.into(), json!({"left":peak,"right":peak}));
        }
        validate()?;
        result["raw_peak"] = Value::Object(raw);
        Ok(result)
    }
    pub(super) fn stop(&mut self) -> anyhow::Result<()> {
        let mut errors = Vec::new();
        for capture in &mut self.captures {
            if let Err(e) = capture.stop() {
                errors.push(format!("{e:#}"));
            }
        }
        ensure!(
            errors.is_empty(),
            "Mixer capture cleanup: {}",
            errors.join("; ")
        );
        Ok(())
    }
}
