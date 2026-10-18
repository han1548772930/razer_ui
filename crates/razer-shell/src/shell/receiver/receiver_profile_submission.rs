//! Actual refresh receipts follow successful workspace persistence.
use super::*;

struct Completion {
    brightness: Option<razer_device::receiver_brightness::Submission>,
    error: Option<String>,
}

fn execute(
    device: &Device,
    operation: Operation,
    profile: &PreparedProfile,
    canceled: &Arc<AtomicBool>,
    lighting: &super::super::super::receiver_lighting_owner::RetainedOwner,
) -> anyhow::Result<Completion> {
    guard(canceled)?;
    let mut client = ServiceClient::spawn()?;
    client.set_cancellation_guard(canceled.clone());
    let result = (|| -> anyhow::Result<Completion> {
        let path = super::super::super::receiver_pairing::route::command_path(
            &mut client,
            device.product_id,
            &device.device_container_id,
        )?;
        ensure!(
            path == profile.path,
            "Receiver command interface changed after local persistence"
        );
        ensure!(
            serial(&mut client, device, &path, canceled)? == profile.snapshot.serial_number(),
            "Receiver serial changed after local persistence"
        );
        let percent = profile.snapshot.brightness()?.percent();
        if let Operation::Brightness(request) = operation {
            ensure!(
                request.percent() == percent,
                "Persisted brightness differs from its request"
            );
        }
        if let Some(refresh) = &profile.refresh {
            ensure!(
                refresh.document() == &profile.snapshot.document()?,
                "Receiver reset refresh differs from the persisted source document"
            );
        }
        ensure!(
            profile.snapshot.active_mappings()?.is_empty(),
            "Primary Linker source unexpectedly contains mappings for another engine"
        );
        guard(canceled)?;
        let response: BrightnessResponse =
            serde_json::from_value(client.request(ServiceRequest::ReceiverBrightnessWrite {
                path: path.clone(),
                device_container_id: device.device_container_id.clone(),
                percent,
            })?)?;
        guard(canceled)?;
        ensure!(
            response.product_id == device.product_id
                && response.path == path
                && response.device_container_id == device.device_container_id
                && response.result.requested_percent == percent,
            "Receiver brightness receipt belongs to a different owner or request"
        );
        let mut error = response.result.error.clone();
        if !response.result.source_completed && error.is_none() {
            error = Some("Receiver brightness task returned no completion receipt".into());
        }
        // Storage-change effects have their own retained source owner. A
        // Feature brightness response cannot certify this stage.
        if !matches!(operation, Operation::Brightness(_)) {
            let configuration = razer_device::receiver_lighting::Configuration {
                product_id: u16::try_from(device.product_id)?,
                edition_id: u8::try_from(device.edition_id)?,
                effect: razer_device::receiver_lighting::QuickEffect::parse(
                    &profile.snapshot.active_quick_effect()?,
                )?,
                selected_docking_mouse: razer_device::receiver_lighting::DockMouseSelection::parse(
                    profile.snapshot.selected_docking_mouse()?,
                )?,
            };
            if let Err(lighting_error) = lighting.apply(&path, configuration, canceled) {
                let detail = format!("接收器配置已保存；灯效提交未确认：{lighting_error:#}");
                error = Some(error.map_or(detail.clone(), |error| format!("{error}; {detail}")));
            }
        }
        Ok(Completion {
            brightness: Some(response.result),
            error,
        })
    })();
    let cleanup = shutdown(&mut client);
    match (result, cleanup) {
        (Ok(mut completion), Err(error)) => {
            let detail = format!("接收器通信清理失败：{error:#}");
            completion.error = Some(
                completion
                    .error
                    .map_or(detail.clone(), |error| format!("{error}; {detail}")),
            );
            Ok(completion)
        }
        (Err(error), Err(cleanup)) => Err(anyhow::anyhow!(
            "{error:#}; receiver cleanup failed: {cleanup:#}"
        )),
        (result, Ok(_)) => result,
    }
}

impl AppShell {
    pub(super) fn start_receiver_profile_submission(
        &mut self,
        save: LocalSave,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let device = save.owner.workspace.read(cx).snapshot(cx);
        let signal = save.owner.canceled.clone();
        let operation = save.owner.operation;
        let key = save.owner.identity.clone();
        let canceled = signal.clone();
        let profile = save.profile;
        let owner = save.owner;
        let lighting = self.receiver_lighting_owner(
            &owner.identity,
            owner.route.clone(),
            owner.connection_revision,
            device.device_container_id.clone(),
            cx,
        );
        let (sender, receiver) = mpsc::sync_channel(1);
        let worker = thread::Builder::new()
            .name("receiver-profile-submit".into())
            .spawn(move || {
                let _ = sender.send(
                    execute(&device, operation, &profile, &canceled, &lighting)
                        .map_err(|error| format!("{error:#}")),
                );
            });
        let worker = match worker {
            Ok(worker) => worker,
            Err(error) => {
                self.finish_receiver_profile_error(
                    &owner,
                    format!("接收器配置已保存；无法启动设备任务：{error}"),
                    window,
                    cx,
                );
                return;
            }
        };
        let delivery = cx.spawn_in(window, async move |shell, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(25))
                    .await;
                let incoming = receiver.try_recv();
                let terminal = !matches!(incoming, Err(mpsc::TryRecvError::Empty));
                let current = shell.update_in(cx, |shell, window, cx| {
                    if !shell.receiver_profile_current(&owner, cx) {
                        shell.finish_receiver_profile_error(
                            &owner,
                            "接收器配置已保存；会话已变化，未采用旧设备回执。".into(),
                            window,
                            cx,
                        );
                        return false;
                    }
                    let completion = match incoming {
                        Ok(Ok(completion)) => Some(completion),
                        Ok(Err(error)) => Some(Completion {
                            brightness: None,
                            error: Some(format!("接收器配置已保存；设备任务未确认：{error}")),
                        }),
                        Err(mpsc::TryRecvError::Disconnected) => Some(Completion {
                            brightness: None,
                            error: Some("接收器配置已保存；设备任务没有返回结果。".into()),
                        }),
                        Err(mpsc::TryRecvError::Empty) => None,
                    };
                    if let Some(completion) = completion {
                        shell.retire_receiver_profile(&owner);
                        shell.status =
                            completion.error.clone().unwrap_or_else(|| match operation {
                                Operation::Brightness(_) => "接收器亮度原链提交完成。".into(),
                                Operation::Help(_) => {
                                    "接收器重置配置已保存，亮度与灯效已提交。".into()
                                }
                                Operation::Selection(_) => {
                                    "接收器配置已保存，亮度与灯效已提交。".into()
                                }
                            });
                        owner.workspace.update(cx, |view, cx| match operation {
                            Operation::Selection(request) => view
                                .finish_receiver_profile_selection(
                                    request,
                                    completion.error,
                                    window,
                                    cx,
                                ),
                            Operation::Help(request) => {
                                view.finish_help_reset(request, None, completion.error, window, cx)
                            }
                            Operation::Brightness(request) => {
                                let submitted = completion
                                    .brightness
                                    .as_ref()
                                    .filter(|result| result.source_completed)
                                    .map(|result| result.requested_percent);
                                let observed = completion.brightness.as_ref().and_then(|result| {
                                    result
                                        .observed
                                        .as_ref()
                                        .map(|brightness| brightness.percent)
                                });
                                view.finish_receiver_brightness(
                                    request.generation(),
                                    Some(request.percent()),
                                    None,
                                    submitted,
                                    observed,
                                    completion.error,
                                    true,
                                    window,
                                    cx,
                                );
                            }
                        });
                        cx.notify();
                    }
                    true
                });
                if current.is_err() {
                    owner.canceled.store(true, Ordering::Release);
                    let _ = shell.update(cx, |shell, _| shell.retire_receiver_profile(&owner));
                }
                if terminal || !matches!(current, Ok(true)) {
                    break;
                }
            }
        });
        self.retain_receiver_profile_worker_parts(&key, operation, signal, worker, delivery);
    }
}
