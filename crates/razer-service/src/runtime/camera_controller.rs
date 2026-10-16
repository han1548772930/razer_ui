//! Own the COM thread until completion/cleanup, including cancellation and shutdown.
use anyhow::{Result, ensure};
use razer_device::camera_submission::{
    CameraCommandTransport, CameraOperationStatus, CameraScope, CameraSubmissionPlan,
    CameraSubmissionProgress, execute_camera_submission,
};
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
};
struct Job {
    scope: CameraScope,
    task_name: Option<String>,
    canceled: Arc<AtomicBool>,
    receiver: mpsc::Receiver<CameraSubmissionProgress>,
    thread: Option<JoinHandle<()>>,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.canceled.store(true, Ordering::Release);
        if let Some(worker) = self.thread.take() {
            let _ = worker.join();
        }
    }
}
#[derive(Default)]
pub(super) struct Controller {
    jobs: HashMap<String, Job>,
}
impl Drop for Controller {
    fn drop(&mut self) {
        // Signal every waiter/owner before any Job joins its worker. A waiter
        // can depend on a mutex held by another job in this same controller.
        for job in self.jobs.values() {
            job.canceled.store(true, Ordering::Release);
        }
    }
}
impl Controller {
    pub(super) fn start(
        &mut self,
        id: String,
        scope: CameraScope,
        plan: CameraSubmissionPlan,
    ) -> Result<()> {
        self.start_task(id, scope, plan, None)
    }
    pub(super) fn start_named(
        &mut self,
        id: String,
        scope: CameraScope,
        task_name: String,
        plan: CameraSubmissionPlan,
    ) -> Result<()> {
        ensure!(
            !task_name.is_empty()
                && task_name.len() <= 96
                && task_name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
            "invalid camera task name"
        );
        self.start_task(id, scope, plan, Some(task_name))
    }
    fn start_task(
        &mut self,
        id: String,
        scope: CameraScope,
        plan: CameraSubmissionPlan,
        task_name: Option<String>,
    ) -> Result<()> {
        let factory_scope = scope.clone();
        let properties = plan.commands.iter().any(|c| {
            matches!(
                c,
                razer_device::camera_submission::CameraCommand::ShowProperties
            )
        });
        self.start_with_lock(id, scope, plan, task_name, acquire_task_lock, move || {
            crate::camera::ScopedCameraSession::open_mode(factory_scope, properties)
        })
    }
    #[cfg(test)]
    fn start_with<T: CameraCommandTransport + 'static>(
        &mut self,
        id: String,
        scope: CameraScope,
        plan: CameraSubmissionPlan,
        factory: impl FnOnce() -> Result<T> + Send + 'static,
    ) -> Result<()> {
        self.start_with_lock(id, scope, plan, None, |_, _, _| Ok(()), factory)
    }
    fn start_with_lock<T: CameraCommandTransport + 'static, L: 'static>(
        &mut self,
        id: String,
        scope: CameraScope,
        plan: CameraSubmissionPlan,
        task_name: Option<String>,
        acquire: impl FnOnce(&CameraScope, Option<&str>, &AtomicBool) -> Result<L> + Send + 'static,
        factory: impl FnOnce() -> Result<T> + Send + 'static,
    ) -> Result<()> {
        ensure!(
            !id.is_empty() && id.len() <= 128 && !id.contains('\0'),
            "invalid camera operation id"
        );
        scope.validate()?;
        plan.validate(scope.product_id)?;
        ensure!(
            !self.jobs.contains_key(&id),
            "camera operation id already exists"
        );
        ensure!(
            task_name.is_some()
                || !self.jobs.values().any(|j| j.task_name.is_none()
                    && j.scope
                        .container_id
                        .eq_ignore_ascii_case(&scope.container_id)),
            "camera already has a retained operation; poll/cancel it first"
        );
        ensure!(self.jobs.len() < 16, "camera operation limit reached");
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = canceled.clone();
        let product_id = scope.product_id;
        let lock_scope = scope.clone();
        let lock_name = task_name.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("camera-com-session".into())
            .spawn(move || {
                let validate = || {
                    ensure!(!signal.load(Ordering::Acquire), "camera operation canceled");
                    Ok(())
                };
                // The source Web Lock spans the entire runner, including all
                // native commands of an autoFocus/imagePreset compound task.
                let locked =
                    validate().and_then(|_| acquire(&lock_scope, lock_name.as_deref(), &signal));
                let progress =
                    match locked.and_then(|guard| factory().map(|transport| (guard, transport))) {
                        Ok((_guard, mut transport)) => {
                            execute_camera_submission(&mut transport, product_id, &plan, &validate)
                        }
                        Err(e) => CameraSubmissionProgress {
                            error: Some(format!("{e:#}")),
                            ..Default::default()
                        },
                    };
                // Transport, filter and COM apartment drop on this thread before delivery.
                let _ = sender.send(progress);
            })?;
        self.jobs.insert(
            id,
            Job {
                scope,
                task_name,
                canceled,
                receiver,
                thread: Some(worker),
            },
        );
        Ok(())
    }
    pub(super) fn poll(&mut self, id: &str) -> Result<CameraOperationStatus> {
        let job = self
            .jobs
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("unknown camera operation"))?;
        let result = match job.receiver.try_recv() {
            Ok(p) => Some(p),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => Some(CameraSubmissionProgress {
                error: Some("camera worker disconnected".into()),
                ..Default::default()
            }),
        };
        if let Some(p) = result {
            self.jobs.remove(id);
            Ok(CameraOperationStatus::Finished(p))
        } else {
            Ok(CameraOperationStatus::Running)
        }
    }
    pub(super) fn cancel(&mut self, id: &str) -> Result<()> {
        let job = self
            .jobs
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("unknown camera operation"))?;
        job.canceled.store(true, Ordering::Release);
        Ok(())
    }
}

#[cfg(test)]
#[path = "camera_controller_tests.rs"]
mod tests;

fn task_lock_name(scope: &CameraScope, task_name: &str) -> String {
    format!(
        "Local\\RazerUi-Camera-{}_{}",
        scope.container_id.to_ascii_lowercase(),
        task_name
    )
}

#[cfg(windows)]
fn acquire_task_lock(
    scope: &CameraScope,
    task_name: Option<&str>,
    canceled: &AtomicBool,
) -> Result<Option<crate::camera::lock::Guard>> {
    let Some(task_name) = task_name else {
        return Ok(None);
    };
    crate::camera::lock::acquire(&task_lock_name(scope, task_name), &|| {
        ensure!(
            !canceled.load(Ordering::Acquire),
            "camera task lock aborted"
        );
        Ok(())
    })
    .map(Some)
}
#[cfg(not(windows))]
fn acquire_task_lock(_: &CameraScope, task_name: Option<&str>, _: &AtomicBool) -> Result<()> {
    ensure!(
        task_name.is_none(),
        "camera task Web Lock adapter requires Windows"
    );
    Ok(())
}
