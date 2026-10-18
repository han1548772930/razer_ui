//! CPU measurements execute under one driver lease and one processor affinity.
use crate::{
    hardware_driver::{Connection, Processor},
    hardware_monitor::{Readings, acpi_timer::AcpiTimer, amd_cpu, intel_cpu},
};
use anyhow::{Context as _, ensure};
use std::{
    arch::x86_64::__cpuid,
    ptr,
    time::{Duration, Instant},
};
use windows_sys::Win32::System::{
    Performance::{QueryPerformanceCounter, QueryPerformanceFrequency},
    SystemInformation::GetSystemFirmwareTable,
};

pub(super) struct Cpu {
    connection: Connection,
    processor: Processor,
    timer: anyhow::Result<AcpiTimer>,
    amd_temperature: Option<super::amd_cpu::Temperature>,
    cleanup_failure: Option<String>,
}
impl Cpu {
    pub(super) fn open() -> anyhow::Result<Self> {
        let processor = *Processor::active()?
            .first()
            .context("CPU topology is empty")?;
        Ok(Self {
            connection: Connection::new()?,
            processor,
            timer: timer(),
            amd_temperature: None,
            cleanup_failure: None,
        })
    }
    pub(super) fn sample(&mut self) -> Readings {
        if let Some(error) = &self.cleanup_failure {
            return failed(error.clone());
        }
        let timer = self.timer.as_ref();
        let amd_temperature = &mut self.amd_temperature;
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
                let clock_mhz = intel_clock(driver,family,model,base.eax,timer);
                Ok(Readings { temperature_celsius, clock_mhz })
            } else if &vendor == b"AuthenticAMD" && matches!(family,0x17 | 0x19 | 0x1a) {
                let clock_mhz=(|| { let timer=*timer.map_err(|error|anyhow::anyhow!("AMD ACPI timer unavailable: {error:#}"))?; let mut frequency=0_i64;ensure!(unsafe {QueryPerformanceFrequency(&mut frequency)}!=0 && frequency>0,"AMD QPC frequency is unavailable");amd_cpu::sample_clock(driver,timer,frequency as u64,||{let mut count=0;ensure!(unsafe{QueryPerformanceCounter(&mut count)}!=0,"AMD QPC read failed");Ok(count)},None) })();
                let temperature_celsius = (|| {
                    if amd_temperature.is_none() {
                        let extended=__cpuid(0x8000_0000).eax;ensure!(extended>=0x8000_001e,"AMD node topology is unavailable");
                        let nodes=((__cpuid(0x8000_001e).ecx>>8)&7)+1;
                        let mut bytes=Vec::with_capacity(48);for leaf in 0x8000_0002..=0x8000_0004 {let value=__cpuid(leaf);for register in [value.eax,value.ebx,value.ecx,value.edx]{bytes.extend_from_slice(&register.to_le_bytes());}}
                        let end=bytes.iter().position(|&value|value==0).unwrap_or(bytes.len());let brand=std::str::from_utf8(&bytes[..end]).context("AMD CPU brand is not valid ASCII")?;
                        *amd_temperature=Some(super::amd_cpu::Temperature::open(family,model,brand,nodes)?);
                    }
                    amd_temperature.as_mut().context("AMD thermal owner is unavailable")?.sample(driver)
                })();
                Ok(Readings {temperature_celsius,clock_mhz})
            } else { anyhow::bail!("CPU sensor branch {} family {family:#x} model {model:#x} is unresolved", String::from_utf8_lossy(&vendor)); }
        }) {
            Ok(readings) => {
                if let Err(error)=&readings.clock_mhz {if !intel_cpu::controls_restored(error){self.cleanup_failure=Some(format!("CPU controls remain unconfirmed: {error:#}"));}}
                readings
            },
            Err(error) => failed(format!("{error:#}")),
        }
    }
    pub(super) fn close(&mut self) -> anyhow::Result<()> {
        self.connection.close()?;
        ensure!(
            self.cleanup_failure.is_none(),
            "{}",
            self.cleanup_failure.as_deref().unwrap_or("")
        );
        Ok(())
    }
}
fn timer() -> anyhow::Result<AcpiTimer> {
    // SDK 0xc2de0 uses these exact ACPI/FACP identifiers.
    let length =
        unsafe { GetSystemFirmwareTable(1_094_930_505, 1_346_584_902, ptr::null_mut(), 0) };
    ensure!(
        (80..=1_048_576).contains(&length),
        "ACPI FADT could not be read: {}",
        std::io::Error::last_os_error()
    );
    let mut bytes = vec![0; length as usize];
    let actual = unsafe {
        GetSystemFirmwareTable(
            1_094_930_505,
            1_346_584_902,
            bytes.as_mut_ptr().cast(),
            length,
        )
    };
    ensure!(actual == length, "ACPI FADT changed during acquisition");
    AcpiTimer::from_fadt(&bytes)
}
fn failed(message: String) -> Readings {
    Readings {
        temperature_celsius: Err(anyhow::anyhow!(message.clone())),
        clock_mhz: Err(anyhow::anyhow!(message)),
    }
}

fn qpc() -> anyhow::Result<i64> {
    let mut value = 0;
    ensure!(
        unsafe { QueryPerformanceCounter(&mut value) } != 0,
        "CPU QPC read failed"
    );
    Ok(value)
}

fn qpc_frequency() -> anyhow::Result<u64> {
    let mut value = 0;
    ensure!(
        unsafe { QueryPerformanceFrequency(&mut value) } != 0 && value > 0,
        "CPU QPC frequency is unavailable"
    );
    Ok(value as u64)
}
fn qpc_wait(ticks: u64) -> anyhow::Result<()> {
    let first = qpc()?;
    let bounded = Instant::now();
    loop {
        let elapsed = qpc()?
            .checked_sub(first)
            .context("CPU QPC interval overflow")?;
        ensure!(elapsed >= 0, "CPU QPC moved backwards");
        if elapsed as u64 >= ticks {
            return Ok(());
        }
        ensure!(
            bounded.elapsed() < Duration::from_millis(100),
            "CPU QPC stopped advancing"
        );
        std::hint::spin_loop();
    }
}
fn intel_clock<T: crate::hardware_driver::Transport>(
    driver: &mut crate::hardware_driver::Driver<T>,
    family: u32,
    model: u32,
    maximum_leaf: u32,
    timer: Result<&AcpiTimer, &anyhow::Error>,
) -> anyhow::Result<f64> {
    use intel_cpu::ClockMethod;
    let method = intel_cpu::clock_method(family, model).context(
        "The current source exposes no verified Intel counter-clock branch for this CPU",
    )?;
    let fixed = (|| {
        ensure!(
            maximum_leaf >= 10,
            "Intel architectural counters are unavailable"
        );
        let counters = __cpuid(10);
        ensure!(
            counters.eax & 255 != 0 && counters.edx & 31 >= 3,
            "Intel fixed counters are unavailable"
        );
        intel_cpu::fixed_counter_clock(
            driver,
            ((counters.eax >> 8) & 255) as u8,
            |driver| match method {
                ClockMethod::Acpi { ticks, .. } => timer
                    .map_err(|error| anyhow::anyhow!("ACPI CPU timer unavailable: {error:#}"))?
                    .wait(driver, ticks),
                ClockMethod::Haswell => match timer {
                    Ok(timer) => timer.wait(
                        driver,
                        (5 * crate::hardware_monitor::acpi_timer::FREQUENCY / 3000) as u32,
                    ),
                    Err(_) => qpc_wait(5 * qpc_frequency()? / 3000),
                },
                ClockMethod::QpcTicks(ticks) => qpc_wait(ticks),
                ClockMethod::QpcMilliseconds(milliseconds) => {
                    qpc_wait(milliseconds * qpc_frequency()? / 1000)
                }
            },
            |driver| match method {
                ClockMethod::Acpi { .. } | ClockMethod::Haswell => match timer {
                    Ok(timer) => timer.measured_tsc(driver),
                    Err(_) => driver.measured_tsc_mhz(),
                },
                _ => driver.measured_tsc_mhz(),
            },
        )
    })();
    match fixed {
        Err(error)
            if matches!(
                method,
                ClockMethod::Acpi {
                    aperf_fallback: true,
                    ..
                }
            ) && intel_cpu::controls_restored(&error) =>
        {
            let fallback = (|| {
                intel_cpu::aperf_clock(
                    driver,
                    *timer.map_err(|error| {
                        anyhow::anyhow!("ACPI CPU timer unavailable: {error:#}")
                    })?,
                    qpc_frequency()?,
                    qpc,
                )
            })();
            fallback
                .with_context(|| format!("Intel fixed counter measurement also failed: {error:#}"))
        }
        result => result,
    }
}
