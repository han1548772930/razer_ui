//! Current SDK IGCL chain: 0x124f20, 0x124ab0, 0x124480 and 0x123250.
use super::{Handle, Library, Readings, status};
use crate::hardware_monitor::{GraphicsReading, PciAddress, measured};
use anyhow::ensure;
use std::{mem::size_of, ptr};

#[repr(C)]
struct Header {
    size: u32,
    version: u8,
    reserved: [u8; 3],
}
impl Header {
    fn of<T>() -> Self {
        Self {
            size: size_of::<T>() as u32,
            version: 0,
            reserved: [0; 3],
        }
    }
}
#[repr(C)]
struct Init {
    header: Header,
    app_version: u32,
    flags: u32,
    supported_version: u32,
    application_id: [u32; 4],
}
#[repr(C)]
struct DeviceProperties {
    header: Header,
    os_id: *mut u64,
    os_id_size: u32,
    device_type: u32,
    supported_functions: u32,
    driver_version: u64,
    firmware_version: [u64; 3],
    vendor: u32,
    device: u32,
    revision: u32,
    eus_per_subslice: u32,
    subslices_per_slice: u32,
    slices: u32,
    name: [u8; 100],
    graphics_properties: u32,
    reserved: [u8; 128],
}
#[repr(C)]
struct Address {
    header: Header,
    domain: u32,
    bus: u32,
    device: u32,
    function: u32,
}
#[repr(C)]
struct Speed {
    header: Header,
    generation: i32,
    width: i32,
    bandwidth: i64,
}
#[repr(C)]
struct PciProperties {
    header: Header,
    address: Address,
    speed: Speed,
    bar_supported: u8,
    bar_enabled: u8,
}
#[repr(C)]
struct TemperatureProperties {
    header: Header,
    kind: u32,
    maximum: f64,
}
#[repr(C)]
struct FrequencyProperties {
    header: Header,
    kind: u32,
    control: u8,
    minimum: f64,
    maximum: f64,
}
#[repr(C)]
struct FrequencyState {
    header: Header,
    voltage: f64,
    request: f64,
    tdp: f64,
    efficient: f64,
    actual: f64,
    throttle_reasons: u32,
}

type Initialize = unsafe extern "C" fn(*mut Init, *mut Handle) -> i32;
type Close = unsafe extern "C" fn(Handle) -> i32;
type Enumerate = unsafe extern "C" fn(Handle, *mut u32, *mut Handle) -> i32;
type DeviceRead = unsafe extern "C" fn(Handle, *mut DeviceProperties) -> i32;
type PciRead = unsafe extern "C" fn(Handle, *mut PciProperties) -> i32;
type TemperatureProps = unsafe extern "C" fn(Handle, *mut TemperatureProperties) -> i32;
type TemperatureState = unsafe extern "C" fn(Handle, *mut f64) -> i32;
type FrequencyProps = unsafe extern "C" fn(Handle, *mut FrequencyProperties) -> i32;
type FrequencyRead = unsafe extern "C" fn(Handle, *mut FrequencyState) -> i32;

struct Adapter {
    handle: Handle,
    address: PciAddress,
    vendor: u32,
    device: u32,
    name: String,
}

pub(super) struct Intel {
    context: Handle,
    close_api: Close,
    temperatures: Enumerate,
    temperature_properties: TemperatureProps,
    temperature_state: TemperatureState,
    frequencies: Enumerate,
    frequency_properties: FrequencyProps,
    frequency_state: FrequencyRead,
    adapters: Vec<Adapter>,
    _library: Library,
}

/// All involved version-zero structures contain only integers, raw pointers and
/// floating values. Zero is their specified initialization representation.
fn zero<T>() -> T {
    // SAFETY: kept private and called only for the repr(C) output records above.
    unsafe { std::mem::zeroed() }
}

fn enumerate(api: Enumerate, parent: Handle, label: &str) -> anyhow::Result<Vec<Handle>> {
    let mut count = 0;
    // SAFETY: live owner and documented two-call enumeration ABI.
    status(unsafe { api(parent, &mut count, ptr::null_mut()) }, label)?;
    ensure!(count <= 4096, "{label} returned an excessive count");
    if count == 0 {
        return Ok(Vec::new());
    }
    let mut values = vec![ptr::null_mut(); count as usize];
    status(
        unsafe { api(parent, &mut count, values.as_mut_ptr()) },
        label,
    )?;
    ensure!(
        count as usize <= values.len(),
        "{label} changed beyond the allocated count"
    );
    values.truncate(count as usize);
    ensure!(
        values.iter().all(|handle| !handle.is_null()),
        "{label} returned a null handle"
    );
    Ok(values)
}

impl Intel {
    pub(super) fn open() -> anyhow::Result<Self> {
        let library = Library::open("ControlLib.dll")?;
        // SAFETY: exact API signatures and version-zero layouts verified against
        // the native calls and the driver vendor's C header.
        let init: Initialize = unsafe { library.symbol(c"ctlInit")? };
        let enumerate_devices: Enumerate = unsafe { library.symbol(c"ctlEnumerateDevices")? };
        let device_properties: DeviceRead = unsafe { library.symbol(c"ctlGetDeviceProperties")? };
        let pci_properties: PciRead = unsafe { library.symbol(c"ctlPciGetProperties")? };
        let mut this = Self {
            context: ptr::null_mut(),
            close_api: unsafe { library.symbol(c"ctlClose")? },
            temperatures: unsafe { library.symbol(c"ctlEnumTemperatureSensors")? },
            temperature_properties: unsafe { library.symbol(c"ctlTemperatureGetProperties")? },
            temperature_state: unsafe { library.symbol(c"ctlTemperatureGetState")? },
            frequencies: unsafe { library.symbol(c"ctlEnumFrequencyDomains")? },
            frequency_properties: unsafe { library.symbol(c"ctlFrequencyGetProperties")? },
            frequency_state: unsafe { library.symbol(c"ctlFrequencyGetState")? },
            adapters: Vec::new(),
            _library: library,
        };
        let mut args = Init {
            header: Header::of::<Init>(),
            app_version: 65537,
            flags: 1,
            supported_version: 0,
            application_id: [0; 4],
        };
        status(
            unsafe { init(&mut args, &mut this.context) },
            "IGCL ctlInit",
        )?;
        ensure!(!this.context.is_null(), "IGCL returned a null API context");
        let result = (|| {
            for handle in enumerate(enumerate_devices, this.context, "IGCL device enumeration")? {
                let mut os_id = 0_u64;
                let mut props: DeviceProperties = zero();
                props.header = Header::of::<DeviceProperties>();
                props.os_id = &mut os_id;
                props.os_id_size = 8;
                status(
                    unsafe { device_properties(handle, &mut props) },
                    "IGCL device properties",
                )?;
                if props.device_type != 1 {
                    continue;
                }
                let mut pci: PciProperties = zero();
                pci.header = Header::of::<PciProperties>();
                status(
                    unsafe { pci_properties(handle, &mut pci) },
                    "IGCL PCI properties",
                )?;
                this.adapters.push(Adapter {
                    handle,
                    address: PciAddress {
                        domain: Some(pci.address.domain),
                        bus: pci.address.bus,
                        device: pci.address.device,
                        function: Some(pci.address.function),
                    },
                    vendor: props.vendor,
                    device: props.device,
                    name: String::from_utf8_lossy(
                        &props.name[..props
                            .name
                            .iter()
                            .position(|&b| b == 0)
                            .unwrap_or(props.name.len())],
                    )
                    .into_owned(),
                });
            }
            Ok(())
        })();
        if let Err(error) = result {
            return match this.close() {
                Ok(()) => Err(error),
                Err(cleanup) => {
                    Err(error.context(format!("IGCL discovery cleanup also failed: {cleanup:#}")))
                }
            };
        }
        Ok(this)
    }

    fn temperature(&self, device: Handle) -> anyhow::Result<f64> {
        let mut observed = Err(anyhow::anyhow!("IGCL has no observed GPU temperature"));
        // The current SDK keeps the last successful matching GPU sensor.
        for handle in enumerate(self.temperatures, device, "IGCL temperature enumeration")? {
            let mut props: TemperatureProperties = zero();
            props.header = Header::of::<TemperatureProperties>();
            status(
                unsafe { (self.temperature_properties)(handle, &mut props) },
                "IGCL temperature properties",
            )?;
            if props.kind != 1 {
                continue;
            }
            let mut value = f64::NAN;
            let result = status(
                unsafe { (self.temperature_state)(handle, &mut value) },
                "IGCL temperature state",
            )
            .and_then(|()| measured(f64::from(value as f32), "IGCL temperature"));
            if result.is_ok() || observed.is_err() {
                observed = result;
            }
        }
        observed
    }

    fn clock(&self, device: Handle) -> anyhow::Result<f64> {
        let mut observed = Err(anyhow::anyhow!("IGCL has no observed GPU core clock"));
        for handle in enumerate(self.frequencies, device, "IGCL frequency enumeration")? {
            let mut props: FrequencyProperties = zero();
            props.header = Header::of::<FrequencyProperties>();
            status(
                unsafe { (self.frequency_properties)(handle, &mut props) },
                "IGCL frequency properties",
            )?;
            if props.kind != 0 {
                continue;
            }
            let mut state: FrequencyState = zero();
            state.header = Header::of::<FrequencyState>();
            state.actual = f64::NAN;
            let result = status(
                unsafe { (self.frequency_state)(handle, &mut state) },
                "IGCL frequency state",
            )
            .and_then(|()| {
                ensure!(state.actual >= 0.0, "IGCL actual frequency is unavailable");
                measured(f64::from(state.actual as f32), "IGCL frequency")
            });
            if result.is_ok() || observed.is_err() {
                observed = result;
            }
        }
        observed
    }

    pub(super) fn is_empty(&self) -> bool {
        self.adapters.is_empty()
    }

    pub(super) fn sample(&mut self) -> Vec<GraphicsReading> {
        self.adapters
            .iter()
            .map(|adapter| GraphicsReading {
                address: adapter.address,
                vendor_id: adapter.vendor,
                device_id: adapter.device,
                name: adapter.name.clone(),
                enabled: None,
                readings: Readings {
                    temperature_celsius: self.temperature(adapter.handle),
                    clock_mhz: self.clock(adapter.handle),
                },
            })
            .collect()
    }

    pub(super) fn close(&mut self) -> anyhow::Result<()> {
        if !self.context.is_null() {
            // SDK 0x123250 assembly passes the live API handle in RCX.
            status(unsafe { (self.close_api)(self.context) }, "IGCL ctlClose")?;
            self.context = ptr::null_mut();
            self.adapters.clear();
        }
        Ok(())
    }
}

impl Drop for Intel {
    fn drop(&mut self) {
        if let Err(error) = self.close() {
            self._library.retain();
            eprintln!("IGCL cleanup failed: {error:#}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn structures_match_current_source_x64_abi() {
        assert_eq!(size_of::<Init>(), 36);
        assert_eq!(size_of::<DeviceProperties>(), 320);
        assert_eq!(std::mem::offset_of!(DeviceProperties, vendor), 64);
        assert_eq!(size_of::<PciProperties>(), 64);
        assert_eq!(size_of::<TemperatureProperties>(), 24);
        assert_eq!(size_of::<FrequencyProperties>(), 32);
        assert_eq!(size_of::<FrequencyState>(), 56);
        assert_eq!(std::mem::offset_of!(FrequencyState, actual), 40);
    }
}
