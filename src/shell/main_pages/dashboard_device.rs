//! Current 22534/z, G/V/K and 55 CSS; separate from the device toolbar battery.
//! See dashboard-device-current-evidence.json for enum literals and selectors.
use crate::{
    i18n,
    model::{DashboardDeviceMetadata, Device, DeviceCategory, PowerStatus, SetupStatus},
    ui::surface::css,
};
use gpui_kit::base::motion::{self, Easing, Transition};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::time::Duration;

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

pub(super) fn disabled(device: &Device) -> bool {
    (power_off(device)
        && device.dashboard.is_xbox != Some(true)
        && device.dashboard.is_playstation != Some(true))
        || device.setup_status == SetupStatus::RestartRequired
        || device.dashboard.device_power_state.as_deref() == Some("off")
        || (device.dashboard.supports_standby_mode == Some(true)
            && device.dashboard.device_power_state.as_deref() == Some("standby"))
}

#[derive(Clone)]
enum Icon {
    Image(String),
    Power(u32),
    /// Preserve the source slot while its distinct SVG is unavailable.
    Missing(f32),
}

#[derive(IntoElement)]
pub(super) struct DashboardBattery {
    id: ElementId,
    power: Option<PowerStatus>,
    fields: DashboardDeviceMetadata,
    setup: SetupStatus,
    audio: bool,
}

impl DashboardBattery {
    pub(super) fn new(id: String, device: &Device) -> Self {
        Self {
            id: SharedString::from(id).into(),
            power: device.power_status.clone(),
            fields: device.dashboard.clone(),
            setup: device.setup_status,
            audio: device.category == DeviceCategory::Audio,
        }
    }
}

struct BatteryDelay {
    off: bool,
    delayed: bool,
    task: Option<Task<()>>,
    hover: bool,
}
impl BatteryDelay {
    fn new(off: bool, cx: &mut Context<Self>) -> Self {
        Self {
            off,
            delayed: !off,
            task: (!off).then(|| Self::timer(cx)),
            hover: false,
        }
    }
    fn timer(cx: &mut Context<Self>) -> Task<()> {
        cx.spawn(async move |state, cx| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            let _ = state.update(cx, |state, cx| {
                state.delayed = false;
                cx.notify();
            });
        })
    }
    fn update_power(&mut self, off: bool, cx: &mut Context<Self>) {
        if self.off && !off {
            self.delayed = true;
            self.task = Some(Self::timer(cx));
        }
        self.off = off;
    }
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
        let off = self
            .power
            .as_ref()
            .is_some_and(|p| p.charging_status.eq_ignore_ascii_case("off"));
        let state = window.use_keyed_state((self.id.clone(), "battery-delay"), cx, |_, cx| {
            BatteryDelay::new(off, cx)
        });
        state.update(cx, |s, cx| s.update_power(off, cx));
        let delayed = state.read(cx).delayed;
        let hovered = state.read(cx).hover;
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
        let ready = self.setup == SetupStatus::Ready || (self.setup == SetupStatus::Waiting && off);
        if !ready {
            return result;
        }
        let fields = self.fields;
        let standby = fields.supports_standby_mode == Some(true);
        let power_state = fields.device_power_state.as_deref();
        let mut tip = None;
        let mut chosen = None;
        let mut value = None;
        if standby {
            match power_state {
                // This branch uses H (icon_device_power_state_off), not the
                // ordinary red batt-off mask. The current H resource is absent.
                Some("off") => {
                    chosen = Some(Icon::Missing(24.));
                    tip = Some("STANDBY_MODE_OFF_TOOLTIP");
                }
                Some("standby") => {
                    chosen = Some(Icon::Power(0xfd8611));
                    tip = Some("DEVICE_STANDBY_TOOLTIP_LINE1");
                }
                _ => {}
            }
        } else {
            let mut hide_icon = fields.hide_battery_icon.unwrap_or(false);
            if let Some(power) = &self.power {
                let incorrect = is_normal(&power.charging_status) && power.level == -1;
                hide_icon |= incorrect;
                chosen = (!hide_icon).then(|| icon(power));
                if off {
                    tip = Some(if self.audio {
                        "DASHBOARD_AUDIO_DEVICE_OFF_TOOLTIP"
                    } else {
                        "DASHBOARD_DEVICE_OFF_TOOLTIP"
                    });
                    if fields.is_xbox == Some(true) || fields.is_playstation == Some(true) {
                        // Their distinct console glyphs and tooltip branches
                        // require uncached current resources and variant data.
                        chosen = (!hide_icon).then_some(Icon::Missing(26.));
                        tip = None;
                    }
                } else if power.charging_status == "batt-warning" {
                    tip = Some("BATTERY_ERROR_TIPS");
                }
                let suppress_value = fields.is_external_batt == Some(true)
                    || (fields.supports_standby_mode.is_none()
                        && power_state.is_some_and(|s| !s.is_empty()));
                if !suppress_value
                    && !incorrect
                    && !off
                    && fields.show_battery_value.unwrap_or(true)
                {
                    let level = if is_charging(&power.charging_status) && power.level > 99 {
                        100
                    } else {
                        power.level
                    };
                    let text = if level == -1 || delayed {
                        "-".to_string()
                    } else {
                        level.to_string()
                    };
                    value = Some((
                        format!("{text}%"),
                        (0..=10).contains(&level) && !is_charging(&power.charging_status),
                    ));
                }
            }
            // The absent supportsStandbyMode path can report power without a battery.
            if fields.supports_standby_mode.is_none() {
                match power_state {
                    Some("off") => {
                        chosen = Some(Icon::Power(0xff0000));
                        tip = Some(if self.audio {
                            "DASHBOARD_AUDIO_DEVICE_OFF_TOOLTIP"
                        } else {
                            "DASHBOARD_DEVICE_OFF_TOOLTIP"
                        });
                    }
                    Some("active") => {
                        chosen = None;
                    }
                    _ => {}
                }
            }
            if fields.is_battery_supported == Some(false) {
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
            let size = if let Icon::Missing(size) = &chosen {
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
                        Icon::Power(color) => svg()
                            .path("synapse/battery-off.svg")
                            .size(css(20.))
                            .text_color(rgb(color))
                            .into_any_element(),
                        Icon::Missing(_) => Empty.into_any_element(),
                    }),
            );
        }
        if let Some(tip) = tip {
            result = result
                .on_hover(window.listener_for(&state, |s, hovered, _, cx| {
                    s.hover = *hovered;
                    cx.notify();
                }))
                .child(
                    div().absolute().left_0().top(css(33.)).w_0().child(
                        div()
                            .absolute()
                            .right(css(-20.))
                            .w_auto()
                            .max_w(css(280.))
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
                            .child(i18n::t(tip)),
                    ),
                );
        }
        result
    }
}
