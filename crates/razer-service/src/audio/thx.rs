//! Direct local THX service. Vendor DLLs are never loaded by these routes.
use crate::{
    thx_direct::{DirectThxTransport, LocalRpc},
    thx_rpc::Cancellation,
};
use anyhow::{Result, ensure};
use razer_device::{
    backend::HidNode,
    thx_blackshark_firmware::{
        BlackSharkFirmwareConnection, BlackSharkFirmwareIdentity, BlackSharkFirmwareSession,
        blackshark_firmware_route,
    },
    thx_submission::{
        ThxFirmwareProgress, ThxSubmissionPlan, ThxSubmissionProgress, ThxSubmissionScope,
    },
    thx_v4::{SpatialStatus, ThxV4Controller, ThxV4Transport},
};
use std::{collections::BTreeSet, time::Duration};

// SetupDiOpenDeviceInterface + ContainerId is collection-independent. Reuse
// the existing exact-path OS guard; no camera filter or vendor DLL is opened.
#[cfg(windows)]
#[path = "../runtime/windows/camera_identity.rs"]
mod interface_identity;

pub fn container_id(node: &HidNode) -> Result<String> {
    #[cfg(windows)]
    {
        interface_identity::container_id(std::str::from_utf8(&node.path)?)
    }
    #[cfg(not(windows))]
    {
        let _ = node;
        anyhow::bail!("THX OS ContainerId adapter is unavailable on this platform")
    }
}
pub fn resolve(scope: &ThxSubmissionScope) -> Result<HidNode> {
    scope.validate()?;
    let nodes = razer_hid::with_backend(|backend| backend.enumerate())?;
    resolve_candidates(scope, &nodes, container_id)
}
fn resolve_candidates(
    scope: &ThxSubmissionScope,
    nodes: &[HidNode],
    mut observe_container: impl FnMut(&HidNode) -> Result<String>,
) -> Result<HidNode> {
    resolve_candidates_for(scope, nodes, true, &mut observe_container)
}
fn resolve_candidates_for(
    scope: &ThxSubmissionScope,
    nodes: &[HidNode],
    rpc: bool,
    mut observe_container: impl FnMut(&HidNode) -> Result<String>,
) -> Result<HidNode> {
    scope.validate()?;
    let mut matching = vec![];
    let mut errors = vec![];
    let mut containers = BTreeSet::new();
    for node in nodes {
        if node.vendor_id != 0x1532
            || node.interface_number != 5
            || blackshark_firmware_route(scope.product_id, node.product_id).is_err()
        {
            continue;
        }
        match observe_container(node) {
            Ok(container) => {
                ensure!(
                    !container.is_empty(),
                    "THX candidate lacks an OS ContainerId"
                );
                containers.insert(container.to_ascii_lowercase());
                if container.eq_ignore_ascii_case(&scope.container_id) {
                    if blackshark_firmware_route(scope.product_id, node.product_id)?
                        == BlackSharkFirmwareConnection::Wired
                        && !scope.serial_number.is_empty()
                    {
                        ensure!(
                            node.serial_number.as_deref() == Some(scope.serial_number.as_str()),
                            "THX wired serial differs from the retained scope"
                        );
                    }
                    matching.push(node.clone());
                }
            }
            Err(error) => errors.push(format!("{error:#}")),
        }
    }
    // A failed candidate observation can conceal another matching collection.
    ensure!(
        errors.is_empty(),
        "THX collection identity incomplete: {}",
        errors.join("; ")
    );
    // Native RPC carries product/endpoint only. Until the local service's
    // physical endpoint association is proved, reject ambiguous containers,
    // including separate processes that cannot share the Controller lock.
    ensure!(
        !rpc || containers.len() == 1,
        "THX product/endpoint cannot be uniquely associated with one HID container; found {}",
        containers.len()
    );
    ensure!(
        matching.len() == 1,
        "THX needs one exact interface-5 collection in the retained container; found {}",
        matching.len()
    );
    Ok(matching.remove(0))
}
pub fn validate_live(
    identity: &BlackSharkFirmwareIdentity,
    cancellation: &Cancellation,
) -> Result<()> {
    cancellation.check()?;
    identity.validate()?;
    let nodes = razer_hid::with_backend(|backend| backend.enumerate())?;
    let scope = ThxSubmissionScope {
        product_id: identity.owner_product_id,
        container_id: identity.device_container_id.clone(),
        serial_number: identity.node.serial_number.clone().unwrap_or_default(),
        profile_id: String::new(),
        generation: identity.generation,
    };
    ensure!(
        resolve_candidates_for(&scope, &nodes, false, container_id)? == identity.node,
        "THX HID collection identity changed or became ambiguous"
    );
    cancellation.check()
}
pub fn wait(ms: u64, cancellation: &Cancellation) -> Result<()> {
    let deadline = std::time::Instant::now() + Duration::from_millis(ms);
    loop {
        cancellation.check()?;
        let Some(remaining) = deadline.checked_duration_since(std::time::Instant::now()) else {
            return Ok(());
        };
        std::thread::sleep(remaining.min(Duration::from_millis(5)));
    }
}
pub trait ThxJobTransport: ThxV4Transport {
    fn has_thx_binding(&self) -> bool {
        true
    }
    fn exchange_hid(
        &mut self,
        _command: razer_device::thx_blackshark_firmware::BlackSharkFirmwareCommand,
    ) -> Result<razer_device::thx_blackshark_firmware::BlackSharkFirmwareCommandProgress> {
        anyhow::bail!("This injected transport does not provide raw headset HID exchanges")
    }
    fn firmware_progress(&self) -> Vec<ThxFirmwareProgress> {
        vec![]
    }
}
impl<R: crate::thx_direct::ThxLocalRpc> ThxJobTransport for DirectThxTransport<R> {
    fn has_thx_binding(&self) -> bool {
        self.bound()
    }
    fn exchange_hid(
        &mut self,
        command: razer_device::thx_blackshark_firmware::BlackSharkFirmwareCommand,
    ) -> Result<razer_device::thx_blackshark_firmware::BlackSharkFirmwareCommandProgress> {
        DirectThxTransport::exchange_hid(self, command)
    }
    fn firmware_progress(&self) -> Vec<ThxFirmwareProgress> {
        DirectThxTransport::firmware_progress(self).to_vec()
    }
}
pub fn open(
    scope: &ThxSubmissionScope,
    cancellation: Cancellation,
) -> Result<DirectThxTransport<LocalRpc>> {
    #[cfg(not(windows))]
    {
        let _ = (scope, cancellation);
        anyhow::bail!("THX local socket/ContainerId adapters are unavailable on this platform")
    }
    #[cfg(windows)]
    {
        cancellation.check()?;
        let nodes = razer_hid::with_backend(|backend| backend.enumerate())?;
        let node = resolve_candidates_for(scope, &nodes, false, container_id)?;
        let identity = BlackSharkFirmwareIdentity::new(
            scope.product_id,
            node,
            &scope.container_id,
            scope.generation,
        )?;
        validate_live(&identity, &cancellation)?;
        let validate_open = || validate_live(&identity, &cancellation);
        let transport = razer_hid::with_backend(|backend| {
            backend.open_validated(&identity.node, std::rc::Rc::new(&validate_open))
        })?;
        let session = BlackSharkFirmwareSession::new(identity, transport)?;
        let check_cancel = cancellation.clone();
        let wait_cancel = cancellation.clone();
        DirectThxTransport::new(
            LocalRpc {
                deadline: Duration::from_secs(4),
                cancellation,
                scope: scope.clone(),
            },
            session,
            Box::new(move |identity| validate_live(identity, &check_cancel)),
            Box::new(move |ms| wait(ms, &wait_cancel)),
        )
    }
}
pub fn failed(
    scope: ThxSubmissionScope,
    plan: ThxSubmissionPlan,
    error: String,
) -> ThxSubmissionProgress {
    ThxSubmissionProgress {
        scope,
        plan,
        render_setup: None,
        render_shutdown: None,
        roller_mapping: None,
        initial_observation: None,
        outcome: None,
        firmware: vec![],
        error: Some(error),
        cleanup_error: None,
    }
}
/// Pure orchestration with injected transport/guards/scheduler. An initial
/// actual getter seeds the change-dependent EQ hook; no writes follow an
/// unavailable initial observation. This is explicit Rust integration policy.
pub fn execute<T: ThxJobTransport>(
    scope: ThxSubmissionScope,
    plan: ThxSubmissionPlan,
    transport: T,
    validate: impl FnMut() -> Result<()>,
    wait_retry: impl FnMut(u64) -> Result<()>,
) -> ThxSubmissionProgress {
    if plan == ThxSubmissionPlan::ShutdownRender {
        return failed(
            scope,
            plan,
            "THX source shutdown requires an existing retained render owner".into(),
        );
    }
    let result = (|| -> Result<ThxV4Controller<T>> {
        scope.validate()?;
        plan.validate()?;
        ThxV4Controller::new(scope.product_id, transport)
    })();
    let mut controller = match result {
        Ok(controller) => controller,
        Err(error) => return failed(scope, plan, format!("{error:#}")),
    };
    let mut progress = execute_retained(scope, plan, &mut controller, validate, wait_retry);
    // One-off query cleanup is an ownership release, not source shutdown.
    if controller.transport().has_thx_binding() {
        if let Err(error) = controller.remove() {
            progress.cleanup_error = Some(format!("{error:#}"));
        }
    }
    progress
}

/// An existing owner retains its binding and exact HID handle across jobs.
/// Closing the source owner is explicit; a completed read/write cannot turn
/// into EQ/spatial=false. Getters stay separate from profile/UI state.
pub fn execute_retained<T: ThxJobTransport>(
    scope: ThxSubmissionScope,
    plan: ThxSubmissionPlan,
    controller: &mut ThxV4Controller<T>,
    mut validate: impl FnMut() -> Result<()>,
    mut wait_retry: impl FnMut(u64) -> Result<()>,
) -> ThxSubmissionProgress {
    let firmware_start = controller.transport().firmware_progress().len();
    let mut progress = ThxSubmissionProgress {
        scope: scope.clone(),
        plan: plan.clone(),
        render_setup: None,
        render_shutdown: None,
        roller_mapping: None,
        initial_observation: None,
        outcome: None,
        firmware: vec![],
        error: None,
        cleanup_error: None,
    };
    let result = (|| -> Result<()> {
        scope.validate()?;
        plan.validate()?;
        ensure!(
            controller.capability().product_id == scope.product_id,
            "THX retained owner product differs"
        );
        validate()?;
        if plan == ThxSubmissionPlan::FactoryReset1398 {
            ensure!(
                scope.product_id == razer_device::blackshark_reset::PRODUCT_ID,
                "Factory reset has no current audit for this product"
            );
            controller.reset_blackshark_local_profile()?;
            // DeviceReset's independent Valkyrie observer reads mic state.
            // A device-level getter failure must not suppress the reset setter.
            let mic = controller.transport_mut().exchange_hid(
                razer_device::thx_blackshark_firmware::BlackSharkFirmwareCommand::GetMicStatus1398,
            )?;
            crate::thx_direct::exceptional(mic.failure.as_ref())?;
            validate()?;
            let command = controller.transport_mut().exchange_hid(
                razer_device::thx_blackshark_firmware::BlackSharkFirmwareCommand::FactoryReset1398,
            )?;
            validate()?;
            ensure!(
                command.succeeded(),
                "Headset factory reset was not confirmed: {:?}",
                command.failure
            );
            return Ok(());
        }
        if plan == ThxSubmissionPlan::ReadFirmwareVersion {
            let command = controller.transport_mut().exchange_hid(
                razer_device::thx_blackshark_firmware::BlackSharkFirmwareCommand::GetFirmwareVersions,
            )?;
            validate()?;
            ensure!(
                command.succeeded(),
                "Headset firmware version query failed: {:?}",
                command.failure
            );
            ensure!(
                command.observed_firmware_version().is_some(),
                "Headset firmware response has no observed nonzero version"
            );
            return Ok(());
        }
        if plan.is_roller_mapping() {
            let mappings = match &plan {
                ThxSubmissionPlan::SetRollerMappings { mappings } => Some(mappings.as_slice()),
                _ => None,
            };
            progress.roller_mapping = Some(razer_device::blackshark_roller_mapping::execute(
                mappings,
                |command| controller.transport_mut().exchange_hid(command),
                &mut validate,
            ));
            return Ok(());
        }
        if plan == ThxSubmissionPlan::ShutdownRender {
            let shutdown = controller.shutdown_render(&mut validate);
            let completed = shutdown.shutdown_completed;
            let error = shutdown.error.clone();
            progress.render_shutdown = Some(shutdown);
            ensure!(
                completed,
                "THX source shutdown did not complete: {}",
                error.as_deref().unwrap_or("missing completion")
            );
            return Ok(());
        }
        if let ThxSubmissionPlan::SetupRender { trigger } = &plan {
            let setup = controller.setup_render(*trigger, &mut validate, &mut wait_retry);
            let succeeded = setup.setup_succeeded;
            let error = setup.error.clone();
            progress.render_setup = Some(setup);
            ensure!(
                succeeded,
                "THX render setup did not complete: {}",
                error.as_deref().unwrap_or("missing completion")
            );
            return Ok(());
        }
        if !controller.state().ready {
            controller.bind(|ms| {
                validate()?;
                wait_retry(ms)?;
                validate()
            })?;
        }
        validate()?;
        let observation =
            controller.refresh_spatial(&controller.observation_scope(), 0, &mut validate);
        let observed = observation.status == SpatialStatus::Observed;
        match &plan {
            ThxSubmissionPlan::SetupRender { .. } => unreachable!("handled above"),
            ThxSubmissionPlan::ShutdownRender => unreachable!("handled above"),
            ThxSubmissionPlan::ReadFirmwareVersion
            | ThxSubmissionPlan::FactoryReset1398
            | ThxSubmissionPlan::ReadRollerMapping
            | ThxSubmissionPlan::SetRollerMappings { .. } => {
                unreachable!("handled above")
            }
            ThxSubmissionPlan::ReadSpatial => {
                progress.outcome = Some(observation);
            }
            ThxSubmissionPlan::SetSpatial {
                enabled,
                presets,
                timer_tick,
            } => {
                let error = observation.error.clone();
                progress.initial_observation = Some(observation);
                ensure!(
                    observed,
                    "THX pre-write spatial observation failed: {}",
                    error.as_deref().unwrap_or("no Bool value")
                );
                let task = controller.prepare_spatial(*enabled, presets.clone(), *timer_tick);
                progress.outcome = Some(controller.execute_spatial(&task, &mut validate));
            }
        }
        Ok(())
    })();
    if let Err(error) = result {
        progress.error = Some(format!("{error:#}"));
    }
    progress.firmware = controller
        .transport()
        .firmware_progress()
        .into_iter()
        .skip(firmware_start)
        .collect();
    progress
}

#[cfg(test)]
#[path = "thx_tests.rs"]
mod tests;
