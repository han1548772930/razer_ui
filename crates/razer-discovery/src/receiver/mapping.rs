//! Source-selected software mapping owners over confirmed receiver observations.
//! Local pairing records choose publication roles; they never establish radio presence.
use crate::discovery::ObservedDevice;
use anyhow::{Context as _, ensure};
use razer_device::{
    device_identity::{self, IdentityLookup},
    mapping_document::MappingProduct,
    mouse_receiver_mapping::{
        self, PeerIdentity, PublicationRole, ReceiverPeer, ReceiverProjection, ReceiverRoute,
    },
};
use razer_ipc::mapping_receiver::{Member, Target};
use razer_model::model::DeviceConnectionObservation;
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
mod tests;

#[derive(Clone)]
pub struct Document {
    value: Value,
    serial: Option<String>,
}

pub struct Projection {
    document: Value,
    products: Vec<u32>,
}

impl Projection {
    pub fn document(&self) -> &Value {
        &self.document
    }
    pub fn into_document(self) -> Value {
        self.document
    }
    pub fn products(&self) -> &[u32] {
        &self.products
    }
}

impl Document {
    pub fn new(value: Value, serial: Option<String>) -> Self {
        Self { value, serial }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Publication {
    Ordinary {
        product: MappingProduct,
    },
    Single {
        master: u32,
        peer: u32,
        peer_dongle: u32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    target: Target,
    publication: Publication,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pairing {
    product_id: u32,
    dongle_id: u32,
    device_container_id: String,
    master: PairingMaster,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct PairingMaster {
    dongle_id: u32,
}

fn live(observed: &ObservedDevice) -> bool {
    observed.hid_node().is_none()
        && matches!(
            observed.connection(),
            DeviceConnectionObservation::UsbPresent
                | DeviceConnectionObservation::HidPresent
                | DeviceConnectionObservation::ReceiverPeer(1)
        )
}

fn member(observed: &ObservedDevice) -> Member {
    match observed.peer_product_id() {
        Some(raw) => Member::peer(observed.product_id(), raw),
        None => Member::direct(observed.product_id()),
    }
}

/// Current ordinary mouse compilers have neither `isMika` nor
/// `canPairMultipleDevices` registrations. Their PostSet owner selects the
/// first local pairing record and the normal single-peer projection.
pub fn plans(
    observations: &[ObservedDevice],
    records: Option<&Value>,
) -> anyhow::Result<Vec<Plan>> {
    let Some(records) = records else {
        return Ok(Vec::new());
    };
    let records = records
        .as_array()
        .context("Mapping pairing records are not an ordered array")?;
    let pairing: Vec<Pairing> = records
        .iter()
        .cloned()
        .map(serde_json::from_value)
        .collect::<Result<_, _>>()
        .context("Mapping pairing record is incomplete")?;
    let mut planned = BTreeSet::new();
    let mut result = Vec::new();
    for observed in observations.iter().filter(|observed| {
        live(observed)
            && MappingProduct::for_product_id(observed.product_id())
                .is_some_and(|product| product.is_ordinary_mouse())
    }) {
        let physical = observed.physical_product_id();
        let product = MappingProduct::for_product_id(observed.product_id())
            .context("Mapping product source is absent")?;
        let IdentityLookup::Unique(identity) = device_identity::lookup(physical) else {
            continue;
        };
        // The master's actual physical alias identifies its own document.
        // A foreign peer must not become a second master of the same receiver.
        if identity.is_ble || identity.product_id != observed.product_id() {
            continue;
        }
        let source_identity = device_identity::middleware_product_identity(observed.product_id())
            .context("Mapping middleware identity is absent")?;
        ensure!(
            source_identity.product_id == product.source_product_id(),
            "Mapping source identity differs from current middleware"
        );
        let is_dongle = identity.is_dongle || source_identity.dongle_id == Some(physical);
        let key = (physical, observed.container().to_ascii_lowercase());
        if planned.contains(&key) {
            continue;
        }
        let role = mouse_receiver_mapping::publication_role(
            Some(records),
            physical,
            observed.container(),
        )?
        .context("Mapping pairing role was not observed")?;
        let (publication, members) = match role {
            PublicationRole::None if is_dongle => {
                (Publication::Ordinary { product }, vec![member(observed)])
            }
            PublicationRole::Master => {
                let record = pairing
                    .iter()
                    .find(|record| {
                        record.master.dongle_id == physical
                            && record.device_container_id == observed.container()
                    })
                    .context("Mapping master has no selected pairing record")?;
                let peers = observations
                    .iter()
                    .filter(|peer| {
                        live(peer)
                            && peer.product_id() == record.product_id
                            && peer.physical_product_id() == physical
                            && peer.peer_product_id() == Some(record.dongle_id)
                            && peer.container().eq_ignore_ascii_case(observed.container())
                    })
                    .collect::<Vec<_>>();
                let mut members = vec![member(observed)];
                if let [peer] = peers.as_slice()
                    && mouse_receiver_mapping::supports_ordinary(peer.product_id())
                {
                    members.push(member(peer));
                }
                (
                    Publication::Single {
                        master: observed.product_id(),
                        peer: record.product_id,
                        peer_dongle: record.dongle_id,
                    },
                    members,
                )
            }
            PublicationRole::None | PublicationRole::Slave => continue,
        };
        let target = Target::new(physical, observed.container().to_owned(), members)?;
        planned.insert(key);
        result.push(Plan {
            target,
            publication,
        });
    }
    Ok(result)
}

impl Plan {
    pub fn target(&self) -> &Target {
        &self.target
    }

    /// Wait for saved documents without accepting their local contents as a
    /// publication acknowledgement. A peer serial is an independent reading.
    pub fn project(
        &self,
        documents: &BTreeMap<u32, Document>,
    ) -> anyhow::Result<Option<Projection>> {
        let master = match self.publication {
            Publication::Ordinary { product } => product.product_id(),
            Publication::Single { master, .. } => master,
        };
        let Some(document) = documents.get(&master) else {
            return Ok(None);
        };
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Identity {
            product_id: u32,
        }
        let identity: Identity = serde_json::from_value(document.value.clone())?;
        let source_product =
            MappingProduct::for_product_id(master).context("Mapping source product disappeared")?;
        ensure!(
            identity.product_id == source_product.source_product_id(),
            "Saved mapping document has another logical identity"
        );
        match self.publication {
            Publication::Ordinary { .. } => Ok(Some(Projection {
                document: document.value.clone(),
                products: vec![master],
            })),
            Publication::Single {
                peer, peer_dongle, ..
            } => {
                let slave = self
                    .target
                    .contains_product(peer)
                    .then(|| documents.get(&peer))
                    .flatten();
                let products = std::iter::once(master)
                    .chain(
                        slave
                            .filter(|slave| {
                                slave
                                    .serial
                                    .as_ref()
                                    .is_some_and(|serial| !serial.is_empty())
                            })
                            .map(|_| peer),
                    )
                    .collect();
                let peer = ReceiverPeer::new(
                    PeerIdentity::new(peer, peer_dongle)?,
                    slave.map(|slave| &slave.value),
                    slave.and_then(|slave| slave.serial.as_deref()),
                );
                let document = ReceiverProjection::new(
                    self.target.physical_product_id(),
                    ReceiverRoute::Normal,
                    false,
                )?
                .project(&document.value, document.serial.as_deref(), &[peer])?;
                Ok(Some(Projection { document, products }))
            }
        }
    }

    /// Current ordinary dispatcher N sends unit43981/report8 to the selected
    /// slave. Explicit multi-device ownership takes precedence when supplied.
    pub fn event_targets(&self, event: &Value) -> anyhow::Result<Vec<u32>> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Envelope {
            source: Option<String>,
            event_type: Option<u32>,
            json_event: Option<Value>,
        }
        #[derive(Deserialize)]
        struct Event {
            input: String,
            output: String,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Input {
            unit_id: Option<u16>,
            report_id: Option<u8>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Output {
            multi_devices_info: Option<Ownership>,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Ownership {
            product_id: Option<u32>,
            dongle_id: Option<u32>,
        }
        let envelope: Envelope = serde_json::from_value(event.clone())
            .context("Mapping callback envelope is invalid")?;
        if envelope.source.as_deref() == Some("devicemodechanged") {
            return Ok(if envelope.event_type == Some(4) {
                self.target
                    .members()
                    .iter()
                    .map(Member::product_id)
                    .collect()
            } else {
                Vec::new()
            });
        }
        let Some(raw) = envelope.json_event else {
            return Ok(Vec::new());
        };
        #[derive(Deserialize)]
        struct Header {
            #[serde(rename = "type")]
            kind: String,
        }
        let header: Header = serde_json::from_value(raw.clone())?;
        if header.kind != "unsupportedmapping" {
            return Ok(Vec::new());
        }
        let event: Event =
            serde_json::from_value(raw).context("Mapping callback fields are invalid")?;
        let input: Input =
            serde_json::from_str(&event.input).context("Mapping callback input is invalid")?;
        let output: Output =
            serde_json::from_str(&event.output).context("Mapping callback output is invalid")?;
        match self.publication {
            Publication::Ordinary { product } => Ok(vec![product.product_id()]),
            Publication::Single {
                master,
                peer,
                peer_dongle,
            } => {
                if let Some(owner) = output.multi_devices_info {
                    let mut products = Vec::new();
                    if owner.product_id == Some(master)
                        || owner.dongle_id == Some(self.target.physical_product_id())
                    {
                        products.push(master);
                    }
                    if self.target.contains_product(peer)
                        && (owner.product_id == Some(peer) || owner.dongle_id == Some(peer_dongle))
                    {
                        products.push(peer);
                    }
                    return Ok(products);
                }
                let product = if input.unit_id == Some(43981) || input.report_id == Some(8) {
                    peer
                } else {
                    master
                };
                Ok(self
                    .target
                    .contains_product(product)
                    .then_some(product)
                    .into_iter()
                    .collect())
            }
        }
    }
}
