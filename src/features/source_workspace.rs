//! Native product ownership. Source navigation identity and local profile data
//! stay independent from the original ten hand-written adapters.
use super::{Choice, WorkspaceEvent};
use crate::{model::{Device, Profile}, product::{self, ProductPage, ProductPageId, ProductPageRole}, ui::{surface, scroll::SourceScrollable as _}};
use gpui_kit::component::{button::{Button, ButtonVariants}, select::{Select, SelectEvent, SelectState}, *};
use gpui_kit::{prelude::FluentBuilder as _, *};
use serde_json::Value;

enum FamilyBody {
    Mouse(Entity<super::mouse_products::MouseProductWorkspace>),
    Keyboard(Entity<super::keyboard_products::KeyboardProductWorkspace>),
    Gamepad(Entity<super::gamepad_products::GamepadProductWorkspace>),
    Controls(Entity<super::source_controls::SourceControls>),
    Audio(Entity<super::audio_products::AudioProductWorkspace>),
    System(Entity<super::system_products::SystemProductWorkspace>),
    AccessorySystem(Entity<super::accessory_system_products::AccessorySystemProductWorkspace>),
    Pending,
}
pub(crate) struct SourceProductWorkspace {
    device: Device,
    saved: Device,
    page: Option<ProductPageId>,
    body: FamilyBody,
    help: Entity<super::source_help::SourceHelp>,
    supplement: Option<Entity<super::source_controls::SourceControls>>,
    profile: Entity<SelectState<Vec<Choice>>>,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<WorkspaceEvent> for SourceProductWorkspace {}

impl SourceProductWorkspace {
    pub(crate) fn new(mut device: Device, window: &mut Window, cx: &mut Context<Self>) -> Self {
        if device.profiles.is_empty() {
            device.profiles.push(Profile { id: "local-default".into(), guid: "local-default".into(), name: "Default".into(), dpi_stages: None, settings: None, source_settings: None });
        }
        if !device.profiles.iter().any(|p| p.id == device.active_profile) {
            device.active_profile = device.profiles[0].id.clone();
        }
        let registered = product::registered(device.product_id);
        let page = registered.and_then(|p| p.primary_navigation()).and_then(|n| n.pages().iter().find(|p| p.role() != ProductPageRole::Help)).map(|p| p.id());
        let mut subscriptions = vec![];
        let body = if super::mouse_products::source_product(device.product_id).is_some() {
            let body = cx.new(|cx| super::mouse_products::MouseProductWorkspace::new(device.product_id, window, cx));
            subscriptions.push(cx.subscribe(&body, |this: &mut Self, body, _: &super::mouse_products::MouseProductChanged, cx| {
                this.capture(body.read(cx).snapshot(), cx);
            }));
            FamilyBody::Mouse(body)
        } else if super::keyboard_products::source_product(device.product_id).is_some() {
            let body = cx.new(|cx| super::keyboard_products::KeyboardProductWorkspace::new(device.product_id, window, cx));
            subscriptions.push(cx.subscribe(&body, |this: &mut Self, body, _: &super::keyboard_products::KeyboardProductChanged, cx| {
                this.capture(body.read(cx).snapshot(), cx);
            }));
            FamilyBody::Keyboard(body)
        } else if super::gamepad_products::source_product(device.product_id).is_some() {
            let body = cx.new(|cx| super::gamepad_products::GamepadProductWorkspace::new(device.product_id, window, cx));
            subscriptions.push(cx.subscribe(&body, |this: &mut Self, body, _: &super::gamepad_products::GamepadProductChanged, cx| {
                this.capture(body.read(cx).snapshot(), cx);
            }));
            FamilyBody::Gamepad(body)
        } else if super::audio_products::source_product(device.product_id).is_some() {
            let body = cx.new(|cx| super::audio_products::AudioProductWorkspace::new(device.product_id, window, cx));
            subscriptions.push(cx.subscribe(&body, |this: &mut Self, body, _: &super::audio_products::AudioProductChanged, cx| {
                this.capture(body.read(cx).snapshot(), cx);
            }));
            FamilyBody::Audio(body)
        } else if super::system_products::source_product(device.product_id).is_some() {
            let body = cx.new(|cx| super::system_products::SystemProductWorkspace::new(device.product_id, window, cx));
            subscriptions.push(cx.subscribe(&body, |this: &mut Self, body, _: &super::system_products::SystemProductChanged, cx| {
                this.capture(body.read(cx).snapshot(), cx);
            }));
            FamilyBody::System(body)
        } else if super::accessory_system_products::source_product(device.product_id).is_some() {
            let body = cx.new(|cx| super::accessory_system_products::AccessorySystemProductWorkspace::new(device.product_id, window, cx));
            subscriptions.push(cx.subscribe(&body, |this: &mut Self, body, _: &super::accessory_system_products::AccessorySystemProductChanged, cx| {
                this.capture(body.read(cx).snapshot(), cx);
            }));
            FamilyBody::AccessorySystem(body)
        } else if super::source_controls::supports(device.product_id) {
            let body = cx.new(|cx| super::source_controls::SourceControls::new(device.product_id, window, cx));
            subscriptions.push(cx.subscribe(&body, |this: &mut Self, body, _: &super::source_controls::SourceControlsChanged, cx| {
                this.capture(body.read(cx).snapshot(), cx);
            }));
            FamilyBody::Controls(body)
        } else { FamilyBody::Pending };
        let supplement = if !matches!(body, FamilyBody::Controls(_)) && super::source_controls::supports(device.product_id) {
            let controls = cx.new(|cx| super::source_controls::SourceControls::new(device.product_id, window, cx));
            subscriptions.push(cx.subscribe(&controls, |this: &mut Self, controls, _: &super::source_controls::SourceControlsChanged, cx| {
                this.capture_supplement(controls.read(cx).snapshot(), cx);
            }));
            Some(controls)
        } else { None };
        let profile = cx.new(|cx| SelectState::new(device.profiles.iter().map(|p| Choice::new(&p.id, &p.name)).collect::<Vec<_>>(), None, window, cx));
        profile.update(cx, |state, cx| state.set_selected_value(&device.active_profile, window, cx));
        subscriptions.push(cx.subscribe_in(&profile, window, |this: &mut Self, _, event, window, cx| {
            if let SelectEvent::Confirm(Some(id)) = event {
                if this.device.profiles.iter().any(|p| p.id == *id) {
                    this.device.active_profile = id.clone();
                    this.restore_active(window, cx);
                    cx.emit(WorkspaceEvent::Changed);
                    cx.notify();
                }
            }
        }));
        let help = cx.new(|cx| super::source_help::SourceHelp::new(device.clone(), cx));
        let mut this = Self { saved: device.clone(), device, page, body, help, supplement, profile, _subscriptions: subscriptions };
        this.restore_active(window, cx);
        this.select_body_page(window, cx);
        this.saved = this.device.clone();
        this
    }
    pub(crate) fn device(&self) -> &Device { &self.device }
    pub(crate) fn saved_snapshot(&self) -> Device { self.saved.clone() }
    pub(crate) fn dirty(&self) -> bool {
        self.device.active_profile != self.saved.active_profile || self.device.profiles.len() != self.saved.profiles.len() || self.device.profiles.iter().zip(&self.saved.profiles).any(|(a,b)| a.id != b.id || a.name != b.name || a.source_settings != b.source_settings)
    }
    pub(crate) fn mark_saved(&mut self, snapshot: Device, cx: &mut Context<Self>) { self.saved = snapshot; cx.notify(); }
    pub(crate) fn discard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.device = self.saved.clone();
        self.profile.update(cx, |p, cx| p.set_selected_value(&self.device.active_profile, window, cx));
        self.restore_active(window, cx);
        cx.emit(WorkspaceEvent::Changed); cx.notify();
    }
    fn capture(&mut self, mut value: Value, cx: &mut Context<Self>) {
        if let Some(p) = self.device.profiles.iter_mut().find(|p| p.id == self.device.active_profile) {
            if let Some(supplement) = p.source_settings.as_ref().and_then(|s| s.get("_supplement")).cloned() {
                value["_supplement"] = supplement;
            }
            p.source_settings = Some(value);
        }
        cx.emit(WorkspaceEvent::Changed); cx.notify();
    }
    fn capture_supplement(&mut self, value: Value, cx: &mut Context<Self>) {
        if let Some(p) = self.device.profiles.iter_mut().find(|p| p.id == self.device.active_profile) {
            let settings = p.source_settings.get_or_insert_with(|| serde_json::json!({}));
            settings["_supplement"] = value;
        }
        cx.emit(WorkspaceEvent::Changed); cx.notify();
    }
    fn restore_active(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self.device.profiles.iter().find(|p| p.id == self.device.active_profile).and_then(|p| p.source_settings.clone());
        let snapshot = match &self.body {
            FamilyBody::Mouse(e) => { e.update(cx, |v,cx| v.restore(value.as_ref(), window, cx)); Some(e.read(cx).snapshot()) },
            FamilyBody::Keyboard(e) => { e.update(cx, |v,cx| v.restore(value.as_ref(), window, cx)); Some(e.read(cx).snapshot()) },
            FamilyBody::Gamepad(e) => { e.update(cx, |v,cx| v.restore(value.as_ref(), window, cx)); Some(e.read(cx).snapshot()) },
            FamilyBody::Controls(e) => { e.update(cx, |v,cx| v.restore(value.as_ref(), window, cx)); Some(e.read(cx).snapshot()) },
            FamilyBody::Audio(e) => { e.update(cx, |v,cx| v.restore(value.as_ref(), window, cx)); Some(e.read(cx).snapshot()) },
            FamilyBody::System(e) => { e.update(cx, |v,cx| v.restore(value.as_ref(), window, cx)); Some(e.read(cx).snapshot()) },
            FamilyBody::AccessorySystem(e) => { e.update(cx, |v,cx| v.restore(value.as_ref(), window, cx)); Some(e.read(cx).snapshot()) },
            FamilyBody::Pending => None,
        };
        let supplement = self.supplement.as_ref().map(|controls| {
            controls.update(cx, |v,cx| v.restore(value.as_ref().and_then(|v| v.get("_supplement").or(Some(v))), window, cx));
            controls.read(cx).snapshot()
        });
        if let Some(mut snapshot) = snapshot {
            if let Some(supplement) = supplement { snapshot["_supplement"] = supplement; }
            if let Some(p) = self.device.profiles.iter_mut().find(|p| p.id == self.device.active_profile) { p.source_settings = Some(snapshot); }
        }
    }
    fn current_page(&self) -> Option<&'static ProductPage> { product::registered(self.device.product_id)?.page(self.page?) }
    pub(crate) fn set_page_key(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(product) = product::registered(self.device.product_id) else { return; };
        let normalized = key.to_ascii_uppercase();
        let page = product.primary_navigation().into_iter().flat_map(|n| n.pages()).find(|p| p.id().key() == key || p.kind().key() == normalized || p.kind().key().strip_prefix("TAB_") == Some(normalized.as_str()));
        if let Some(page) = page { self.set_page(page.id(), window, cx); }
    }
    fn set_page(&mut self, page: ProductPageId, window: &mut Window, cx: &mut Context<Self>) {
        if product::registered(self.device.product_id).and_then(|p| p.page(page)).is_none() { return; }
        self.page = Some(page); self.select_body_page(window, cx); cx.notify();
    }
    fn select_body_page(&self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(page) = self.current_page() else { return; };
        if page.role() == ProductPageRole::Help {
            self.help.update(cx, |help, cx| {
                help.set_device(&self.device, cx);
                help.set_page(page.offset(), cx);
            });
            return;
        }
        if let Some(controls) = &self.supplement { controls.update(cx, |v,cx| v.set_page(page.kind().key(), window, cx)); }
        match &self.body {
            FamilyBody::Mouse(e) => e.update(cx, |v,cx| v.set_page(page.kind().key(), window, cx)),
            FamilyBody::Keyboard(e) => e.update(cx, |v,cx| v.set_page(page.kind().key(), window, cx)),
            FamilyBody::Gamepad(e) => e.update(cx, |v,cx| v.set_page(page.kind().key(), window, cx)),
            FamilyBody::Controls(e) => e.update(cx, |v,cx| v.set_page(page.kind().key(), window, cx)),
            FamilyBody::Audio(e) => e.update(cx, |v,cx| v.set_page(page.kind().key(), window, cx)),
            FamilyBody::System(e) => e.update(cx, |v,cx| v.set_page(page.kind().key(), window, cx)),
            FamilyBody::AccessorySystem(e) => e.update(cx, |v,cx| v.set_page(page.kind().key(), window, cx)),
            FamilyBody::Pending => {},
        }
    }
    fn use_supplement_for(&self, key: &str) -> bool {
        if !super::source_controls::supports_page(self.device.product_id, key) { return false; }
        match &self.body {
            FamilyBody::Audio(_) => !super::audio_products::supports_page(self.device.product_id, key),
            FamilyBody::AccessorySystem(_) => !super::accessory_system_products::supports_page(self.device.product_id, key),
            FamilyBody::Pending => true,
            _ => false,
        }
    }
}
impl Render for SourceProductWorkspace {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let registered = product::registered(self.device.product_id);
        let navigation = registered.and_then(|p| p.primary_navigation());
        let is_help = self.current_page().is_some_and(|page| page.role() == ProductPageRole::Help);
        let body = if is_help {
            self.help.clone().into_any_element()
        } else if let Some(controls) = self.supplement.as_ref().filter(|_| self.current_page().is_some_and(|page| self.use_supplement_for(page.kind().key()))) {
            controls.clone().into_any_element()
        } else { match &self.body {
            FamilyBody::Mouse(e) => e.clone().into_any_element(),
            FamilyBody::Keyboard(e) => e.clone().into_any_element(),
            FamilyBody::Gamepad(e) => e.clone().into_any_element(),
            FamilyBody::Controls(e) => e.clone().into_any_element(),
            FamilyBody::Audio(e) => e.clone().into_any_element(),
            FamilyBody::System(e) => e.clone().into_any_element(),
            FamilyBody::AccessorySystem(e) => e.clone().into_any_element(),
            FamilyBody::Pending => surface::note("此页面的原生控件仍在接入。", cx).into_any_element(),
        } };
        v_flex().size_full().min_h_0().child(
            h_flex().h(surface::css(48.)).flex_shrink_0().border_b_2().border_color(cx.theme().title_bar)
                .child(h_flex().flex_1().min_w_0().px_3().when(!is_help, |view| view.child(Select::new(&self.profile).w(surface::css(180.)))))
                .child(gpui_kit::base::Tabs::new("source-product-navigation").flex().items_center().gap(surface::css(20.)).children(navigation.into_iter().flat_map(|n| n.pages()).filter(|p| p.role() != ProductPageRole::Help).map(|page| {
                    let id = page.id();
                    surface::navigation_button(SharedString::from(format!("product-page-{}", id.key())), page.label(), self.page == Some(id), cx).role(Role::Tab).on_click(cx.listener(move |this,_,window,cx| this.set_page(id, window, cx)))
                })))
                .child(h_flex().flex_1().min_w_0().justify_end().children(navigation.into_iter().flat_map(|n| n.pages()).filter(|p| p.role() == ProductPageRole::Help).map(|page| {
                    let id = page.id();
                    div().id(SharedString::from(format!("source-help-{}", id.key()))).child(surface::asset_button("source-product-help", if self.page == Some(id) { "synapse/help-active.svg" } else { "synapse/help-default.svg" }, page.label(), cx).size(surface::css(24.)).mr(surface::css(10.)).on_click(cx.listener(move |this,_,window,cx| this.set_page(id, window, cx))))
                })))
        ).child(div().id("source-product-content").flex_1().min_h_0().scrollable_both().child(body))
        .when(self.dirty(), |view| view.child(h_flex().flex_shrink_0().px_5().py_2().gap_3().child(surface::note("有未保存的本地更改",cx)).child(div().flex_1())
            .child(Button::new("source-product-discard").label("丢弃更改").outline().on_click(cx.listener(|this,_,window,cx| this.discard(window,cx))))
            .child(Button::new("source-product-save").label("保存到本机").primary().on_click(cx.listener(|_,_,_,cx| cx.emit(WorkspaceEvent::SaveRequested))))))
    }
}
