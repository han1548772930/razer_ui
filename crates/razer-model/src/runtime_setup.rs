//! Current MW runtimeData/setupOnDeviceReady producers, queried by
//! `tools/workspace.py audit-runtime-setup`. Runtime readiness precedes profile
//! initialization and is independent of device read/write completion.
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Deserialize)]
struct Policy {
    product_id: u32,
    ready_before_profile_initialization: bool,
    #[serde(default = "waiting")]
    initial_setup_status: crate::model::SetupStatus,
}

fn waiting() -> crate::model::SetupStatus {
    crate::model::SetupStatus::Waiting
}

fn policy(product_id: u32) -> Option<&'static Policy> {
    static POLICIES: OnceLock<Vec<Policy>> = OnceLock::new();
    POLICIES
        .get_or_init(|| crate::data::runtime_setup_data!())
        .iter()
        .find(|policy| {
            policy.product_id == product_id && policy.ready_before_profile_initialization
        })
}

pub(crate) fn initial_setup_status(product_id: u32) -> Option<crate::model::SetupStatus> {
    policy(product_id).map(|policy| policy.initial_setup_status)
}

#[cfg(test)]
mod startup_state_tests {
    use crate::model::{Device, DeviceConnectionObservation, SetupStatus};
    use razer_device::device_reads::DeviceReadValues;

    #[test]
    fn runtime_ready_is_distinct_from_profile_and_device_io_completion() {
        let mut device = crate::test_devices::mouse();
        device.begin_local_session();
        let profiles = serde_json::to_value(&device.profiles).unwrap();
        device.observe_connection(Some(DeviceConnectionObservation::ReceiverPeer(1)));
        assert_eq!(device.setup_status, SetupStatus::Waiting);
        assert!(device.current_power_status().is_none());
        device.observe_read_values(Some(DeviceReadValues {
            battery_percent: Some(89),
            charging_status: Some("NoCharge_BatteryFull".into()),
            ..Default::default()
        }));
        assert_eq!(device.setup_status, SetupStatus::Waiting);
        device.mount_local_runtime();
        assert_eq!(device.setup_status, SetupStatus::Ready);
        assert_eq!(device.current_power_status().unwrap().level, 89);
        assert_eq!(serde_json::to_value(&device.profiles).unwrap(), profiles);
        device.observe_read_values(Some(DeviceReadValues::default()));
        assert_eq!(device.setup_status, SetupStatus::Ready);
        assert!(device.current_power_status().is_none());
        device.observe_connection(None);
        assert_eq!(device.setup_status, SetupStatus::Unknown);
        assert!(device.dashboard.readonly_values.is_none());
        device.observe_connection(Some(DeviceConnectionObservation::ReceiverPeer(1)));
        assert_eq!(device.setup_status, SetupStatus::Ready);
        assert!(device.current_power_status().is_none());
    }

    #[test]
    fn saved_ready_status_cannot_bootstrap_a_new_session() {
        let mut device = crate::test_devices::mouse();
        device.mount_local_runtime();
        device.observe_connection(Some(DeviceConnectionObservation::UsbPresent));
        let mut restored: Device =
            serde_json::from_value(serde_json::to_value(&device).unwrap()).unwrap();
        assert!(!restored.dashboard.local_runtime_mounted);
        restored.begin_local_session();
        assert_eq!(restored.setup_status, SetupStatus::Unknown);
        restored.observe_connection(Some(DeviceConnectionObservation::UsbPresent));
        assert_eq!(restored.setup_status, SetupStatus::Waiting);
    }

    #[test]
    fn successful_serial_query_keeps_the_product_container_owner() {
        let mut device = crate::test_devices::mouse();
        device.serial_number.clear();
        let owner = device.workspace_identity();
        device.serial_number = "SOURCE-READ-SERIAL".into();
        assert_eq!(device.workspace_identity(), owner);
        device.device_container_id.push_str("-other");
        assert_ne!(device.workspace_identity(), owner);
        device.device_container_id.clear();
        let preview = device.workspace_identity();
        device.serial_number = "PREVIEW-OTHER".into();
        assert_ne!(device.workspace_identity(), preview);
    }

    #[test]
    fn original_unknown_bootstrap_variants_are_preserved_until_module_mount() {
        for product_id in [117, 2594] {
            let mut device = crate::test_devices::mouse();
            device.product_id = product_id;
            device.begin_local_session();
            device.observe_connection(Some(DeviceConnectionObservation::UsbPresent));
            assert_eq!(device.setup_status, SetupStatus::Unknown);
            device.mount_local_runtime();
            assert_eq!(device.setup_status, SetupStatus::Ready);
            assert!(device.current_power_status().is_none());
        }
    }
}
