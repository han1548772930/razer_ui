//! Resolve the audited physical factory and drain cancellation before release.
use super::*;
use razer_device::mouse_tracking::{self as tracking, Progress, Target, TrackingSetting};
use razer_device::receiver_session::ReceiverSession;
use std::sync::atomic::{AtomicBool, Ordering};

/// A transport error can coexist with a final, acknowledged partial chain.
/// The service is drained before either fact is delivered to its caller.
pub type Completion =
    super::feature_submission::Completion<TrackingSetting, tracking::SubmissionReport>;

struct Resolved {
    target: Target,
    source_class: String,
    session: Option<ReceiverSession>,
}

#[derive(serde::Deserialize)]
struct Shutdown {
    stopped: bool,
    mouse_tracking_cleanup: Option<Progress>,
}

pub fn available(observed: &ObservedDevice) -> bool {
    tracking::capability(observed.product_id()).is_some()
        && super::mouse_current_dpi::available(observed)
}

pub fn request(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
    operation_id: String,
    setting: TrackingSetting,
    valid: &AtomicBool,
) -> Completion {
    let mut resolved = None;
    let mut task_lease = None;
    let result = (|| {
        task_lease = Some(
            razer_ipc::device_tasks::Registry::global()
                .shared(observed.device_task_scope()?, || {
                    !valid.load(Ordering::Acquire)
                })?,
        );
        resolved = Some(resolve(client, observed, setting, valid)?);
        let scope = resolved.as_ref().expect("resolved tracking scope");
        ensure!(
            valid.load(Ordering::Acquire),
            "Tracking scope canceled before service start"
        );
        let progress = run_with(
            scope.target.clone(),
            operation_id.clone(),
            setting,
            valid,
            |request| client.request(request),
        )?;
        scope.validate(&progress, &operation_id, setting)?;
        Ok(progress)
    })();
    // Shutdown joins the retained adapter even if polling or validation failed.
    // Its reply can contain the last real ACKs from a canceled operation.
    let shutdown = client.request(ServiceRequest::Shutdown);
    drop(task_lease);
    complete(result, shutdown, &operation_id, setting, resolved.as_ref())
}

fn resolve(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
    setting: TrackingSetting,
    valid: &AtomicBool,
) -> anyhow::Result<Resolved> {
    ensure!(
        valid.load(Ordering::Acquire),
        "Tracking scope canceled before resolution"
    );
    ensure!(
        available(observed),
        "Tracking has no live current source route"
    );
    tracking::prepare(
        tracking::capability(observed.product_id()).context("Orca Plus source is absent")?,
        setting,
    )?;
    let source = razer_device::mouse_current_dpi::capability(observed.product_id())
        .context("Tracking physical factory is absent")?;
    let session = observed
        .peer_product_id()
        .map(|peer| source.receiver_session(observed.physical_product_id(), peer))
        .transpose()?;
    let route = if let Some(session) = &session {
        super::receiver_settings::resolve(observed, session, &mut |request| {
            client.request(request)
        })?
    } else {
        super::mouse_current_dpi::resolve(client, observed)?
    };
    let target = match route {
        DirectRoute::Collection { node, product_id } => Target::Collection {
            node,
            product_id,
            peer_product_id: observed.peer_product_id(),
        },
        #[cfg(windows)]
        DirectRoute::Container(target) => Target::WindowsContainer(target),
    };
    let cap = if let Some(session) = &session {
        session.transport().clone()
    } else {
        source
            .transport()
            .for_physical_product(observed.physical_product_id())?
    };
    Ok(Resolved {
        target,
        source_class: cap.source_class,
        session,
    })
}

impl Resolved {
    fn validate(
        &self,
        progress: &Progress,
        operation_id: &str,
        setting: TrackingSetting,
    ) -> anyhow::Result<()> {
        validate_progress(progress, operation_id, &self.target, setting)?;
        if let Some(report) = &progress.report {
            ensure!(
                report.transport_source_class == self.source_class,
                "Tracking response sender differs"
            );
            if report.acknowledged() {
                if let Some(session) = &self.session {
                    session.validate_peer(
                        report
                            .receiver_before
                            .as_deref()
                            .context("Tracking receiver pre-observation is absent")?,
                    )?;
                    session.validate_peer(
                        report
                            .receiver_after
                            .as_deref()
                            .context("Tracking receiver post-observation is absent")?,
                    )?;
                }
            }
        }
        Ok(())
    }
}

fn complete(
    result: anyhow::Result<Progress>,
    shutdown: anyhow::Result<Value>,
    operation_id: &str,
    setting: TrackingSetting,
    resolved: Option<&Resolved>,
) -> Completion {
    let shutdown = (|| -> anyhow::Result<_> {
        let shutdown: Shutdown = serde_json::from_value(shutdown?)?;
        Ok((shutdown.stopped, shutdown.mouse_tracking_cleanup))
    })();
    super::feature_submission::complete("Tracking", result, shutdown, |progress| {
        resolved
            .context("Tracking cleanup has no resolved target")?
            .validate(progress, operation_id, setting)
    })
}

fn run_with(
    target: Target,
    operation_id: String,
    setting: TrackingSetting,
    valid: &AtomicBool,
    request: impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<Progress> {
    let commands = super::feature_submission::Commands {
        start: ServiceRequest::MouseTrackingStart {
            operation_id: operation_id.clone(),
            target: target.clone(),
            setting,
        },
        poll: |operation_id| ServiceRequest::MouseTrackingPoll { operation_id },
        cancel: |operation_id| ServiceRequest::MouseTrackingCancel { operation_id },
    };
    super::feature_submission::run_with(&operation_id, valid, commands, request, |progress| {
        validate_progress(progress, &operation_id, &target, setting)
    })
}

fn validate_progress(
    progress: &Progress,
    id: &str,
    target: &Target,
    setting: TrackingSetting,
) -> anyhow::Result<()> {
    ensure!(
        progress.operation_id == id && progress.target == *target && progress.requested == setting,
        "Tracking response owner, target or settings differ"
    );
    ensure!(
        progress.terminal || (progress.report.is_none() && progress.error.is_none()),
        "Nonterminal tracking response contains a fabricated outcome"
    );
    ensure!(
        !progress.terminal || progress.report.is_some() != progress.error.is_some(),
        "Terminal tracking response needs exactly one outcome"
    );
    if let Some(report) = &progress.report {
        ensure!(
            report.requested == setting,
            "Tracking report settings differ"
        );
        let commands = tracking::prepare(
            tracking::capability(target.product_id())
                .context("Tracking product source is absent")?,
            setting,
        )?;
        if let Some(outcome) = &report.outcome {
            let acknowledgements = match outcome {
                Ok(result) => {
                    ensure!(
                        result.requested == setting
                            && result.source_chain_acknowledged
                            && !result.verified
                            && result.acknowledgements.len() == commands.len(),
                        "Tracking setter ACKs cannot claim getter verification"
                    );
                    &result.acknowledgements
                }
                Err(error) => {
                    ensure!(
                        error.requested == setting && !error.error.is_empty(),
                        "Tracking failure lost its requested settings or error"
                    );
                    &error.acknowledgements
                }
            };
            ensure!(
                acknowledgements.len() <= commands.len()
                    && acknowledgements
                        .iter()
                        .zip(&commands)
                        .all(|(ack, command)| ack.step == command.step),
                "Tracking ACKs differ from the source command order"
            );
        } else {
            ensure!(
                report.observation_error.is_some(),
                "Tracking report contains no attempted operation or observation failure"
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn target() -> Target {
        Target::WindowsContainer(razer_device::device_reads::DeviceReadTarget {
            product_id: 170,
            physical_product_id: 170,
            peer_product_id: None,
            device_container_id: "simulated-container".into(),
            path: "simulated-path".into(),
        })
    }
    #[test]
    fn cancellation_is_drained_to_terminal_with_exact_owner_identity() {
        let target = target();
        let setting = TrackingSetting::Symmetric {
            tracking_distance: 2,
        };
        let valid = AtomicBool::new(true);
        let mut calls = Vec::new();
        let result = run_with(target.clone(), "owner".into(), setting, &valid, |request| {
            let terminal = matches!(request, ServiceRequest::MouseTrackingCancel { .. });
            calls.push(match request {
                ServiceRequest::MouseTrackingStart { .. } => "start",
                ServiceRequest::MouseTrackingCancel { .. } => "cancel",
                _ => panic!("Unexpected polling before cancellation"),
            });
            valid.store(false, Ordering::Release);
            Ok(serde_json::to_value(Progress {
                operation_id: "owner".into(),
                target: target.clone(),
                requested: setting,
                terminal,
                canceled: terminal,
                report: None,
                error: terminal.then(|| "Canceled before write".into()),
            })?)
        })
        .unwrap();
        assert_eq!(calls, ["start", "cancel"]);
        assert!(result.terminal && result.canceled);
    }
    #[test]
    fn another_target_or_owner_and_unverified_empty_terminal_are_rejected() {
        let target = target();
        let setting = TrackingSetting::Symmetric {
            tracking_distance: 2,
        };
        let mut progress = Progress {
            operation_id: "old-owner".into(),
            target: target.clone(),
            requested: setting,
            terminal: false,
            canceled: false,
            report: None,
            error: None,
        };
        assert!(validate_progress(&progress, "new-owner", &target, setting).is_err());
        progress.operation_id = "owner".into();
        progress.terminal = true;
        assert!(validate_progress(&progress, "owner", &target, setting).is_err());
        progress.error = Some("Genuine adapter failure".into());
        let mut wrong = target.clone();
        if let Target::WindowsContainer(wrong) = &mut wrong {
            wrong.path = "replacement-device".into();
        }
        assert!(validate_progress(&progress, "owner", &wrong, setting).is_err());
        assert!(validate_progress(&progress, "owner", &target, setting).is_ok());
    }

    fn scope() -> Resolved {
        Resolved {
            target: target(),
            source_class: "rzDevice25".into(),
            session: None,
        }
    }

    fn partial(scope: &Resolved, id: &str, setting: TrackingSetting) -> Progress {
        Progress {
            operation_id: id.into(),
            target: scope.target.clone(),
            requested: setting,
            terminal: true,
            canceled: true,
            error: None,
            report: Some(tracking::SubmissionReport {
                requested: setting,
                transport_source_class: scope.source_class.clone(),
                outcome: Some(Err(tracking::TrackingWriteFailure {
                    requested: setting,
                    phase: tracking::TrackingFailurePhase::BeforeCommand,
                    step: Some(tracking::TrackingStep::LiftSetting),
                    acknowledgements: vec![tracking::TrackingAcknowledgement {
                        step: tracking::TrackingStep::SensorState,
                        payload: vec![0, 4, 1],
                    }],
                    command_may_have_reached_device: false,
                    error: "Canceled before next setter".into(),
                })),
                receiver_before: None,
                receiver_after: None,
                observation_error: None,
            }),
        }
    }

    #[test]
    fn shutdown_drains_and_retains_partial_acknowledgements_after_polling_failure() {
        let scope = scope();
        let setting = TrackingSetting::Symmetric {
            tracking_distance: 2,
        };
        let cleanup = partial(&scope, "owner", setting);
        let completion = complete(
            Err(anyhow::anyhow!("Polling interrupted")),
            Ok(serde_json::json!({"stopped":true,"mouse_tracking_cleanup":cleanup})),
            "owner",
            setting,
            Some(&scope),
        );
        assert_eq!(completion.progress, Some(cleanup));
        assert!(completion.error.unwrap().contains("Polling interrupted"));
    }

    #[test]
    fn shutdown_rejects_wrong_owner_unjoined_and_unconfirmed_cleanup() {
        let scope = scope();
        let setting = TrackingSetting::Symmetric {
            tracking_distance: 2,
        };
        let mut unjoined = partial(&scope, "owner", setting);
        unjoined.terminal = false;
        unjoined.report = None;
        for shutdown in [
            serde_json::json!({"stopped":true,"mouse_tracking_cleanup":partial(&scope, "another-owner", setting)}),
            serde_json::json!({"stopped":true,"mouse_tracking_cleanup":unjoined}),
            serde_json::json!({"stopped":false}),
            serde_json::json!({"mouse_tracking_cleanup":null}),
        ] {
            let completion = complete(
                Err(anyhow::anyhow!("Polling interrupted")),
                Ok(shutdown),
                "owner",
                setting,
                Some(&scope),
            );
            assert!(completion.progress.is_none());
            assert!(completion.error.unwrap().contains("worker cleanup failed"));
        }
    }
}
