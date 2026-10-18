//! ADL monitoring chain from SDK 0x134820, 0x133410, 0x134160 and 0x1318d0.
use super::{Handle, Library, Readings, status};
use crate::hardware_monitor::{GraphicsReading, PciAddress};
use anyhow::{Context as _, ensure};
use std::{
    ffi::c_void,
    mem::size_of,
    ptr,
    sync::{Mutex, MutexGuard},
};

// ADL's allocator callback has no context parameter. A lease prevents one
// monitor from releasing another monitor's allocations.
static OWNER: Mutex<()> = Mutex::new(());
static ALLOCATIONS: Mutex<Vec<usize>> = Mutex::new(Vec::new());
#[link(name = "ucrt")]
unsafe extern "C" {
    fn malloc(size: usize) -> *mut c_void;
    fn free(pointer: *mut c_void);
}
unsafe extern "C" fn allocate(size: i32) -> *mut c_void {
    if size <= 0 {
        return ptr::null_mut();
    }
    let Ok(mut allocations) = ALLOCATIONS.lock() else {
        return ptr::null_mut();
    };
    // SAFETY: ADL requests byte storage, which remains allocated until the
    // context has synchronously closed. No API returning caller-owned buffers
    // is used here, so none of these pointers can be freed by another caller.
    let pointer = unsafe { malloc(size as usize) };
    if !pointer.is_null() {
        allocations.push(pointer as usize);
    }
    pointer
}

#[repr(C)]
struct AdapterInfo {
    size: i32,
    index: i32,
    udid: [u8; 256],
    bus: i32,
    device: i32,
    function: i32,
    vendor: i32,
    name: [u8; 256],
    display: [u8; 256],
    present: i32,
    exists: i32,
    driver_path: [u8; 256],
    driver_path_ext: [u8; 256],
    pnp: [u8; 256],
    display_index: i32,
}
#[repr(C)]
struct Support {
    sensors: [u16; 256],
    reserved: [u32; 16],
}
#[repr(C)]
struct LogStart {
    sensors: [u16; 256],
    interval_ms: u32,
    reserved: [u32; 15],
}
#[repr(C)]
struct LogOutput {
    address: *const LogData,
    reserved: [u32; 14],
}
#[repr(C)]
struct LogData {
    version: u32,
    interval_ms: u32,
    updated: u64,
    values: [[u32; 2]; 256],
    reserved: [u32; 256],
}
#[repr(C)]
struct QueryData {
    size: i32,
    values: [[i32; 2]; 256],
}
#[repr(C)]
struct Temperature {
    size: i32,
    millidegrees: i32,
}
#[repr(C)]
struct Activity {
    size: i32,
    core: i32,
    memory: i32,
    voltage: i32,
    activity: i32,
    level: i32,
    bus_speed: i32,
    bus_lanes: i32,
    max_lanes: i32,
    reserved: i32,
}

type Allocator = unsafe extern "C" fn(i32) -> *mut c_void;
type Create = unsafe extern "C" fn(Allocator, i32) -> i32;
type Destroy = unsafe extern "C" fn() -> i32;
type Create2 = unsafe extern "C" fn(Allocator, i32, *mut Handle) -> i32;
type Destroy2 = unsafe extern "C" fn(Handle) -> i32;
type Count = unsafe extern "C" fn(*mut i32) -> i32;
type Adapters = unsafe extern "C" fn(*mut AdapterInfo, i32) -> i32;
type Active = unsafe extern "C" fn(i32, *mut i32) -> i32;
type ReadSupport = unsafe extern "C" fn(Handle, i32, *mut Support) -> i32;
type Query = unsafe extern "C" fn(Handle, i32, *mut QueryData) -> i32;
type DesktopCreate = unsafe extern "C" fn(Handle, i32, *mut u32) -> i32;
type DesktopDestroy = unsafe extern "C" fn(Handle, u32) -> i32;
type Start = unsafe extern "C" fn(Handle, i32, *mut LogStart, *mut LogOutput, u32) -> i32;
type Stop = unsafe extern "C" fn(Handle, i32, u32) -> i32;
type TemperatureGet = unsafe extern "C" fn(i32, i32, *mut Temperature) -> i32;
type ActivityGet = unsafe extern "C" fn(i32, *mut Activity) -> i32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Query,
    Logging,
    Legacy,
}
fn mode(device: u32) -> Mode {
    match device {
        5597 | 5607 | 5686 | 5688 | 5708 | 26287 | 26751 => Mode::Logging,
        4372 | 5056 | 5382 | 5390 | 5510 | 5567 | 5576 | 5710 | 5761 | 6402 | 6403 | 29471
        | 29504 | 29605 | 29615 | 29631 | 29663 | 29679 | 29695 | 29759 | 29772 | 29822 | 29824
        | 30032 | 30096 => Mode::Query,
        _ => Mode::Legacy,
    }
}
/// SDK 0x12bc00 initializes these Kaveri/Devastator branch offsets; all
/// other legacy branches retain the constructor's negative, unused offset.
fn legacy_temperature_offset(device: u32) -> f32 {
    if matches!(device, 4879 | 4883 | 4886 | 39169) {
        42.0
    } else {
        0.0
    }
}
struct Adapter {
    index: i32,
    address: PciAddress,
    device: u32,
    name: String,
    enabled: bool,
    mode: Mode,
    desktop: u32,
    logging: bool,
    data: *const LogData,
    support: Option<Support>,
    last_log_update: Option<u64>,
}
pub(super) struct Amd {
    context: Handle,
    active: bool,
    destroy: Destroy,
    destroy2: Option<Destroy2>,
    support: Option<ReadSupport>,
    query: Option<Query>,
    desktop_create: Option<DesktopCreate>,
    desktop_destroy: Option<DesktopDestroy>,
    start: Option<Start>,
    stop: Option<Stop>,
    temperature: Option<TemperatureGet>,
    activity: Option<ActivityGet>,
    adapters: Vec<Adapter>,
    owner: Option<MutexGuard<'static, ()>>,
    library: Library,
}

impl Amd {
    pub(super) fn open() -> anyhow::Result<Self> {
        let owner = OWNER
            .try_lock()
            .map_err(|_| anyhow::anyhow!("ADL already has an active or unclosed monitor owner"))?;
        let library = Library::open("atiadlxx.dll")?;
        // SAFETY: signatures verified against the retained call bodies and AMD's ABI.
        let create: Create = unsafe { library.symbol(c"ADL_Main_Control_Create")? };
        let create2: Option<Create2> = unsafe { library.symbol(c"ADL2_Main_Control_Create") }.ok();
        let count: Count = unsafe { library.symbol(c"ADL_Adapter_NumberOfAdapters_Get")? };
        let adapters: Adapters = unsafe { library.symbol(c"ADL_Adapter_AdapterInfo_Get")? };
        let active: Active = unsafe { library.symbol(c"ADL_Adapter_Active_Get")? };
        let mut this = Self {
            context: ptr::null_mut(),
            active: false,
            destroy: unsafe { library.symbol(c"ADL_Main_Control_Destroy")? },
            destroy2: unsafe { library.symbol(c"ADL2_Main_Control_Destroy") }.ok(),
            support: unsafe { library.symbol(c"ADL2_Adapter_PMLog_Support_Get") }.ok(),
            query: unsafe { library.symbol(c"ADL2_New_QueryPMLogData_Get") }.ok(),
            desktop_create: unsafe { library.symbol(c"ADL2_Desktop_Device_Create") }.ok(),
            desktop_destroy: unsafe { library.symbol(c"ADL2_Desktop_Device_Destroy") }.ok(),
            start: unsafe { library.symbol(c"ADL2_Adapter_PMLog_Start") }.ok(),
            stop: unsafe { library.symbol(c"ADL2_Adapter_PMLog_Stop") }.ok(),
            temperature: unsafe { library.symbol(c"ADL_Overdrive5_Temperature_Get") }.ok(),
            activity: unsafe { library.symbol(c"ADL_Overdrive5_CurrentActivity_Get") }.ok(),
            adapters: Vec::new(),
            owner: Some(owner),
            library,
        };
        status(unsafe { create(allocate, 1) }, "ADL Main Control Create")?;
        this.active = true;
        let result = (|| {
            if let Some(create2) = create2.filter(|_| this.destroy2.is_some()) {
                status(
                    unsafe { create2(allocate, 1, &mut this.context) },
                    "ADL2 Main Control Create",
                )?;
                ensure!(!this.context.is_null(), "ADL2 returned a null context");
            }
            let mut length = 0;
            status(unsafe { count(&mut length) }, "ADL adapter count")?;
            ensure!(
                (0..=250).contains(&length),
                "ADL adapter count exceeds the verified limit"
            );
            if length == 0 {
                return Ok(());
            }
            let mut infos: Vec<AdapterInfo> = (0..length)
                .map(|_| {
                    // SAFETY: AdapterInfo contains only integer fields and byte arrays.
                    let mut info: AdapterInfo = unsafe { std::mem::zeroed() };
                    info.size = size_of::<AdapterInfo>() as i32;
                    info
                })
                .collect();
            status(
                unsafe { adapters(infos.as_mut_ptr(), length * size_of::<AdapterInfo>() as i32) },
                "ADL adapter information",
            )?;
            for info in infos {
                let mut enabled = 0;
                status(
                    unsafe { active(info.index, &mut enabled) },
                    "ADL adapter activity",
                )?;
                let device = device_id(&info.pnp).or_else(|_| device_id(&info.udid))?;
                ensure!(
                    (0..250).contains(&info.index),
                    "ADL adapter index exceeds the verified limit"
                );
                let address = PciAddress {
                    domain: None,
                    bus: info.bus.try_into()?,
                    device: info.device.try_into()?,
                    function: Some(info.function.try_into()?),
                };
                if this.adapters.iter().any(|entry| entry.address == address) {
                    continue;
                }
                this.adapters.push(Adapter {
                    index: info.index,
                    address,
                    device,
                    name: String::from_utf8_lossy(
                        &info.name[..info
                            .name
                            .iter()
                            .position(|&b| b == 0)
                            .unwrap_or(info.name.len())],
                    )
                    .into_owned(),
                    enabled: enabled != 0,
                    mode: mode(device),
                    desktop: 0,
                    logging: false,
                    data: ptr::null(),
                    support: None,
                    last_log_update: None,
                });
            }
            for adapter in &mut this.adapters {
                if adapter.mode == Mode::Legacy {
                    continue;
                }
                ensure!(
                    !this.context.is_null(),
                    "AMD telemetry requires an ADL2 context"
                );
                let support = this.support.context("ADL2 PMLog support API is missing")?;
                let mut supported = Support {
                    sensors: [0; 256],
                    reserved: [0; 16],
                };
                status(
                    unsafe { support(this.context, adapter.index, &mut supported) },
                    "ADL2 PMLog support",
                )?;
                ensure!(
                    supported.sensors.contains(&0),
                    "ADL2 sensor list has no terminator"
                );
                if adapter.mode == Mode::Logging {
                    let create = this
                        .desktop_create
                        .context("ADL2 desktop creation API is missing")?;
                    this.desktop_destroy
                        .context("ADL2 desktop cleanup API is missing")?;
                    let start = this.start.context("ADL2 PMLog start API is missing")?;
                    this.stop.context("ADL2 PMLog stop API is missing")?;
                    status(
                        unsafe { create(this.context, adapter.index, &mut adapter.desktop) },
                        "ADL2 desktop creation",
                    )?;
                    ensure!(adapter.desktop != 0, "ADL2 desktop handle is invalid");
                    let mut input = LogStart {
                        sensors: supported.sensors,
                        interval_ms: 1000,
                        reserved: [0; 15],
                    };
                    let mut output = LogOutput {
                        address: ptr::null(),
                        reserved: [0; 14],
                    };
                    status(
                        unsafe {
                            start(
                                this.context,
                                adapter.index,
                                &mut input,
                                &mut output,
                                adapter.desktop,
                            )
                        },
                        "ADL2 PMLog start",
                    )?;
                    adapter.logging = true;
                    adapter.data = output.address;
                    ensure!(
                        !adapter.data.is_null(),
                        "ADL2 PMLog returned no mapped data"
                    );
                }
                adapter.support = Some(supported);
            }
            Ok(())
        })();
        if let Err(error) = result {
            return match this.close() {
                Ok(()) => Err(error),
                Err(cleanup) => {
                    Err(error.context(format!("ADL discovery cleanup also failed: {cleanup:#}")))
                }
            };
        }
        Ok(this)
    }

    pub(super) fn sample(&mut self) -> Vec<GraphicsReading> {
        self.adapters
            .iter_mut()
            .map(|adapter| {
                let readings = match adapter.mode {
                    Mode::Legacy => Readings {
                        temperature_celsius: (|| {
                            let api = self
                                .temperature
                                .context("ADL temperature API unavailable")?;
                            let mut value = Temperature {
                                size: 8,
                                millidegrees: 0,
                            };
                            status(
                                unsafe { api(adapter.index, 0, &mut value) },
                                "ADL temperature",
                            )?;
                            Ok(f64::from(
                                value.millidegrees as f32 / 1000.0
                                    + legacy_temperature_offset(adapter.device),
                            ))
                        })(),
                        clock_mhz: (|| {
                            let api = self
                                .activity
                                .context("ADL current activity API unavailable")?;
                            let mut value = Activity {
                                size: 40,
                                core: 0,
                                memory: 0,
                                voltage: 0,
                                activity: 0,
                                level: 0,
                                bus_speed: 0,
                                bus_lanes: 0,
                                max_lanes: 0,
                                reserved: 0,
                            };
                            status(
                                unsafe { api(adapter.index, &mut value) },
                                "ADL current activity",
                            )?;
                            ensure!(value.core >= 0, "ADL core clock is unavailable");
                            Ok(f64::from(value.core as f32 / 100.0))
                        })(),
                    },
                    Mode::Query => {
                        let data: anyhow::Result<QueryData> = (|| {
                            let api = self.query.context("ADL2 PMLog query API unavailable")?;
                            let mut output = QueryData {
                                size: 2052,
                                values: [[0; 2]; 256],
                            };
                            status(
                                unsafe { api(self.context, adapter.index, &mut output) },
                                "ADL2 PMLog query",
                            )?;
                            Ok(output)
                        })();
                        match data {
                            Ok(data) => Readings {
                                temperature_celsius: query_value(adapter, &data, 28, true),
                                clock_mhz: query_value(adapter, &data, 1, false),
                            },
                            Err(error) => failed(format!("{error:#}")),
                        }
                    }
                    Mode::Logging => {
                        let sampled = (|| {
                            ensure!(
                                adapter.logging && !adapter.data.is_null(),
                                "AMD logging owner is unavailable"
                            );
                            let before = unsafe {
                                ptr::read_volatile(ptr::addr_of!((*adapter.data).updated))
                            };
                            ensure!(
                                before != 0 && adapter.last_log_update != Some(before),
                                "AMD telemetry has not produced a fresh driver sample"
                            );
                            let readings = Readings {
                                temperature_celsius: mapped_value(adapter, 28, true),
                                clock_mhz: mapped_value(adapter, 1, false),
                            };
                            let after = unsafe {
                                ptr::read_volatile(ptr::addr_of!((*adapter.data).updated))
                            };
                            ensure!(
                                before == after,
                                "AMD telemetry changed between temperature and clock observations"
                            );
                            adapter.last_log_update = Some(after);
                            Ok(readings)
                        })();
                        match sampled {
                            Ok(readings) => readings,
                            Err(error) => failed(format!("{error:#}")),
                        }
                    }
                };
                GraphicsReading {
                    address: adapter.address,
                    vendor_id: 0x1002,
                    device_id: adapter.device,
                    name: adapter.name.clone(),
                    enabled: Some(adapter.enabled),
                    readings,
                }
            })
            .collect()
    }

    pub(super) fn close(&mut self) -> anyhow::Result<()> {
        for adapter in &mut self.adapters {
            if adapter.logging {
                status(
                    unsafe {
                        (self.stop.context("ADL2 stop API disappeared")?)(
                            self.context,
                            adapter.index,
                            adapter.desktop,
                        )
                    },
                    "ADL2 PMLog stop",
                )?;
                adapter.logging = false;
                adapter.data = ptr::null();
            }
        }
        for adapter in &mut self.adapters {
            if adapter.desktop != 0 {
                status(
                    unsafe {
                        (self
                            .desktop_destroy
                            .context("ADL2 desktop cleanup API disappeared")?)(
                            self.context,
                            adapter.desktop,
                        )
                    },
                    "ADL2 desktop cleanup",
                )?;
                adapter.desktop = 0;
            }
        }
        if !self.context.is_null() {
            status(
                unsafe {
                    (self
                        .destroy2
                        .context("ADL2 context cleanup API disappeared")?)(
                        self.context
                    )
                },
                "ADL2 context cleanup",
            )?;
            self.context = ptr::null_mut();
        }
        if self.active {
            status(unsafe { (self.destroy)() }, "ADL context cleanup")?;
            self.active = false;
        }
        if self.owner.is_some() {
            let mut allocations = ALLOCATIONS
                .lock()
                .map_err(|_| anyhow::anyhow!("ADL allocator owner is unavailable"))?;
            // SAFETY: both APIs and all logging threads have closed; every
            // pointer originated from this owner's malloc callback.
            for pointer in allocations.drain(..) {
                unsafe {
                    free(pointer as *mut c_void);
                }
            }
            self.owner.take();
        }
        Ok(())
    }
}

impl Drop for Amd {
    fn drop(&mut self) {
        if let Err(error) = self.close() {
            self.library.retain();
            if let Some(owner) = self.owner.take() {
                std::mem::forget(owner);
            }
            eprintln!("ADL cleanup remains incomplete; native owner retained: {error:#}");
        }
    }
}

fn device_id(value: &[u8]) -> anyhow::Result<u32> {
    let end = value
        .iter()
        .position(|&value| value == 0)
        .unwrap_or(value.len());
    let text = std::str::from_utf8(&value[..end])?.to_ascii_uppercase();
    let offset = text
        .find("DEV_")
        .context("ADL adapter has no PCI device identity")?
        + 4;
    Ok(u32::from_str_radix(
        text.get(offset..offset + 4)
            .context("Truncated ADL PCI identity")?,
        16,
    )?)
}
fn failed(message: String) -> Readings {
    Readings {
        temperature_celsius: Err(anyhow::anyhow!(message.clone())),
        clock_mhz: Err(anyhow::anyhow!(message)),
    }
}
fn query_value(
    adapter: &Adapter,
    data: &QueryData,
    sensor: usize,
    positive: bool,
) -> anyhow::Result<f64> {
    let support = adapter
        .support
        .as_ref()
        .context("AMD sensor support is unobserved")?;
    ensure!(
        support
            .sensors
            .iter()
            .take_while(|&&id| id != 0)
            .any(|&id| usize::from(id) == sensor),
        "AMD sensor {sensor} is not supported"
    );
    let entry = data
        .values
        .get(sensor)
        .context("AMD sensor index is invalid")?;
    ensure!(entry[0] != 0, "AMD sensor {sensor} value is not available");
    ensure!(
        !positive || entry[1] > 0,
        "AMD temperature sensor is unavailable"
    );
    Ok(f64::from(entry[1] as f32))
}
fn mapped_value(adapter: &Adapter, sensor: u32, nonzero: bool) -> anyhow::Result<f64> {
    ensure!(
        adapter.logging && !adapter.data.is_null(),
        "AMD sensor logging is not active"
    );
    // SAFETY: PMLog owns a mapped ADLPMLogData through successful Stop. All
    // accesses are bounded and volatile because the driver updates this memory.
    unsafe {
        let before = ptr::read_volatile(ptr::addr_of!((*adapter.data).updated));
        ensure!(before != 0, "AMD sensor log has not produced a sample");
        for index in 0..256 {
            let entry = ptr::read_volatile(ptr::addr_of!((*adapter.data).values[index]));
            if entry[0] == 0 {
                break;
            }
            if entry[0] == sensor && (!nonzero || entry[1] != 0) {
                let after = ptr::read_volatile(ptr::addr_of!((*adapter.data).updated));
                ensure!(before == after, "AMD sensor log changed during observation");
                return Ok(f64::from(entry[1] as i32 as f32));
            }
        }
    }
    anyhow::bail!("AMD sensor {sensor} has no current observation")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn adl_layouts_match_captured_x64_call_buffers() {
        assert_eq!(size_of::<AdapterInfo>(), 1572);
        assert_eq!(size_of::<Support>(), 576);
        assert_eq!(size_of::<LogStart>(), 576);
        assert_eq!(size_of::<LogOutput>(), 64);
        assert_eq!(size_of::<QueryData>(), 2052);
        assert_eq!(size_of::<Activity>(), 40);
        assert_eq!(std::mem::offset_of!(LogData, values), 16);
    }
    #[test]
    fn telemetry_branch_comes_from_current_pci_table() {
        assert_eq!(mode(26751), Mode::Logging);
        assert_eq!(mode(29631), Mode::Query);
        assert_eq!(mode(0x67df), Mode::Legacy);
        assert_eq!(
            device_id(b"PCI\\VEN_1002&DEV_73BF&SUBSYS_1\0").unwrap(),
            0x73bf
        );
        assert!(device_id(b"PCI\\VEN_1002&DEV_X").is_err());
    }
}
