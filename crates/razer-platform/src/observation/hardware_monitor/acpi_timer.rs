//! Current SDK 0xc2de0, 0x61f10 and 0x61dc0 ACPI measurement timer.
use crate::hardware_driver::{Driver, Transport};
use anyhow::ensure;
use std::time::{Duration, Instant};

pub const FREQUENCY: u64 = 3_579_545;

#[derive(Clone, Copy)]
pub struct AcpiTimer { port: u16 }
impl AcpiTimer {
    pub fn from_fadt(bytes: &[u8]) -> anyhow::Result<Self> {
        ensure!(bytes.len() >= 80 && &bytes[..4] == b"FACP", "ACPI FADT is missing or truncated");
        let length = u32::from_le_bytes(bytes[4..8].try_into()?) as usize;
        ensure!(length >= 80 && length <= bytes.len(), "ACPI FADT length is invalid");
        ensure!(bytes[..length].iter().fold(0_u8, |sum, &byte| sum.wrapping_add(byte)) == 0, "ACPI FADT checksum is invalid");
        ensure!(bytes[8] < 3 || length < 300, "ACPI timer branch is unavailable for this FADT revision");
        let port = u32::from_le_bytes(bytes[76..80].try_into()?);
        ensure!(port != 0 && port <= u32::from(u16::MAX), "ACPI timer port is unavailable");
        Ok(Self { port: port as u16 })
    }
    pub fn read<T: Transport>(&self, driver: &mut Driver<T>) -> anyhow::Result<u32> {
        Ok(driver.read_port_u32(self.port)? & 0x00ff_ffff)
    }
    pub fn wait<T: Transport>(&self, driver: &mut Driver<T>, ticks: u32) -> anyhow::Result<()> {
        ensure!(ticks < 0x0080_0000, "ACPI measurement interval exceeds the wrapping counter");
        let first = self.read(driver)?;
        let started = Instant::now();
        loop {
            let current = self.read(driver)?;
            if current.wrapping_sub(first) & 0x00ff_ffff >= ticks { return Ok(()); }
            ensure!(started.elapsed() < Duration::from_millis(100), "ACPI measurement timer stopped advancing");
            std::hint::spin_loop();
        }
    }
}
