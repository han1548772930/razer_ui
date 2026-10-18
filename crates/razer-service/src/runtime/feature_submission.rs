//! A bounded physical command owns cancellation and its worker through join.
use anyhow::{Context as _, ensure};
use razer_device::feature_submission::{Progress, Target};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, JoinHandle},
};

pub(super) trait WorkerReport {
    fn cleanup_error(&self) -> Option<String>;
}
struct Operation<S, R> {
    id: String,
    target: Target,
    setting: S,
    canceled: Arc<AtomicBool>,
    worker: JoinHandle<anyhow::Result<R>>,
}
pub(super) struct Controller<S: Copy, R: WorkerReport> {
    active: Option<Operation<S, R>>,
}
impl<S: Copy, R: WorkerReport> Default for Controller<S, R> {
    fn default() -> Self {
        Self { active: None }
    }
}
impl<S: Copy, R: WorkerReport> Controller<S, R> {
    pub(super) fn start_with(
        &mut self,
        id: String,
        target: Target,
        setting: S,
        execute: impl FnOnce(Target, S, Arc<AtomicBool>) -> anyhow::Result<R> + Send + 'static,
    ) -> anyhow::Result<Progress<S, R>>
    where
        S: Send + 'static,
        R: Send + 'static,
    {
        ensure!(
            !id.is_empty() && id.len() <= 128 && !id.contains('\0'),
            "Feature operation ID is invalid"
        );
        ensure!(
            self.active.is_none(),
            "Previous feature operation has not been joined"
        );
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = canceled.clone();
        let selected = target.clone();
        let worker = thread::Builder::new()
            .name("hid-submission".into())
            .spawn(move || execute(selected, setting, signal))?;
        self.active = Some(Operation {
            id: id.clone(),
            target: target.clone(),
            setting,
            canceled,
            worker,
        });
        Ok(Progress {
            operation_id: id,
            target,
            requested: setting,
            terminal: false,
            canceled: false,
            report: None,
            error: None,
        })
    }
    pub(super) fn cancel(&mut self, id: &str) -> anyhow::Result<Progress<S, R>> {
        let active = self
            .active
            .as_ref()
            .context("Feature operation is absent")?;
        ensure!(active.id == id, "Feature operation owner differs");
        active.canceled.store(true, Ordering::Release);
        self.poll(id)
    }
    pub(super) fn poll(&mut self, id: &str) -> anyhow::Result<Progress<S, R>> {
        let active = self
            .active
            .as_ref()
            .context("Feature operation is absent")?;
        ensure!(active.id == id, "Feature operation owner differs");
        let mut progress = Progress {
            operation_id: id.into(),
            target: active.target.clone(),
            requested: active.setting,
            terminal: false,
            canceled: active.canceled.load(Ordering::Acquire),
            report: None,
            error: None,
        };
        if !active.worker.is_finished() {
            return Ok(progress);
        }
        let active = self.active.take().expect("checked active operation");
        progress.terminal = true;
        progress.canceled = active.canceled.load(Ordering::Acquire);
        match active.worker.join() {
            Ok(Ok(report)) => progress.report = Some(report),
            Ok(Err(error)) => progress.error = Some(format!("{error:#}")),
            Err(_) => {
                progress.error =
                    Some("Feature worker terminated; hardware completion is unknown".into())
            }
        }
        Ok(progress)
    }
    pub(super) fn shutdown(&mut self) -> anyhow::Result<Option<Progress<S, R>>> {
        let Some(active) = self.active.take() else {
            return Ok(None);
        };
        active.canceled.store(true, Ordering::Release);
        let outcome = active.worker.join().map_err(|_| {
            anyhow::anyhow!(
                "Feature worker terminated during cleanup; hardware completion is unknown"
            )
        })?;
        let (report, error) = match outcome {
            Ok(report) => (Some(report), None),
            Err(error) => (None, Some(format!("{error:#}"))),
        };
        Ok(Some(Progress {
            operation_id: active.id,
            target: active.target,
            requested: active.setting,
            terminal: true,
            canceled: true,
            report,
            error,
        }))
    }
}
impl<S: Copy, R: WorkerReport> Drop for Controller<S, R> {
    fn drop(&mut self) {
        match self.shutdown() {
            Ok(Some(progress)) => {
                if let Some(error) = progress.error {
                    eprintln!("[hid-submission] Retired worker: {error}");
                }
                if let Some(error) = progress
                    .report
                    .as_ref()
                    .and_then(WorkerReport::cleanup_error)
                {
                    eprintln!("[hid-submission] Retired command chain: {error}");
                }
            }
            Ok(None) => {}
            Err(error) => eprintln!("[hid-submission] Worker cleanup failed: {error:#}"),
        }
    }
}
