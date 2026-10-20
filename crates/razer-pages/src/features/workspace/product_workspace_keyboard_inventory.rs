//! The shell publishes physical reads into the source page's observation owner.
use super::*;
use std::sync::{Arc, atomic::AtomicBool};

impl ProductWorkspace {
    pub fn begin_keyboard_profile_inventory(
        &mut self,
        id: uuid::Uuid,
        canceled: Arc<AtomicBool>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<Option<razer_device::keyboard_onboard_profiles::ReadOptions>, String> {
        match &self.body {
            Body::Source(body) => body.update(cx, |body, cx| {
                body.begin_keyboard_profile_inventory(id, canceled, window, cx)
            }),
            Body::Existing(_) => Err("Keyboard inventory requires a source workspace".into()),
        }
    }

    pub fn finish_keyboard_profile_inventory(
        &mut self,
        id: uuid::Uuid,
        result: Result<razer_ipc::keyboard_profile_inventory::Observation, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Body::Source(body) = &self.body {
            body.update(cx, |body, cx| {
                body.finish_keyboard_profile_inventory(id, result, window, cx)
            });
        }
    }

    pub fn finish_keyboard_onboard_profiles(
        &mut self,
        id: uuid::Uuid,
        result: Result<razer_ipc::keyboard_onboard_profiles::Observation, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Body::Source(body) = &self.body {
            body.update(cx, |body, cx| {
                body.finish_keyboard_onboard_profiles(id, result, window, cx)
            });
        }
    }

    pub fn finish_keyboard_profile_slots(
        &mut self,
        id: uuid::Uuid,
        result: Result<razer_ipc::keyboard_profile_inventory::SlotsObservation, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Body::Source(body) = &self.body {
            body.update(cx, |body, cx| {
                body.finish_keyboard_profile_slots(id, result, window, cx)
            });
        }
    }

    pub fn clear_keyboard_profile_inventory(
        &mut self,
        connected: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Body::Source(body) = &self.body {
            body.update(cx, |body, cx| {
                body.clear_keyboard_profile_inventory(connected, window, cx)
            });
        }
    }
}
