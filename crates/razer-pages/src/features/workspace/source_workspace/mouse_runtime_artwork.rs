//! Real mouse runtimeData observations stay independent from local profiles.
use super::*;
use razer_device::mouse_runtime_artwork::{self as artwork, Update};
use serde_json::json;

impl SourceProductWorkspace {
    pub fn observe_mouse_runtime_artwork(
        &mut self,
        update: Option<&Update>,
        cx: &mut Context<Self>,
    ) {
        if artwork::source(self.device.product_id).is_none() {
            return;
        }
        self.device.observe_mouse_runtime_artwork(update);
        self.saved.observe_mouse_runtime_artwork(update);
        let snapshot = match update {
            Some(Update::Sidepad {
                status,
                side_panel_name,
                panel_module_id,
                ..
            }) => json!({
                "sidepadLayout": status,
                "sidePanelName": side_panel_name,
                "panelModuleID": panel_module_id,
            }),
            Some(Update::Combo {
                mat_plugged,
                mouse_connected,
                ..
            }) => {
                let mut fields = serde_json::Map::new();
                if let Some(value) = mat_plugged {
                    fields.insert("isMatPlugged".into(), json!(value));
                }
                if let Some(value) = mouse_connected {
                    fields.insert("isMouseConnected".into(), json!(value));
                }
                Value::Object(fields)
            }
            Some(Update::Diagnostic { .. }) => return,
            None => {
                if let FamilyBody::Mouse(body) = &self.body {
                    body.update(cx, |body, cx| body.reset_mouse_customize_runtime(cx));
                }
                cx.notify();
                return;
            }
        };
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| body.observe_mouse_customize(&snapshot, cx));
        }
        cx.notify();
    }
}
