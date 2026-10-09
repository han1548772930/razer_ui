//! Source-derived populated systray widgets (current chunk 492).
//!
//! This module deliberately accepts observations from the host/device channel.
//! It does not discover devices, invent names, or provide fallback rows.  The
//! parent may mount these elements once that channel supplies real data.

use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::*;
use gpui_kit::prelude::FluentBuilder as _;
use gpui_kit::*;
use razer_model::model::DeviceCategory;
use razer_widgets::surface;
use razer_widgets::theme::TrayColors;

/// A source `synapse.devices` item after the host has resolved its display
/// fields.  Every field is optional where the current component checks it.
#[derive(Clone, Debug, Default)]
pub struct TrayWidgetDevice {
    pub id: String,
    pub icon: SharedString,
    pub title: String,
    pub profile: Option<TrayWidgetProfile>,
    pub battery: Option<TrayWidgetBattery>,
}

#[derive(Clone, Debug, Default)]
pub struct TrayWidgetProfile {
    pub name: String,
    pub disabled: bool,
    pub has_dropdown: bool,
    /// This profile belongs to a local workspace, not a device/service read.
    pub local_draft: bool,
}

/// Preserve the source's exact category name lookup. An unrecognised category
/// renders no SVG, as 5492/f does; product pictures are not a fallback.
pub fn source_category_icon(category: &str, sub_category: Option<&str>) -> SharedString {
    // main 5596/C (export Su) gates a truthy subCategory lookup.
    const CATEGORIES: &[&str] = &[
        "KEYBOARD",
        "MOUSE",
        "MOUSEPLUSMAT",
        "GAMEPAD",
        "MONITOR",
        "CASE",
        "AUDIO",
        "BROADCASTER",
        "EGPU",
        "SYSTEM",
        "KEYPAD",
        "CHROMAHDK",
        "ACCESSORY",
        "MOUSEMAT",
        "IOT",
    ];
    const NAMES: &[&str] = &[
        "KEYBOARD",
        "MOUSE",
        "MOUSEMAT",
        "ACCESSORY",
        "CHROMAHDK",
        "KEYPAD",
        "SYSTEM",
        "MOUSEPLUSMAT",
        "EGPU",
        "AUDIO",
        "CASE",
        "MONITOR",
        "CAMERA",
        "MICROPHONE",
        "ARGB_CONTROLLER",
        "GAME_CAPTURE_CARD",
        "MIXER_AUDIO",
        "STRIP",
        "KEY_LIGHT",
        "LAMP",
        "BULB",
        "SPEAKER",
        "STREAM_CONTROLLER",
        "MAINBOARD",
        "GAMEPAD",
        "SPEAKER_HEAD_CUSHION",
        "SPEAKER_HEAD_CUSHION_CLIO_X",
        "CHAIR",
        "LIQUID_CONTROLLER",
        "EARBUDS",
        "EARPHONES",
    ];
    let name = if CATEGORIES.contains(&category) {
        sub_category
            .filter(|value| !value.is_empty())
            .unwrap_or(category)
    } else {
        category
    };
    if NAMES.contains(&name) {
        format!(
            "synapse/tray-widget-category-{}.svg",
            name.to_ascii_lowercase()
        )
        .into()
    } else {
        "".into()
    }
}

/// Lossless subset of the local enum. The source has AUDIO and GAMEPAD, but
/// no HEADSET, CONTROLLER or OTHER entry; callers with the source string must
/// use `source_category_icon` instead of guessing a category conversion.
pub fn category_icon(category: DeviceCategory) -> SharedString {
    let name = match category {
        DeviceCategory::Mouse => "MOUSE",
        DeviceCategory::Keyboard => "KEYBOARD",
        DeviceCategory::Mousepad => "MOUSEMAT",
        DeviceCategory::Keypad => "KEYPAD",
        DeviceCategory::Accessory => "ACCESSORY",
        DeviceCategory::Audio => "AUDIO",
        DeviceCategory::Headset | DeviceCategory::Controller | DeviceCategory::Other => "",
    };
    source_category_icon(name, None)
}

/// Current 492 battery classes. `Paused` maps to the generated paused assets.
/// `Off` uses the current manifest's 26px `icon_device_off.svg` mask.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TrayWidgetBattery {
    Percent { level: u8, charging: bool },
    Paused { level: u8 },
    Charging,
    Charging100,
    Off,
}

impl TrayWidgetBattery {
    fn bucket(level: u8) -> u8 {
        // 492's D(e): 10 stays 10, 11..19 becomes 20, otherwise floor to a
        // ten-point bucket. The caller must provide the observed raw level.
        if level == 10 {
            10
        } else if level > 10 && level < 20 {
            20
        } else {
            (level / 10) * 10
        }
    }

    fn icon_path(&self) -> SharedString {
        match self {
            Self::Percent { level, charging } => {
                if *charging && *level == 100 {
                    "synapse/tray-widget-battery-charging-100.svg".into()
                } else if *charging {
                    "synapse/tray-widget-battery-charging.svg".into()
                } else {
                    format!("synapse/tray-widget-battery-{}.svg", Self::bucket(*level)).into()
                }
            }
            Self::Paused { level } => format!(
                "synapse/tray-widget-battery-paused-{}.svg",
                Self::bucket(*level)
            )
            .into(),
            Self::Charging => "synapse/tray-widget-battery-charging.svg".into(),
            Self::Charging100 => "synapse/tray-widget-battery-charging-100.svg".into(),
            Self::Off => "synapse/tray-widget-battery-off.svg".into(),
        }
    }

    fn level_text(&self) -> Option<String> {
        match self {
            Self::Percent { level, .. } | Self::Paused { level } => Some(format!("{level}%")),
            Self::Charging | Self::Charging100 | Self::Off => None,
        }
    }

    fn is_low(&self) -> bool {
        matches!(self, Self::Percent { level, charging: false } | Self::Paused { level } if *level <= 10)
    }
}

/// Render one source `.synapse>.devices>li` without an activation transport.
/// Use [`widget_row_with_click`] when the parent has a real host route.
pub fn widget_row(device: &TrayWidgetDevice) -> AnyElement {
    widget_row_with_click(device, |_, _, _| {})
}

/// Variant used by the parent once the host has supplied the source row
/// activation transport (`focusDeviceTab`).
pub fn widget_row_with_click(
    device: &TrayWidgetDevice,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let battery = device.battery.as_ref();
    let info = div()
        .id(format!("tray-widget-info-{}", device.id))
        .relative()
        .min_w_0()
        .max_w(surface::css(220.))
        .ml(surface::css(10.))
        .when_some(device.profile.as_ref(), |info, profile| {
            let mut label = div()
                .id(format!("tray-widget-profile-{}", device.id))
                .relative()
                .max_w_full()
                .pr(surface::css(18.))
                .line_height(relative(1.))
                // `disabled` disables pointer events; only :after is .3
                // opacity in the source. The label itself remains #ccc.
                .text_color(TrayColors::text())
                .whitespace_nowrap()
                .truncate()
                .when(profile.local_draft, |label| {
                    label.tooltip(|window, cx| {
                        Tooltip::new(if razer_i18n::locale().eq_ignore_ascii_case("zh-cn") {
                            "本地草稿配置；尚未读取设备的活动配置"
                        } else {
                            "Local draft profile; the active device profile has not been read"
                        })
                        .build(window, cx)
                    })
                })
                .child(profile.name.clone());
            // Source CSS border triangle: 5px 5px 0, centered vertically.
            if profile.has_dropdown {
                let color = TrayColors::muted().opacity(if profile.disabled { 0.3 } else { 1. });
                label = label.child(
                    div()
                        .absolute()
                        .right_0()
                        .top_0()
                        .bottom_0()
                        .flex()
                        .items_center()
                        .child(
                            canvas(
                                |_, _, _| (),
                                move |bounds, _, window, _| {
                                    let mut path = PathBuilder::fill();
                                    path.move_to(bounds.origin);
                                    path.line_to(point(bounds.right(), bounds.top()));
                                    path.line_to(point(bounds.center().x, bounds.bottom()));
                                    path.close();
                                    if let Ok(path) = path.build() {
                                        window.paint_path(path, color);
                                    }
                                },
                            )
                            .w(surface::css(10.))
                            .h(surface::css(5.)),
                        ),
                );
            }
            // Inline-block profile occupies the parent's 16px/1.22 line box.
            // Its intrinsic width keeps the triangle close to the profile.
            info.child(
                h_flex()
                    .min_w_0()
                    .max_w_full()
                    .h(surface::css(16. * 1.22))
                    .child(label),
            )
        })
        .child(
            div()
                .id(format!("tray-widget-title-{}", device.id))
                .min_w_0()
                .text_size(surface::css(12.))
                .text_color(TrayColors::muted())
                .truncate()
                .child(device.title.to_uppercase()),
        );

    h_flex()
        .id(format!("tray-widget-row-{}", device.id))
        .w_full()
        .px(surface::css(20.))
        .py(surface::css(10.))
        .bg(TrayColors::surface())
        .hover(|row| row.bg(TrayColors::launcher()))
        .active(|row| row.bg(rgb(0x393939)))
        .on_click(on_click)
        // f(t) returns a bare SVG, not .icon. Thus .icon margin-right:10px
        // is inapplicable; only .info margin-left:10px separates these lanes.
        .when(!device.icon.is_empty(), |row| {
            row.child(
                img(device.icon.clone())
                    .size(surface::css(40.))
                    .min_w(surface::css(40.))
                    .flex_shrink_0()
                    .object_fit(ObjectFit::Contain),
            )
        })
        .child(info)
        .when_some(battery, |row, battery| {
            row.child(
                h_flex()
                    .id(format!("tray-widget-battery-{}", device.id))
                    .ml_auto()
                    .pl(surface::css(10.))
                    .flex_shrink_0()
                    .justify_end()
                    .text_size(surface::css(14.))
                    .text_color(if battery.is_low() {
                        rgb(0xc8323c).into()
                    } else {
                        TrayColors::text()
                    })
                    .when_some(battery.level_text(), |view, text| view.child(text))
                    .child(
                        div()
                            .size(surface::css(26.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(if matches!(battery, TrayWidgetBattery::Off) {
                                // CSS uses the SVG alpha as a 26px red mask.
                                svg()
                                    .path(battery.icon_path())
                                    .size(surface::css(26.))
                                    .text_color(rgb(0xff0000))
                                    .into_any_element()
                            } else {
                                img(battery.icon_path())
                                    .size(surface::css(20.))
                                    .object_fit(ObjectFit::Contain)
                                    .into_any_element()
                            }),
                    ),
            )
        })
        .into_any_element()
}

/// Render the source `ul.devices` container. An empty list uses the source
/// `.app-section-default` dimensions and leaves the text to the host locale.
pub fn widget_list(
    devices: &[TrayWidgetDevice],
    empty_label: impl Into<SharedString>,
) -> AnyElement {
    widget_list_with_click(devices, empty_label, |_, _, _, _| {}, |_, _, _| {})
}

/// The parent supplies the real focusDeviceTab and activateWindowServiceClient
/// commands. Rendering and measurement never create a transport or a result.
pub fn widget_list_with_click(
    devices: &[TrayWidgetDevice],
    empty_label: impl Into<SharedString>,
    on_device_click: impl Fn(&TrayWidgetDevice, &ClickEvent, &mut Window, &mut App) + Clone + 'static,
    on_title_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let contents = if devices.is_empty() {
        div()
            .id("tray-widgets-empty")
            .w_full()
            .h(surface::css(60.))
            .flex()
            .items_center()
            .justify_center()
            .px(surface::css(20.))
            .text_size(surface::css(14.))
            .text_color(TrayColors::muted())
            .child(empty_label.into())
            .into_any_element()
    } else {
        div()
            .id("tray-widgets")
            .w_full()
            .children(devices.iter().map(|device| {
                let clicked_device = device.clone();
                let on_click = on_device_click.clone();
                widget_row_with_click(device, move |event, window, cx| {
                    on_click(&clicked_device, event, window, cx)
                })
            }))
            .into_any_element()
    };
    div()
        .id("tray-synapse-widget-section")
        .w_full()
        .child(
            div()
                .id("tray-synapse-widget-title")
                .w_full()
                .bg(TrayColors::surface())
                .border_t_2()
                .border_color(TrayColors::launcher())
                .py(surface::css(6.))
                .text_size(surface::css(10.))
                .line_height(relative(1.22))
                .text_color(TrayColors::muted())
                .text_center()
                .on_click(on_title_click)
                .child(super::text("host", "RAZER_SYNAPSE").to_uppercase()),
        )
        .child(
            div()
                .id("tray-synapse-widget-container")
                .w_full()
                .min_h(surface::css(if devices.is_empty() {
                    60.
                } else {
                    60. * devices.len() as f32
                }))
                .child(contents),
        )
        .into_any_element()
}
