//! Worker entry point; the service client and physical task lease outlive I/O.
use razer_device::earbuds_auto_pause::{Operation, Report};
use razer_discovery::{direct, discovery::ObservedDevice};
use std::sync::{Arc, atomic::AtomicBool};

pub enum MonitorUpdate {
    Initial(Result<Report, String>),
    Ready,
    Notification(razer_device::earbuds_events::Notification),
}

pub fn monitor(
    route: ObservedDevice,
    active: Arc<AtomicBool>,
    refresh: Arc<AtomicBool>,
    mut publish: impl FnMut(MonitorUpdate) -> anyhow::Result<()>,
    before_shutdown: impl FnOnce() -> anyhow::Result<()>,
) -> crate::mouse_worker::Completion<()> {
    use razer_device::earbuds_events::{PollReply, StartReply};
    use razer_ipc::ServiceRequest;
    use std::{sync::atomic::Ordering, time::Duration};
    let keep = active.clone();
    crate::mouse_worker::device(
        route,
        active,
        |route| razer_device::earbuds_protocol40::supports_auto_pause(route.product_id()),
        move |client, route| {
            let result = crate::mouse_worker::guarded(|| {
                let session_id = uuid::Uuid::new_v4().to_string();
                let target = direct::earbuds_auto_pause::event_target(client, route)?;
                let started: StartReply = serde_json::from_value(client.request(
                    ServiceRequest::EarbudsEventsStart {
                        session_id: session_id.clone(),
                        target,
                    },
                )?)?;
                anyhow::ensure!(
                    started.session_id == session_id && started.started,
                    "Earbud event source did not start"
                );
                publish(MonitorUpdate::Initial(
                    direct::earbuds_auto_pause::request(client, route, Operation::Read)
                        .map_err(|error| format!("{error:#}")),
                ))?;
                publish(MonitorUpdate::Ready)?;
                while keep.load(Ordering::Acquire) {
                    if refresh.swap(false, Ordering::AcqRel) {
                        publish(MonitorUpdate::Initial(
                            direct::earbuds_auto_pause::request(client, route, Operation::Read)
                                .map_err(|error| format!("{error:#}")),
                        ))?;
                    }
                    let batch: PollReply = serde_json::from_value(client.request(
                        ServiceRequest::EarbudsEventsPoll {
                            session_id: session_id.clone(),
                        },
                    )?)?;
                    anyhow::ensure!(
                        batch.session_id == session_id,
                        "Earbud event response belongs to another owner"
                    );
                    for event in batch.events {
                        let observation = match &event {
                            razer_device::earbuds_events::Notification::Connection { .. } => {
                                Some(Operation::Read)
                            }
                            razer_device::earbuds_events::Notification::Battery { left, right } => {
                                Some(Operation::Battery {
                                    levels: Some([*left, *right]),
                                })
                            }
                            _ => None,
                        };
                        publish(MonitorUpdate::Notification(event))?;
                        if let Some(operation) = observation {
                            publish(MonitorUpdate::Initial(
                                direct::earbuds_auto_pause::request(client, route, operation)
                                    .map_err(|error| format!("{error:#}")),
                            ))?;
                        }
                    }
                    if let Some(error) = batch.error {
                        anyhow::bail!("Earbud event source failed: {error}");
                    }
                    if batch.terminal {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                // The common worker's unconditional Shutdown invokes the event owner's
                // cancel/join/finish chain even after the request guard is canceled.
                Ok(())
            })
            .map_err(anyhow::Error::msg);
            let cleanup = before_shutdown();
            match (result, cleanup) {
                (Ok(()), Ok(())) => Ok(()),
                (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
                (Err(error), Err(cleanup)) => {
                    anyhow::bail!("{error:#}; operation cleanup: {cleanup:#}")
                }
            }
        },
    )
}

pub fn device(
    route: ObservedDevice,
    operation: Operation,
    active: Arc<AtomicBool>,
) -> crate::mouse_worker::Completion<Report> {
    let mut retained = None;
    let completion = crate::mouse_worker::device(
        route,
        active,
        |route| razer_device::earbuds_protocol40::supports_auto_pause(route.product_id()),
        |client, route| {
            let report = direct::earbuds_auto_pause::request(client, route, operation)?;
            if report.equalizer.is_some() {
                retained = Some(report.clone());
            }
            Ok(report)
        },
    );
    retain_equalizer_completion(completion, retained)
}

fn retain_equalizer_completion(
    mut completion: crate::mouse_worker::Completion<Report>,
    retained: Option<Report>,
) -> crate::mouse_worker::Completion<Report> {
    if let (Err(error), Some(mut report)) = (&completion.result, retained.filter(|report| report.equalizer.is_some())) {
        let error = match &report.write {
            razer_device::earbuds_protocol40::WriteOutcome::Failed(previous) => format!("{previous}; {error}"),
            _ => error.clone(),
        };
        report.write = razer_device::earbuds_protocol40::WriteOutcome::Failed(error);
        completion.result = Ok(report);
    }
    completion
}

#[cfg(test)]
mod equalizer_tests {
    use super::*;
    use razer_device::{earbuds_equalizer::{Mode, Preset, Setting, Submission}, earbuds_protocol40::WriteOutcome};
    #[test]
    fn earbuds_equalizer_post_delivery_cancellation_retains_ack_and_cleanup_failure() {
        let report = Report::from_equalizer(2664, razer_device::earbuds_equalizer::Report {
            setting: Setting { mode: Mode::Standard, preset: Preset::Custom, bands: [0; 10] },
            preset: Submission::Confirmed, bands: Submission::Confirmed,
        }).unwrap();
        let completion = retain_equalizer_completion(crate::mouse_worker::Completion {
            result: Err("scope expired after IPC delivery".into()),
            cleanup_failure: Some("shutdown failed".into()),
        }, Some(report));
        assert_eq!(completion.cleanup_failure.as_deref(), Some("shutdown failed"));
        let report = completion.result.unwrap();
        assert!(matches!(report.write, WriteOutcome::Failed(_)));
        let submission = report.equalizer.unwrap();
        assert!(submission.preset.acknowledged() && submission.bands.acknowledged());
        let missing = retain_equalizer_completion(crate::mouse_worker::Completion {
            result: Err("no response".into()), cleanup_failure: None,
        }, None);
        assert!(missing.result.is_err());
    }
}
