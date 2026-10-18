//! Help's local default publication and retained headset factory submission.
use super::*;
use razer_device::thx_submission::{ThxSubmissionProgress, ThxSubmissionScope};

pub(in crate::shell) struct LocalSave {
    workspace: Entity<ProductWorkspace>,
    identity: String,
    request: HelpResetRequest,
    canceled: Arc<AtomicBool>,
    original: Device,
    candidate: Device,
    expected_profile_id: String,
    connection_revision: u64,
    owner: std::result::Result<super::super::thx_sound::OwnerSlot, String>,
    scope: ThxSubmissionScope,
}

#[cfg(test)]
#[path = "help_hardware_reset_tests.rs"]
mod tests;

impl LocalSave {
    pub(in crate::shell) fn owner(&self) -> (&str, WeakEntity<ProductWorkspace>) {
        (&self.identity, self.workspace.downgrade())
    }
}

#[derive(Debug, PartialEq)]
struct ResetFields<'a> {
    profiles: &'a [razer_model::model::Profile],
    settings: &'a Option<Value>,
    active: &'a str,
}

fn reset_fields(device: &Device) -> ResetFields<'_> {
    ResetFields {
        profiles: &device.profiles,
        settings: &device.source_device_settings,
        active: &device.active_profile,
    }
}

fn candidate(device: &Device, computer_name: &str, guid: &str) -> anyhow::Result<Device> {
    ensure!(
        !device
            .profiles
            .iter()
            .any(|profile| profile.id == guid || profile.guid == guid),
        "Fresh reset GUID already exists in this owner"
    );
    let reset = razer_device::blackshark_reset::prepare(
        device.product_id,
        device.source_device_settings.as_ref(),
        &device
            .profiles
            .iter()
            .map(|profile| profile.name.as_str())
            .collect::<Vec<_>>(),
        computer_name,
        guid,
    )?;
    let profile = serde_json::to_value(&reset.profile)?;
    let defaults = serde_json::to_value(reset.device)?;
    let mut candidate = device.clone();
    let settings = candidate
        .source_device_settings
        .get_or_insert_with(|| Value::Object(Default::default()))
        .as_object_mut()
        .context("Current reset settings are not an object")?;
    settings.extend(
        defaults
            .as_object()
            .context("Typed reset defaults are not an object")?
            .clone(),
    );
    candidate.profiles.push(razer_model::model::Profile {
        id: reset.profile.guid.clone(),
        guid: reset.profile.guid.clone(),
        name: reset.profile.name,
        source_settings: Some(profile),
        settings: None,
        dpi_stages: None,
    });
    candidate.active_profile = reset.profile.guid;
    Ok(candidate)
}

fn reset_error(
    progress: &ThxSubmissionProgress,
    expected: &ThxSubmissionScope,
    physical: &razer_device::thx_lifecycle_discovery::ThxConnectionIdentity,
) -> Option<String> {
    use razer_device::{
        thx_blackshark_firmware::BlackSharkFirmwareCommand,
        thx_submission::{ThxFirmwareProgress, ThxSubmissionPlan},
    };
    let mut errors = Vec::new();
    errors.extend(progress.error.clone());
    errors.extend(progress.cleanup_error.clone());
    if progress.scope != *expected || progress.plan != ThxSubmissionPlan::FactoryReset1398 {
        errors.push("Hardware reset response identity differs".into());
    }
    if !progress.firmware.iter().any(|receipt| matches!(receipt,
        ThxFirmwareProgress::Command(command)
        if command.command == BlackSharkFirmwareCommand::FactoryReset1398
            && command.identity.owner_product_id == expected.product_id
            && command.identity.device_container_id.eq_ignore_ascii_case(&expected.container_id)
            && command.identity.node.path == physical.hid_path
            && command.identity.node.product_id == physical.physical_product_id
            && command.identity.node.serial_number.as_ref().is_none_or(|serial| serial == &physical.serial_number)
            && command.succeeded() && command.acknowledged())) {
        errors.push("Hardware reset has no confirmed factory command receipt".into());
    }
    (!errors.is_empty()).then(|| {
        format!(
            "Default profile was saved locally; hardware reset was not confirmed: {}",
            errors.join("; ")
        )
    })
}

impl AppShell {
    pub(in crate::shell) fn request_help_hardware_reset1398(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: HelpResetRequest,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let prepared = (|| -> anyhow::Result<LocalSave> {
            ensure!(
                self.storage_error.is_none() && !self.close_requested,
                "Local reset storage is unavailable"
            );
            ensure!(
                workspace.read(cx).help_reset_matches(request, cx)
                    && request.hardware_reset_message().is_some(),
                "Help reset request has expired"
            );
            let device = workspace.read(cx).snapshot(cx);
            let candidate = candidate(
                &device,
                &std::env::var("COMPUTERNAME").unwrap_or_default(),
                &uuid::Uuid::new_v4().to_string(),
            )?;
            let scope = ThxSubmissionScope {
                product_id: device.product_id,
                container_id: device.device_container_id.clone(),
                serial_number: device.serial_number.clone(),
                profile_id: candidate.active_profile.clone(),
                generation: request.generation(),
            };
            let owner = scope
                .validate()
                .and_then(|_| self.help_factory_reset1398_owner(&workspace, &scope, cx))
                .map_err(|error| format!("{error:#}"));
            Ok(LocalSave {
                identity: workspace.read(cx).identity(cx),
                connection_revision: workspace.read(cx).connection_revision(),
                workspace: workspace.clone(),
                request,
                canceled: Arc::new(AtomicBool::new(false)),
                expected_profile_id: device.active_profile.clone(),
                original: device,
                candidate,
                owner,
                scope,
            })
        })();
        let save = match prepared {
            Ok(save) => save,
            Err(error) => {
                workspace.update(cx, |view, cx| view.finish_help_reset(request, None,
                    Some(format!("Default profile was not saved; hardware reset was not submitted: {error:#}")), window, cx));
                return;
            }
        };
        self.retire_help_reset(&save.identity);
        self.receiver_reset.insert(
            save.identity.clone(),
            Session {
                request,
                canceled: save.canceled.clone(),
                worker: None,
                _delivery: None,
            },
        );
        let file = self.workspace_file(
            self.devices
                .iter()
                .map(|owner| owner.read(cx).saved_snapshot(cx))
                .collect(),
            cx,
        );
        self.pending_saves.push_back(self.prepare_save(
            window.window_handle(),
            file,
            Some(ProductSave::HelpHardwareReset(save)),
            cx,
        ));
        self.start_pending_save(cx);
    }

    pub(in crate::shell) fn rebase_help_hardware_reset_save(
        &self,
        save: &mut PreparedSave,
        cx: &App,
    ) -> anyhow::Result<()> {
        let Some(ProductSave::HelpHardwareReset(reset)) = &mut save.product else {
            return Ok(());
        };
        ensure!(
            self.help_hardware_reset_current(reset, cx),
            "Help reset owner or request expired before persistence"
        );
        ensure!(
            reset_fields(&reset.workspace.read(cx).snapshot(cx)) == reset_fields(&reset.original),
            "Profiles changed before Help reset was persisted"
        );
        save.file.devices = self
            .devices
            .iter()
            .map(|owner| owner.read(cx).saved_snapshot(cx))
            .collect();
        let current = save
            .file
            .devices
            .iter_mut()
            .find(|device| device.workspace_identity() == reset.identity)
            .context("Help reset persistence owner is absent")?;
        current.profiles = reset.candidate.profiles.clone();
        current.source_device_settings = reset.candidate.source_device_settings.clone();
        current.active_profile = reset.candidate.active_profile.clone();
        Ok(())
    }

    fn help_hardware_reset_current(&self, save: &LocalSave, cx: &App) -> bool {
        !self.close_requested
            && !save.canceled.load(Ordering::Acquire)
            && self
                .receiver_reset
                .get(&save.identity)
                .is_some_and(|session| session.matches(save.request, &save.canceled))
            && self.devices.contains(&save.workspace)
            && save.workspace.read(cx).identity(cx) == save.identity
            && save.workspace.read(cx).connection_revision() == save.connection_revision
            && save.workspace.read(cx).device(cx).active_profile == save.expected_profile_id
            && save.workspace.read(cx).help_reset_matches(save.request, cx)
    }

    pub(in crate::shell) fn finish_help_hardware_reset_save(
        &mut self,
        mut save: LocalSave,
        succeeded: bool,
        window: AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let weak = cx.entity().downgrade();
        cx.defer(move |cx| {
            let retained = save.workspace.clone();
            let key = save.identity.clone();
            let signal = save.canceled.clone();
            let result = window.update(cx, |_, window, cx| weak.update(cx, |shell, cx| {
                if !shell.help_hardware_reset_current(&save, cx) {
                    if shell.receiver_reset.get(&save.identity).is_some_and(|session|
                        Arc::ptr_eq(&session.canceled, &save.canceled)) {
                        shell.retire_help_reset(&save.identity);
                        if shell.devices.contains(&save.workspace) {
                            let message = if succeeded {
                                "Default profile was saved locally; the Help owner changed and hardware reset was not submitted"
                            } else {
                                "Default profile was not saved; the Help reset owner expired"
                            };
                            save.workspace.update(cx, |view, cx| view.finish_help_reset(save.request, None,
                                Some(message.into()), window, cx));
                        }
                    }
                    return;
                }
                if !succeeded {
                    shell.retire_help_reset(&save.identity);
                    save.workspace.update(cx, |view, cx| view.finish_help_reset(save.request, None,
                        Some("Default profile was not persisted; hardware reset was not submitted".into()), window, cx));
                    return;
                }
                let unchanged = reset_fields(&save.workspace.read(cx).snapshot(cx)) == reset_fields(&save.original);
                let published = unchanged && save.workspace.update(cx, |view, cx|
                    view.publish_help_hardware_reset(save.request, save.candidate.clone(), window, cx));
                if !published {
                    shell.retire_help_reset(&save.identity);
                    save.workspace.update(cx, |view, cx| view.finish_help_reset(save.request, None,
                        Some("Default profile was saved locally; current profiles changed and hardware reset was not submitted".into()), window, cx));
                    return;
                }
                save.expected_profile_id = save.candidate.active_profile.clone();
                shell.start_help_hardware_reset1398(save, window, cx);
            }));
            if !matches!(result, Ok(Ok(()))) {
                let _ = weak.update(cx, |shell, _| {
                    if shell.devices.contains(&retained) && shell.receiver_reset.get(&key)
                        .is_some_and(|session| Arc::ptr_eq(&session.canceled, &signal)) {
                        shell.retire_help_reset(&key);
                    }
                });
            }
        });
    }

    fn start_help_hardware_reset1398(
        &mut self,
        save: LocalSave,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let available = save.owner.as_ref().map_err(Clone::clone).and_then(|owner| {
            if self.help_hardware_reset_owner_current(&save.identity, owner) {
                Ok(owner.clone())
            } else {
                Err("The retained physical headset connection expired".into())
            }
        });
        let owner = match available {
            Ok(owner) => owner,
            Err(error) => {
                self.retire_help_reset(&save.identity);
                save.workspace.update(cx, |view, cx| view.finish_help_reset(save.request, None,
                    Some(format!("Default profile was saved locally; hardware reset was not submitted: {error}")), window, cx));
                return;
            }
        };
        let physical = owner.connection().identity.clone();
        let (sender, receiver) = mpsc::sync_channel(1);
        let canceled = save.canceled.clone();
        let scope = save.scope.clone();
        let device = save.candidate.clone();
        let history = self.thx_receipt_history.clone();
        let worker = thread::Builder::new()
            .name("help-headset-reset".into())
            .spawn(move || {
                let result = super::super::thx_sound::run_help_factory_reset1398(
                    scope, device, &canceled, &owner, &history,
                )
                .map_err(|error| format!("{error:#}"));
                let _ = sender.send(result);
            });
        let worker = match worker {
            Ok(worker) => worker,
            Err(error) => {
                self.retire_help_reset(&save.identity);
                save.workspace.update(cx, |view, cx| view.finish_help_reset(save.request, None,
                    Some(format!("Default profile was saved locally; hardware reset worker failed: {error}")), window, cx));
                return;
            }
        };
        let identity = save.identity.clone();
        let signal = save.canceled.clone();
        let delivery = cx.spawn_in(window, async move |shell, cx| {
            loop {
                cx.background_executor().timer(Duration::from_millis(25)).await;
                let incoming = receiver.try_recv();
                let terminal = !matches!(incoming, Err(mpsc::TryRecvError::Empty));
                let current = shell.update_in(cx, |shell, window, cx| {
                    if !shell.help_hardware_reset_current(&save, cx)
                        || !save.owner.as_ref().is_ok_and(|owner|
                            shell.help_hardware_reset_owner_current(&save.identity, owner)) {
                        if shell.receiver_reset.get(&save.identity).is_some_and(|session|
                            Arc::ptr_eq(&session.canceled, &save.canceled)) {
                            shell.retire_help_reset(&save.identity);
                            if shell.devices.contains(&save.workspace) {
                                save.workspace.update(cx, |view, cx| view.finish_help_reset(save.request, None,
                                    Some("Default profile was saved locally; the Help owner changed and hardware reset completion was not adopted".into()),
                                    window, cx));
                            }
                        }
                        return false;
                    }
                    let result: Option<Result<ThxSubmissionProgress, String>> = match incoming {
                        Ok(result) => Some(result),
                        Err(mpsc::TryRecvError::Disconnected) => Some(Err("Hardware reset worker returned no receipt".into())),
                        Err(mpsc::TryRecvError::Empty) => None,
                    };
                    if let Some(result) = result {
                        shell.retire_help_reset(&save.identity);
                        let error = match result {
                            Ok(progress) => {
                                let error = reset_error(&progress, &save.scope, &physical);
                                shell.thx_progress.insert(save.identity.clone(), progress);
                                error
                            }
                            Err(error) => Some(format!("Default profile was saved locally; hardware reset was not confirmed: {error}")),
                        };
                        if let Some(error) = &error { shell.status = error.clone(); }
                        save.workspace.update(cx, |view, cx| view.finish_help_reset(save.request, None, error, window, cx));
                        cx.notify();
                    }
                    true
                });
                if terminal || !matches!(current, Ok(true)) { break; }
            }
        });
        if let Some(session) = self
            .receiver_reset
            .get_mut(&identity)
            .filter(|session| Arc::ptr_eq(&session.canceled, &signal))
        {
            session.worker = Some(worker);
            session._delivery = Some(delivery);
        } else {
            signal.store(true, Ordering::Release);
            self.receiver_reset_cleanup.push(worker);
        }
    }
}
