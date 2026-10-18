//! Shared IPC lifetime for a bounded worker: cancellation, join and shutdown.
use anyhow::ensure;
use razer_device::feature_submission::Progress;
use razer_ipc::ServiceRequest;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::{
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

pub struct Completion<S, R> {
    pub progress: Option<Progress<S, R>>,
    pub error: Option<String>,
}
pub(super) struct Commands {
    pub start: ServiceRequest,
    pub poll: fn(String) -> ServiceRequest,
    pub cancel: fn(String) -> ServiceRequest,
}
pub(super) fn run_with<S: DeserializeOwned, R: DeserializeOwned>(
    id: &str,
    valid: &AtomicBool,
    commands: Commands,
    mut request: impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
    validate: impl Fn(&Progress<S, R>) -> anyhow::Result<()>,
) -> anyhow::Result<Progress<S, R>> {
    let mut progress = serde_json::from_value(request(commands.start)?)?;
    let started = Instant::now();
    let mut cancel_sent = false;
    loop {
        validate(&progress)?;
        if progress.terminal {
            return Ok(progress);
        }
        if !cancel_sent
            && (!valid.load(Ordering::Acquire) || started.elapsed() >= Duration::from_secs(25))
        {
            cancel_sent = true;
            progress = serde_json::from_value(request((commands.cancel)(id.into()))?)?;
        } else {
            std::thread::sleep(Duration::from_millis(25));
            progress = serde_json::from_value(request((commands.poll)(id.into()))?)?;
        }
        ensure!(
            started.elapsed() < Duration::from_secs(35),
            "Feature cleanup deadline exceeded; worker shutdown is required"
        );
    }
}
/// The feature parses its typed Shutdown envelope, then supplies the exact
/// target/setting validator. Failed polling still retains real cleanup ACKs.
pub(super) fn complete<S, R>(
    feature: &str,
    result: anyhow::Result<Progress<S, R>>,
    shutdown: anyhow::Result<(bool, Option<Progress<S, R>>)>,
    validate: impl Fn(&Progress<S, R>) -> anyhow::Result<()>,
) -> Completion<S, R> {
    let mut errors = Vec::new();
    let mut progress = match result {
        Ok(progress) => Some(progress),
        Err(error) => {
            errors.push(format!("{feature} transport completion failed: {error:#}"));
            None
        }
    };
    let cleanup = (|| -> anyhow::Result<()> {
        let (stopped, cleanup) = shutdown?;
        ensure!(stopped, "Feature worker did not confirm owned cleanup");
        if let Some(cleanup) = cleanup {
            validate(&cleanup)?;
            ensure!(
                cleanup.terminal,
                "Feature cleanup returned before its adapter was joined"
            );
            ensure!(
                progress.is_none(),
                "Feature worker returned a second terminal operation"
            );
            progress = Some(cleanup);
        }
        Ok(())
    })();
    if let Err(error) = cleanup {
        errors.push(format!("{feature} worker cleanup failed: {error:#}"));
    }
    Completion {
        progress,
        error: (!errors.is_empty()).then(|| errors.join("; ")),
    }
}
