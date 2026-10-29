//! Independent Unpair reconnect owner. Its Feature handle stays on the worker
//! thread until explicit owner disposal; polling never reads a firmware reply.
use super::{hid, hid_transport, receiver};
use anyhow::{Context as _, ensure};
use razer_device::{
    receiver_pairing::{PairingCapability, for_source},
    receiver_reconnect::{self as protocol, Adapter, Continuation},
};
use serde_json::{Value, json};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

struct Operation {
    id: String,
    canceled: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
    updates: mpsc::Receiver<Value>,
    state: Value,
}

#[derive(Default)]
pub(super) struct Controller {
    active: Option<Operation>,
}

impl Controller {
    pub(super) fn shutdown(&mut self) -> anyhow::Result<()> {
        let Some(mut operation) = self.active.take() else {
            return Ok(());
        };
        operation.canceled.store(true, Ordering::Release);
        if let Some(worker) = operation.worker.take() {
            worker
                .join()
                .map_err(|_| anyhow::anyhow!("Receiver reconnect cleanup worker panicked"))?;
        }
        Ok(())
    }

    pub(super) fn start(
        &mut self,
        id: String,
        product_id: u16,
        container: String,
        category: String,
        source_product_id: Option<u16>,
        next_transaction_sequence: u8,
        remaining_delay_ms: u64,
    ) -> anyhow::Result<Value> {
        ensure!(
            !id.is_empty() && id.len() <= 128 && !id.contains('\0'),
            "重连操作 ID 无效"
        );
        ensure!(self.active.is_none(), "重连句柄仍由之前的操作持有");
        ensure!(
            receiver::valid_container(&container),
            "重连 ContainerId 无效"
        );
        let cap = for_source(product_id, &category, source_product_id)?;
        ensure!(
            next_transaction_sequence < cap.transport.transaction_modulus
                && remaining_delay_ms <= 500,
            "重连事务序号或计时器无效"
        );
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = canceled.clone();
        let (sender, updates) = mpsc::channel();
        let worker = thread::Builder::new().name("receiver-reconnect".into()).spawn(move || {
            let mut adapter = WindowsAdapter {
                cap,
                container,
                canceled: signal,
                updates: sender.clone(),
                handle: None,
                target: None,
                sequence: next_transaction_sequence,
            };
            let continuation = if adapter.cap.transport.source_product_id == 756 {
                Continuation::ReturnedObject
            } else {
                Continuation::ResultField
            };
            let result = protocol::run_with_remaining_delay(&mut adapter, continuation, Duration::from_millis(remaining_delay_ms));
            let retained = result.as_ref().is_ok_and(|result| result.feature_send_complete());
            let state = match result {
                Ok(outcome) => json!({"state":"completed","outcome":outcome,"target":adapter.target,"feature_handle_retained":retained,"firmware_read":false,"device_acknowledgement":false}),
                Err(error) => json!({"state":if adapter.canceled.load(Ordering::Acquire){"canceled"}else{"failed"},"error":format!("{error:#}"),"feature_handle_retained":false}),
            };
            let _ = sender.send(state);
            // Source's host deviceMap retains the successful handle. Rust owns
            // its exact lifetime here, including initial-write failure cleanup.
            if retained {
                while !adapter.canceled.load(Ordering::Acquire) {
                    thread::sleep(Duration::from_millis(50));
                }
            }
            adapter.handle = None;
            let _ = sender.send(json!({"handle_cleanup_complete":true}));
        })?;
        let state = json!({"state":"scheduled","source_timer_ms":500,"feature_handle_retained":false,"source_runtime_ids_reset":false});
        self.active = Some(Operation {
            id: id.clone(),
            canceled,
            worker: Some(worker),
            updates,
            state: state.clone(),
        });
        Ok(json!({"operation_id":id,"reconnect":state}))
    }

    pub(super) fn poll(&mut self, id: &str) -> anyhow::Result<Value> {
        let operation = self.active.as_mut().context("没有独立重连操作")?;
        ensure!(operation.id == id, "重连操作 ID 不匹配");
        while let Ok(update) = operation.updates.try_recv() {
            operation
                .state
                .as_object_mut()
                .unwrap()
                .extend(update.as_object().unwrap().clone());
        }
        if operation
            .worker
            .as_ref()
            .is_some_and(JoinHandle::is_finished)
        {
            let worker = operation.worker.take().unwrap();
            if worker.join().is_err() {
                operation.state = json!({"state":"failed","error":"重连线程异常终止；句柄由线程析构释放","feature_handle_retained":false});
            }
            operation.state["worker_finished"] = json!(true);
            operation.state["feature_handle_retained"] = json!(false);
        }
        Ok(json!({"operation_id":id,"reconnect":operation.state}))
    }

    pub(super) fn cancel(&mut self, id: &str) -> anyhow::Result<Value> {
        let operation = self.active.as_ref().context("没有独立重连操作")?;
        ensure!(operation.id == id, "重连操作 ID 不匹配");
        operation.canceled.store(true, Ordering::Release);
        Ok(json!({"operation_id":id,"owner_disposal_requested":true}))
    }
}

impl Drop for Controller {
    fn drop(&mut self) {
        if let Some(mut operation) = self.active.take() {
            operation.canceled.store(true, Ordering::Release);
            if let Some(worker) = operation.worker.take() {
                let _ = worker.join();
            }
        }
    }
}

struct WindowsAdapter {
    cap: PairingCapability,
    container: String,
    canceled: Arc<AtomicBool>,
    updates: mpsc::Sender<Value>,
    handle: Option<hid_transport::Device>,
    target: Option<Value>,
    sequence: u8,
}

impl Adapter for WindowsAdapter {
    type Target = Value;
    fn check_owner(&self) -> anyhow::Result<()> {
        ensure!(!self.canceled.load(Ordering::Acquire), "重连所有者已关闭");
        Ok(())
    }
    fn wait(&mut self, duration: Duration) -> anyhow::Result<()> {
        let until = Instant::now() + duration;
        while let Some(remaining) = until.checked_duration_since(Instant::now()) {
            self.check_owner()?;
            thread::sleep(remaining.min(Duration::from_millis(50)));
        }
        self.check_owner()
    }
    fn reset_runtime_ids(&mut self) -> anyhow::Result<()> {
        self.check_owner()?;
        self.updates.send(json!({"state":"running","source_runtime_ids_reset":true,"dual_link_primary_pid":0,"dual_link_secondary_pid":65535}))
            .context("重连状态所有者已释放")
    }
    fn enumerate(&mut self) -> anyhow::Result<Option<Vec<Value>>> {
        self.check_owner()?;
        let snapshot =
            hid::enumerate_for_product(self.cap.transport.vendor_id, self.cap.product_id)?;
        select_targets(&snapshot, &self.container, &self.cap)
    }
    fn report(&mut self) -> anyhow::Result<[u8; 91]> {
        self.check_owner()?;
        //164/241 f2(category,true) uses the Unpair class, not the ordinary
        // binding-query factory. Preserve its namespace for initial open.
        let transaction = self.cap.unpair_prefix | self.sequence;
        self.sequence = (self.sequence + 1) % self.cap.transport.transaction_modulus;
        Ok(protocol::initial_report(
            self.cap.transport.claim_interface,
            transaction,
        ))
    }
    fn open_and_send(&mut self, target: &Value, report: &[u8]) -> anyhow::Result<()> {
        self.check_owner()?;
        let _lock = receiver::ReceiverLock::acquire(&self.container, &|| self.check_owner())?;
        let path = target["path"].as_str().context("重连接口缺少实际路径")?;
        let snapshot =
            hid::enumerate_for_product(self.cap.transport.vendor_id, self.cap.product_id)?;
        let current = select_targets(&snapshot, &self.container, &self.cap)?
            .context("重连前物理接收器已断开")?;
        ensure!(
            current.as_slice() == [target.clone()],
            "重连前原物理接口或实例变化"
        );
        let bytes = target["feature_report_bytes"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .context("重连接口缺少 Feature 长度")?;
        self.check_owner()?;
        let validate = || {
            self.check_owner()?;
            let (after, _) = receiver::select_target(
                &hid::enumerate_collection(
                    self.cap.transport.vendor_id,
                    self.cap.product_id,
                    path,
                )?,
                path,
                &self.container,
            )?;
            ensure!(
                after["device_instance_id"] == target["device_instance_id"],
                "重连发送期间设备实例变化"
            );
            Ok(())
        };
        let device = hid_transport::open_validated(path, bytes, std::rc::Rc::new(&validate))?;
        let options = razer_device::backend::ExchangeOptions::mutex_out(Duration::from_millis(
            self.cap.transport.sleep_between_out_ms,
        ))
        .with_cancellation(self.canceled.clone());
        let lease = razer_device::backend::checked_exchange(
            razer_device::backend::FeatureTransport::begin_exchange_validated(
                &device,
                options,
                std::rc::Rc::new(&validate),
            )?,
            &validate,
            &validate,
            None,
        );
        let result = lease.transport().send_feature(report);
        razer_device::backend::finish_exchange(lease, result)?;
        // FeatureTransport checks a complete send. Source's host tests >0;
        // its current HID.node returns the original length on API success.
        self.target = Some(target.clone());
        self.handle = Some(device);
        Ok(())
    }
}

fn select_targets(
    snapshot: &Value,
    container: &str,
    cap: &PairingCapability,
) -> anyhow::Result<Option<Vec<Value>>> {
    ensure!(snapshot["complete"] == true, "重连 HID 枚举未完成");
    let interfaces = snapshot["interfaces"]
        .as_array()
        .context("重连缺少接口列表")?;
    let physical: Vec<_> = interfaces
        .iter()
        .filter(|row| {
            row["vendor_id"] == cap.transport.vendor_id
                && row["product_id"] == cap.product_id
                && row["device_container_id"]
                    .as_str()
                    .is_some_and(|id| id.eq_ignore_ascii_case(container))
        })
        .collect();
    if physical.is_empty() {
        return Ok(None);
    }
    let commands: Vec<_> = physical
        .into_iter()
        .filter(|row| row["claim_interface"] == cap.transport.claim_interface)
        .collect();
    ensure!(commands.len() <= 1, "重连命令 collection 存在歧义");
    commands
        .into_iter()
        .map(|row| {
            let path = row["path"].as_str().context("重连 collection 缺少路径")?;
            let (verified, _) = receiver::select_target(snapshot, path, container)?;
            ensure!(
                verified["product_id"] == cap.product_id
                    && verified["claim_interface"] == cap.transport.claim_interface,
                "重连实际产品接口不匹配"
            );
            Ok(verified)
        })
        .collect::<anyhow::Result<Vec<_>>>()
        .map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn target_enumeration_distinguishes_absence_from_interface_failure() {
        let cap = for_source(164, "MOUSE", None).unwrap();
        let owner = "{11111111-2222-3333-4444-555555555555}";
        let row = |interface| json!({"vendor_id":5426,"product_id":164,"claim_interface":interface,"device_container_id":owner,"feature_report_bytes":91,"path":r"\\?\hid#reconnect","device_instance_id":"simulated-instance"});
        assert!(
            select_targets(&json!({"complete":true,"interfaces":[]}), owner, &cap)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            select_targets(&json!({"complete":true,"interfaces":[row(9)]}), owner, &cap).unwrap(),
            Some(vec![])
        );
        let selected = row(cap.transport.claim_interface);
        assert_eq!(
            select_targets(
                &json!({"complete":true,"interfaces":[selected.clone()]}),
                owner,
                &cap
            )
            .unwrap(),
            Some(vec![selected.clone()])
        );
        assert!(
            select_targets(
                &json!({"complete":true,"interfaces":[selected.clone(),selected]}),
                owner,
                &cap
            )
            .is_err()
        );
        assert!(select_targets(&json!({"complete":false,"interfaces":[]}), owner, &cap).is_err());
    }

    #[test]
    fn controller_disposal_joins_the_thread_and_releases_its_retained_simulated_handle() {
        struct SimulatedHandle(Arc<AtomicBool>);
        impl Drop for SimulatedHandle {
            fn drop(&mut self) {
                self.0.store(true, Ordering::Release);
            }
        }
        let canceled = Arc::new(AtomicBool::new(false));
        let signal = canceled.clone();
        let released = Arc::new(AtomicBool::new(false));
        let release = released.clone();
        let (ready_sender, ready) = mpsc::channel();
        let worker = thread::spawn(move || {
            let _handle = SimulatedHandle(release);
            ready_sender.send(()).unwrap();
            while !signal.load(Ordering::Acquire) {
                thread::yield_now();
            }
        });
        let (_sender, updates) = mpsc::channel();
        let mut controller = Controller {
            active: Some(Operation {
                id: "simulated-owner".into(),
                canceled: canceled.clone(),
                worker: Some(worker),
                updates,
                state: json!({"state":"completed","feature_handle_retained":true}),
            }),
        };
        ready.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(!released.load(Ordering::Acquire));
        assert!(controller.cancel("another-owner").is_err());
        assert!(!canceled.load(Ordering::Acquire));
        controller.cancel("simulated-owner").unwrap();
        drop(controller);
        assert!(released.load(Ordering::Acquire));
    }
}
