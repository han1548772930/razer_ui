//! Sensor API and kernel handles stay on the monitoring worker's thread.
use crate::hardware_monitor::{GraphicsReading, Readings};
use anyhow::ensure;
use std::{
    ffi::{CStr, c_void},
    marker::PhantomData,
    mem, ptr,
    rc::Rc,
};
use windows_sys::Win32::{
    Foundation::{FreeLibrary, HMODULE},
    System::LibraryLoader::{GetProcAddress, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW},
};

#[path = "hardware_monitor/amd.rs"]
mod amd;
#[path = "hardware_monitor/amd_cpu.rs"]
mod amd_cpu;
#[path = "hardware_monitor/cpu.rs"]
mod cpu;
#[path = "hardware_monitor/display.rs"]
mod display;
#[path = "hardware_monitor/intel.rs"]
mod intel;
#[path = "hardware_monitor/level_zero.rs"]
mod level_zero;
#[path = "hardware_monitor/nvidia.rs"]
mod nvidia;

/// Only installed system/GPU driver APIs are loaded. Names are private constants
/// at the adapter call sites; this cannot load a Razer business DLL or user path.
struct Library {
    module: HMODULE,
    retained: bool,
    thread: PhantomData<Rc<()>>,
}

impl Library {
    fn open(name: &str) -> anyhow::Result<Self> {
        ensure!(
            !name.contains(['/', '\\', ':', '\0']),
            "Invalid system driver library name"
        );
        let name: Vec<_> = name.encode_utf16().chain([0]).collect();
        // SAFETY: terminated name; constrained system-directory lookup.
        let module =
            unsafe { LoadLibraryExW(name.as_ptr(), ptr::null_mut(), LOAD_LIBRARY_SEARCH_SYSTEM32) };
        ensure!(
            !module.is_null(),
            "Installed graphics driver API is unavailable: {}",
            std::io::Error::last_os_error()
        );
        Ok(Self {
            module,
            retained: false,
            thread: PhantomData,
        })
    }

    fn retain(&mut self) {
        self.retained = true;
    }

    /// Caller verifies the exact API signature against the retained native call
    /// chain. The function may only be used while this owner is alive.
    unsafe fn symbol<T: Copy>(&self, name: &CStr) -> anyhow::Result<T> {
        // SAFETY: library is retained and name is terminated.
        let function = unsafe { GetProcAddress(self.module, name.as_ptr().cast()) };
        ensure!(
            function.is_some(),
            "Graphics driver API is missing {}",
            name.to_string_lossy()
        );
        ensure!(
            mem::size_of::<T>() == mem::size_of_val(&function),
            "Invalid graphics function pointer width"
        );
        // SAFETY: caller supplies a verified function-pointer type of equal size.
        Ok(unsafe { mem::transmute_copy(&function) })
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        // SAFETY: adapters close their API contexts before their library drops.
        if !self.retained {
            unsafe {
                FreeLibrary(self.module);
            }
        }
    }
}

fn status(value: i32, operation: &str) -> anyhow::Result<()> {
    ensure!(value == 0, "{operation} failed with driver status {value}");
    Ok(())
}

type Handle = *mut c_void;

/// No driver installation, service start or elevation occurs during discovery.
pub struct Monitor {
    cpu: Option<cpu::Cpu>,
    nvidia: Option<nvidia::Nvidia>,
    intel: Option<intel::Intel>,
    level_zero: Option<level_zero::LevelZero>,
    amd: Option<amd::Amd>,
    diagnostics: Vec<String>,
}

impl Monitor {
    pub fn new() -> Self {
        let mut diagnostics = Vec::new();
        let cpu = match cpu::Cpu::open() {
            Ok(provider) => Some(provider),
            Err(error) => {
                diagnostics.push(format!("CPU discovery: {error:#}"));
                None
            }
        };
        let nvidia = match nvidia::Nvidia::open() {
            Ok(provider) => Some(provider),
            Err(error) => {
                diagnostics.push(format!("NVAPI discovery: {error:#}"));
                None
            }
        };
        let intel = match intel::Intel::open() {
            Ok(provider) => Some(provider),
            Err(error) => {
                diagnostics.push(format!("IGCL discovery: {error:#}"));
                None
            }
        };
        let level_zero = if intel.as_ref().is_none_or(|provider| provider.is_empty()) {
            match level_zero::LevelZero::open() {
                Ok(provider) => Some(provider),
                Err(error) => {
                    diagnostics.push(format!("oneAPI discovery: {error:#}"));
                    None
                }
            }
        } else {
            None
        };
        let amd = match amd::Amd::open() {
            Ok(provider) => Some(provider),
            Err(error) => {
                diagnostics.push(format!("ADL discovery: {error:#}"));
                None
            }
        };
        Self {
            cpu,
            nvidia,
            intel,
            level_zero,
            amd,
            diagnostics,
        }
    }

    pub fn cpu(&mut self) -> Readings {
        match self.cpu.as_mut() {
            Some(cpu) => cpu.sample(),
            None => Readings {
                temperature_celsius: Err(anyhow::anyhow!("CPU provider is unavailable")),
                clock_mhz: Err(anyhow::anyhow!("CPU provider is unavailable")),
            },
        }
    }

    pub fn graphics(&mut self) -> Vec<GraphicsReading> {
        let mut graphics = self
            .nvidia
            .as_mut()
            .map_or_else(Vec::new, nvidia::Nvidia::sample);
        if let Some(intel) = self.intel.as_mut() {
            graphics.extend(intel.sample());
        }
        if let Some(level_zero) = self.level_zero.as_mut() {
            graphics.extend(level_zero.sample());
        }
        if let Some(amd) = self.amd.as_mut() {
            graphics.extend(amd.sample());
        }
        display::annotate(&mut graphics, &mut self.diagnostics);
        graphics
    }

    pub fn take_diagnostics(&mut self) -> Vec<String> {
        mem::take(&mut self.diagnostics)
    }

    pub fn close(&mut self) -> anyhow::Result<()> {
        let mut errors = Vec::new();
        if let Some(provider) = self.level_zero.as_mut() {
            if let Err(error) = provider.close() {
                errors.push(format!("oneAPI: {error:#}"));
            } else {
                self.level_zero.take();
            }
        }
        if let Some(provider) = self.cpu.as_mut() {
            if let Err(error) = provider.close() {
                errors.push(format!("CPU: {error:#}"));
            } else {
                self.cpu.take();
            }
        }
        if let Some(provider) = self.amd.as_mut() {
            if let Err(error) = provider.close() {
                errors.push(format!("AMD: {error:#}"));
            } else {
                self.amd.take();
            }
        }
        if let Some(provider) = self.intel.as_mut() {
            if let Err(error) = provider.close() {
                errors.push(format!("Intel: {error:#}"));
            } else {
                self.intel.take();
            }
        }
        if let Some(provider) = self.nvidia.as_mut() {
            if let Err(error) = provider.close() {
                errors.push(format!("NVIDIA: {error:#}"));
            } else {
                self.nvidia.take();
            }
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
}

impl Default for Monitor {
    fn default() -> Self {
        Self::new()
    }
}
