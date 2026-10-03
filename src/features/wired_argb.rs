//! Current 778/3871 ARGB layout pages. Port observations live only in this
//! session; saved drafts cannot create connected hardware or detection results.
use super::Choice;
use crate::{i18n, model::Device, ui::surface};
use gpui_kit::component::button::ButtonVariants;
use gpui_kit::component::*;
use gpui_kit::{prelude::FluentBuilder as _, *};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::OnceLock};

mod loading;
mod port;
mod preview;
mod state;
mod theme;
use port::{PortChanged, PortEditor};
pub(crate) use preview::open_preview;
use state::{PortDraft, PortObservation, Status};
use theme::Colors;

#[derive(Deserialize)]
struct Spec {
    product_id: u32,
    page: String,
    name: String,
    minimum_leds: u32,
    fan_counts: Vec<u32>,
    mainboard_ports: Vec<u32>,
    translations: BTreeMap<String, BTreeMap<String, String>>,
    assets: Vec<Asset>,
}
#[derive(Deserialize)]
struct Asset {
    name: String,
    output: String,
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
    fn asset(&self, name: &str) -> SharedString {
        self.assets
            .iter()
            .find(|a| a.name == name)
            .map(|a| a.output.trim_start_matches("assets/").to_owned())
            .unwrap_or_default()
            .into()
    }
    fn mainboard(&self) -> bool {
        self.product_id == 778
    }
}
fn spec(pid: u32) -> &'static Spec {
    static SPECS: OnceLock<Vec<Spec>> = OnceLock::new();
    SPECS
        .get_or_init(|| {
            serde_json::from_str(include_str!("wired_argb_data.json"))
                .expect("audited ARGB sources")
        })
        .iter()
        .find(|spec| spec.product_id == pid)
        .expect("supported ARGB product")
}
pub(crate) fn supports_page(pid: u32, key: &str) -> bool {
    matches!(pid, 778 | 3871) && spec(pid).page == key
}
#[derive(Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Draft {
    ports: BTreeMap<u32, PortDraft>,
    auto_detection: Option<bool>,
}
pub(crate) struct WiredArgbChanged;
pub(crate) struct WiredArgbWorkspace {
    spec: &'static Spec,
    edition: u32,
    draft: Draft,
    observations: Vec<PortObservation>,
    ports: BTreeMap<u32, Entity<PortEditor>>,
    status: Status,
    auto_detection: Option<bool>,
    preview: bool,
    limit_dismissed: bool,
    last_request: Option<String>,
    subscriptions: Vec<Subscription>,
}
impl EventEmitter<WiredArgbChanged> for WiredArgbWorkspace {}
impl WiredArgbWorkspace {
    pub(crate) fn new(device: &Device, _: &mut Window, _: &mut Context<Self>) -> Self {
        Self::empty(spec(device.product_id), device.edition_id)
    }
    fn empty(spec: &'static Spec, edition: u32) -> Self {
        Self {
            spec,
            edition,
            draft: Draft::default(),
            observations: vec![],
            ports: BTreeMap::new(),
            status: Status::Unavailable,
            auto_detection: None,
            preview: false,
            limit_dismissed: false,
            last_request: None,
            subscriptions: vec![],
        }
    }
    pub(crate) fn snapshot(&self) -> Value {
        serde_json::to_value(&self.draft).unwrap_or_else(|_| json!({}))
    }
    pub(crate) fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        for port in self.ports.values() {
            port.update(cx, |port, cx| port.dismiss(window, cx));
        }
        cx.notify();
    }
    pub(crate) fn restore(&mut self, value: &Value, window: &mut Window, cx: &mut Context<Self>) {
        let mut draft = serde_json::from_value::<Draft>(value.clone()).unwrap_or_default();
        draft.ports.retain(|id, port| {
            (!self.spec.mainboard() || self.spec.mainboard_ports.contains(id))
                && port.valid(self.spec.minimum_leds, &self.spec.fan_counts)
        });
        self.draft = draft;
        // Restore edits into existing observed ports only. Never recover physical
        // port activation, power, LED count or refresh results from a profile.
        for (id, port) in &self.ports {
            if let Some(draft) = self.draft.ports.get(id) {
                port.update(cx, |port, cx| port.restore(draft.clone(), window, cx));
            }
        }
        cx.notify();
    }
    fn mount_ports(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.ports.clear();
        self.subscriptions.clear();
        for fact in &self.observations {
            if !fact.active {
                continue;
            }
            let Some(draft) = self.draft.ports.get(&fact.id) else {
                continue;
            };
            let port = cx.new(|cx| {
                PortEditor::new(
                    self.spec,
                    fact.clone(),
                    draft.clone(),
                    self.preview,
                    window,
                    cx,
                )
            });
            self.subscriptions
                .push(cx.subscribe(&port, |this, _, event: &PortChanged, cx| {
                    this.draft.ports.insert(event.id, event.draft.clone());
                    this.limit_dismissed = false;
                    cx.emit(WiredArgbChanged);
                    cx.notify();
                }));
            self.ports.insert(fact.id, port);
        }
    }
    fn center(&self, cx: &Context<Self>) -> AnyElement {
        let art = if self
            .spec
            .assets
            .iter()
            .any(|a| a.name == format!("product-{}", self.edition))
        {
            format!("product-{}", self.edition)
        } else {
            "product-0".into()
        };
        let enabled =
            self.preview && self.status != Status::NoPower && self.status != Status::Refreshing;
        let show_warning = self
            .observations
            .iter()
            .filter(|port| port.active)
            .filter_map(|port| self.draft.ports.get(&port.id))
            .map(PortDraft::total)
            .sum::<u32>()
            > 240
            || matches!(
                self.status,
                Status::Protection | Status::NoDevices | Status::LedLimit
            );
        v_flex().relative().items_center().flex_shrink_0().mt(surface::css(10.)).h(surface::css(275.))
            .w(surface::css(if self.spec.mainboard() { 600. } else { 150. }))
            .child(img(self.spec.asset(&art)).object_fit(ObjectFit::Contain)
                .w(surface::css(if self.spec.mainboard() { 600. } else { 130. }))
                .h(surface::css(if self.spec.mainboard() { 275. } else { 270. })))
            .when(!self.spec.mainboard(), |v| v.child(h_flex().absolute().bottom(surface::css(60.)).left_0().w_full().justify_center().gap(surface::css(12.))
                .child(button::Button::new("argb-auto-detection")
                    .child(img(self.spec.asset(if self.draft.auto_detection.or(self.auto_detection).unwrap_or(false) { "auto-active" } else { "auto" })).size(surface::css(20.))).ghost().p_0().size(surface::css(20.))
                    .accessibility_label(self.spec.text("GLITTER_MESSAGE_AUTO_DETECTION"))
                    .disabled(!enabled)
                    .tooltip(self.spec.text("GLITTER_MESSAGE_AUTO_DETECTION"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.preview && this.status != Status::NoPower {
                            let enabled = !this.draft.auto_detection.or(this.auto_detection).unwrap_or(false);
                            this.draft.auto_detection = Some(enabled);
                            this.last_request = Some(format!("ON_SET_AUTO_DETECTION_ENABLE: {{isAutoDetectionEnable:{enabled}}}"));
                            cx.emit(WiredArgbChanged); cx.notify();
                        }
                    })))
                .child(button::Button::new("argb-refresh").child(img(self.spec.asset("refresh")).size(surface::css(20.))).ghost().p_0().size(surface::css(20.))
                    .accessibility_label(self.spec.text("GLITTER_MESSAGE_REFRESH_ICON"))
                    .disabled(!enabled).tooltip(self.spec.text("GLITTER_MESSAGE_REFRESH_ICON"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.last_request = Some("ON_REFRESH_PORTS".into());
                        // Only the explicit preview controls can supply completion.
                        this.status = Status::Refreshing; cx.notify();
                    })))
                .child(img(self.spec.asset("warning")).size(surface::css(20.)).opacity(if show_warning { 1. } else { 0. }))))
            .into_any_element()
    }
    fn warning(&self, status: Status, cx: &Context<Self>) -> AnyElement {
        let (title, message) = match status {
            Status::NoPower => (
                "GLITTER_CONNECTION_REQUIRED",
                "GLITTER_CONNECTION_REQUIRED_MSG",
            ),
            Status::Protection => (
                "GLITTER_PROTECTION_ENABLED",
                "GLITTER_PROTECTION_ENABLED_MSG_0",
            ),
            Status::LedLimit => (
                "GLITTER_LED_LIMIT_EXCEEDED",
                "GLITTER_LED_LIMIT_EXCEEDED_MSG",
            ),
            _ => (
                "GLITTER_NO_DEVICES_DETECTED",
                "GLITTER_NO_DEVICES_DETECTED_MSG",
            ),
        };
        v_flex()
            .gap(surface::css(10.))
            .w(surface::css(600.))
            .p(surface::css(20.))
            .bg(cx.theme().group_box)
            .border_1()
            .border_color(cx.theme().warning)
            .rounded(surface::css(5.))
            .child(
                div()
                    .text_center()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.spec.text(title)),
            )
            .child(
                div()
                    .px(surface::css(40.))
                    .text_center()
                    .child(self.spec.text(message)),
            )
            .when(status == Status::Protection, |v| {
                v.child(
                    v_flex()
                        .gap(surface::css(20.))
                        .px(surface::css(40.))
                        .children((1..=4).map(|step| {
                            div().child(format!(
                                "{step}. {}",
                                self.spec
                                    .text(&format!("GLITTER_PROTECTION_ENABLED_MSG_{step}"))
                            ))
                        })),
                )
            })
            .when(status == Status::LedLimit, |v| {
                v.child(
                    button::Button::new("argb-dismiss-limit")
                        .label(self.spec.text("DISMISS"))
                        .ghost()
                        .xsmall()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.limit_dismissed = true;
                            if this.status == Status::LedLimit {
                                this.status = Status::Ready;
                            }
                            cx.notify();
                        })),
                )
            })
            .into_any_element()
    }
    fn page(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let total: u32 = self
            .observations
            .iter()
            .filter(|fact| fact.active)
            .filter_map(|fact| self.draft.ports.get(&fact.id))
            .map(PortDraft::total)
            .sum();
        let warning = if !self.spec.mainboard()
            && total > 240
            && !self.limit_dismissed
            && self.status == Status::Ready
        {
            Some(Status::LedLimit)
        } else if matches!(
            self.status,
            Status::NoPower | Status::Protection | Status::NoDevices
        ) || self.status == Status::LedLimit && !self.limit_dismissed
        {
            Some(self.status)
        } else {
            None
        };
        let compact =
            f32::from(window.viewport_size().width) / f32::from(window.rem_size()) * 16. <= 1024.;
        let center = self.center(cx);
        let mut page = v_flex()
            .relative()
            .min_h(surface::css(600.))
            .w_full()
            .items_center()
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .gap(surface::css(20.))
            .child(
                img(SharedString::from(format!(
                    "synapse/wired-argb-{}-dots.svg",
                    self.spec.product_id
                )))
                .absolute()
                .top(surface::css(20.))
                .w_full()
                .max_w(surface::css(1210.))
                .h(surface::css(400.))
                .object_fit(ObjectFit::Fill),
            );
        if self.status == Status::Unavailable {
            return page.child(center).child(surface::note(local("尚未读取到 ARGB 端口信息。连接设备服务后可读取端口和检测结果。", "ARGB port information is unavailable. Connect the device service to read ports and detection results."), cx)).into_any_element();
        }
        if self.status == Status::Refreshing {
            return page
                .child(div().mt(surface::css(20.)).child(loading::detecting(cx)))
                .into_any_element();
        }
        if self.spec.mainboard() || compact || !self.status.ports_visible() {
            page = page.child(center);
        }
        if self.status.ports_visible() {
            if self.spec.mainboard() {
                page = page.child(
                    h_flex()
                        .items_start()
                        .justify_center()
                        .flex_wrap()
                        .gap(surface::css(20.))
                        .w_full()
                        .children(
                            self.spec
                                .mainboard_ports
                                .iter()
                                .filter_map(|id| self.ports.get(id).cloned()),
                        ),
                );
            } else {
                let mut columns = h_flex()
                    .items_start()
                    .justify_center()
                    .gap(surface::css(0.))
                    .w_full();
                let lane = |ids: &[usize]| {
                    v_flex()
                        .w(surface::css(480.))
                        .px(surface::css(10.))
                        .pt(surface::css(10.))
                        .gap(surface::css(20.))
                        .children(
                            ids.iter()
                                .filter_map(|ix| self.observations.get(*ix))
                                .filter_map(|fact| self.ports.get(&fact.id))
                                .cloned(),
                        )
                };
                columns = columns.child(lane(&[3, 4, 5]));
                if !compact {
                    columns = columns.child(self.center(cx));
                }
                columns = columns.child(lane(&[0, 1, 2]));
                page = page.child(columns);
            }
        }
        if let Some(warning) = warning {
            page = page.child(
                div()
                    .absolute()
                    .left_0()
                    .top(surface::css(20.))
                    .w_full()
                    .flex()
                    .justify_center()
                    .child(self.warning(warning, cx)),
            );
        }
        page.into_any_element()
    }
}
impl Render for WiredArgbWorkspace {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex().w_full().child(self.page(window, cx)).when_some(
            self.last_request.clone().filter(|_| self.preview),
            |v, request| {
                v.child(surface::note(
                    format!("{} {request}", local("示例请求：", "Sample request:")),
                    cx,
                ))
            },
        )
    }
}
fn local(zh: &str, en: &str) -> String {
    if i18n::locale().starts_with("zh") {
        zh.into()
    } else {
        en.into()
    }
}
