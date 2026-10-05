//! Current Chroma Dashboard 23322/Ks, 62296/Wn and 77778/Se.
//! This owns local view preferences; it does not invent SDK applications,
//! firmware releases, installed modules, or immersive-engine capabilities.
use super::{app_picker::PickerModule, introduction_tour::TourKind};
use crate::{
    features::{
        ProductWorkspace,
        chroma_product::{self, ChromaPreset, ChromaProduct},
    },
    i18n,
    model::SetupStatus,
    resources,
    ui::{
        app_introduction_banner::AppIntroductionBanner, scroll::SourceScrollable as _, surface::css,
    },
};
use gpui_kit::base::{Button as BaseButton, Switch};
use gpui_kit::component::{
    checkbox::Checkbox,
    input::{Input, InputState},
    select::{Select, SelectEvent, SelectState},
    tooltip::Tooltip,
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::PathBuf};

mod presentation;
use presentation::{CollapseIcon, GroupContent, NavigationButton, grid_height};

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
    OpenTour(TourKind),
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
    pub(super) fn focus(&self, window: &mut Window, cx: &mut App) {
        self.focus.focus(window, cx);
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
                    .child(
                        Icon::default()
                            .path("synapse/history-back.svg")
                            .size(css(16.)),
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.step_history(false, cx))),
            )
            .child(
                BaseButton::new("chroma-forward")
                    .size(css(20.))
                    .p_0()
                    .disabled(!self.can_step_history(true))
                    .accessibility_label(tr("FORWARD"))
                    .child(
                        Icon::default()
                            .path("synapse/history-forward.svg")
                            .size(css(16.)),
                    )
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
            .h(css(48.))
            .w_full()
            .flex_shrink_0()
            .justify_center()
            .gap(css(20.))
            .bg(rgb(0x222222))
            .border_b_2()
            .border_color(rgb(0x000000))
            .children(
                [ChromaTab::Dashboard, ChromaTab::Modules, ChromaTab::Apps].map(|tab| {
                    NavigationButton {
                        id: tab.key(),
                        selected: self.tab == tab,
                        button: BaseButton::new(tab.key())
                            .selected(self.tab == tab)
                            .child(tr(tab.key()).to_uppercase())
                            .on_click(cx.listener(move |this, _, _, cx| this.navigate(tab, cx))),
                    }
                }),
            )
            .into_any_element()
    }
    fn group(
        &self,
        key: &'static str,
        title: String,
        height: f32,
        children: AnyElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let collapsed = self.preferences.collapsed.contains(key);
        v_flex()
            .w_full()
            .pb(css(20.))
            .child(
                h_flex()
                    .w_full()
                    .items_start()
                    .child(
                        BaseButton::new(key)
                            .group(key)
                            .h(css(if key == "chroma-apply-effects" {
                                17.
                            } else {
                                18.
                            }))
                            .p_0()
                            .self_start()
                            .gap(css(10.))
                            .text_size(css(14.))
                            .child(CollapseIcon { id: key, collapsed })
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
                    .when(key == "chroma-apply-effects", |row| {
                        row.child(div().ml(css(8.)).mt(css(1.)).child(
                            crate::ui::surface::help_control(
                                "chroma-apply-help",
                                tr("TEXT_APPLY_EFFECTS_TIPS"),
                            ),
                        ))
                    }),
            )
            .child(GroupContent {
                id: key,
                collapsed,
                height,
                children,
            })
            .into_any_element()
    }
    fn introduction(&self, cx: &mut Context<Self>) -> AnyElement {
        AppIntroductionBanner::new(
            "chroma-introduction-banner",
            "CHROMA_SOURCE.",
            cx.listener(|this, _, _, cx| {
                this.preferences.introduction = false;
                this.preferences.save();
                cx.notify();
            }),
            cx.listener(|_, _, _, cx| cx.emit(ChromaPageEvent::OpenTour(TourKind::Synapse))),
            cx.listener(|_, _, _, cx| cx.emit(ChromaPageEvent::OpenTour(TourKind::Chroma))),
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
                let available = workspace.has_chroma_device_page(cx)
                    && device.setup_status == SetupStatus::Ready
                    && !device
                        .power_status
                        .as_ref()
                        .is_some_and(|s| s.charging_status.eq_ignore_ascii_case("off"));
                let name = device.display_name();
                let edition = device
                    .dashboard
                    .edition_name
                    .as_ref()
                    .map(|text| text.get(&self.locale.to_lowercase()).to_owned())
                    .filter(|text| !text.is_empty());
                let target = entity.clone();
                let snapshot = chroma_product::lighting_snapshot(workspace, cx);
                Some(
                    BaseButton::new(("chroma-device", entity.entity_id()))
                        .w(css(290.))
                        .h(css(245.))
                        .p_0()
                        // `.box-item-device{padding:10px 20px 20px}`
                        .pt(css(10.))
                        .px(css(20.))
                        .pb(css(20.))
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
                            b.tooltip(|window, cx| {
                                Tooltip::new("此设备的 Chroma 灯光页尚未接入。").build(window, cx)
                            })
                        })
                        // `.box-item-device .disabled{opacity:.3;pointer-events:none}`
                        .child(
                            v_flex()
                                .w_full()
                                .h_full()
                                .when(!available, |v| v.opacity(0.3).cursor_default())
                                .child(div().w(css(250.)).h(css(140.)).flex_shrink_0().when_some(
                                    resources::dashboard_image(
                                        device.product_id,
                                        device.edition_id,
                                        device.layout_id,
                                    ),
                                    |v, image| {
                                        v.child(
                                            img(image).size_full().object_fit(ObjectFit::Contain),
                                        )
                                    },
                                ))
                                .child(
                                    // `.name-tag{display:inline-flex;flex-direction:column;
                                    // height:50px;justify-content:flex-end}`
                                    v_flex()
                                        .w_full()
                                        .h(css(50.))
                                        .justify_end()
                                        .child(
                                            div()
                                                .text_size(css(14.))
                                                .line_height(css(16.))
                                                .min_h(css(17.))
                                                .max_h(css(33.))
                                                .overflow_hidden()
                                                .child(name.to_uppercase()),
                                        )
                                        .when_some(edition, |view, edition| {
                                            view.child(
                                                div()
                                                    .text_size(css(12.))
                                                    .line_height(css(14.))
                                                    .text_color(rgb(0x707070))
                                                    .child(edition.to_uppercase()),
                                            )
                                        }),
                                )
                                .when_some(snapshot, |view, state| {
                                    view.child(
                                        h_flex()
                                            .mt(css(5.))
                                            .gap(css(8.))
                                            .text_size(css(12.))
                                            .child(
                                                img(
                                                    "synapse/chroma-icon_device_brightness_100.svg",
                                                )
                                                .size(css(18.)),
                                            )
                                            .child(format!(
                                                "{}%",
                                                if state.enabled { state.brightness } else { 0 }
                                            ))
                                            .child(state.effect),
                                    )
                                }),
                        )
                        .on_click(cx.listener(move |this, _, window, cx| {
                            if !available {
                                return;
                            }
                            let product = cx.new(|cx| {
                                ChromaProduct::new(&target, cx).expect("chromaApp root was checked")
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
        let chroma_devices: Vec<_> = self
            .devices
            .iter()
            .filter(|d| d.read(cx).device(cx).is_chroma_device)
            .collect();
        h_flex()
            .flex_wrap()
            .gap(css(10.))
            .children(ChromaPreset::ALL.map(|preset| {
                let supported_names: Vec<_> = chroma_devices
                    .iter()
                    .filter(|d| preset.supported(d.read(cx), cx))
                    .map(|d| d.read(cx).device(cx).display_name())
                    .collect();
                let active = !chroma_devices.is_empty()
                    && chroma_devices
                        .iter()
                        .all(|d| preset.matches(d.read(cx), cx));
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
                    .w(css(120.))
                    .h(css(130.))
                    .p_0()
                    .pt(css(20.))
                    .px(css(10.))
                    .flex_shrink_0()
                    .flex_col()
                    .justify_start()
                    .gap_0()
                    .accessibility_label(tr(preset.key()))
                    .tooltip(move |window, cx| {
                        let names = if supported_names.is_empty() {
                            tr("TEXT_NO_DEVICE")
                        } else {
                            supported_names.join("\n")
                        };
                        Tooltip::new(format!(
                            "{}\n{}",
                            tr("TEXT_APPLY_EFFECTS_SUPPORT_EFFECT_TIP"),
                            names
                        ))
                        .build(window, cx)
                    })
                    .rounded(css(5.))
                    .bg(if active {
                        rgb(0x111111)
                    } else {
                        rgba(0x00000000)
                    })
                    .border_1()
                    .border_color(if active {
                        rgb(0x5d5d5d)
                    } else {
                        rgba(0x00000000)
                    })
                    .hover(move |s| {
                        s.bg(if active {
                            rgb(0x111111)
                        } else {
                            rgba(0x00000099)
                        })
                        .border_color(rgba(0x44d62c4d))
                    })
                    .when(!active, |b| {
                        b.active(|s| s.bg(rgba(0x00000099)).border_color(rgb(0x399c26)))
                    })
                    .child(
                        img(format!("synapse/chroma-dashboard_{icon}.svg"))
                            .size(css(48.))
                            .flex_shrink_0(),
                    )
                    .child(
                        div()
                            .my(css(12.))
                            .text_size(css(12.))
                            .text_center()
                            .text_color(rgb(0xcccccc))
                            .child(tr(preset.key()).to_uppercase()),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        for entity in &this.devices {
                            if entity.read(cx).device(cx).is_chroma_device {
                                entity.update(cx, |workspace, cx| {
                                    preset.apply(workspace, window, cx)
                                });
                            }
                        }
                        cx.notify();
                    }))
            }))
            .child(
                BaseButton::new("chroma-preset-apps")
                    .w(css(120.))
                    .h(css(130.))
                    .p_0()
                    .pt(css(20.))
                    .px(css(10.))
                    .flex_shrink_0()
                    .flex_col()
                    .justify_start()
                    .gap_0()
                    .rounded(css(5.))
                    .border_1()
                    .border_color(rgba(0x00000000))
                    .hover(|s| s.bg(rgba(0x00000099)).border_color(rgba(0x44d62c4d)))
                    .active(|s| s.bg(rgba(0x00000099)).border_color(rgb(0x399c26)))
                    .accessibility_label(tr("CHROMA_APPS"))
                    .child(
                        img("synapse/chroma-dashboard_chromaapps.svg")
                            .size(css(48.))
                            .flex_shrink_0(),
                    )
                    .child(
                        div()
                            .my(css(12.))
                            .text_size(css(12.))
                            .text_center()
                            .text_color(rgb(0xcccccc))
                            .child(tr("CHROMA_APPS").to_uppercase()),
                    )
                    .on_click(cx.listener(|this, _, _, cx| this.activate_apps(cx))),
            )
            .into_any_element()
    }
    fn dashboard(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        let devices = self.devices(cx);
        let device_count = devices.len();
        let has_devices = device_count > 0;
        let viewport =
            f32::from(window.viewport_size().width) * 16. / f32::from(window.rem_size()).max(1.);
        let max_width: f32 = if viewport <= 1279. {
            910.
        } else if self.devices.len() > 4 {
            2460.
        } else {
            1220.
        };
        let width = (viewport - 40.).max(290.).min(max_width);
        let groups = v_flex()
            .w_full()
            .max_w(css(max_width))
            .when(viewport > 1279., |v| v.mx_auto())
            .mt(css(20.))
            .child(self.group(
                "chroma-apply-effects",
                tr("APPLY_LIGHTING_EFFECTS"),
                grid_height(width, 8, 120., 130., 10.),
                self.presets(cx),
                cx,
            ))
            .when(has_devices, |v| {
                v.child(
                    self.group(
                        "chroma-device-group",
                        tr("TEXT_RAZER_CHROMA_DEVICES"),
                        grid_height(width, device_count, 290., 245., 20.),
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
                    grid_height(width, 4, 290., 245., 20.),
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
            );
        v_flex()
            .w_full()
            .when(self.preferences.introduction, |v| {
                v.child(self.introduction(cx))
            })
            .child(groups)
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
                    .child(
                        Icon::default()
                            .path("synapse/host-close.svg")
                            .size(css(16.)),
                    )
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
                        Icon::default()
                            .path("synapse/onboard-help.svg")
                            .size(css(14.)),
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
    fn modules(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        // 65596/P5 is the current Modules allowlist. Service records such as
        // createdAt, firmwareUpdateInfo, descriptions and sizes remain absent
        // until actually supplied; this catalogue never fabricates them.
        let viewport =
            f32::from(window.viewport_size().width) * 16. / f32::from(window.rem_size()).max(1.);
        let name_width = if viewport <= 720. {
            150.
        } else if viewport <= 1000. {
            250.
        } else if viewport <= 1160. {
            400.
        } else {
            500.
        };
        let entries = [
            (
                PickerModule::ChromaConnect,
                "Chroma Connect",
                "DASHBOARD_CHROMA_CONNECT",
                "synapse/module-chroma-connect.svg",
            ),
            (
                PickerModule::ChromaStudio,
                "Chroma Studio",
                "DASHBOARD_CHROMA_STUDIO",
                "synapse/module-chroma-studio.svg",
            ),
            (
                PickerModule::AudioVisualizer,
                "Chroma Visualizer",
                "DASHBOARD_AUDIO_VISUALIZER",
                "synapse/module-audio-visualizer.svg",
            ),
            (
                PickerModule::SensaHd,
                "Sensa HD Haptics",
                "DASHBOARD_SENSA",
                "synapse/module-sensa-hd.svg",
            ),
        ];
        v_flex()
            .w(css(1220.))
            .mx_auto()
            .mb(css(40.))
            .child(
                div()
                    .font_family("RazerF5")
                    .text_size(css(24.))
                    .text_color(rgb(0x44d62c))
                    .mb(css(10.))
                    .child(tr("AVAILABLE_MODULES").to_uppercase()),
            )
            .children(entries.map(|(module, english, key, icon)| {
                // 40554/k's current Chinese name overrides.
                let name = if self.locale.eq_ignore_ascii_case("zh-CN") {
                    match module {
                        PickerModule::ChromaStudio => "幻彩控制室 (Chroma Studio)".to_string(),
                        PickerModule::ChromaConnect => "幻彩互联 (Chroma Connect)".to_string(),
                        PickerModule::AudioVisualizer => {
                            "幻彩可视化工具 (Chroma Visualizer)".to_string()
                        }
                        PickerModule::SensaHd => {
                            "Razer Sensa HD 触觉反馈技术 (Sensa HD Haptic)".to_string()
                        }
                        _ => unreachable!(),
                    }
                } else if self.locale.eq_ignore_ascii_case("en") {
                    english.to_string()
                } else {
                    tr(key)
                };
                h_flex()
                    .w_full()
                    .h(css(80.))
                    .mb(css(1.))
                    .pl(css(20.))
                    .pr(css(30.))
                    .bg(rgb(0x111111))
                    .child(img(icon).size(css(40.)).flex_shrink_0())
                    .child(
                        div()
                            .ml(css(10.))
                            .w(css(name_width))
                            .flex_shrink_0()
                            .text_size(css(16.))
                            .text_ellipsis()
                            .child(name),
                    )
                    .child(div().flex_1())
                    .child(
                        super::service_pages::module_action(
                            module.key(),
                            tr("INSTALL"),
                            true,
                            true,
                            cx,
                        )
                        .w(css(90.))
                        .ml(css(30.)),
                    )
            }))
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
                                        .child(
                                            Icon::default()
                                                .path("synapse/host-close.svg")
                                                .size(css(20.)),
                                        )
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
            ChromaTab::Dashboard => self.dashboard(window, cx),
            ChromaTab::Apps => self.apps(cx),
            ChromaTab::Modules => self.modules(window, cx),
        };
        let dialog = self.dialog(window, cx);
        v_flex()
            .relative()
            .size_full()
            .min_h_0()
            .bg(rgb(0x222222))
            .font_family("Roboto")
            .text_color(rgb(0xcccccc))
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
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
