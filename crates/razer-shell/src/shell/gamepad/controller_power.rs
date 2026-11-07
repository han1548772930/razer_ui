//! The retained controller session owns the source's display/idle brightness
//! handlers independently of whichever product page is currently mounted.
use super::*;
use razer_device::controller_lighting::power_policy::{Brightness, Settings};
use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Configuration {
    pub(super) profile_id: String,
    pub(super) settings: Settings,
    pub(super) brightness: Brightness,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceProfile {
    switch_off_lighting: Settings,
    brightness: Brightness,
}
#[derive(Deserialize)]
struct SourceSettings {
    profile: SourceProfile,
}
impl Configuration {
    fn read(device: &Device) -> anyhow::Result<Self> {
        let profile = device
            .active_profile_obj()
            .ok_or_else(|| anyhow::anyhow!("Controller active profile is missing"))?;
        let source: SourceSettings = serde_json::from_value(
            profile
                .source_settings
                .clone()
                .ok_or_else(|| anyhow::anyhow!("Controller source profile is missing"))?,
        )?;
        source.profile.switch_off_lighting.validate()?;
        source.profile.brightness.validate()?;
        Ok(Self {
            profile_id: profile.id.clone(),
            settings: source.profile.switch_off_lighting,
            brightness: source.profile.brightness,
        })
    }
}
impl AppShell {
    pub(super) fn refresh_controller_power_configuration(&mut self, cx: &mut Context<Self>) {
        for (identity, session) in &self.controller_lighting_owners {
            let Some(workspace) = self
                .devices
                .iter()
                .find(|workspace| workspace.read(cx).identity(cx) == *identity)
            else {
                continue;
            };
            let saved = workspace.read(cx).saved_snapshot(cx);
            match Configuration::read(&saved)
                .and_then(|configuration| session.owner().configure_power(configuration))
            {
                Ok(()) => {}
                Err(error) => self.status = format!("手柄熄灯策略未读取：{error:#}"),
            }
        }
    }
    pub(super) fn start_controller_power_monitor(&mut self, cx: &mut Context<Self>) {
        self.ensure_power_observation(cx);
    }
}
