//! Core Audio implementation of current simple_service endpoint volume calls.
//! Original getter reads a notification-maintained cache. This adapter observes
//! the endpoint for each request; persistent callbacks remain a separate gap.
use crate::audio_util::windows::Com;
use anyhow::{Context, Result, ensure};
use razer_device::simple_audio_volume::{
    AudioVolumeReadResult, AudioVolumeWriteResponse, AudioVolumeWriteResult, scalar_to_volume,
    volume_to_scalar,
};
use std::{
    ffi::c_void,
    fs::{File, OpenOptions},
    ptr::{null, null_mut},
};
use windows_sys::{
    Win32::System::Com::{CoCreateInstance, CoInitializeEx, CoUninitialize},
    core::GUID,
};

const ENUMERATOR_CLASS: GUID = GUID::from_u128(0xbcde0395_e52f_467c_8e3d_c4579291692e);
const ENUMERATOR_INTERFACE: GUID = GUID::from_u128(0xa95664d2_9614_4f35_a746_de8db63617e6);
const VOLUME_INTERFACE: GUID = GUID::from_u128(0x5cdf2c82_841e_4546_9722_0cf74078229a);

struct Apartment;
impl Apartment {
    fn open() -> Result<Self> {
        let hr = unsafe { CoInitializeEx(null(), 0) };
        ensure!(hr >= 0, "CoInitializeEx failed: 0x{:08x}", hr as u32);
        Ok(Self)
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

fn operation_lock(device_id: &str) -> Result<File> {
    ensure!(
        !device_id.is_empty() && !device_id.contains('\0'),
        "Audio endpoint ID is empty or contains NUL"
    );
    // Application policy, distinct from the source's service-thread queue:
    // serialize full read/mutate/read-back sequences across our workers.
    // Do not wait, or delete the persistent lock file and create another inode.
    let directory = std::env::temp_dir().join("razer-ui-audio-volume-locks");
    std::fs::create_dir_all(&directory).context("Create audio volume lock directory")?;
    let path = directory.join(format!("{:x}.lock", md5::compute(device_id.as_bytes())));
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(path)
        .context("Open audio volume operation lock")?;
    lock.try_lock()
        .context("Audio endpoint is busy in another worker or its operation lock is unavailable")?;
    Ok(lock)
}

fn endpoint(device_id: &str) -> Result<Com> {
    ensure!(
        !device_id.is_empty() && !device_id.contains('\0'),
        "Audio endpoint ID is empty or contains NUL"
    );
    let mut pointer = null_mut();
    let hr = unsafe {
        CoCreateInstance(
            &ENUMERATOR_CLASS,
            null_mut(),
            1,
            &ENUMERATOR_INTERFACE,
            &mut pointer,
        )
    };
    let enumerator = Com::from_hresult(hr, pointer, "MMDeviceEnumerator creation")?;
    type GetDevice = unsafe extern "system" fn(*mut c_void, *const u16, *mut *mut c_void) -> i32;
    let get: GetDevice = unsafe { enumerator.method(5) };
    let id = device_id.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    pointer = null_mut();
    let hr = unsafe { get(enumerator.raw(), id.as_ptr(), &mut pointer) };
    let device = Com::from_hresult(hr, pointer, "IMMDeviceEnumerator::GetDevice")?;
    ensure!(
        device.id_hresult()? == device_id,
        "Audio endpoint identity changed"
    );
    // The original cache is populated from active endpoints. Require an active
    // live endpoint rather than treating a saved or unplugged ID as observed.
    type GetState = unsafe extern "system" fn(*mut c_void, *mut u32) -> i32;
    let get_state: GetState = unsafe { device.method(6) };
    let mut state = 0;
    let hr = unsafe { get_state(device.raw(), &mut state) };
    ensure!(
        hr >= 0 && state & 1 != 0,
        "Audio endpoint is unavailable (HRESULT 0x{:08x}, state {state})",
        hr as u32
    );
    type Activate = unsafe extern "system" fn(
        *mut c_void,
        *const GUID,
        u32,
        *const c_void,
        *mut *mut c_void,
    ) -> i32;
    let activate: Activate = unsafe { device.method(3) };
    pointer = null_mut();
    let hr = unsafe { activate(device.raw(), &VOLUME_INTERFACE, 0x17, null(), &mut pointer) };
    Com::from_hresult(hr, pointer, "IMMDevice::Activate(IAudioEndpointVolume)")
}

fn observe(endpoint: &Com, device_id: &str) -> Result<AudioVolumeReadResult> {
    type GetScalar = unsafe extern "system" fn(*mut c_void, *mut f32) -> i32;
    type GetMute = unsafe extern "system" fn(*mut c_void, *mut i32) -> i32;
    let get_scalar: GetScalar = unsafe { endpoint.method(9) };
    let mut scalar = f32::NAN;
    let hr = unsafe { get_scalar(endpoint.raw(), &mut scalar) };
    ensure!(
        hr >= 0,
        "GetMasterVolumeLevelScalar failed: 0x{:08x}",
        hr as u32
    );
    ensure!(
        scalar.is_finite() && (0.0..=1.0).contains(&scalar),
        "Core Audio returned an invalid volume scalar"
    );
    let get_mute: GetMute = unsafe { endpoint.method(15) };
    let mut muted = 0;
    let hr = unsafe { get_mute(endpoint.raw(), &mut muted) };
    ensure!(hr >= 0, "GetMute failed: 0x{:08x}", hr as u32);
    Ok(AudioVolumeReadResult {
        result: true,
        reason: String::new(),
        device_id: device_id.to_owned(),
        muted: muted != 0,
        volume: scalar_to_volume(scalar),
    })
}

pub(super) fn read(device_id: &str) -> Result<AudioVolumeReadResult> {
    let _lock = operation_lock(device_id)?;
    let _apartment = Apartment::open()?;
    let endpoint = endpoint(device_id)?;
    observe(&endpoint, device_id)
}

pub(super) fn write(device_id: &str, mute: bool, volume: u8) -> Result<AudioVolumeWriteResult> {
    let _lock = operation_lock(device_id)?;
    let _apartment = Apartment::open()?;
    let endpoint = endpoint(device_id)?;
    let previous = observe(&endpoint, device_id).context("Read audio state before writing")?;
    let volume_submitted = previous.volume != volume;
    let mut mute_submitted = false;
    let mut hr = 0;
    if volume_submitted {
        type SetScalar = unsafe extern "system" fn(*mut c_void, f32, *const GUID) -> i32;
        let set: SetScalar = unsafe { endpoint.method(7) };
        hr = unsafe { set(endpoint.raw(), volume_to_scalar(volume), null()) };
    }
    // Native returns immediately after failed scalar mutation. A later mute
    // failure retains the already successful volume mutation; no rollback.
    if hr >= 0 && previous.muted != mute {
        type SetMute = unsafe extern "system" fn(*mut c_void, i32, *const GUID) -> i32;
        let set: SetMute = unsafe { endpoint.method(14) };
        mute_submitted = true;
        hr = unsafe { set(endpoint.raw(), i32::from(mute), null()) };
    }
    let (observation, observation_error) = match observe(&endpoint, device_id) {
        Ok(value) => (Some(value), None),
        Err(error) => (None, Some(format!("{error:#}"))),
    };
    Ok(AudioVolumeWriteResult {
        source_response: AudioVolumeWriteResponse {
            result: hr >= 0,
            reason: if hr >= 0 {
                String::new()
            } else {
                "Error: Failed to set volume.".into()
            },
        },
        previous,
        observation,
        observation_error,
        volume_submitted,
        mute_submitted,
        hresult: hr,
    })
}
