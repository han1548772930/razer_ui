use super::*;
use crate::features::HyperpollingIndicatorRequested;

pub(super) fn accept_saved_version(device: &mut Device, previous: &Device, snapshot: &Device) {
    if device.product_id != 179 || device.real_product_id != 179 {
        return;
    }
    let version = |device: &Device| {
        device
            .source_device_settings
            .as_ref()
            .and_then(|settings| settings.get("version"))
            .cloned()
    };
    if version(device) != version(previous) {
        return;
    }
    let Some(saved_version) = version(snapshot) else {
        return;
    };
    if let Some(root) = device
        .source_device_settings
        .get_or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
    {
        root.insert("version".into(), saved_version);
    }
}

impl SourceProductWorkspace {
    pub fn begin_hyperpolling_connection(&mut self, epoch: Option<u64>, cx: &mut Context<Self>) {
        if let FamilyBody::Controls(body) = &self.body {
            body.update(cx, |body, cx| body.begin_hyperpolling_connection(epoch, cx));
        }
    }
    pub fn hyperpolling_request_matches(
        &self,
        request: &HyperpollingIndicatorRequested,
        cx: &App,
    ) -> bool {
        matches!(&self.body, FamilyBody::Controls(body) if body.read(cx).hyperpolling_request_matches(request))
    }
    pub fn initialize_hyperpolling_indicator(
        &mut self,
        epoch: u64,
        status: u8,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Controls(body) = &self.body {
            body.update(cx, |body, cx| {
                body.initialize_hyperpolling_indicator(epoch, status, cx)
            });
        }
    }
    pub fn publish_hyperpolling_indicator(
        &mut self,
        request: &HyperpollingIndicatorRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let FamilyBody::Controls(body) = &self.body else {
            return false;
        };
        if !body.update(cx, |body, cx| {
            body.publish_hyperpolling_indicator(request, window, cx)
        }) {
            return false;
        }
        // This mutation records only the local profile saved by the queue.
        // Transport success is retained separately in the source control owner.
        let Some(profile) = self
            .device
            .profiles
            .iter_mut()
            .find(|profile| profile.id == self.device.active_profile)
        else {
            return false;
        };
        let settings = profile
            .source_settings
            .get_or_insert_with(|| serde_json::json!({}));
        let Some(settings) = settings.as_object_mut() else {
            return false;
        };
        settings.insert(
            "indicatorLedStatus".into(),
            serde_json::json!(request.status()),
        );
        if let Some(runtime) = settings.get_mut("runtime").and_then(Value::as_object_mut) {
            runtime.insert(
                "indicatorLedStatus".into(),
                serde_json::json!(request.status()),
            );
        }
        cx.notify();
        true
    }
    pub fn observe_hyperpolling_identity(&mut self, serial: &str, cx: &mut Context<Self>) {
        if self.device.product_id != 179 {
            return;
        }
        self.device.serial_number = serial.into();
        self.saved.serial_number = serial.into();
        // Only edition zero is present in the mounted current product table.
        // Observed raw metadata stays with the runtime's diagnostic receipt.
        self.device.edition_id = 0;
        self.saved.edition_id = 0;
        self.device.layout_id = 0;
        self.saved.layout_id = 0;
        cx.notify();
    }
    pub fn finish_hyperpolling_indicator(
        &mut self,
        request: &HyperpollingIndicatorRequested,
        diagnostic: Option<razer_device::hyperpolling_receiver::Submission>,
        cx: &mut Context<Self>,
    ) {
        if let FamilyBody::Controls(body) = &self.body {
            body.update(cx, |body, cx| {
                body.finish_hyperpolling_indicator(request, diagnostic, cx)
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn hyperpolling_runtime_saved_version_preserves_live_edits_and_conflicts() {
        let mut previous = razer_model::product_preview::registered_preview(179).unwrap();
        previous.source_device_settings = Some(serde_json::json!({"version":3,"extension":1}));
        let mut snapshot = previous.clone();
        snapshot.source_device_settings.as_mut().unwrap()["version"] = 4.into();
        let mut live = previous.clone();
        live.source_device_settings.as_mut().unwrap()["extension"] = 9.into();
        accept_saved_version(&mut live, &previous, &snapshot);
        assert_eq!(live.source_device_settings.as_ref().unwrap()["version"], 4);
        assert_eq!(
            live.source_device_settings.as_ref().unwrap()["extension"],
            9
        );
        live.source_device_settings.as_mut().unwrap()["version"] = 8.into();
        accept_saved_version(&mut live, &previous, &snapshot);
        assert_eq!(live.source_device_settings.as_ref().unwrap()["version"], 8);
        previous.source_device_settings = None;
        live.source_device_settings = None;
        accept_saved_version(&mut live, &previous, &snapshot);
        assert_eq!(live.source_device_settings.as_ref().unwrap()["version"], 4);
    }
}
