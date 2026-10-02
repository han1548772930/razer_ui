//! IotPopupRoot 28256: gt -> GENERAL / dt (Gamer Room) / rt (Key Light).
//! Network/device results below exist only in explicitly selected preview scenes.
//! No native discovery, Wi-Fi configuration, localStorage or identify call is made.
use super::service_pages::source_link;
use crate::ui::scroll::SourceScrollable as _;
use crate::{
    features::Choice,
    i18n,
    ui::{
        surface,
        theme::{IotColors, MainPageColors, PaletteColors, ProfileAlertColors},
    },
};
use gpui_kit::base::Button as BaseButton;
use gpui_kit::component::{
    button::{Button, ButtonCustomVariant, ButtonVariants},
    input::{Input, InputState},
    select::{SelectEvent, SelectState},
    spinner::Spinner,
    tooltip::Tooltip,
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};

#[cfg(test)]
#[path = "iot_popup_tests.rs"]
mod tests;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum DeviceKind {
    General,
    GamerRoom,
    KeyLight,
}
pub(super) fn open(kind: DeviceKind, window: &mut Window, cx: &mut App) -> Entity<IotPopup> {
    cx.new(|cx| IotPopup::new(kind, window, cx))
}
impl DeviceKind {
    fn title(self) -> String {
        i18n::t(match self {
            Self::General => "ADD_WIFI_DEVICE",
            Self::GamerRoom => "ADD_OTHER_WIFI_DEVICE",
            Self::KeyLight => "ADD_KEY_LIGHT",
        })
    }
    fn help_url(self) -> &'static str {
        if self == Self::GamerRoom {
            "https://mysupport.razer.com/app/answers/detail/a_id/5753"
        } else {
            "https://mysupport.razer.com/app/answers/detail/a_id/5911"
        }
    }
    fn compatible_url(self) -> &'static str {
        if self == Self::GamerRoom {
            "https://mysupport.razer.com/app/answers/detail/a_id/5895"
        } else {
            "https://mysupport.razer.com/app/answers/detail/a_id/6194"
        }
    }
    fn app_url(self) -> &'static str {
        if self == Self::GamerRoom {
            "https://rzr.to/gamer-room-app"
        } else {
            "https://rzr.to/streaming-dl"
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Discovery {
    Unknown,
    Scanning,
    Devices,
    NetworkDevices,
    NewDevices,
    Empty,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum NetworkForm {
    Ready,
    PasswordError,
    Connecting,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Screen {
    General,
    Prepare,
    Mobile,
    Discover(Discovery),
    Network(NetworkForm),
    Success { new_device: bool },
    WifiDisabled,
    NoNetwork,
    DeviceNotFound,
    UnableToAdd,
}
#[derive(Clone, Copy)]
struct PreviewDevice {
    id: &'static str,
    name: &'static str,
    icon: &'static str,
    image: &'static str,
    new_device: bool,
}
const KEY_LIGHT: PreviewDevice = PreviewDevice {
    id: "key-light-network",
    name: "Razer Key Light Chroma · 示例",
    icon: "synapse/iot-category-key-light.svg",
    image: "synapse/iot-key-light-product.png",
    new_device: false,
};
const NEW_KEY_LIGHT: PreviewDevice = PreviewDevice {
    id: "key-light-new",
    name: "Razer Key Light Chroma A1B2 · 示例",
    icon: "synapse/iot-category-key-light.svg",
    image: "synapse/iot-key-light-product.png",
    new_device: true,
};
const ROOM_DEVICES: &[PreviewDevice] = &[
    PreviewDevice {
        id: "room-bulb",
        name: "Aether Light Bulb · 示例",
        icon: "synapse/iot-category-bulb.svg",
        image: "synapse/gr-bulb.png",
        new_device: false,
    },
    PreviewDevice {
        id: "room-lamp",
        name: "Aether Lamp Pro · 示例",
        icon: "synapse/iot-category-lamp.svg",
        image: "synapse/gr-lamp-pro.png",
        new_device: false,
    },
    PreviewDevice {
        id: "room-strip",
        name: "Aether Light Strip · 示例",
        icon: "synapse/iot-category-strip.svg",
        image: "synapse/gr-strip.png",
        new_device: false,
    },
];
const SCENES: &[(&str, &str)] = &[
    ("live", "真实状态 · 设备扫描未读取"),
    ("general", "预览 · 设备类型选择"),
    ("gr-prepare", "预览 · Gamer Room 准备"),
    ("gr-mobile", "预览 · Gamer Room 手机扫码"),
    ("gr-scanning", "预览 · Gamer Room 扫描中"),
    ("gr-devices", "预览 · Gamer Room 设备列表"),
    ("gr-empty", "预览 · Gamer Room 未找到设备"),
    ("gr-success", "预览 · Gamer Room 添加成功"),
    ("gr-wifi", "预览 · Gamer Room Wi-Fi 未启用"),
    ("gr-network", "预览 · Gamer Room 无网络"),
    ("kl-prepare", "预览 · Key Light 准备"),
    ("kl-mobile", "预览 · Key Light 手机扫码"),
    ("kl-scanning", "预览 · Key Light 扫描中"),
    ("kl-devices", "预览 · Key Light 新旧设备列表"),
    ("kl-network-devices", "预览 · Key Light 仅已有设备"),
    ("kl-new-devices", "预览 · Key Light 仅新设备"),
    ("kl-empty", "预览 · Key Light 未找到设备"),
    ("kl-choose", "预览 · Key Light 网络选择"),
    ("kl-password", "预览 · Key Light 密码错误"),
    ("kl-connecting", "预览 · Key Light 连接中"),
    ("kl-success", "预览 · Key Light 已有设备添加成功"),
    ("kl-new-success", "预览 · Key Light 新设备安装入口"),
    ("kl-wifi", "预览 · Key Light Wi-Fi 未启用"),
    ("kl-network", "预览 · Key Light 无网络"),
    ("kl-not-found", "预览 · Key Light 入网后未找到"),
    ("kl-unable", "预览 · Key Light 无法添加"),
];
fn scene_choices() -> Vec<Choice> {
    SCENES
        .iter()
        .map(|(id, name)| Choice::new(*id, *name))
        .collect()
}
fn network_choices() -> Vec<Choice> {
    vec![
        Choice::new("sample-home", "示例家庭网络 2.4 GHz"),
        Choice::new("sample-guest", "示例访客网络 2.4 GHz"),
    ]
}

pub(super) struct IotPopup {
    initial_kind: DeviceKind,
    kind: DeviceKind,
    screen: Screen,
    preview: bool,
    scenes: Entity<SelectState<Vec<Choice>>>,
    networks: Entity<SelectState<Vec<Choice>>>,
    password: Entity<InputState>,
    password_visible: bool,
    device: PreviewDevice,
    notice: String,
    focus: FocusHandle,
    open: bool,
    return_focus: Option<FocusHandle>,
    _subscriptions: Vec<Subscription>,
}
impl IotPopup {
    fn new(kind: DeviceKind, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let scenes =
            cx.new(|cx| SelectState::new(scene_choices(), Some(IndexPath::new(0)), window, cx));
        let networks =
            cx.new(|cx| SelectState::new(network_choices(), Some(IndexPath::new(0)), window, cx));
        let password = cx.new(|cx| InputState::new(window, cx).masked(true));
        let subscription =
            cx.subscribe_in(&scenes, window, |this: &mut Self, _, event, window, cx| {
                if let SelectEvent::Confirm(Some(key)) = event {
                    this.choose_scene(key, window, cx);
                }
            });
        let return_focus = window.focused(cx);
        let focus = cx.focus_handle();
        focus.focus(window, cx);
        Self {
            initial_kind: kind,
            kind,
            screen: if kind == DeviceKind::General {
                Screen::General
            } else {
                Screen::Prepare
            },
            preview: false,
            scenes,
            networks,
            password,
            password_visible: false,
            device: KEY_LIGHT,
            notice: String::new(),
            focus,
            open: true,
            return_focus,
            _subscriptions: vec![subscription],
        }
    }
    pub(super) fn is_open(&self) -> bool {
        self.open
    }
    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.reset_network_form(window, cx);
        if let Some(focus) = self.return_focus.take() {
            focus.focus(window, cx);
        }
        self.open = false;
        cx.notify();
    }
    fn show(&mut self, screen: Screen, window: &mut Window, cx: &mut Context<Self>) {
        // React unmounts `it` when leaving CHOOSE_NETWORK. Its local password,
        // visibility and selected-network index must not leak into a later visit.
        if matches!(self.screen, Screen::Network(_)) && !matches!(screen, Screen::Network(_)) {
            self.reset_network_form(window, cx);
        }
        self.screen = screen;
        self.notice.clear();
        self.focus.focus(window, cx);
        cx.notify();
    }
    fn choose_kind(&mut self, kind: DeviceKind, window: &mut Window, cx: &mut Context<Self>) {
        self.kind = kind;
        self.show(Screen::Prepare, window, cx);
    }
    fn reset_network_form(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.password.update(cx, |state, cx| {
            state.set_value("", window, cx);
            state.set_masked(true, window, cx);
        });
        self.password_visible = false;
        self.networks.update(cx, |state, cx| {
            state.set_selected_index(Some(IndexPath::new(0)), window, cx);
        });
    }
    fn choose_scene(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.reset_network_form(window, cx);
        self.preview = key != "live";
        self.kind = if key.starts_with("gr-") {
            DeviceKind::GamerRoom
        } else if key.starts_with("kl-") {
            DeviceKind::KeyLight
        } else if key == "general" {
            DeviceKind::General
        } else {
            self.initial_kind
        };
        self.device = if self.kind == DeviceKind::GamerRoom {
            ROOM_DEVICES[0]
        } else if matches!(
            key,
            "kl-choose" | "kl-password" | "kl-connecting" | "kl-new-success"
        ) {
            NEW_KEY_LIGHT
        } else {
            KEY_LIGHT
        };
        let suffix = key.split_once('-').map(|(_, v)| v).unwrap_or(key);
        let screen = match suffix {
            "general" => Screen::General,
            "mobile" => Screen::Mobile,
            "scanning" => Screen::Discover(Discovery::Scanning),
            "devices" => Screen::Discover(Discovery::Devices),
            "network-devices" => Screen::Discover(Discovery::NetworkDevices),
            "new-devices" => Screen::Discover(Discovery::NewDevices),
            "empty" => Screen::Discover(Discovery::Empty),
            "choose" => Screen::Network(NetworkForm::Ready),
            "password" => Screen::Network(NetworkForm::PasswordError),
            "connecting" => Screen::Network(NetworkForm::Connecting),
            "success" => Screen::Success { new_device: false },
            "new-success" => Screen::Success { new_device: true },
            "wifi" => Screen::WifiDisabled,
            "network" => Screen::NoNetwork,
            "not-found" => Screen::DeviceNotFound,
            "unable" => Screen::UnableToAdd,
            _ => {
                if self.kind == DeviceKind::General {
                    Screen::General
                } else {
                    Screen::Prepare
                }
            }
        };
        self.show(screen, window, cx);
    }
    fn mobile(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show(Screen::Mobile, window, cx);
    }
    fn search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show(
            Screen::Discover(if self.preview {
                Discovery::Scanning
            } else {
                Discovery::Unknown
            }),
            window,
            cx,
        );
    }
    fn select_device(
        &mut self,
        device: PreviewDevice,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.preview {
            return;
        }
        self.device = device;
        self.show(
            if self.kind == DeviceKind::KeyLight && device.new_device {
                Screen::Network(NetworkForm::Ready)
            } else {
                Screen::Success { new_device: false }
            },
            window,
            cx,
        );
    }
    fn general(&self, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .items_center()
            .text_center()
            .child(
                div()
                    .mt(surface::css(20.))
                    .child(i18n::t("LIGHTING_DEVICE_INTRO")),
            )
            .child(
                h_flex()
                    .justify_center()
                    .flex_wrap()
                    .gap(surface::css(20.))
                    .children(
                        [DeviceKind::GamerRoom, DeviceKind::KeyLight]
                            .into_iter()
                            .map(|kind| {
                                v_flex()
                                    .items_center()
                                    .child(
                                        BaseButton::new(if kind == DeviceKind::GamerRoom {
                                            "iot-type-gr"
                                        } else {
                                            "iot-type-kl"
                                        })
                                        .p_0()
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .accessibility_label(i18n::t(
                                            if kind == DeviceKind::GamerRoom {
                                                "GAMER_ROOM_DEVICES"
                                            } else {
                                                "KEY_LIGHTS"
                                            },
                                        ))
                                        .w(surface::css(250.))
                                        .h(surface::css(150.))
                                        .mt(surface::css(46.))
                                        .mb(surface::css(20.))
                                        .rounded(cx.theme().font_size * (5. / 16.))
                                        .border_2()
                                        .border_color(MainPageColors.detail_surface())
                                        .bg(MainPageColors.detail_surface())
                                        .hover(|style| style.border_color(cx.theme().primary))
                                        .focus_visible(|style| {
                                            style.border_color(cx.theme().primary)
                                        })
                                        .child(
                                            v_flex()
                                                .items_center()
                                                .child(
                                                    img(if kind == DeviceKind::GamerRoom {
                                                        "synapse/gr-add-device.svg"
                                                    } else {
                                                        "synapse/iot-key-light.svg"
                                                    })
                                                    .size(surface::css(56.)),
                                                )
                                                .child(
                                                    div()
                                                        .mt(surface::css(20.))
                                                        .text_size(surface::css(12.))
                                                        .child(i18n::t(
                                                            if kind == DeviceKind::GamerRoom {
                                                                "GAMER_ROOM_DEVICES"
                                                            } else {
                                                                "KEY_LIGHTS"
                                                            },
                                                        )),
                                                ),
                                        )
                                        .on_click(
                                            cx.listener(move |this, _, window, cx| {
                                                this.choose_kind(kind, window, cx)
                                            }),
                                        ),
                                    )
                                    .child(source_link(
                                        if kind == DeviceKind::GamerRoom {
                                            "iot-compatible-gr"
                                        } else {
                                            "iot-compatible-kl"
                                        },
                                        i18n::t("LIGHTING_DEVICE_COMPATIBLE_DEVICES"),
                                        kind.compatible_url(),
                                        cx,
                                    ))
                            }),
                    ),
            )
            .into_any_element()
    }
    fn prepare(&self, cx: &mut Context<Self>) -> AnyElement {
        let room = self.kind == DeviceKind::GamerRoom;
        let mut body = v_flex()
            .w(surface::css(520.))
            .max_w_full()
            .mx_auto()
            .items_center()
            .text_center()
            .child(
                img(if room {
                    "synapse/gr-add-device.svg"
                } else {
                    "synapse/iot-wifi-add.svg"
                })
                .w(surface::css(if room { 56. } else { 48. }))
                .h(surface::css(if room { 56. } else { 36. }))
                .mt(surface::css(if room { 30. } else { 25. })),
            )
            .child(
                div()
                    .mt(surface::css(if room { 24. } else { 20. }))
                    .child(i18n::t(if room {
                        "ADD_OTHER_WIFI_DEVICE_HOME_DESC"
                    } else {
                        "ADD_KEY_LIGHTS_DES"
                    })),
            )
            .when(!room, |column| {
                column.child(
                    div()
                        .mt(surface::css(20.))
                        .mb(surface::css(6.))
                        .child(i18n::t("LIGHTING_DEVICE_SCANNING_MSG")),
                )
            })
            .child(
                div()
                    .mt(surface::css(6.))
                    .mb(surface::css(if room { 22. } else { 0. }))
                    .child(source_link(
                        "iot-prepare-help",
                        i18n::t("SHOW_ME_HOW"),
                        self.kind.help_url(),
                        cx,
                    )),
            );
        if room {
            body = body
                .child(
                    self.mobile_button(
                        "iot-prepare-mobile",
                        i18n::t("ADD_OTHER_WIFI_DEVICE_HOME_MOBILE_DEVICE_DESC"),
                        420.,
                        cx,
                    )
                    .on_click(cx.listener(|this, _, window, cx| this.mobile(window, cx))),
                )
                .child(
                    div()
                        .mt(surface::css(60.))
                        .child(i18n::t("ADD_OTHER_WIFI_DEVICE_HOME_DESC1")),
                );
        }
        body = body.child(
            h_flex()
                .justify_center()
                .gap(surface::css(10.))
                .mt(surface::css(27.))
                .when(!room || self.initial_kind == DeviceKind::General, |row| {
                    row.child(
                        action("iot-type-back", i18n::t("BACK"), false, cx).on_click(cx.listener(
                            |this, _, window, cx| {
                                this.kind = DeviceKind::General;
                                this.show(Screen::General, window, cx)
                            },
                        )),
                    )
                })
                .child(
                    action("iot-search", i18n::t("SEARCH"), true, cx)
                        .on_click(cx.listener(|this, _, window, cx| this.search(window, cx))),
                ),
        );
        if !room {
            body = body.child(
                self.mobile_button(
                    "iot-key-light-mobile",
                    i18n::t("LIGHTING_DEVICE_SETUP_USING_MOBILE_PHONE"),
                    486.,
                    cx,
                )
                .mt(surface::css(60.))
                .on_click(cx.listener(|this, _, window, cx| this.mobile(window, cx))),
            );
        }
        body.into_any_element()
    }
    fn mobile_button(&self, id: &'static str, label: String, width: f32, cx: &App) -> Button {
        Button::new(id)
            .small()
            .w(surface::css(width))
            .max_w_full()
            .h(surface::css(60.))
            .p(surface::css(10.))
            .border_0()
            .rounded(cx.theme().font_size * (5. / 16.))
            .custom(
                ButtonCustomVariant::new(cx)
                    .color(MainPageColors.mobile_action())
                    .hover(MainPageColors.mobile_action())
                    .active(MainPageColors.mobile_action()),
            )
            .child(
                h_flex()
                    .w_full()
                    .gap(surface::css(10.))
                    .child(img("synapse/gr-mobile.svg").size(surface::css(40.)))
                    .child(div().flex_1().text_left().whitespace_normal().child(label))
                    .child(img("synapse/gr-action-arrow.svg").size(surface::css(24.))),
            )
    }
    fn mobile_page(&self, cx: &mut Context<Self>) -> AnyElement {
        let room = self.kind == DeviceKind::GamerRoom;
        v_flex()
            .items_center()
            .text_center()
            .child(
                h_flex()
                    .mt(surface::css(10.))
                    .gap(surface::css(10.))
                    .child(
                        img(if room {
                            "synapse/gr-app.svg"
                        } else {
                            "synapse/iot-streaming-app.svg"
                        })
                        .size(surface::css(32.)),
                    )
                    .child(if room {
                        "Razer Gamer Room"
                    } else {
                        "Razer Streaming"
                    }),
            )
            .child(
                div()
                    .mt(surface::css(14.))
                    .mb(surface::css(20.))
                    .child(i18n::t("LIGHTING_DEVICE_MOBILE_QR_DOWNLOAD_APP")),
            )
            .child(
                img(if room {
                    "synapse/gr-qr.svg"
                } else {
                    "synapse/iot-key-light-qr.svg"
                })
                .size(surface::css(110.))
                .mb(surface::css(20.)),
            )
            .child(div().mb(surface::css(20.)).child(source_link(
                "iot-app-download",
                self.kind.app_url(),
                self.kind.app_url(),
                cx,
            )))
            .child(source_link(
                "iot-mobile-help",
                i18n::t("LIGHTING_DEVICE_MOBILE_QR_FAQ"),
                "https://mysupport.razer.com/app/answers/detail/a_id/5911",
                cx,
            ))
            .child(
                action("iot-mobile-back", i18n::t("BACK"), false, cx)
                    .mt(surface::css(30.))
                    .on_click(cx.listener(|this, _, window, cx| {
                        // Both `rt` and `dt` pass their start page to `B`,
                        // including Key Light entry from the network error.
                        this.show(Screen::Prepare, window, cx)
                    })),
            )
            .into_any_element()
    }
    fn empty(&self, cx: &App) -> AnyElement {
        let room = self.kind == DeviceKind::GamerRoom;
        v_flex()
            .w(surface::css(520.))
            .max_w_full()
            .items_center()
            .gap(surface::css(20.))
            .text_center()
            .child(
                img(if room {
                    "synapse/gr-add-device.svg"
                } else {
                    "synapse/iot-not-found.svg"
                })
                .size(surface::css(48.)),
            )
            .child(
                div()
                    .line_height(surface::css(14.))
                    .child(i18n::t("LIGHTING_DEVICE_NO_DEVICE_FOUND").to_uppercase()),
            )
            .when(room, |column| {
                column.child(source_link(
                    "iot-empty-compatible",
                    i18n::t("VIEW_COMPATIBLE_DEVICES"),
                    self.kind.compatible_url(),
                    cx,
                ))
            })
            .child(i18n::t(if room {
                "ADD_OTHER_WIFI_DEVICE_HOME_SCAN_DESC"
            } else {
                "LIGHTING_DEVICE_NO_DEVICE_FOUND_DESC"
            }))
            .child(source_link(
                "iot-empty-help",
                i18n::t("SHOW_ME_HOW"),
                self.kind.help_url(),
                cx,
            ))
            .into_any_element()
    }
    fn list_header(&self, new: bool, scanning: bool, cx: &App) -> AnyElement {
        h_flex()
            .w(surface::css(420.))
            .max_w_full()
            .gap(surface::css(10.))
            .mb(surface::css(10.))
            .child(
                div().text_size(surface::css(14.)).child(
                    i18n::t(if new {
                        "NEW_DEVICES"
                    } else {
                        "LIGHTING_DEVICE_SCANNED_DEVICES_DEVICES_ON_THE_NETWORK"
                    })
                    .to_uppercase(),
                ),
            )
            .when(new, |row| {
                row.child(
                    BaseButton::new("iot-new-device-help")
                        .p_0()
                        .size(surface::css(14.))
                        .border_0()
                        .accessibility_label(i18n::t(
                            "LIGHTING_DEVICE_SCANNING_SUCCESS_NEW_DEVICE_TOOLTIP",
                        ))
                        .child(img("synapse/iot-new-help.svg").size_full())
                        .tooltip(|window, cx| {
                            Tooltip::element(|_, cx| {
                                v_flex()
                                    .w(surface::css(305.))
                                    .px(surface::css(10.))
                                    .py(surface::css(8.))
                                    .bg(cx.theme().popover)
                                    .border_1()
                                    .border_color(PaletteColors.picker_border())
                                    .text_size(surface::css(14.))
                                    .child(div().mb(surface::css(10.)).child(i18n::t(
                                        "LIGHTING_DEVICE_SCANNING_SUCCESS_NEW_DEVICE_TOOLTIP",
                                    )))
                                    .child(
                                        img("synapse/iot-new-device-mac.png")
                                            .w(surface::css(285.))
                                            .h(surface::css(145.))
                                            .rounded(surface::css(3.)),
                                    )
                            })
                            .build(window, cx)
                        }),
                )
            })
            .when(scanning, |row| {
                row.child(Spinner::new().with_size(cx.theme().font_size * 1.25))
            })
            .into_any_element()
    }
    fn empty_list(&self, cx: &App) -> AnyElement {
        // `Be` keeps a plain empty row inside either section when the other
        // section contains devices; `Pe` is only the whole-list empty state.
        div()
            .w(surface::css(420.))
            .max_w_full()
            .mb(surface::css(10.))
            .text_left()
            .text_size(surface::css(14.))
            .text_color(cx.theme().muted_foreground)
            .child(i18n::t("NO_DEVICE_FOUND"))
            .into_any_element()
    }
    fn device_row(&self, device: PreviewDevice, cx: &mut Context<Self>) -> AnyElement {
        h_flex()
            .w(surface::css(420.))
            .max_w_full()
            .gap(surface::css(5.))
            .mb(surface::css(5.))
            .child(
                BaseButton::new(SharedString::from(format!("iot-device-{}", device.id)))
                    .accessibility_label(device.name)
                    .flex()
                    .items_center()
                    .w(surface::css(375.))
                    .h(surface::css(40.))
                    .p(surface::css(10.))
                    .border_2()
                    .border_color(cx.theme().transparent)
                    .rounded(cx.theme().font_size * (5. / 16.))
                    .bg(cx.theme().popover)
                    .hover(|style| style.border_color(cx.theme().primary))
                    .focus_visible(|style| style.border_color(cx.theme().primary))
                    .child(
                        h_flex()
                            .w_full()
                            .gap(surface::css(7.))
                            .child(img(device.icon).size(surface::css(20.)))
                            .child(div().text_ellipsis().child(device.name)),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.select_device(device, window, cx)
                    })),
            )
            .child(
                BaseButton::new(SharedString::from(format!("iot-identify-{}", device.id)))
                    .flex()
                    .items_center()
                    .justify_center()
                    .w(surface::css(40.))
                    .h(surface::css(40.))
                    .p_0()
                    .border_2()
                    .border_color(cx.theme().transparent)
                    .rounded(cx.theme().font_size * (3. / 16.))
                    .bg(cx.theme().popover)
                    .hover(|style| style.border_color(cx.theme().primary))
                    .focus_visible(|style| style.border_color(cx.theme().primary))
                    .accessibility_label(format!("识别 {}", device.name))
                    .tooltip(|window, cx| Tooltip::new("识别设备").build(window, cx))
                    .child(img("synapse/iot-identify.svg").size(surface::css(20.)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.notice = format!(
                            "界面预览 · 已触发 {} 的识别按钮；没有发送设备指令。",
                            device.name
                        );
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
    fn discovery(&self, state: Discovery, cx: &mut Context<Self>) -> AnyElement {
        let room = self.kind == DeviceKind::GamerRoom;
        let scanning = state == Discovery::Scanning;
        // `ze` chooses this footer from !isScanning && !newDeviceList.length,
        // independently of whether already-networked devices were found.
        let cancel = !scanning
            && (room
                || matches!(
                    state,
                    Discovery::Unknown | Discovery::Empty | Discovery::NetworkDevices
                ));
        v_flex()
            .w(surface::css(600.))
            .max_w_full()
            .mx_auto()
            .items_center()
            .text_center()
            .child(
                v_flex()
                    .mt(surface::css(20.))
                    .mb(surface::css(100.))
                    .items_center()
                    .when(state == Discovery::Unknown, |column| {
                        column.child(
                            div()
                                .w(surface::css(520.))
                                .max_w_full()
                                .py(surface::css(40.))
                                .text_color(cx.theme().muted_foreground)
                                .child("尚未连接 IoT 扫描服务，设备和网络状态未读取。"),
                        )
                    })
                    .when(state == Discovery::Empty, |column| {
                        column.child(self.empty(cx))
                    })
                    .when(
                        matches!(
                            state,
                            Discovery::Scanning
                                | Discovery::Devices
                                | Discovery::NetworkDevices
                                | Discovery::NewDevices
                        ),
                        |column| {
                            let column = column
                                .child(div().mb(surface::css(20.)).child(i18n::t(
                                    "LIGHTING_DEVICE_SCANNED_DEVICES_SELECT_DEVICE",
                                )))
                                .child(self.list_header(false, scanning, cx));
                            let column = if scanning {
                                column.child(skeleton(3, cx))
                            } else if state == Discovery::NewDevices {
                                column.child(self.empty_list(cx))
                            } else if room {
                                column.children(
                                    ROOM_DEVICES
                                        .iter()
                                        .map(|device| self.device_row(*device, cx)),
                                )
                            } else {
                                column.child(self.device_row(KEY_LIGHT, cx))
                            };
                            if room {
                                column
                            } else {
                                column
                                    .child(
                                        div()
                                            .mt(surface::css(20.))
                                            .child(self.list_header(true, scanning, cx)),
                                    )
                                    .child(if scanning {
                                        skeleton(1, cx)
                                    } else if state == Discovery::NetworkDevices {
                                        self.empty_list(cx)
                                    } else {
                                        self.device_row(NEW_KEY_LIGHT, cx)
                                    })
                            }
                        },
                    ),
            )
            .child(
                h_flex()
                    .justify_center()
                    .gap(surface::css(10.))
                    .child(
                        action(
                            "iot-discover-back",
                            i18n::t(if cancel { "CANCEL" } else { "BACK" }),
                            false,
                            cx,
                        )
                        .when(cancel, |button| button.w(surface::css(117.)))
                        .on_click(
                            cx.listener(|this, _, window, cx| {
                                this.show(Screen::Prepare, window, cx)
                            }),
                        ),
                    )
                    .child(
                        refresh_action(
                            "iot-discover-refresh",
                            i18n::t("LIGHTING_DEVICE_SCANNED_DEVICES_DEVICES_ON_THE_NETWORK"),
                            cancel,
                            cx,
                        )
                        .disabled(matches!(state, Discovery::Unknown | Discovery::Scanning))
                        .on_click(cx.listener(|this, _, window, cx| this.search(window, cx))),
                    ),
            )
            .into_any_element()
    }
    fn network_form(&self, state: NetworkForm, cx: &mut Context<Self>) -> AnyElement {
        v_flex().items_center().text_center().text_size(surface::css(14.))
            // These strings are literal English in the original `it` component.
            .child(div().mt(surface::css(20.)).child(format!("Select the network for {} to join",self.device.name)))
            .when(state==NetworkForm::PasswordError,|column|column.child(div().mt(surface::css(20.)).text_color(ProfileAlertColors::new().danger())
                .child("Please enter the correct network password")))
            .child(v_flex().w(surface::css(300.)).mt(surface::css(20.)).items_start()
                .child(h_flex().ml(surface::css(10.)).mb(surface::css(5.)).gap(surface::css(7.))
                    .child("WIFI NETWORK (2.4 GHZ)")
                    .child(Button::new("iot-network-help").ghost().p_0().border_0().size(surface::css(14.))
                        .child(img("synapse/help-default.svg").size_full()).tooltip("Make sure to select the same 2.4 GHz WiFi network used by your PC.")))
                .child(surface::select(&self.networks).items(network_choices()).w(surface::css(285.)).bg(cx.theme().popover).accessibility_label("Choose network"))
                .child(div().ml(surface::css(10.)).mt(surface::css(20.)).mb(surface::css(5.)).child("WIFI PASSWORD"))
                .child(h_flex().relative().ml(surface::css(10.)).w(surface::css(279.))
                    .child(Input::new(&self.password).id("iot-password").aria_label("WIFI PASSWORD").h(surface::css(23.)).w_full()
                        .appearance(false).bordered(false).focus_bordered(false).px(surface::css(4.)).pr(surface::css(28.)).py_0()
                        .border_1().border_color(PaletteColors.border()).bg(cx.theme().popover).rounded(px(0.)).text_size(surface::css(14.)))
                    .child(Button::new("iot-password-visible").ghost().absolute().right(surface::css(5.)).top(surface::css(3.)).p_0().size(surface::css(17.)).border_0()
                        .accessibility_label(if self.password_visible{"隐藏密码"}else{"显示密码"}).child(img("synapse/iot-see-password.svg").size_full())
                        .on_click(cx.listener(|this,_,window,cx|{this.password_visible=!this.password_visible;this.password.update(cx,|state,cx|state.toggle_masked(window,cx));cx.notify();})))))
            .child(v_flex().mt(surface::css(20.)).h(surface::css(104.)).items_center()
                .when(state==NetworkForm::Connecting,|column|column.child(Spinner::new().with_size(cx.theme().font_size*1.25))
                    .child(div().mt(surface::css(20.)).child("This may take a while..."))
                    .child(div().mt(surface::css(20.)).child("Your WiFi connection may be briefly interrupted during the setup. "))))
            .child(h_flex().mt(surface::css(47.)).justify_center().gap(surface::css(10.))
                .child(action("iot-network-back",i18n::t("BACK"),false,cx).on_click(cx.listener(|this,_,window,cx|this.search(window,cx))))
                .child(action("iot-network-connect","connect".into(),true,cx).disabled(state==NetworkForm::Connecting||!self.preview)
                    .on_click(cx.listener(|this,_,window,cx|{if this.preview{this.show(Screen::Network(NetworkForm::Connecting),window,cx)}}))))
            .into_any_element()
    }
    fn network_error(&self, wifi_disabled: bool, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .items_center()
            .text_center()
            .child(
                img("synapse/iot-wifi-error.svg")
                    .w(surface::css(48.))
                    .h(surface::css(36.))
                    .mt(surface::css(23.))
                    .mb(surface::css(30.)),
            )
            .child(
                div().mb(surface::css(20.)).child(
                    i18n::t(if wifi_disabled {
                        "LIGHTING_DEVICE_WIFI_NOT_ENABLED_HEADER"
                    } else {
                        "LIGHTING_DEVICE_NO_NETWORK_DETECTED"
                    })
                    .to_uppercase(),
                ),
            )
            .child(
                h_flex()
                    .justify_center()
                    .flex_wrap()
                    .mb(surface::css(60.))
                    .child(i18n::t(if wifi_disabled {
                        "LIGHTING_DEVICE_WIFI_NOT_ENABLED_ENABLE_WIFI"
                    } else {
                        "LIGHTING_DEVICE_HAS_NETWORK_ACCESS"
                    }))
                    // `st` draws this link with no handler in the source.
                    .when(wifi_disabled, |row| {
                        row.child(
                            div().underline().child(i18n::t(
                                "LIGHTING_DEVICE_WIFI_NOT_ENABLED_WINDOWS_SETTINGS",
                            )),
                        )
                    }),
            )
            .child(
                self.mobile_button(
                    "iot-network-mobile",
                    i18n::t("LIGHTING_DEVICE_SETUP_USING_MOBILE_PHONE"),
                    520.,
                    cx,
                )
                // dt passes goToQRCodeSHDevice, while st only reads goToQRCode.
                // Keep that absent Gamer Room callback; do not invent a native action.
                .when(self.kind == DeviceKind::KeyLight, |button| {
                    button.on_click(cx.listener(|this, _, window, cx| this.mobile(window, cx)))
                }),
            )
            .into_any_element()
    }
    fn device_error(&self, not_found: bool, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .min_h(surface::css(367.))
            .justify_between()
            .items_center()
            .text_center()
            .child(
                v_flex()
                    .items_center()
                    .child(
                        img("synapse/iot-not-found.svg")
                            .size(surface::css(48.))
                            .my(surface::css(24.)),
                    )
                    .child(
                        div().mb(surface::css(20.)).child(
                            i18n::t(if not_found {
                                "LIGHTING_DEVICE_NO_DEVICE_FOUND"
                            } else {
                                "LIGHTING_DEVICE_PAIRING_FAILED"
                            })
                            .to_uppercase(),
                        ),
                    )
                    .child(div().max_w(surface::css(520.)).child(i18n::t(if not_found {
                        "LIGHTING_DEVICE_NO_DEVICE_FOUND_DESC"
                    } else {
                        "LIGHTING_DEVICE_PAIRING_FAILED_MSG_1"
                    })))
                    .when(not_found, |column| {
                        column.child(div().mt(surface::css(22.)).child(source_link(
                            "iot-error-help",
                            i18n::t("SHOW_ME_HOW"),
                            DeviceKind::KeyLight.help_url(),
                            cx,
                        )))
                    })
                    .when(!not_found, |column| {
                        column.child(
                            v_flex()
                                .max_w(surface::css(520.))
                                .my(surface::css(20.))
                                .child(i18n::t("LIGHTING_DEVICE_PAIRING_FAILED_MSG_2"))
                                .child(
                                    h_flex()
                                        .justify_center()
                                        .flex_wrap()
                                        .gap(surface::css(3.))
                                        .child(i18n::t("LIGHTING_DEVICE_PAIRING_FAILED_MSG_3"))
                                        .child(
                                            div().text_color(PaletteColors.white()).child(i18n::t(
                                                "LIGHTING_DEVICE_PAIRING_FAILED_MSG_4",
                                            )),
                                        )
                                        .child(i18n::t("LIGHTING_DEVICE_PAIRING_FAILED_MSG_5"))
                                        .child(
                                            div().text_color(IotColors::help_link()).child(
                                                i18n::t("LIGHTING_DEVICE_PAIRING_FAILED_MSG_6"),
                                            ),
                                        )
                                        .child(i18n::t("LIGHTING_DEVICE_PAIRING_FAILED_MSG_7")),
                                ),
                        )
                    }),
            )
            .child(
                h_flex()
                    .justify_center()
                    .gap(surface::css(10.))
                    .child(
                        action("iot-error-cancel", i18n::t("CANCEL"), false, cx)
                            .on_click(cx.listener(|this, _, window, cx| this.search(window, cx))),
                    )
                    .child(
                        refresh_action("iot-error-rescan", i18n::t("SCAN_AGAIN"), true, cx)
                            .border_0()
                            .on_click(cx.listener(|this, _, window, cx| this.search(window, cx))),
                    ),
            )
            .into_any_element()
    }
    fn success(&self, new_device: bool, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .min_h(surface::css(403.))
            .mt(surface::css(20.))
            .justify_between()
            .items_center()
            .text_center()
            .child(
                v_flex()
                    .items_center()
                    .child(i18n::t("LIGHTING_DEVICE_PAIRING_SUCCESS_HEADER"))
                    .when(!new_device, |column| {
                        column.child(
                            div()
                                .mt(surface::css(10.))
                                .child(i18n::t("LIGHTING_DEVICE_PAIRING_SUCCESS_HEADER_1")),
                        )
                    })
                    .child(
                        img(if new_device {
                            "synapse/iot-success.svg"
                        } else {
                            self.device.image
                        })
                        .w(surface::css(if new_device { 48. } else { 250. }))
                        .h(surface::css(if new_device { 48. } else { 140. }))
                        .my(surface::css(20.)),
                    )
                    .when(!new_device, |column| column.child(self.device.name))
                    .when(new_device, |column| {
                        column
                            .child(
                                div()
                                    .mb(surface::css(4.))
                                    .child("We will begin to install your new device."),
                            )
                            .child(
                                Button::new("iot-view-progress")
                                    .ghost()
                                    .small()
                                    .border_0()
                                    .p_0()
                                    .text_size(surface::css(14.))
                                    .child(div().underline().child("View progress"))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.notice =
                                            "界面预览 · 已展示安装进度入口；没有启动设备安装。"
                                                .into();
                                        cx.notify();
                                    })),
                            )
                    }),
            )
            .child(
                h_flex()
                    .justify_center()
                    .gap(surface::css(10.))
                    .child(
                        action(
                            "iot-success-add-more",
                            i18n::t("LIGHTING_DEVICE_PAIRING_SUCCESS_ADD_MORE"),
                            false,
                            cx,
                        )
                        .on_click(cx.listener(|this, _, window, cx| this.search(window, cx))),
                    )
                    .when(!new_device, |row| {
                        row.child(
                            action("iot-success-customize", i18n::t("TAB_CUSTOMIZE"), true, cx)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.notice =
                                        "界面预览 · 已展示设备自定义入口；未创建或连接真实设备。"
                                            .into();
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .into_any_element()
    }
}
fn action(id: &'static str, label: String, primary: bool, cx: &App) -> Button {
    let background = if primary {
        cx.theme().primary
    } else {
        PaletteColors.secondary()
    };
    Button::new(id)
        .xsmall()
        .label(label.to_uppercase())
        .min_w(surface::css(90.))
        .h(surface::css(27.))
        .px(surface::css(16.))
        .py_0()
        .rounded(cx.theme().font_size * (3. / 16.))
        .text_size(surface::css(12.))
        .line_height(surface::css(14.))
        .border_0()
        .custom(
            ButtonCustomVariant::new(cx)
                .color(background)
                .hover(background)
                .active(background.opacity(0.3))
                .foreground(if primary {
                    cx.theme().background
                } else {
                    PaletteColors.white()
                }),
        )
}
fn refresh_action(id: &'static str, label: String, primary: bool, cx: &App) -> Button {
    // `G`/`.icon-btn` has distinct colors and puts its icon before the label.
    let background = if primary {
        cx.theme().primary
    } else {
        cx.theme().background
    };
    Button::new(id)
        .xsmall()
        .accessibility_label(label.clone())
        .min_w(surface::css(90.))
        .h(surface::css(27.))
        .px(surface::css(12.))
        .py_0()
        .rounded(cx.theme().font_size * (3. / 16.))
        .border_1()
        .border_color(if primary {
            PaletteColors.swatch_border()
        } else {
            cx.theme().foreground
        })
        .custom(
            ButtonCustomVariant::new(cx)
                .color(background)
                .hover(background)
                .active(background)
                .foreground(if primary {
                    cx.theme().button_primary_foreground
                } else {
                    cx.theme().foreground
                }),
        )
        .child(
            h_flex()
                .gap(surface::css(7.))
                .child(
                    img(if primary {
                        "synapse/iot-refresh-green.svg"
                    } else {
                        "synapse/iot-refresh.svg"
                    })
                    .size(surface::css(19.))
                    // Original .icon-btn img has margin-left: -1px.
                    .ml(surface::css(-1.)),
                )
                .child(label),
        )
}
fn skeleton(count: usize, cx: &App) -> AnyElement {
    v_flex()
        .w(surface::css(420.))
        // `ee` clips a 420 × 130 SVG; one row still occupies its full height.
        .h(surface::css(130.))
        .max_w_full()
        .gap(surface::css(5.))
        .children((0..count).map(|_| {
            h_flex()
                .h(surface::css(40.))
                .p(surface::css(10.))
                .rounded(surface::css(5.))
                .bg(cx.theme().title_bar.opacity(0.3))
                .child(
                    div()
                        .w(surface::css(300.))
                        .h(surface::css(20.))
                        .rounded(surface::css(3.))
                        .bg(PaletteColors.white().opacity(0.03)),
                )
        }))
        .into_any_element()
}
impl Render for IotPopup {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().into_any_element();
        }
        // Shared modal 82830 adds .show 100ms after mounting; 55 CSS fades
        // the surface and backdrop over 150ms linear, with no slide/scale.
        let opacity = gpui_kit::base::motion::Presence::new("iot-modal-opacity", true)
            .transition(
                gpui_kit::base::motion::Transition::new(std::time::Duration::from_millis(150))
                    .delay(std::time::Duration::from_millis(100))
                    .easing(gpui_kit::base::motion::Easing::Linear),
            )
            .sample(window, cx)
            .progress;
        let body = match self.screen {
            Screen::General => self.general(cx),
            Screen::Prepare => self.prepare(cx),
            Screen::Mobile => self.mobile_page(cx),
            Screen::Discover(state) => self.discovery(state, cx),
            Screen::Network(state) => self.network_form(state, cx),
            Screen::Success { new_device } => self.success(new_device, cx),
            Screen::WifiDisabled => self.network_error(true, cx),
            Screen::NoNetwork => self.network_error(false, cx),
            Screen::DeviceNotFound => self.device_error(true, cx),
            Screen::UnableToAdd => self.device_error(false, cx),
        };
        let viewport = window.viewport_size();
        let display_width = window
            .display(cx)
            .map(|d| d.bounds().size.width)
            .unwrap_or(viewport.width);
        let width = (window.rem_size()
            * (if display_width >= px(1331.) {
                1280.
            } else {
                850.
            } / 16.))
            .min(viewport.width);
        let top = (window.rem_size() * (106. / 16.)).min(viewport.height);
        gpui_kit::base::Dialog::new(cx)
            .focus_handle(self.focus.clone())
            .close_on_backdrop_press(false)
            .on_ok(|_, _, _| false)
            .on_close(cx.listener(|this, _, window, cx| this.close(window, cx)))
            .backdrop(
                div()
                    .absolute()
                    .size_full()
                    .bg(MainPageColors.banner_shade().opacity(0.7 * opacity)),
            )
            .popup(
                v_flex()
                    .id("gamer-room-add-dialog")
                    .test_support()
                    .opacity(opacity)
                    .absolute()
                    .top(top)
                    .left((viewport.width - width) / 2.)
                    .w(width)
                    .h(viewport.height - top)
                    .rounded(surface::css(5.))
                    .bg(cx.theme().background)
                    .occlude()
                    .text_size(surface::css(14.))
                    .line_height(surface::css(17.))
                    .text_color(cx.theme().foreground)
                    .child(
                        h_flex()
                            .relative()
                            .w_full()
                            .h(surface::css(36.))
                            .flex_shrink_0()
                            .justify_center()
                            .border_b_1()
                            .border_color(PaletteColors.picker_border())
                            .font_weight(FontWeight::LIGHT)
                            .child(self.kind.title().to_uppercase())
                            .child(
                                surface::modal_close_button(
                                    "gr-add-close",
                                    "关闭添加设备",
                                    window,
                                    cx,
                                )
                                .absolute()
                                .top_0()
                                .right_0()
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.close(window, cx)),
                                ),
                            ),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .flex_shrink_0()
                            .justify_center()
                            .gap(surface::css(12.))
                            .py(surface::css(10.))
                            .flex_wrap()
                            .child(
                                div()
                                    .text_size(surface::css(12.))
                                    .text_color(if self.preview {
                                        cx.theme().primary
                                    } else {
                                        cx.theme().muted_foreground
                                    })
                                    .child(if self.preview {
                                        "界面预览 · 示例设备及网络"
                                    } else {
                                        "设备与网络状态未读取"
                                    }),
                            )
                            .child(
                                surface::select(&self.scenes)
                                    .items(scene_choices())
                                    .w(surface::css(340.))
                                    .accessibility_label("添加 Wi-Fi 设备界面预览"),
                            ),
                    )
                    .child(
                        v_flex()
                            .id("iot-popup-scroll")
                            .flex_1()
                            .min_h_0()
                            .w_full()
                            .scrollable_y()
                            .items_center()
                            .child(div().w_full().child(body))
                            .when(!self.notice.is_empty(), |column| {
                                column.child(
                                    div()
                                        .mt(surface::css(20.))
                                        .p(surface::css(12.))
                                        .text_color(cx.theme().muted_foreground)
                                        .child(self.notice.clone()),
                                )
                            }),
                    ),
            )
            .into_any_element()
    }
}
