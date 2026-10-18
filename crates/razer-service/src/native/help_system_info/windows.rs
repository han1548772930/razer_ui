use super::{
    devices,
    registry::{self, Key},
    semantics, wmi,
};
use anyhow::ensure;
use razer_device::system_info::{
    Data, FirmwareDetails, FullReport, Monitor, NativeValue, Query, Response, SystemDetails,
};
use std::ptr::{null, null_mut};
use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory, DXGI_ERROR_NOT_FOUND, IDXGIFactory};
use windows_sys::Win32::{
    Devices::Display::GetNumberOfPhysicalMonitorsFromHMONITOR,
    Foundation::{LPARAM, RECT},
    Graphics::Gdi::*,
    System::{
        Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE},
        SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX},
    },
};

#[derive(Default)]
struct Observations {
    diagnostics: Vec<String>,
}
impl Observations {
    fn wmi<const N: usize>(
        &mut self,
        class: &str,
        fields: [&str; N],
    ) -> Option<[Option<String>; N]> {
        match wmi::strings(class, fields, &mut self.diagnostics) {
            Ok(values) => Some(values),
            Err(error) => {
                self.diagnostics.push(format!("WMI {class}: {error:#}"));
                None
            }
        }
    }
    fn read<T>(&mut self, field: &str, operation: impl FnOnce() -> anyhow::Result<T>) -> Option<T> {
        match operation() {
            Ok(value) => Some(value),
            Err(error) => {
                self.diagnostics.push(format!("{field}: {error:#}"));
                None
            }
        }
    }
    fn optional<T>(
        &mut self,
        field: &str,
        operation: impl FnOnce() -> anyhow::Result<Option<T>>,
    ) -> Option<T> {
        self.read(field, operation).flatten()
    }
    fn partial<T>(
        &mut self,
        field: &str,
        operation: impl FnOnce(&mut Vec<String>) -> anyhow::Result<Option<T>>,
    ) -> Option<T> {
        let result = operation(&mut self.diagnostics);
        self.optional(field, || result)
    }
}

pub(super) fn query(product_id: u32, query: Query) -> anyhow::Result<Response> {
    let mut observed = Observations::default();
    let computer = observed.wmi("Win32_ComputerSystem", ["Name", "Model"]);
    let processor = observed.wmi("Win32_Processor", ["Name", "Manufacturer"]);
    let graphics = observed.partial("Graphics", graphics);
    let ram = observed.wmi("Win32_PhysicalMemory", ["Name", "Manufacturer"]);
    let ram_size = ram
        .as_ref()
        .and_then(|_| observed.read("MemorySize", memory));
    let os = observed.wmi("Win32_OperatingSystem", ["Caption"]);
    let serial = observed.wmi("Win32_Bios", ["SerialNumber"]);
    let monitors = monitors();
    observed.diagnostics.extend(monitors.diagnostics);
    let display = if monitors.values.is_empty() {
        NativeValue::Null(())
    } else {
        NativeValue::Value(monitors.values)
    };
    let mut details = SystemDetails::observed_display(display);
    if let Some([name, model]) = computer {
        details.device_name = name;
        details.model_name = model;
    }
    if let Some([name, manufacturer]) = processor {
        details.cpu = name;
        details.cpu_manufacturer = manufacturer;
    }
    if let Some((name, manufacturer)) = graphics {
        details.gpu = Some(name);
        details.gpu_manufacturer = Some(manufacturer);
    }
    if let Some([_, manufacturer]) = ram {
        details.ram_manufacturer = manufacturer;
        details.ram = ram_size;
    }
    if let Some([version]) = os {
        details.os_version = version;
    }
    if let Some([serial]) = serial {
        details.serial_number = serial.filter(|serial| !serial.is_empty());
    }

    details.razer_app_engine = observed.optional("RazerAppEngine", || {
        registry::software(
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\WOW6432Node\Razer\RazerAppEngine",
            "version",
        )
    });
    details.razer_synapse3 = observed.optional("RazerSynapse3", || {
        registry::software(
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\WOW6432Node\Razer\Synapse3\RazerSynapse",
            "ProductVersion",
        )
    });
    details.nvidia_geforce_experience = observed.optional("NVIDIAGeForceExperience", || {
        registry::software(
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\WOW6432Node\NVIDIA Corporation\Global\GFExperience",
            "Version",
        )
    });
    details.nvidia_app = observed.optional("NVIDIANvApp", || {
        registry::software(
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\WOW6432Node\NVIDIA Corporation\Global\NvApp",
            "Version",
        )
    });
    if let Some((version, platform)) = observed.optional("NVIDIAControlPanel", || {
        package("NVIDIACorp.NVIDIAControlPanel")
    }) {
        details.nvidia_control_panel = Some(version);
        details.nvidia_control_panel_platform = Some(platform);
    }
    if let Some((version, platform)) = observed.optional("IntelGraphicCommandCenter", || {
        package("AppUp.IntelGraphicsExperience")
    }) {
        // The producer copies the NVIDIA temporaries into these fields. Keep
        // the actual Intel query result under its own identity instead.
        details.intel_graphic_command_center = Some(version);
        details.intel_graphic_command_center_platform = Some(platform);
    }
    details.amd_adrenalin_edition = observed.optional("AMDAdrenalinEdition", || {
        registry::software(
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\ATI Technologies\Install",
            "RadeonSoftwareVersion",
        )
    });
    details.razer_cortex = observed.optional("RazerCortex", || {
        registry::software(
            HKEY_LOCAL_MACHINE,
            r"SOFTWARE\WOW6432Node\Razer\RazerCortex",
            "CurrentVersion",
        )
    });
    details.razer_axon = observed.optional("RazerAxon", || {
        registry::software(
            HKEY_CURRENT_USER,
            r"SOFTWARE\Razer\RazerAxon",
            "CurrentVersion",
        )
    });
    let data = match query {
        Query::Display => Data::Display(details),
        Query::Full => {
            let (mut bios, mut ec) = (None, None);
            if let Some(Some(key)) =
                observed.read("BIOS", || Key::open(HKEY_LOCAL_MACHINE, registry::BIOS))
            {
                bios = observed.optional("BiosFW", || key.string("BIOSVersion"));
                let major = observed.optional("ECFirmwareMajorRelease", || {
                    key.dword("ECFirmwareMajorRelease")
                });
                let minor = observed.optional("ECFirmwareMinorRelease", || {
                    key.dword("ECFirmwareMinorRelease")
                });
                // Missing registry values are unobserved, never a fabricated 0.00.
                ec = major
                    .zip(minor)
                    .map(|(major, minor)| format!("{major:x}.{minor:02x}"));
            }
            let camera = observed.partial("CameraFW", devices::camera);
            let vbios = observed.partial("VBiosFW", devices::vbios);
            let firmware = if bios.is_none() && ec.is_none() && camera.is_none() && vbios.is_none()
            {
                NativeValue::Null(())
            } else {
                NativeValue::Value(FirmwareDetails::observed(bios, ec, camera, vbios))
            };
            let drivers = devices::drivers(&mut observed.diagnostics);
            let drivers = if drivers.is_empty() {
                NativeValue::Null(())
            } else {
                NativeValue::Value(drivers)
            };
            Data::Full(FullReport::observed(details, firmware, drivers))
        }
    };
    Ok(Response {
        product_id,
        data,
        diagnostics: observed.diagnostics,
    })
}

fn package(product: &str) -> anyhow::Result<Option<(String, String)>> {
    let Some(key) = Key::open(HKEY_LOCAL_MACHINE, registry::CAP_AUTHZ)? else {
        return Ok(None);
    };
    let names = key.subkeys()?;
    Ok(Some(semantics::package_version(
        names.iter().map(String::as_str),
        product,
    )))
}
fn memory() -> anyhow::Result<String> {
    let mut memory = MEMORYSTATUSEX {
        dwLength: size_of::<MEMORYSTATUSEX>() as u32,
        ..unsafe { std::mem::zeroed() }
    };
    ensure!(
        unsafe { GlobalMemoryStatusEx(&mut memory) } != 0,
        "Read physical memory: {}",
        std::io::Error::last_os_error()
    );
    Ok(semantics::memory_size(memory.ullTotalPhys))
}
fn graphics(diagnostics: &mut Vec<String>) -> anyhow::Result<Option<(String, String)>> {
    let factory: IDXGIFactory = unsafe { CreateDXGIFactory()? };
    let mut values = Vec::new();
    for index in 0.. {
        let adapter = match unsafe { factory.EnumAdapters(index) } {
            Ok(adapter) => adapter,
            Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => break,
            Err(error) => {
                diagnostics.push(format!("Graphics adapter enumeration: {error}"));
                break;
            }
        };
        let description = match unsafe { adapter.GetDesc() } {
            Ok(description) => description,
            Err(error) => {
                diagnostics.push(format!("Graphics adapter description: {error}"));
                break;
            }
        };
        let end = description
            .Description
            .iter()
            .position(|unit| *unit == 0)
            .unwrap_or(description.Description.len());
        // The source constructs std::string from the wchar iterator, retaining
        // each low byte. Invalid resulting UTF-8 must remain an explicit error.
        let name = match String::from_utf8(
            description.Description[..end]
                .iter()
                .map(|unit| *unit as u8)
                .collect(),
        ) {
            Ok(name) => name,
            Err(error) => {
                diagnostics.push(format!(
                    "Graphics description is not valid source UTF-8: {error}"
                ));
                break;
            }
        };
        values.push((
            name,
            semantics::manufacturer(description.VendorId).to_owned(),
        ));
    }
    let preferred = values
        .iter()
        .position(|(name, vendor)| {
            vendor == "NVIDIA" && name.to_ascii_lowercase().contains("laptop")
        })
        .or_else(|| values.iter().position(|(_, vendor)| vendor == "NVIDIA"))
        .or_else(|| (!values.is_empty()).then_some(0));
    Ok(preferred.map(|index| values.remove(index)))
}

#[derive(Default)]
struct Monitors {
    values: Vec<Monitor>,
    diagnostics: Vec<String>,
}
fn monitors() -> Monitors {
    let mut output = Monitors::default();
    // EnumDisplayMonitors calls synchronously; output remains alive and pinned
    // on this stack through every callback. No callback pointer is retained.
    let ok = unsafe {
        EnumDisplayMonitors(
            null_mut(),
            null(),
            Some(monitor),
            &mut output as *mut Monitors as LPARAM,
        )
    };
    if ok == 0 && output.diagnostics.is_empty() {
        output.diagnostics.push(format!(
            "Display enumeration: {}",
            std::io::Error::last_os_error()
        ));
    }
    output
}
unsafe extern "system" fn monitor(handle: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> i32 {
    let output = unsafe { &mut *(data as *mut Monitors) };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        enumerate_monitor(handle, output)
    }));
    match result {
        Ok(Ok(())) => 1,
        Ok(Err(error)) => {
            output.diagnostics.push(format!("Display: {error:#}"));
            0
        }
        Err(_) => {
            output
                .diagnostics
                .push("Display enumeration callback panicked".into());
            0
        }
    }
}
fn enumerate_monitor(handle: HMONITOR, output: &mut Monitors) -> anyhow::Result<()> {
    let mut info: MONITORINFOEXW = unsafe { std::mem::zeroed() };
    info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
    ensure!(
        unsafe { GetMonitorInfoW(handle, (&mut info as *mut MONITORINFOEXW).cast()) } != 0,
        "Read monitor info: {}",
        std::io::Error::last_os_error()
    );
    for index in 0.. {
        let mut device: DISPLAY_DEVICEW = unsafe { std::mem::zeroed() };
        device.cb = size_of::<DISPLAY_DEVICEW>() as u32;
        if unsafe { EnumDisplayDevicesW(info.szDevice.as_ptr(), index, &mut device, 0) } == 0 {
            break;
        }
        let id = registry::text(&device.DeviceID)?;
        let (name, manufacturer) = match devices::edid(&id) {
            Ok(Some(value)) => value,
            Ok(None) => (None, String::new()),
            Err(error) => {
                output.diagnostics.push(format!("Monitor EDID: {error:#}"));
                (None, String::new())
            }
        };
        let mut count = 0;
        ensure!(
            unsafe { GetNumberOfPhysicalMonitorsFromHMONITOR(handle, &mut count) } != 0,
            "Read physical monitor count: {}",
            std::io::Error::last_os_error()
        );
        let name = name
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| semantics::monitor_fallback(&id).to_owned());
        output.values.push(Monitor::new(name, manufacturer));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_system_info_partial_failure_does_not_replace_observed_values() {
        let mut observations = Observations::default();
        let cpu = observations.read("CPU", || Ok("Actual CPU".to_owned()));
        let gpu = observations.read::<String>("GPU", || anyhow::bail!("DXGI failed"));
        let absent_app = observations.optional::<String>("AbsentApp", || Ok(None));
        assert_eq!(cpu.as_deref(), Some("Actual CPU"));
        assert!(gpu.is_none());
        assert!(absent_app.is_none());
        assert_eq!(observations.diagnostics, ["GPU: DXGI failed"]);
    }

    #[test]
    fn help_system_info_late_failure_keeps_partial_query_and_diagnostic() {
        let mut observations = Observations::default();
        let revision = observations.partial("CameraFW", |diagnostics| {
            diagnostics.push("Camera interfaces: later device failed".into());
            Ok(Some("0123".to_owned()))
        });
        assert_eq!(revision.as_deref(), Some("0123"));
        assert_eq!(
            observations.diagnostics,
            ["Camera interfaces: later device failed"]
        );
        let unavailable =
            observations.partial::<String>("VBiosFW", |_| anyhow::bail!("open failed"));
        assert!(unavailable.is_none());
        assert_eq!(observations.diagnostics[1], "VBiosFW: open failed");
    }
}
