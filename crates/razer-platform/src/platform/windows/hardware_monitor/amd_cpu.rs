//! AMD package selection uses present PCI functions and current driver identity.
use crate::{
    hardware_driver::{Driver, Transport},
    hardware_monitor::amd_cpu::{TemperatureSensor, temperature_offset},
};
use anyhow::{Context as _, ensure};
use std::{mem, ptr};
use windows_sys::Win32::{
    Devices::DeviceAndDriverInstallation::{
        DIGCF_ALLCLASSES, DIGCF_PRESENT, HDEVINFO, SP_DEVINFO_DATA, SPDRP_ADDRESS, SPDRP_BUSNUMBER,
        SetupDiDestroyDeviceInfoList, SetupDiEnumDeviceInfo, SetupDiGetClassDevsW,
        SetupDiGetDeviceInstanceIdW, SetupDiGetDeviceRegistryPropertyW,
    },
    Foundation::{ERROR_NO_MORE_ITEMS, INVALID_HANDLE_VALUE},
    System::Registry::REG_DWORD,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Pci {
    bus: u8,
    device: u8,
    function: u8,
    identity: u32,
}
struct Devices(HDEVINFO);
impl Drop for Devices {
    fn drop(&mut self) {
        unsafe {
            SetupDiDestroyDeviceInfoList(self.0);
        }
    }
}

fn property(devices: &Devices, info: &SP_DEVINFO_DATA, property: u32) -> anyhow::Result<u32> {
    let mut value = 0_u32;
    let mut kind = 0;
    let mut length = 0;
    // SAFETY: all pointers refer to initialized, correctly sized local storage.
    ensure!(
        unsafe {
            SetupDiGetDeviceRegistryPropertyW(
                devices.0,
                info,
                property,
                &mut kind,
                (&mut value as *mut u32).cast(),
                4,
                &mut length,
            )
        } != 0,
        "PCI location property unavailable: {}",
        std::io::Error::last_os_error()
    );
    ensure!(
        kind == REG_DWORD && length == 4,
        "PCI location property has an invalid type"
    );
    Ok(value)
}

fn enumerate() -> anyhow::Result<Vec<Pci>> {
    let pci = [80_u16, 67, 73, 0];
    // SAFETY: a terminated enumerator name; list ownership is retained below.
    let handle = unsafe {
        SetupDiGetClassDevsW(
            ptr::null(),
            pci.as_ptr(),
            ptr::null_mut(),
            DIGCF_ALLCLASSES | DIGCF_PRESENT,
        )
    };
    ensure!(
        handle != INVALID_HANDLE_VALUE as isize as isize,
        "PCI enumeration failed: {}",
        std::io::Error::last_os_error()
    );
    let devices = Devices(handle);
    let mut found = Vec::new();
    for index in 0.. {
        let mut info: SP_DEVINFO_DATA = unsafe { mem::zeroed() };
        info.cbSize = mem::size_of::<SP_DEVINFO_DATA>() as u32;
        if unsafe { SetupDiEnumDeviceInfo(devices.0, index, &mut info) } == 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(ERROR_NO_MORE_ITEMS as i32) {
                break;
            }
            return Err(error.into());
        }
        let mut name = [0_u16; 512];
        ensure!(
            unsafe {
                SetupDiGetDeviceInstanceIdW(
                    devices.0,
                    &info,
                    name.as_mut_ptr(),
                    name.len() as u32,
                    ptr::null_mut(),
                )
            } != 0,
            "PCI device identity unavailable: {}",
            std::io::Error::last_os_error()
        );
        let name = String::from_utf16(
            &name[..name
                .iter()
                .position(|&v| v == 0)
                .context("Unterminated PCI identity")?],
        )?;
        let name = name.to_ascii_uppercase();
        if !name.starts_with("PCI\\VEN_1022&DEV_") {
            continue;
        }
        let device_id =
            u16::from_str_radix(name.get(17..21).context("Truncated AMD PCI identity")?, 16)?;
        if !CANDIDATES.contains(&device_id) {
            continue;
        }
        let bus = property(&devices, &info, SPDRP_BUSNUMBER)?;
        let address = property(&devices, &info, SPDRP_ADDRESS)?;
        let device = address >> 16;
        let function = address & 0xffff;
        ensure!(
            bus <= 255 && device < 32 && function < 8,
            "AMD PCI location is outside the driver protocol"
        );
        found.push(Pci {
            bus: bus as u8,
            device: device as u8,
            function: function as u8,
            identity: (u32::from(device_id) << 16) | 0x1022,
        });
    }
    found.sort();
    found.dedup();
    Ok(found)
}

const CANDIDATES: [u16; 12] = [
    5200, 5248, 5584, 5680, 5336, 5284, 5352, 5383, 5301, 5434, 4386, 4443,
];

/// The monitor consumes CPUInfo[0]. Its package/node index is zero; it never
/// substitutes another package after a failed read (SDK 0x1841d0).
pub(super) struct Temperature {
    target: Pci,
    family: u32,
    model: u32,
    single_node: bool,
    offset: f32,
    sensor: Option<TemperatureSensor>,
}
impl Temperature {
    pub(super) fn open(
        family: u32,
        model: u32,
        brand: &str,
        nodes_per_processor: u32,
    ) -> anyhow::Result<Self> {
        ensure!(
            (1..=8).contains(&nodes_per_processor),
            "AMD node topology was not observed"
        );
        let devices = enumerate()?;
        let single_node = nodes_per_processor == 1;
        let candidates = if single_node {
            &CANDIDATES[..]
        } else {
            &CANDIDATES[..5]
        };
        let target = candidates
            .iter()
            .find_map(|&id| {
                devices
                    .iter()
                    .find(|pci| pci.identity >> 16 == u32::from(id))
            })
            .copied()
            .context("AMD package sensor PCI function is absent")?;
        Ok(Self {
            target,
            family,
            model,
            single_node,
            offset: temperature_offset(family, model, brand),
            sensor: None,
        })
    }

    pub(super) fn sample<T: Transport>(&mut self, driver: &mut Driver<T>) -> anyhow::Result<f64> {
        let mut target = self.target;
        ensure!(
            driver.read_pci_u32(target.bus, target.device, target.function, 0)? == target.identity,
            "AMD package PCI identity changed"
        );
        // SDK 0x191c60 obtains the first package's SMN bus from its data fabric
        // configuration before 0x1841d0 uses the selected function's device ID.
        let register = match (self.family, self.model & 0xf0) {
            (0x17, 0x30) | (0x19, 0 | 0x20) => Some(0x84),
            (0x19, 0x10 | 0x60 | 0x70 | 0xa0) | (0x1a, 0x40) => Some(0xc04),
            _ => None,
        };
        if self.single_node {
            if let Some(register) = register {
                target.bus = driver.read_pci_u32(0, 24, 0, register)? as u8;
            }
        }
        let value = driver.read_smn(target.bus, target.device, target.function, 2, 0x59800)?;
        if self.sensor.is_none() {
            self.sensor = Some(TemperatureSensor::new(
                value,
                self.offset,
                self.single_node,
            )?);
        }
        self.sensor
            .as_ref()
            .context("AMD thermal calibration absent")?
            .decode(value)
    }
}
