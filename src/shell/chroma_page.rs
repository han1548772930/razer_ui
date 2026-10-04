//! Current Chroma Dashboard 23322/Ks, 62296/Wn and 77778/Se.
//! This owns local view preferences; it does not invent SDK applications,
//! firmware releases, installed modules, or immersive-engine capabilities.
use crate::{
    features::{
        ProductWorkspace,
        chroma_product::{self, ChromaPreset, ChromaProduct},
    },
    i18n,
    model::SetupStatus,
    resources,
    ui::{scroll::SourceScrollable as _, surface::css},
};
use gpui_kit::base::Button as BaseButton;
use gpui_kit::component::{
    input::{Input, InputState},
    select::{Select, SelectEvent, SelectState},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::PathBuf};

fn tr(key: &str) -> String {
    i18n::t(&format!("CHROMA_SOURCE.{key}"))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ChromaTab {
    Dashboard,
    Modules,
    Apps,
}
impl ChromaTab {
    fn key(self) -> &'static str {
        match self {
            Self::Dashboard => "DASHBOARD_HEADER",
            Self::Modules => "DEVICES_AND_MODULES_HEADER",
            Self::Apps => "CHROMA_APPS",
        }
    }
}
pub(super) enum ChromaPageEvent {
    OpenSettings,
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
struct Preferences {
    introduction: bool,
    onboard: bool,
    auto_prioritize: bool,
    show_disabled: bool,
    filter: usize,
    collapsed: BTreeSet<String>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            introduction: true,
            onboard: true,
            auto_prioritize: false,
            show_disabled: true,
            filter: 0,
            collapsed: BTreeSet::new(),
        }
    }
}
impl Preferences {
    fn path() -> PathBuf {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("razer_ui")
            .join("chroma-ui.json")
    }
    fn load() -> Self {
        std::fs::read(Self::path())
            .ok()
            .and_then(|s| serde_json::from_slice(&s).ok())
            .unwrap_or_default()
    }
    fn save(&self) {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(bytes) = serde_json::to_vec(self) {
            let _ = std::fs::write(path, bytes);
        }
    }
}
pub(super) struct ChromaPage {
    tab: ChromaTab,
    history: Vec<ChromaTab>,
    history_index: usize,
    preferences: Preferences,
    devices: Vec<Entity<ProductWorkspace>>,
    device_subscriptions: Vec<Subscription>,
    search: Entity<InputState>,
    filter: Entity<SelectState<Vec<String>>>,
    locale: String,
    focus: FocusHandle,
    dialog: Option<(Entity<ProductWorkspace>, Entity<ChromaProduct>)>,
    dialog_return_focus: Option<FocusHandle>,
    _subscriptions: Vec<Subscription>,
}
impl EventEmitter<ChromaPageEvent> for ChromaPage {}

impl ChromaPage {
    pub(super) fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let preferences = Preferences::load();
        let search = cx.new(|cx| InputState::new(window, cx).placeholder(tr("TEXT_SEARCH")));
        let filter = cx.new(|cx| {
            SelectState::new(
                Self::filter_choices(),
                Some(IndexPath::new(preferences.filter.min(2))),
                window,
                cx,
            )
        });
        let subscriptions = vec![
            cx.observe(&search, |_, _, cx| cx.notify()),
            cx.subscribe_in(&filter, window, |this, _, event, _, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    this.preferences.filter = Self::filter_choices()
                        .iter()
                        .position(|v| v == value)
                        .unwrap_or(0);
                    this.preferences.save();
                    cx.notify();
                }
            }),
        ];
        Self {
            tab: ChromaTab::Dashboard,
            history: vec![ChromaTab::Dashboard],
            history_index: 0,
            preferences,
            devices: Vec::new(),
            device_subscriptions: Vec::new(),
            search,
            filter,
            locale: i18n::locale(),
            focus: cx.focus_handle(),
            dialog: None,
            dialog_return_focus: None,
            _subscriptions: subscriptions,
        }
    }
    fn filter_choices() -> Vec<String> {
        [
            "TEXT_DROPDOWN_ALL",
            "TEXT_CHROMA_WORKSHOP_TITLE_APPS",
            "TEXT_CHROMA_WORKSHOP_TITLE_GAMES",
        ]
        .map(tr)
        .to_vec()
    }
    pub(super) fn set_devices(
        &mut self,
        devices: Vec<Entity<ProductWorkspace>>,
        cx: &mut Context<Self>,
    ) {
        self.devices = devices;
        self.device_subscriptions = self
            .devices
            .iter()
            .map(|entity| cx.observe(entity, |_, _, cx| cx.notify()))
            .collect();
        cx.notify();
    }
    pub(super) fn activate_apps(&mut self, cx: &mut Context<Self>) {
        self.navigate(ChromaTab::Apps, cx);
    }
    pub(super) fn can_step_history(&self, forward: bool) -> bool {
        if forward {
            self.history_index + 1 < self.history.len()
        } else {
            self.history_index > 0
        }
    }
    pub(super) fn step_history(&mut self, forward: bool, cx: &mut Context<Self>) {
        if !self.can_step_history(forward) {
            return;
        }
        self.history_index = if forward {
            self.history_index + 1
        } else {
            self.history_index - 1
        };
        self.tab = self.history[self.history_index];
        self.dialog = None;
        cx.notify();
    }
    fn navigate(&mut self, tab: ChromaTab, cx: &mut Context<Self>) {
        if self.tab == tab {
            return;
        }
        self.history.truncate(self.history_index + 1);
        self.history.push(tab);
        self.history_index += 1;
        self.tab = tab;
        self.dialog = None;
        cx.notify();
    }
    fn close_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dialog = None;
        if let Some(focus) = self.dialog_return_focus.take() {
            focus.focus(window, cx);
        }
        cx.notify();
    }
    fn toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .h(css(40.))
            .px(css(20.))
            .flex_shrink_0()
            .gap(css(10.))
            .bg(rgb(0x111111))
            .child(
                BaseButton::new("chroma-back")
                    .size(css(20.))
                    .p_0()
                    .disabled(!self.can_step_history(false))
                    .accessibility_label(tr("BACK"))
                    .child(Icon::default().path("synapse/history-back.svg").size(css(16.)))
                    .on_click(cx.listener(|this, _, _, cx| this.step_history(false, cx))),
            )
            .child(
                BaseButton::new("chroma-forward")
                    .size(css(20.))
                    .p_0()
                    .disabled(!self.can_step_history(true))
                    .accessibility_label(tr("FORWARD"))
                    .child(Icon::default().path("synapse/history-forward.svg").size(css(16.)))
                    .on_click(cx.listener(|this, _, _, cx| this.step_history(true, cx))),
            )
            .child(
                div()
                    .flex_1()
                    .font_family("RazerF5")
                    .text_size(css(18.))
                    .child("RAZER CHROMA"),
            )
            .child(
                BaseButton::new("chroma-settings")
                    .size(css(24.))
                    .p_0()
                    .accessibility_label(tr("SETTINGS_HEADER"))
                    .child(Icon::default().path("synapse/settings.svg").size(css(20.)))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(ChromaPageEvent::OpenSettings))),
            )
            .into_any_element()
    }
    fn navigation(&self, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .h(css(46.))
            .w_full()
            .flex_shrink_0()
            .justify_center()
            .gap(css(36.))
            .bg(rgb(0x222222))
            .border_b_2()
            .border_color(rgb(0x111111))
            .children(
                [ChromaTab::Dashboard, ChromaTab::Modules, ChromaTab::Apps].map(|tab| {
                    BaseButton::new(tab.key())
                        .h_full()
                        .p_0()
                        .text_size(css(14.))
                        .text_color(if self.tab == tab {
                            rgb(0x44d62c)
                        } else {
                            rgb(0x999999)
                        })
                        .hover(|s| s.text_color(rgb(0x44d62c)))
                        .child(tr(tab.key()).to_uppercase())
                        .on_click(cx.listener(move |this, _, _, cx| this.navigate(tab, cx)))
                }),
            )
            .into_any_element()
    }
    fn group(
        &self,
        key: &'static str,
        title: String,
        children: AnyElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let collapsed = self.preferences.collapsed.contains(key);
        v_flex()
            .w_full()
            .pb(css(20.))
            .child(
                BaseButton::new(key)
                    .h(css(18.))
                    .p_0()
                    .self_start()
                    .gap(css(10.))
                    .text_size(css(14.))
                    .child(
                        Icon::default().path("synapse/expand.svg").transform(Transformation::rotate(radians(if collapsed { -std::f32::consts::FRAC_PI_2 } else { 0. })))
                        .size(css(10.)),
                    )
                    .child(title.to_uppercase())
                    .hover(|s| s.text_color(rgb(0xffffff)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !this.preferences.collapsed.remove(key) {
                            this.preferences.collapsed.insert(key.into());
                        }
                        this.preferences.save();
                        cx.notify();
                    })),
            )
            .when(!collapsed, |v| v.child(div().mt(css(10.)).child(children)))
            .into_any_element()
    }
    fn introduction(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .relative()
            .w_full()
            .min_w(css(1000.))
            .min_h(css(531.))
            .rounded(css(5.))
            .mb(css(20.))
            .child(
                img("synapse/chroma-introduction_background.png")
                    .absolute()
                    .inset_0()
                    .size_full()
                    .object_fit(ObjectFit::Cover),
            )
            .child(
                BaseButton::new("chroma-intro-close")
                    .absolute()
                    .top(css(10.))
                    .right(css(10.))
                    .size(css(24.))
                    .p_0()
                    .accessibility_label(tr("CLOSE"))
                    .child(Icon::default().path("synapse/host-close.svg").size(css(20.)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.preferences.introduction = false;
                        this.preferences.save();
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .relative()
                    .mt(css(20.))
                    .px(css(30.))
                    .font_family("RazerF5")
                    .text_size(css(42.))
                    .text_color(rgb(0x44d62c))
                    .text_center()
                    .child(tr("INTRODUCTION_BANNER_HEADING_1")),
            )
            .child(
                div()
                    .relative()
                    .m(css(16.))
                    .font_family("RazerF5")
                    .text_size(css(24.))
                    .text_center()
                    .child(tr("INTRODUCTION_BANNER_HEADING_2")),
            )
            .child(
                div()
                    .relative()
                    .mb(css(16.))
                    .text_size(css(14.))
                    .text_center()
                    .child(tr("INTRODUCTION_BANNER_HEADING_3")),
            )
            .child(
                h_flex()
                    .relative()
                    .justify_around()
                    .items_start()
                    .pb(css(30.))
                    .children(
                        [
                            ("SYNAPSE", "synapse/chroma-big_synapse_4.svg"),
                            ("CHROMA_APP", "synapse/chroma-introduction-logo.png"),
                        ]
                        .map(|(kind, image)| {
                            v_flex()
                                .w(css(360.))
                                .items_center()
                                .child(img(image).size(css(100.)).mt(css(27.)))
                                .child(
                                    div()
                                        .mt(css(16.))
                                        .font_family("RazerF5")
                                        .text_size(css(16.))
                                        .child(tr(&format!("INTRODUCTION_BANNER_{kind}_BODY_1"))),
                                )
                                .child(
                                    div()
                                        .my(css(10.))
                                        .text_size(css(14.))
                                        .child(tr(&format!("INTRODUCTION_BANNER_{kind}_BODY_2"))),
                                )
                                .child(
                                    div()
                                        .text_size(css(14.))
                                        .text_center()
                                        .child(tr(&format!("INTRODUCTION_BANNER_{kind}_BODY_3"))),
                                )
                        }),
                    ),
            )
            .into_any_element()
    }
    fn devices(&self, cx: &mut Context<Self>) -> Vec<AnyElement> {
        self.devices
            .iter()
            .filter_map(|entity| {
                let workspace = entity.read(cx);
                let device = workspace.device(cx);
                if !device.is_chroma_device {
                    return None;
                }
                let available = workspace.chroma_lighting_workspace(cx).is_some()
                    && device.setup_status == SetupStatus::Ready
                    && !device
                        .power_status
                        .as_ref()
                        .is_some_and(|s| s.charging_status == "off");
                let name = device.display_name();
                let target = entity.clone();
                let snapshot = chroma_product::lighting_snapshot(workspace, cx);
                Some(
                    BaseButton::new(("chroma-device", entity.entity_id()))
                        .w(css(290.))
                        .h(css(245.))
                        .p_0()
                        .pt(css(10.))
                        .px(css(18.))
                        .pb(css(18.))
                        .flex_col()
                        .justify_start()
                        .gap_0()
                        .bg(rgb(0x111111))
                        .border_2()
                        .border_color(rgb(0x111111))
                        .rounded(css(5.))
                        .hover(|s| s.border_color(rgba(0x44d62c4d)))
                        .active(|s| s.border_color(rgb(0x44d62c)))
                        .accessibility_label(name.clone())
                        .when(!available, |b| {
                            b.tooltip("此设备的 Chroma 灯光页尚未接入。")
                        })
                        .child(div().w(css(250.)).h(css(140.)).flex_shrink_0().when_some(
                            resources::dashboard_image(
                                device.product_id,
                                device.edition_id,
                                device.layout_id,
                            ),
                            |v, image| {
                                v.child(img(image).size_full().object_fit(ObjectFit::Contain))
                            },
                        ))
                        .child(
                            v_flex()
                                .w_full()
                                .h(css(50.))
                                .justify_end()
                                .items_center()
                                .child(
                                    div()
                                        .text_size(css(14.))
                                        .text_center()
                                        .child(name.to_uppercase()),
                                ),
                        )
                        .when_some(snapshot, |button, state| {
                            button.child(
                                h_flex()
                                    .mt(css(5.))
                                    .gap(css(8.))
                                    .text_size(css(12.))
                                    .child(
                                        img("synapse/chroma-icon_device_brightness_100.svg")
                                            .size(css(18.)),
                                    )
                                    .child(format!(
                                        "{}%",
                                        if state.enabled { state.brightness } else { 0 }
                                    ))
                                    .child(state.effect),
                            )
                        })
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if !available {
                                return;
                            }
                            let product = target.update(cx, |workspace, cx| {
                                cx.new(|cx| {
                                    ChromaProduct::new(workspace, cx).expect("653 root was checked")
                                })
                            });
                            this.dialog_return_focus = window.focused(cx);
                            this.focus.focus(window, cx);
                            this.dialog = Some((target.clone(), product));
                            cx.notify();
                        }))
                        .into_any_element(),
                )
            })
            .collect()
    }
    fn presets(&self, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .flex_wrap()
            .gap(css(10.))
            .children(ChromaPreset::ALL.map(|preset| {
                let supported = self
                    .devices
                    .iter()
                    .any(|d| preset.supported(d.read(cx), cx));
                let icon = match preset {
                    ChromaPreset::Spectrum => "spectrumcycling",
                    ChromaPreset::StaticGreen => "static_green",
                    ChromaPreset::BreathingGreen => "breathing_green",
                    ChromaPreset::Wave => "wave",
                    ChromaPreset::Fire => "fire",
                    ChromaPreset::Starlight => "starlight",
                    ChromaPreset::Wheel => "wheel",
                };
                BaseButton::new(preset.key())
                    .size(css(80.))
                    .p_0()
                    .flex_col()
                    .gap(css(4.))
                    .disabled(!supported)
                    .accessibility_label(tr(preset.key()))
                    .tooltip(tr(preset.key()))
                    .rounded(css(5.))
                    .bg(rgb(0x111111))
                    .border_1()
                    .border_color(rgb(0x111111))
                    .hover(|s| s.border_color(rgb(0x44d62c)))
                    .child(img(format!("synapse/chroma-dashboard_{icon}.svg")).size(css(54.)))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        for entity in &this.devices {
                            entity.update(cx, |workspace, cx| preset.apply(workspace, window, cx));
                        }
                        cx.notify();
                    }))
            }))
            .child(
                BaseButton::new("chroma-preset-apps")
                    .size(css(80.))
                    .p_0()
                    .bg(rgb(0x111111))
                    .rounded(css(5.))
                    .accessibility_label(tr("CHROMA_APPS"))
                    .tooltip(tr("CHROMA_APPS"))
                    .child(img("synapse/chroma-dashboard_chromaapps.svg").size(css(54.)))
                    .on_click(cx.listener(|this, _, _, cx| this.activate_apps(cx))),
            )
            .into_any_element()
    }
    fn dashboard(&self, cx: &mut Context<Self>) -> AnyElement {
        let devices = self.devices(cx);
        let has_devices = !devices.is_empty();
        v_flex()
            .w_full()
            .max_w(css(1220.))
            .mx_auto()
            .pt(css(10.))
            .when(self.preferences.introduction, |v| {
                v.child(self.introduction(cx))
            })
            .child(self.group(
                "chroma-apply-effects",
                tr("APPLY_LIGHTING_EFFECTS"),
                self.presets(cx),
                cx,
            ))
            .when(has_devices, |v| {
                v.child(
                    self.group(
                        "chroma-device-group",
                        tr("TEXT_RAZER_CHROMA_DEVICES"),
                        h_flex()
                            .flex_wrap()
                            .gap(css(20.))
                            .children(devices)
                            .into_any_element(),
                        cx,
                    ),
                )
            })
            .child(
                self.group(
                    "chroma-services",
                    tr("ONLINE_SERVICES_HEADER"),
                    h_flex()
                        .flex_wrap()
                        .gap(css(20.))
                        .children(
                            [
                                (
                                    "RAZER_STORE",
                                    "services_store_placeholder",
                                    "https://www.razer.com/store/",
                                ),
                                (
                                    "RAZER_GOLD_AND_SILVER",
                                    "service_gold_and_silver",
                                    "https://gold.razer.com/",
                                ),
                                (
                                    "RAZER_COMMUNITY",
                                    "services_community_placeholder",
                                    "https://www.razer.com/community/",
                                ),
                                (
                                    "RAZER_SUPPORT",
                                    "services_support_placeholder",
                                    "https://support.razer.com/",
                                ),
                            ]
                            .map(|(key, image, url)| {
                                BaseButton::new(key)
                                    .w(css(290.))
                                    .h(css(245.))
                                    .p_0()
                                    .flex_col()
                                    .justify_start()
                                    .gap_0()
                                    .bg(rgb(0x111111))
                                    .border_2()
                                    .border_color(rgb(0x111111))
                                    .rounded(css(5.))
                                    .hover(|s| s.border_color(rgba(0x44d62c4d)))
                                    .accessibility_label(tr(key))
                                    .child(
                                        img(format!("synapse/chroma-{image}.png"))
                                            .w_full()
                                            .h(css(140.))
                                            .object_fit(ObjectFit::Contain),
                                    )
                                    .child(
                                        div()
                                            .px(css(20.))
                                            .mt(css(10.))
                                            .text_size(css(14.))
                                            .child(tr(key)),
                                    )
                                    .child(
                                        div()
                                            .px(css(20.))
                                            .mt(css(5.))
                                            .text_size(css(12.))
                                            .text_color(rgb(0x707070))
                                            .text_center()
                                            .child(tr(&format!("{key}_DESC"))),
                                    )
                                    .on_click(move |_, _, cx| cx.open_url(url))
                            }),
                        )
                        .into_any_element(),
                    cx,
                ),
            )
            .into_any_element()
    }
    fn apps(&self, cx: &mut Context<Self>) -> AnyElement {
        let onboard = v_flex()
            .relative()
            .p(css(20.))
            .mb(css(20.))
            .bg(rgb(0x2d2d2d))
            .text_center()
            .child(
                div()
                    .font_family("RazerF5")
                    .text_size(css(25.))
                    .line_height(css(30.))
                    .text_color(rgb(0x44d62c))
                    .mb(css(10.))
                    .child(tr("TEXT_ONBOARD_HEAD")),
            )
            .child(div().mb(css(10.)).child(tr("TEXT_ONBOARD_BODY_1")))
            .child(div().child(tr("TEXT_ONBOARD_BODY_2")))
            .child(
                BaseButton::new("chroma-onboard-close")
                    .absolute()
                    .top(css(10.))
                    .right(css(10.))
                    .size(css(20.))
                    .p_0()
                    .accessibility_label(tr("CLOSE"))
                    .child(Icon::default().path("synapse/host-close.svg").size(css(16.)))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.preferences.onboard = false;
                        this.preferences.save();
                        cx.notify();
                    })),
            );
        v_flex()
            .w_full()
            .max_w(css(1220.))
            .mx_auto()
            .text_size(css(14.))
            .line_height(css(17.))
            .when(self.preferences.onboard, |v| v.child(onboard))
            .child(
                h_flex()
                    .justify_between()
                    .child(
                        h_flex()
                            .gap(css(10.))
                            .text_size(css(24.))
                            .line_height(css(28.))
                            .text_color(rgb(0x44d62c))
                            .child(tr("TEXT_CHROMA_APPS"))
                            .child(
                                Switch::new("chroma-sdk-enable")
                                    .checked(false)
                                    .disabled(true)
                                    .accessibility_label(tr("TEXT_CHROMA_APPS")),
                            ),
                    )
                    .child(
                        Icon::default().path("synapse/onboard-help.svg")
                            .size(css(14.))
                            .tooltip(tr("TEXT_CHROMA_APPS_TIPS")),
                    ),
            )
            .child(
                h_flex()
                    .flex_wrap()
                    .justify_between()
                    .gap(css(10.))
                    .py(css(10.))
                    .child(
                        Checkbox::new("chroma-auto-prioritize")
                            .label(tr("TEXT_AUTO_PRIORITIZE"))
                            .checked(self.preferences.auto_prioritize)
                            .on_change(cx.listener(|this, value, _, cx| {
                                this.preferences.auto_prioritize = *value;
                                this.preferences.save();
                                cx.notify();
                            })),
                    )
                    .child(
                        h_flex()
                            .gap(css(20.))
                            .child(
                                Input::new(&self.search)
                                    .w(css(200.))
                                    .h(css(27.))
                                    .cleanable(true),
                            )
                            .child(
                                h_flex()
                                    .gap(css(6.))
                                    .child(tr("TEXT_VIEW"))
                                    .child(Select::new(&self.filter).w(css(150.)).h(css(27.))),
                            )
                            .child(
                                Checkbox::new("chroma-show-disabled")
                                    .label(tr("TEXT_SHOW_DISABLED_APPS"))
                                    .checked(self.preferences.show_disabled)
                                    .on_change(cx.listener(|this, value, _, cx| {
                                        this.preferences.show_disabled = *value;
                                        this.preferences.save();
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .mt(css(10.))
                    .mb(css(8.))
                    .text_size(css(16.))
                    .line_height(css(19.))
                    .child(tr("TEXT_APP_PRIORITY_LIST")),
            )
            .child(
                div()
                    .py(css(32.))
                    .bg(rgb(0x111111))
                    .text_center()
                    .child(tr("TEXT_NO_CHROMA_APPS_FOUND")),
            )
            .child(
                div()
                    .mt(css(30.))
                    .mb(css(8.))
                    .font_family("RazerF5")
                    .text_size(css(16.))
                    .child(tr("TEXT_CHROMA_WORKSHOP")),
            )
            .children(
                [
                    (
                        "APPS",
                        "apps",
                        "https://www.razer.com/chroma-workshop#--apps",
                    ),
                    (
                        "GAMES",
                        "games",
                        "https://www.razer.com/chroma-workshop/games",
                    ),
                    (
                        "PROFILES",
                        "profiles",
                        "https://www.razer.com/chroma-workshop/profiles",
                    ),
                ]
                .map(|(kind, image, url)| {
                    h_flex()
                        .mt(css(2.))
                        .p(css(20.))
                        .bg(rgb(0x111111))
                        .gap(css(10.))
                        .child(
                            img(format!("synapse/chroma-icon_chromaapps_{image}.svg"))
                                .size(css(40.))
                                .flex_shrink_0(),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .min_w_0()
                                .gap(css(4.))
                                .child(
                                    div()
                                        .text_size(css(16.))
                                        .child(tr(&format!("TEXT_CHROMA_WORKSHOP_TITLE_{kind}"))),
                                )
                                .child(div().text_size(css(12.)).text_color(rgb(0x999999)).child(
                                    tr(&format!("TEXT_CHROMA_WORKSHOP_{kind}_DESCRIPTION")),
                                )),
                        )
                        .child(
                            BaseButton::new(kind)
                                .p_0()
                                .text_size(css(14.))
                                .text_color(rgb(0x707070))
                                .underline()
                                .hover(|s| s.text_color(rgb(0x44d62c)))
                                .child(tr("TEXT_VISIT_WEBSITE"))
                                .on_click(move |_, _, cx| cx.open_url(url)),
                        )
                }),
            )
            .into_any_element()
    }
    fn dialog(&self, window: &mut Window, cx: &mut Context<Self>) -> Option<AnyElement> {
        let (workspace, view) = self.dialog.as_ref()?;
        let name = workspace.read(cx).device(cx).display_name();
        let width = (f32::from(window.viewport_size().width) * 0.8)
            .max(800.)
            .min(1280.);
        Some(
            div()
                .absolute()
                .inset_0()
                .size_full()
                .bg(rgba(0x000000aa))
                .flex()
                .justify_center()
                .items_start()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    v_flex()
                        .mt(css(54.))
                        .w(css(width))
                        .h_full()
                        .bg(rgb(0x222222))
                        .rounded_t(css(5.))
                        .child(
                            h_flex()
                                .relative()
                                .h(css(36.))
                                .flex_shrink_0()
                                .w_full()
                                .justify_center()
                                .border_b_1()
                                .border_color(rgb(0x5d5d5d))
                                .font_family("RazerF5")
                                .text_size(css(16.))
                                .text_color(rgb(0x999999))
                                .child(name.to_uppercase())
                                .child(
                                    BaseButton::new("chroma-modal-close")
                                        .absolute()
                                        .right_0()
                                        .top_0()
                                        .size(css(36.))
                                        .p_0()
                                        .accessibility_label(tr("CLOSE"))
                                        .child(Icon::default().path("synapse/host-close.svg").size(css(20.)))
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.close_dialog(window, cx)
                                        })),
                                ),
                        )
                        .child(div().flex_1().min_h_0().pb(css(54.)).child(view.clone())),
                )
                .into_any_element(),
        )
    }
}
impl Render for ChromaPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.locale != i18n::locale() {
            self.locale = i18n::locale();
            self.search
                .update(cx, |s, cx| s.set_placeholder(tr("TEXT_SEARCH"), window, cx));
            self.filter
                .update(cx, |s, cx| s.set_items(Self::filter_choices(), window, cx));
        }
        let content = match self.tab {
            ChromaTab::Dashboard => self.dashboard(cx),
            ChromaTab::Apps => self.apps(cx),
            // 40554/Z returns null for every empty group. No service inventory is fabricated.
            ChromaTab::Modules => div()
                .id("chroma-empty-module-groups")
                .w_full()
                .into_any_element(),
        };
        let dialog = self.dialog(window, cx);
        v_flex()
            .relative()
            .size_full()
            .min_h_0()
            .bg(rgb(0x222222))
            .text_color(rgb(0xcccccc))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event, window, cx| {
                if event.keystroke.key == "escape" && this.dialog.is_some() {
                    this.close_dialog(window, cx);
                    cx.stop_propagation();
                }
            }))
            .child(self.toolbar(cx))
            .child(self.navigation(cx))
            .child(
                div()
                    .id("chroma-page-scroll")
                    .flex_1()
                    .min_h_0()
                    .scrollable_both()
                    .px(css(20.))
                    .py(css(20.))
                    .child(content),
            )
            .when_some(dialog, |v, d| v.child(d))
    }
}
