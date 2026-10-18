//! Portable physical collection adapter for the Uma primary command owner.
use super::*;
use razer_device::hyperpolling_receiver::{self as receiver, Command, Observation, Target};

impl PortableRuntime {
    pub(super) fn hyperpolling_receiver(
        &mut self,
        target: &Target,
        command: Command,
    ) -> anyhow::Result<Value> {
        match target {
            Target::Collection(node) => {
                receiver::validate_node(node)?;
                let cap = receiver::source_capability();
                let started = Instant::now();
                let validate = || {
                    ensure!(
                        started.elapsed() < Duration::from_secs(12),
                        "Uma request expired"
                    );
                    Self::revalidate(node)
                };
                validate()?;
                let device = with_backend(|backend| backend.open(node))?;
                ensure!(
                    device.report_lengths()?.feature.values().max().copied()
                        == Some(cap.report_bytes),
                    "Uma Feature descriptor length differs"
                );
                let transaction = self.transaction(
                    node,
                    &cap.source_class,
                    cap.transaction_prefix,
                    cap.transaction_modulus,
                )?;
                let reading =
                    receiver::execute(device.as_ref(), cap, command, transaction, &validate)?;
                Ok(serde_json::to_value(Observation::new(
                    target.clone(),
                    reading,
                ))?)
            }
            Target::WindowsContainer { path, container } => {
                #[cfg(windows)]
                {
                    super::super::native::hyperpolling_receiver(path, container, command)
                }
                #[cfg(not(windows))]
                {
                    let _ = (path, container, command);
                    bail!("A Windows receiver ContainerId requires Windows");
                }
            }
        }
    }
}
