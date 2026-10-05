//! Current 22534/z, G/V/K and 55 CSS; separate from the device toolbar battery.
//! See dashboard-device-current-evidence.json for enum literals and selectors.
use crate::{
    i18n,
    model::{DashboardDeviceMetadata, Device, DeviceCategory, PowerStatus, SetupStatus},
    ui::surface::css,
};
use gpui_kit::base::{
    ElementExt as _,
    motion::{self, Easing, Transition},
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::{cell::Cell, rc::Rc, time::Duration};

pub(super) fn name(device: &Device) -> &str {
    let locale = locale();
    device
        .name
        .values
        .get(locale)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            device
                .dashboard
                .base_product_name
                .as_ref()
                .filter(|s| !s.is_empty())
        })
        .or_else(|| device.name.values.get("en").filter(|s| !s.is_empty()))
        .map(String::as_str)
        .unwrap_or("")
}

fn locale() -> &'static str {
    if i18n::locale().eq_ignore_ascii_case("zh-cn") {
        "zh-cn"
    } else {
        "en"
    }
}

pub(super) fn edition(device: &Device) -> &str {
    device
        .dashboard
        .edition_name
        .as_ref()
        .and_then(|names| names.values.get(locale()))
        .map(String::as_str)
        .unwrap_or("")
}

pub(super) fn status(device: &Device) -> String {
    let fields = &device.dashboard;
    if device.is_single_profile
        && fields.is_show_profile_name_in_dashboard != Some(true)
        && !fields
            .inter_device_mapping_config
            .as_ref()
            .is_some_and(|c| c.is_supported == Some(false))
    {
        return String::new();
    }
    let key = match device.setup_status {
        SetupStatus::Waiting => Some(("WAITING", true)),
        SetupStatus::Downloading => Some(("DOWNLOADING", true)),
        SetupStatus::Installing => Some(("INSTALLING", true)),
        SetupStatus::Syncing => Some(("SYNCING", true)),
        SetupStatus::Updating => Some(("UPDATING", true)),
        SetupStatus::InstallCanceled | SetupStatus::Error => Some(("INSTALLATION_FAILED", false)),
        _ => None,
    };
    if let Some((key, dots)) = key {
        return format!("{}{}", i18n::t(key), if dots { "..." } else { "" });
    }
    if device.setup_status != SetupStatus::Ready || fields.not_show_profile_name == Some(true) {
        return String::new();
    }
    // Compound devices use the selected sub-device's source GUID and profiles.
    let selected = device.sub_devices.as_ref().and_then(|items| {
        items
            .iter()
            .find(|item| item.get("isShownOnUI").and_then(|v| v.as_bool()) == Some(true))
    });
    let name = if let Some(selected) = selected {
        selected
            .get("activeProfile")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .and_then(|guid| {
                selected
                    .get("profiles")
                    .and_then(|v| v.as_array())
                    .and_then(|items| {
                        items
                            .iter()
                            .find(|p| p.get("guid").and_then(|v| v.as_str()) == Some(guid))
                    })
            })
            .and_then(|p| p.get("name").and_then(|v| v.as_str()))
    } else {
        // Local active_profile stores an ID. Resolve it exactly to the source
        // GUID first, never use active_profile_obj()'s first-profile fallback.
        device
            .profiles
            .iter()
            .find(|p| p.id == device.active_profile && !p.guid.is_empty())
            .and_then(|active| device.profiles.iter().find(|p| p.guid == active.guid))
            .map(|p| p.name.as_str())
    };
    name.map(|name| {
        format!(
            "{name}{}",
            if fields.auto_switch == Some(true) {
                " (Auto)"
            } else {
                ""
            }
        )
    })
    .unwrap_or_default()
}

pub(super) fn power_off(device: &Device) -> bool {
    device
        .power_status
        .as_ref()
        .is_some_and(|p| p.charging_status.eq_ignore_ascii_case("off"))
}

pub(super) fn standby_or_off(device: &Device) -> bool {
    (device.dashboard.supports_standby_mode != Some(false)
        && device.dashboard.device_power_state.as_deref() == Some("off"))
        || (device.dashboard.supports_standby_mode == Some(true)
            && device.dashboard.device_power_state.as_deref() == Some("standby"))
}

pub(super) fn disabled(device: &Device, observed: &DisplayedPower) -> bool {
    (observed.off()
        && device.dashboard.is_xbox != Some(true)
        && device.dashboard.is_playstation != Some(true))
        || device.setup_status == SetupStatus::RestartRequired
        || observed.standby_or_off()
}

/// Persistent fields written by z.generateBatteryData/checkStandbyState.
/// Unknown power states and explicit unsupported values do not reset them.
#[derive(Clone)]
pub(super) struct DisplayedPower {
    power: Option<PowerStatus>,
    delayed: bool,
    is_off: bool,
    is_standby: bool,
    with_battery: bool,
    icon: Icon,
    hide_icon: bool,
    show_value: bool,
    incorrect: bool,
}

impl DisplayedPower {
    pub(super) fn off(&self) -> bool {
        self.power
            .as_ref()
            .is_some_and(|power| power.charging_status.eq_ignore_ascii_case("off"))
    }

    pub(super) fn standby_or_off(&self) -> bool {
        self.is_off || self.is_standby
    }

    fn accept_power(&mut self, power: &PowerStatus, fields: &DashboardDeviceMetadata) {
        self.power = Some(power.clone());
        self.with_battery = true;
        self.incorrect = is_normal(&power.charging_status) && power.level == -1;
        self.hide_icon = fields.hide_battery_icon.unwrap_or(false) || self.incorrect;
        self.icon = if self.off() && fields.is_playstation == Some(true) {
            Icon::Mask("synapse/dashboard-card/ps-icon.svg")
        } else if self.off() && fields.is_xbox == Some(true) {
            Icon::Mask("synapse/dashboard-card/xbox-icon.svg")
        } else {
            icon(power)
        };
    }

    fn accept_standby(&mut self, fields: &DashboardDeviceMetadata) {
        let state = fields.device_power_state.as_deref();
        if fields.supports_standby_mode == Some(true) {
            match state {
                Some("off" | "standby" | "active") => {
                    self.is_off = state == Some("off");
                    self.is_standby = state == Some("standby");
                    self.with_battery = true;
                    self.icon = if self.is_off {
                        Icon::Power(0xff0000)
                    } else if self.is_standby {
                        Icon::Power(0xfd8611)
                    } else {
                        // batt-active has no final CSS override of the base image.
                        Icon::Image("synapse/battery-charging-100.svg".into())
                    };
                }
                _ => {}
            }
        } else if fields.supports_standby_mode.is_none() {
            match state {
                Some("off" | "active") => {
                    self.is_off = state == Some("off");
                    self.with_battery = true;
                    self.hide_icon = !self.is_off;
                    self.icon = if self.is_off {
                        Icon::Power(0xff0000)
                    } else {
                        Icon::Image("synapse/battery-charging-100.svg".into())
                    };
                    // The null/absent-support branch does not write isStandby.
                }
                _ => {}
            }
        }
    }
}

/// z only accepts powerStatus into its display state while ready or waiting.
/// Other setup states retain the last accepted observation; a card initially
/// mounted in error/installing does not gain a battery from an ineligible value.
struct ObservedPower {
    previous: Option<PowerStatus>,
    previous_support: Option<bool>,
    previous_state: Option<String>,
    previous_hide: Option<bool>,
    previous_show: Option<bool>,
    displayed: DisplayedPower,
}

impl ObservedPower {
    fn timer(cx: &mut Context<Self>) {
        cx.spawn(async move |state, cx| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            let _ = state.update(cx, |state, cx| {
                state.displayed.delayed = false;
                cx.notify();
            });
        })
        .detach();
    }
}

fn same_power(a: Option<&PowerStatus>, b: Option<&PowerStatus>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.level == b.level && a.charging_status == b.charging_status,
        (None, None) => true,
        _ => false,
    }
}

pub(super) fn observed_power(
    id: &str,
    device: &Device,
    window: &mut Window,
    cx: &mut App,
) -> DisplayedPower {
    let eligible = matches!(
        device.setup_status,
        SetupStatus::Ready | SetupStatus::Waiting
    );
    let off = power_off(device);
    let fields = &device.dashboard;
    let id: ElementId = SharedString::from(id.to_owned()).into();
    let state = window.use_keyed_state((id, "observed-battery"), cx, |_, cx| {
        if !off {
            ObservedPower::timer(cx);
        }
        let mut displayed = DisplayedPower {
            power: None,
            delayed: !off,
            is_off: false,
            is_standby: false,
            with_battery: false,
            icon: Icon::Image("synapse/battery-charging-100.svg".into()),
            hide_icon: fields.hide_battery_icon.unwrap_or(false),
            show_value: fields.show_battery_value.unwrap_or(true),
            incorrect: false,
        };
        if eligible {
            if let Some(power) = &device.power_status {
                displayed.accept_power(power, fields);
            }
        }
        displayed.accept_standby(fields);
        ObservedPower {
            previous: device.power_status.clone(),
            previous_support: fields.supports_standby_mode,
            previous_state: fields.device_power_state.clone(),
            previous_hide: fields.hide_battery_icon,
            previous_show: fields.show_battery_value,
            displayed,
        }
    });
    state.update(cx, |state, cx| {
        if !same_power(state.previous.as_ref(), device.power_status.as_ref()) {
            if eligible && device.power_status.is_some() {
                if state
                    .previous
                    .as_ref()
                    .is_some_and(|power| power.charging_status.eq_ignore_ascii_case("off"))
                    && !off
                {
                    state.displayed.delayed = true;
                    // Source setTimeout calls do not cancel earlier delays.
                    ObservedPower::timer(cx);
                }
                if let Some(power) = &device.power_status {
                    state.displayed.accept_power(power, fields);
                }
            }
            state.previous = device.power_status.clone();
        }
        // componentDidUpdate writes prop overrides after generateBatteryData,
        // then runs checkStandbyState only when its two inputs have changed.
        if state.previous_show != fields.show_battery_value {
            state.displayed.show_value = fields.show_battery_value == Some(true);
            state.previous_show = fields.show_battery_value;
        }
        if state.previous_hide != fields.hide_battery_icon {
            state.displayed.hide_icon = fields.hide_battery_icon == Some(true);
            state.previous_hide = fields.hide_battery_icon;
        }
        if state.previous_support != fields.supports_standby_mode
            || state.previous_state != fields.device_power_state
        {
            state.displayed.accept_standby(fields);
            state.previous_support = fields.supports_standby_mode;
            state.previous_state = fields.device_power_state.clone();
        }
        state.displayed.clone()
    })
}

pub(super) fn show_spinner(setup: SetupStatus, off: bool) -> bool {
    !matches!(
        setup,
        SetupStatus::RestartRequired
            | SetupStatus::InstallCanceled
            | SetupStatus::Ready
            | SetupStatus::Error
    ) && !(setup == SetupStatus::Waiting && off)
}

pub(super) fn js_truthy(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(value) => *value,
        serde_json::Value::String(value) => !value.is_empty(),
        serde_json::Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.),
        _ => true,
    }
}

pub(super) fn min_firmware(device: &Device) -> bool {
    device
        .dashboard
        .firmware_update_info
        .as_ref()
        .and_then(|value| value.get("minRequiredVersion"))
        .is_some_and(js_truthy)
}

pub(super) fn preset_loading(device: &Device) -> bool {
    device
        .dashboard
        .device_state
        .as_ref()
        .and_then(|value| value.get("type"))
        .and_then(|value| value.as_str())
        == Some("presetLoading")
}

pub(super) fn can_focus(device: &Device) -> bool {
    !min_firmware(device)
        && !preset_loading(device)
        && !power_off(device)
        && device.dashboard.is_xbox != Some(true)
        && device.dashboard.is_playstation != Some(true)
        && device.dashboard.device_init_status_fail.as_deref() != Some("mixer_system_check_failed")
        && !standby_or_off(device)
        && device.dashboard.device_power_state.as_deref() != Some("off")
        && !matches!(
            device.setup_status,
            SetupStatus::Updating | SetupStatus::RestartRequired
        )
}

pub(super) fn category(device: &Device) -> &str {
    device
        .dashboard
        .source_category
        .as_deref()
        .unwrap_or(match device.category {
            DeviceCategory::Controller => "Controller",
            DeviceCategory::Headset => "Headset",
            DeviceCategory::Audio => "AUDIO",
            _ => "",
        })
}

#[derive(Clone)]
enum Icon {
    Image(String),
    Power(u32),
    Source(&'static str, f32),
    Mask(&'static str),
}

#[derive(IntoElement)]
pub(super) struct DashboardBattery {
    id: ElementId,
    observed: DisplayedPower,
    fields: DashboardDeviceMetadata,
    setup: SetupStatus,
    source_spinner: bool,
    category: String,
    card: String,
    grid: Entity<super::dashboard_grid::DashboardState>,
}

impl DashboardBattery {
    pub(super) fn new(
        id: String,
        device: &Device,
        observed: DisplayedPower,
        source_spinner: bool,
        card: String,
        grid: Entity<super::dashboard_grid::DashboardState>,
    ) -> Self {
        Self {
            id: SharedString::from(id).into(),
            observed,
            fields: device.dashboard.clone(),
            setup: device.setup_status,
            source_spinner,
            category: category(device).to_owned(),
            card,
            grid,
        }
    }
}

#[derive(Default)]
struct BatteryHover {
    hover: bool,
    tooltip_right: Rc<Cell<Pixels>>,
    anchor_right: Rc<Cell<Pixels>>,
    flipped: bool,
}

fn is_charging(status: &str) -> bool {
    status.eq_ignore_ascii_case("Charging")
}
fn is_normal(status: &str) -> bool {
    // Uppercase NOT_CHARGING is the established local snapshot encoding;
    // NoCharge_BatteryFull is current 29228/pV.NOT_CHARGING's wire literal.
    status.eq_ignore_ascii_case("NoCharge_BatteryFull") || status == "NOT_CHARGING"
}
fn icon(power: &PowerStatus) -> Icon {
    let status = power.charging_status.as_str();
    if status.eq_ignore_ascii_case("off") {
        return Icon::Power(0xff0000);
    }
    let file = if is_charging(status) && power.level <= 99 {
        "battery-charging.svg".into()
    } else if status == "batt-warning" {
        "battery-error.svg".into()
    } else if is_normal(status) || status == "ReachChargingLimit" {
        let bucket = power.level.div_euclid(10) * 10;
        // Values outside the CSS's 0..100 classes inherit its base image.
        if (0..=100).contains(&bucket) {
            format!(
                "battery-{}{bucket}.svg",
                if status == "ReachChargingLimit" {
                    "paused-"
                } else {
                    ""
                }
            )
        } else {
            "battery-charging-100.svg".into()
        }
    } else {
        "battery-charging-100.svg".into()
    };
    Icon::Image(format!("synapse/{file}"))
}

impl RenderOnce for DashboardBattery {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let off = self.observed.off();
        let state = window.use_keyed_state((self.id.clone(), "battery-hover"), cx, |_, _| {
            BatteryHover::default()
        });
        let delayed = self.observed.delayed;
        let hovered = state.read(cx).hover;
        let tip_right = state.read(cx).tooltip_right.clone();
        let anchor_right = state.read(cx).anchor_right.clone();
        let flipped = state.read(cx).flipped;
        let opacity = motion::transition(
            (self.id.clone(), "tooltip-opacity"),
            if hovered { 1. } else { 0. },
            Transition::new(Duration::from_millis(300)).easing(Easing::Linear),
            window,
            cx,
        );
        let mut result = div()
            .id(self.id)
            .absolute()
            .top(css(10.))
            .right(css(10.))
            .flex()
            .items_center()
            .text_size(css(14.))
            .text_color(rgb(0xcccccc));
        // z checks spinner before mounting either of its battery branches.
        if self.source_spinner {
            return result;
        }
        let fields = self.fields;
        let standby = fields.supports_standby_mode == Some(true);
        let power_state = fields.device_power_state.as_deref();
        let mut tip = None;
        let mut chosen = None;
        let mut value = None;
        if standby {
            match (self.observed.is_off, self.observed.is_standby) {
                (true, _) => {
                    chosen = Some(Icon::Source(
                        "synapse/dashboard-card/icon_device_power_state_off.svg",
                        24.,
                    ));
                    tip = Some("STANDBY_MODE_OFF_TOOLTIP");
                }
                (_, true) => {
                    chosen = Some(Icon::Power(0xfd8611));
                    tip = Some("DEVICE_STANDBY_TOOLTIP_LINE1");
                }
                _ => {}
            }
        } else if self.observed.with_battery {
            chosen = (!self.observed.hide_icon).then(|| self.observed.icon.clone());
            if (off || power_state == Some("off"))
                && fields.is_xbox != Some(true)
                && fields.is_playstation != Some(true)
            {
                tip = Some(if self.category.eq_ignore_ascii_case("AUDIO") {
                    "DASHBOARD_AUDIO_DEVICE_OFF_TOOLTIP"
                } else {
                    "DASHBOARD_DEVICE_OFF_TOOLTIP"
                });
            }
            if off {
                if fields.is_playstation == Some(true) {
                    tip = Some(
                        if fields.sub_category.as_deref() == Some("ARCADE_CONTROLLER") {
                            "ARCADE_PS_MODE_TOOLTIP"
                        } else {
                            "PS_MODE_TOOLTIP"
                        },
                    );
                } else if fields.is_xbox == Some(true) {
                    tip = if self.category == "Controller" {
                        Some(
                            if fields.controller_mode_variant.as_deref() == Some("THREE_MODE") {
                                "XBOX_SHORTCUT_TOOLTIP_THREE_MODE"
                            } else {
                                "XBOX_SHORTCUT_TOOLTIP"
                            },
                        )
                    } else if self.category == "Headset" {
                        Some("XBOX_SHORTCUT_TOOLTIP_HEADSET")
                    } else if self.category.eq_ignore_ascii_case("AUDIO")
                        && fields
                            .sub_category
                            .as_deref()
                            .is_some_and(|s| s.eq_ignore_ascii_case("EARBUDS"))
                    {
                        Some("XBOX_EARBUDS_TOOLTIP")
                    } else {
                        None
                    };
                }
            } else if self
                .observed
                .power
                .as_ref()
                .is_some_and(|p| p.charging_status == "batt-warning")
            {
                tip = Some("BATTERY_ERROR_TIPS");
            }
            let suppress_value = fields.is_external_batt == Some(true)
                || (fields.supports_standby_mode.is_none()
                    && power_state.is_some_and(|s| !s.is_empty()));
            if !suppress_value && !self.observed.incorrect && !off && self.observed.show_value {
                let power = self.observed.power.as_ref();
                let charging = power.is_some_and(|p| is_charging(&p.charging_status));
                let level = power.map_or(0, |p| {
                    if charging && p.level > 99 {
                        100
                    } else {
                        p.level
                    }
                });
                let text = if level == -1 || delayed {
                    "-".to_string()
                } else {
                    level.to_string()
                };
                value = Some((format!("{text}%"), (0..=10).contains(&level) && !charging));
            }
            if fields.is_battery_supported == Some(false)
                || self.setup == SetupStatus::RestartRequired
            {
                chosen = None;
                value = None;
            }
        }
        if let Some((value, low)) = value {
            result = result.child(
                div()
                    .text_color(rgb(if low { 0xc8323c } else { 0xcccccc }))
                    .child(value),
            );
        }
        if let Some(chosen) = chosen {
            let size = if let Icon::Source(_, size) = &chosen {
                *size
            } else {
                26.
            };
            result = result.child(
                div()
                    .size(css(size))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(match chosen {
                        Icon::Image(path) => img(SharedString::from(path))
                            .size(css(20.))
                            .into_any_element(),
                        // background-size:20px does not size a CSS mask. This
                        // viewBox-only mask fills its 26px positioning area.
                        Icon::Power(color) => svg()
                            .path("synapse/battery-off.svg")
                            .size(css(26.))
                            .text_color(rgb(color))
                            .into_any_element(),
                        Icon::Source(path, size) => img(path).size(css(size)).into_any_element(),
                        Icon::Mask(path) => div()
                            .relative()
                            .size(css(26.))
                            .overflow_hidden()
                            .child(
                                svg()
                                    .absolute()
                                    .left_0()
                                    .top_0()
                                    .path(path)
                                    // mask-size:auto uses the PS file's intrinsic
                                    // 24px dimensions at the default 0% position.
                                    // Its 2px repeated strips contain no artwork.
                                    .size(css(if fields.is_playstation == Some(true) {
                                        24.
                                    } else {
                                        26.
                                    }))
                                    .text_color(rgb(0xffffff)),
                            )
                            .into_any_element(),
                    }),
            );
        }
        if let Some(tip) = tip {
            let console = fields.is_xbox == Some(true) || fields.is_playstation == Some(true);
            let headset = fields.is_xbox == Some(true) && self.category == "Headset";
            let measure = fields.is_playstation == Some(true)
                || (fields.is_xbox == Some(true)
                    && self.category.eq_ignore_ascii_case("AUDIO")
                    && fields
                        .sub_category
                        .as_deref()
                        .is_some_and(|s| s.eq_ignore_ascii_case("EARBUDS")));
            let grid = self.grid;
            let card = self.card;
            result = result
                .on_prepaint(move |bounds, _, _| anchor_right.set(bounds.left()))
                .on_hover(window.listener_for(&state, move |s, hovered, window, cx| {
                    s.hover = *hovered;
                    s.flipped = *hovered
                        && if headset {
                            f32::from(window.viewport_size().width - s.anchor_right.get())
                                < f32::from(window.rem_size()) / 16. * 300.
                        } else {
                            measure && s.tooltip_right.get() > window.viewport_size().width
                        };
                    grid.update(cx, |grid, cx| grid.hover_card(&card, *hovered, cx));
                    cx.notify();
                }))
                .child(
                    div()
                        .absolute()
                        .left_0()
                        // The zero-height source tips-container is vertically
                        // centered by .batt's align-items:center. A 24px standby
                        // icon therefore differs from the 26px ordinary icon.
                        .top(relative(0.5))
                        .w_0()
                        .child(
                            div()
                                .absolute()
                                .top(css(if headset { 35. } else { 20. }))
                                .when(!console || flipped, |view| view.right(css(-20.)))
                                .when(console && !flipped, |view| {
                                    view.left(css(if headset { 10. } else { 0. }))
                                })
                                .w_auto()
                                .max_w(css(if headset {
                                    300.
                                } else if console {
                                    270.
                                } else {
                                    280.
                                }))
                                .when(tip == "STANDBY_MODE_OFF_TOOLTIP", |view| view.w(css(280.)))
                                .when(tip == "BATTERY_ERROR_TIPS", |view| {
                                    view.w(css(352.)).left_0()
                                })
                                .px(css(10.))
                                .py(css(8.))
                                .border_1()
                                .border_color(rgb(0x5d5d5d))
                                .bg(rgb(0x000000))
                                .text_color(rgb(0xcccccc))
                                .font_family("Roboto")
                                .text_size(css(14.))
                                .line_height(css(16.))
                                .whitespace_normal()
                                .opacity(opacity)
                                .when(!hovered, |s| s.invisible())
                                .on_prepaint(move |bounds, _, _| tip_right.set(bounds.right()))
                                .child(tooltip_text(
                                    tip,
                                    matches!(
                                        tip,
                                        "XBOX_SHORTCUT_TOOLTIP"
                                            | "XBOX_SHORTCUT_TOOLTIP_THREE_MODE"
                                            | "XBOX_SHORTCUT_TOOLTIP_HEADSET"
                                            | "XBOX_EARBUDS_TOOLTIP"
                                            | "PS_MODE_TOOLTIP"
                                            | "ARCADE_PS_MODE_TOOLTIP"
                                    ),
                                )),
                        ),
                );
        }
        result
    }
}

/// CSS pre-line retains explicit newlines while collapsing each line's spaces.
/// Normal tooltips collapse all whitespace, including source-language newlines.
fn tooltip_text(key: &str, pre_line: bool) -> String {
    let text = i18n::t(key);
    if pre_line {
        text.split('\n')
            .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        text.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}
