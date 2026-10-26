//! Product sessions stop before the shell waits for service cleanup.
use super::*;

impl SourceProductWorkspace {
    pub(in crate::features) fn prepare_exit(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.exiting {
            return;
        }
        self.exiting = true;
        self.invalidate_controller_initialization(cx);
        self.invalidate_keyboard_profile_inventory(false);
        self.receiver_profile_selection.borrow_mut().prepare_exit();
        if matches!(self.device.product_id, 164 | 241)
            && let FamilyBody::Controls(body) = &self.body
        {
            body.update(cx, |body, cx| body.prepare_receiver_exit(cx));
        }
        if matches!(self.device.product_id, 3594..=3596)
            && let FamilyBody::Controls(body) = &self.body
        {
            body.update(cx, |body, cx| body.prepare_camera_exit(cx));
        }
        self.dismiss_profile_dialog(window, cx);
        self.set_active(false, window, cx);
        match &self.body {
            FamilyBody::Mouse(body) => body.update(cx, |body, cx| {
                body.dismiss_editors(window, cx);
            }),
            FamilyBody::Keyboard(body) => body.update(cx, |body, cx| {
                body.dismiss_editors(window, cx);
            }),
            FamilyBody::Audio(body) => body.update(cx, |body, cx| {
                body.prepare_exit(cx);
            }),
            FamilyBody::Gamepad(body) => body.update(cx, |body, cx| {
                body.prepare_controller_sleep_exit(cx);
            }),
            _ => {}
        }
    }

    pub(in crate::features) fn finish_exit(&mut self, cx: &mut Context<Self>) {
        match &self.body {
            FamilyBody::Mouse(body) => body.update(cx, |body, _| body.finish_mapping_service()),
            FamilyBody::Keyboard(body) => body.update(cx, |body, _| body.finish_mapping_service()),
            _ => {}
        }
    }

    pub(in crate::features) fn cancel_exit(
        &mut self,
        active: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.exiting {
            self.set_active(active, window, cx);
            return;
        }
        self.exiting = false;
        self.controller_initialization.resume_after_canceled_exit();
        self.receiver_profile_selection.borrow_mut().cancel_exit();
        if matches!(self.device.product_id, 164 | 241)
            && let FamilyBody::Controls(body) = &self.body
        {
            body.update(cx, |body, _| body.cancel_receiver_exit());
        }
        match &self.body {
            FamilyBody::Mouse(body) => body.update(cx, |body, _| {
                body.cancel_calibration_exit();
                body.cancel_mapping_service_exit();
            }),
            FamilyBody::Keyboard(body) => body.update(cx, |body, _| {
                body.set_legacy_mapping_connection(None);
            }),
            FamilyBody::Gamepad(body) => body.update(cx, |body, cx| {
                body.resume_controller_sleep_exit(
                    self.device.dashboard.connection_observation.is_some(),
                    cx,
                );
            }),
            _ => {}
        }
        self.set_active(active, window, cx);
        self.sync_receiver_profile_initialization(cx);
        if razer_device::mapping_document::MappingProduct::for_product_id(self.device.product_id)
            .is_some_and(|product| product.uses_retained_mapping_owner())
        {
            let document = match self.mapping_reconnect.document(&self.saved) {
                Ok(document) => document,
                Err(error) => {
                    eprintln!("[saved mapping restart] {error:#}");
                    None
                }
            };
            if let Some(document) = document {
                self.publish_legacy_mapping(document, cx);
            }
        }
    }
}
