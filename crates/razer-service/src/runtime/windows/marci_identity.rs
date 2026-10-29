//! SetupAPI binds Marci's two collections to one observed physical container.
use super::{hid, receiver};
use anyhow::{Context as _, ensure};
use razer_device::backend::HidNode;
use razer_device::marci_lighting::{validate_control_node, validate_output_node};
use razer_hid::with_backend;
use serde::Deserialize;

#[derive(Deserialize)]
struct Snapshot {
    complete: bool,
    interfaces: Vec<Interface>,
}
#[derive(Deserialize)]
struct Interface {
    path: String,
    vendor_id: u16,
    product_id: u16,
    version_number: u16,
    device_container_id: Option<String>,
    device_instance_id: Option<String>,
    parent_instance_id: Option<String>,
    claim_interface: Option<u8>,
    usage_page: Option<u16>,
    usage: Option<u16>,
    output_report_bytes: Option<usize>,
    feature_report_bytes: Option<usize>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct Identity {
    path: String,
    instance: String,
    parent: String,
    container: String,
    version: u16,
    output_bytes: usize,
    feature_bytes: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::runtime) struct Channels {
    pub output: HidNode,
    pub control: HidNode,
    output_identity: Identity,
    control_identity: Identity,
}

fn identity(item: &Interface) -> anyhow::Result<Identity> {
    ensure!(
        item.vendor_id == 5426 && item.product_id == 3942 && item.claim_interface == Some(2),
        "Marci collection is not source interface 2"
    );
    ensure!(
        item.path.is_ascii()
            && !item.path.contains('\0')
            && item.path.to_ascii_lowercase().starts_with(r"\\?\hid#"),
        "Invalid Marci Windows HID path"
    );
    let container = item
        .device_container_id
        .as_deref()
        .context("Marci ContainerId is unobserved")?;
    ensure!(
        receiver::valid_container(container),
        "Invalid Marci ContainerId"
    );
    let instance = item
        .device_instance_id
        .as_deref()
        .filter(|s| !s.is_empty())
        .context("Marci instance is unobserved")?;
    let parent = item
        .parent_instance_id
        .as_deref()
        .filter(|s| !s.is_empty())
        .context("Marci parent instance is unobserved")?;
    Ok(Identity {
        path: item.path.to_ascii_lowercase(),
        instance: instance.to_ascii_lowercase(),
        parent: parent.to_ascii_lowercase(),
        container: container.to_ascii_lowercase(),
        version: item.version_number,
        output_bytes: item
            .output_report_bytes
            .context("Marci Output report length is unobserved")?,
        feature_bytes: item
            .feature_report_bytes
            .context("Marci Feature report length is unobserved")?,
    })
}

pub(in crate::runtime) fn resolve(container: &str) -> anyhow::Result<Channels> {
    ensure!(
        receiver::valid_container(container),
        "Marci requires a real physical ContainerId"
    );
    let snapshot: Snapshot = serde_json::from_value(hid::enumerate_for_product(5426, 3942)?)?;
    ensure!(snapshot.complete, "Marci Windows enumeration is incomplete");
    let select = |usage_page| -> anyhow::Result<Identity> {
        let matches = snapshot
            .interfaces
            .iter()
            .filter(|item| {
                item.device_container_id
                    .as_deref()
                    .is_some_and(|id| id.eq_ignore_ascii_case(container))
                    && item.usage_page == Some(usage_page)
                    && item.usage == Some(1)
                    && item.claim_interface == Some(2)
            })
            .collect::<Vec<_>>();
        let [item] = matches.as_slice() else {
            anyhow::bail!("Marci usage {usage_page}/1 has no unique physical collection");
        };
        identity(item)
    };
    let output_identity = select(0xff14)?;
    let control_identity = select(12)?;
    ensure!(
        output_identity.output_bytes == 781 && control_identity.feature_bytes == 91,
        "Marci collections differ from Output781/Feature91"
    );
    ensure!(
        output_identity.parent == control_identity.parent
            && output_identity.path != control_identity.path,
        "Marci collections do not share one observed interface parent"
    );
    let nodes = with_backend(|backend| backend.enumerate())?;
    let find = |proof: &Identity, usage_page| -> anyhow::Result<HidNode> {
        let matches = nodes
            .iter()
            .filter(|node| {
                std::str::from_utf8(&node.path)
                    .is_ok_and(|path| path.eq_ignore_ascii_case(&proof.path))
                    && node.vendor_id == 5426
                    && node.product_id == 3942
                    && node.release_number == proof.version
                    && node.usage_page == usage_page
                    && node.usage == 1
            })
            .collect::<Vec<_>>();
        let [node] = matches.as_slice() else {
            anyhow::bail!("Marci Windows collection has no unique current hidapi node");
        };
        Ok((*node).clone())
    };
    let output = find(&output_identity, 0xff14)?;
    let control = find(&control_identity, 12)?;
    validate_output_node(&output)?;
    validate_control_node(&control)?;
    Ok(Channels {
        output,
        control,
        output_identity,
        control_identity,
    })
}

impl Channels {
    pub(in crate::runtime) fn validate(&self, container: &str) -> anyhow::Result<()> {
        ensure!(
            Self::reobserve(&self.output, &self.output_identity)? == self.output_identity
                && Self::reobserve(&self.control, &self.control_identity)? == self.control_identity,
            "Marci physical instance changed"
        );
        ensure!(
            self.output_identity
                .container
                .eq_ignore_ascii_case(container)
                && self
                    .control_identity
                    .container
                    .eq_ignore_ascii_case(container),
            "Marci ContainerId changed"
        );
        Ok(())
    }
    fn reobserve(node: &HidNode, retained: &Identity) -> anyhow::Result<Identity> {
        let snapshot: Snapshot =
            serde_json::from_value(hid::enumerate_collection(5426, 3942, &retained.path)?)?;
        ensure!(
            snapshot.complete,
            "Marci collection re-observation is incomplete"
        );
        let matches = snapshot
            .interfaces
            .iter()
            .filter(|item| {
                item.path.eq_ignore_ascii_case(&retained.path)
                    && item.usage_page == Some(node.usage_page)
                    && item.usage == Some(node.usage)
            })
            .collect::<Vec<_>>();
        let [item] = matches.as_slice() else {
            anyhow::bail!("Marci retained collection was removed or replaced");
        };
        identity(item)
    }
}
