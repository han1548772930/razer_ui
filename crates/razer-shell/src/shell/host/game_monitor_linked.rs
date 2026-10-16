//! Serial background owner for local profile-stack I/O and foreground IPC.
//! No filesystem or service request is performed while borrowing GPUI entities.
use gpui_kit::EntityId;
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_storage::{
    linked_profiles::{self, ProfileStack},
    source_local,
};
use serde_json::Value;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, Sender},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

pub(super) enum Command {
    Load {
        owner: EntityId,
        epoch: u64,
        product_id: u32,
        base: String,
        valid: Arc<AtomicBool>,
    },
    Save {
        owner: EntityId,
        epoch: u64,
        product_id: u32,
        stack: ProfileStack,
        valid: Arc<AtomicBool>,
    },
    SelectBase {
        product_id: u32,
        guid: String,
    },
    Foreground(bool),
    Stop,
}
pub(super) enum Reply {
    Loaded {
        owner: EntityId,
        epoch: u64,
        result: Result<ProfileStack, String>,
    },
    Saved {
        owner: EntityId,
        epoch: u64,
        result: Result<(), String>,
    },
    Foreground(Vec<Value>),
    Diagnostic(String),
}
pub(super) struct Worker {
    commands: Sender<Command>,
    worker: Option<JoinHandle<()>>,
    valid: Arc<AtomicBool>,
}
impl Worker {
    pub(super) fn start(view_url: &str) -> anyhow::Result<(Self, Receiver<Reply>)> {
        let (commands, receiver) = mpsc::channel();
        let (updates, replies) = mpsc::channel();
        let valid = Arc::new(AtomicBool::new(true));
        let signal = valid.clone();
        let url = view_url.to_owned();
        let worker = thread::Builder::new()
            .name("local-keyboard-game-links".into())
            .spawn(move || {
                let path = source_local::path();
                let mut foreground: Option<ServiceClient> = None;
                let mut observe_foreground = false;
                let mut retry = Instant::now();
                let mut last_error = None;
                while signal.load(Ordering::Acquire) {
                    match receiver.recv_timeout(Duration::from_millis(100)) {
                        Ok(Command::Load {
                            owner,
                            epoch,
                            product_id,
                            base,
                            valid,
                        }) => {
                            if !valid.load(Ordering::Acquire) {
                                continue;
                            }
                            let result = linked_profiles::read(&path, product_id)
                                .map(|stack| stack.unwrap_or_else(|| ProfileStack::new(base)))
                                .map_err(|error| format!("{error:#}"));
                            if valid.load(Ordering::Acquire)
                                && updates
                                    .send(Reply::Loaded {
                                        owner,
                                        epoch,
                                        result,
                                    })
                                    .is_err()
                            {
                                break;
                            }
                        }
                        Ok(Command::Save {
                            owner,
                            epoch,
                            product_id,
                            stack,
                            valid,
                        }) => {
                            if !valid.load(Ordering::Acquire) {
                                continue;
                            }
                            let result = linked_profiles::save(&path, product_id, &stack)
                                .map_err(|error| format!("{error:#}"));
                            if valid.load(Ordering::Acquire)
                                && updates
                                    .send(Reply::Saved {
                                        owner,
                                        epoch,
                                        result,
                                    })
                                    .is_err()
                            {
                                break;
                            }
                        }
                        Ok(Command::SelectBase { product_id, guid }) => {
                            let result = (|| -> anyhow::Result<()> {
                                let mut stack = linked_profiles::read(&path, product_id)?
                                    .unwrap_or_else(|| ProfileStack::new(&guid));
                                stack.select_base(guid);
                                linked_profiles::save(&path, product_id, &stack)
                            })();
                            if let Err(error) = result {
                                let _ = updates.send(Reply::Diagnostic(format!(
                                    "离线键盘配置选择：{error:#}"
                                )));
                            }
                        }
                        Ok(Command::Foreground(enabled)) => {
                            observe_foreground = enabled;
                            retry = Instant::now();
                            if !enabled {
                                stop_foreground(&mut foreground, &url, &updates);
                            }
                        }
                        Ok(Command::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                    }
                    if !observe_foreground || Instant::now() < retry {
                        continue;
                    }
                    if foreground.is_none() {
                        let started = (|| -> anyhow::Result<ServiceClient> {
                            let mut client = ServiceClient::spawn()?;
                            client.set_request_guard(signal.clone());
                            anyhow::ensure!(
                                client.request(ServiceRequest::ForegroundMonitorStart {
                                    view_url: url.clone()
                                })? == true,
                                "Foreground monitor did not acknowledge its subscription"
                            );
                            Ok(client)
                        })();
                        match started {
                            Ok(client) => {
                                foreground = Some(client);
                                last_error = None;
                            }
                            Err(error) => {
                                let error = format!("前台窗口观察：{error:#}");
                                if last_error.as_ref() != Some(&error) {
                                    let _ = updates.send(Reply::Diagnostic(error.clone()));
                                    last_error = Some(error);
                                }
                                retry = Instant::now() + Duration::from_secs(1);
                                continue;
                            }
                        }
                    }
                    if let Some(client) = &mut foreground {
                        let result = client.request(ServiceRequest::ForegroundMonitorEvents {
                            view_url: url.clone(),
                        });
                        match result.and_then(|events| {
                            events.as_array().cloned().ok_or_else(|| {
                                anyhow::anyhow!("Foreground event reply is not an array")
                            })
                        }) {
                            Ok(events) => {
                                if !events.is_empty()
                                    && updates.send(Reply::Foreground(events)).is_err()
                                {
                                    break;
                                }
                            }
                            Err(error) => {
                                let _ = updates
                                    .send(Reply::Diagnostic(format!("前台窗口观察：{error:#}")));
                                stop_foreground(&mut foreground, &url, &updates);
                                retry = Instant::now() + Duration::from_secs(1);
                            }
                        }
                    }
                }
                stop_foreground(&mut foreground, &url, &updates);
            })?;
        Ok((
            Self {
                commands,
                worker: Some(worker),
                valid,
            },
            replies,
        ))
    }
    pub(super) fn send(&self, command: Command) -> anyhow::Result<()> {
        self.commands
            .send(command)
            .map_err(|_| anyhow::anyhow!("Local keyboard linked-profile worker stopped"))
    }
    pub(super) fn retire(mut self) -> Option<JoinHandle<()>> {
        self.valid.store(false, Ordering::Release);
        let _ = self.commands.send(Command::Stop);
        self.worker.take()
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.valid.store(false, Ordering::Release);
        let _ = self.commands.send(Command::Stop);
    }
}
fn stop_foreground(client: &mut Option<ServiceClient>, url: &str, updates: &Sender<Reply>) {
    let Some(mut client) = client.take() else {
        return;
    };
    // Cleanup has its own bounded guard: withdrawing the owner cancels new
    // observations, while the stop request must still reach the local service.
    client.set_request_guard(Arc::new(AtomicBool::new(true)));
    if let Err(error) = client.request(ServiceRequest::ForegroundMonitorStop {
        view_url: url.into(),
    }) {
        let _ = updates.send(Reply::Diagnostic(format!("清理前台窗口观察：{error:#}")));
    }
    let _ = client.request(ServiceRequest::Shutdown);
}
