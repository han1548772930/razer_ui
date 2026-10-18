//! Physical identity revalidation for the retained mapping owner.
use anyhow::{Context as _, ensure};
use serde_json::{Value, json};
use super::super::native_target;

pub(super) fn mapping_target(product: u32, container: &str) -> anyhow::Result<(u32, Value)> {
    if product != 756 {
        return native_target(container, Some(product));
    }
    ensure!(
        super::super::receiver::valid_container(container),
        "756 mapping requires an observed nonzero ContainerId"
    );
    let observation = super::super::usb::enumerate()?;
    ensure!(
        observation["complete"] == true
            && observation["failures"]
                .as_array()
                .is_some_and(Vec::is_empty),
        "756 mapping physical USB enumeration is incomplete"
    );
    let devices = observation["devices"]
        .as_array()
        .context("756 mapping USB observations are absent")?
        .iter()
        .filter(|device| {
            device["vendor_id"] == 5426
                && device["device_container_id"]
                    .as_str()
                    .is_some_and(|id| id.eq_ignore_ascii_case(container))
        })
        .collect::<Vec<_>>();
    if devices.is_empty() {
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
        anyhow::bail!("756 mapping physical container is offline or ambiguous")
    };
    let physical = u32::try_from(
        device["product_id"]
            .as_u64()
            .context("756 mapping physical PID is absent")?,
    )?;
    ensure!(
        matches!(physical, 756 | 757),
        "756 mapping USB owner has a foreign physical PID"
    );
    let razer_device::device_identity::IdentityLookup::Unique(identity) =
        razer_device::device_identity::lookup(physical)
    else {
        anyhow::bail!("756 mapping physical adapter attribution is ambiguous")
    };
    ensure!(
        identity.product_id == 756 && !identity.is_ble && identity.is_dongle == (physical == 757),
        "756 mapping physical attribution differs from current factory"
    );
    for field in ["path", "device_instance_id"] {
        ensure!(
            device[field]
                .as_str()
                .is_some_and(|value| !value.is_empty()),
            "756 mapping physical identity lacks {field}"
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
    pub logical_product: razer_device::mapping_document::MappingProduct,
    pub info: DeviceInfo,
    physical: Value,
}
impl Target {
    pub fn observe(product: u32, container: &str, owner: &str) -> anyhow::Result<Self> {
        let logical_product = razer_device::mapping_document::MappingProduct::for_product_id(product)
            .context("Mapping product registration has no current document compiler")?;
        ensure!(!owner.is_empty() && !owner.contains('\0'), "Invalid mapping owner identity");
        let (product_id, physical) = mapping_target(product, container)?;
        let info = DeviceInfo { vendor_id: 5426, product_id,
            container_id: physical.get("device_container_id").and_then(Value::as_str).context("Missing observed container")?.into(),
            usb_instance_id: physical.get("device_instance_id").and_then(Value::as_str).context("Missing observed USB instance")?.into(),
            guid: owner.into() };
        Ok(Self { logical_product, info, physical })
    }
    pub fn revalidate(&self) -> anyhow::Result<()> {
        let (_, physical) = mapping_target(self.logical_product.product_id(), &self.info.container_id)?;
        ensure!(physical == self.physical, "Physical mapping device changed");
        Ok(())
    }
    pub fn discovery_query(&self) -> anyhow::Result<razer_device::mapping_native_session::NativeDiscoveryQuery> {
        use razer_device::mapping_native_session::*;
        let guid = uuid::Uuid::parse_str(self.info.container_id.trim_matches(['{','}']))?;
        let (a,b,c,d) = guid.as_fields();
        Ok(NativeDiscoveryQuery {
            container: NativeGuid::new(a,b,c,*d),
            product: NativeProductIdentity::razer(self.info.product_id.try_into()?,
                if self.logical_product.is_keyboard() { NativeDeviceKind::Keyboard } else { NativeDeviceKind::Mouse }),
            // Routing uses only the observed container/PID pair. An empty
            // bitmap adds no unobserved per-product capabilities.
            verified_capabilities: NativeCapabilitySet::empty(),
        })
    }
}

pub(super) fn gaming_indicator(target:&Target,enabled:bool)->anyhow::Result<Value> {
    target.revalidate()?;
    let product=target.info.product_id;let container=&target.info.container_id;
    ensure!(matches!(product,515|679),"This mapping registration has no audited gaming LED route");
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
