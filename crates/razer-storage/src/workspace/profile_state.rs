//! Local profile revisions are independent of current connection observations.
use anyhow::ensure;
use razer_model::model::Device;

pub fn same_local_profiles(left: &Device, right: &Device) -> bool {
    left.profiles == right.profiles
        && left.active_profile == right.active_profile
        && left.source_device_settings == right.source_device_settings
        && left.controller_power_saving == right.controller_power_saving
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

#[cfg(test)]
mod tests {
    use super::*;
    use razer_device::controller_sleep::PowerSaving;

    #[test]
    fn controller_sleep_edits_invalidate_captured_local_revisions_but_observations_do_not() {
        let baseline = razer_model::product_preview::registered_preview(2636).unwrap();
        let mut observed = baseline.clone();
        observed.serial_number = "Read from current connection".into();
        observed.dashboard.readonly_power = Some(razer_model::model::PowerStatus {
            level: 73,
            charging_status: "NOT_CHARGING".into(),
        });
        assert!(same_local_profiles(&observed, &baseline));
        ensure_local_profiles_unchanged(&observed, &baseline).unwrap();

        observed.controller_power_saving = Some(PowerSaving::new(15, true).unwrap());
        assert!(!same_local_profiles(&observed, &baseline));
        assert!(
            ensure_local_profiles_unchanged(&observed, &baseline)
                .unwrap_err()
                .to_string()
                .contains("local profiles changed")
        );
    }
}
