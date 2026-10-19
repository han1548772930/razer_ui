//! Local profile revisions are independent of current connection observations.
use anyhow::ensure;
use razer_model::model::Device;

pub fn same_local_profiles(left: &Device, right: &Device) -> bool {
    left.profiles == right.profiles
        && left.active_profile == right.active_profile
        && left.source_device_settings == right.source_device_settings
}

/// A serial can become known during a read. The stable endpoint and local
/// revision must still belong to the request captured by its lifecycle owner.
pub fn ensure_local_profiles_unchanged(device: &Device, baseline: &Device) -> anyhow::Result<()> {
    ensure!(
        device.product_id == baseline.product_id
            && device.device_container_id == baseline.device_container_id,
        "local profile device identity changed during reading"
    );
    ensure!(
        same_local_profiles(device, baseline),
        "local profiles changed during reading"
    );
    Ok(())
}
