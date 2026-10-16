//! Own local THX jobs until complete teardown, including cancel and worker exit.
use crate::{
    thx::{self, ThxJobTransport},
    thx_rpc::Cancellation,
};
use anyhow::{Result, ensure};
use razer_device::thx_submission::{
    ThxOperationStatus, ThxSubmissionPlan, ThxSubmissionProgress, ThxSubmissionScope,
};
use std::{
    collections::HashMap,
    sync::mpsc,
    thread::{self, JoinHandle},
};
struct Job {
    scope: ThxSubmissionScope,
    plan: ThxSubmissionPlan,
    canceled: Cancellation,
    receiver: mpsc::Receiver<ThxSubmissionProgress>,
    worker: Option<JoinHandle<()>>,
    owner_id: Option<String>,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.canceled.cancel();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
#[derive(Default)]
pub(super) struct Controller {
    jobs: HashMap<String, Job>,
    owners: HashMap<String, owner::Owner>,
}
#[path = "thx_owner.rs"]
mod owner;
impl Controller {
    pub(super) fn start(
        &mut self,
        id: String,
        scope: ThxSubmissionScope,
        plan: ThxSubmissionPlan,
    ) -> Result<()> {
        let retained = scope.clone();
        self.start_with(id, scope, plan, move |cancellation| {
            thx::open(&retained, cancellation)
        })
    }
    fn start_with<T: ThxJobTransport + 'static>(
        &mut self,
        id: String,
        scope: ThxSubmissionScope,
        plan: ThxSubmissionPlan,
        factory: impl FnOnce(Cancellation) -> Result<T> + Send + 'static,
    ) -> Result<()> {
        ensure!(
            !id.is_empty() && id.len() <= 128 && !id.contains('\0'),
            "Invalid THX operation id"
        );
        scope.validate()?;
        plan.validate()?;
        ensure!(
            !self.jobs.contains_key(&id),
            "THX operation id already exists"
        );
        ensure!(
            !self.jobs.values().any(|job| job
                .scope
                .container_id
                .eq_ignore_ascii_case(&scope.container_id)),
            "THX container already has a retained job"
        );
        // The native service addresses product/endpoint, without ContainerId.
        // This process must not overlap sessions sharing that endpoint.
        let endpoint = razer_device::thx_v4::capability(scope.product_id).unwrap();
        ensure!(
            !self
                .owners
                .values()
                .any(|owner| owner.conflicts_for(&scope, &plan)),
            "THX endpoint belongs to a retained render owner"
        );
        ensure!(
            !self.jobs.values().any(|job| {
                let other = razer_device::thx_v4::capability(job.scope.product_id).unwrap();
                !plan.is_direct_hid()
                    && !job.plan.is_direct_hid()
                    && other.thx_product_id == endpoint.thx_product_id
                    && other.endpoint_id == endpoint.endpoint_id
            }),
            "THX product/endpoint already has a retained job"
        );
        ensure!(self.jobs.len() < 16, "THX job limit reached");
        let canceled = Cancellation::default();
        let signal = canceled.clone();
        let retained = scope.clone();
        let retained_plan = plan.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("thx-local-session".into())
            .spawn(move || {
                let result = signal.check().and_then(|_| factory(signal.clone()));
                let progress = match result {
                    Ok(transport) => thx::execute(
                        retained,
                        plan,
                        transport,
                        || signal.check(),
                        |ms| thx::wait(ms, &signal),
                    ),
                    Err(error) => thx::failed(retained, plan, format!("{error:#}")),
                };
                // execute has removed binding and dropped HID/RPC before delivery.
                let _ = sender.send(progress);
            })?;
        self.jobs.insert(
            id,
            Job {
                scope,
                plan: retained_plan,
                canceled,
                receiver,
                worker: Some(worker),
                owner_id: None,
            },
        );
        Ok(())
    }
    pub(super) fn poll(&mut self, id: &str) -> Result<ThxOperationStatus> {
        let job = self
            .jobs
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("Unknown THX operation"))?;
        let result = match job.receiver.try_recv() {
            Ok(progress) => Some(progress),
            Err(mpsc::TryRecvError::Empty) => None,
            Err(mpsc::TryRecvError::Disconnected) => {
                // Return a terminal failure and release ownership. The worker
                // may have sent settings before unwinding; no ACK is inferred.
                Some(thx::failed(
                    job.scope.clone(),
                    job.plan.clone(),
                    "THX worker disconnected without a receipt; submitted settings outcome is unknown".into(),
                ))
            }
        };
        if let Some(progress) = result {
            let job = self.jobs.remove(id).unwrap();
            if job.plan == ThxSubmissionPlan::ShutdownRender {
                if let Some(owner_id) = &job.owner_id {
                    // The worker dropped the transport before delivering the
                    // shutdown receipt; join before freeing its endpoint.
                    self.owners.remove(owner_id);
                }
            }
            Ok(ThxOperationStatus::Finished(progress))
        } else {
            Ok(ThxOperationStatus::Running)
        }
    }
    pub(super) fn cancel(&mut self, id: &str) -> Result<()> {
        let job = self
            .jobs
            .get(id)
            .ok_or_else(|| anyhow::anyhow!("Unknown THX operation"))?;
        job.canceled.cancel();
        Ok(())
    }
}

#[cfg(test)]
#[path = "thx_controller_tests.rs"]
mod tests;
