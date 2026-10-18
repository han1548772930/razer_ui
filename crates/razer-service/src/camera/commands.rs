//! Camera IPC commands execute on the retained camera worker owner.
//! Completion follows the operation, native Shutdown and process reclamation.
use razer_device::camera_submission::{
    CameraCommand, CameraOperationStatus, CameraScope, CameraSubmissionPlan,
    CameraSubmissionProgress,
};
use razer_ipc::{ServiceClient, ServiceRequest};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

/// Retain the native notification owner on this I/O worker through unregister
/// and thread join; the page receives only observed events.
pub fn observe_sessions(
    canceled: Arc<AtomicBool>,
    outgoing: async_channel::Sender<razer_platform::session_changes::SessionChange>,
) -> Result<(), String> {
    use razer_platform::session_changes::{SessionChange, SessionMonitor};
    if canceled.load(Ordering::Acquire) {
        return Ok(());
    }
    let (monitor, events) = SessionMonitor::start()?;
    let result = loop {
        if canceled.load(Ordering::Acquire) || outgoing.is_closed() {
            break Ok(());
        }
        match events.try_recv() {
            Ok(event) => {
                if outgoing.send_blocking(event).is_err() {
                    break Ok(());
                }
            }
            Err(async_channel::TryRecvError::Empty) => thread::sleep(Duration::from_millis(40)),
            Err(async_channel::TryRecvError::Closed) => {
                break Err("camera session observation channel closed".to_owned());
            }
        }
    };
    let cleanup = monitor.shutdown();
    let result = match (result, cleanup) {
        (Err(error), Err(cleanup)) => Err(format!("{error}; camera session cleanup: {cleanup}")),
        (Err(error), _) | (_, Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    };
    if let Err(error) = &result {
        let _ = outgoing.send_blocking(SessionChange::Failed(error.clone()));
    }
    result
}

pub fn execute(
    scope: CameraScope,
    plan: CameraSubmissionPlan,
    signal: Arc<AtomicBool>,
    stopped: Option<mpsc::Receiver<Result<(), String>>>,
) -> Result<CameraSubmissionProgress, String> {
    execute_named(scope, plan, signal, stopped, None)
}
pub fn execute_named(
    scope: CameraScope,
    plan: CameraSubmissionPlan,
    signal: Arc<AtomicBool>,
    stopped: Option<mpsc::Receiver<Result<(), String>>>,
    task_name: Option<String>,
) -> Result<CameraSubmissionProgress, String> {
    let run = || -> anyhow::Result<CameraSubmissionProgress> {
        anyhow::ensure!(
            !signal.load(Ordering::Acquire),
            "camera operation canceled before connection"
        );
        if let Some(stopped) = stopped {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                anyhow::ensure!(
                    !signal.load(Ordering::Acquire),
                    "camera operation canceled while stopping capture"
                );
                match stopped.recv_timeout(Duration::from_millis(40)) {
                    Ok(result) => {
                        result.map_err(anyhow::Error::msg)?;
                        break;
                    }
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        anyhow::bail!("camera stop receipt disconnected")
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => {
                        anyhow::ensure!(Instant::now() < deadline, "camera capture stop timed out")
                    }
                }
            }
        }
        let properties = plan
            .commands
            .iter()
            .any(|c| matches!(c, CameraCommand::ShowProperties));
        let mut client = ServiceClient::spawn()?;
        let id = uuid::Uuid::new_v4().to_string();
        let named = task_name.is_some();
        let result = (|| -> anyhow::Result<CameraSubmissionProgress> {
            client.request(match task_name {
                Some(task_name) => ServiceRequest::CameraTaskStart {
                    operation_id: id.clone(),
                    scope,
                    task_name,
                    plan,
                },
                None => ServiceRequest::CameraStart {
                    operation_id: id.clone(),
                    scope,
                    plan,
                },
            })?;
            let deadline = Instant::now() + Duration::from_secs(30);
            let mut cancel_sent = false;
            loop {
                if signal.load(Ordering::Acquire) && !cancel_sent {
                    client.request(ServiceRequest::CameraCancel {
                        operation_id: id.clone(),
                    })?;
                    cancel_sent = true;
                }
                let status: CameraOperationStatus =
                    serde_json::from_value(client.request(ServiceRequest::CameraPoll {
                        operation_id: id.clone(),
                    })?)?;
                if let CameraOperationStatus::Finished(p) = status {
                    return Ok(p);
                }
                anyhow::ensure!(
                    properties || named || Instant::now() < deadline,
                    "camera response timed out; device outcome remains unconfirmed"
                );
                thread::sleep(Duration::from_millis(40));
            }
        })();
        // The worker joins the camera thread and releases COM before its reply.
        let shutdown = client.request(ServiceRequest::Shutdown).map(|_| ());
        let termination = client.finish_termination();
        drop(client);
        finish_progress(result, shutdown, termination)
    };
    run().map_err(|e| format!("{e:#}"))
}

fn finish_progress(
    result: anyhow::Result<CameraSubmissionProgress>,
    shutdown: anyhow::Result<()>,
    termination: anyhow::Result<()>,
) -> anyhow::Result<CameraSubmissionProgress> {
    let errors: Vec<_> = [
        ("camera Shutdown", shutdown),
        ("camera process release", termination),
    ]
    .into_iter()
    .filter_map(|(stage, result)| result.err().map(|error| format!("{stage}: {error:#}")))
    .collect();
    if errors.is_empty() {
        return result;
    }
    let cleanup = errors.join("; ");
    match result {
        Ok(mut progress) => {
            progress.completed = false;
            progress.error = Some(match progress.error {
                Some(error) => format!("{error}; {cleanup}"),
                None => cleanup,
            });
            Ok(progress)
        }
        Err(error) => Err(anyhow::anyhow!("{error:#}; {cleanup}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn camera_cleanup_preserves_real_outcomes_and_both_independent_release_failures() {
        let progress = CameraSubmissionProgress {
            completed: true,
            outcomes: vec![razer_device::camera_submission::CameraCommandOutcome::PropertiesClosed],
            error: Some("device returned a partial result".into()),
            ..Default::default()
        };
        let result = finish_progress(
            Ok(progress),
            Err(anyhow::anyhow!("native release failed")),
            Err(anyhow::anyhow!("pipe join failed")),
        )
        .unwrap();
        assert!(!result.completed);
        assert_eq!(
            result.outcomes,
            vec![razer_device::camera_submission::CameraCommandOutcome::PropertiesClosed]
        );
        let error = result.error.unwrap();
        assert!(error.contains("device returned a partial result"));
        assert!(error.contains("native release failed"));
        assert!(error.contains("pipe join failed"));
    }

    #[test]
    fn camera_operation_failure_is_not_replaced_by_cleanup_success_or_failure() {
        for fail in [false, true] {
            let cleanup = if fail {
                Err(anyhow::anyhow!("Shutdown rejected"))
            } else {
                Ok(())
            };
            let error = finish_progress(Err(anyhow::anyhow!("GET rejected")), cleanup, Ok(()))
                .unwrap_err()
                .to_string();
            assert!(error.contains("GET rejected"));
            assert_eq!(error.contains("Shutdown rejected"), fail);
        }
    }
}
