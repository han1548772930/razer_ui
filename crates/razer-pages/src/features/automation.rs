//! Base Station V3 Chroma automation preferences. The local draft does not execute actions.
use super::Choice;
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    select::{SelectEvent, SelectState},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_i18n as i18n;
use razer_model::model::Device;
use razer_widgets::scroll::SourceScrollable as _;
use razer_widgets::surface;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::OnceLock};
mod editor;
mod launch_sound;
mod preview;
mod theme;
use editor::AutomationEditor;
pub use preview::open_preview;
use theme::Colors;

#[derive(Deserialize)]
struct ActionSpec {
    content: String,
}
#[derive(Deserialize)]
struct EffectSpec {
    id: u32,
    name: String,
}
#[derive(Deserialize)]
struct Spec {
    product_id: u32,
    page: String,
    actions: Vec<ActionSpec>,
    effects: Vec<EffectSpec>,
    effect_defaults: BTreeMap<String, Value>,
    empty_messages: BTreeMap<String, String>,
    translations: BTreeMap<String, BTreeMap<String, String>>,
}
fn spec() -> &'static Spec {
    static SPEC: OnceLock<Spec> = OnceLock::new();
    SPEC.get_or_init(|| {
        serde_json::from_str(include_str!("automation_data.json"))
            .expect("audited automation source")
    })
}
fn text(key: &str) -> String {
    spec()
        .translations
        .get(&i18n::locale())
        .and_then(|t| t.get(key))
        .or_else(|| spec().translations.get("en").and_then(|t| t.get(key)))
        .cloned()
        .unwrap_or_else(|| i18n::t(key))
}
fn icon(kind: u32) -> Img {
    img(SharedString::from(format!("synapse/automation-{kind}.svg"))).size(surface::css(24.))
}
fn action_name(kind: u32) -> &'static str {
    [
        "chroma",
        "outputDevice",
        "inputDevice",
        "globalShortcut",
        "launchGame",
        "macro",
    ][kind as usize]
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct Lane {
    is_enabled: bool,
    data: Vec<Value>,
}
impl Default for Lane {
    fn default() -> Self {
        Self {
            is_enabled: false,
            data: vec![],
        }
    }
}
#[derive(Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct Rule {
    id: u32,
    name: String,
    is_enabled: bool,
    pick_up: Lane,
    put_down: Lane,
}
impl Rule {
    fn new(id: u32) -> Self {
        Self {
            id,
            name: action_name(id).into(),
            is_enabled: false,
            pick_up: Lane::default(),
            put_down: Lane::default(),
        }
    }
    fn lane(&self, down: bool) -> &Lane {
        if down { &self.put_down } else { &self.pick_up }
    }
    fn lane_mut(&mut self, down: bool) -> &mut Lane {
        if down {
            &mut self.put_down
        } else {
            &mut self.pick_up
        }
    }
    fn valid(&self) -> bool {
        [&self.pick_up, &self.put_down].into_iter().all(|l| {
            !l.is_enabled
                || l.data
                    .first()
                    .is_some_and(|v| v.as_object().is_some_and(|o| !o.is_empty()))
        })
    }
}
pub struct AutomationChanged;
impl EventEmitter<AutomationChanged> for Automation {}
pub struct Automation {
    rules: Vec<Rule>,
    preview: bool,
    catalogs: BTreeMap<u32, Vec<Value>>,
    chroma_installed: Option<bool>,
    chroma_busy: bool,
    edition: u32,
    layout: u32,
    alert: Option<String>,
    editor: Option<Entity<AutomationEditor>>,
    subscriptions: Vec<Subscription>,
    audio_mode: u8,
}
pub fn supports_page(pid: u32, key: &str) -> bool {
    pid == spec().product_id && key == spec().page
}
impl Automation {
    pub fn new(device: &Device, _: &mut Window, _: &mut Context<Self>) -> Self {
        Self::empty(device.edition_id, device.layout_id)
    }
    fn empty(edition: u32, layout: u32) -> Self {
        Self {
            rules: vec![],
            preview: false,
            catalogs: BTreeMap::new(),
            chroma_installed: None,
            chroma_busy: false,
            edition,
            layout,
            alert: None,
            editor: None,
            subscriptions: vec![],
            audio_mode: 0,
        }
    }
    pub fn snapshot(&self) -> Value {
        json!({"automationWidget":self.rules})
    }
    pub fn restore(&mut self, saved: Option<&Value>, _: &mut Window, cx: &mut Context<Self>) {
        self.rules.clear();
        if let Some(rules) = saved
            .and_then(|v| v.get("automationWidget"))
            .and_then(|v| serde_json::from_value::<Vec<Rule>>(v.clone()).ok())
        {
            for mut rule in rules {
                if rule.id > 5 || self.rules.iter().any(|r| r.id == rule.id) || !rule.valid() {
                    continue;
                }
                rule.name = action_name(rule.id).into();
                rule.pick_up.data.truncate(1);
                rule.put_down.data.truncate(1);
                if rule.id == 5 {
                    for value in rule.pick_up.data.iter().chain(rule.put_down.data.iter()) {
                        let id = value.get("id").and_then(Value::as_str).unwrap_or_default();
                        if !id.is_empty()
                            && !self.catalogs.entry(5).or_default().iter().any(|existing| {
                                existing.get("id").and_then(Value::as_str) == Some(id)
                            })
                        {
                            self.catalogs.entry(5).or_default().push(value.clone());
                        }
                    }
                }
                self.rules.push(rule);
            }
        }
        self.editor = None;
        self.subscriptions.clear();
        cx.notify();
    }
    pub fn dismiss(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.editor = None;
        self.subscriptions.clear();
        cx.notify();
    }
    fn changed(&mut self, cx: &mut Context<Self>) {
        if !self.preview {
            cx.emit(AutomationChanged);
        }
        cx.notify();
    }
    fn open(&mut self, id: Option<u32>, window: &mut Window, cx: &mut Context<Self>) {
        let available = if let Some(id) = id {
            vec![id]
        } else {
            (0..6)
                .filter(|id| !self.rules.iter().any(|r| r.id == *id))
                .collect::<Vec<_>>()
        };
        let Some(first) = available.first().copied() else {
            return;
        };
        let rule = id
            .and_then(|id| self.rules.iter().find(|r| r.id == id))
            .cloned()
            .unwrap_or_else(|| Rule::new(first));
        let editor = cx.new(|cx| {
            AutomationEditor::new(
                rule,
                id.is_some(),
                available,
                self.catalogs.clone(),
                self.chroma_installed,
                self.chroma_busy,
                self.preview,
                window,
                cx,
            )
        });
        self.subscriptions.clear();
        self.subscriptions.push(cx.subscribe(
            &editor,
            |this, _, event: &editor::EditorEvent, cx| match event {
                editor::EditorEvent::Save(rule) => {
                    if rule.id == 5 {
                        for value in rule.pick_up.data.iter().chain(rule.put_down.data.iter()) {
                            let id = value.get("id").and_then(Value::as_str).unwrap_or_default();
                            if !id.is_empty()
                                && !this.catalogs.entry(5).or_default().iter().any(|existing| {
                                    existing.get("id").and_then(Value::as_str) == Some(id)
                                })
                            {
                                this.catalogs.entry(5).or_default().push(value.clone());
                            }
                        }
                    }
                    this.rules.retain(|r| r.id != rule.id);
                    this.rules.push(rule.clone());
                    this.rules.sort_by_key(|r| r.id);
                    this.changed(cx);
                }
                editor::EditorEvent::Delete(id) => {
                    this.rules.retain(|r| r.id != *id);
                    this.changed(cx);
                }
            },
        ));
        self.editor = Some(editor.clone());
        window.open_dialog(cx, move |dialog, window, _| {
            dialog
                .w(window.rem_size() * (850. / 16.))
                .margin_top(window.rem_size() * (109. / 16.))
                .close_button(false)
                .overlay_closable(false)
                .p_0()
                .bg(Colors::panel())
                .rounded(surface::css(3.))
                .child(editor.clone())
        });
    }
    fn summary(rule: &Rule) -> String {
        let lane = if rule.pick_up.is_enabled {
            &rule.pick_up
        } else {
            &rule.put_down
        };
        let Some(data) = lane.data.first() else {
            return "-".into();
        };
        if data["id"] == "restore" {
            return text("RESTORE_PREVIOUS_SETTINGS");
        }
        if data["isOff"] == true {
            return text("CHROMA_LIGHTING_OFF");
        }
        if data["pauseGame"] == true {
            return text("PAUSE_GAME_AUTO");
        }
        if rule.id == 0 && !data["isAdvEffect"].as_bool().unwrap_or(false) {
            return spec()
                .effects
                .iter()
                .find(|e| Some(e.id as u64) == data["selectedEffectId"].as_u64())
                .map(|e| text(&e.name))
                .unwrap_or_else(|| "-".into());
        }
        data["name"]
            .as_str()
            .or_else(|| data["assignmentValue"].as_str())
            .unwrap_or("-")
            .into()
    }
}
impl Render for Automation {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut widget = v_flex()
            .w(surface::css(600.))
            .px(surface::css(40.))
            .pt(surface::css(30.))
            .pb(surface::css(21.))
            .my(surface::css(10.))
            .rounded(surface::css(5.))
            .bg(Colors::panel())
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .text_color(Colors::foreground())
            .child(
                h_flex()
                    .justify_between()
                    .child(
                        div()
                            .font_family("RazerF5")
                            .text_size(surface::css(16.))
                            .text_color(Colors::primary())
                            .child(text("AUTOMATIONS").to_uppercase()),
                    )
                    .child(
                        Button::new("automation-help")
                            .ghost()
                            .small()
                            .child(
                                img("synapse/automation-tooltip_questionmark.svg")
                                    .size(surface::css(14.)),
                            )
                            .accessibility_label(text("AUTOMATION_TIPS"))
                            .tooltip(text("AUTOMATION_TIPS")),
                    ),
            )
            .child(
                div()
                    .mt(surface::css(15.))
                    .mb(surface::css(10.))
                    .child(text("AUTOMATION_DESC")),
            );
        for rule in &self.rules {
            let id = rule.id;
            widget = widget.child(
                h_flex()
                    .w(surface::css(520.))
                    .h(surface::css(64.))
                    .my(surface::css(5.))
                    .px(surface::css(20.))
                    .rounded(surface::css(5.))
                    .bg(Colors::row())
                    .child(
                        gpui_kit::base::Button::new(("automation-edit", id))
                            .accessibility_label(text("EDIT_AUTOMATION"))
                            .flex()
                            .items_center()
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .hover(|s| s.bg(Colors::row_hover()))
                            .active(|s| s.bg(Colors::row_active()))
                            .child(icon(id))
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .pl(surface::css(20.))
                                    .gap(surface::css(2.))
                                    .opacity(if rule.is_enabled { 1. } else { 0.5 })
                                    .child(
                                        div()
                                            .text_color(Colors::muted())
                                            .child(text(&spec().actions[id as usize].content)),
                                    )
                                    .child(div().text_ellipsis().child(Self::summary(rule))),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.open(Some(id), window, cx)
                            })),
                    )
                    .child(
                        surface::SynapseSwitch::new(SharedString::from(format!(
                            "automation-toggle-{id}"
                        )))
                        .checked(rule.is_enabled)
                        .accessibility_label(text(&spec().actions[id as usize].content))
                        .on_change(cx.listener(
                            move |this, value, _, cx| {
                                if let Some(rule) = this.rules.iter_mut().find(|r| r.id == id) {
                                    rule.is_enabled = *value;
                                }
                                this.changed(cx);
                            },
                        )),
                    ),
            );
        }
        if self.rules.len() < 6 {
            widget = widget.child(
                gpui_kit::base::Button::new("automation-add")
                    .accessibility_label(text("ADD_AUTOMATION"))
                    .h(surface::css(60.))
                    .w(surface::css(520.))
                    .my(surface::css(5.))
                    .rounded(surface::css(5.))
                    .border_1()
                    .border_color(Colors::border())
                    .flex()
                    .items_center()
                    .justify_center()
                    .hover(|s| s.border_color(Colors::primary()))
                    .child(
                        img("synapse/automation-icon_add_light_grey.svg").size(surface::css(15.)),
                    )
                    .on_click(cx.listener(|this, _, window, cx| this.open(None, window, cx))),
            );
        }
        v_flex()
            .w_full()
            .child(surface::product_banner(3946, self.edition, self.layout, cx))
            .child(
                surface::page_columns()
                    .child(surface::page_column(widget))
                    .child(surface::page_column(self.render_launch_sound(cx))),
            )
            .when_some(self.alert.clone(), |v, text| {
                v.child(surface::note(text, cx))
            })
    }
}
