//! Device status from current controller84910 -> GET_BATTERY_STATE/w6.
//! No profile/draft fields are changed and none of these observations persist.
use crate::model::{Device, PowerStatus};
use razer_device::controller_input::{Event, Notification, capability};

impl Device {
    pub fn clear_controller_input_observations(&mut self) -> bool {
        self.dashboard.controller_power_status.take().is_some()
    }

    pub fn observe_controller_notification(&mut self, event: &Event) -> bool {
        if self.dashboard.connection_observation.is_none()
            || !capability(self.product_id).is_some_and(|source| source.is_battery_supported())
        {
            return false;
        }
        let power = match event.notification(self.product_id) {
            Some(Notification::Battery(reading)) => PowerStatus {
                level: i32::from(reading.level()),
                charging_status: reading.charging_status().into(),
            },
            Some(Notification::Wireless { connected: false }) => PowerStatus {
                // Source wireless-off publishes the retained level, or zero
                // when there is none. It does not publish an analog input sample.
                level: self.current_power_status().map_or(0, |power| power.level),
                charging_status: "off".into(),
            },
            _ => return false,
        };
        if self
            .dashboard
            .controller_power_status
            .as_ref()
            .is_some_and(|old| {
                old.level == power.level && old.charging_status == power.charging_status
            })
        {
            return false;
        }
        self.dashboard.controller_power_status = Some(power);
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::model::{Device, DeviceConnectionObservation};
    use razer_device::{
        controller_input::{Event, Parser},
        device_reads::DeviceReadValues,
    };
    use serde_json::json;

    fn device(product: u32) -> Device {
        let mut device = crate::test_devices::mouse();
        device.product_id = product;
        device.begin_local_session();
        device.observe_connection(Some(DeviceConnectionObservation::UsbPresent));
        device
    }
    fn event(product: u32, id: u8, value: u8, next: u8) -> Event {
        let record05 = product == 2676;
        let mut frame = vec![0; if record05 { 16 } else { 64 }];
        frame[0] = if record05 { 5 } else { 1 };
        let offset = if record05 { 1 } else { 9 };
        frame[offset] = id;
        frame[offset + 1] = value;
        frame[offset + 2] = next;
        Parser::new(product, product)
            .unwrap()
            .parse(&json!(frame))
            .unwrap()
            .remove(0)
    }

    #[test]
    fn controller_input_power_uses_actual_notifications_without_changing_local_profile_storage() {
        for product in [2636, 2647, 2676, 4133, 4144] {
            let mut device = device(product);
            device.observe_read_values(Some(DeviceReadValues {
                firmware: Some("1.4.0.0".into()),
                ..Default::default()
            }));
            let persisted = serde_json::to_value(&device).unwrap();
            let event = event(product, if product == 2676 { 49 } else { 8 }, 128, 1);
            assert!(device.observe_controller_notification(&event));
            assert!(!device.observe_controller_notification(&event));
            assert_eq!(device.current_power_status().unwrap().level, 50);
            assert_eq!(
                device.current_power_status().unwrap().charging_status,
                "Charging"
            );
            assert_eq!(device.current_firmware_version(), "1.4.0.0");
            assert_eq!(serde_json::to_value(&device).unwrap(), persisted);
            let restored: Device = serde_json::from_value(persisted).unwrap();
            assert!(restored.dashboard.controller_power_status.is_none());
            assert!(device.clear_controller_input_observations());
            assert!(device.current_power_status().is_none());
        }
    }

    #[test]
    fn controller_input_disconnect_and_new_connection_expire_observations() {
        let mut device = device(2636);
        assert!(device.observe_controller_notification(&event(2636, 8, 128, 0)));
        assert!(device.observe_controller_notification(&event(2636, 9, 2, 0)));
        assert_eq!(device.current_power_status().unwrap().level, 50);
        assert_eq!(
            device.current_power_status().unwrap().charging_status,
            "off"
        );
        // A connected event schedules source refresh; it cannot manufacture a
        // new charging state or pretend that a subsequent query succeeded.
        assert!(!device.observe_controller_notification(&event(2636, 9, 3, 0)));
        assert_eq!(
            device.current_power_status().unwrap().charging_status,
            "off"
        );
        device.observe_connection(Some(DeviceConnectionObservation::HidPresent));
        assert!(device.current_power_status().is_none());
        assert!(device.observe_controller_notification(&event(2636, 8, 255, 0)));
        device.observe_connection(None);
        assert!(device.current_power_status().is_none());
        assert!(!device.observe_controller_notification(&event(2636, 8, 255, 0)));
    }

    #[test]
    fn controller_input_notifications_cannot_give_a_preview_or_wired_product_a_battery() {
        let mut preview = device(2636);
        preview.begin_local_session();
        assert!(!preview.observe_controller_notification(&event(2636, 8, 255, 0)));
        let mut wired = device(2629);
        assert!(!wired.observe_controller_notification(&event(2629, 8, 255, 0)));
        assert!(wired.current_power_status().is_none());
    }
}
