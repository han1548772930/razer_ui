//! CPU measurements execute under one driver lease and one processor affinity.
use crate::{hardware_driver::{Connection, Processor}, hardware_monitor::{Readings, acpi_timer::AcpiTimer, amd_cpu, intel_cpu}};
use anyhow::{Context as _, ensure};
use std::{arch::x86_64::__cpuid, ptr, time::{Duration, Instant}};
use windows_sys::Win32::System::{SystemInformation::GetSystemFirmwareTable, Performance::{QueryPerformanceCounter, QueryPerformanceFrequency}};

pub(super) struct Cpu { connection: Connection, processor: Processor, timer: anyhow::Result<AcpiTimer> }
impl Cpu {
    pub(super) fn open() -> anyhow::Result<Self> {
        let processor = *Processor::active()?.first().context("CPU topology is empty")?;
        Ok(Self { connection: Connection::new()?, processor, timer: timer() })
    }
    pub(super) fn sample(&mut self) -> Readings {
        let timer = self.timer.as_ref();
        match self.connection.sample(self.processor, |driver| {
            ensure!(driver.protocol_version()? == 0x165, "Hardware driver protocol does not match the installed sensor adapter");
            // SAFETY: x86_64 guarantees CPUID. Affinity remains pinned throughout.
            let base = __cpuid(0);
            let mut vendor = [0; 12]; vendor[..4].copy_from_slice(&base.ebx.to_le_bytes()); vendor[4..8].copy_from_slice(&base.edx.to_le_bytes()); vendor[8..].copy_from_slice(&base.ecx.to_le_bytes());
            ensure!(base.eax >= 1, "CPU signature is unavailable");
            let signature = __cpuid(1).eax;
            let mut family = (signature >> 8) & 15; if family == 15 { family += (signature >> 20) & 255; }
            let mut model = (signature >> 4) & 15; if matches!((signature >> 8) & 15, 6 | 15) { model += ((signature >> 16) & 15) << 4; }
            if &vendor == b"GenuineIntel" {
                let temperature_celsius = if intel_cpu::has_temperature_target(family, model) { intel_cpu::temperature(driver) } else { Err(anyhow::anyhow!("Intel thermal calibration branch family {family:#x} model {model:#x} has not been resolved")) };
                let clock_mhz = (|| {
                    ensure!(base.eax >= 10, "Intel architectural counters are unavailable");
                    let counters = __cpuid(10);
                    ensure!(counters.eax & 255 != 0 && counters.edx & 31 >= 3, "Intel fixed counters are unavailable");
                    let haswell = matches!(model, 0x3c | 0x3d | 0x3f | 0x45 | 0x46 | 0x47 | 0x4f | 0x56);
                    intel_cpu::fixed_counter_clock(driver, ((counters.eax >> 8) & 255) as u8, |driver| {
                        if haswell {
                            // SDK 0x166ea0: 5 * QPC frequency / 3000.
                            if let Ok(timer) = timer { timer.wait(driver, (5 * crate::hardware_monitor::acpi_timer::FREQUENCY / 3000) as u32) }
                            else { let mut frequency=0_i64; ensure!(unsafe {QueryPerformanceFrequency(&mut frequency)}!=0 && frequency>0,"Intel QPC frequency is unavailable"); let first=qpc()?; let target=5*frequency/3000; let bounded=Instant::now(); loop { if qpc()?.checked_sub(first).context("Intel QPC interval overflow")? >= target {break;} ensure!(bounded.elapsed()<Duration::from_millis(100),"Intel QPC stopped advancing");std::hint::spin_loop();} Ok(()) }
                        } else {
                            timer.map_err(|error| anyhow::anyhow!("ACPI CPU timer unavailable: {error:#}"))?.wait(driver, 2500)
                        }
                    })
                })();
                Ok(Readings { temperature_celsius, clock_mhz })
            } else if &vendor == b"AuthenticAMD" && matches!(family,0x17 | 0x19 | 0x1a) {
                let clock_mhz=(|| { let timer=*timer.map_err(|error|anyhow::anyhow!("AMD ACPI timer unavailable: {error:#}"))?; let mut frequency=0_i64;ensure!(unsafe {QueryPerformanceFrequency(&mut frequency)}!=0 && frequency>0,"AMD QPC frequency is unavailable");amd_cpu::sample_clock(driver,timer,frequency as u64,||{let mut count=0;ensure!(unsafe{QueryPerformanceCounter(&mut count)}!=0,"AMD QPC read failed");Ok(count)},None) })();
                Ok(Readings {temperature_celsius:Err(anyhow::anyhow!("AMD package PCI target is not yet resolved")),clock_mhz})
            } else { anyhow::bail!("CPU sensor branch {} family {family:#x} model {model:#x} is unresolved", String::from_utf8_lossy(&vendor)); }
        }) {
            Ok(readings) => readings,
            Err(error) => failed(format!("{error:#}")),
        }
    }
    pub(super) fn close(&mut self) -> anyhow::Result<()> { self.connection.close() }
}
fn timer() -> anyhow::Result<AcpiTimer> {
    // SDK 0xc2de0 uses these exact ACPI/FACP identifiers.
    let length = unsafe { GetSystemFirmwareTable(1_094_930_505, 1_346_584_902, ptr::null_mut(), 0) };
    ensure!((80..=1_048_576).contains(&length), "ACPI FADT could not be read: {}", std::io::Error::last_os_error());
    let mut bytes = vec![0; length as usize];
    let actual = unsafe { GetSystemFirmwareTable(1_094_930_505, 1_346_584_902, bytes.as_mut_ptr().cast(), length) };
    ensure!(actual == length, "ACPI FADT changed during acquisition");
    AcpiTimer::from_fadt(&bytes)
}
fn failed(message: String) -> Readings { Readings { temperature_celsius: Err(anyhow::anyhow!(message.clone())), clock_mhz: Err(anyhow::anyhow!(message)) } }

fn qpc()->anyhow::Result<i64> {let mut value=0;ensure!(unsafe {QueryPerformanceCounter(&mut value)}!=0,"CPU QPC read failed");Ok(value)}
