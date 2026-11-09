//! Physical identity revalidation for the retained mapping owner.
use super::super::native_target;
use anyhow::{Context as _, ensure};
use serde_json::{Value, json};

pub(super) fn mapping_target(product: u32, container: &str) -> anyhow::Result<(u32, Value)> {
    if !matches!(product, 158 | 756) {
        return native_target(container, Some(product));
    }
    ensure!(
        super::super::receiver::valid_container(container),
        "Receiver mapping requires an observed nonzero ContainerId"
    );
    let observation = super::super::usb::enumerate()?;
    ensure!(
        observation["complete"] == true
            && observation["failures"]
                .as_array()
                .is_some_and(Vec::is_empty),
        "Receiver mapping physical USB enumeration is incomplete"
    );
    let devices = observation["devices"]
        .as_array()
        .context("Receiver mapping USB observations are absent")?
        .iter()
        .filter(|device| {
            device["vendor_id"] == 5426
                && device["device_container_id"]
                    .as_str()
                    .is_some_and(|id| id.eq_ignore_ascii_case(container))
        })
        .collect::<Vec<_>>();
    if devices.is_empty() && product == 756 {
        let observed = super::super::hid::enumerate_metadata()?;
        let target = razer_device::mouse_runtime_artwork::WindowsBleTarget::from_hid(
            756, 758, container, &observed,
        )?;
        return Ok((
            758,
            json!({"product_id":758,"vendor_id":5426,"device_container_id":target.device_container_id,"device_instance_id":target.device_instance_id,"path":target.hid_path,"ble_identity":target}),
        ));
    }
    let [device] = devices.as_slice() else {
        anyhow::bail!("Receiver mapping physical container is offline or ambiguous")
    };
    let physical = u32::try_from(
        device["product_id"]
            .as_u64()
            .context("Receiver mapping physical PID is absent")?,
    )?;
    ensure!(
        match product {
            158 => matches!(physical, 158 | 159),
            756 => matches!(physical, 756 | 757),
            _ => false,
        },
        "Receiver mapping USB owner has a foreign physical PID"
    );
    let razer_device::device_identity::IdentityLookup::Unique(identity) =
        razer_device::device_identity::lookup(physical)
    else {
        anyhow::bail!("Receiver mapping physical adapter attribution is ambiguous")
    };
    ensure!(
        identity.product_id == product
            && !identity.is_ble
            && identity.is_dongle == (physical != product),
        "Receiver mapping physical attribution differs from current factory"
    );
    for field in ["path", "device_instance_id"] {
        ensure!(
            device[field]
                .as_str()
                .is_some_and(|value| !value.is_empty()),
            "Receiver mapping physical identity lacks {field}"
        );
    }
    Ok((physical, (**device).clone()))
}

#[derive(Clone, Debug, serde::Serialize)]
pub(super) struct DeviceInfo {
    #[serde(rename = "vendorId")]
    pub vendor_id: u32,
    #[serde(rename = "productId")]
    pub product_id: u32,
    #[serde(rename = "containerId")]
    pub container_id: String,
    #[serde(rename = "usbInstanceId")]
    pub usb_instance_id: String,
    pub guid: String,
}

pub(super) struct Target {
    identity: Identity,
    pub info: DeviceInfo,
    physical: Value,
}

enum Identity {
    Single(razer_device::mapping_document::MappingProduct),
    Receiver(razer_ipc::mapping_receiver::Target),
}
impl Target {
    pub fn observe(product: u32, container: &str, owner: &str) -> anyhow::Result<Self> {
        let logical_product =
            razer_device::mapping_document::MappingProduct::for_product_id(product)
                .context("Mapping product registration has no current document compiler")?;
        ensure!(
            !owner.is_empty() && !owner.contains('\0'),
            "Invalid mapping owner identity"
        );
        let (product_id, physical) = mapping_target(product, container)?;
        let info = DeviceInfo {
            vendor_id: 5426,
            product_id,
            container_id: physical
                .get("device_container_id")
                .and_then(Value::as_str)
                .context("Missing observed container")?
                .into(),
            usb_instance_id: physical
                .get("device_instance_id")
                .and_then(Value::as_str)
                .context("Missing observed USB instance")?
                .into(),
            guid: owner.into(),
        };
        Ok(Self {
            identity: Identity::Single(logical_product),
            info,
            physical,
        })
    }

    pub fn observe_receiver(target: razer_ipc::mapping_receiver::Target, owner: &str) -> anyhow::Result<Self> {
        target.validate()?;
        ensure!(!owner.is_empty() && !owner.contains('\0'), "Invalid mapping owner identity");
        let (product_id, physical) = native_target(target.device_container_id(), Some(target.physical_product_id()))?;
        validate_receiver_members(&target)?;
        let info = DeviceInfo {
            vendor_id: 5426,
            product_id,
            container_id: physical.get("device_container_id").and_then(Value::as_str)
                .context("Missing observed receiver container")?.into(),
            usb_instance_id: physical.get("device_instance_id").and_then(Value::as_str)
                .context("Missing observed receiver instance")?.into(),
            guid: owner.into(),
        };
        let (_, after) = native_target(target.device_container_id(), Some(target.physical_product_id()))?;
        ensure!(after == physical, "Receiver changed during mapping member qualification");
        Ok(Self { identity: Identity::Receiver(target), info, physical })
    }

    pub fn contains_product(&self, product: u32) -> bool {
        match &self.identity {
            Identity::Receiver(receiver) => receiver.contains_product(product),
            Identity::Single(logical) => logical.product_id() == product,
        }
    }

    pub fn admits_document(&self, product: u32, dual_linked: bool) -> bool {
        match &self.identity {
            Identity::Receiver(receiver) => product == receiver.physical_product_id() || receiver.contains_product(product),
            Identity::Single(logical) => {
                product == if logical.product_id() == 158 && dual_linked {
                    self.info.product_id
                } else { logical.product_id() }
            },
        }
    }

    pub fn revalidate(&self) -> anyhow::Result<()> {
        let (_, physical) = match &self.identity {
            Identity::Receiver(receiver) => {
                native_target(&self.info.container_id, Some(receiver.physical_product_id()))?
            },
            Identity::Single(logical) => mapping_target(logical.product_id(), &self.info.container_id)?,
        };
        ensure!(physical == self.physical, "Physical mapping device changed");
        Ok(())
    }

    /// Radio qualification belongs to registration and publication boundaries.
    /// The input pump only checks its retained physical instance; the caller
    /// replaces this owner when its observed pairing generation changes.
    pub fn revalidate_publication(&self) -> anyhow::Result<()> {
        self.revalidate()?;
        if let Identity::Receiver(receiver) = &self.identity {
            validate_receiver_members(receiver)?;
            self.revalidate()?;
        }
        Ok(())
    }
}

fn validate_receiver_members(target: &razer_ipc::mapping_receiver::Target) -> anyhow::Result<()> {
    use razer_device::device_identity::{self, IdentityLookup};
    use razer_ipc::mapping_receiver::Route;
    let physical = target.physical_product_id();
    let receiver = razer_device::receiver_capabilities::capability(u16::try_from(physical)?);
    let mut radio_members = Vec::new();
    for member in target.members() {
        match member.route() {
            Route::Direct => {
                let IdentityLookup::Unique(identity) = device_identity::lookup(physical) else {
                    anyhow::bail!("Mapping physical product attribution is ambiguous")
                };
                ensure!(identity.product_id == member.product_id() && !identity.is_ble,
                    "Mapping direct member belongs to another physical product");
                if identity.is_dongle { radio_members.push((member.product_id(), physical)); }
            }
            Route::Peer { product_id } => {
                let cap = receiver.context("Mapping receiver query has no current source capability")?;
                let IdentityLookup::Unique(identity) = device_identity::lookup_receiver_peer(product_id, cap.peer_match_product_id) else {
                    anyhow::bail!("Mapping peer attribution is ambiguous")
                };
                ensure!(identity.product_id == member.product_id(), "Mapping peer belongs to another logical product");
                radio_members.push((member.product_id(), product_id));
            }
        }
    }
    if radio_members.is_empty() { return Ok(()); }
    let cap = receiver.context("Mapping radio member has no current receiver query")?;
    let observation = super::super::hid::enumerate_metadata()?;
    ensure!(observation["complete"] == true, "Mapping receiver HID enumeration is incomplete");
    let interfaces = observation["interfaces"].as_array().context("Mapping receiver interfaces are absent")?;
    let mut errors = Vec::new();
    for row in interfaces.iter().filter(|row| {
        row["vendor_id"] == 5426 && row["product_id"] == physical
            && row["claim_interface"] == cap.claim_interface
            && row["feature_report_bytes"] == cap.report_bytes
            && row["device_container_id"].as_str().is_some_and(|container| container.eq_ignore_ascii_case(target.device_container_id()))
    }) {
        let Some(path) = row["path"].as_str().filter(|path| !path.is_empty()) else { continue };
        let queried = super::super::receiver::query(path, target.device_container_id(), Some(cap.source_product_id))
            .and_then(|response| razer_device::receiver_protocol::receiver_peers(&response));
        match queried {
            Ok(peers) => {
                for (logical, raw) in &radio_members {
                    ensure!(peers.iter().filter(|(pid, _)| pid == raw).count() == 1
                        && peers.iter().any(|(pid, status)| pid == raw && *status == 1),
                        "Receiver has not confirmed mapping member {logical} / raw peer {raw} online");
                }
                return Ok(());
            }
            Err(error) => errors.push(format!("{error:#}")),
        }
    }
    anyhow::bail!("Mapping receiver member query failed: {}", if errors.is_empty() {
        "no current HID collection".into()
    } else { errors.join("; ") })
}

pub(super) fn gaming_indicator(target: &Target, enabled: bool) -> anyhow::Result<Value> {
    target.revalidate()?;
    let product = target.info.product_id;
    let container = &target.info.container_id;
    ensure!(
        matches!(product, 515 | 679),
        "This mapping registration has no audited gaming LED route"
    );
    let settings = razer_device::keyboard_settings::capability(product)
        .context("Missing source keyboard wired capability")?;
    let cap = &settings.transport;
    let observation = super::super::hid::enumerate()?;
    let interfaces = observation["interfaces"]
        .as_array()
        .context("Missing Windows HID interfaces")?;
    let mut paths = std::collections::BTreeMap::new();
    for interface in interfaces {
        if interface["vendor_id"] == 5426
            && interface["product_id"] == product
            && interface["claim_interface"] == cap.claim_interface_for(product)?
            && interface["feature_report_bytes"] == cap.report_bytes
            && interface["device_container_id"]
                .as_str()
                .is_some_and(|v| v.eq_ignore_ascii_case(container))
        {
            if let Some(path) = interface["path"].as_str() {
                paths.insert(path.to_ascii_lowercase(), path.to_owned());
            }
        }
    }
    ensure!(
        paths.len() == 1,
        "Gaming LED HID interface is absent or ambiguous"
    );
    super::super::device_reads::keyboard_gaming_indicator(
        &razer_device::device_reads::DeviceReadTarget {
            product_id: product,
            physical_product_id: product,
            peer_product_id: None,
            device_container_id: container.into(),
            path: paths.into_values().next().unwrap(),
        },
        enabled,
    )
}
