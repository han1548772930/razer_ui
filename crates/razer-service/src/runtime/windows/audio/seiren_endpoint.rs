//! Marie 500b0/51b60 endpoint selection and 4dc60 volume observations.
//! The observed physical container disambiguates equal model names.
use crate::audio_util::windows::{Com, endpoint_container, router_friendly_name};
use super::super::super::mixer_meter::Apartment;
use anyhow::{Result, ensure};
use razer_device::seiren_driver::Product;
use std::{ffi::c_void, ptr::{null, null_mut}, sync::{Arc, atomic::{AtomicBool, Ordering}}};
use razer_device::seiren_mixer::EndpointObservation;
use windows::{core::{Interface, implement}, Win32::Media::Audio::{AUDIO_VOLUME_NOTIFICATION_DATA, Endpoints::{IAudioEndpointVolume, IAudioEndpointVolumeCallback, IAudioEndpointVolumeCallback_Impl}}};
use windows_sys::{Win32::System::Com::CoCreateInstance, core::GUID};

fn ok(hr: i32, operation: &str) -> Result<()> {
    ensure!(hr >= 0, "{operation}: HRESULT {:#010x}", hr as u32);
    Ok(())
}

fn select(product: Product, container: &str, device_id: u32) -> Result<Com> {
    let (flow, name) = match device_id {
        262145 => (1, "Microphone"),
        262146 => (1, "Service"),
        524289 => (0, "Headphones"),
        _ => anyhow::bail!("Marie device ID {device_id} is not a Windows volume endpoint"),
    };
    let mut raw = null_mut();
    let hr = unsafe { CoCreateInstance(
        &GUID::from_u128(0xbcde0395_e52f_467c_8e3d_c4579291692e), null_mut(), 0x17,
        &GUID::from_u128(0xa95664d2_9614_4f35_a746_de8db63617e6), &mut raw) };
    let enumerator = Com::from_hresult(hr, raw, "Marie endpoint enumerator")?;
    type Enumerate = unsafe extern "system" fn(*mut c_void, i32, u32, *mut *mut c_void) -> i32;
    let enumerate: Enumerate = unsafe { enumerator.method(3) };
    let mut raw = null_mut();
    let hr = unsafe { enumerate(enumerator.raw(), flow, 1, &mut raw) };
    let collection = Com::from_hresult(hr, raw, "Marie active endpoints")?;
    type Count = unsafe extern "system" fn(*mut c_void, *mut u32) -> i32;
    let count: Count = unsafe { collection.method(3) };
    let mut total = 0;
    ok(unsafe { count(collection.raw(), &mut total) }, "Marie endpoint count")?;
    type Item = unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> i32;
    let item: Item = unsafe { collection.method(4) };
    let mut selected = Vec::new();
    for index in 0..total {
        let mut raw = null_mut();
        let hr = unsafe { item(collection.raw(), index, &mut raw) };
        let device = Com::from_hresult(hr, raw, "Marie endpoint item")?;
        let observed_name = router_friendly_name(&device)?;
        if observed_name.contains(product.endpoint_name()) && observed_name.contains(name)
            && endpoint_container(&device)?.eq_ignore_ascii_case(container) {
            selected.push(device);
        }
    }
    ensure!(selected.len() == 1, "Marie {name} endpoint is absent or ambiguous in the observed container");
    Ok(selected.remove(0))
}

#[implement(IAudioEndpointVolumeCallback)]
struct Notification { dirty: Arc<AtomicBool> }
#[allow(non_snake_case)]
impl IAudioEndpointVolumeCallback_Impl for Notification_Impl {
    fn OnNotify(&self, _: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> windows::core::Result<()> {
        // A COM notification is a wakeup, not a borrowed observation. The
        // retained owner re-reads and revalidates its exact endpoint identity.
        self.dirty.store(true, Ordering::Release);
        Ok(())
    }
}

pub(super) struct Endpoint {
    volume: IAudioEndpointVolume,
    device: Com,
    callback: Option<IAudioEndpointVolumeCallback>,
    dirty: Arc<AtomicBool>,
    product: Product,
    container: String,
    device_id: u32,
    // COM interfaces are dropped before the apartment, on this same thread.
    _apartment: Apartment,
}
impl Endpoint {
    pub(super) fn open(product: Product, container: &str, device_id: u32) -> Result<Self> {
        let apartment = Apartment::open()?;
        let device = select(product, container, device_id)?;
        type Activate = unsafe extern "system" fn(*mut c_void, *const GUID, u32, *const c_void, *mut *mut c_void) -> i32;
        let activate: Activate = unsafe { device.method(3) };
        let mut raw = null_mut();
        let hr = unsafe { activate(device.raw(), &GUID::from_u128(0x5cdf2c82_841e_4546_9722_0cf74078229a), 0x17, null(), &mut raw) };
        let raw_volume = Com::from_hresult(hr, raw, "Marie endpoint volume activation")?;
        // SAFETY: Activate returned exactly IAudioEndpointVolume. clone takes
        // its own COM reference before the temporary raw owner is released.
        let volume_pointer = raw_volume.raw();
        let volume = unsafe { IAudioEndpointVolume::from_raw_borrowed(&volume_pointer) }.expect("validated non-null COM pointer").clone();
        let dirty = Arc::new(AtomicBool::new(true));
        let callback: IAudioEndpointVolumeCallback = Notification { dirty: dirty.clone() }.into();
        unsafe { volume.RegisterControlChangeNotify(&callback) }?;
        Ok(Self { volume, device, callback: Some(callback), dirty, product, container: container.into(), device_id, _apartment: apartment })
    }
    pub(super) fn current(&self) -> Result<()> {
        let observed = select(self.product, &self.container, self.device_id)?;
        ensure!(observed.id_hresult()? == self.device.id_hresult()?, "Marie endpoint changed during observation");
        Ok(())
    }
    pub(super) fn take_dirty(&self) -> bool { self.dirty.swap(false, Ordering::AcqRel) }
    pub(super) fn observe(&self) -> Result<EndpointObservation> {
        self.current()?;
        let scalar = unsafe { self.volume.GetMasterVolumeLevelScalar() }?;
        let muted = unsafe { self.volume.GetMute() }?.as_bool();
        ensure!(scalar.is_finite() && (0.0..=1.0).contains(&scalar), "Marie endpoint returned an invalid volume scalar");
        self.current()?;
        Ok(EndpointObservation { level: (scalar * 100.0).round() as u32, muted })
    }
    pub(super) fn set_volume(&self, level: u32) -> Result<EndpointObservation> {
        ensure!(level <= 100, "Marie endpoint level exceeds source range");
        self.current()?;
        unsafe { self.volume.SetMasterVolumeLevelScalar(level as f32 / 100.0, null()) }?;
        self.observe()
    }
    pub(super) fn set_muted(&self, muted: bool) -> Result<EndpointObservation> {
        self.current()?;
        unsafe { self.volume.SetMute(muted, null()) }?;
        self.observe()
    }
    pub(super) fn close(&mut self) -> Result<()> {
        if let Some(callback) = self.callback.as_ref() {
            unsafe { self.volume.UnregisterControlChangeNotify(callback) }?;
            self.callback = None;
        }
        Ok(())
    }
}
impl Drop for Endpoint {
    fn drop(&mut self) {
        if let Err(error) = self.close() { eprintln!("[Marie endpoint cleanup] {error:#}"); }
    }
}
