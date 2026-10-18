//! Independent local Blade reports from Windows system interfaces.
use anyhow::ensure;
use razer_device::system_info::{Query, Response, layout};

#[cfg(windows)]
#[path = "help_system_info/devices.rs"]
mod devices;
#[cfg(windows)]
#[path = "help_system_info/windows.rs"]
mod platform;
#[cfg(windows)]
#[path = "help_system_info/registry.rs"]
mod registry;
#[cfg(any(windows, test))]
#[path = "help_system_info/semantics.rs"]
mod semantics;
#[cfg(windows)]
#[path = "help_system_info/wmi.rs"]
mod wmi;

pub fn query(product_id: u32, query: Query) -> anyhow::Result<Response> {
    ensure!(
        layout(product_id).is_some(),
        "Product has no current Blade report caller"
    );
    platform::query(product_id, query)
}

#[cfg(not(windows))]
mod platform {
    use super::*;
    pub(super) fn query(_: u32, _: Query) -> anyhow::Result<Response> {
        anyhow::bail!("Local Blade reports require the Windows system adapter")
    }
}

#[cfg(test)]
#[path = "help_system_info/tests.rs"]
mod tests;
