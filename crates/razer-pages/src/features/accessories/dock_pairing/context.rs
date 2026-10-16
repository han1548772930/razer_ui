//! Current 241/Ps hs/Cs/Rs/_s, and 164/useLe's validDevices predicate.
//! Profiles here are local drafts; connection evidence is session-only.
use super::*;
use razer_model::model::{DeviceConnectionObservation, SetupStatus};
use std::collections::BTreeSet;

#[derive(Clone, Default, PartialEq)]
pub(super) struct DeviceContext {
    valid: Vec<(u32, u32)>,
    runtime: Vec<RuntimeDevice>,
    polling_profiles: BTreeSet<u32>,
    details: Vec<(u32, u32)>,
    names: Vec<(u32, Option<u32>, String)>,
}
#[derive(Clone, PartialEq)]
struct RuntimeDevice {
    product_id: u32,
    container: String,
    ready: bool,
    sub_devices: Vec<serde_json::Value>,
}

fn present(device: &Device) -> bool {
    matches!(
        device.dashboard.connection_observation,
        Some(DeviceConnectionObservation::UsbPresent | DeviceConnectionObservation::HidPresent)
    ) || matches!(
        device.dashboard.connection_observation,
        Some(DeviceConnectionObservation::ReceiverPeer(1))
    )
}

impl DeviceContext {
    pub(super) fn from_devices(devices: &[Device]) -> Self {
        Self {
            names: devices
                .iter()
                .filter(|device| present(device))
                .map(|device| {
                    (
                        device.product_id,
                        device
                            .source_device_settings
                            .as_ref()
                            .and_then(|settings| settings["dongleId"].as_u64())
                            .and_then(|id| id.try_into().ok()),
                        device
                            .product_name
                            .get(&i18n::locale().to_lowercase())
                            .to_owned(),
                    )
                })
                .collect(),
            valid: devices
                .iter()
                .filter(|device| present(device))
                .map(|device| (device.product_id, device.edition_id))
                .collect(),
            runtime: devices
                .iter()
                .filter(|device| present(device))
                .map(|device| RuntimeDevice {
                    product_id: device.product_id,
                    container: device.device_container_id.clone(),
                    ready: device.setup_status == SetupStatus::Ready,
                    sub_devices: device.sub_devices.clone().unwrap_or_default(),
                })
                .collect(),
            // Cs considers any stored profile once the active profile is
            // checked. Field presence, including null/zero, is authoritative.
            polling_profiles: devices
                .iter()
                .filter(|device| {
                    device.profiles.iter().any(|profile| {
                        profile
                            .source_settings
                            .as_ref()
                            .and_then(serde_json::Value::as_object)
                            .is_some_and(|profile| {
                                ["pollingRate", "pollingRateWireless", "pollingRateBle"]
                                    .iter()
                                    .any(|key| profile.contains_key(*key))
                            })
                    })
                })
                .map(|device| device.product_id)
                .collect(),
            details: devices
                .iter()
                .filter_map(|device| {
                    Some((
                        device.product_id,
                        u32::try_from(
                            device.source_device_settings.as_ref()?["dongleId"].as_u64()?,
                        )
                        .ok()?,
                    ))
                })
                .collect(),
        }
    }
    pub(super) fn valid_devices(&self) -> Vec<(u32, u32)> {
        self.valid.clone()
    }
    pub(super) fn device_name(&self, product_id: u32, dongle_id: Option<u32>) -> Option<String> {
        self.names
            .iter()
            .find(|(pid, radio, _)| {
                *pid == product_id || Some(*pid) == dongle_id || *radio == Some(product_id)
            })
            .map(|(_, _, name)| name.clone())
    }
    pub(super) fn connected(&self, peer: &Peer, dock_product_id: u32) -> bool {
        peer.payload
            .as_ref()
            .is_some_and(|payload| payload["status"].as_u64() == Some(1))
            || self.valid.iter().any(|(id, _)| *id == peer.product_id)
            || self
                .runtime
                .iter()
                .any(|device| device.ready && device.product_id == peer.product_id)
            || self
                .runtime
                .iter()
                .filter(|device| device.ready && device.product_id == dock_product_id)
                .flat_map(|device| &device.sub_devices)
                .any(|device| device["productId"].as_u64() == Some(u64::from(peer.product_id)))
    }
    pub(super) fn has_polling_profile(&self, peer: &Peer) -> bool {
        self.polling_profiles.contains(&peer.product_id)
    }
    pub(super) fn original_dongle(&self, peers: &[&Peer], dock_id: u32) -> bool {
        peers
            .iter()
            .filter(|peer| peer.product_id != dock_id)
            .any(|peer| {
                let matching: Vec<_> = self
                    .details
                    .iter()
                    .filter(|(id, _)| *id == peer.product_id)
                    .collect();
                if matching.iter().any(|(_, dongle)| *dongle == dock_id)
                    && matching.iter().any(|(_, dongle)| *dongle != dock_id)
                {
                    return true;
                }
                let runtime: Vec<_> = self
                    .runtime
                    .iter()
                    .filter(|device| device.ready && device.product_id == peer.product_id)
                    .collect();
                let containers: BTreeSet<_> = runtime
                    .iter()
                    .map(|device| &device.container)
                    .filter(|container| !container.is_empty())
                    .collect();
                if containers.len() >= 2 {
                    return true;
                }
                self.runtime
                    .iter()
                    .filter(|device| device.ready && device.product_id == dock_id)
                    .any(|dock| {
                        dock.sub_devices.iter().any(|sub| {
                            let matches = sub["productId"].as_u64()
                                == Some(u64::from(peer.product_id))
                                || peer
                                    .payload
                                    .as_ref()
                                    .and_then(|payload| payload["serialNumber"].as_str())
                                    .is_some_and(|serial| {
                                        !serial.is_empty()
                                            && sub["serialNumber"].as_str() == Some(serial)
                                    });
                            let container =
                                sub["deviceContainerId"].as_str().unwrap_or(&dock.container);
                            matches
                                && !container.is_empty()
                                && runtime.iter().any(|device| {
                                    !device.container.is_empty() && device.container != container
                                })
                        })
                    })
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn peer(product_id: u32) -> Peer {
        Peer {
            id: product_id.to_string(),
            name: "Observed mouse".into(),
            product_id,
            dongle_id: None,
            edition: None,
            layout: None,
            lane: Lane::Mouse,
            payload: Some(json!({"productId":product_id,"category":"MOUSE"})),
        }
    }

    #[::core::prelude::v1::test]
    fn a_persisted_profile_supports_polling_copy_without_becoming_connected() {
        let mut device = crate::test_devices::mouse();
        device.begin_local_session();
        let peer = peer(device.product_id);
        device.profiles[0].source_settings = Some(json!({"pollingRateWireless":null}));
        let context = DeviceContext::from_devices(&[device.clone()]);
        assert!(context.has_polling_profile(&peer));
        assert!(!context.connected(&peer, 241));
        assert!(context.valid_devices().is_empty());
        device.observe_connection(Some(DeviceConnectionObservation::HidPresent));
        assert!(DeviceContext::from_devices(&[device]).connected(&peer, 241));
    }

    #[::core::prelude::v1::test]
    fn live_dock_subdevices_and_duplicate_containers_follow_separate_predicates() {
        let mut device = crate::test_devices::mouse();
        device.begin_local_session();
        let peer = peer(device.product_id);
        device.observe_connection(Some(DeviceConnectionObservation::UsbPresent));
        device.setup_status = SetupStatus::Ready;
        device.device_container_id = "mouse-owner".into();
        let mut dock = device.clone();
        dock.product_id = 241;
        dock.device_container_id = "dock-owner".into();
        dock.sub_devices = Some(vec![
            json!({"productId":peer.product_id,"deviceContainerId":"dock-owner"}),
        ]);
        let context = DeviceContext::from_devices(&[dock.clone()]);
        assert!(context.connected(&peer, 241));
        assert!(!context.original_dongle(&[&peer], 241));
        assert!(DeviceContext::from_devices(&[device, dock]).original_dongle(&[&peer], 241));
    }
}
