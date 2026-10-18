//! Retained render owners from current THX feature lifetime. Vendor code never
//! runs. Job completion, cancellation, owner release and source shutdown have
//! separate meanings; the endpoint stays locked until transport teardown.
use super::*;
use razer_device::thx_v4::{ThxRenderSetupTrigger, ThxV4Controller, capability};
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

struct Command {
    scope: ThxSubmissionScope,
    plan: ThxSubmissionPlan,
    cancellation: Cancellation,
    sender: mpsc::SyncSender<ThxSubmissionProgress>,
}
pub(super) struct Owner {
    scope: ThxSubmissionScope,
    lifetime: Cancellation,
    sender: Option<mpsc::SyncSender<Command>>,
    worker: Option<JoinHandle<()>>,
    cleanup_error: Arc<Mutex<Option<String>>>,
}
impl Owner {
    pub(super) fn conflicts_for(
        &self,
        scope: &ThxSubmissionScope,
        plan: &ThxSubmissionPlan,
    ) -> bool {
        if plan.is_direct_hid() {
            self.scope
                .container_id
                .eq_ignore_ascii_case(&scope.container_id)
        } else {
            self.conflicts(scope)
        }
    }
    pub(super) fn conflicts(&self, scope: &ThxSubmissionScope) -> bool {
        let a = capability(self.scope.product_id).unwrap();
        let b = capability(scope.product_id).unwrap();
        self.scope
            .container_id
            .eq_ignore_ascii_case(&scope.container_id)
            || (a.thx_product_id == b.thx_product_id && a.endpoint_id == b.endpoint_id)
    }
    fn matches(&self, scope: &ThxSubmissionScope) -> bool {
        self.scope.product_id == scope.product_id
            && self
                .scope
                .container_id
                .eq_ignore_ascii_case(&scope.container_id)
            && self.scope.serial_number == scope.serial_number
    }
    fn stop(&mut self) -> Result<()> {
        self.lifetime.cancel();
        self.sender.take();
        if let Some(worker) = self.worker.take() {
            ensure!(
                worker.join().is_ok(),
                "THX retained owner panicked; transport released, submitted settings outcome unknown"
            );
        }
        Ok(())
    }
}
impl Drop for Owner {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

impl Controller {
    pub(super) fn shutdown_owners(&mut self, cleanup: &mut crate::runtime::shutdown::Cleanup) {
        for owner in self.owners.values() {
            owner.lifetime.cancel();
        }
        for (id, mut owner) in self.owners.drain() {
            let name = format!("thx_owner:{id}");
            cleanup.record(&name, owner.stop());
            match owner.cleanup_error.lock() {
                Ok(error) => {
                    if let Some(error) = error.as_ref() {
                        cleanup.fail(&name, error);
                    }
                }
                Err(_) => cleanup.fail(&name, "THX cleanup receipt lock failed"),
            }
        }
    }

    fn owner_slot(
        &self,
        owner_id: &str,
        operation_id: &str,
        scope: &ThxSubmissionScope,
        plan: &ThxSubmissionPlan,
    ) -> Result<()> {
        for (name, value) in [("owner", owner_id), ("operation", operation_id)] {
            ensure!(
                !value.is_empty() && value.len() <= 128 && !value.contains('\0'),
                "Invalid THX {name} id"
            );
        }
        scope.validate()?;
        plan.validate()?;
        ensure!(
            !self.jobs.contains_key(operation_id),
            "THX operation id already exists"
        );
        let endpoint = capability(scope.product_id).unwrap();
        ensure!(
            !self.jobs.values().any(|job| {
                let other = capability(job.scope.product_id).unwrap();
                job.scope
                    .container_id
                    .eq_ignore_ascii_case(&scope.container_id)
                    || (!plan.is_direct_hid()
                        && !job.plan.is_direct_hid()
                        && other.thx_product_id == endpoint.thx_product_id
                        && other.endpoint_id == endpoint.endpoint_id)
            }),
            "THX endpoint already has a retained job; poll its completion before submitting again"
        );
        ensure!(
            !self
                .owners
                .iter()
                .any(|(id, owner)| id != owner_id && owner.conflicts_for(scope, plan)),
            "THX endpoint belongs to another render owner"
        );
        ensure!(self.jobs.len() < 16, "THX job limit reached");
        Ok(())
    }
    pub(crate) fn start_owner(
        &mut self,
        owner_id: String,
        operation_id: String,
        scope: ThxSubmissionScope,
        plan: ThxSubmissionPlan,
    ) -> Result<()> {
        let retained = scope.clone();
        self.start_owner_with(owner_id, operation_id, scope, plan, move |gate| {
            thx::open(&retained, gate)
        })
    }
    fn start_owner_with<T: ThxJobTransport + 'static>(
        &mut self,
        owner_id: String,
        operation_id: String,
        scope: ThxSubmissionScope,
        plan: ThxSubmissionPlan,
        factory: impl FnOnce(Cancellation) -> Result<T> + Send + 'static,
    ) -> Result<()> {
        self.owner_slot(&owner_id, &operation_id, &scope, &plan)?;
        ensure!(
            !self.owners.contains_key(&owner_id),
            "THX render owner already exists"
        );
        ensure!(self.owners.len() < 16, "THX owner limit reached");
        ensure!(
            plan.is_direct_hid()
                || plan
                    == ThxSubmissionPlan::SetupRender {
                        trigger: ThxRenderSetupTrigger::InitialDeviceSetup
                    },
            "A retained owner must start with explicit render setup or a direct roller command"
        );
        let lifetime = Cancellation::default();
        let owner_signal = lifetime.clone();
        let gate = Cancellation::owner_gate(&lifetime);
        let canceled = Cancellation::default();
        let (reply_sender, receiver) = mpsc::sync_channel(1);
        let first = Command {
            scope: scope.clone(),
            plan: plan.clone(),
            cancellation: canceled.clone(),
            sender: reply_sender,
        };
        let (sender, commands) = mpsc::sync_channel(1);
        let cleanup_error = Arc::new(Mutex::new(None));
        let cleanup = cleanup_error.clone();
        let worker = thread::Builder::new()
            .name("thx-render-owner".into())
            .spawn(move || {
                let opened = gate
                    .begin_operation(&first.cancellation)
                    .and_then(|_| gate.check())
                    .and_then(|_| factory(gate.clone()))
                    .and_then(|transport| ThxV4Controller::new(first.scope.product_id, transport));
                let mut controller = match opened {
                    Ok(controller) => controller,
                    Err(error) => {
                        let _ = gate.end_operation();
                        let _ = first.sender.send(thx::failed(
                            first.scope,
                            first.plan,
                            format!("{error:#}"),
                        ));
                        return;
                    }
                };
                let mut next = Some(first);
                loop {
                    let command = match next.take() {
                        Some(command) => command,
                        None => {
                            if owner_signal.check().is_err() {
                                break;
                            }
                            match commands.recv_timeout(Duration::from_millis(20)) {
                                Ok(command) => command,
                                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                                Err(mpsc::RecvTimeoutError::Disconnected) => break,
                            }
                        }
                    };
                    let closed = command.plan == ThxSubmissionPlan::ShutdownRender;
                    let mut progress = match gate.begin_operation(&command.cancellation) {
                        Ok(()) => thx::execute_retained(
                            command.scope.clone(),
                            command.plan.clone(),
                            &mut controller,
                            || gate.check(),
                            |ms| thx::wait(ms, &gate),
                        ),
                        Err(error) => {
                            thx::failed(command.scope, command.plan, format!("{error:#}"))
                        }
                    };
                    if let Err(error) = gate.end_operation() {
                        progress.cleanup_error = Some(format!("{error:#}"));
                    }
                    if closed {
                        // Source shutdown has its own RemoveDevice step. If it
                        // was interrupted, dropping Rust resources does not claim
                        // that the missing source step or setters succeeded.
                        drop(controller);
                        let _ = command.sender.send(progress);
                        return;
                    }
                    if command.sender.send(progress).is_err() {
                        break;
                    }
                }
                // Identity expiry/disconnect/client exit releases resources;
                // it never writes EQ/spatial false as an invented event.
                if controller.transport().has_thx_binding() {
                    if let Err(error) = controller.remove() {
                        if let Ok(mut slot) = cleanup.lock() {
                            *slot = Some(format!("{error:#}"));
                        }
                    }
                }
            })?;
        self.owners.insert(
            owner_id.clone(),
            Owner {
                scope: scope.clone(),
                lifetime,
                sender: Some(sender),
                worker: Some(worker),
                cleanup_error,
            },
        );
        self.jobs.insert(
            operation_id,
            Job {
                scope,
                plan,
                canceled,
                receiver,
                worker: None,
                owner_id: Some(owner_id),
            },
        );
        Ok(())
    }
    pub(crate) fn submit_owner(
        &mut self,
        owner_id: &str,
        operation_id: String,
        scope: ThxSubmissionScope,
        plan: ThxSubmissionPlan,
    ) -> Result<()> {
        self.owner_slot(owner_id, &operation_id, &scope, &plan)?;
        let owner = self
            .owners
            .get(owner_id)
            .ok_or_else(|| anyhow::anyhow!("Unknown THX retained owner"))?;
        ensure!(owner.matches(&scope), "THX physical owner identity changed");
        ensure!(
            owner
                .worker
                .as_ref()
                .is_some_and(|worker| !worker.is_finished()),
            "THX retained owner exited; transport outcome unavailable"
        );
        owner.lifetime.check()?;
        let canceled = Cancellation::default();
        let (sender, receiver) = mpsc::sync_channel(1);
        owner
            .sender
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("THX render owner is closing"))?
            .try_send(Command {
                scope: scope.clone(),
                plan: plan.clone(),
                cancellation: canceled.clone(),
                sender,
            })
            .map_err(|_| anyhow::anyhow!("THX render owner cannot accept another command"))?;
        self.jobs.insert(
            operation_id,
            Job {
                scope,
                plan,
                canceled,
                receiver,
                worker: None,
                owner_id: Some(owner_id.into()),
            },
        );
        Ok(())
    }
    pub(crate) fn release_owner(&mut self, owner_id: &str) -> Result<serde_json::Value> {
        let mut owner = self
            .owners
            .remove(owner_id)
            .ok_or_else(|| anyhow::anyhow!("Unknown THX retained owner"))?;
        for job in self
            .jobs
            .values()
            .filter(|job| job.owner_id.as_deref() == Some(owner_id))
        {
            job.canceled.cancel();
        }
        let stopped = owner.stop().err().map(|error| format!("{error:#}"));
        let cleanup = owner
            .cleanup_error
            .lock()
            .map_err(|_| anyhow::anyhow!("THX cleanup receipt lock failed"))?
            .clone();
        Ok(
            serde_json::json!({"owner_id":owner_id,"released":true,"cleanup_error":stopped.or(cleanup)}),
        )
    }
}

#[cfg(test)]
#[path = "thx_owner_tests.rs"]
mod tests;
