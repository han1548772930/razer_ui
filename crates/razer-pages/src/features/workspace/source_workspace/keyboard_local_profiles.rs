//! Serial-specific local documents are independent of device observations.
use super::*;
use razer_storage::host_keyboard_profiles::LocalProfiles;

pub(super) fn active_keyboard_factory_profile(device: &Device) -> bool {
    device
        .profiles
        .iter()
        .find(|profile| profile.id == device.active_profile)
        .is_some_and(|profile| {
            super::super::keyboard_products::is_factory_profile(device.product_id, &profile.guid)
        })
}

impl SourceProductWorkspace {
    pub fn observe_keyboard_serial(&mut self, serial: &str, cx: &mut Context<Self>) {
        self.device.observe_keyboard_serial(serial);
        self.saved.observe_keyboard_serial(serial);
        cx.notify();
    }

    pub fn accept_keyboard_local_profiles(
        &mut self,
        baseline: &Device,
        source: &LocalProfiles,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<bool, String> {
        if !matches!(&self.body, FamilyBody::Keyboard(_)) {
            return Err("Local keyboard profiles require a keyboard workspace".into());
        }
        let changed =
            razer_storage::host_keyboard_profiles::import(&mut self.device, baseline, source)
                .map_err(|error| format!("{error:#}"))?;
        self.refresh_profile_choices(window, cx);
        self.restore_active(window, cx);
        cx.notify();
        Ok(changed)
    }
}
