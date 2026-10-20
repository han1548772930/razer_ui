//! A read owns its service until shutdown, process termination and destruction.
use anyhow::ensure;
use razer_device::{
    keyboard_onboard_profiles::{CompleteReading, ReadOptions},
    keyboard_profile_inventory::ProfileInventory,
};
use razer_discovery::{direct, discovery::ObservedDevice};
use razer_ipc::{
    ServiceClient, ServiceRequest,
    keyboard_profile_inventory::{Observation, Target},
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

pub(super) enum Reading {
    Inventory(Observation),
    Complete(razer_ipc::keyboard_onboard_profiles::Observation),
}

pub(super) struct Completion {
    pub(super) result: Result<Reading, String>,
    pub(super) cleanup_error: Option<String>,
}

trait Transport {
    fn resolve(&mut self) -> anyhow::Result<Target>;
    fn inventory(&mut self, target: &Target) -> anyhow::Result<ProfileInventory>;
    fn complete(
        &mut self,
        target: &Target,
        options: &ReadOptions,
    ) -> anyhow::Result<CompleteReading>;
    fn shutdown(&mut self) -> anyhow::Result<()>;
    fn finish_termination(&mut self) -> anyhow::Result<()>;
}

struct NativeTransport {
    client: ServiceClient,
    route: ObservedDevice,
}

impl Transport for NativeTransport {
    fn resolve(&mut self) -> anyhow::Result<Target> {
        direct::keyboard_profile_inventory::resolve(&mut self.client, &self.route)
    }
    fn inventory(&mut self, target: &Target) -> anyhow::Result<ProfileInventory> {
        direct::keyboard_profile_inventory::read(&mut self.client, target)
    }
    fn complete(
        &mut self,
        target: &Target,
        options: &ReadOptions,
    ) -> anyhow::Result<CompleteReading> {
        direct::keyboard_onboard_profiles::read(&mut self.client, target, options)
    }
    fn shutdown(&mut self) -> anyhow::Result<()> {
        ensure!(
            !self.client.is_stopped(),
            "Keyboard inventory service stopped without a shutdown receipt"
        );
        self.client.request(ServiceRequest::Shutdown).map(|_| ())
    }
    fn finish_termination(&mut self) -> anyhow::Result<()> {
        self.client.finish_termination()
    }
}

fn guarded<T>(operation: impl FnOnce() -> anyhow::Result<T>) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(operation))
        .map_err(|_| "Keyboard inventory worker panicked".to_owned())?
        .map_err(|error| format!("{error:#}"))
}

fn check(canceled: &AtomicBool) -> anyhow::Result<()> {
    ensure!(
        !canceled.load(Ordering::Acquire),
        "Keyboard inventory request canceled"
    );
    Ok(())
}

pub(super) fn run(
    route: ObservedDevice,
    options: Option<ReadOptions>,
    canceled: Arc<AtomicBool>,
) -> Completion {
    let signal = canceled.clone();
    run_with(options, &canceled, move || {
        let mut client = ServiceClient::spawn()?;
        client.set_cancellation_guard(signal);
        Ok(NativeTransport { client, route })
    })
}

fn read<T: Transport>(
    transport: &mut T,
    options: Option<ReadOptions>,
    canceled: &AtomicBool,
) -> anyhow::Result<Reading> {
    check(canceled)?;
    let target = transport.resolve()?;
    target.validate()?;
    check(canceled)?;
    let reading = match options {
        Some(options) => {
            razer_ipc::keyboard_onboard_profiles::validate_target(&target)?;
            options.validate()?;
            // The mounted 679 load begins with macro storage, then reads this
            // inventory once before brightness, assignments and kill switch.
            let reading = transport.complete(&target, &options)?;
            let observation = razer_ipc::keyboard_onboard_profiles::Observation {
                target: target.clone(),
                options: options.clone(),
                reading,
            };
            observation.validate_for(&target, &options)?;
            Reading::Complete(observation)
        }
        None => {
            ensure!(
                target.product_id() != razer_device::keyboard_onboard_profiles::PRODUCT_ID,
                "679 must use its complete mounted read order"
            );
            let inventory = transport.inventory(&target)?;
            let observation = Observation {
                target: target.clone(),
                inventory,
            };
            observation.validate_for(&target)?;
            Reading::Inventory(observation)
        }
    };
    check(canceled)?;
    Ok(reading)
}

fn run_with<T: Transport>(
    options: Option<ReadOptions>,
    canceled: &AtomicBool,
    factory: impl FnOnce() -> anyhow::Result<T>,
) -> Completion {
    let mut transport = match guarded(|| {
        check(canceled)?;
        factory()
    }) {
        Ok(transport) => transport,
        Err(error) => {
            return Completion {
                result: Err(error),
                cleanup_error: None,
            };
        }
    };
    let result = guarded(|| read(&mut transport, options, canceled));
    let shutdown = guarded(|| transport.shutdown()).err();
    let termination = guarded(|| transport.finish_termination()).err();
    let destruction = guarded(|| {
        drop(transport);
        Ok(())
    })
    .err();
    let errors = [shutdown, termination, destruction]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    let cleanup_error = (!errors.is_empty()).then(|| errors.join("; "));
    let result = result.and_then(|reading| {
        guarded(|| {
            check(canceled)?;
            Ok(reading)
        })
    });
    Completion {
        result,
        cleanup_error,
    }
}

#[cfg(test)]
#[path = "keyboard_profile_inventory_worker_tests.rs"]
mod tests;
