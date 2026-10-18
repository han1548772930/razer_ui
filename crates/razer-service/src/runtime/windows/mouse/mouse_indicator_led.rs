use super::*;
use razer_device::{
    backend::{HidBackend, HidNode},
    mouse_indicator_led::{self as indicator, Change, Notification, Target},
};
use razer_hid::with_backend;
use std::sync::atomic::{AtomicBool, Ordering};

pub(super) fn request(target: &DeviceReadTarget, change: &Change) -> anyhow::Result<Value> {
    let source = indicator::capability(target.product_id).context("Three-LED source is absent")?;
    ensure!(
        source.accepts(target.physical_product_id, target.peer_product_id),
        "Three-LED target is not its physical receiver"
    );
    let cap = source.transport(target.physical_product_id)?;
    let before = current_target(target, &cap)?;
    let started = Instant::now();
    let validate = || {
        ensure!(
            started.elapsed() < Duration::from_secs(20),
            "Receiver LED task deadline exceeded"
        );
        ensure!(
            current_target(target, &cap)?["device_instance_id"] == before["device_instance_id"],
            "Receiver LED device instance changed"
        );
        Ok(())
    };
    validate()?;
    let report = {
        let _lock = receiver::ReceiverLock::acquire(&target.device_container_id)?;
        validate()?;
        ensure!(
            before["feature_report_bytes"] == cap.report_bytes,
            "Receiver LED Feature size differs"
        );
        let device = hid_transport::open(&target.path, cap.report_bytes)?;
        indicator::execute(
            target.product_id,
            &device,
            &cap,
            change,
            || transaction(target, &cap),
            &validate,
        )?
    };
    validate()?;
    Ok(
        json!({"target":target,"report":report,"source_class":cap.source_class,
        "setting_scope":"active_profile_receiver_indicator","elapsed_ms":started.elapsed().as_millis() as u64}),
    )
}

fn collection_target(node: &HidNode, product_id: u32) -> anyhow::Result<DeviceReadTarget> {
    let source = indicator::capability(product_id).context("Three-LED source is absent")?;
    let cap = source.transport(u32::from(node.product_id))?;
    ensure!(
        node.vendor_id == cap.vendor_id && node.interface_number == i32::from(cap.claim_interface),
        "Receiver quality collection interface differs"
    );
    let nodes = with_backend(|backend| backend.enumerate())?;
    ensure!(
        nodes.iter().filter(|current| *current == node).count() == 1,
        "Receiver quality collection changed"
    );
    let observation = hid::enumerate_collection(
        cap.vendor_id,
        node.product_id,
        std::str::from_utf8(&node.path)?,
    )?;
    ensure!(
        observation["complete"] == true,
        "Receiver quality physical enumeration is incomplete"
    );
    let matches = observation["interfaces"]
        .as_array()
        .context("Receiver quality interfaces are absent")?
        .iter()
        .filter(|item| {
            item["path"]
                .as_str()
                .is_some_and(|path| node.path.eq_ignore_ascii_case(path.as_bytes()))
        })
        .collect::<Vec<_>>();
    ensure!(
        matches.len() == 1,
        "Receiver quality has no unique physical collection"
    );
    let observed = matches[0];
    Ok(DeviceReadTarget {
        product_id,
        physical_product_id: u32::from(node.product_id),
        peer_product_id: None,
        device_container_id: observed["device_container_id"]
            .as_str()
            .context("Receiver quality ContainerId is absent")?
            .into(),
        path: observed["path"]
            .as_str()
            .context("Receiver quality path is absent")?
            .into(),
    })
}
pub(super) fn monitor(
    selected: &Target,
    canceled: &AtomicBool,
    publish: &mut impl FnMut(Notification) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let target = match selected {
        Target::Collection { node, product_id } => collection_target(node, *product_id)?,
        Target::WindowsContainer(target) => target.clone(),
    };
    let source = indicator::capability(target.product_id).context("Three-LED source is absent")?;
    ensure!(
        source.accepts(target.physical_product_id, target.peer_product_id),
        "Receiver quality physical owner differs"
    );
    let cap = source.transport(target.physical_product_id)?;
    let before = current_target(&target, &cap)?;
    let guard = || {
        ensure!(
            !canceled.load(Ordering::Acquire),
            "Receiver quality owner canceled"
        );
        ensure!(
            current_target(&target, &cap)?["device_instance_id"] == before["device_instance_id"],
            "Receiver quality physical instance changed"
        );
        Ok(())
    };
    let mut events = super::super::receiver_events::Events::new(
        u16::try_from(target.physical_product_id)?,
        &target.device_container_id,
    );
    indicator::monitor_notifications(&mut events, &guard, publish)
}
