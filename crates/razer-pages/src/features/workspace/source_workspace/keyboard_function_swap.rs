//! Explicit function-key edits wait for their HID setter before persistence.
use super::*;
use razer_device::keyboard_function_swap::{FunctionKeyPrimary, FunctionKeySwapWriteReport};

#[derive(Clone)]
pub struct KeyboardFunctionSwapRequested {
    id: uuid::Uuid,
    owner: Option<uuid::Uuid>,
    generation: u64,
    primary: FunctionKeyPrimary,
    snapshot: Device,
}

#[cfg(test)]
mod tests {
    use super::{SourceProductWorkspace,FamilyBody,WorkspaceEvent,KeyboardFunctionSwapRequested,FunctionKeyPrimary,FunctionKeySwapWriteReport,primary_value};
    use gpui_kit::{AppContext,TestAppContext,px,size};
    use gpui_kit::component::Root;
    use gpui_kit::test::TestWindowExt;
    use serde_json::json;

    fn ack(primary:FunctionKeyPrimary)->FunctionKeySwapWriteReport {
        serde_json::from_value(json!({"requested_primary":primary,"attempted":true,"send_attempts":1,
            "acknowledgment_data":[165],"observed":null,"verified":false})).unwrap()
    }

    #[gpui_kit::test]
    fn function_swap_keeps_optimistic_field_separate_and_saves_only_after_current_set_ack(cx:&mut TestAppContext) {
        cx.update(gpui_kit::init);
        let mut page=None;
        let window=cx.open_window(size(px(1200.),px(900.)),|window,cx|{
            let recipe=razer_device::keyboard_current_profile::local::initial_template(717,0,"Desktop").unwrap();
            let mut profile=serde_json::to_value(&recipe.profiles[0]).unwrap();profile["guid"]="profile".into();
            let mut device=razer_model::product_preview::registered_preview(717).unwrap();
            device.device_container_id="{11111111-2222-3333-4444-555555555555}".into();device.serial_number="ACTUAL".into();device.real_product_id=730;
            device.dashboard.connection_observation=Some(razer_model::model::DeviceConnectionObservation::ReceiverPeer(1));
            device.active_profile="profile".into();
            device.profiles=vec![razer_model::model::Profile{id:"profile".into(),guid:"profile".into(),name:"Saved".into(),source_settings:Some(profile),settings:None,dpi_stages:None}];
            device.source_device_settings=Some(serde_json::to_value(recipe.device).unwrap());
            let view=cx.new(|cx|SourceProductWorkspace::new(device,window,cx));page=Some(view.clone());Root::new(view,window,cx)
        });
        let page=page.unwrap();
        let events=cx.new(|_|(Vec::<KeyboardFunctionSwapRequested>::new(),0usize));
        let _subscription=events.update(cx,|_,cx|cx.subscribe(&page,|events,_,event:&WorkspaceEvent,_|match event {
            WorkspaceEvent::KeyboardFunctionSwapRequested(request)=>events.0.push(request.clone()),
            WorkspaceEvent::KeyboardLegacyProfileRequested=>events.1+=1,
            _=>{},
        }));
        page.update(cx,|page,cx|{page.observe_keyboard_serial("ACTUAL",cx);page.configure_shared_mouse_mapping(uuid::Uuid::new_v4(),197,cx);});
        cx.run_until_parked();
        events.read_with(cx,|events,_|assert!(events.0.is_empty(),"restoration must not write a profile default"));
        let edit=|cx:&mut TestAppContext,primary:&str,mapped:Option<&str>|{
            cx.update_window(window.into(),|_,window,cx|page.update(cx,|page,cx|{
                let FamilyBody::Keyboard(body)=&page.body else {panic!("keyboard body required")};
                body.update(cx,|body,cx|{
                    let mut draft=body.snapshot();draft["functionKeyPrimary"]=primary.into();
                    if let Some(key)=mapped { draft["mappings"]=json!([{"inputID":"KEY_A","inputType":"KeyInput","outputType":"keyboardGroup","keyboardGroup":{"key":key}}]); }
                    body.restore(Some(&draft),window,cx);
                    cx.emit(crate::features::keyboard_products::KeyboardProductChanged);
                });
            })).unwrap();
            cx.run_until_parked();
        };
        edit(cx,"func",None);
        let first=events.read_with(cx,|events,_|{assert_eq!(events.0.len(),1);assert_eq!(events.1,0);events.0[0].clone()});
        page.update(cx,|page,_|{
            assert_eq!(primary_value(&page.device).unwrap(),"multi");
            page.device.power_status=Some(razer_model::model::PowerStatus{level:37,charging_status:"NOT_CHARGING".into()});
        });
        edit(cx,"func",Some("KEY_B"));
        events.read_with(cx,|events,_|{assert_eq!(events.0.len(),1);assert_eq!(events.1,0);});
        page.update(cx,|page,cx|{
            assert!(page.finish_keyboard_function_swap(&first,Err("SET refused".into()),cx).is_none());
            assert_eq!(primary_value(&page.device).unwrap(),"multi");
            let FamilyBody::Keyboard(body)=&page.body else {unreachable!()};
            assert_eq!(body.read(cx).snapshot()["functionKeyPrimary"],"func");
        });
        cx.run_until_parked();
        edit(cx,"func",Some("KEY_C"));
        events.read_with(cx,|events,_|{assert_eq!(events.0.len(),1,"another setting cannot retry a failed Fn edit implicitly");assert_eq!(events.1,2);});
        edit(cx,"multi",None);
        edit(cx,"func",None);
        let (second,third)=events.read_with(cx,|events,_|{assert_eq!(events.0.len(),3);(events.0[1].clone(),events.0[2].clone())});
        page.update(cx,|page,cx|{
            assert!(!page.keyboard_function_swap_current(&second,cx));
            assert!(page.finish_keyboard_function_swap(&second,Ok(ack(FunctionKeyPrimary::Multimedia)),cx).is_none());
            let saved=page.finish_keyboard_function_swap(&third,Ok(ack(FunctionKeyPrimary::Function)),cx).unwrap();
            assert_eq!(primary_value(&saved).unwrap(),"func");
            assert_eq!(saved.power_status.as_ref().unwrap().level,37);
            assert_eq!(saved.profiles[0].source_settings.as_ref().unwrap()["mappings"][0]["keyboardGroup"]["key"],"KEY_C");
        });
        edit(cx,"multi",None);
        let close_request=events.read_with(cx,|events,_|events.0.last().unwrap().clone());
        page.update(cx,|page,cx|{
            page.exiting=true;
            assert!(page.keyboard_function_swap_current(&close_request,cx));
            assert!(page.finish_keyboard_function_swap(&close_request,Ok(ack(FunctionKeyPrimary::Multimedia)),cx).is_some());
            page.observe_connection(None,cx);
            assert!(!page.keyboard_function_swap_current(&close_request,cx));
        });
    }
}

impl KeyboardFunctionSwapRequested {
    pub fn id(&self) -> uuid::Uuid {
        self.id
    }
    pub fn primary(&self) -> FunctionKeyPrimary {
        self.primary
    }
    pub fn snapshot(&self) -> &Device {
        &self.snapshot
    }
    fn matches(&self, device: &Device, owner: Option<uuid::Uuid>, generation: u64) -> bool {
        self.owner == owner
            && self.generation == generation
            && razer_model::profile_transfer::ProfileContext::from_device(&self.snapshot)
                .matches(device)
    }
}

#[derive(Default)]
pub(super) struct State {
    pending: Option<KeyboardFunctionSwapRequested>,
    draft: Option<(String, Option<FunctionKeyPrimary>)>,
    deferred_save: bool,
}
impl State {
    pub(super) fn invalidate(&mut self) {
        self.pending = None;
        self.draft = None;
        self.deferred_save = false;
    }
    pub(super) fn restored(&mut self, profile: &str, value: Option<&Value>) -> Result<(), String> {
        self.invalidate();
        self.draft = Some((profile.to_owned(), parse_primary(value)?));
        Ok(())
    }
}

fn parse_primary(value: Option<&Value>) -> Result<Option<FunctionKeyPrimary>, String> {
    match value {
        None => Ok(None),
        Some(value) => match value.as_str() {
            Some("func") => Ok(Some(FunctionKeyPrimary::Function)),
            Some("multi") => Ok(Some(FunctionKeyPrimary::Multimedia)),
            _ => Err("Function-key profile value is invalid".into()),
        },
    }
}
fn primary_value(device: &Device) -> Option<&Value> {
    device
        .active_profile_obj()
        .and_then(|profile| profile.source_settings.as_ref())
        .and_then(|profile| profile.get("functionKeyPrimary"))
}
fn selected_snapshot(device: &Device, primary: FunctionKeyPrimary) -> Result<Device, String> {
    let mut snapshot = device.clone();
    let profile = snapshot
        .profiles
        .iter_mut()
        .find(|profile| profile.id == snapshot.active_profile)
        .and_then(|profile| profile.source_settings.as_mut())
        .and_then(Value::as_object_mut)
        .ok_or("Function-key source profile is missing")?;
    profile.insert(
        "functionKeyPrimary".into(),
        Value::from(match primary {
            FunctionKeyPrimary::Function => "func",
            FunctionKeyPrimary::Multimedia => "multi",
        }),
    );
    Ok(snapshot)
}

impl SourceProductWorkspace {
    fn function_swap_owner(&self, cx: &App) -> Option<uuid::Uuid> {
        match &self.body {
            FamilyBody::Keyboard(body) => body.read(cx).shared_keyboard_owner(),
            _ => None,
        }
    }

    pub(super) fn capture_keyboard_product_change(
        &mut self,
        mut value: Value,
        cx: &mut Context<Self>,
    ) {
        if self.device.product_id != 717 {
            self.capture(value, cx);
            self.persist_source_profile(false, cx);
            return;
        }
        if self.exiting {
            return;
        }
        let desired = match parse_primary(value.get("functionKeyPrimary")) {
            Ok(desired) => desired,
            Err(error) => {
                cx.emit(WorkspaceEvent::KeyboardLegacyRuntimeStatus(error));
                return;
            }
        };
        let previous = self
            .keyboard_function_swap
            .draft
            .as_ref()
            .filter(|(profile, _)| profile == &self.device.active_profile)
            .map(|(_, primary)| *primary)
            .unwrap_or_else(|| parse_primary(primary_value(&self.device)).ok().flatten());
        self.keyboard_function_swap.draft = Some((self.device.active_profile.clone(), desired));
        // The checkbox remains optimistic in the page. The document offered to
        // persistence keeps its prior value until this particular setter ACK.
        if let Some(prior) = primary_value(&self.device) {
            value["functionKeyPrimary"] = prior.clone();
        } else if let Some(fields) = value.as_object_mut() {
            fields.remove("functionKeyPrimary");
        }
        self.capture(value, cx);
        let owner = self.function_swap_owner(cx);
        let generation = self.mapping_connection_generation;
        if self
            .keyboard_function_swap
            .pending
            .as_ref()
            .is_some_and(|pending| !pending.matches(&self.device, owner, generation))
        {
            self.keyboard_function_swap.pending = None;
            self.keyboard_function_swap.deferred_save = false;
        }
        if desired == previous {
            if let Some(pending) = &mut self.keyboard_function_swap.pending {
                if Some(pending.primary) == desired
                    && pending.matches(&self.device, owner, generation)
                {
                    self.keyboard_function_swap.deferred_save = true;
                    match selected_snapshot(&self.device, pending.primary) {
                        Ok(snapshot) => pending.snapshot = snapshot,
                        Err(error) => cx.emit(WorkspaceEvent::KeyboardLegacyRuntimeStatus(error)),
                    }
                    return;
                }
            }
            self.persist_source_profile(false, cx);
            return;
        }
        let Some(primary) = desired else {
            self.persist_source_profile(false, cx);
            return;
        };
        if let Err(error) = self.update_keyboard_selection_for_save() {
            cx.emit(WorkspaceEvent::KeyboardLegacyRuntimeStatus(error));
            return;
        }
        let snapshot = match selected_snapshot(&self.device, primary) {
            Ok(snapshot) => snapshot,
            Err(error) => {
                cx.emit(WorkspaceEvent::KeyboardLegacyRuntimeStatus(error));
                return;
            }
        };
        let request = KeyboardFunctionSwapRequested {
            id: uuid::Uuid::new_v4(),
            owner,
            generation,
            primary,
            snapshot,
        };
        self.keyboard_function_swap.pending = Some(request.clone());
        cx.emit(WorkspaceEvent::KeyboardFunctionSwapRequested(request));
    }

    pub fn keyboard_function_swap_current(
        &self,
        request: &KeyboardFunctionSwapRequested,
        cx: &App,
    ) -> bool {
        self.keyboard_function_swap
            .pending
            .as_ref()
            .is_some_and(|pending| pending.id == request.id)
            && self
                .keyboard_function_swap
                .draft
                .as_ref()
                .is_some_and(|(_, primary)| *primary == Some(request.primary))
            && request.matches(
                &self.device,
                self.function_swap_owner(cx),
                self.mapping_connection_generation,
            )
    }

    pub fn finish_keyboard_function_swap(
        &mut self,
        request: &KeyboardFunctionSwapRequested,
        result: Result<FunctionKeySwapWriteReport, String>,
        cx: &mut Context<Self>,
    ) -> Option<Device> {
        if !self.keyboard_function_swap_current(request, cx) {
            return None;
        }
        self.keyboard_function_swap.pending.take().unwrap();
        let deferred_save = std::mem::take(&mut self.keyboard_function_swap.deferred_save);
        match result.and_then(|report| {
            report
                .validate_write_ack(request.primary)
                .map_err(|error| format!("{error:#}"))
        }) {
            Ok(()) => match selected_snapshot(&self.device, request.primary) {
                Ok(snapshot) => {
                    self.device = snapshot;
                    Some(self.device.clone())
                }
                Err(error) => {
                    cx.emit(WorkspaceEvent::KeyboardLegacyRuntimeStatus(error));
                    None
                }
            },
            Err(error) => {
                cx.emit(WorkspaceEvent::KeyboardLegacyRuntimeStatus(format!(
                    "功能键修改未确认：{error}"
                )));
                if deferred_save {
                    self.persist_source_profile(false, cx);
                }
                cx.notify();
                None
            }
        }
    }
}
