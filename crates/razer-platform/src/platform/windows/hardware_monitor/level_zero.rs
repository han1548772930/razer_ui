//! Current SDK oneAPI branch 0x1229b0/0x1225d0/0x121fd0/0x1216c0.
use super::{Handle, Library, Readings, status};
use crate::hardware_monitor::{GraphicsReading, PciAddress, measured};
use anyhow::ensure;
use std::{ptr, sync::{Mutex, MutexGuard}};
use windows_sys::Win32::{Foundation::{GetLastError, SetLastError, ERROR_ENVVAR_NOT_FOUND}, System::Environment::{GetEnvironmentVariableW, SetEnvironmentVariableW}};

static ENVIRONMENT_OWNER: Mutex<()> = Mutex::new(());
struct Variable { name: Vec<u16>, previous: Option<Vec<u16>>, assigned: Vec<u16>, restored: bool }
impl Variable {
    fn read(name: &[u16]) -> anyhow::Result<Option<Vec<u16>>> {
        unsafe { SetLastError(0); }
        let needed = unsafe { GetEnvironmentVariableW(name.as_ptr(), ptr::null_mut(), 0) };
        if needed == 0 { let error=unsafe { GetLastError() }; ensure!(error==0 || error==ERROR_ENVVAR_NOT_FOUND, "Cannot inspect GPU loader environment: {error}"); return Ok((error!=ERROR_ENVVAR_NOT_FOUND).then(|| vec![0])); }
        let mut value=vec![0;needed as usize];
        let written=unsafe { GetEnvironmentVariableW(name.as_ptr(),value.as_mut_ptr(),needed) };
        ensure!(written < needed, "GPU loader environment changed during inspection"); value.truncate(written as usize+1); Ok(Some(value))
    }
    fn assign(name: &str, value: &str) -> anyhow::Result<Self> {
        let name: Vec<_>=name.encode_utf16().chain([0]).collect(); let assigned: Vec<_>=value.encode_utf16().chain([0]).collect();
        let previous=Self::read(&name)?;
        ensure!(unsafe { SetEnvironmentVariableW(name.as_ptr(),assigned.as_ptr()) }!=0,"Cannot configure installed GPU loader");
        Ok(Self { name, previous, assigned, restored:false })
    }
    fn restore(&mut self) -> anyhow::Result<()> {
        if self.restored { return Ok(()); }
        ensure!(Self::read(&self.name)?.as_ref()==Some(&self.assigned), "GPU loader environment ownership changed");
        ensure!(unsafe { SetEnvironmentVariableW(self.name.as_ptr(), self.previous.as_ref().map_or(ptr::null(), |value|value.as_ptr())) }!=0,"Cannot restore GPU loader environment"); self.restored=true; Ok(())
    }
}
impl Drop for Variable { fn drop(&mut self) { if let Err(error)=self.restore() { eprintln!("GPU loader environment cleanup: {error:#}"); } } }
#[repr(C)]
struct Header { kind:u32, next:*mut std::ffi::c_void }
impl Header { fn new(kind:u32)->Self { Self { kind, next:ptr::null_mut() } } }
#[repr(C)]
struct DeviceProperties { header:Header, kind:u32, vendor:u32, device:u32, flags:u32, subdevice:u32, nominal_clock:u32, allocation:u64, contexts:u32, priority:u32, threads:u32, simd:u32, eus:u32, subslices:u32, slices:u32, timer:u64, timestamp_bits:u32, kernel_bits:u32, uuid:[u8;16], name:[u8;256] }
#[repr(C)]
struct PciProperties { header:Header, domain:u32, bus:u32, device:u32, function:u32, generation:i32, width:i32, bandwidth:i64 }
#[repr(C)]
struct TemperatureProperties { header:Header, kind:u32, on_subdevice:u32, subdevice:u32, maximum:f64, critical:u32, threshold1:u32, threshold2:u32 }
#[repr(C)]
struct FrequencyProperties { header:Header, kind:u32, on_subdevice:u32, subdevice:u32, control:u32, throttle_event:u32, minimum:f64, maximum:f64 }
#[repr(C)]
struct FrequencyState { header:Header, voltage:f64, request:f64, tdp:f64, efficient:f64, actual:f64, reasons:u32 }
type Initialize=unsafe extern "C" fn(u32)->i32;
type Drivers=unsafe extern "C" fn(*mut u32,*mut Handle)->i32;
type Enumerate=unsafe extern "C" fn(Handle,*mut u32,*mut Handle)->i32;
type DeviceRead=unsafe extern "C" fn(Handle,*mut DeviceProperties)->i32;
type PciRead=unsafe extern "C" fn(Handle,*mut PciProperties)->i32;
type TemperatureProps=unsafe extern "C" fn(Handle,*mut TemperatureProperties)->i32;
type TemperatureRead=unsafe extern "C" fn(Handle,*mut f64)->i32;
type FrequencyProps=unsafe extern "C" fn(Handle,*mut FrequencyProperties)->i32;
type FrequencyRead=unsafe extern "C" fn(Handle,*mut FrequencyState)->i32;
struct Adapter { handle:Handle, address:PciAddress, vendor:u32, device:u32, name:String }
pub(super) struct LevelZero { adapters:Vec<Adapter>, temperatures:Enumerate, temperature_props:TemperatureProps, temperature_read:TemperatureRead, frequencies:Enumerate, frequency_props:FrequencyProps, frequency_read:FrequencyRead, library:Option<Library>, environment:Vec<Variable>, owner:Option<MutexGuard<'static,()>> }
fn zero<T>()->T { // All call sites are private repr(C) integer/pointer/float records.
    unsafe { std::mem::zeroed() }
}
fn enumerate(api:Enumerate,parent:Handle,label:&str)->anyhow::Result<Vec<Handle>> {
    let mut count=0; status(unsafe {api(parent,&mut count,ptr::null_mut())},label)?; ensure!(count<=4096,"{label} count exceeds bound"); if count==0 {return Ok(Vec::new());}
    let mut handles=vec![ptr::null_mut();count as usize];status(unsafe{api(parent,&mut count,handles.as_mut_ptr())},label)?;ensure!(count as usize<=handles.len(),"{label} count changed beyond buffer"); handles.truncate(count as usize);ensure!(handles.iter().all(|handle|!handle.is_null()),"{label} returned null handle");Ok(handles)
}
impl LevelZero {
    pub(super) fn open()->anyhow::Result<Self> {
        let owner=ENVIRONMENT_OWNER.try_lock().map_err(|_|anyhow::anyhow!("oneAPI already has an environment owner"))?;
        let library=Library::open("ze_loader.dll")?;
        let initialize:Initialize=unsafe{library.symbol(c"zeInit")?};let drivers:Drivers=unsafe{library.symbol(c"zeDriverGet")?};let devices:Enumerate=unsafe{library.symbol(c"zeDeviceGet")?};let props:DeviceRead=unsafe{library.symbol(c"zeDeviceGetProperties")?};let pci:PciRead=unsafe{library.symbol(c"zeDevicePciGetPropertiesExt")?};
        let mut this=Self {adapters:Vec::new(), temperatures:unsafe{library.symbol(c"zesDeviceEnumTemperatureSensors")?},temperature_props:unsafe{library.symbol(c"zesTemperatureGetProperties")?},temperature_read:unsafe{library.symbol(c"zesTemperatureGetState")?},frequencies:unsafe{library.symbol(c"zesDeviceEnumFrequencyDomains")?},frequency_props:unsafe{library.symbol(c"zesFrequencyGetProperties")?},frequency_read:unsafe{library.symbol(c"zesFrequencyGetState")?},library:Some(library),environment:Vec::new(),owner:Some(owner)};
        this.environment.push(Variable::assign("ZES_ENABLE_SYSMAN","1")?);this.environment.push(Variable::assign("ZE_ENABLE_LOADER_DEBUG_TRACE","0")?);
        status(unsafe{initialize(1)},"oneAPI zeInit")?;
        let mut count=0;status(unsafe{drivers(&mut count,ptr::null_mut())},"oneAPI driver enumeration")?;ensure!(count<=4096,"oneAPI driver count exceeds bound");
        let mut handles=vec![ptr::null_mut();count as usize]; if count!=0 {status(unsafe{drivers(&mut count,handles.as_mut_ptr())},"oneAPI drivers")?;ensure!(count as usize<=handles.len(),"oneAPI driver count changed beyond buffer");handles.truncate(count as usize);}
        for driver in handles.into_iter().take(16) {
            ensure!(!driver.is_null(),"oneAPI returned null driver");
            for handle in enumerate(devices,driver,"oneAPI devices")? {
                let mut properties:DeviceProperties=zero();properties.header=Header::new(3);status(unsafe{props(handle,&mut properties)},"oneAPI device properties")?;if properties.kind!=1 {continue;}
                let mut address:PciProperties=zero();address.header=Header::new(65544);status(unsafe{pci(handle,&mut address)},"oneAPI PCI properties")?;
                this.adapters.push(Adapter {handle,address:PciAddress {domain:Some(address.domain),bus:address.bus,device:address.device,function:Some(address.function)},vendor:properties.vendor,device:properties.device,name:String::from_utf8_lossy(&properties.name[..properties.name.iter().position(|&v|v==0).unwrap_or(properties.name.len())]).into_owned()});
            }
        }
        Ok(this)
    }
    fn temperature(&self,handle:Handle)->anyhow::Result<f64> {
        for sensor in enumerate(self.temperatures,handle,"oneAPI temperature sensors")? {
            let mut properties:TemperatureProperties=zero();properties.header=Header::new(20);status(unsafe{(self.temperature_props)(sensor,&mut properties)},"oneAPI temperature properties")?;if properties.kind!=1 {continue;}
            let mut value=f64::NAN;status(unsafe{(self.temperature_read)(sensor,&mut value)},"oneAPI temperature state")?;ensure!(value>0.0,"oneAPI GPU temperature is unavailable");return measured(f64::from(value as f32),"oneAPI temperature");
        }
        anyhow::bail!("oneAPI has no GPU temperature sensor")
    }
    fn clock(&self,handle:Handle)->anyhow::Result<f64> {
        let mut observed=Err(anyhow::anyhow!("oneAPI has no graphics clock observation"));
        for sensor in enumerate(self.frequencies,handle,"oneAPI frequency domains")? {
            let mut properties:FrequencyProperties=zero();properties.header=Header::new(9);status(unsafe{(self.frequency_props)(sensor,&mut properties)},"oneAPI frequency properties")?;if properties.kind!=0 {continue;}
            let mut value:FrequencyState=zero();value.header=Header::new(27);value.actual=f64::NAN;
            let result=status(unsafe{(self.frequency_read)(sensor,&mut value)},"oneAPI frequency state").and_then(|()|{ensure!(value.actual>=0.0,"oneAPI actual graphics clock is unavailable");measured(f64::from(value.actual as f32),"oneAPI actual clock")});if result.is_ok() || observed.is_err() {observed=result;}
        }
        observed
    }
    pub(super) fn sample(&mut self)->Vec<GraphicsReading> {self.adapters.iter().map(|adapter|GraphicsReading{address:adapter.address,vendor_id:adapter.vendor,device_id:adapter.device,name:adapter.name.clone(),enabled:None,readings:Readings{temperature_celsius:self.temperature(adapter.handle),clock_mhz:self.clock(adapter.handle)}}).collect()}
    pub(super) fn close(&mut self)->anyhow::Result<()> {
        self.adapters.clear(); self.library.take();let mut errors=Vec::new();for value in self.environment.iter_mut().rev(){if let Err(error)=value.restore(){errors.push(format!("{error:#}"));}}ensure!(errors.is_empty(),"{}",errors.join("; "));self.environment.clear();self.owner.take();Ok(())
    }
}
impl Drop for LevelZero {fn drop(&mut self){if let Err(error)=self.close(){eprintln!("oneAPI cleanup failed: {error:#}");}}}
#[cfg(test)]
mod tests {use super::*;use std::mem::size_of;#[test]fn level_zero_layouts_match_current_native_buffers(){assert_eq!(size_of::<DeviceProperties>(),368);assert_eq!(size_of::<PciProperties>(),48);assert_eq!(std::mem::offset_of!(TemperatureProperties,kind),16);assert_eq!(std::mem::offset_of!(FrequencyState,actual),48);assert_eq!(size_of::<FrequencyState>(),64);}}
