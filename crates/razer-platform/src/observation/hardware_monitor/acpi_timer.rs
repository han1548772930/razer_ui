//! Current SDK 0xc2de0, 0x61f10 and 0x61dc0 ACPI measurement timer.
use crate::hardware_driver::{Driver, Transport};
use anyhow::ensure;
use std::time::{Duration, Instant};

pub const FREQUENCY: u64 = 3_579_545;

#[derive(Clone, Copy)]
pub struct AcpiTimer {
    port: u16,
}
impl AcpiTimer {
    pub fn from_fadt(bytes: &[u8]) -> anyhow::Result<Self> {
        ensure!(
            bytes.len() >= 80 && &bytes[..4] == b"FACP",
            "ACPI FADT is missing or truncated"
        );
        let length = u32::from_le_bytes(bytes[4..8].try_into()?) as usize;
        ensure!(
            length >= 80 && length <= bytes.len(),
            "ACPI FADT length is invalid"
        );
        ensure!(
            bytes[..length]
                .iter()
                .fold(0_u8, |sum, &byte| sum.wrapping_add(byte))
                == 0,
            "ACPI FADT checksum is invalid"
        );
        ensure!(
            bytes[8] < 3 || length < 300,
            "ACPI timer branch is unavailable for this FADT revision"
        );
        let port = u32::from_le_bytes(bytes[76..80].try_into()?);
        ensure!(
            port != 0 && port <= u32::from(u16::MAX),
            "ACPI timer port is unavailable"
        );
        Ok(Self { port: port as u16 })
    }
    pub fn measured_tsc<T: Transport>(&self, driver: &mut Driver<T>) -> anyhow::Result<f32> {
        driver.measured_tsc_mhz_with_acpi(self.port)
    }
    pub fn read<T: Transport>(&self, driver: &mut Driver<T>) -> anyhow::Result<u32> {
        Ok(driver.read_port_u32(self.port)? & 0x00ff_ffff)
    }
    pub fn wait<T: Transport>(&self, driver: &mut Driver<T>, ticks: u32) -> anyhow::Result<()> {
        ensure!(
            ticks < 0x0080_0000,
            "ACPI measurement interval exceeds the wrapping counter"
        );
        let first = self.read(driver)?;
        let started = Instant::now();
        loop {
            let current = self.read(driver)?;
            if current.wrapping_sub(first) & 0x00ff_ffff >= ticks {
                return Ok(());
            }
            ensure!(
                started.elapsed() < Duration::from_millis(100),
                "ACPI measurement timer stopped advancing"
            );
            std::hint::spin_loop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn table() -> Vec<u8> {
        let mut bytes = vec![0; 116];
        bytes[..4].copy_from_slice(b"FACP");
        bytes[4..8].copy_from_slice(&116_u32.to_le_bytes());
        bytes[8] = 1;
        bytes[76..80].copy_from_slice(&0x408_u32.to_le_bytes());
        bytes[9] = 0_u8.wrapping_sub(bytes.iter().fold(0_u8, |sum, &byte| sum.wrapping_add(byte)));
        bytes
    }
    #[test]
    fn firmware_length_checksum_and_port_must_be_observed() {
        let bytes = table();
        assert_eq!(AcpiTimer::from_fadt(&bytes).unwrap().port, 0x408);
        assert!(AcpiTimer::from_fadt(&bytes[..79]).is_err());
        let mut invalid = bytes;
        invalid[9] ^= 1;
        assert!(AcpiTimer::from_fadt(&invalid).is_err());
    }
    struct Ports(std::collections::VecDeque<u32>);
    impl Transport for Ports {
        fn ioctl(&mut self, code: u32, input: &[u8], output: &mut [u8]) -> anyhow::Result<usize> {
            assert_eq!(code, 0x9c402488);
            assert_eq!(input, [8, 4, 0, 0]);
            output.fill(0);
            output[..4].copy_from_slice(
                &self
                    .0
                    .pop_front()
                    .expect("unexpected timer read")
                    .to_le_bytes(),
            );
            Ok(8)
        }
    }
    #[test]
    fn wrapping_timer_counts_use_the_24_bit_hardware_domain() {
        let timer = AcpiTimer::from_fadt(&table()).unwrap();
        let mut driver = Driver::new(Ports(std::collections::VecDeque::from([
            0x00fff900, 0x00000500,
        ])));
        timer.wait(&mut driver, 2500).unwrap();
        assert!(driver.into_transport().0.is_empty());
    }
}
