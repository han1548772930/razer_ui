//! Current product 3884/3886 ARGB controller pages. Hardware status is session-only;
//! editable layout preferences are local drafts, independently of discovery.
use super::Choice;
use crate::{i18n, model::Device, ui::surface};
use gpui_kit::base::{NumberInput, StepAction, step_value};
use gpui_kit::component::{
    button::{Button, ButtonVariants},
    input::{Input, InputEvent, InputState},
    select::{SelectEvent, SelectState},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::OnceLock};

mod preview;
mod state;
mod theme;
pub(crate) use preview::open_preview;
use state::{Observation, Port};
use theme::Colors;

#[derive(Deserialize)]
struct Spec {
    product_id: u32,
    page: String,
    translations: BTreeMap<String, BTreeMap<String, String>>,
    fan_values: Vec<u32>,
}
fn spec(pid: u32) -> &'static Spec {
    static DATA: OnceLock<Vec<Spec>> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("wireless_argb_data.json")).expect("audited ARGB sources")
    })
    .iter()
    .find(|s| s.product_id == pid)
    .expect("supported ARGB product")
}
impl Spec {
    fn text(&self, key: &str) -> String {
        self.translations
            .get(&i18n::locale())
            .and_then(|v| v.get(key))
            .or_else(|| self.translations.get("en").and_then(|v| v.get(key)))
            .cloned()
            .unwrap_or_else(|| i18n::t(key))
    }
    fn minimum(&self) -> u32 {
        if self.product_id == 3884 { 1 } else { 4 }
    }
    fn asset(&self, name: &str) -> SharedString {
        format!("synapse/wireless-argb-{}-{name}.png", self.product_id).into()
    }
    fn icon(&self, name: &str) -> Img {
        img(SharedString::from(format!(
            "synapse/wireless-argb-{}-{name}.svg",
            self.product_id
        )))
        .size(surface::css(20.))
    }
    fn mode_choices(&self) -> Vec<Choice> {
        vec![
            Choice::new("strip", self.text("TEXT_LED_STRIP")),
            Choice::new("fan", self.text("TEXT_FAN")),
        ]
    }
    fn fan_choices(&self) -> Vec<Choice> {
        self.fan_values
            .iter()
            .map(|v| Choice::new(v.to_string(), v.to_string()))
            .collect()
    }
}
pub(crate) fn supports_page(pid: u32, key: &str) -> bool {
    matches!(pid, 3884 | 3886) && spec(pid).page == key
}
pub(crate) struct WirelessArgbChanged;
impl EventEmitter<WirelessArgbChanged> for WirelessArgb {}

pub(crate) struct WirelessArgb {
    spec: &'static Spec,
    ports: Vec<Port>,
    observation: Observation,
    active_ports: Vec<u32>,
    detected: BTreeMap<u32, u32>,
    auto_detection: bool,
    max_leds: u32,
    limit_dismissed: bool,
    preview: bool,
    reveal_editor: bool,
    alert: Option<String>,
    last_request: Option<String>,
    names: BTreeMap<u32, Entity<InputState>>,
    modes: BTreeMap<u32, Entity<SelectState<Vec<Choice>>>>,
    counts: BTreeMap<(u32, u32), Entity<InputState>>,
    fans: BTreeMap<(u32, u32), Entity<SelectState<Vec<Choice>>>>,
    editing_name: Option<u32>,
    subscriptions: Vec<Subscription>,
    syncing: bool,
    next_id: u32,
}
impl WirelessArgb {
    pub(crate) fn new(device: &Device, window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::for_product(device.product_id, window, cx)
    }
    fn for_product(pid: u32, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let spec = spec(pid);
        let mut this = Self {
            spec,
            ports: (1..=3)
                .map(|id| Port::new(id, format!("{} {id}", spec.text("TEXT_PORT"))))
                .collect(),
            observation: Observation::Unavailable,
            active_ports: vec![],
            detected: BTreeMap::new(),
            auto_detection: false,
            max_leds: 80,
            limit_dismissed: false,
            preview: false,
            reveal_editor: false,
            alert: None,
            last_request: None,
            names: BTreeMap::new(),
            modes: BTreeMap::new(),
            counts: BTreeMap::new(),
            fans: BTreeMap::new(),
            editing_name: None,
            subscriptions: vec![],
            syncing: false,
            next_id: 1,
        };
        this.rebuild(window, cx);
        this
    }
    pub(crate) fn snapshot(&self) -> Value {
        json!({"ports":self.ports})
    }
    pub(crate) fn restore(
        &mut self,
        saved: Option<&Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.ports = (1..=3)
            .map(|id| Port::new(id, format!("{} {id}", self.spec.text("TEXT_PORT"))))
            .collect();
        if let Some(saved) = saved
            .and_then(|s| s.get("ports"))
            .and_then(|v| serde_json::from_value::<Vec<Port>>(v.clone()).ok())
        {
            for port in &mut self.ports {
                if let Some(candidate) = saved.iter().find(|p| p.id == port.id) {
                    let valid = !candidate.name.trim().is_empty()
                        && candidate.name.chars().count() <= 128
                        && !candidate.strip.is_empty()
                        && candidate.strip.len() <= 4
                        && !candidate.fan.is_empty()
                        && candidate.fan.len() <= 16
                        && candidate
                            .strip
                            .iter()
                            .all(|s| s.value >= self.spec.minimum() && s.value <= 240)
                        && candidate
                            .fan
                            .iter()
                            .all(|s| self.spec.fan_values.contains(&s.value));
                    if valid {
                        *port = candidate.clone();
                        port.dismissed = false;
                        if self.spec.product_id == 3884 {
                            port.name = port.name.chars().take(32).collect();
                        } else {
                            port.fan.truncate(1);
                        }
                    }
                }
            }
        }
        // IDs are local retained identities; never trust duplicates from a profile.
        self.next_id = 1;
        for port in &mut self.ports {
            for segment in port.strip.iter_mut().chain(&mut port.fan) {
                segment.id = self.next_id;
                self.next_id += 1;
            }
        }
        self.editing_name = None;
        self.rebuild(window, cx);
        cx.notify();
    }
    pub(crate) fn dismiss(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        self.editing_name = None;
        self.alert = None;
        cx.notify();
    }
    fn changed(&mut self, cx: &mut Context<Self>) {
        if !self.preview {
            cx.emit(WirelessArgbChanged);
        }
        cx.notify();
    }
    fn editable(&self) -> bool {
        self.preview && self.observation.ports_visible()
    }
    fn maximum(&self, port: u32) -> u32 {
        if self.spec.product_id == 3884 {
            self.max_leds
        } else {
            self.detected.get(&port).copied().unwrap_or(0).max(40)
        }
    }
    fn rebuild(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.subscriptions.clear();
        self.names.clear();
        self.modes.clear();
        self.counts.clear();
        self.fans.clear();
        for port in self.ports.clone() {
            let id = port.id;
            let name = cx.new(|cx| InputState::new(window, cx).default_value(port.name));
            self.subscriptions.push(cx.subscribe_in(
                &name,
                window,
                move |this, input, event, window, cx| {
                    if !this.syncing
                        && matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. })
                    {
                        let value = input.read(cx).value().to_string();
                        this.rename(id, value, window, cx);
                    }
                },
            ));
            self.names.insert(id, name);
            let choices = self.spec.mode_choices();
            let mode = cx.new(|cx| SelectState::new(choices, None, window, cx));
            self.subscriptions.push(cx.subscribe_in(
                &mode,
                window,
                move |this, _, event, window, cx| {
                    if !this.syncing && this.editable() {
                        if let SelectEvent::Confirm(Some(value)) = event {
                            if let Some(port) = this.ports.iter_mut().find(|p| p.id == id) {
                                port.strip_mode = value == "strip";
                                port.dismissed = false;
                            }
                            this.sync(window, cx);
                            this.changed(cx);
                        }
                    }
                },
            ));
            self.modes.insert(id, mode);
            for segment in port.strip {
                let sid = segment.id;
                let input = cx
                    .new(|cx| InputState::new(window, cx).default_value(segment.value.to_string()));
                self.subscriptions.push(cx.subscribe_in(
                    &input,
                    window,
                    move |this, input, event, window, cx| {
                        if !this.syncing
                            && matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. })
                        {
                            if let Ok(value) = input.read(cx).value().parse::<u32>() {
                                this.set_leds(id, sid, value, window, cx);
                            } else {
                                this.sync(window, cx);
                            }
                        }
                    },
                ));
                self.counts.insert((id, sid), input);
            }
            for segment in port.fan {
                let sid = segment.id;
                let choices = self.spec.fan_choices();
                let select = cx.new(|cx| SelectState::new(choices, None, window, cx));
                self.subscriptions.push(cx.subscribe_in(
                    &select,
                    window,
                    move |this, _, event, window, cx| {
                        if !this.syncing {
                            if let SelectEvent::Confirm(Some(value)) = event {
                                if let Ok(value) = value.parse::<u32>() {
                                    this.set_leds(id, sid, value, window, cx);
                                }
                            }
                        }
                    },
                ));
                self.fans.insert((id, sid), select);
            }
        }
        self.sync(window, cx);
    }
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.syncing = true;
        for port in &self.ports {
            self.names[&port.id].update(cx, |s, cx| s.set_value(port.name.clone(), window, cx));
            self.modes[&port.id].update(cx, |s, cx| {
                s.set_selected_value(
                    &if port.strip_mode { "strip" } else { "fan" }.to_string(),
                    window,
                    cx,
                )
            });
            for segment in &port.strip {
                if let Some(input) = self.counts.get(&(port.id, segment.id)) {
                    input.update(cx, |s, cx| {
                        s.set_value(segment.value.to_string(), window, cx)
                    });
                }
            }
            for segment in &port.fan {
                if let Some(select) = self.fans.get(&(port.id, segment.id)) {
                    select.update(cx, |s, cx| {
                        s.set_selected_value(&segment.value.to_string(), window, cx)
                    });
                }
            }
        }
        self.syncing = false;
    }
    fn rename(&mut self, id: u32, value: String, window: &mut Window, cx: &mut Context<Self>) {
        if !self.editable() || self.editing_name != Some(id) {
            return;
        }
        if !value.trim().is_empty() {
            let max = if self.spec.product_id == 3884 {
                32
            } else {
                128
            };
            if let Some(port) = self.ports.iter_mut().find(|p| p.id == id) {
                port.name = value.chars().take(max).collect();
            }
        }
        self.editing_name = None;
        self.sync(window, cx);
        self.changed(cx);
    }
    fn set_leds(
        &mut self,
        id: u32,
        sid: u32,
        value: u32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.editable() {
            return;
        }
        let minimum = self.spec.minimum();
        let maximum = self.maximum(id);
        if let Some(port) = self.ports.iter_mut().find(|p| p.id == id) {
            if port.strip_mode {
                port.set_leds(sid, value, minimum, maximum);
            } else if self.spec.fan_values.contains(&value) {
                port.set_leds(sid, value, 1, maximum);
            }
        }
        self.sync(window, cx);
        self.changed(cx);
    }
    fn step_leds(
        &mut self,
        id: u32,
        sid: u32,
        action: StepAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(input) = self.counts.get(&(id, sid)) else {
            return;
        };
        let draft = input.read(cx).value();
        let Some(value) = step_value(
            &draft,
            action,
            1.,
            Some(self.spec.minimum() as f64),
            Some(self.maximum(id) as f64),
        )
        .and_then(|v| v.parse::<u32>().ok()) else {
            return;
        };
        self.set_leds(id, sid, value, window, cx);
    }
    fn add(&mut self, id: u32, window: &mut Window, cx: &mut Context<Self>) {
        if !self.editable() {
            return;
        }
        if let Some(port) = self.ports.iter_mut().find(|p| p.id == id) {
            if port.strip_mode && port.strip.len() >= 4 {
                return;
            }
            if !port.strip_mode && (self.spec.product_id != 3884 || port.total() >= self.max_leds) {
                return;
            }
            port.add(self.spec.minimum(), self.next_id);
            self.next_id += 1;
        }
        self.rebuild(window, cx);
        self.changed(cx);
    }
    fn remove(&mut self, id: u32, sid: u32, window: &mut Window, cx: &mut Context<Self>) {
        if !self.editable() {
            return;
        }
        if let Some(port) = self.ports.iter_mut().find(|p| p.id == id) {
            let values = port.segments_mut();
            if values.len() > 1 && values[0].id != sid {
                values.retain(|s| s.id != sid);
            }
            port.dismissed = false;
        }
        self.rebuild(window, cx);
        self.changed(cx);
    }
    fn request(&mut self, kind: &str, cx: &mut Context<Self>) {
        // No service adapter is present: never turn a request into an observation.
        if self.preview {
            self.last_request = Some(kind.into());
        } else {
            self.alert = Some("暂时无法读取控制器状态，请连接设备后重试。".into());
        }
        cx.notify();
    }
    fn render_product(&self, cx: &Context<Self>) -> AnyElement {
        let visible = self.observation.power_on() && self.observation != Observation::Protection;
        let product = div()
            .relative()
            .w(surface::css(260.))
            .h(surface::css(275.))
            .child(
                img(self.spec.asset("prd-3x"))
                    .size(surface::css(260.))
                    .object_fit(ObjectFit::Contain),
            )
            .when(self.observation == Observation::Detecting, |view| {
                view.child(
                    img(SharedString::from(format!(
                        "synapse/wireless-argb-{}-detecting.svg",
                        self.spec.product_id
                    )))
                    .absolute()
                    .top(surface::css(33.))
                    .left(surface::css(20.))
                    .w(surface::css(220.))
                    .h(surface::css(115.)),
                )
            })
            .child(
                v_flex()
                    .absolute()
                    .bottom(surface::css(127.))
                    .left_0()
                    .w_full()
                    .h(surface::css(115.))
                    .justify_between()
                    .when(self.observation == Observation::Detecting, |view| {
                        view.invisible()
                    })
                    .child(
                        h_flex()
                            .justify_center()
                            .gap(surface::css(20.))
                            .when(!visible, |s| s.invisible())
                            .child(
                                Button::new("argb-auto")
                                    .ghost()
                                    .small()
                                    .child(self.spec.icon(if self.auto_detection {
                                        "auto_on"
                                    } else {
                                        "auto_off"
                                    }))
                                    .selected(self.auto_detection)
                                    .accessibility_label(
                                        self.spec.text("GLITTER_MESSAGE_AUTO_DETECTION"),
                                    )
                                    .tooltip(self.spec.text("GLITTER_MESSAGE_AUTO_DETECTION"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.request("CHANGE_AUTO_DETECTION_STATUS", cx)
                                    })),
                            )
                            .child(
                                Button::new("argb-refresh")
                                    .ghost()
                                    .small()
                                    .child(self.spec.icon("refresh"))
                                    .accessibility_label(
                                        self.spec.text("GLITTER_MESSAGE_REFRESH_ICON"),
                                    )
                                    .tooltip(self.spec.text("GLITTER_MESSAGE_REFRESH_ICON"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.request("REFRESH_PORTS", cx)
                                    })),
                            ),
                    )
                    .child(
                        h_flex().justify_center().child(
                            Button::new("argb-power")
                                .ghost()
                                .small()
                                .child(self.spec.icon(if self.observation.power_on() {
                                    "power"
                                } else {
                                    "power_off"
                                }))
                                .selected(self.observation.power_on())
                                .accessibility_label(
                                    self.spec.text("GLITTER_MESSAGE_STAND_BY_MODE"),
                                )
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.request("CHANGE_POWER_STATUS", cx)
                                })),
                        ),
                    ),
            );
        h_flex()
            .justify_center()
            .m(surface::css(10.))
            .child(product)
            .into_any_element()
    }
    fn render_status(&self, cx: &Context<Self>) -> AnyElement {
        let old = self.spec.product_id == 3886;
        let observation = if !old
            && self.observation == Observation::Ready
            && !self.limit_dismissed
            && self
                .ports
                .iter()
                .filter(|p| self.active_ports.contains(&p.id))
                .map(Port::total)
                .sum::<u32>()
                > 240
        {
            Observation::LedLimit
        } else {
            self.observation
        };
        let (title, body) = match observation {
            Observation::Unavailable => {
                return h_flex()
                    .justify_center()
                    .child(surface::note("暂时无法读取控制器状态。", cx))
                    .into_any_element();
            }
            Observation::Detecting => {
                return h_flex()
                    .justify_center()
                    .child(surface::note("正在检测 ARGB 设备…", cx))
                    .into_any_element();
            }
            Observation::Empty => (
                if old {
                    "GLITTER_NO_DEVICES_DETECT"
                } else {
                    "GLITTER_NO_DEVICES_DETECTED"
                },
                if old {
                    "GLITTER_NO_DEVICES_DETECT_MSG"
                } else {
                    "GLITTER_NO_DEVICES_DETECTED_MSG"
                },
            ),
            Observation::Standby => (
                "GLITTER_MESSAGE_STAND_BY_MODE",
                "GLITTER_MESSAGE_STAND_BY_MODE_MSG",
            ),
            Observation::Mobile => ("", "GLITTER_MESSAGE_STOP_MOBILE_SYNC"),
            Observation::Bluetooth => (
                "GLITTER_MESSAGE_BLUETOOTH_MODE",
                "GLITTER_MESSAGE_BLUETOOTH_MODE_MSG",
            ),
            Observation::DcRequired => (
                "GLITTER_CONNECTION_REQUIRED",
                "GLITTER_CONNECTION_REQUIRED_MSG",
            ),
            Observation::Protection => (
                "GLITTER_PROTECTION_ENABLED",
                "GLITTER_PROTECTION_ENABLED_MSG_0",
            ),
            Observation::LedLimit => (
                if old {
                    "GLITTER_LIMIT_EXCEEDED"
                } else {
                    "GLITTER_LED_LIMIT_EXCEEDED"
                },
                if old {
                    "GLITTER_LIMIT_EXCEEDED_MSG"
                } else {
                    "GLITTER_LED_LIMIT_EXCEEDED_MSG"
                },
            ),
            Observation::Ready => return div().into_any_element(),
        };
        let mut card = v_flex()
            .w(surface::css(600.))
            .p(surface::css(20.))
            .gap(surface::css(10.))
            .rounded(surface::css(5.))
            .bg(Colors::panel())
            .border_1()
            .border_color(
                if matches!(
                    self.observation,
                    Observation::Mobile | Observation::Bluetooth
                ) {
                    Colors::message_border()
                } else {
                    Colors::warning()
                },
            )
            .text_center();
        if !title.is_empty() {
            card = card.child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.spec.text(title)),
            );
        }
        card = card.child(div().px(surface::css(40.)).child(self.spec.text(body)));
        if self.observation == Observation::Protection {
            for ix in 1..=4 {
                card = card.child(div().text_left().px(surface::css(40.)).child(format!(
                        "{ix}. {}",
                        self.spec
                            .text(&format!("GLITTER_PROTECTION_ENABLED_MSG_{ix}"))
                    )));
            }
        }
        if self.observation == Observation::Mobile {
            card = card.child(
                Button::new("argb-connect")
                    .label(self.spec.text("TEXT_CONNECT"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.request("CHANGE_CONNECTION_MODE:2.4G", cx)
                    })),
            );
        }
        if observation == Observation::LedLimit {
            card = card.child(
                Button::new("argb-dismiss-limit")
                    .ghost()
                    .label(self.spec.text("DISMISS"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.preview {
                            this.observation = Observation::Ready;
                            this.limit_dismissed = true;
                        }
                        cx.notify();
                    })),
            );
        }
        h_flex().justify_center().child(card).into_any_element()
    }
    fn render_port(&self, port: &Port, cx: &Context<Self>) -> AnyElement {
        let id = port.id;
        let mut card = v_flex()
            .w(surface::css(600.))
            .min_h(surface::css(199.))
            .px(surface::css(40.))
            .pt(surface::css(26.))
            .pb(surface::css(30.))
            .rounded(surface::css(5.))
            .bg(Colors::panel());
        let name = if self.editing_name == Some(id) {
            Input::new(&self.names[&id])
                .w(surface::css(200.))
                .into_any_element()
        } else {
            Button::new(("argb-rename", id))
                .ghost()
                .small()
                .label(port.name.clone())
                .child(self.spec.icon("rename"))
                .font_family("RazerF5")
                .text_size(surface::css(16.))
                .text_color(Colors::primary())
                .on_click(cx.listener(move |this, _, window, cx| {
                    if this.editable() {
                        this.editing_name = Some(id);
                        this.names[&id].update(cx, |s, cx| s.focus(window, cx));
                        cx.notify();
                    }
                }))
                .into_any_element()
        };
        card = card.child(
            h_flex().justify_between().child(name).child(
                Button::new(("argb-help", id))
                    .ghost()
                    .small()
                    .child(img("synapse/help-default.svg").size(surface::css(20.)))
                    .accessibility_label(self.spec.text("GLITTER_TIP_HELP_PORT_MESSAGE"))
                    .tooltip(self.spec.text("GLITTER_TIP_HELP_PORT_MESSAGE")),
            ),
        );
        let mut rows = v_flex().flex_1().min_w_0();
        for (ix, segment) in port.segments().iter().enumerate() {
            let sid = segment.id;
            let device = if ix == 0 {
                surface::select(&self.modes[&id])
                    .items(self.spec.mode_choices())
                    .accessibility_label(self.spec.text("TEXT_DEVICE_TYPE"))
                    .w(surface::css(150.))
                    .into_any_element()
            } else {
                div()
                    .w(surface::css(150.))
                    .child(format!(
                        "{} {}",
                        self.spec.text(if port.strip_mode {
                            "TEXT_LED_STRIP"
                        } else {
                            "TEXT_FAN"
                        }),
                        ix + 1
                    ))
                    .into_any_element()
            };
            let count = if port.strip_mode {
                let owner = cx.entity().downgrade();
                div()
                    .w(surface::css(70.))
                    .h(surface::css(27.))
                    .border_1()
                    .border_color(Colors::border())
                    .child(
                        NumberInput::new(&self.counts[&(id, sid)])
                            .size_full()
                            .controls_right()
                            .input(
                                Input::new(&self.counts[&(id, sid)])
                                    .appearance(false)
                                    .bordered(false)
                                    .focus_bordered(false)
                                    .h_full()
                                    .pl_1()
                                    .text_sm(),
                            )
                            .on_step(move |action, window, cx| {
                                let _ = owner.update(cx, |this, cx| {
                                    this.step_leds(id, sid, action, window, cx)
                                });
                            }),
                    )
                    .into_any_element()
            } else {
                surface::select(&self.fans[&(id, sid)])
                    .items(self.spec.fan_choices())
                    .accessibility_label(self.spec.text("GLITTER_NO_OF_LED"))
                    .w(surface::css(70.))
                    .into_any_element()
            };
            if ix == 0 {
                rows = rows.child(
                    h_flex()
                        .gap(surface::css(20.))
                        .mb(surface::css(10.))
                        .child(
                            div()
                                .w(surface::css(150.))
                                .child(self.spec.text("TEXT_DEVICE_TYPE")),
                        )
                        .child(self.spec.text("GLITTER_NO_OF_LED")),
                );
            }
            rows = rows.child(
                h_flex()
                    .gap(surface::css(20.))
                    .my(surface::css(5.))
                    .child(device)
                    .child(count)
                    .when(ix > 0, |row| {
                        row.child(
                            Button::new(SharedString::from(format!("argb-remove-{id}-{sid}")))
                                .ghost()
                                .small()
                                .child(self.spec.icon("remove"))
                                .accessibility_label(i18n::t("REMOVE"))
                                .on_click(cx.listener(move |this, _, window, cx| {
                                    this.remove(id, sid, window, cx)
                                })),
                        )
                    }),
            );
        }
        if (port.strip_mode && port.strip.len() < 4)
            || (!port.strip_mode && self.spec.product_id == 3884)
        {
            rows = rows.child(
                Button::new(("argb-add", id))
                    .ghost()
                    .small()
                    .label(self.spec.text(if port.strip_mode {
                        "GLITTER_ADD_BEND"
                    } else {
                        "GLITTER_ADD_FAN"
                    }))
                    .disabled(!port.strip_mode && port.total() >= self.max_leds)
                    .on_click(cx.listener(move |this, _, window, cx| this.add(id, window, cx))),
            );
        }
        let detected = self.detected.get(&id).copied().unwrap_or(0);
        if port.segments().len() > 1 {
            rows = rows.child(
                h_flex()
                    .gap(surface::css(10.))
                    .h(surface::css(27.))
                    .child(
                        div()
                            .w(surface::css(150.))
                            .child(self.spec.text("GLITTER_TOTAL_LED_COUNT")),
                    )
                    .child(port.total().to_string()),
            );
        }
        rows = rows.child(
            Button::new(("argb-detected", id))
                .ghost()
                .small()
                .label(format!("{detected} LED"))
                .tooltip(
                    self.spec
                        .text("GLITTER_DETECTED_LED_COUNT")
                        .replace("{{ledCount}}", &detected.to_string()),
                ),
        );
        let asset = if port.strip_mode {
            format!("strip-{}", port.strip.len())
        } else {
            "fan".into()
        };
        card = card.child(
            h_flex()
                .items_start()
                .mt(surface::css(11.))
                .gap(surface::css(20.))
                .child(rows)
                .child(
                    img(self.spec.asset(&asset))
                        .size(surface::css(100.))
                        .flex_shrink_0(),
                ),
        );
        if !port.dismissed && port.total() != detected {
            card = card.child(
                v_flex()
                    .mt(surface::css(10.))
                    .p(surface::css(20.))
                    .border_1()
                    .border_color(Colors::notice_border())
                    .rounded(surface::css(5.))
                    .text_size(surface::css(13.))
                    .child(self.spec.text("GLITTER_MESSAGE_CHROMA_STUDIO"))
                    .child(
                        h_flex()
                            .justify_between()
                            .mt(surface::css(10.))
                            .child(
                                Button::new(("argb-chroma", id))
                                    .outline()
                                    .small()
                                    .label(self.spec.text("DASHBOARD_CHROMA_STUDIO"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.request("LAUNCH_CHROMA_STUDIO", cx)
                                    })),
                            )
                            .child(
                                Button::new(("argb-dismiss", id))
                                    .ghost()
                                    .small()
                                    .label(self.spec.text("DISMISS"))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        if let Some(p) = this.ports.iter_mut().find(|p| p.id == id)
                                        {
                                            p.dismissed = true;
                                        }
                                        cx.notify();
                                    })),
                            ),
                    ),
            );
        }
        let limit = if self.spec.product_id == 3884 {
            self.max_leds
        } else {
            120
        };
        if port.total() > limit {
            card = card.child(
                div()
                    .mt(surface::css(10.))
                    .text_color(Colors::warning())
                    .child(if self.spec.product_id == 3884 {
                        format!(
                            "{} {}",
                            self.spec
                                .text("GLITTER_MESSAGE_EXCEEDED_WARNING_1")
                                .replace("{{maxLed}}", &limit.to_string()),
                            self.spec.text("GLITTER_MESSAGE_EXCEEDED_WARNING_2")
                        )
                    } else {
                        self.spec.text("GLITTER_MESSAGE_EXCEEDED_WARNING")
                    }),
            );
        }
        card.into_any_element()
    }
}
impl Render for WirelessArgb {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let show_ports = self.observation.ports_visible()
            && (self.spec.product_id == 3884 || self.reveal_editor);
        v_flex()
            .w_full()
            .min_w(surface::css(620.))
            .font_family("Roboto")
            .text_color(Colors::foreground())
            .text_size(surface::css(14.))
            .child(self.render_product(cx))
            .child(self.render_status(cx))
            .when(show_ports, |view| {
                view.child(
                    h_flex()
                        .items_start()
                        .justify_center()
                        .flex_wrap()
                        .gap(surface::css(20.))
                        .mt(surface::css(20.))
                        .children(
                            self.ports
                                .iter()
                                .filter(|p| self.active_ports.contains(&p.id))
                                .map(|port| self.render_port(port, cx)),
                        ),
                )
            })
            .when_some(self.alert.clone(), |view, alert| {
                view.child(
                    h_flex()
                        .justify_center()
                        .mt_3()
                        .child(surface::note(alert, cx)),
                )
            })
    }
}
