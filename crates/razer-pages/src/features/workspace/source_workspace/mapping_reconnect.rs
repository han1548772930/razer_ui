//! Derived software-engine documents retain the committed source revision.
//! They never initialize or acknowledge an onboard device profile.
use super::*;
use anyhow::Context as _;

#[derive(Default)]
pub(super) struct State {
    initial_macros: Option<razer_model::macro_library::MacroLibraryFile>,
    compiled: std::cell::RefCell<Option<(Device, Value)>>,
}

impl State {
    pub(super) fn initialize_macros(
        &mut self,
        file: &razer_model::macro_library::MacroLibraryFile,
    ) {
        // Later successful macro saves carry their own replacement catalogs in
        // the durable compiled document. Editor drafts cannot change bootstrap.
        if self.initial_macros.is_none() {
            self.initial_macros = Some(file.clone());
        }
    }

    pub(super) fn document(&self, saved: &Device) -> anyhow::Result<Option<Value>> {
        let Some(product) =
            razer_device::mapping_document::MappingProduct::for_product_id(saved.product_id)
        else {
            return Ok(None);
        };
        if let Some(document) = saved
            .source_device_settings
            .as_ref()
            .and_then(|settings| settings.get(product.storage_field()))
        {
            return Ok(Some(document.clone()));
        }
        let Some(macros) = &self.initial_macros else {
            return Ok(None);
        };
        if let Some((source, document)) = self.compiled.borrow().as_ref()
            && razer_model::mapping::matches_saved_revision(source, saved)
        {
            return Ok(Some(document.clone()));
        }
        let document = razer_model::mapping::prepare_document(
            saved,
            macros,
            (0..256).map(|_| uuid::Uuid::new_v4().to_string()),
        )?
        .context("Saved mapping product omitted its compiled document")?;
        *self.compiled.borrow_mut() = Some((saved.clone(), document.clone()));
        Ok(Some(document))
    }
}

pub(super) fn ordinary_mouse_owner(device: &Device) -> Option<String> {
    use razer_model::model::DeviceConnectionObservation;
    let serial = device.serial_number.to_ascii_uppercase();
    ((device.product_id == 70 || razer_device::mouse_receiver_mapping::supports_ordinary(device.product_id))
        && device.real_product_id == device.product_id
        && !device.device_container_id.is_empty()
        && !serial.starts_with("PREVIEW-")
        && !serial.starts_with("DEMO-")
        && matches!(
            device.dashboard.connection_observation,
            Some(DeviceConnectionObservation::UsbPresent | DeviceConnectionObservation::HidPresent)
        ))
    .then(|| device.device_container_id.clone())
}

impl SourceProductWorkspace {
    pub fn receiver_mapping_serial(&self) -> Option<&str> {
        (self.mapping_serial_generation == Some(self.mapping_connection_generation))
            .then(|| self.mouse_profile_observed_serial.as_deref()).flatten()
    }
    pub fn configure_shared_mouse_mapping(&mut self, owner: uuid::Uuid, physical_product: u32, cx: &mut Context<Self>) {
        if let FamilyBody::Mouse(body) = &self.body {
            let body = body.clone();
            let container = self.device.device_container_id.clone();
            let generation = self.mapping_connection_generation;
            body.update(cx, |body, _| body.configure_shared_mapping(owner, physical_product, container, generation));
            match self.mapping_reconnect.document(&self.saved) {
                Ok(Some(document)) => self.publish_saved_mapping(document, cx),
                Ok(None) => {},
                Err(error) => body.update(cx, |body, cx| body.release_shared_mapping(owner, Some(format!("{error:#}")), cx)),
            }
        }
    }

    pub(super) fn shared_mouse_mapping_current(&self, cx: &App) -> bool {
        matches!(&self.body, FamilyBody::Mouse(body) if body.read(cx).shared_mapping_connection_matches(
            &self.device.device_container_id, self.mapping_connection_generation))
    }

    pub fn observe_shared_mouse_mapping_publication(&mut self, owner: uuid::Uuid, revision: u64, result: Result<(), String>, cx: &mut Context<Self>) -> bool {
        match &self.body {
            FamilyBody::Mouse(body) => body.update(cx, |body, cx| body.observe_shared_mapping_publication(owner, revision, result, cx)),
            _ => false,
        }
    }

    pub fn observe_shared_mouse_mapping_event(&mut self, owner: uuid::Uuid, event: Value, cx: &mut Context<Self>) -> bool {
        match &self.body {
            FamilyBody::Mouse(body) => body.update(cx, |body, cx| body.observe_shared_mapping_event(owner, event, cx)),
            _ => false,
        }
    }

    pub fn release_shared_mouse_mapping(&mut self, owner: uuid::Uuid, error: Option<String>, cx: &mut Context<Self>) {
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| body.release_shared_mapping(owner, error, cx));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn saved_device() -> Device {
        let mut saved = razer_model::product_preview::registered_preview(158).unwrap();
        saved.active_profile = "saved-profile".into();
        saved.profiles = vec![razer_model::model::Profile {
            id: saved.active_profile.clone(), guid: saved.active_profile.clone(), name: "Saved".into(),
            source_settings: Some(json!({"mappings":[],"dpi":{"x":1600,"y":1700}})),
            settings: None, dpi_stages: None,
        }];
        saved
    }

    #[::core::prelude::v1::test]
    fn reconnect_compiles_only_committed_profiles_and_reuses_the_derived_revision() {
        let saved = saved_device();
        let mut draft = saved.clone();
        draft.profiles[0].source_settings.as_mut().unwrap()["dpi"] = json!({"x":3000,"y":4000});
        let mut state = State::default();
        assert!(state.document(&saved).unwrap().is_none());
        state.initialize_macros(&Default::default());
        let initial = state.document(&saved).unwrap().unwrap();
        assert_eq!(initial["profiles"][0]["dpi"], json!({"x":1600,"y":1700}));
        assert_eq!(state.document(&saved).unwrap(), Some(initial.clone()));
        assert!(saved.source_device_settings.is_none());
        assert_eq!(
            draft.profiles[0].source_settings.as_ref().unwrap()["dpi"],
            json!({"x":3000,"y":4000})
        );
        let mut accepted = saved.clone();
        let mut durable = initial;
        durable["version"] = json!(27);
        durable["macros"] = json!([{"guid":"persisted-macro"}]);
        let mut root = json!({});
        root[razer_device::mouse_receiver_mapping::STORAGE_FIELD] = durable;
        accepted.source_device_settings = Some(root);
        assert_eq!(
            state.document(&accepted).unwrap(),
            accepted
                .source_device_settings
                .as_ref()
                .map(|root| root[razer_device::mouse_receiver_mapping::STORAGE_FIELD].clone())
        );
    }

    #[::core::prelude::v1::test]
    fn reconnect_keeps_missing_or_malformed_source_as_a_failure() {
        let mut state = State::default();
        state.initialize_macros(&Default::default());
        let mut saved = saved_device();
        saved.profiles[0].source_settings = None;
        assert!(state.document(&saved).is_err());
        saved.profiles[0].source_settings = Some(json!([]));
        assert!(state.document(&saved).is_err());
        saved.profiles[0].source_settings = Some(json!({"dpi":{"x":800,"y":800}}));
        assert!(state.document(&saved).is_err());
    }

    #[::core::prelude::v1::test]
    fn ordinary_owner_requires_actual_current_source_identity() {
        use razer_model::model::DeviceConnectionObservation;
        let mut device = saved_device();
        device.observe_connection(Some(DeviceConnectionObservation::HidPresent));
        assert!(ordinary_mouse_owner(&device).is_none());
        device.serial_number = "observed-serial".into();
        device.device_container_id = "observed-container".into();
        assert_eq!(
            ordinary_mouse_owner(&device).as_deref(),
            Some("observed-container")
        );
        device.real_product_id = 159;
        assert!(ordinary_mouse_owner(&device).is_none());
        device.real_product_id = 158;
        device.observe_connection(Some(DeviceConnectionObservation::ReceiverPeer(1)));
        assert!(ordinary_mouse_owner(&device).is_none());
        device.observe_connection(None);
        assert!(ordinary_mouse_owner(&device).is_none());
    }

    #[::core::prelude::v1::test]
    fn current_mouse_batch_admits_only_its_own_observed_physical_product() {
        use razer_model::model::DeviceConnectionObservation;
        for pid in [89,96,104,112,115,140,145,148,161,178,185,190,192,194,196,222,224,229,239] {
            let mut device=razer_model::product_preview::registered_preview(pid).unwrap();
            device.serial_number="observed".into();
            device.device_container_id="current-container".into();
            device.real_product_id=pid;
            device.observe_connection(Some(DeviceConnectionObservation::HidPresent));
            assert_eq!(ordinary_mouse_owner(&device).as_deref(),Some("current-container"));
            device.real_product_id=pid+1;
            assert!(ordinary_mouse_owner(&device).is_none());
            device.real_product_id=pid;
            device.observe_connection(Some(DeviceConnectionObservation::ReceiverPeer(1)));
            assert!(ordinary_mouse_owner(&device).is_none());
        }
    }
}
