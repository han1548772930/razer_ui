//! Windows identity selection for PID653. Protocol interpretation stays shared.
use super::{current_target, hid, hid_transport, receiver, transaction, verify_routing};
use anyhow::{Context as _, ensure};
use razer_device::{
    backend::{HidBackend, HidNode},
    device_reads::DeviceReadTarget,
    keyboard_wristrest::{self as wristrest, Target, Update},
};
use razer_hid::with_backend;
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

fn collection_target(node: &HidNode, product_id: u32) -> anyhow::Result<DeviceReadTarget> {
    let cap = wristrest::capability(product_id).context("Wristrest capability is absent")?;
    ensure!(
        node.vendor_id == cap.vendor_id
            && node.product_id == wristrest::PRODUCT_ID as u16
            && node.interface_number == i32::from(cap.claim_interface),
        "Wristrest collection differs from the source interface"
    );
    let nodes = with_backend(|backend| backend.enumerate())?;
    ensure!(
        nodes.iter().filter(|current| *current == node).count() == 1,
        "Wristrest collection changed before ContainerId resolution"
    );
    let observation = hid::enumerate_collection(
        cap.vendor_id,
        node.product_id,
        std::str::from_utf8(&node.path)?,
    )?;
    ensure!(
        observation["complete"] == true,
        "Wristrest Windows identity enumeration is incomplete"
    );
    let matches = observation["interfaces"]
        .as_array()
        .context("Wristrest Windows interfaces are absent")?
        .iter()
        .filter(|item| {
            item["path"]
                .as_str()
                .is_some_and(|path| node.path.eq_ignore_ascii_case(path.as_bytes()))
        })
        .collect::<Vec<_>>();
    ensure!(
        matches.len() == 1,
        "Wristrest collection has no unique actual Windows identity"
    );
    let observed = matches[0];
    Ok(DeviceReadTarget {
        product_id,
        physical_product_id: u32::from(node.product_id),
        peer_product_id: None,
        device_container_id: observed["device_container_id"]
            .as_str()
            .context("Wristrest collection has no actual ContainerId")?
            .to_owned(),
        path: observed["path"]
            .as_str()
            .context("Wristrest collection has no actual Windows path")?
            .to_owned(),
    })
}

pub(super) fn run(
    selected: &Target,
    canceled: &AtomicBool,
    publish: &mut impl FnMut(Update) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let cap =
        wristrest::capability(selected.product_id()).context("Wristrest capability is absent")?;
    let target = match selected {
        Target::Collection { node, product_id } => collection_target(node, *product_id)?,
        Target::WindowsContainer(target) => target.clone(),
    };
    ensure!(
        target.peer_product_id.is_none() && target.physical_product_id == wristrest::PRODUCT_ID,
        "Wristrest reader requires the source's direct wired PID653"
    );
    let before = current_target(&target, cap)?;
    verify_routing(&target, cap)?;
    ensure!(
        before["feature_report_bytes"].as_u64() == Some(91),
        "Wristrest Feature Report size differs from the source"
    );
    let guard = || {
        ensure!(
            !canceled.load(Ordering::Acquire),
            "Wristrest owner canceled"
        );
        ensure!(
            current_target(&target, cap)?["device_instance_id"] == before["device_instance_id"],
            "Wristrest physical device instance changed"
        );
        Ok(())
    };
    let initial = || {
        let started = Instant::now();
        let validate = || {
            ensure!(
                started.elapsed() < Duration::from_secs(10),
                "Wristrest initial query deadline expired"
            );
            guard()
        };
        validate()?;
        let _lock = receiver::ReceiverLock::acquire(&target.device_container_id, &validate)?;
        validate()?;
        let device = hid_transport::open(&target.path, cap.report_bytes)?;
        wristrest::query(&device, transaction(&target, cap)?, &validate)
    };
    // Current mapping_engine selects every collection with this real
    // ContainerId and PID. No guessed sibling path or serial grouping.
    let mut events = super::super::receiver_events::Events::new(
        u16::try_from(target.physical_product_id)?,
        &target.device_container_id,
    );
    wristrest::monitor(&mut events, initial, &guard, publish)
}
