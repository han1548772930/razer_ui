//! Mapping AudioService default eMultimedia endpoint operations (b5cd2).
use crate::audio_util::windows::Com;
use anyhow::{Result, ensure};
use razer_device::macro_runtime::{AudioAction, AudioTarget};
use std::{ffi::c_void, ptr};
use windows_sys::{
    Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize},
    core::GUID,
};

const CLASS: GUID = GUID::from_u128(0xbcde0395_e52f_467c_8e3d_c4579291692e);
const ENUMERATOR: GUID = GUID::from_u128(0xa95664d2_9614_4f35_a746_de8db63617e6);
const VOLUME: GUID = GUID::from_u128(0x5cdf2c82_841e_4546_9722_0cf74078229a);

#[derive(Clone, Debug)]
pub struct Observation {
    pub endpoint: String,
    pub scalar: f32,
    pub muted: bool,
}
#[derive(Debug, Default)]
pub struct Report {
    pub observations: Vec<Observation>,
    pub errors: Vec<String>,
}

struct Apartment;
impl Apartment {
    fn open() -> Result<Self> {
        let hr = unsafe { CoInitializeEx(ptr::null(), 2) };
        ensure!(hr >= 0, "Mapping audio apartment failed: {hr:#x}");
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

struct Endpoint {
    id: String,
    volume: Com,
}
impl Endpoint {
    fn open(enumerator: &Com, flow: i32) -> Result<Self> {
        type DefaultEndpoint =
            unsafe extern "system" fn(*mut c_void, i32, i32, *mut *mut c_void) -> i32;
        let get: DefaultEndpoint = unsafe { enumerator.method(4) };
        let mut pointer = ptr::null_mut();
        let hr = unsafe { get(enumerator.raw(), flow, 1, &mut pointer) };
        let device = Com::from_hresult(hr, pointer, "GetDefaultAudioEndpoint(eMultimedia)")?;
        let id = device.id_hresult()?;
        type Activate = unsafe extern "system" fn(
            *mut c_void,
            *const GUID,
            u32,
            *const c_void,
            *mut *mut c_void,
        ) -> i32;
        let activate: Activate = unsafe { device.method(3) };
        pointer = ptr::null_mut();
        let hr = unsafe { activate(device.raw(), &VOLUME, 23, ptr::null(), &mut pointer) };
        Ok(Self {
            id,
            volume: Com::from_hresult(hr, pointer, "Activate(IAudioEndpointVolume)")?,
        })
    }
    fn observe(&self) -> Result<Observation> {
        type Scalar = unsafe extern "system" fn(*mut c_void, *mut f32) -> i32;
        type Mute = unsafe extern "system" fn(*mut c_void, *mut i32) -> i32;
        let scalar: Scalar = unsafe { self.volume.method(9) };
        let mute: Mute = unsafe { self.volume.method(15) };
        let mut value = f32::NAN;
        let mut muted = 0;
        let hr = unsafe { scalar(self.volume.raw(), &mut value) };
        ensure!(hr >= 0, "GetMasterVolumeLevelScalar: {hr:#x}");
        ensure!(
            value.is_finite() && (0.0..=1.0).contains(&value),
            "Invalid observed volume scalar"
        );
        let hr = unsafe { mute(self.volume.raw(), &mut muted) };
        ensure!(hr >= 0, "GetMute: {hr:#x}");
        Ok(Observation {
            endpoint: self.id.clone(),
            scalar: value,
            muted: muted != 0,
        })
    }
    fn set_volume(&self, volume: u8) -> Result<()> {
        type Set = unsafe extern "system" fn(*mut c_void, f32, *const GUID) -> i32;
        let set: Set = unsafe { self.volume.method(7) };
        let hr = unsafe { set(self.volume.raw(), f32::from(volume) / 100.0, ptr::null()) };
        ensure!(hr >= 0, "SetMasterVolumeLevelScalar: {hr:#x}");
        Ok(())
    }
    fn set_mute(&self, mute: bool) -> Result<()> {
        type Set = unsafe extern "system" fn(*mut c_void, i32, *const GUID) -> i32;
        let set: Set = unsafe { self.volume.method(14) };
        let hr = unsafe { set(self.volume.raw(), i32::from(mute), ptr::null()) };
        ensure!(hr >= 0, "SetMute: {hr:#x}");
        Ok(())
    }
    fn step(&self, increase: bool) -> Result<()> {
        type Step = unsafe extern "system" fn(*mut c_void, *const GUID) -> i32;
        let step: Step = unsafe { self.volume.method(if increase { 17 } else { 18 }) };
        let hr = unsafe { step(self.volume.raw(), ptr::null()) };
        ensure!(hr >= 0, "VolumeStep: {hr:#x}");
        Ok(())
    }
}

fn retain(report: &mut Report, result: Result<()>) {
    if let Err(error) = result {
        report.errors.push(format!("{error:#}"));
    }
}

pub(super) fn execute(action: AudioAction) -> Result<Report> {
    let _apartment = Apartment::open()?;
    let mut pointer = ptr::null_mut();
    let hr = unsafe { CoCreateInstance(&CLASS, ptr::null_mut(), 1, &ENUMERATOR, &mut pointer) };
    let enumerator = Com::from_hresult(hr, pointer, "Mapping audio enumerator")?;
    let target = match action {
        AudioAction::Set { target, .. } | AudioAction::Step { target, .. } => target,
    };
    let flows: &[i32] = match target {
        AudioTarget::All => &[0, 1],
        AudioTarget::Speaker => &[0],
        AudioTarget::Microphone => &[1],
    };
    let mut report = Report::default();
    let mut endpoints = Vec::new();
    for flow in flows {
        match Endpoint::open(&enumerator, *flow) {
            Ok(endpoint) => endpoints.push(endpoint),
            Err(error) => report.errors.push(format!("Audio flow {flow}: {error:#}")),
        }
    }
    match action {
        AudioAction::Step { increase, .. } => {
            for endpoint in &endpoints {
                retain(&mut report, endpoint.step(increase));
            }
        }
        AudioAction::Set { volume, mute, .. } => {
            if let Some(volume) = volume {
                for endpoint in &endpoints {
                    retain(&mut report, endpoint.set_volume(volume));
                }
            }
            if let Some(mute) = mute {
                if mute == 2 {
                    let mut observed = Vec::new();
                    for endpoint in &endpoints {
                        match endpoint.observe() {
                            Ok(value) => observed.push((endpoint, value.muted)),
                            Err(error) => report.errors.push(format!("{error:#}")),
                        }
                    }
                    let next = observed.iter().any(|(_, muted)| !muted);
                    for (endpoint, _) in observed {
                        retain(&mut report, endpoint.set_mute(next));
                    }
                } else {
                    for endpoint in &endpoints {
                        retain(&mut report, endpoint.set_mute(mute != 0));
                    }
                }
            }
        }
    }
    // Read the actual endpoint after all submitted operations. A partial write
    // retains both this observation and the failed HRESULT, without rollback.
    for endpoint in &endpoints {
        match endpoint.observe() {
            Ok(value) => report.observations.push(value),
            Err(error) => report.errors.push(format!("{error:#}")),
        }
    }
    Ok(report)
}
