//! The mounted utility owns one source request until completion or dismissal.
//! Its future borrows the common transport's retained, joined physical worker.
use super::*;
use razer_pages::features::{
    ReceiverOperation, ReceiverPairingEvent, ReceiverPairingIntent, ReceiverPairingObservation,
    ReceiverPeer,
};
use serde_json::{Value, json};

pub(in crate::shell) struct Session {
    pub(super) session: u64,
    busy: Arc<AtomicU64>,
    valid: Option<Arc<AtomicBool>>,
    _delivery: Option<Task<()>>,
}
impl Drop for Session {
    fn drop(&mut self) {
        if let Some(valid) = &self.valid {
            valid.store(false, Ordering::Release);
        }
        // A replaced future cannot release the successor's physical owner.
        let _ = self
            .busy
            .compare_exchange(self.session, 0, Ordering::AcqRel, Ordering::Acquire);
    }
}

fn request(
    intent: &ReceiverPairingIntent,
) -> anyhow::Result<(&'static str, Value, Option<ReceiverOperation>)> {
    Ok(match intent {
        ReceiverPairingIntent::QueryBindings => (
            "DUALLINK_BIND_INFO",
            json!({}),
            Some(ReceiverOperation::Bindings),
        ),
        ReceiverPairingIntent::Scan(category) => (
            "DUALLINK_SCAN_DEVICE",
            json!({"status":1,"category":category}),
            Some(ReceiverOperation::Scan),
        ),
        ReceiverPairingIntent::Bind(peer) => {
            let device = serde_json::to_value(peer)?;
            ensure!(
                device["dongleId"]
                    .as_u64()
                    .is_some_and(|id| id > 0 && id < u64::from(u16::MAX)),
                "配对候选缺少真实 radio 身份"
            );
            (
                "DUALLINK_BIND_DEVICE",
                json!({"mode":1,"device":device}),
                Some(ReceiverOperation::Bind),
            )
        }
        ReceiverPairingIntent::Unbind(product_id) => {
            ensure!(
                *product_id > 0 && *product_id < u32::from(u16::MAX),
                "解绑 radio 身份无效"
            );
            (
                "DUALLINK_UNBIND_DEVICE",
                json!({"productId":product_id}),
                Some(ReceiverOperation::Unbind),
            )
        }
        ReceiverPairingIntent::Cancel => ("DUALLINK_CANCEL", json!({}), None),
    })
}

fn observation(
    intent: &ReceiverPairingIntent,
    session: u64,
    value: Value,
) -> anyhow::Result<ReceiverPairingObservation> {
    Ok(match intent {
        ReceiverPairingIntent::QueryBindings => {
            ReceiverPairingObservation::bindings(session, serde_json::from_value(value)?)
        }
        ReceiverPairingIntent::Scan(_) => {
            ReceiverPairingObservation::scanned(session, serde_json::from_value(value)?)
        }
        ReceiverPairingIntent::Bind(candidate) => {
            ensure!(value["hardware_confirmed"] == true, "配对缺少真实完成事件");
            let device = value.get("device").context("配对完成缺少设备描述")?;
            let expected = serde_json::to_value(candidate)?;
            ensure!(
                device["dongleId"] == expected["dongleId"],
                "配对完成属于另一 radio"
            );
            ReceiverPairingObservation::bound(session, serde_json::from_value(device.clone())?)
        }
        ReceiverPairingIntent::Unbind(radio) => {
            ensure!(
                value["productId"].as_u64() == Some(u64::from(*radio)),
                "解绑完成属于另一 radio"
            );
            ReceiverPairingObservation::unbound(session)
        }
        ReceiverPairingIntent::Cancel => anyhow::bail!("取消没有源 UI 完成响应"),
    })
}

fn queue_collection_bindings(
    route: &ReceiverRoute,
    intent: &ReceiverPairingIntent,
    session: u64,
    valid: Arc<AtomicBool>,
    sender: &Sender<Command>,
) -> Option<Result<(), mpsc::SendError<Command>>> {
    if matches!(route, ReceiverRoute::Collection(_))
        && matches!(intent, ReceiverPairingIntent::QueryBindings)
    {
        Some(sender.send(Command::Bindings { session, valid }))
    } else {
        None
    }
}

impl AppShell {
    pub(in crate::shell) fn request_hyperpolling_pairing(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        event: &ReceiverPairingEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let identity = workspace.read(cx).identity(cx);
        let Some(owner) = self
            .hyperpolling_receivers
            .get(&identity)
            .filter(|owner| !owner.stopped)
        else {
            if let Ok((_, _, Some(operation))) = request(event.intent()) {
                workspace.update(cx, |workspace, cx| {
                    workspace.observe_receiver_pairing(
                        ReceiverPairingObservation::failed(event.session(), operation),
                        cx,
                    )
                });
            }
            self.status = "接收器当前连接尚未初始化，操作未提交".into();
            cx.notify();
            return;
        };
        if self.close_requested {
            return;
        }
        let epoch = owner.epoch;
        let connection_revision = owner.connection_revision;
        let route = owner.route.clone();
        let busy = owner.pairing_active.clone();
        let cancel_barrier = owner.pairing_cancels.clone();
        let sender = owner.commands.clone();
        let (operation, payload, failure) = match request(event.intent()) {
            Ok(request) => request,
            Err(error) => {
                self.status = format!("配对参数无效：{error:#}");
                cx.notify();
                return;
            }
        };
        let session = event.session();
        busy.store(session, Ordering::Release);
        self.hyperpolling_pairing.remove(&identity);
        let valid = Arc::new(AtomicBool::new(true));
        let worker_valid = valid.clone();
        let container = workspace.read(cx).device(cx).device_container_id.clone();
        let master = json!({"productId":179,"deviceContainerId":container});
        let transport = self.pairing_transport.clone();
        if matches!(event.intent(), ReceiverPairingIntent::Cancel) {
            cancel_barrier.fetch_add(1, Ordering::AcqRel);
            self.hyperpolling_pairing.insert(
                identity.clone(),
                Session {
                    session,
                    busy,
                    valid: None,
                    _delivery: None,
                },
            );
            if sender
                .send(Command::Cancel {
                    session,
                    cleanup: transport,
                    master,
                })
                .is_err()
            {
                cancel_barrier.fetch_sub(1, Ordering::AcqRel);
                self.deliver_hyperpolling_cancel(
                    &identity,
                    session,
                    Err("Uma communication owner stopped before scan4".into()),
                    vec![],
                    cx,
                );
            }
            return;
        }
        if let Some(result) = queue_collection_bindings(
            &route,
            event.intent(),
            session,
            worker_valid.clone(),
            &sender,
        ) {
            self.hyperpolling_pairing.insert(
                identity.clone(),
                Session {
                    session,
                    busy,
                    valid: Some(valid),
                    _delivery: None,
                },
            );
            if result.is_err() {
                self.deliver_hyperpolling_pairing_failure(
                    &identity,
                    session,
                    failure,
                    "Uma collection owner stopped".into(),
                    cx,
                );
            }
            return;
        }
        // Invoke this before replacing the request task so cancellation reaches
        // every still-retained physical predecessor, including retired workers.
        let cleanup = transport.cancel_product_and_wait(master.clone());
        let intent = event.intent().clone();
        let weak = workspace.downgrade();
        let owner_key = identity.clone();
        let task = cx.spawn_in(window, async move |shell, cx| {
            while cancel_barrier.load(Ordering::Acquire) > 0 {
                if !worker_valid.load(Ordering::Acquire) {
                    return;
                }
                cx.background_executor()
                    .timer(Duration::from_millis(25))
                    .await;
            }
            let previous = match cleanup.await {
                Ok(previous) => previous,
                Err(error) => {
                    let _ = shell.update_in(cx, |shell, _window, cx| {
                        shell.deliver_hyperpolling_pairing_failure(
                            &owner_key,
                            session,
                            failure,
                            format!("Pairing predecessor cleanup is unconfirmed: {error}"),
                            cx,
                        );
                    });
                    return;
                }
            };
            if !worker_valid.load(Ordering::Acquire) {
                return;
            }
            if matches!(intent, ReceiverPairingIntent::QueryBindings) {
                if sender
                    .send(Command::Bindings {
                        session,
                        valid: worker_valid,
                    })
                    .is_err()
                {
                    let _ = shell.update_in(cx, |shell, _window, cx| {
                        shell.deliver_hyperpolling_pairing_failure(
                            &owner_key,
                            session,
                            failure,
                            "Uma communication owner stopped".into(),
                            cx,
                        );
                    });
                }
                return;
            }
            let result = transport
                .request_product(master.clone(), operation, payload)
                .await;
            let _ = shell.update_in(cx, |shell, _window, cx| {
                let Some(workspace) = weak.upgrade() else {
                    return;
                };
                let current = shell
                    .hyperpolling_pairing
                    .get(&owner_key)
                    .is_some_and(|owner| owner.session == session)
                    && shell
                        .hyperpolling_receivers
                        .get(&owner_key)
                        .is_some_and(|owner| {
                            owner.epoch == epoch && !owner.stopped && owner.route == route
                        })
                    && shell.devices.contains(&workspace)
                    && workspace.read(cx).identity(cx) == owner_key
                    && workspace.read(cx).connection_revision() == connection_revision
                    && shell.device_observations.iter().any(|observed| {
                        observed.matches(workspace.read(cx).device(cx)) && route.matches(observed)
                    });
                if !current {
                    return;
                }
                if let Some(failure) = failure {
                    let result = result
                        .map_err(anyhow::Error::msg)
                        .and_then(|value| observation(&intent, session, value));
                    let observed = match result {
                        Ok(observation) => observation,
                        Err(error) => {
                            shell.status = format!("Receiver pairing failed: {error:#}");
                            ReceiverPairingObservation::failed(session, failure)
                        }
                    };
                    workspace.update(cx, |workspace, cx| {
                        workspace.observe_receiver_pairing(observed, cx)
                    });
                }
                cx.notify();
            });
            // Source Unpair can respond before local cache/reconnect cleanup.
            // Keep physical admission closed until its actual worker has joined.
            let mut diagnostics = previous;
            match transport.finish_product_and_wait(master).await {
                Ok(errors) => diagnostics.extend(errors),
                Err(error) => diagnostics.push(error),
            }
            let _ = shell.update_in(cx, |shell, _window, cx| {
                if shell
                    .hyperpolling_pairing
                    .get(&owner_key)
                    .is_some_and(|owner| owner.session == session)
                {
                    if !diagnostics.is_empty() {
                        shell.status =
                            format!("Receiver pairing diagnostics: {}", diagnostics.join("; "));
                    }
                    shell.hyperpolling_pairing.remove(&owner_key);
                    cx.notify();
                }
            });
        });
        self.hyperpolling_pairing.insert(
            identity,
            Session {
                session,
                busy,
                valid: Some(valid),
                _delivery: Some(task),
            },
        );
    }

    fn deliver_hyperpolling_pairing_failure(
        &mut self,
        identity: &str,
        session: u64,
        operation: Option<ReceiverOperation>,
        error: String,
        cx: &mut Context<Self>,
    ) {
        if !self
            .hyperpolling_pairing
            .get(identity)
            .is_some_and(|owner| owner.session == session)
        {
            return;
        }
        if let Some(operation) = operation
            && let Some(workspace) = self
                .hyperpolling_receivers
                .get(identity)
                .and_then(|owner| owner.workspace.upgrade())
        {
            workspace.update(cx, |workspace, cx| {
                workspace.observe_receiver_pairing(
                    ReceiverPairingObservation::failed(session, operation),
                    cx,
                )
            });
        }
        self.status = format!("Receiver pairing delivery is unconfirmed: {error}");
        self.hyperpolling_pairing.remove(identity);
        cx.notify();
    }

    pub(super) fn deliver_hyperpolling_cancel(
        &mut self,
        identity: &str,
        session: u64,
        result: Result<(), String>,
        mut previous: Vec<String>,
        cx: &mut Context<Self>,
    ) {
        if !self
            .hyperpolling_pairing
            .get(identity)
            .is_some_and(|owner| owner.session == session)
        {
            return;
        }
        if let Err(error) = result {
            previous.push(error);
        }
        if !previous.is_empty() {
            self.status = format!("Receiver cancellation diagnostics: {}", previous.join("; "));
        }
        // Cancel has no source UI response. This only releases the retained
        // physical owner after predecessor join and the primary scan4 exchange.
        self.hyperpolling_pairing.remove(identity);
        cx.notify();
    }

    pub(in crate::shell) fn deliver_hyperpolling_bindings(
        &mut self,
        identity: &str,
        workspace: &Entity<ProductWorkspace>,
        session: u64,
        result: Result<ReceiverQueryProjection, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self
            .hyperpolling_pairing
            .get(identity)
            .is_some_and(|owner| owner.session == session)
        {
            return;
        }
        let peers = result
            .as_ref()
            .map_err(|error| anyhow::anyhow!("{error}"))
            .and_then(|projection| projection.pairing_payload())
            .and_then(|value| Ok(serde_json::from_value::<Vec<ReceiverPeer>>(value)?));
        let observed = match peers {
            Ok(peers) => ReceiverPairingObservation::bindings(session, peers),
            Err(error) => {
                self.status = format!("读取配对信息失败：{error:#}");
                ReceiverPairingObservation::failed(session, ReceiverOperation::Bindings)
            }
        };
        let accepted = workspace.update(cx, |workspace, cx| {
            workspace.observe_receiver_pairing(observed, cx)
        });
        self.hyperpolling_pairing.remove(identity);
        if accepted {
            let container = workspace.read(cx).device(cx).device_container_id.clone();
            self.publish_receiver_query(&container, 179, result.as_ref().ok(), window, cx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn hyperpolling_runtime_collection_bindings_keep_hid_scope_without_a_container_parser() {
        let node = razer_device::backend::HidNode {
            path: b"portable-collection".to_vec(),
            vendor_id: 0x1532,
            product_id: 179,
            interface_number: 0,
            usage_page: 0xff00,
            usage: 1,
            release_number: 0,
            serial_number: None,
            manufacturer: None,
            product: None,
        };
        let route = ReceiverRoute::Collection(node);
        let (sender, commands) = mpsc::channel();
        let valid = Arc::new(AtomicBool::new(true));
        queue_collection_bindings(
            &route,
            &ReceiverPairingIntent::QueryBindings,
            73,
            valid.clone(),
            &sender,
        )
        .unwrap()
        .unwrap();
        let Command::Bindings {
            session,
            valid: admitted,
        } = commands.recv().unwrap()
        else {
            panic!()
        };
        assert_eq!(session, 73);
        assert!(Arc::ptr_eq(&admitted, &valid));
        assert!(
            queue_collection_bindings(&route, &ReceiverPairingIntent::Cancel, 74, valid, &sender)
                .is_none()
        );
    }
    #[::core::prelude::v1::test]
    fn hyperpolling_runtime_pairing_source_payload_preserves_radio_and_extensions() {
        let candidate:ReceiverPeer=serde_json::from_value(json!({"productId":183,"dongleId":517,"category":"MOUSE","editionId":0,"extension":{"x":9}})).unwrap();
        let (operation, payload, _) =
            request(&ReceiverPairingIntent::Bind(candidate.clone())).unwrap();
        assert_eq!(operation, "DUALLINK_BIND_DEVICE");
        assert_eq!(payload["mode"], 1);
        assert_eq!(payload["device"]["dongleId"], 517);
        assert_eq!(payload["device"]["extension"]["x"], 9);
        assert!(
            observation(
                &ReceiverPairingIntent::Bind(candidate.clone()),
                7,
                json!({"hardware_confirmed":false,"device":payload["device"]})
            )
            .is_err()
        );
        assert!(
            observation(
                &ReceiverPairingIntent::Bind(candidate),
                7,
                json!({"hardware_confirmed":true,"device":{"productId":183,"dongleId":518}})
            )
            .is_err()
        );
        assert!(
            observation(
                &ReceiverPairingIntent::Unbind(517),
                7,
                json!({"productId":183})
            )
            .is_err()
        );
        assert!(
            observation(
                &ReceiverPairingIntent::Unbind(517),
                7,
                json!({"productId":517})
            )
            .is_ok()
        );
    }
    #[::core::prelude::v1::test]
    fn hyperpolling_runtime_pairing_retired_owner_cannot_release_successor() {
        let busy = Arc::new(AtomicU64::new(11));
        let first = Session {
            session: 11,
            busy: busy.clone(),
            valid: None,
            _delivery: None,
        };
        busy.store(12, Ordering::Release);
        drop(first);
        assert_eq!(busy.load(Ordering::Acquire), 12);
        drop(Session {
            session: 12,
            busy: busy.clone(),
            valid: None,
            _delivery: None,
        });
        assert_eq!(busy.load(Ordering::Acquire), 0);
    }
}
