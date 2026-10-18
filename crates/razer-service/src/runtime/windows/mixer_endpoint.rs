//! CmMixerLib current CBD0/D290 endpoint branch. Select exact native jack type
//! through 6F90/9050/6EE0 topology; never choose an OS default audio device.
use super::{
    mixer_driver,
    mixer_meter::Apartment,
};
use crate::audio_util::windows::{Com, endpoint_container};
use anyhow::{Context as _, ensure};
use razer_device::{audio_mixer::MixerValue, backend::HidNode};
use serde_json::{Value, json};
use std::{
    ffi::c_void,
    ptr::{null, null_mut},
};
use windows_sys::{Win32::System::Com::CoCreateInstance, core::GUID};

const TOPOLOGY: GUID = GUID::from_u128(0x2a07407e_6497_4a18_9787_32f79bd0d98f);
const PART: GUID = GUID::from_u128(0xae2de0e4_5bca_4f2d_aa46_5d13f8fdb3a9);
const JACK: GUID = GUID::from_u128(0x4509f757_2d46_4637_8e62_ce7db944f57b);
const VOLUME: GUID = GUID::from_u128(0x5cdf2c82_841e_4546_9722_0cf74078229a);
const EVENT: GUID = GUID::from_u128(0xc54782e0_34fa_45c7_8b5b_cb26add0f269);
fn ok(hr: i32, name: &str) -> anyhow::Result<()> {
    ensure!(hr >= 0, "{name}: 0x{:08x}", hr as u32);
    Ok(())
}
fn activate(device: &Com, iid: &GUID) -> anyhow::Result<Com> {
    type Activate = unsafe extern "system" fn(
        *mut c_void,
        *const GUID,
        u32,
        *const c_void,
        *mut *mut c_void,
    ) -> i32;
    let call: Activate = unsafe { device.method(3) };
    let mut raw = null_mut();
    let hr = unsafe { call(device.raw(), iid, 0x17, null(), &mut raw) };
    Com::from_hresult(hr, raw, "Mixer endpoint Activate")
}
fn first_part(device: &Com) -> anyhow::Result<Com> {
    let topology = activate(device, &TOPOLOGY)?;
    type Connector = unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> i32;
    let call: Connector = unsafe { topology.method(4) };
    let mut raw = null_mut();
    let hr = unsafe { call(topology.raw(), 0, &mut raw) };
    let connector = Com::from_hresult(hr, raw, "Mixer topology connector0")?;
    type Connected = unsafe extern "system" fn(*mut c_void, *mut *mut c_void) -> i32;
    let call: Connected = unsafe { connector.method(8) };
    let mut raw = null_mut();
    let hr = unsafe { call(connector.raw(), &mut raw) };
    let connected = Com::from_hresult(hr, raw, "Mixer connected topology")?;
    type Query = unsafe extern "system" fn(*mut c_void, *const GUID, *mut *mut c_void) -> i32;
    let call: Query = unsafe { connected.method(0) };
    let mut raw = null_mut();
    let hr = unsafe { call(connected.raw(), &PART, &mut raw) };
    Com::from_hresult(hr, raw, "Mixer first IPart")
}
fn has_jack_field(part: &Com, field: usize, expected: u32) -> anyhow::Result<bool> {
    type Activate =
        unsafe extern "system" fn(*mut c_void, u32, *const GUID, *mut *mut c_void) -> i32;
    let call: Activate = unsafe { part.method(13) };
    let mut raw = null_mut();
    let hr = unsafe { call(part.raw(), 1, &JACK, &mut raw) };
    if hr < 0 {
        return Ok(false);
    }
    let jack = Com::from_hresult(hr, raw, "Mixer IKsJackDescription")?;
    type Count = unsafe extern "system" fn(*mut c_void, *mut u32) -> i32;
    let call: Count = unsafe { jack.method(3) };
    let mut count = 0;
    ok(unsafe { call(jack.raw(), &mut count) }, "Mixer jack count")?;
    type Description = unsafe extern "system" fn(*mut c_void, u32, *mut u32) -> i32;
    let call: Description = unsafe { jack.method(4) };
    for index in 0..count {
        let mut description = [0u32; 7];
        if unsafe { call(jack.raw(), index, description.as_mut_ptr()) } >= 0
            && description[field] == expected
        {
            return Ok(true);
        }
    }
    Ok(false)
}
fn jack_type(device: &Com) -> anyhow::Result<u8> {
    let part = first_part(device)?;
    type Subtype = unsafe extern "system" fn(*mut c_void, *mut GUID) -> i32;
    let call: Subtype = unsafe { part.method(7) };
    let mut subtype = GUID::default();
    ok(
        unsafe { call(part.raw(), &mut subtype) },
        "Mixer first part subtype",
    )?;
    if subtype.data2 != 0xf70f
        || subtype.data3 != 0x11d0
        || subtype.data4 != [0xb9, 0x17, 0, 0xa0, 0xc9, 0x22, 0x31, 0x96]
    {
        return Ok(0);
    }
    Ok(match subtype.data1 {
        0xdff21ce1 => {
            if has_jack_field(&part, 2, 2)? {
                12
            } else {
                1
            }
        }
        0xdff21fe3 => 3,
        0xdff21ce2 => {
            if has_jack_field(&part, 3, 2)? {
                16
            } else {
                7
            }
        }
        0xdff21be2 => 10,
        0xdff21de2 => 13,
        0xdff21be1 => {
            if has_jack_field(&part, 3, 2)? {
                15
            } else {
                2
            }
        }
        _ => 0,
    })
}
fn select(container: &str, flow: u8, jack: u8) -> anyhow::Result<Com> {
    let class = GUID::from_u128(0xbcde0395_e52f_467c_8e3d_c4579291692e);
    let iid = GUID::from_u128(0xa95664d2_9614_4f35_a746_de8db63617e6);
    let mut raw = null_mut();
    let hr = unsafe { CoCreateInstance(&class, null_mut(), 0x17, &iid, &mut raw) };
    let enumerator = Com::from_hresult(hr, raw, "Mixer endpoint enumerator")?;
    type Enumerate = unsafe extern "system" fn(*mut c_void, i32, u32, *mut *mut c_void) -> i32;
    let call: Enumerate = unsafe { enumerator.method(3) };
    let mut raw = null_mut();
    let hr = unsafe { call(enumerator.raw(), i32::from(flow), 1, &mut raw) };
    let collection = Com::from_hresult(hr, raw, "Mixer active endpoints")?;
    type Count = unsafe extern "system" fn(*mut c_void, *mut u32) -> i32;
    let call: Count = unsafe { collection.method(3) };
    let mut count = 0;
    ok(
        unsafe { call(collection.raw(), &mut count) },
        "Mixer endpoint count",
    )?;
    type Item = unsafe extern "system" fn(*mut c_void, u32, *mut *mut c_void) -> i32;
    let call: Item = unsafe { collection.method(4) };
    let mut selected = Vec::new();
    for index in 0..count {
        let mut raw = null_mut();
        let hr = unsafe { call(collection.raw(), index, &mut raw) };
        let device = Com::from_hresult(hr, raw, "Mixer endpoint item")?;
        if endpoint_container(&device).is_ok_and(|c| c.eq_ignore_ascii_case(container))
            && jack_type(&device)? == jack
        {
            selected.push(device);
        }
    }
    ensure!(
        selected.len() == 1,
        "Mixer source jack {jack}, flow {flow} is not unique in actual container"
    );
    Ok(selected.remove(0))
}
pub(super) fn write(
    node: &HidNode,
    jack: u8,
    flow: u8,
    value: &MixerValue,
) -> anyhow::Result<Value> {
    ensure!(
        matches!((jack, flow), (1 | 3 | 7 | 16, 0) | (10 | 13, 1)),
        "Mixer endpoint target outside mounted source calls"
    );
    let proof = mixer_driver::observe_node(node, 1342)?;
    let container = proof["device_container_id"]
        .as_str()
        .context("Mixer endpoint container absent")?;
    let _apartment = Apartment::open()?;
    let device = select(container, flow, jack)?;
    let volume = activate(&device, &VOLUME)?;
    let hr = match value {
        MixerValue::Scalar { value } => {
            ensure!(
                value.is_finite() && (-96. ..=0.).contains(value),
                "Mixer endpoint volume outside source range"
            );
            type Set = unsafe extern "system" fn(*mut c_void, f32, *const GUID) -> i32;
            let call: Set = unsafe { volume.method(6) };
            unsafe { call(volume.raw(), *value, &EVENT) }
        }
        MixerValue::Boolean { enabled } => {
            type Set = unsafe extern "system" fn(*mut c_void, i32, *const GUID) -> i32;
            let call: Set = unsafe { volume.method(14) };
            unsafe { call(volume.raw(), i32::from(*enabled), &EVENT) }
        }
        _ => anyhow::bail!("Mixer endpoint source setter requires volume or mute"),
    };
    ok(hr, "Mixer endpoint source write")?;
    let after = mixer_driver::observe_node(node, 1342)?;
    ensure!(
        after["device_container_id"] == proof["device_container_id"],
        "Mixer owner changed after endpoint write"
    );
    Ok(
        json!({"endpoint_id":device.id_hresult()?,"jack":jack,"flow":flow,"hresult":hr,"requested":value,"transport_completed":true}),
    )
}

/// CmMixer DE60 invokes IAudioMeterInformation::GetPeakValue once and copies
/// the same float into both native return fields for a software endpoint.
pub(super) fn peak(container: &str, jack: u8) -> anyhow::Result<f32> {
    ensure!(
        [10, 13].contains(&jack),
        "Mixer meter software jack outside source capture set"
    );
    source_peak(container, 1, jack)
}
pub(super) fn source_peak(container: &str, flow: u8, jack: u8) -> anyhow::Result<f32> {
    ensure!(
        matches!((flow, jack), (0, 1 | 3 | 7 | 16) | (1, 10 | 13)),
        "Mixer source peak endpoint outside source table"
    );
    let device = select(container, flow, jack)?;
    let meter = activate(
        &device,
        &GUID::from_u128(0xc02216f6_8c67_4b5b_9d00_d008e73e0064),
    )?;
    type Peak = unsafe extern "system" fn(*mut c_void, *mut f32) -> i32;
    let call: Peak = unsafe { meter.method(3) };
    let mut value = 0.;
    ok(
        unsafe { call(meter.raw(), &mut value) },
        "Mixer endpoint GetPeakValue",
    )?;
    ensure!(
        value.is_finite() && value >= 0.,
        "Mixer endpoint peak not finite"
    );
    Ok(value)
}
