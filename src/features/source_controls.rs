//! Retained native controls whose labels, bounds, options and bindings are
//! supplied by statically audited per-product descriptors.
use super::Choice;
use crate::ui::surface;
use gpui_kit::component::{button::Button, checkbox::Checkbox, select::{Select, SelectEvent, SelectState}, slider::{Slider, SliderEvent, SliderState}, *};
use gpui_kit::{prelude::FluentBuilder as _, *};
use serde::Deserialize;
use serde_json::Value;
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Deserialize)]
struct OptionSpec { label: String, value: Value }
#[derive(Deserialize)]
struct ConditionSpec { path: String, value: Value }
#[derive(Deserialize)]
struct MinimumSpec { path: String, value: Value, min: f32 }
#[derive(Deserialize)]
struct ControlSpec {
    key: String, label: String, path: String, kind: String,
    #[serde(default)] min: f32, #[serde(default)] max: f32, #[serde(default)] step: f32,
    #[serde(default)] options: Vec<OptionSpec>,
    #[serde(default)] disabled_when: Option<String>,
    #[serde(default)] disabled_unless: Option<String>,
    #[serde(default)] disabled_unless_all: Vec<String>,
    #[serde(default)] visible_when: Option<ConditionSpec>,
    #[serde(default)] reset_value: Option<Value>,
    #[serde(default)] minimum_when: Option<MinimumSpec>,
    #[serde(default)] enabled_from_value: Option<String>,
}
#[derive(Deserialize)]
struct SectionSpec { title: String, #[serde(default)] description: Option<String>, controls: Vec<ControlSpec> }
#[derive(Deserialize)]
struct PageSpec { key: String, sections: Vec<SectionSpec> }
#[derive(Deserialize)]
struct ProductSpec {
    product_id: u32, profile: Value, pages: Vec<PageSpec>,
    #[serde(default)] support: Option<String>,
}
fn specs() -> &'static [ProductSpec] {
    static SPECS: OnceLock<Vec<ProductSpec>> = OnceLock::new();
    SPECS.get_or_init(|| {
        let mut specs: Vec<ProductSpec> = serde_json::from_str(include_str!("source_controls_data.json")).expect("validated camera descriptors");
        specs.extend(serde_json::from_str::<Vec<ProductSpec>>(include_str!("accessory_controls_data.json")).expect("validated accessory descriptors"));
        specs
    })
}
pub(crate) fn supports(pid: u32) -> bool { specs().iter().any(|s| s.product_id == pid) }
pub(crate) fn supports_page(pid: u32, key: &str) -> bool {
    specs().iter().find(|s| s.product_id == pid).is_some_and(|s| s.pages.iter().any(|p| p.key == key && !p.sections.is_empty()))
}
pub(crate) struct SourceControlsChanged;
pub(crate) struct SourceControls {
    spec: &'static ProductSpec,
    page: String,
    draft: Value,
    sliders: BTreeMap<String, Entity<SliderState>>,
    selects: BTreeMap<String, Entity<SelectState<Vec<Choice>>>>,
    subscriptions: Vec<Subscription>,
    syncing: bool,
}
impl EventEmitter<SourceControlsChanged> for SourceControls {}
impl SourceControls {
    pub(crate) fn new(pid: u32, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let spec = specs().iter().find(|s| s.product_id == pid).expect("source controls registered");
        let mut this = Self { spec, page: spec.pages.first().map_or(String::new(), |p| p.key.clone()), draft: spec.profile.clone(), sliders: BTreeMap::new(), selects: BTreeMap::new(), subscriptions: vec![], syncing: false };
        for control in spec.pages.iter().flat_map(|p| &p.sections).flat_map(|s| &s.controls) {
            let key = control.key.clone();
            if control.kind == "slider" && !this.sliders.contains_key(&key) {
                let initial = this.value(control).and_then(Value::as_f64).map(|v| v as f32).unwrap_or(control.min);
                let state = cx.new(|_| SliderState::new().min(control.min).max(control.max).step(control.step).default_value(initial));
                let target = key.clone();
                this.subscriptions.push(cx.subscribe_in(&state, window, move |this,_,event,window,cx| {
                    if this.syncing { return; }
                    let SliderEvent::Change(value) = event else { return; };
                    this.edit(&target, serde_json::json!(value.start()), window, cx);
                }));
                this.sliders.insert(key, state);
            } else if control.kind == "select" && !this.selects.contains_key(&key) {
                let choices = control.options.iter().enumerate().map(|(ix,o)| Choice::new(ix.to_string(), crate::i18n::t(&o.label))).collect::<Vec<_>>();
                let state = cx.new(|cx| SelectState::new(choices, None, window, cx));
                let target = key.clone();
                this.subscriptions.push(cx.subscribe_in(&state, window, move |this,_,event,window,cx| {
                    if this.syncing { return; }
                    if let SelectEvent::Confirm(Some(value)) = event {
                        if let Some(value) = value.parse::<usize>().ok().and_then(|ix| this.control(&target).and_then(|c| c.options.get(ix))).map(|o| o.value.clone()) {
                            this.edit(&target, value, window, cx);
                        }
                    }
                }));
                this.selects.insert(key, state);
            }
        }
        this.sync(window, cx); this
    }
    fn control(&self, key: &str) -> Option<&'static ControlSpec> { self.spec.pages.iter().flat_map(|p| &p.sections).flat_map(|s| &s.controls).find(|c| c.key == key) }
    fn image_index(&self) -> usize { self.draft.get("imageMode").or_else(|| self.draft.get("imageSettingMode")).and_then(Value::as_u64).unwrap_or(0) as usize }
    fn path(&self, control: &ControlSpec) -> String {
        self.resolve_path(&control.path)
    }
    fn resolve_path(&self, path: &str) -> String {
        if let Some(field) = path.strip_prefix("@image/") {
            format!("/image/{}/dataSet/{field}", self.image_index())
        } else if let Some(field) = path.strip_prefix("@view/") {
            let mode = self.draft.pointer("/camera/viewPresets/viewMode");
            let ix = self.draft.pointer("/camera/viewPresets/data").and_then(Value::as_array)
                .and_then(|presets| presets.iter().position(|p| p.get("mode") == mode));
            ix.map(|ix| format!("/camera/viewPresets/data/{ix}/{field}")).unwrap_or_else(|| "/missing-view-preset".into())
        } else { path.to_owned() }
    }
    fn value(&self, control: &ControlSpec) -> Option<&Value> { self.draft.pointer(&self.path(control)) }
    fn minimum(&self, control: &ControlSpec) -> f32 {
        control.minimum_when.as_ref().filter(|condition| self.draft.pointer(&self.resolve_path(&condition.path)) == Some(&condition.value))
            .map_or(control.min, |condition| condition.min)
    }
    fn disabled(&self, control: &ControlSpec) -> bool {
        self.value(control).is_none()
            || control.disabled_when.as_ref().is_some_and(|path| self.draft.pointer(&self.resolve_path(path)).and_then(Value::as_bool).unwrap_or(false))
            || control.disabled_unless.as_ref().is_some_and(|path| self.draft.pointer(&self.resolve_path(path)).and_then(Value::as_bool) != Some(true))
            || control.disabled_unless_all.iter().any(|path| self.draft.pointer(&self.resolve_path(path)).and_then(Value::as_bool) != Some(true))
    }
    fn edit(&mut self, key: &str, mut value: Value, window: &mut Window, cx: &mut Context<Self>) {
        let Some(control) = self.control(key) else { return; };
        if self.disabled(control) || control.visible_when.as_ref().is_some_and(|condition| self.draft.pointer(&self.resolve_path(&condition.path)) != Some(&condition.value)) { return; }
        if control.kind == "reset" {
            let Some(reset) = &control.reset_value else { return; };
            value = reset.clone();
        }
        if matches!(control.kind.as_str(), "toggle" | "switch") && control.options.len() == 2 {
            value = control.options[usize::from(value.as_bool().unwrap_or(false))].value.clone();
        }
        if control.kind == "slider" {
            let Some(number) = value.as_f64() else { return; };
            if !number.is_finite() || control.step <= 0. { return; }
            let min = self.minimum(control);
            let n = (number as f32).clamp(min, control.max);
            let snapped = (min + ((n-min)/control.step).round()*control.step).clamp(min, control.max);
            value = if control.step.fract() == 0. { serde_json::json!(snapped as i64) } else { serde_json::json!(snapped) };
        }
        if matches!(control.kind.as_str(), "select" | "options") && !control.options.iter().any(|option| option.value == value) { return; }
        if self.value(control) == Some(&value) { return; }
        if control.path.starts_with("@image/") {
            let current = self.image_index();
            let custom = self.draft.get("image").and_then(Value::as_array).and_then(|a| a.iter().position(|p| p.get("name").and_then(Value::as_str) == Some("custom")));
            if let Some(custom) = custom {
                if current != custom {
                    let source = self.draft.pointer(&format!("/image/{current}/dataSet")).cloned();
                    if let Some(source) = source { self.draft["image"][custom]["dataSet"] = source; }
                    let mode = if self.draft.get("imageMode").is_some() { "imageMode" } else { "imageSettingMode" };
                    self.draft[mode] = serde_json::json!(custom);
                }
            }
        }
        let path = self.path(control);
        if let Some(target) = self.draft.pointer_mut(&path) { *target = value.clone(); }
        if let Some(enabled) = &control.enabled_from_value {
            let enabled = self.resolve_path(enabled);
            if let Some(target) = self.draft.pointer_mut(&enabled) { *target = Value::Bool(value.as_f64().is_some_and(|v| v != 0.)); }
        }
        self.normalize();
        self.sync(window, cx); cx.emit(SourceControlsChanged); cx.notify();
    }
    fn normalize(&mut self) {
        for control in self.spec.pages.iter().flat_map(|p| &p.sections).flat_map(|s| &s.controls) {
            if control.kind != "slider" { continue; }
            let path = self.path(control);
            let min = self.minimum(control);
            if let Some(value) = self.draft.pointer_mut(&path).filter(|v| v.is_number()) {
                let number = value.as_f64().unwrap_or(min as f64) as f32;
                let number = (min + ((number.clamp(min, control.max)-min)/control.step).round()*control.step).clamp(min, control.max);
                *value = if control.step.fract() == 0. { serde_json::json!(number as i64) } else { serde_json::json!(number) };
            }
        }
    }
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.syncing = true;
        for (key, state) in &self.sliders {
            if let Some(control) = self.control(key) {
                if let Some(value) = self.value(control).and_then(Value::as_f64) {
                    let min = self.minimum(control);
                    state.update(cx, |s,cx| {
                        if s.min_value() != min { *s = SliderState::new().min(min).max(control.max).step(control.step); }
                        s.set_value(value as f32, window, cx);
                    });
                }
            }
        }
        for (key, state) in &self.selects {
            if let Some(control) = self.control(key) {
                let value = self.value(control);
                let selected = control.options.iter().position(|o| Some(&o.value) == value).map(|i| i.to_string());
                state.update(cx, |s,cx| s.set_selected_value(&selected.unwrap_or_default(), window, cx));
            }
        }
        self.syncing = false;
    }
    pub(crate) fn snapshot(&self) -> Value { self.draft.clone() }
    pub(crate) fn restore(&mut self, value: Option<&Value>, window: &mut Window, cx: &mut Context<Self>) {
        self.draft = self.spec.profile.clone();
        if let Some(value) = value { merge_known(&mut self.draft, value); }
        self.normalize();
        self.sync(window, cx); cx.notify();
    }
    pub(crate) fn set_page(&mut self, page: &str, _: &mut Window, cx: &mut Context<Self>) { self.page = page.into(); cx.notify(); }
    fn render_control(&self, control: &ControlSpec, cx: &mut Context<Self>) -> AnyElement {
        if control.visible_when.as_ref().is_some_and(|condition| self.draft.pointer(&self.resolve_path(&condition.path)) != Some(&condition.value)) { return div().into_any_element(); }
        let key = control.key.clone();
        let label = crate::i18n::t(&control.label);
        let disabled = self.disabled(control);
        match control.kind.as_str() {
            "options" => v_flex().gap_2().child(label).child(h_flex().gap(surface::css(10.)).flex_wrap().children(control.options.iter().map(|option| {
                use crate::ui::theme::CameraProductColors as Colors;
                let selected = self.value(control) == Some(&option.value);
                let value = option.value.clone(); let key = key.clone();
                gpui_kit::base::Button::new(SharedString::from(format!("{}:{}",key, value)))
                    .accessibility_label(crate::i18n::t(&option.label)).disabled(disabled)
                    .min_w(surface::css(90.)).px(surface::css(16.)).pt(surface::css(7.)).pb(surface::css(6.))
                    .text_size(surface::css(12.)).line_height(surface::css(14.)).text_color(Colors::text())
                    .rounded(surface::css(3.)).border_1().border_color(if selected { cx.theme().primary } else { Colors::border() })
                    .when(selected, |b| b.bg(Colors::selected()))
                    .styles(|s| s.disabled(|s| s.opacity(0.3)))
                    .when(!disabled, |b| b.hover(|b| b.bg(Colors::hover()).border_color(cx.theme().primary)).active(|b| b.bg(Colors::pressed())))
                    .focus_visible(|b| b.border_color(cx.theme().primary))
                    .child(crate::i18n::t(&option.label)).on_click(cx.listener(move |this,_,window,cx| this.edit(&key,value.clone(),window,cx)))
            }))).into_any_element(),
            "reset" => Button::new(SharedString::from(key.clone())).label(label).outline().disabled(disabled)
                .on_click(cx.listener(move |this,_,window,cx| this.edit(&key, Value::Null, window,cx))).into_any_element(),
            "switch" => surface::SynapseSwitch::new(SharedString::from(key.clone())).label(label).checked(if control.options.len() == 2 { self.value(control) == Some(&control.options[1].value) } else { self.value(control).and_then(Value::as_bool).unwrap_or(false) }).disabled(disabled)
                .on_change(cx.listener(move |this,value,window,cx| this.edit(&key, Value::Bool(*value), window,cx))).into_any_element(),
            "toggle" => Checkbox::new(SharedString::from(key.clone())).label(label).checked(if control.options.len() == 2 { self.value(control) == Some(&control.options[1].value) } else { self.value(control).and_then(Value::as_bool).unwrap_or(false) }).disabled(disabled)
                .on_click(cx.listener(move |this,value,window,cx| this.edit(&key, Value::Bool(*value), window,cx))).into_any_element(),
            "slider" => v_flex().gap_2().child(h_flex().child(label).child(div().flex_1()).child(self.value(control).map(|v| v.to_string()).unwrap_or_else(|| "—".into())))
                .child(Slider::new(&self.sliders[&key]).disabled(disabled)).into_any_element(),
            "select" => v_flex().gap_2().child(label).child(Select::new(&self.selects[&key]).disabled(disabled)).into_any_element(),
            _ => div().into_any_element(),
        }
    }
}

// Keep new source defaults when restoring an older local profile. An unrelated
// product's object or a changed JSON type cannot replace the current schema.
fn merge_known(target: &mut Value, saved: &Value) {
    match (target, saved) {
        (Value::Object(target), Value::Object(saved)) => {
            for (key, value) in target { if let Some(saved) = saved.get(key) { merge_known(value, saved); } }
        }
        (Value::Array(target), Value::Array(saved)) => {
            for (value, saved) in target.iter_mut().zip(saved) { merge_known(value, saved); }
        }
        (target @ Value::Bool(_), Value::Bool(_))
        | (target @ Value::Number(_), Value::Number(_))
        | (target @ Value::String(_), Value::String(_)) => *target = saved.clone(),
        _ => {}
    }
}
impl Render for SourceControls {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut view = v_flex().min_w(surface::css(600.)).w_full().max_w(surface::css(1240.)).mx_auto().p(surface::css(20.)).gap(surface::css(20.));
        if self.page == "HELP" {
            if let Some(url) = &self.spec.support {
                let url = url.clone();
                view = view.child(Button::new("source-product-support").label(crate::i18n::t("SUPPORT")).outline().on_click(move |_,_,cx| cx.open_url(&url)));
            }
        } else if let Some(page) = self.spec.pages.iter().find(|p| p.key == self.page && !p.sections.is_empty()) {
            for section in &page.sections {
                let mut panel = surface::panel(crate::i18n::t(&section.title), cx);
                if let Some(description) = &section.description { panel = panel.child(surface::note(crate::i18n::t(description), cx)); }
                for control in &section.controls { panel = panel.child(self.render_control(control, cx)); }
                view = view.child(panel);
            }
        } else {
            view = view.child(surface::note("此页面的原生控件仍在接入。", cx));
        }
        view
    }
}
