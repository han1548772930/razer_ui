//! PID 70's direct legacy factory and a fully drained calibration submission.
use super::*;
use razer_device::mouse_philips::{self as philips, Progress, Setting, Target};
use std::sync::atomic::{AtomicBool, Ordering};
pub type Completion = super::feature_submission::Completion<Setting, philips::SubmissionReport>;
#[derive(serde::Deserialize)]
struct Shutdown {
    stopped: bool,
    mouse_philips_cleanup: Option<Progress>,
}

pub fn available(observed: &ObservedDevice) -> bool {
    let cap = razer_device::mouse_legacy_polling::read_capability();
    observed.product_id() == philips::PRODUCT_ID
        && observed.physical_product_id() == philips::PRODUCT_ID
        && observed.peer_product_id().is_none()
        && cap.direct_pids.contains(&observed.physical_product_id())
        && observed.transport() == Some(crate::discovery::ObservedTransport::Wired)
}
pub fn request(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
    operation_id: String,
    setting: Setting,
    valid: &AtomicBool,
) -> Completion {
    let mut target = None;
    let result = (|| {
        ensure!(
            valid.load(Ordering::Acquire),
            "Philips calibration scope canceled before resolution"
        );
        ensure!(
            available(observed),
            "Philips calibration has no live source wired route"
        );
        let cap = razer_device::mouse_legacy_polling::read_capability();
        let route = super::resolve(client, observed, cap)?;
        target = Some(match route {
            DirectRoute::Collection { node, product_id } => Target::Collection {
                node,
                product_id,
                peer_product_id: None,
            },
            #[cfg(windows)]
            DirectRoute::Container(target) => Target::WindowsContainer(target),
        });
        let selected = target.as_ref().expect("resolved Philips target");
        ensure!(
            valid.load(Ordering::Acquire),
            "Philips calibration scope canceled before service start"
        );
        run_with(selected.clone(), &operation_id, setting, valid, |request| {
            client.request(request)
        })
    })();
    let shutdown = client.request(ServiceRequest::Shutdown);
    complete(result, shutdown, target.as_ref(), &operation_id, setting)
}
fn run_with(
    target: Target,
    id: &str,
    setting: Setting,
    valid: &AtomicBool,
    request: impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<Progress> {
    let commands = super::feature_submission::Commands {
        start: ServiceRequest::MousePhilipsStart {
            operation_id: id.into(),
            target: target.clone(),
            setting,
        },
        poll: |operation_id| ServiceRequest::MousePhilipsPoll { operation_id },
        cancel: |operation_id| ServiceRequest::MousePhilipsCancel { operation_id },
    };
    super::feature_submission::run_with(id, valid, commands, request, |progress| {
        validate_progress(progress, id, &target, setting)
    })
}
fn complete(
    result: anyhow::Result<Progress>,
    shutdown: anyhow::Result<Value>,
    target: Option<&Target>,
    id: &str,
    setting: Setting,
) -> Completion {
    let shutdown = (|| -> anyhow::Result<_> {
        let shutdown: Shutdown = serde_json::from_value(shutdown?)?;
        Ok((shutdown.stopped, shutdown.mouse_philips_cleanup))
    })();
    super::feature_submission::complete("Philips calibration", result, shutdown, |progress| {
        validate_progress(
            progress,
            id,
            target.context("Philips cleanup has no resolved target")?,
            setting,
        )
    })
}
fn validate_progress(
    progress: &Progress,
    id: &str,
    target: &Target,
    setting: Setting,
) -> anyhow::Result<()> {
    ensure!(
        target.product_id() == philips::PRODUCT_ID,
        "Philips target refers to another feature"
    );
    ensure!(
        progress.operation_id == id && progress.target == *target && progress.requested == setting,
        "Philips response owner, target or settings differ"
    );
    ensure!(
        progress.terminal || (progress.report.is_none() && progress.error.is_none()),
        "Nonterminal Philips response contains an outcome"
    );
    ensure!(
        !progress.terminal || progress.report.is_some() != progress.error.is_some(),
        "Terminal Philips response needs exactly one outcome"
    );
    if let Some(report) = &progress.report {
        ensure!(
            report.requested == setting && report.transport_source_class == "rzDevice25Legacy",
            "Philips report settings or transport differ"
        );
        let commands = philips::prepare(setting);
        let acknowledgements = match &report.outcome {
            Ok(result) => {
                ensure!(
                    result.requested == setting
                        && result.source_chain_acknowledged
                        && !result.verified
                        && result.acknowledgements.len() == commands.len(),
                    "Philips setter ACKs cannot claim effective readback"
                );
                &result.acknowledgements
            }
            Err(error) => {
                ensure!(
                    error.requested == setting && !error.error.is_empty(),
                    "Philips failure lost its setting or error"
                );
                &error.acknowledgements
            }
        };
        ensure!(
            acknowledgements.len() <= commands.len()
                && acknowledgements
                    .iter()
                    .zip(commands)
                    .all(|(ack, command)| ack.step == command.step),
            "Philips ACKs differ from the source command order"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn target() -> Target {
        Target::WindowsContainer(razer_device::device_reads::DeviceReadTarget {
            product_id: 70,
            physical_product_id: 70,
            peer_product_id: None,
            device_container_id: "simulated-container".into(),
            path: "simulated-hid".into(),
        })
    }
    fn partial(id: &str) -> Progress {
        Progress {
            operation_id: id.into(),
            target: target(),
            requested: Setting::Disabled,
            terminal: true,
            canceled: true,
            error: None,
            report: Some(philips::SubmissionReport {
                requested: Setting::Disabled,
                transport_source_class: "rzDevice25Legacy".into(),
                initialization_error: None,
                outcome: Err(philips::WriteFailure {
                    requested: Setting::Disabled,
                    phase: philips::FailurePhase::AfterAcknowledgement,
                    step: Some(philips::Step::SensorState),
                    acknowledgements: vec![philips::Acknowledgement {
                        step: philips::Step::SensorState,
                        payload: vec![0, 5, 0],
                    }],
                    command_may_have_reached_device: false,
                    error: "Scope canceled after sensor ACK".into(),
                }),
            }),
        }
    }
    #[test]
    fn cancellation_uses_philips_endpoint_and_waits_for_joined_terminal() {
        let valid = AtomicBool::new(true);
        let mut calls = Vec::new();
        let result = run_with(target(), "philips", Setting::Disabled, &valid, |request| {
            let terminal = matches!(request, ServiceRequest::MousePhilipsCancel { .. });
            calls.push(match request {
                ServiceRequest::MousePhilipsStart { .. } => "start",
                ServiceRequest::MousePhilipsCancel { .. } => "cancel",
                _ => panic!("Unexpected endpoint"),
            });
            valid.store(false, Ordering::Release);
            let mut progress = partial("philips");
            if !terminal {
                progress.terminal = false;
                progress.canceled = false;
                progress.report = None;
            }
            Ok(serde_json::to_value(progress)?)
        })
        .unwrap();
        assert!(result.terminal && result.canceled);
        assert_eq!(calls, ["start", "cancel"]);
    }
    #[test]
    fn cleanup_retains_ack_after_polling_failure_and_rejects_wrong_feature() {
        let progress = partial("philips");
        let completion = complete(
            Err(anyhow::anyhow!("Polling disconnected")),
            Ok(json!({"stopped":true,"mouse_philips_cleanup":progress})),
            Some(&target()),
            "philips",
            Setting::Disabled,
        );
        assert_eq!(completion.progress, Some(progress));
        assert!(completion.error.unwrap().contains("Polling disconnected"));
        let mut wrong = target();
        if let Target::WindowsContainer(target) = &mut wrong {
            target.product_id = 170;
        }
        assert!(
            validate_progress(&partial("philips"), "philips", &wrong, Setting::Disabled).is_err()
        );
    }
    #[test]
    fn cleanup_failure_or_unjoined_report_never_becomes_success() {
        for reply in [
            json!({"stopped":false}),
            json!({"stopped":true,"mouse_philips_cleanup":partial("orca")}),
            json!({"mouse_philips_cleanup":null}),
        ] {
            let completion = complete(
                Err(anyhow::anyhow!("Polling failed")),
                Ok(reply),
                Some(&target()),
                "philips",
                Setting::Disabled,
            );
            assert!(completion.progress.is_none());
            assert!(completion.error.unwrap().contains("worker cleanup failed"));
        }
    }
}
