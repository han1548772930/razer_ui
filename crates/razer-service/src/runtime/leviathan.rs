use super::*;
use razer_device::leviathan::{self, Operation};

impl PortableRuntime {
    pub(super) fn leviathan(
        &mut self,
        node: &HidNode,
        operation: Operation,
    ) -> anyhow::Result<Value> {
        let cap = leviathan::transport();
        ensure!(
            node.vendor_id == cap.vendor_id
                && u32::from(node.product_id) == leviathan::PRODUCT_ID
                && node.interface_number == 4,
            "Noa physical collection differs"
        );
        operation.validate()?;
        let started = Instant::now();
        let validate = || {
            ensure!(
                started.elapsed() < Duration::from_secs(20),
                "Noa task deadline exceeded"
            );
            Self::revalidate(node)
        };
        validate()?;
        let device = with_backend(|backend| backend.open(node))?;
        ensure!(
            device.report_lengths()?.feature.values().max().copied() == Some(65),
            "Noa Feature collection length differs"
        );
        let report = leviathan::execute(
            device.as_ref(),
            cap,
            operation,
            || self.transaction(node, &cap.source_class, 0, 31),
            &validate,
        )?;
        validate()?;
        Ok(
            json!({"node":node,"product_id":1352,"report":report,"source_class":cap.source_class,"setting_scope":"sound"}),
        )
    }
}
