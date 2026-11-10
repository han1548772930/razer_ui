use super::*;
use razer_device::{
    earbuds_auto_pause::{self, Operation},
    earbuds_protocol40::Protocol40Session,
};

pub(super) fn event_source(
    selected: &razer_device::earbuds_events::Target,
) -> anyhow::Result<Box<dyn razer_device::receiver_pairing::HardwareEvents>> {
    use razer_device::earbuds_events::Target;
    let product_id = selected.product_id()?;
    let cap = earbuds_auto_pause::transport(product_id)?;
    let target = match selected {
        Target::WindowsContainer(target) => target.clone(),
        Target::Collection(node) => {
            let snapshot = hid::enumerate_for_product(cap.vendor_id, product_id as u16)?;
            ensure!(
                snapshot["complete"] == true,
                "Earbud event enumeration is incomplete"
            );
            let interfaces = snapshot["interfaces"]
                .as_array()
                .context("Earbud event interfaces are absent")?;
            let selected = interfaces
                .iter()
                .filter(|item| {
                    item["path"]
                        .as_str()
                        .is_some_and(|path| node.path.eq_ignore_ascii_case(path.as_bytes()))
                })
                .collect::<Vec<_>>();
            ensure!(
                selected.len() == 1,
                "Earbud collection has no unique physical identity"
            );
            DeviceReadTarget {
                product_id,
                physical_product_id: product_id,
                peer_product_id: None,
                path: selected[0]["path"]
                    .as_str()
                    .context("Earbud path is absent")?
                    .into(),
                device_container_id: selected[0]["device_container_id"]
                    .as_str()
                    .filter(|value| !value.is_empty())
                    .context("Earbud ContainerId is absent")?
                    .into(),
            }
        }
    };
    current_target(&target, &cap)?;
    Ok(Box::new(super::super::receiver_events::Events::new(
        product_id as u16,
        &target.device_container_id,
    )))
}

pub(super) fn request(target: &DeviceReadTarget, operation: Operation) -> anyhow::Result<Value> {
    crate::runtime::hid_broker::check_current_request()?;
    let cap = earbuds_auto_pause::transport(target.product_id)?;
    ensure!(
        target.physical_product_id == target.product_id && target.peer_product_id.is_none(),
        "Earbud route is not its direct physical owner"
    );
    let before = current_target(target, &cap)?;
    let started = Instant::now();
    let validate_physical = || {
        ensure!(
            current_target(target, &cap)?["device_instance_id"] == before["device_instance_id"],
            "Earbud device instance changed"
        );
        Ok(())
    };
    let validate = || {
        crate::runtime::hid_broker::check_current_request()?;
        ensure!(
            started.elapsed() < Duration::from_secs(30),
            "Earbud operation deadline expired"
        );
        validate_physical()
    };
    let _lock = receiver::ReceiverLock::acquire(&target.device_container_id, &validate)?;
    let device =
        hid_transport::open_validated(&target.path, cap.report_bytes, std::rc::Rc::new(&validate))?;
    let counter_target = razer_device::earbuds_events::Target::WindowsContainer(target.clone());
    let transactions = crate::runtime::earbuds_auto_pause::borrow_transactions(&counter_target)?;
    let mut next = |kind| transactions.next(kind);
    let source_validate = || {
        validate()?;
        transactions.validate()
    };
    let source_physical = || {
        transactions.validate()?;
        validate_physical()
    };
    let report = transactions.with_anc_cache(&source_validate, |cache| {
        earbuds_auto_pause::execute(
            &mut Protocol40Session::with_transaction_source(
                &device,
                razer_device::earbuds_protocol40::ThreadSleeper,
                &mut next,
            ),
            target.product_id,
            operation,
            cache,
            &source_validate,
            &source_physical,
        )
    })?;
    crate::runtime::earbuds_auto_pause::encode_report(report, &source_validate)
}
