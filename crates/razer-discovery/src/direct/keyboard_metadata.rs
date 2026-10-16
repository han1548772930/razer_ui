use super::DirectRoute;
use crate::discovery::{ObservedDevice, ObservedTransport};
use anyhow::{Context as _, ensure};
use razer_device::keyboard_metadata::KeyboardMetadataReading;
use razer_ipc::{ServiceClient, ServiceRequest};

pub enum Route {
    Hid(DirectRoute),
    RelayCollection {
        node: razer_device::backend::HidNode,
        product_id: u32,
        peer_product_id: u32,
    },
    #[cfg(windows)]
    Ble(razer_device::mouse_runtime_artwork::WindowsBleTarget),
}

pub fn resolve(client: &mut ServiceClient, observed: &ObservedDevice) -> anyhow::Result<Route> {
    use razer_device::keyboard_metadata;
    if matches!(observed.transport(), Some(ObservedTransport::Ble)) {
        ensure!(
            observed.peer_product_id().is_none()
                && keyboard_metadata::ble_product_id(observed.product_id())
                    == Some(observed.physical_product_id()),
            "Keyboard BLE identity differs from its source"
        );
        #[cfg(windows)]
        {
            let hid = client.request(ServiceRequest::HidDevices)?;
            return Ok(Route::Ble(
                razer_device::mouse_runtime_artwork::WindowsBleTarget::from_hid(
                    observed.product_id(),
                    observed.physical_product_id(),
                    observed.container(),
                    &hid,
                )?,
            ));
        }
        #[cfg(not(windows))]
        anyhow::bail!("Keyboard BLE metadata requires a platform GATT adapter");
    }
    let capability = match keyboard_metadata::hid_capability(
        observed.product_id(),
        observed.physical_product_id(),
    ) {
        Some(capability) => capability.clone(),
        None if observed.peer_product_id().is_some() => keyboard_metadata::relay_capability(
            observed.product_id(),
            observed.physical_product_id(),
        )?,
        None => anyhow::bail!("Keyboard physical metadata route is not source-audited"),
    };
    ensure!(
        matches!(
            observed.transport(),
            Some(ObservedTransport::Wired | ObservedTransport::Dongle)
        ),
        "Keyboard metadata cannot use an unrelated receiver peer"
    );
    ensure!(
        !matches!(observed.connection(), razer_model::model::DeviceConnectionObservation::ReceiverPeer(status) if status != 1),
        "Offline keyboard peer is not a metadata read target"
    );
    if let Some(peer) = observed.peer_product_id() {
        ensure!(
            matches!(
                observed.connection(),
                razer_model::model::DeviceConnectionObservation::ReceiverPeer(1)
            ),
            "Keyboard relay lacks an online peer observation"
        );
        keyboard_metadata::validate_peer(
            observed.product_id(),
            peer,
            &[(u16::try_from(peer)?, 1)],
        )?;
    }
    if let Some(node) = observed.hid_node() {
        ensure!(
            node.vendor_id == capability.vendor_id
                && u32::from(node.product_id) == observed.physical_product_id()
                && node.interface_number
                    == i32::from(capability.claim_interface_for(observed.physical_product_id())?),
            "Keyboard metadata collection interface differs"
        );
        let current = client.request(ServiceRequest::HidNodes)?;
        let nodes: Vec<razer_device::backend::HidNode> =
            serde_json::from_value(current["nodes"].clone())?;
        ensure!(
            nodes.iter().filter(|current| *current == node).count() == 1,
            "Keyboard metadata collection changed or is ambiguous"
        );
        super::source_reports_match(client, node, capability.report_bytes)?;
        if let Some(peer_product_id) = observed.peer_product_id() {
            return Ok(Route::RelayCollection {
                node: node.clone(),
                product_id: observed.product_id(),
                peer_product_id,
            });
        }
        return Ok(Route::Hid(DirectRoute::Collection {
            node: node.clone(),
            product_id: observed.product_id(),
        }));
    }
    #[cfg(windows)]
    {
        let mut target = super::windows::resolve(client, observed, &capability)?;
        target.peer_product_id = observed.peer_product_id();
        Ok(Route::Hid(DirectRoute::Container(target)))
    }
    #[cfg(not(windows))]
    anyhow::bail!("Keyboard ContainerId metadata requires its platform HID adapter")
}

impl Route {
    pub fn read_metadata(
        &self,
        client: &mut ServiceClient,
    ) -> anyhow::Result<KeyboardMetadataReading> {
        match self {
            Self::Hid(route) => route.read_keyboard_metadata(client),
            Self::RelayCollection {
                node,
                product_id,
                peer_product_id,
            } => {
                let reply = client.request(ServiceRequest::HidNodeKeyboardRelayMetadataRead {
                    node: node.clone(),
                    product_id: *product_id,
                    peer_product_id: *peer_product_id,
                })?;
                self.check_reply(&reply)?;
                ensure!(
                    reply["method"] == "getEdition",
                    "Keyboard relay metadata method differs"
                );
                Ok(serde_json::from_value(reply["reading"].clone())?)
            }
            #[cfg(windows)]
            Self::Ble(target) => {
                let reply = client.request(ServiceRequest::BleKeyboardMetadataRead {
                    target: target.clone(),
                })?;
                ensure!(
                    reply["target"] == serde_json::to_value(target)?
                        && reply["method"] == "getEdition",
                    "Keyboard BLE metadata response identity differs"
                );
                Ok(serde_json::from_value(reply["reading"].clone())?)
            }
        }
    }

    pub fn read_serial(&self, client: &mut ServiceClient) -> anyhow::Result<String> {
        self.read_serial_with(|request| client.request(request))
    }

    fn check_reply(&self, reply: &serde_json::Value) -> anyhow::Result<()> {
        match self {
            Self::Hid(route) => route.check_reply(reply),
            Self::RelayCollection {
                node,
                product_id,
                peer_product_id,
            } => {
                ensure!(
                    reply["node"] == serde_json::to_value(node)?
                        && reply["product_id"] == *product_id
                        && reply["peer_product_id"] == *peer_product_id,
                    "Keyboard relay response identity differs"
                );
                Ok(())
            }
            #[cfg(windows)]
            Self::Ble(target) => {
                ensure!(
                    reply["target"] == serde_json::to_value(target)?,
                    "Keyboard BLE response identity differs"
                );
                Ok(())
            }
        }
    }

    fn read_serial_with(
        &self,
        mut request: impl FnMut(ServiceRequest) -> anyhow::Result<serde_json::Value>,
    ) -> anyhow::Result<String> {
        let message = match self {
            Self::RelayCollection {
                node,
                product_id,
                peer_product_id,
            } => ServiceRequest::HidNodeKeyboardRelaySerialRead {
                node: node.clone(),
                product_id: *product_id,
                peer_product_id: *peer_product_id,
            },
            Self::Hid(DirectRoute::Collection { node, product_id }) => {
                ServiceRequest::HidNodeKeyboardSerialRead {
                    node: node.clone(),
                    product_id: *product_id,
                }
            }
            #[cfg(windows)]
            Self::Hid(DirectRoute::Container(target)) => ServiceRequest::DeviceKeyboardSerialRead {
                target: target.clone(),
            },
            #[cfg(windows)]
            Self::Ble(target) => ServiceRequest::BleKeyboardSerialRead {
                target: target.clone(),
            },
        };
        let reply = request(message)?;
        self.check_reply(&reply)?;
        ensure!(
            reply["method"] == "getSerialNumber",
            "Keyboard serial response method differs"
        );
        reply["serial_number"]
            .as_str()
            .map(str::to_owned)
            .context("Keyboard serial response is absent")
    }
}

impl DirectRoute {
    pub fn read_keyboard_metadata(
        &self,
        client: &mut ServiceClient,
    ) -> anyhow::Result<KeyboardMetadataReading> {
        self.read_keyboard_metadata_with(|request| client.request(request))
    }

    fn read_keyboard_metadata_with(
        &self,
        mut request: impl FnMut(ServiceRequest) -> anyhow::Result<serde_json::Value>,
    ) -> anyhow::Result<KeyboardMetadataReading> {
        let message = match self {
            Self::Collection { node, product_id } => ServiceRequest::HidNodeKeyboardMetadataRead {
                node: node.clone(),
                product_id: *product_id,
            },
            #[cfg(windows)]
            Self::Container(target) => ServiceRequest::DeviceKeyboardMetadataRead {
                target: target.clone(),
            },
        };
        let reply = request(message)?;
        self.check_reply(&reply)?;
        ensure!(
            reply["method"] == "getEdition",
            "Keyboard metadata reply method differs"
        );
        Ok(serde_json::from_value(reply["reading"].clone())?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use razer_device::backend::HidNode;
    use serde_json::json;

    fn route() -> DirectRoute {
        DirectRoute::Collection {
            product_id: 756,
            node: HidNode {
                path: b"observed-keyboard".to_vec(),
                vendor_id: 0x1532,
                product_id: 756,
                interface_number: 2,
                usage_page: 0xff00,
                usage: 1,
                release_number: 0,
                serial_number: None,
                manufacturer: None,
                product: None,
            },
        }
    }

    #[test]
    fn keyboard_metadata_query_accepts_only_its_current_reply_scope() {
        let route = route();
        let reading = route
            .read_keyboard_metadata_with(|message| {
                let ServiceRequest::HidNodeKeyboardMetadataRead { node, product_id } = message
                else {
                    panic!("not a keyboard metadata request");
                };
                Ok(
                    json!({"node": node, "product_id": product_id, "method": "getEdition",
                "reading": {"keyboard_layout": 3, "edition_id": 128, "firmware_id": 7}}),
                )
            })
            .unwrap();
        assert_eq!(reading.keyboard_layout, 3);
        assert_eq!(reading.edition_id, 128);
        assert_eq!(reading.firmware_id, Some(7));
        assert!(
            route
                .read_keyboard_metadata_with(|message| {
                    let ServiceRequest::HidNodeKeyboardMetadataRead { node, .. } = message else {
                        unreachable!();
                    };
                    Ok(
                        json!({"node": node, "product_id": 679, "method": "getEdition",
                "reading": {"keyboard_layout": 1, "edition_id": 0}}),
                    )
                })
                .is_err()
        );
    }

    #[test]
    fn keyboard_metadata_failure_or_incomplete_payload_never_becomes_layout_zero() {
        let route = route();
        assert!(
            route
                .read_keyboard_metadata_with(|_| anyhow::bail!("read failed"))
                .is_err()
        );
        for method in ["getEdition", "getFirmwareVersion"] {
            assert!(
                route
                    .read_keyboard_metadata_with(|message| {
                        let ServiceRequest::HidNodeKeyboardMetadataRead { node, product_id } =
                            message
                        else {
                            unreachable!();
                        };
                        Ok(
                            json!({"node": node, "product_id": product_id, "method": method,
                    "reading": {"edition_id": 0}}),
                        )
                    })
                    .is_err()
            );
        }
    }

    #[test]
    fn keyboard_serial_response_keeps_source_text_and_rejects_unrelated_or_missing_reads() {
        let route = super::Route::Hid(route());
        let queried = route.read_serial_with(|request| {
            let ServiceRequest::HidNodeKeyboardSerialRead { node, product_id } = request else {
                panic!("wrong serial request");
            };
            Ok(json!({"node":node,"product_id":product_id,"method":"getSerialNumber","serial_number":" SOURCE SERIAL "}))
        }).unwrap();
        assert_eq!(queried, " SOURCE SERIAL ");
        for payload in [json!(null), json!(42)] {
            assert!(route.read_serial_with(|request| {
                let ServiceRequest::HidNodeKeyboardSerialRead { node, product_id } = request else { unreachable!() };
                Ok(json!({"node":node,"product_id":product_id,"method":"getSerialNumber","serial_number":payload}))
            }).is_err());
        }
        assert!(route.read_serial_with(|request| {
            let ServiceRequest::HidNodeKeyboardSerialRead { node, .. } = request else { unreachable!() };
            Ok(json!({"node":node,"product_id":691,"method":"getSerialNumber","serial_number":"unrelated"}))
        }).is_err());
    }

    #[test]
    fn keyboard_relay_serial_requires_the_retained_raw_peer_identity() {
        let DirectRoute::Collection { mut node, .. } = route() else {
            unreachable!()
        };
        node.product_id = 179;
        let route = super::Route::RelayCollection {
            node,
            product_id: 756,
            peer_product_id: 757,
        };
        for returned_peer in [757, 756, 183] {
            let result = route.read_serial_with(|request| {
                let ServiceRequest::HidNodeKeyboardRelaySerialRead {
                    node,
                    product_id,
                    peer_product_id,
                } = request
                else {
                    panic!("wrong relay request");
                };
                assert_eq!(peer_product_id, 757);
                Ok(
                    json!({"node":node,"product_id":product_id,"peer_product_id":returned_peer,
                    "method":"getSerialNumber","serial_number":"queried-peer"}),
                )
            });
            assert_eq!(result.is_ok(), returned_peer == 757);
        }
    }
}
