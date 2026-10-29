use super::*;
use razer_device::leviathan::{self, Operation};

pub(super) fn event_source(
    selected: &leviathan::EventTarget,
) -> anyhow::Result<Box<dyn razer_device::receiver_pairing::HardwareEvents>> {
    let target = match selected {
        leviathan::EventTarget::WindowsContainer(target) => target.clone(),
        leviathan::EventTarget::Collection(node) => {
            ensure!(
                node.vendor_id == 5426 && node.product_id == 1352,
                "Noa event physical product differs"
            );
            let snapshot = hid::enumerate_for_product(5426, 1352)?;
            ensure!(
                snapshot["complete"] == true,
                "Noa native event enumeration is incomplete"
            );
            let interfaces = snapshot["interfaces"]
                .as_array()
                .context("Noa event interfaces are absent")?;
            let rows = interfaces
                .iter()
                .filter(|item| {
                    item["path"]
                        .as_str()
                        .is_some_and(|path| node.path.eq_ignore_ascii_case(path.as_bytes()))
                })
                .collect::<Vec<_>>();
            ensure!(
                rows.len() == 1,
                "Noa collection has no unique native identity"
            );
            DeviceReadTarget {
                product_id: 1352,
                physical_product_id: 1352,
                peer_product_id: None,
                path: rows[0]["path"]
                    .as_str()
                    .context("Noa path is absent")?
                    .into(),
                device_container_id: rows[0]["device_container_id"]
                    .as_str()
                    .context("Noa ContainerId is absent")?
                    .into(),
            }
        }
    };
    ensure!(
        target.product_id == 1352
            && target.physical_product_id == 1352
            && target.peer_product_id.is_none(),
        "Noa event target differs"
    );
    current_target(&target, leviathan::transport())?;
    Ok(Box::new(super::super::receiver_events::Events::new(
        1352,
        &target.device_container_id,
    )))
}

pub(super) fn request(target: &DeviceReadTarget, operation: Operation) -> anyhow::Result<Value> {
    let cap = leviathan::transport();
    ensure!(
        target.product_id == 1352
            && target.physical_product_id == 1352
            && target.peer_product_id.is_none(),
        "Noa target differs"
    );
    operation.validate()?;
    let before = current_target(target, cap)?;
    let started = Instant::now();
    let validate = || {
        ensure!(
            started.elapsed() < Duration::from_secs(20),
            "Noa task deadline exceeded"
        );
        ensure!(
            current_target(target, cap)?["device_instance_id"] == before["device_instance_id"],
            "Noa instance changed"
        );
        Ok(())
    };
    let report = {
        let _lock = receiver::ReceiverLock::acquire(&target.device_container_id, &validate)?;
        validate()?;
        ensure!(
            before["feature_report_bytes"] == 65,
            "Noa Feature collection length differs"
        );
        let device = hid_transport::open_validated(&target.path, 65, std::rc::Rc::new(&validate))?;
        leviathan::execute(
            &device,
            cap,
            operation,
            || transaction(target, cap),
            &validate,
        )?
    };
    validate().map_err(|error| {
        error.context(leviathan::ExecutionFailure {
            progress: report.clone(),
        })
    })?;
    Ok(
        json!({"target":target,"report":report,"source_class":cap.source_class,"setting_scope":"sound"}),
    )
}
