use super::*;
use razer_device::keyboard_current_profile::LightingOff;
use razer_device::keyboard_settings::{
    BrightnessSubmission,
    power::{Event, Policy, Recorder},
};
use razer_discovery::{direct, discovery::ObservedDevice};
use razer_platform::power_idle;
use razer_storage::keyboard_lighting::Configuration;
use std::collections::BTreeMap;

struct Owner {
    workspace: WeakEntity<ProductWorkspace>,
    profile: String,
    route: ObservedDevice,
    saved: Configuration,
    policy: Policy,
}

#[derive(Default)]
pub(super) struct State {
    owners: BTreeMap<String, Owner>,
    recorder_events: Vec<Event>,
}

impl State {
    pub(super) fn has_pending_recorder(&self) -> bool {
        !self.recorder_events.is_empty()
    }
    pub(super) fn record(
        &mut self,
        notification: razer_storage::macro_recorder::RecorderNotification,
    ) {
        use razer_storage::macro_recorder::{RecorderMode, RecorderNotification};
        let mode = |mode| match mode {
            RecorderMode::Software => Recorder::Software,
            RecorderMode::Otfm => Recorder::Otfm,
        };
        self.recorder_events.push(match notification {
            RecorderNotification::Started(recorder) => Event::RecorderStarted(mode(recorder)),
            RecorderNotification::Stopped(recorder) => Event::RecorderStopped(mode(recorder)),
        });
    }
}

impl AppShell {
    pub(super) fn sync_keyboard_power_owners(&mut self, cx: &mut Context<Self>) {
        self.refresh_macro_recorder_storage();
        let recorder = if self.receiver_macro_exclusive(cx) {
            Recorder::Unknown
        } else {
            Recorder::Free
        };
        let mut current = BTreeMap::new();
        let mut outputs = Vec::new();
        for workspace in &self.devices {
            let device = workspace.read(cx).device(cx);
            if !razer_storage::keyboard_lighting::supported(device.product_id) {
                continue;
            }
            let routes = self
                .device_observations
                .iter()
                .filter(|route| {
                    route.matches(device)
                        && direct::keyboard_brightness_available(route)
                        && razer_device::keyboard_settings::power::supports_transport(
                            route.product_id(),
                            route.transport()
                                == Some(razer_discovery::discovery::ObservedTransport::Ble),
                        )
                })
                .collect::<Vec<_>>();
            let [route] = routes.as_slice() else {
                continue;
            };
            let saved = workspace.read(cx).saved_snapshot(cx);
            let Ok(configuration) =
                razer_storage::keyboard_lighting::read(&saved, &device.active_profile)
            else {
                continue;
            };
            let identity = workspace.read(cx).identity(cx);
            let owner = self
                .keyboard_power
                .owners
                .remove(&identity)
                .filter(|owner| {
                    owner
                        .workspace
                        .upgrade()
                        .is_some_and(|bound| bound == *workspace)
                        && owner.profile == device.active_profile
                        && device_discovery::retained_connection(
                            std::slice::from_ref(&owner.route),
                            &self.device_observations,
                            device,
                        )
                });
            let mut owner = if let Some(owner) = owner {
                owner
            } else {
                let Ok(policy) = Policy::new(
                    configuration.switch_off_lighting().clone(),
                    configuration.brightness().clone(),
                    recorder,
                ) else {
                    continue;
                };
                Owner {
                    workspace: workspace.downgrade(),
                    profile: device.active_profile.clone(),
                    route: (**route).clone(),
                    saved: configuration.clone(),
                    policy,
                }
            };
            if recorder == Recorder::Unknown && owner.policy.recorder() == Recorder::Free {
                owner
                    .policy
                    .event(Event::RecorderStarted(Recorder::Unknown));
            }
            if owner.saved.brightness() != configuration.brightness() {
                if let Err(error) = owner
                    .policy
                    .restore_local_brightness(configuration.brightness().clone())
                {
                    self.status = format!("键盘后台亮度配置无效：{error:#}");
                    continue;
                }
            }
            if owner.saved.switch_off_lighting() != configuration.switch_off_lighting() {
                match owner
                    .policy
                    .configure(configuration.switch_off_lighting().clone())
                {
                    Ok(Some(submission)) => outputs.push((workspace.clone(), Some(submission))),
                    Ok(None) => {}
                    Err(error) => {
                        self.status = format!("键盘熄灯配置无效：{error:#}");
                        continue;
                    }
                }
            }
            owner.saved = configuration;
            current.insert(identity, owner);
        }
        self.keyboard_power.owners = current;
        let recorder_events = std::mem::take(&mut self.keyboard_power.recorder_events);
        for event in recorder_events {
            for owner in self.keyboard_power.owners.values_mut() {
                if let Some(submission) = owner.policy.event(event) {
                    if let Some(workspace) = owner.workspace.upgrade() {
                        outputs.push((workspace, Some(submission)));
                    }
                }
            }
        }
        for (workspace, submission) in outputs {
            workspace.update(cx, |workspace, cx| {
                workspace.request_keyboard_runtime_brightness(submission, cx)
            });
        }
    }

    pub(super) fn keyboard_power_submitted(
        &mut self,
        workspace: &Entity<ProductWorkspace>,
        submission: BrightnessSubmission,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<bool> {
        self.sync_keyboard_power_owners(cx);
        let identity = workspace.read(cx).identity(cx);
        if let Some(owner) = self.keyboard_power.owners.get_mut(&identity) {
            return owner.policy.submitted(submission);
        }
        Ok(!self.receiver_macro_exclusive(cx))
    }

    pub(super) fn keyboard_switch_off_committed(
        &mut self,
        workspace: &Entity<ProductWorkspace>,
        setting: &LightingOff,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<()> {
        let saved = workspace.read(cx).saved_snapshot(cx);
        let profile = workspace.read(cx).device(cx).active_profile.clone();
        let configuration = razer_storage::keyboard_lighting::read(&saved, &profile)?;
        razer_device::keyboard_settings::power::validate(setting, configuration.brightness())?;
        self.sync_keyboard_power_owners(cx);
        anyhow::ensure!(
            self.keyboard_power
                .owners
                .contains_key(&workspace.read(cx).identity(cx)),
            "键盘熄灯运行时未取得当前配置和连接"
        );
        self.ensure_power_observation(cx);
        Ok(())
    }

    pub(super) fn keyboard_power_event(
        &mut self,
        event: &power_idle::Event,
        cx: &mut Context<Self>,
    ) {
        self.sync_keyboard_power_owners(cx);
        let mut outputs = Vec::new();
        for owner in self.keyboard_power.owners.values_mut() {
            let Some(workspace) = owner.workspace.upgrade() else {
                continue;
            };
            let mapped = match event {
                power_idle::Event::DisplayOff => Some(Event::DisplayOff),
                power_idle::Event::DisplayOn => Some(Event::DisplayOn),
                power_idle::Event::DisplayDimmed => Some(Event::DisplayDimmed),
                power_idle::Event::Idle { milliseconds } => Some(Event::Idle {
                    milliseconds: *milliseconds,
                }),
                _ => None,
            };
            let display_subscribed = owner.policy.display_subscribed();
            if let Some(event) = mapped {
                if let Some(submission) = owner.policy.event(event) {
                    outputs.push((workspace.clone(), Some(submission)));
                }
            }
            if razer_device::keyboard_settings::power::supports_resume_probe(
                owner.route.product_id(),
            ) && (matches!(
                event,
                power_idle::Event::Resume | power_idle::Event::ScreenUnlocked
            ) || matches!(event, power_idle::Event::DisplayOn) && display_subscribed)
            {
                outputs.push((workspace, None));
            }
        }
        for (workspace, submission) in outputs {
            workspace.update(cx, |workspace, cx| {
                workspace.request_keyboard_runtime_brightness(submission, cx)
            });
        }
    }
}
