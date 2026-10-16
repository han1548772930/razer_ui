//! Serial owner for the source Ea modal's K/Z lifecycle. Native suppression,
//! keyboard-only input redirect, hardware commands and serial-keyed local
//! records complete on this thread; no GPUI entity is borrowed during I/O.
use razer_device::keyboard_legacy_calibration::{
    self as calibration, Message, Operation, Report, Target,
};
use razer_discovery::{
    direct::{keyboard_legacy_calibration as route, keyboard_metadata},
    discovery::ObservedDevice,
};
use razer_ipc::{KEY_CAPTURE_LIFECYCLE, ServiceClient, ServiceRequest};
use razer_storage::{keyboard_calibration, source_local};
use serde_json::{Value, json};
use std::{
    path::Path,
    sync::{
        Arc, MutexGuard, TryLockError,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};

pub(super) enum Command {
    Mount(u64, bool),
    Active(bool),
    Initialize,
    Cleanup,
    Unmount,
    Intent(u64, Value),
}
pub(super) enum Reply {
    At(u64, Box<Reply>),
    Capture(bool),
    Input(String),
    Foreground(String),
    Messages(Vec<Message>),
    Finished(u64, Result<(), String>),
    Diagnostic(String),
    Stopped,
}
fn modal_reply(replies: &mpsc::Sender<Reply>, epoch: u64, reply: Reply) -> bool {
    replies.send(Reply::At(epoch, Box::new(reply))).is_ok()
}
fn diagnostic(replies: &mpsc::Sender<Reply>, error: impl std::fmt::Display) {
    let _ = replies.send(Reply::Diagnostic(error.to_string()));
}
fn current_executable(event: &Value) -> bool {
    let Some(name) = event["data"]["name"].as_str() else {
        return false;
    };
    let Some(path) = event["data"]["path"].as_str() else {
        return false;
    };
    let Ok(executable) = std::env::current_exe() else {
        return false;
    };
    let observed = Path::new(path).join(name).to_string_lossy().into_owned();
    executable
        .to_string_lossy()
        .trim_start_matches(r"\\?\")
        .eq_ignore_ascii_case(observed.trim_start_matches(r"\\?\"))
}
fn cleanup(
    client: &mut ServiceClient,
    target: &Target,
    initialized: &mut bool,
    lifecycle: &mut Option<MutexGuard<'static, ()>>,
    replies: &mpsc::Sender<Reply>,
    epoch: u64,
) -> anyhow::Result<()> {
    if !*initialized && lifecycle.is_none() {
        return Ok(());
    }
    // Ea.Z resets local state before enabling mappings and releasing redirects.
    modal_reply(replies, epoch, Reply::Capture(false));
    client.set_request_guard(Arc::new(AtomicBool::new(true)));
    let capture = client
        .request(ServiceRequest::StopMappingKeyCapture)
        .map_err(|error| anyhow::anyhow!("恢复键盘映射及输入重定向：{error:#}"));
    match route::execute(client, target.clone(), Operation::Stop) {
        Ok(report) => {
            if let Some(error) = report.error {
                diagnostic(replies, error)
            }
            modal_reply(replies, epoch, Reply::Messages(report.messages));
        }
        Err(error) => diagnostic(replies, format!("退出键盘校准模式：{error:#}")),
    }
    *initialized = false;
    // If native cleanup failed, retain the lock until the service's final
    // Shutdown attempt. A new recorder must not race the old restoration.
    capture?;
    *lifecycle = None;
    Ok(())
}
fn perform(
    client: &mut ServiceClient,
    target: &Target,
    pid: u32,
    japanese: bool,
    serial: &str,
    message: &Value,
) -> anyhow::Result<Report> {
    let operation = calibration::operation(pid, message, japanese)?
        .ok_or_else(|| anyhow::anyhow!("Unrecognized calibration command"))?;
    let mut report = route::execute(client, target.clone(), operation)?;
    if report.verified_top {
        let saved = (|| -> anyhow::Result<Vec<Value>> {
            let input = message["payload"][0]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Calibration input missing"))?;
            let (input, input_type) = calibration::key_record(pid, input)?;
            keyboard_calibration::save_key(&source_local::path(), pid, serial, input, input_type)
        })();
        match saved {
            Ok(keys) => report.messages.push(Message {
                action: "MW_UPDATE_ANALOG_CALIBRATED_KEY_LIST_TO_UI".into(),
                payload: json!({"calibratedKeys":keys}),
            }),
            Err(error) => {
                // Current MW catches storage failures before publishing the
                // verification result, then stops and reports false.
                let error = format!("保存本地校准记录：{error:#}");
                report.messages.clear();
                match route::execute(client, target.clone(), Operation::Stop) {
                    Ok(stop) => {
                        report.messages.extend(stop.messages);
                        report.error = Some(match stop.error {
                            Some(stop) => format!("{error}; {stop}"),
                            None => error,
                        });
                    }
                    Err(stop) => report.error = Some(format!("{error}; stop: {stop:#}")),
                }
                report.messages.push(Message {
                    action: "MW_UPDATE_ANALOG_ADC_VERIFICATION_STATUS_TO_UI".into(),
                    payload: json!({"direction":"top","isSuccess":false}),
                });
                report.verified_top = false;
                report.completed = false;
            }
        }
    }
    Ok(report)
}
pub(super) fn run(
    observed: ObservedDevice,
    valid: Arc<AtomicBool>,
    commands: mpsc::Receiver<Command>,
    replies: mpsc::Sender<Reply>,
    url: String,
) {
    let result = (|| -> anyhow::Result<()> {
        let mut client = ServiceClient::spawn()?;
        client.set_request_guard(valid.clone());
        let mut initialized = false;
        let mut lifecycle = None;
        let mut monitoring = false;
        let mut mounted = false;
        let mut host_active = false;
        let mut desired = false;
        let mut modal_epoch = 0;
        let result = (|| -> anyhow::Result<()> {
            let target = route::resolve(&mut client, &observed)?;
            let identity = keyboard_metadata::resolve(&mut client, &observed)?;
            let metadata = identity.read_metadata(&mut client)?;
            let serial = identity.read_serial(&mut client)?;
            anyhow::ensure!(!serial.is_empty(), "键盘没有返回可用于校准记录的序列号");
            let keys =
                keyboard_calibration::read(&source_local::path(), observed.product_id(), &serial)?;
            if !keys.is_empty() {
                let _ = replies.send(Reply::Messages(vec![Message {
                    action: "MW_UPDATE_ANALOG_CALIBRATED_KEY_LIST_TO_UI".into(),
                    payload: json!({"calibratedKeys":keys}),
                }]));
            }
            let result = (|| -> anyhow::Result<()> {
                while valid.load(Ordering::Acquire) {
                    let command = match commands.recv_timeout(Duration::from_millis(16)) {
                        Ok(command) => Some(command),
                        Err(mpsc::RecvTimeoutError::Timeout) => None,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    };
                    match command {
                        Some(Command::Mount(epoch, active)) => {
                            modal_epoch = epoch;
                            if !monitoring {
                                anyhow::ensure!(
                                    client.request(ServiceRequest::ForegroundMonitorStart {
                                        view_url: url.clone()
                                    })? == true,
                                    "校准前台观察未确认启动"
                                );
                                monitoring = true;
                            }
                            mounted = true;
                            host_active = active;
                            desired = active;
                        }
                        Some(Command::Active(active)) => {
                            host_active = active;
                            desired = mounted && active;
                        }
                        Some(Command::Initialize) => {
                            desired = mounted && host_active;
                        }
                        Some(Command::Cleanup) => {
                            desired = false;
                        }
                        Some(Command::Unmount) => {
                            desired = false;
                            mounted = false;
                            // Source unmount detaches the monitor before Z.
                            if monitoring {
                                client.request(ServiceRequest::ForegroundMonitorStop {
                                    view_url: url.clone(),
                                })?;
                                monitoring = false;
                            }
                        }
                        Some(Command::Intent(generation, message)) => {
                            let result = if initialized {
                                perform(
                                    &mut client,
                                    &target,
                                    observed.product_id(),
                                    metadata.keyboard_layout == 12,
                                    &serial,
                                    &message,
                                )
                                .map(|report| {
                                    let result = match report.error {
                                        Some(error) => Err(error),
                                        None => Ok(()),
                                    };
                                    modal_reply(
                                        &replies,
                                        modal_epoch,
                                        Reply::Messages(report.messages),
                                    );
                                    result
                                })
                                .unwrap_or_else(|error| Err(format!("{error:#}")))
                            } else {
                                Err("键盘校准尚未取得输入重定向".into())
                            };
                            let _ = replies.send(Reply::Finished(generation, result));
                        }
                        None => {}
                    }
                    if !desired && initialized {
                        cleanup(
                            &mut client,
                            &target,
                            &mut initialized,
                            &mut lifecycle,
                            &replies,
                            modal_epoch,
                        )?;
                        client.set_request_guard(valid.clone());
                    }
                    if monitoring {
                        let events = client.request(ServiceRequest::ForegroundMonitorEvents {
                            view_url: url.clone(),
                        })?;
                        for event in events
                            .as_array()
                            .ok_or_else(|| anyhow::anyhow!("校准前台事件列表缺失"))?
                        {
                            let own = current_executable(event);
                            desired = mounted && host_active && own;
                            let name = if own {
                                "RazerAppEngine.exe"
                            } else {
                                event["data"]["name"].as_str().unwrap_or("")
                            };
                            modal_reply(&replies, modal_epoch, Reply::Foreground(name.into()));
                        }
                    }
                    if !desired && initialized {
                        cleanup(
                            &mut client,
                            &target,
                            &mut initialized,
                            &mut lifecycle,
                            &replies,
                            modal_epoch,
                        )?;
                        client.set_request_guard(valid.clone());
                    }
                    if desired && !initialized {
                        let guard = match KEY_CAPTURE_LIFECYCLE.try_lock() {
                            Ok(guard) => Some(guard),
                            Err(TryLockError::WouldBlock) => None,
                            Err(TryLockError::Poisoned(_)) => {
                                anyhow::bail!("Keyboard capture lifecycle lock poisoned")
                            }
                        };
                        if let Some(guard) = guard {
                            lifecycle = Some(guard);
                            let ready = client.request(
                                ServiceRequest::StartKeyboardCalibrationCapture {
                                    product_id: observed.product_id(),
                                    container_id: observed.container().into(),
                                },
                            )?;
                            anyhow::ensure!(
                                ready["capturing"] == true
                                    && ready["devices"].as_array().is_some_and(|devices| devices
                                        .len()
                                        == 1
                                        && devices[0]["productId"] == observed.product_id()
                                        && devices[0]["containerId"].as_str().is_some_and(
                                            |id| id.eq_ignore_ascii_case(observed.container())
                                        )),
                                "校准输入重定向没有实际目标设备确认"
                            );
                            initialized = true;
                            modal_reply(&replies, modal_epoch, Reply::Capture(true));
                        }
                    }
                    if initialized {
                        let events = client.request(ServiceRequest::MappingKeyCaptureEvents)?;
                        for event in events["events"]
                            .as_array()
                            .ok_or_else(|| anyhow::anyhow!("校准输入事件列表缺失"))?
                        {
                            if event["eventType"] != 3
                                || event["deviceInfo"]["productId"] != observed.product_id()
                                || !event["deviceInfo"]["containerId"]
                                    .as_str()
                                    .is_some_and(|id| id.eq_ignore_ascii_case(observed.container()))
                            {
                                continue;
                            }
                            let input = event["input"]
                                .as_str()
                                .map(str::to_owned)
                                .unwrap_or_else(|| event["input"].to_string());
                            if !modal_reply(&replies, modal_epoch, Reply::Input(input)) {
                                return Ok(());
                            }
                        }
                    }
                }
                Ok(())
            })();
            client.set_request_guard(Arc::new(AtomicBool::new(true)));
            if monitoring {
                if let Err(error) = client.request(ServiceRequest::ForegroundMonitorStop {
                    view_url: url.clone(),
                }) {
                    diagnostic(&replies, format!("退出校准前台观察：{error:#}"));
                }
            }
            if let Err(error) = cleanup(
                &mut client,
                &target,
                &mut initialized,
                &mut lifecycle,
                &replies,
                modal_epoch,
            ) {
                diagnostic(&replies, format!("{error:#}"));
            }
            result
        })();
        client.set_request_guard(Arc::new(AtomicBool::new(true)));
        if let Err(error) = client.request(ServiceRequest::Shutdown) {
            diagnostic(&replies, format!("清理键盘校准服务：{error:#}"))
        }
        result
    })();
    if let Err(error) = result {
        diagnostic(&replies, format!("{error:#}"))
    }
    for command in commands.try_iter() {
        if let Command::Intent(generation, _) = command {
            let _ = replies.send(Reply::Finished(
                generation,
                Err("键盘校准通信已结束".into()),
            ));
        }
    }
    let _ = replies.send(Reply::Stopped);
}
