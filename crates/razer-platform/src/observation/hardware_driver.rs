//! The local hardware sensor driver's byte protocol. No SDK DLL is loaded.
use anyhow::{Context as _, ensure};

mod installation {
    include!(concat!(env!("OUT_DIR"), "/hardware_driver_installation.rs"));
}
pub use installation::{
    DEVICE_PATH, DRIVER_DIRECTORY, DRIVER_FILENAME, MUTEX_NAME, SERVICE_NAME,
    SYSTEM_DRIVER_DIRECTORY,
};

/// A single synchronous driver operation. The owner must hold the driver's
/// process-shared mutex and keep the target processor affinity until it returns.
pub trait Transport {
    fn ioctl(&mut self, code: u32, input: &[u8], output: &mut [u8]) -> anyhow::Result<usize>;
}

/// Owns the transport; failed calls never leave a cached sensor observation.
pub struct Driver<T> {
    transport: T,
}

impl<T: Transport> Driver<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    pub fn into_transport(self) -> T {
        self.transport
    }

    fn read<const N: usize>(&mut self, code: u32, input: &[u8]) -> anyhow::Result<[u8; N]> {
        let mut output = [0; N];
        let actual = self.transport.ioctl(code, input, &mut output)?;
        ensure!(
            actual == N,
            "Hardware driver reply length is {actual}, expected {N} for {code:#010x}"
        );
        Ok(output)
    }

    /// SDK RVA 0x15860: the version comes from the running driver's response.
    pub fn protocol_version(&mut self) -> anyhow::Result<u32> {
        let version = u32::from_le_bytes(self.read(0x9c40_2400, &[])?);
        ensure!(
            version != u32::MAX,
            "Hardware driver returned no protocol version"
        );
        Ok(version)
    }

    /// SDK RVA 0x14e58: wire order is low dword followed by high dword;
    /// the C++ getter reverses its two out-pointer arguments.
    pub fn read_msr(&mut self, register: u32) -> anyhow::Result<u64> {
        let value = u64::from_le_bytes(self.read(0x9c40_2440, &register.to_le_bytes())?);
        ensure!(value != u64::MAX, "MSR {register:#x} was not observed");
        Ok(value)
    }

    /// SDK 0x15e68/0x15db8: the address identifies a PCI function, followed
    /// by the SMN access method and register. Driver failure words are not data.
    pub fn read_smn(
        &mut self,
        bus: u8,
        device: u8,
        function: u8,
        access_method: u32,
        register: u32,
    ) -> anyhow::Result<u32> {
        ensure!(device < 32 && function < 8, "Invalid SMN PCI function");
        let address = 0x8000_0000
            | (u32::from(bus) << 16)
            | (u32::from(device) << 11)
            | (u32::from(function) << 8);
        let mut request = [0; 12];
        request[..4].copy_from_slice(&address.to_le_bytes());
        request[4..8].copy_from_slice(&access_method.to_le_bytes());
        request[8..].copy_from_slice(&register.to_le_bytes());
        let value = u32::from_le_bytes(self.read(0x9c40_2584, &request)?);
        ensure!(
            !matches!(value, 0xaaaa_aaaa | 0xbbbb_bbbb | 0xcccc_cccc),
            "SMN register {register:#x} was not observed ({value:#x})"
        );
        Ok(value)
    }

    /// SDK 0x14048: extended PCI configuration dword response includes its
    /// byte count. An all-ones value is retained for the caller's identity check.
    pub fn read_pci_u32(
        &mut self,
        bus: u8,
        device: u8,
        function: u8,
        offset: u16,
    ) -> anyhow::Result<u32> {
        ensure!(
            device < 32 && function < 8 && offset <= 4092 && offset % 4 == 0,
            "Invalid PCI configuration address"
        );
        let mut request = [0; 16];
        for (chunk, value) in request.chunks_exact_mut(4).zip([
            u32::from(bus),
            u32::from(device),
            u32::from(function),
            u32::from(offset),
        ]) {
            chunk.copy_from_slice(&value.to_le_bytes());
        }
        let response = self.read::<8>(0x9c40_2720, &request)?;
        ensure!(
            u32::from_le_bytes(response[..4].try_into()?) == 4,
            "PCI register was not observed"
        );
        Ok(u32::from_le_bytes(response[4..].try_into()?))
    }

    /// SDK RVA 0x14f48 sends register, high word, then low word. A successful
    /// IOCTL with the driver's -1 reply is still a rejected write.
    pub fn write_msr(&mut self, register: u32, value: u64) -> anyhow::Result<()> {
        let mut input = [0; 12];
        input[..4].copy_from_slice(&register.to_le_bytes());
        input[4..8].copy_from_slice(&((value >> 32) as u32).to_le_bytes());
        input[8..].copy_from_slice(&(value as u32).to_le_bytes());
        let reply = self.read::<8>(0x9c40_2444, &input)?;
        ensure!(
            u32::from_le_bytes(reply[..4].try_into()?) != u32::MAX,
            "Hardware driver rejected write to MSR {register:#x}"
        );
        Ok(())
    }

    /// SDK RVA 0x15a58 with register 0x10, as used by 0x13f820 and
    /// 0x152230. This is a driver measurement, not CPUID's nominal frequency.
    pub fn measured_tsc_mhz(&mut self) -> anyhow::Result<f32> {
        let bytes: [u8; 8] = self.read(0x9c40_2608, &0x10_u32.to_le_bytes())?;
        let high = u32::from_le_bytes(bytes[..4].try_into().expect("fixed dword"));
        let low = u32::from_le_bytes(bytes[4..].try_into().expect("fixed dword"));
        let frequency = ((u64::from(high) << 32) | u64::from(low)) as i64;
        let mhz = (frequency / 1_000_000) as i32 as f32;
        ensure!(
            mhz > 0.0,
            "The hardware driver did not measure a positive TSC frequency"
        );
        Ok(mhz)
    }

    /// SDK 0x15c9c retries its ACPI measurement only after the primary driver
    /// counter fails. The alternate reply's high DWORD is an IEEE-754 float.
    pub fn measured_tsc_mhz_with_acpi(&mut self, port: u16) -> anyhow::Result<f32> {
        match self.measured_tsc_mhz() {
            Ok(value) => Ok(value),
            Err(primary) => (|| {
                let mut input = [0; 8];
                input[..4].copy_from_slice(&u32::from(port).to_le_bytes());
                input[4..].copy_from_slice(&0x10_u32.to_le_bytes());
                let reply = self.read::<8>(0x9c40_2600, &input)?;
                let ratio = f32::from_bits(u32::from_le_bytes(reply[4..].try_into()?));
                let value =
                    (3_579_545_f64 / 1_000_000.0) as f32 * (f64::from(ratio) / 3.579545) as f32;
                ensure!(
                    value.is_finite() && value > 0.0,
                    "ACPI TSC driver measurement is invalid"
                );
                Ok(value)
            })()
            .with_context(|| format!("Primary TSC measurement also failed: {primary:#}")),
        }
    }

    /// SDK RVAs 0x14a00, 0x14ac4 and 0x14b90 return eight wire bytes for
    /// each port width; only the requested low bytes are the register value.
    pub fn read_port_u8(&mut self, port: u16) -> anyhow::Result<u8> {
        Ok(self.read::<8>(0x9c40_2480, &u32::from(port).to_le_bytes())?[0])
    }

    pub fn read_port_u16(&mut self, port: u16) -> anyhow::Result<u16> {
        let bytes = self.read::<8>(0x9c40_2484, &u32::from(port).to_le_bytes())?;
        Ok(u16::from_le_bytes(
            bytes[..2].try_into().expect("fixed word"),
        ))
    }

    pub fn read_port_u32(&mut self, port: u16) -> anyhow::Result<u32> {
        let bytes = self.read::<8>(0x9c40_2488, &u32::from(port).to_le_bytes())?;
        Ok(u32::from_le_bytes(
            bytes[..4].try_into().expect("fixed dword"),
        ))
    }
}

/// The packaged driver must be delivered by the installation lifecycle. Sensor
/// discovery never searches source trees, analysis inputs or downloaded SDKs.
pub fn packaged_driver(executable: &std::path::Path) -> anyhow::Result<std::path::PathBuf> {
    ensure!(
        executable.is_absolute(),
        "The installation executable path must be absolute"
    );
    Ok(executable
        .parent()
        .context("Executable has no installation directory")?
        .join(DRIVER_DIRECTORY)
        .join(DRIVER_FILENAME))
}

#[cfg(target_os = "windows")]
pub use crate::platform::windows::hardware_driver::{Connection, Processor};

/// Explicit elevated installation action; never called by discovery or reads.
pub fn install_from_application_directory() -> anyhow::Result<()> {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return crate::platform::windows::hardware_driver::install_from_application_directory();
    #[cfg(not(all(target_os = "windows", target_arch = "x86_64")))]
    anyhow::bail!(
        "The current hardware sensor driver has no native implementation for this platform"
    )
}

/// Remove only the service and protected driver file owned by this application.
pub fn uninstall_owned_installation() -> anyhow::Result<()> {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return crate::platform::windows::hardware_driver::uninstall_owned_installation();
    #[cfg(not(all(target_os = "windows", target_arch = "x86_64")))]
    anyhow::bail!(
        "The current hardware sensor driver has no native implementation for this platform"
    )
}

#[cfg(test)]
#[path = "hardware_driver_tests.rs"]
mod tests;
