//! Profile tasks borrow a physical dock's retained lighting connection. Ending
//! a page request cannot stop its software frames or skip restoration on exit.
use super::*;
use anyhow::ensure;
use razer_device::{
    lighting_engine::SoftwarePhase,
    receiver_lighting::{
        ApplicationReceipt, ChromaAttributeExchange, ChromaAttributeReceipt,
        ChromaAttributeRequest, Configuration, Status,
    },
};
use razer_discovery::receiver::ReceiverRoute;
use razer_ipc::{ServiceClient, ServiceRequest};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

struct Connection {
    client: ServiceClient,
    id: String,
    path: String,
    started: bool,
    chroma_exchange: Option<ChromaAttributeExchange>,
}

fn cached_chroma_receipt(
    previous: Option<&ChromaAttributeExchange>,
    request: &ChromaAttributeRequest,
) -> anyhow::Result<Option<ChromaAttributeReceipt>> {
    match previous {
        Some(exchange)
            if exchange.request.worker_id == request.worker_id
                && exchange.request.id == request.id =>
        {
            ensure!(
                exchange.request == *request,
                "Dock reused a local-service request identity with different parameters"
            );
            Ok(Some(exchange.receipt.clone()))
        }
        _ => Ok(None),
    }
}

struct ConnectionSlot<C> {
    connection: Option<C>,
    native_receipt: Option<Result<(), String>>,
}
impl<C> ConnectionSlot<C> {
    fn release(
        &mut self,
        release_native: impl FnOnce(&mut C) -> anyhow::Result<()>,
        finish_termination: impl FnOnce(&mut C) -> anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        if let Some(connection) = self.connection.as_mut() {
            if self.native_receipt.is_none() {
                self.native_receipt =
                    Some(release_native(connection).map_err(|error| format!("{error:#}")));
            }
            if let Err(error) = finish_termination(connection) {
                let native = self
                    .native_receipt
                    .as_ref()
                    .and_then(|result| result.as_ref().err());
                anyhow::bail!(
                    "Dock lighting process termination is unconfirmed; retained for cleanup: {error:#}{}",
                    native.map_or(String::new(), |error| format!("; native cleanup: {error}"))
                );
            }
            self.connection.take();
        }
        match &self.native_receipt {
            Some(Err(error)) => anyhow::bail!("Dock native cleanup remains unconfirmed: {error}"),
            _ => Ok(()),
        }
    }
}
pub(in crate::shell) struct RetainedOwner {
    route: ReceiverRoute,
    connection_revision: u64,
    container: String,
    accepting: AtomicBool,
    connection: Mutex<ConnectionSlot<Connection>>,
    predecessor: Mutex<Option<Arc<RetainedOwner>>>,
    chroma_connect: Arc<Mutex<razer_service::chroma_connect::Session>>,
}
pub(super) struct Session {
    owner: Arc<RetainedOwner>,
    _poll: Task<()>,
}
pub(super) struct Cleanup {
    owner: Arc<RetainedOwner>,
    task: Task<()>,
}
impl RetainedOwner {
    pub(in crate::shell) fn apply(
        &self,
        path: &str,
        configuration: Configuration,
        canceled: &AtomicBool,
    ) -> anyhow::Result<Status> {
        ensure!(
            self.accepting.load(Ordering::Acquire) && !canceled.load(Ordering::Acquire),
            "Dock lighting profile owner was replaced"
        );
        let mut slot = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("Dock lighting connection lock failed"))?;
        ensure!(
            self.accepting.load(Ordering::Acquire) && !canceled.load(Ordering::Acquire),
            "Dock lighting profile owner was replaced while waiting"
        );
        if slot.connection.is_none() {
            self.release_predecessor()?;
            ensure!(
                self.accepting.load(Ordering::Acquire) && !canceled.load(Ordering::Acquire),
                "Dock lighting profile owner was replaced during predecessor cleanup"
            );
            slot.connection = Some(Connection {
                client: ServiceClient::spawn()?,
                id: uuid::Uuid::new_v4().to_string(),
                path: path.into(),
                started: false,
                chroma_exchange: None,
            });
        }
        let connection = slot.connection.as_mut().unwrap();
        ensure!(
            connection.path == path,
            "Dock command collection changed before engine release"
        );
        // This child owns ongoing frames and cleanup; the short profile guard
        // must not terminate it when the local Save request finishes.
        connection.started = true;
        let deadline = Instant::now() + Duration::from_secs(95);
        let mut status: Status = serde_json::from_value(connection.client.request(
            ServiceRequest::ReceiverLightingApply {
                operation_id: connection.id.clone(),
                path: path.into(),
                device_container_id: self.container.clone(),
                configuration: configuration.clone(),
            },
        )?)?;
        loop {
            status = self.bridge_status(connection, status, &|| {
                ensure!(
                    self.accepting.load(Ordering::Acquire) && !canceled.load(Ordering::Acquire),
                    "Dock lighting profile owner was replaced during local-service delivery"
                );
                Ok(())
            })?;
            if status.ensure_applied(&configuration).is_ok() {
                break;
            }
            if !matches!(
                status.application,
                ApplicationReceipt::AwaitingFrame { .. }
                    | ApplicationReceipt::AwaitingLocalService { .. }
            ) || status.software.phase == SoftwarePhase::Failed
            {
                status.ensure_applied(&configuration)?;
            }
            ensure!(
                Instant::now() < deadline,
                "Dock lighting application timed out"
            );
            ensure!(
                self.accepting.load(Ordering::Acquire) && !canceled.load(Ordering::Acquire),
                "Dock lighting profile owner was replaced while awaiting a frame"
            );
            std::thread::sleep(Duration::from_millis(16));
            status = serde_json::from_value(connection.client.request(
                ServiceRequest::ReceiverLightingPoll {
                    operation_id: connection.id.clone(),
                },
            )?)?;
        }
        ensure!(
            self.accepting.load(Ordering::Acquire) && !canceled.load(Ordering::Acquire),
            "Dock lighting response belongs to a retired profile"
        );
        Ok(status)
    }
    fn validate_status(&self, connection: &Connection, status: &Status) -> anyhow::Result<()> {
        ensure!(
            status.operation_id == connection.id
                && status.path == connection.path
                && status.device_container_id == self.container
                && u32::from(status.product_id) == self.route.product_id(),
            "Dock lighting response identity differs"
        );
        Ok(())
    }
    fn poll(&self) -> anyhow::Result<Option<Status>> {
        let mut slot = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("Dock lighting connection lock failed"))?;
        let Some(connection) = slot
            .connection
            .as_mut()
            .filter(|connection| connection.started)
        else {
            return Ok(None);
        };
        if !self.accepting.load(Ordering::Acquire) {
            return Ok(None);
        }
        let status: Status = serde_json::from_value(connection.client.request(
            ServiceRequest::ReceiverLightingPoll {
                operation_id: connection.id.clone(),
            },
        )?)?;
        let status = self.bridge_status(connection, status, &|| {
            ensure!(
                self.accepting.load(Ordering::Acquire),
                "Dock lighting owner was retired"
            );
            Ok(())
        })?;
        Ok(Some(status))
    }
    fn bridge_status(
        &self,
        connection: &mut Connection,
        mut status: Status,
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Status> {
        self.validate_status(connection, &status)?;
        validate()?;
        let mut shared = loop {
            validate()?;
            match self.chroma_connect.try_lock() {
                Ok(session) => break session,
                Err(std::sync::TryLockError::WouldBlock) => {
                    std::thread::sleep(Duration::from_millis(16));
                }
                Err(std::sync::TryLockError::Poisoned(_)) => {
                    anyhow::bail!("Application Chroma Connect owner lock failed")
                }
            }
        };
        if let Err(error) = shared.maintain(validate) {
            eprintln!("[chroma-connect] initialization was not confirmed: {error:#}");
        }
        if let Some(error) = shared.take_status_error() {
            eprintln!("[chroma-connect] application status was not confirmed: {error}");
        }
        // A worker has at most one suspended source call. Cache its real
        // receipt before IPC delivery so a failed ACK never repeats the write.
        if let Some(request) = &status.monitoring.pending_chroma_attribute {
            ensure!(
                *request
                    == ChromaAttributeRequest::charging(
                        request.worker_id,
                        request.id,
                        request.pid
                    )?,
                "Dock local-service request differs from the captured charging operation"
            );
            let receipt = match cached_chroma_receipt(connection.chroma_exchange.as_ref(), request)?
            {
                Some(receipt) => receipt,
                None => {
                    let receipt = shared.set_attribute(
                        request.pid,
                        request.attribute,
                        request.value,
                        validate,
                    );
                    connection.chroma_exchange = Some(ChromaAttributeExchange::new(
                        request.clone(),
                        receipt.clone(),
                    ));
                    receipt
                }
            };
            drop(shared);
            validate()?;
            let request_id = request.id;
            let worker_id = request.worker_id;
            status = serde_json::from_value(connection.client.request(
                ServiceRequest::ReceiverLightingChromaAttributeResult {
                    operation_id: connection.id.clone(),
                    worker_id,
                    request_id,
                    receipt,
                },
            )?)?;
            self.validate_status(connection, &status)?;
            ensure!(
                status
                    .monitoring
                    .pending_chroma_attribute
                    .as_ref()
                    .is_none_or(
                        |request| request.worker_id != worker_id || request.id != request_id
                    ),
                "Dock local-service receipt was not consumed"
            );
        }
        validate()?;
        Ok(status)
    }
    fn release(&self) -> anyhow::Result<()> {
        self.accepting.store(false, Ordering::Release);
        let mut slot = self
            .connection
            .lock()
            .map_err(|_| anyhow::anyhow!("Dock lighting connection lock failed"))?;
        // An owner replaced before its first Apply still carries the older
        // physical connection. Never lose that predecessor's release barrier.
        let predecessor = self.release_predecessor();
        let current = slot.release(
            |connection| self.release_native(connection),
            |connection| connection.client.finish_termination(),
        );
        let failures = Some(predecessor)
            .into_iter()
            .chain(Some(current))
            .filter_map(Result::err)
            .map(|error| format!("{error:#}"))
            .collect::<Vec<_>>();
        ensure!(failures.is_empty(), "{}", failures.join("; "));
        Ok(())
    }
    fn release_predecessor(&self) -> anyhow::Result<()> {
        let mut predecessor = self
            .predecessor
            .lock()
            .map_err(|_| anyhow::anyhow!("Dock predecessor lock failed"))?;
        if let Some(previous) = predecessor.as_ref() {
            previous.release()?;
            predecessor.take();
        }
        Ok(())
    }
    fn release_native(&self, connection: &mut Connection) -> anyhow::Result<()> {
        let mut failures = Vec::new();
        if connection.started && connection.client.is_stopped() {
            failures.push("Dock lighting child stopped before native release was confirmed".into());
        }
        if connection.started && !connection.client.is_stopped() {
            let stopped = connection
                .client
                .request(ServiceRequest::ReceiverLightingStop {
                    operation_id: connection.id.clone(),
                })
                .and_then(|value| serde_json::from_value::<Status>(value).map_err(Into::into));
            match stopped {
                Ok(status) => {
                    if let Err(error) = self.validate_status(connection, &status) {
                        failures.push(format!("{error:#}"));
                    }
                    if !status.software.cleanup_errors.is_empty() || !status.restoration_complete()
                    {
                        failures.push(format!(
                            "Dock lighting release was not confirmed: {:?}",
                            status.software
                        ));
                    }
                }
                Err(error) => failures.push(format!("Dock engine stop failed: {error:#}")),
            }
        }
        if let Err(error) = super::receiver_reset::local::shutdown(&mut connection.client) {
            failures.push(format!("Dock lighting child cleanup: {error:#}"));
        }
        ensure!(failures.is_empty(), "{}", failures.join("; "));
        Ok(())
    }
}

impl AppShell {
    pub(in crate::shell) fn receiver_lighting_owner(
        &mut self,
        identity: &str,
        route: ReceiverRoute,
        connection_revision: u64,
        container: String,
        cx: &mut Context<Self>,
    ) -> Arc<RetainedOwner> {
        if let Some(session) = self.receiver_lighting_owners.get(identity) {
            if session.owner.connection_revision == connection_revision
                && session.owner.route == route
            {
                return session.owner.clone();
            }
        }
        let predecessor = self
            .receiver_lighting_owners
            .remove(identity)
            .map(|session| session.owner)
            .or_else(|| {
                self.receiver_lighting_cleanup
                    .iter()
                    .rev()
                    .find(|cleanup| cleanup.owner.container == container)
                    .map(|cleanup| cleanup.owner.clone())
            });
        if let Some(previous) = &predecessor {
            previous.accepting.store(false, Ordering::Release);
            self.release_receiver_lighting_owner(previous.clone(), cx);
        }
        let owner = Arc::new(RetainedOwner {
            route,
            connection_revision,
            container,
            accepting: AtomicBool::new(true),
            connection: Mutex::new(ConnectionSlot {
                connection: None,
                native_receipt: None,
            }),
            predecessor: Mutex::new(predecessor),
            chroma_connect: self.receiver_chroma_connect.clone(),
        });
        let retained = owner.clone();
        let key = identity.to_owned();
        let poll = cx.spawn(async move |shell, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                if !retained.accepting.load(Ordering::Acquire) {
                    break;
                }
                let selected = retained.clone();
                let result = cx.background_spawn(async move { selected.poll() }).await;
                let current = shell.update(cx, |shell, cx| {
                    if !shell
                        .receiver_lighting_owners
                        .get(&key)
                        .is_some_and(|session| Arc::ptr_eq(&session.owner, &retained))
                    {
                        return false;
                    }
                    let error = match result {
                        Ok(Some(status))
                            if status.software.phase == SoftwarePhase::Failed
                                || !status.software.errors.is_empty()
                                || !status.software.cleanup_errors.is_empty() =>
                        {
                            Some(format!(
                                "接收器灯光引擎错误：{}",
                                status
                                    .software
                                    .errors
                                    .iter()
                                    .chain(&status.software.cleanup_errors)
                                    .cloned()
                                    .collect::<Vec<_>>()
                                    .join("；")
                            ))
                        }
                        Err(error) => Some(format!("接收器灯光状态未读取：{error:#}")),
                        _ => None,
                    };
                    if let Some(error) = error {
                        shell.status = error;
                        cx.notify();
                    }
                    true
                });
                if !matches!(current, Ok(true)) {
                    break;
                }
            }
        });
        self.receiver_lighting_owners.insert(
            identity.into(),
            Session {
                owner: owner.clone(),
                _poll: poll,
            },
        );
        owner
    }
    fn release_receiver_lighting_owner(
        &mut self,
        owner: Arc<RetainedOwner>,
        cx: &mut Context<Self>,
    ) {
        owner.accepting.store(false, Ordering::Release);
        if self
            .receiver_lighting_cleanup
            .iter()
            .any(|cleanup| Arc::ptr_eq(&cleanup.owner, &owner))
        {
            return;
        }
        let retained = owner.clone();
        let task = cx.spawn(async move |shell, cx| {
            let selected = retained.clone();
            let result = cx.background_spawn(async move { selected.release() }).await;
            let _ = shell.update(cx, |shell, cx| {
                if result.is_ok() {
                    shell
                        .receiver_lighting_cleanup
                        .retain(|cleanup| !Arc::ptr_eq(&cleanup.owner, &retained));
                }
                if let Err(error) = result {
                    shell.status = format!("接收器灯效恢复或释放未确认：{error:#}");
                    cx.notify();
                }
            });
        });
        self.receiver_lighting_cleanup.push(Cleanup { owner, task });
    }
    pub(in crate::shell) fn sync_receiver_lighting_owners(
        &mut self,
        complete: bool,
        cx: &mut Context<Self>,
    ) {
        if !complete {
            return;
        }
        let stale = self
            .receiver_lighting_owners
            .iter()
            .filter_map(|(key, session)| {
                let workspace = self
                    .devices
                    .iter()
                    .find(|workspace| workspace.read(cx).identity(cx) == *key);
                let current = workspace.is_some_and(|workspace| {
                    workspace.read(cx).connection_revision() == session.owner.connection_revision
                        && self.device_observations.iter().any(|observed| {
                            observed.matches(workspace.read(cx).device(cx))
                                && session.owner.route.matches(observed)
                        })
                });
                (!current).then(|| key.clone())
            })
            .collect::<Vec<_>>();
        for key in stale {
            if let Some(session) = self.receiver_lighting_owners.remove(&key) {
                self.release_receiver_lighting_owner(session.owner, cx);
            }
        }
    }
    pub(in crate::shell) fn install_receiver_lighting_cleanup(&mut self) {
        self.register_exit_cleanup(|shell, cx| {
            let chroma_connect = shell.receiver_chroma_connect.clone();
            let cleanups = std::mem::take(&mut shell.receiver_lighting_cleanup);
            let owners = std::mem::take(&mut shell.receiver_lighting_owners)
                .into_values()
                .map(|session| session.owner)
                .collect::<Vec<_>>();
            for owner in owners
                .iter()
                .chain(cleanups.iter().map(|cleanup| &cleanup.owner))
            {
                owner.accepting.store(false, Ordering::Release);
            }
            let cleanup = cx.spawn(async move |_, cx| {
                for cleanup in cleanups {
                    cleanup.task.await;
                    let result = cx
                        .background_spawn(async move { cleanup.owner.release() })
                        .await;
                    if let Err(error) = result {
                        eprintln!("[receiver-lighting] {error:#}");
                    }
                }
                cx.background_spawn(async move {
                    for owner in owners {
                        if let Err(error) = owner.release() {
                            eprintln!("[receiver-lighting] {error:#}");
                        }
                    }
                    match chroma_connect.lock() {
                        Ok(mut session) => {
                            if let Err(error) = session.finish() {
                                eprintln!("[chroma-connect] application cleanup was not confirmed: {error:#}");
                            }
                        }
                        Err(_) => eprintln!("[chroma-connect] application cleanup owner lock failed"),
                    }
                })
                .await;
            });
            async move { cleanup.await }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[::core::prelude::v1::test]
    fn receiver_lighting_chroma_ack_reuses_actual_receipt_only_for_the_original_request() {
        let worker = uuid::Uuid::from_u128(1);
        let request = ChromaAttributeRequest::charging(worker, 1, 221).unwrap();
        let receipt = ChromaAttributeReceipt::Failed {
            message: "response timed out after sending".into(),
        };
        let exchange = ChromaAttributeExchange::new(request.clone(), receipt.clone());
        assert_eq!(
            cached_chroma_receipt(Some(&exchange), &request).unwrap(),
            Some(receipt)
        );
        let next = ChromaAttributeRequest::charging(worker, 2, 221).unwrap();
        assert_eq!(cached_chroma_receipt(Some(&exchange), &next).unwrap(), None);
        let changed = ChromaAttributeRequest::charging(worker, 1, 235).unwrap();
        assert!(cached_chroma_receipt(Some(&exchange), &changed).is_err());
    }

    #[::core::prelude::v1::test]
    fn receiver_lighting_chroma_worker_restart_cannot_reuse_a_predecessor_receipt() {
        let first = ChromaAttributeRequest::charging(uuid::Uuid::from_u128(1), 1, 221).unwrap();
        let exchange = ChromaAttributeExchange::new(
            first,
            ChromaAttributeReceipt::Replied {
                response_json:
                    r#"{"token":{"id":1,"param":{"pid":221,"attribute":2,"value":0}},"result":0}"#
                        .into(),
            },
        );
        for pid in [221, 235] {
            let successor =
                ChromaAttributeRequest::charging(uuid::Uuid::from_u128(2), 1, pid).unwrap();
            assert_eq!(
                cached_chroma_receipt(Some(&exchange), &successor).unwrap(),
                None
            );
        }
    }

    #[::core::prelude::v1::test]
    fn receiver_lighting_retains_transport_until_termination_and_keeps_native_failure() {
        let mut slot = ConnectionSlot {
            connection: Some(()),
            native_receipt: None,
        };
        let error = slot
            .release(
                |_| anyhow::bail!("Shutdown acknowledgement failed"),
                |_| anyhow::bail!("process wait failed"),
            )
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Shutdown acknowledgement failed")
        );
        assert!(slot.connection.is_some());
        assert!(
            slot.release(
                |_| panic!("native receipt must not be overwritten by a second shutdown"),
                |_| Ok(()),
            )
            .unwrap_err()
            .to_string()
            .contains("Shutdown acknowledgement failed")
        );
        assert!(slot.connection.is_none());
        assert!(
            slot.release(
                |_| panic!("released connection must not reopen"),
                |_| panic!("released connection must not terminate again"),
            )
            .is_err()
        );
    }

    #[::core::prelude::v1::test]
    fn receiver_lighting_retries_only_unconfirmed_transport_after_native_release() {
        let mut slot = ConnectionSlot {
            connection: Some(()),
            native_receipt: None,
        };
        assert!(
            slot.release(|_| Ok(()), |_| anyhow::bail!("wait failed"))
                .is_err()
        );
        assert!(slot.connection.is_some());
        slot.release(
            |_| panic!("native cleanup already acknowledged"),
            |_| Ok(()),
        )
        .unwrap();
        assert!(slot.connection.is_none());
        slot.release(|_| panic!("no connection"), |_| panic!("no connection"))
            .unwrap();
    }
}
