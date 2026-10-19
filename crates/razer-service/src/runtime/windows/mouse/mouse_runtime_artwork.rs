//! Windows observes actual ContainerId/interface identities; shared mouse
//! protocol owns query interpretation and source UI failure semantics.
use super::{current_target, hid, hid_transport, receiver, transaction};
use anyhow::{Context as _, ensure};
use razer_device::{
    backend::{HidBackend, HidNode},
    device_reads::DeviceReadTarget,
    mouse_runtime_artwork::{self as artwork, Query, Source, Target, Transport, Update},
};
use razer_hid::with_backend;
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
#[path = "mouse_runtime_artwork_ble.rs"]
pub(super) mod ble;

fn collection_target(
    node: &HidNode,
    product_id: u32,
    source: Source,
) -> anyhow::Result<DeviceReadTarget> {
    ensure!(
        node.vendor_id == 0x1532
            && source.accepts(u32::from(node.product_id))
            && node.interface_number == i32::from(source.interface()),
        "Mouse artwork collection source/interface differs"
    );
    let nodes = with_backend(|backend| backend.enumerate())?;
    ensure!(
        nodes.iter().filter(|current| *current == node).count() == 1,
        "Mouse artwork collection changed before identity resolution"
    );
    let observation =
        hid::enumerate_collection(0x1532, node.product_id, std::str::from_utf8(&node.path)?)?;
    ensure!(
        observation["complete"] == true,
        "Mouse artwork identity enumeration is incomplete"
    );
    let matches = observation["interfaces"]
        .as_array()
        .context("Mouse artwork Windows interfaces are absent")?
        .iter()
        .filter(|item| {
            item["path"]
                .as_str()
                .is_some_and(|path| node.path.eq_ignore_ascii_case(path.as_bytes()))
        })
        .collect::<Vec<_>>();
    ensure!(
        matches.len() == 1,
        "Mouse artwork collection has no unique Windows identity"
    );
    let observed = matches[0];
    Ok(DeviceReadTarget {
        product_id,
        physical_product_id: u32::from(node.product_id),
        peer_product_id: None,
        device_container_id: observed["device_container_id"]
            .as_str()
            .context("Mouse artwork ContainerId is absent")?
            .into(),
        path: observed["path"]
            .as_str()
            .context("Mouse artwork path is absent")?
            .into(),
    })
}

pub(super) fn run(
    selected: &Target,
    canceled: &AtomicBool,
    publish: &mut impl FnMut(Update) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    if let Target::WindowsBle(target) = selected {
        return ble::run(target, canceled, publish);
    }
    let source =
        artwork::source(selected.product_id()).context("Mouse artwork current source is absent")?;
    let target = match selected {
        Target::Collection { node, product_id } => collection_target(node, *product_id, source)?,
        Target::WindowsContainer(target) => target.clone(),
        Target::WindowsBle(_) => unreachable!("BLE selected above"),
    };
    let relay = target.peer_product_id.is_some();
    ensure!(
        !relay || source.supports_relay(),
        "Mouse artwork source has no current DualLink relay branch"
    );
    ensure!(
        if let Some(peer) = target.peer_product_id {
            source.accepts(peer) && !source.is_ble(peer)
        } else {
            source.accepts(target.physical_product_id)
        },
        "Mouse artwork physical/peer branch differs"
    );
    let cap = source.transport(target.physical_product_id, relay)?;
    let before = current_target(&target, &cap)?;
    ensure!(
        before["feature_report_bytes"].as_u64() == Some(91),
        "Mouse artwork actual Feature Report size differs"
    );
    let guard = || {
        ensure!(
            !canceled.load(Ordering::Acquire),
            "Mouse artwork owner canceled"
        );
        ensure!(
            current_target(&target, &cap)?["device_instance_id"] == before["device_instance_id"],
            "Mouse artwork physical device instance changed"
        );
        Ok(())
    };
    if let Some(peer) = target.peer_product_id {
        let read = receiver::query(&target.path, &target.device_container_id, None)?;
        let peers = razer_device::receiver_protocol::receiver_peers(&read)?;
        ensure!(
            peers
                .iter()
                .any(|(pid, status)| u32::from(*pid) == peer && *status == 1),
            "Mouse artwork receiver has no current matching online peer"
        );
    }
    struct Feature<'a, F> {
        target: &'a DeviceReadTarget,
        cap: &'a razer_device::device_reads::DeviceReadCapability,
        guard: F,
    }
    impl<F: Fn() -> anyhow::Result<()>> Transport for Feature<'_, F> {
        fn query(
            &mut self,
            query: Query,
            validate: &dyn Fn() -> anyhow::Result<()>,
        ) -> anyhow::Result<u8> {
            let started = Instant::now();
            let check = || {
                ensure!(
                    started.elapsed() < Duration::from_secs(10),
                    "Mouse artwork query deadline expired"
                );
                validate()?;
                (self.guard)()
            };
            check()?;
            let _lock = receiver::ReceiverLock::acquire(&self.target.device_container_id, &check)?;
            check()?;
            let device = hid_transport::open(&self.target.path, self.cap.report_bytes)?;
            artwork::query_hid(
                &device,
                self.cap,
                transaction(self.target, self.cap)?,
                query,
                &check,
            )
        }
    }
    let mut transport = Feature {
        target: &target,
        cap: &cap,
        guard: &guard,
    };
    let mut events = super::super::receiver_events::Events::new(
        u16::try_from(target.physical_product_id)?,
        &target.device_container_id,
    );
    artwork::monitor(
        source,
        target.physical_product_id,
        &mut transport,
        &mut events,
        &guard,
        publish,
    )
}
