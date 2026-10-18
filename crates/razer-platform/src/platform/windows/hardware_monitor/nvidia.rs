//! SDK 0x120540 discovery and 0x11d7f0 cleanup, using the installed NVAPI driver.
use super::{Handle, Library, Readings, status};
use crate::hardware_monitor::{GraphicsReading, PciAddress, nvidia as reply};
use anyhow::{Context as _, ensure};
use std::{mem, ptr};

type Query = unsafe extern "C" fn(u32) -> *const ();
type Global = unsafe extern "C" fn() -> i32;
type Enumerate = unsafe extern "C" fn(*mut Handle, *mut u32) -> i32;
type Scalar = unsafe extern "C" fn(Handle, *mut u32) -> i32;
type Name = unsafe extern "C" fn(Handle, *mut u8) -> i32;
type PciIds = unsafe extern "C" fn(Handle, *mut u32, *mut u32, *mut u32, *mut u32) -> i32;
type Read = unsafe extern "C" fn(Handle, *mut u32) -> i32;
type SelectedRead = unsafe extern "C" fn(Handle, u32, *mut u32) -> i32;

struct Adapter {
    handle: Handle,
    address: PciAddress,
    device_id: u32,
    name: String,
    mode: Option<ClockMode>,
}

#[derive(Clone, Copy)]
enum ClockMode { Current, PerformanceState }

pub(super) struct Nvidia {
    unload: Global,
    thermal_capabilities: Option<Read>,
    thermal_value: Option<Read>,
    thermal_legacy: Option<SelectedRead>,
    clocks: Option<Read>,
    pstates: Option<SelectedRead>,
    active: bool,
    adapters: Vec<Adapter>,
    _library: Library,
}

unsafe fn query<T: Copy>(function: Query, id: u32) -> Option<T> {
    // SAFETY: numeric IDs/signatures are traced from the current SDK's table;
    // the caller retains NVAPI's library through all uses.
    let pointer = unsafe { function(id) };
    if pointer.is_null() { return None; }
    assert_eq!(mem::size_of::<T>(), mem::size_of_val(&pointer));
    Some(unsafe { mem::transmute_copy(&pointer) })
}

impl Nvidia {
    pub(super) fn open() -> anyhow::Result<Self> {
        let library = Library::open("nvapi64.dll")?;
        // SAFETY: query IDs and all signatures below follow SDK 0x120540.
        let query_interface: Query = unsafe { library.symbol(c"nvapi_QueryInterface")? };
        let initialize: Global = unsafe { query(query_interface, 22079528) }.context("NVAPI initialization is unavailable")?;
        let unload: Global = unsafe { query(query_interface, 3526090110) }.context("NVAPI cleanup is unavailable")?;
        let enumerate: Enumerate = unsafe { query(query_interface, 3853292063) }.context("NVAPI enumeration is unavailable")?;
        let name: Name = unsafe { query(query_interface, 0xceee8e9f) }.context("NVAPI GPU name is unavailable")?;
        let pci_ids: PciIds = unsafe { query(query_interface, 769635950) }.context("NVAPI PCI identity is unavailable")?;
        let bus: Scalar = unsafe { query(query_interface, 467712229) }.context("NVAPI PCI bus is unavailable")?;
        let slot: Scalar = unsafe { query(query_interface, 705312015) }.context("NVAPI PCI slot is unavailable")?;
        let mut this = Self {
            unload,
            thermal_capabilities: unsafe { query(query_interface, 197662269) },
            thermal_value: unsafe { query(query_interface, 1711159981) },
            thermal_legacy: unsafe { query(query_interface, 3814984278) },
            clocks: unsafe { query(query_interface, 3702920899) },
            pstates: unsafe { query(query_interface, 514148923) },
            active: false, adapters: Vec::new(), _library: library,
        };
        status(unsafe { initialize() }, "NVAPI Initialize")?;
        this.active = true;
        let result = (|| {
            let mut handles = [ptr::null_mut(); 64];
            let mut count = 0;
            status(unsafe { enumerate(handles.as_mut_ptr(), &mut count) }, "NVAPI EnumPhysicalGPUs")?;
            ensure!(count as usize <= handles.len(), "NVAPI adapter count exceeds the ABI maximum");
            for handle in handles.into_iter().take(count as usize) {
                ensure!(!handle.is_null(), "NVAPI returned a null physical adapter");
                let (mut combined, mut subsystem, mut revision, mut extended) = (0, 0, 0, 0);
                status(unsafe { pci_ids(handle, &mut combined, &mut subsystem, &mut revision, &mut extended) }, "NVAPI GetPCIIdentifiers")?;
                let (mut bus_id, mut slot_id) = (0, 0);
                status(unsafe { bus(handle, &mut bus_id) }, "NVAPI GetBusId")?;
                status(unsafe { slot(handle, &mut slot_id) }, "NVAPI GetBusSlotId")?;
                let mut text = [0_u8; 64];
                status(unsafe { name(handle, text.as_mut_ptr()) }, "NVAPI full name")?;
                let name = String::from_utf8_lossy(&text[..text.iter().position(|&b| b == 0).unwrap_or(text.len())]).into_owned();
                this.adapters.push(Adapter { handle, address: PciAddress { domain: None, bus: bus_id, device: slot_id, function: None }, device_id: combined >> 16, name, mode: None });
            }
            Ok(())
        })();
        if let Err(error) = result {
            return match this.close() { Ok(()) => Err(error), Err(cleanup) => Err(error.context(format!("NVAPI discovery cleanup also failed: {cleanup:#}"))) };
        }
        Ok(this)
    }

    pub(super) fn sample(&mut self) -> Vec<GraphicsReading> {
        self.adapters.iter_mut().map(|adapter| {
            let temperature_celsius = temperature(adapter.handle, self.thermal_capabilities, self.thermal_value, self.thermal_legacy);
            let clock_mhz = clock(adapter, self.clocks, self.pstates);
            GraphicsReading { address: adapter.address, vendor_id: 0x10de, device_id: adapter.device_id, name: adapter.name.clone(), enabled: None, readings: Readings { temperature_celsius, clock_mhz } }
        }).collect()
    }

    pub(super) fn close(&mut self) -> anyhow::Result<()> {
        if self.active {
            // SAFETY: all calls are synchronous and the worker owns this API lease.
            status(unsafe { (self.unload)() }, "NVAPI Unload")?;
            self.active = false;
            self.adapters.clear();
        }
        Ok(())
    }
}

impl Drop for Nvidia {
    fn drop(&mut self) { if let Err(error) = self.close() { self._library.retain(); eprintln!("NVAPI cleanup failed: {error:#}"); } }
}

fn temperature(handle: Handle, capabilities: Option<Read>, values: Option<Read>, legacy: Option<SelectedRead>) -> anyhow::Result<f64> {
    let modern = (|| {
        let (caps, read) = (capabilities.context("NVAPI thermal capability API unavailable")?, values.context("NVAPI thermal value API unavailable")?);
        let mut caps_reply = [0; 684]; caps_reply[0] = 133808;
        // SAFETY: live physical handle and exact versioned writable reply.
        status(unsafe { caps(handle, caps_reply.as_mut_ptr()) }, "NVAPI thermal capabilities")?;
        let (index, mask) = reply::thermal_sensor(&caps_reply)?;
        let mut value_reply = [0; 42]; value_reply[0] = 131240; value_reply[1] = mask;
        status(unsafe { read(handle, value_reply.as_mut_ptr()) }, "NVAPI thermal value")?;
        reply::thermal_value(&value_reply, index)
    })();
    match modern {
        Ok(value) => Ok(value),
        Err(modern_error) => {
            let fallback = (|| {
                let read = legacy.context("NVAPI thermal settings API unavailable")?;
                let mut buffer = [0; 17]; buffer[0] = 131140;
                status(unsafe { read(handle, 15, buffer.as_mut_ptr()) }, "NVAPI thermal settings")?;
                reply::legacy_temperature(&buffer)
            })();
            fallback.with_context(|| format!("NVAPI extended thermal query also failed: {modern_error:#}"))
        }
    }
}

fn read_clocks(handle: Handle, read: Read, versions: &[u32]) -> anyhow::Result<[u32; 66]> {
    let mut error = None;
    for &version in versions {
        let mut buffer = [0; 66]; buffer[0] = version;
        match status(unsafe { read(handle, buffer.as_mut_ptr()) }, "NVAPI current clocks") {
            Ok(()) => return Ok(buffer), Err(value) => error = Some(value),
        }
    }
    Err(error.context("NVAPI clock versions were not supplied")?)
}

fn read_pstates(handle: Handle, read: SelectedRead) -> anyhow::Result<[u32; 2717]> {
    let mut error = None;
    for version in [141940, 141940, 76404] {
        let mut buffer = [0; 2717]; buffer[0] = version;
        match status(unsafe { read(handle, u32::MAX, buffer.as_mut_ptr()) }, "NVAPI performance states") {
            Ok(()) => return Ok(buffer), Err(value) => error = Some(value),
        }
    }
    Err(error.context("NVAPI performance states did not respond")?)
}

fn clock(adapter: &mut Adapter, clocks: Option<Read>, pstates: Option<SelectedRead>) -> anyhow::Result<f64> {
    if adapter.mode.is_none() {
        if let Some(read) = clocks {
            if read_clocks(adapter.handle, read, &[196872, 131336]).is_ok() { adapter.mode = Some(ClockMode::Current); }
        }
        if adapter.mode.is_none() {
            let read = pstates.context("NVAPI has no usable clock interface")?;
            read_pstates(adapter.handle, read)?;
            adapter.mode = Some(ClockMode::PerformanceState);
        }
    }
    match adapter.mode.context("NVAPI clock mode is unobserved")? {
        ClockMode::Current => reply::clock(&read_clocks(adapter.handle, clocks.context("NVAPI clock API vanished")?, &[196872, 196872, 131336])?),
        ClockMode::PerformanceState => reply::pstate_clock(&read_pstates(adapter.handle, pstates.context("NVAPI performance-state API vanished")?)?),
    }
}
