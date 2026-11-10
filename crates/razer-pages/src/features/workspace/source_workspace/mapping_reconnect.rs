//! Derived software-engine documents retain the committed source revision.
//! They never initialize or acknowledge an onboard device profile.
use super::*;
use anyhow::Context as _;

#[derive(Default)]
pub(super) struct State {
    initial_macros: Option<razer_model::macro_library::MacroLibraryFile>,
    compiled: std::cell::RefCell<Option<(Device, Value)>>,
    keyboard_serial: Option<(u64, String)>,
}

impl State {
    pub(super) fn observe_keyboard_serial(&mut self, generation:u64, serial:&str) {
        self.keyboard_serial=(!serial.is_empty()).then(||(generation,serial.to_owned()));
    }
    pub(super) fn keyboard_serial(&self,generation:u64)->Option<&str> {
        self.keyboard_serial.as_ref().filter(|(observed,_)|*observed==generation).map(|(_,serial)|serial.as_str())
    }
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
    let razer_device::device_identity::IdentityLookup::Unique(identity) =
        razer_device::device_identity::lookup(device.real_product_id)
    else { return None; };
    let source_identity = razer_device::device_identity::middleware_product_identity(device.product_id)?;
    let serial = device.serial_number.to_ascii_uppercase();
    ((device.product_id == 70 || razer_device::mouse_receiver_mapping::supports_ordinary(device.product_id))
        && device.real_product_id == device.product_id
        && !identity.is_dongle && !identity.is_ble
        && source_identity.dongle_id != Some(device.real_product_id)
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
    pub(super) fn update_keyboard_selection_for_save(&mut self)->Result<(),String> {
        if !razer_device::keyboard_receiver_mapping::supports(self.device.product_id) { return Ok(()); }
        let serial=self.mapping_reconnect.keyboard_serial(self.mapping_connection_generation)
            .ok_or("Keyboard serial has not been observed for this connection")?;
        let profile=self.device.active_profile_obj().ok_or("Keyboard active profile is missing")?;
        let settings=self.device.source_device_settings.as_ref().ok_or("Keyboard source settings are absent")?;
        self.device.source_device_settings=Some(razer_device::keyboard_receiver_mapping::select_profile_for_serial(settings,serial,&profile.guid).map_err(|error|format!("{error:#}"))?);
        Ok(())
    }

    pub fn receiver_mapping_serial(&self) -> Option<&str> {
        if razer_device::keyboard_receiver_mapping::supports(self.device.product_id) {
            return self.mapping_reconnect.keyboard_serial(self.mapping_connection_generation);
        }
        (self.mapping_serial_generation == Some(self.mapping_connection_generation))
            .then(|| self.mouse_profile_observed_serial.as_deref()).flatten()
    }
    pub fn configure_shared_mouse_mapping(&mut self, owner: uuid::Uuid, physical_product: u32, cx: &mut Context<Self>) {
        if let FamilyBody::Keyboard(body)=&self.body {
            if !razer_device::keyboard_receiver_mapping::supports(self.device.product_id) { return; }
            let body=body.clone();
            body.update(cx,|body,cx|body.configure_shared_keyboard_mapping(owner,physical_product,self.device.device_container_id.clone(),self.mapping_connection_generation,cx));
            match self.mapping_reconnect.document(&self.saved) {
                Ok(Some(document))=>self.publish_saved_mapping(document,cx),
                Ok(None)=>{},
                Err(error)=>body.update(cx,|body,cx|body.release_shared_keyboard_mapping(owner,Some(format!("{error:#}")),cx)),
            }
            return;
        }
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
        if let FamilyBody::Keyboard(body)=&self.body {
            return body.read(cx).shared_keyboard_mapping_matches(&self.device.device_container_id,self.mapping_connection_generation);
        }
        matches!(&self.body, FamilyBody::Mouse(body) if body.read(cx).shared_mapping_connection_matches(
            &self.device.device_container_id, self.mapping_connection_generation))
    }

    pub fn observe_shared_mouse_mapping_publication(&mut self, owner: uuid::Uuid, revision: u64, result: Result<(), String>, cx: &mut Context<Self>) -> bool {
        match &self.body {
            FamilyBody::Mouse(body) => body.update(cx, |body, cx| body.observe_shared_mapping_publication(owner, revision, result, cx)),
            FamilyBody::Keyboard(body) => body.update(cx, |body,cx|body.observe_shared_keyboard_publication(owner,revision,result,cx)),
            _ => false,
        }
    }

    pub fn observe_shared_mouse_mapping_event(&mut self, owner: uuid::Uuid, event: Value, cx: &mut Context<Self>) -> bool {
        match &self.body {
            FamilyBody::Mouse(body) => body.update(cx, |body, cx| body.observe_shared_mapping_event(owner, event, cx)),
            FamilyBody::Keyboard(body) => body.update(cx,|body,cx|body.observe_shared_keyboard_event(owner,event,cx)),
            _ => false,
        }
    }

    pub fn release_shared_mouse_mapping(&mut self, owner: uuid::Uuid, error: Option<String>, cx: &mut Context<Self>) {
        if let FamilyBody::Keyboard(body)=&self.body {
            body.update(cx,|body,cx|body.release_shared_keyboard_mapping(owner,error,cx));
            return;
        }
        if let FamilyBody::Mouse(body) = &self.body {
            body.update(cx, |body, cx| body.release_shared_mapping(owner, error, cx));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{State, SourceProductWorkspace, FamilyBody, WorkspaceEvent, ordinary_mouse_owner};
    use razer_model::model::Device;
    use gpui_kit::{AppContext, TestAppContext, px, size};
    use gpui_kit::component::Root;
    use serde_json::json;

    #[gpui_kit::test]
    fn keyboard_peer_changes_wait_for_saved_publication_and_serial_is_connection_scoped(cx:&mut TestAppContext) {
        use gpui_kit::test::TestWindowExt;
        cx.update(gpui_kit::init);
        let mut page=None;
        let window=cx.open_window(size(px(1200.),px(900.)),|window,cx|{
            let recipe=razer_device::keyboard_current_profile::local::initial_template(600,0,"Desktop").unwrap();
            let mut profile=serde_json::to_value(&recipe.profiles[0]).unwrap();
            profile["guid"]="saved-profile".into();
            let mut device=razer_model::product_preview::registered_preview(600).unwrap();
            device.real_product_id=625;
            device.device_container_id="{11111111-2222-3333-4444-555555555555}".into();
            device.serial_number="ACTUAL-SERIAL".into();
            device.dashboard.connection_observation=Some(razer_model::model::DeviceConnectionObservation::ReceiverPeer(1));
            device.active_profile="saved-profile".into();
            device.profiles=vec![razer_model::model::Profile{id:"saved-profile".into(),guid:"saved-profile".into(),name:"Saved".into(),source_settings:Some(profile),settings:None,dpi_stages:None}];
            device.source_device_settings=Some(serde_json::to_value(recipe.device).unwrap());
            let view=cx.new(|cx|SourceProductWorkspace::new(device,window,cx));
            page=Some(view.clone());
            Root::new(view,window,cx)
        });
        let page=page.unwrap();
        let requests=cx.new(|_|(Vec::<crate::features::ReceiverMappingPublication>::new(),0usize));
        let _subscription=requests.update(cx,|_,cx|cx.subscribe(&page,|requests,_,event:&WorkspaceEvent,_|match event {
            WorkspaceEvent::ReceiverMappingPublication(request)=>requests.0.push(request.clone()),
            WorkspaceEvent::KeyboardLegacyProfileRequested=>requests.1+=1,
            _=>{},
        }));
        let owner=uuid::Uuid::new_v4();
        page.update(cx,|page,cx|{
            page.mapping_reconnect.initialize_macros(&Default::default());
            assert!(page.receiver_mapping_serial().is_none());
            page.observe_keyboard_serial("ACTUAL-SERIAL",cx);
            assert_eq!(page.receiver_mapping_serial(),Some("ACTUAL-SERIAL"));
            page.configure_shared_mouse_mapping(owner,197,cx);
        });
        cx.run_until_parked();
        let initial=requests.update(cx,|requests,_|{assert_eq!(requests.0.len(),1);requests.0.remove(0)});
        page.update(cx,|page,cx|{assert!(page.observe_shared_mouse_mapping_publication(owner,initial.revision(),Ok(()),cx));});
        cx.update_window(window.into(),|_,window,cx|{
            page.update(cx,|page,cx|{
                let FamilyBody::Keyboard(body)=&page.body else {panic!("real keyboard body required")};
                body.update(cx,|body,cx|{
                    let mut draft=body.snapshot();
                    draft["mappings"]=json!([{"inputID":"KEY_A","inputType":"KeyInput","outputType":"keyboardGroup","keyboardGroup":{"key":"KEY_B"}}]);
                    body.restore(Some(&draft),window,cx);
                    cx.emit(crate::features::keyboard_products::KeyboardProductChanged);
                });
            });
        }).unwrap();
        cx.run_until_parked();
        requests.read_with(cx,|requests,_|{assert_eq!(requests.1,1);assert!(requests.0.is_empty(),"a draft/save request is not an accepted software publication");});
        page.update(cx,|page,cx|{
            page.observe_connection(None,cx);
            assert!(page.receiver_mapping_serial().is_none());
            assert!(!page.observe_shared_mouse_mapping_publication(owner,initial.revision(),Ok(()),cx));
            assert!(!page.observe_shared_mouse_mapping_event(owner,json!({"jsonEvent":{"type":"ignored"}}),cx));
        });
    }

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
            assert_eq!(ordinary_mouse_owner(&device).as_deref(),
                (!matches!(pid,148|185|224)).then_some("current-container"));
            device.real_product_id=pid+1;
            assert!(ordinary_mouse_owner(&device).is_none());
            device.real_product_id=pid;
            device.observe_connection(Some(DeviceConnectionObservation::ReceiverPeer(1)));
            assert!(ordinary_mouse_owner(&device).is_none());
        }
    }
}
