//! Product 164/241 pairing pages. Pairing observations belong to the session,
//! independently of the SourceControls lighting profile owned by the workspace.
use crate::{
    i18n,
    model::Device,
    ui::{surface, theme::DockPairingColors as Colors},
};
use gpui_kit::component::*;
use gpui_kit::{prelude::FluentBuilder as _, *};
use serde::Deserialize;
use std::{collections::BTreeMap, sync::OnceLock};

mod dialog;
mod preview;
mod state;
use dialog::DockDialog;
pub(crate) use preview::open_preview;
use state::{Lane, PairingState, Peer, Status};

#[derive(Deserialize)]
struct Spec {
    product_id: u32,
    page: String,
    config: serde_json::Value,
    translations: BTreeMap<String, BTreeMap<String, String>>,
}
fn spec(pid: u32) -> &'static Spec {
    static SPECS: OnceLock<Vec<Spec>> = OnceLock::new();
    SPECS
        .get_or_init(|| {
            serde_json::from_str(include_str!("dock_pairing_data.json"))
                .expect("audited dock sources")
        })
        .iter()
        .find(|s| s.product_id == pid)
        .expect("supported dock product")
}
impl Spec {
    fn dual(&self) -> bool {
        self.config["canPairTwoDevices"].as_bool() == Some(true)
    }
    /// 当前源 `DeviceInfo` 的布尔开关（`showBothDevicesConnectedWarning` 等）。
    fn flag(&self, name: &str) -> bool {
        self.config[name].as_bool() == Some(true)
    }
    /// 当前源每个产品包自己写死的语言键（例如配对工具说明：164 是
    /// `ENABLE_LAUNCH_PAIRING_UTILITY_INFO`，241 源码里是拼写错误的
    /// `ENABLE_LAUNCH_PARING_UTILITY_INFO`）。键从产品配置读取，不用通用包顶替。
    fn config_key(&'static self, name: &str) -> Option<&'static str> {
        self.config[name].as_str()
    }
    fn text(&self, key: &str) -> String {
        self.translations
            .get(&i18n::locale())
            .and_then(|d| d.get(key))
            .or_else(|| self.translations.get("en").and_then(|d| d.get(key)))
            .cloned()
            .unwrap_or_else(|| i18n::t(key))
    }
    fn asset(&self, name: &str) -> SharedString {
        format!("synapse/dock-{}-{name}.svg", self.product_id).into()
    }
}
pub(crate) fn supports_page(pid: u32, key: &str) -> bool {
    matches!(pid, 164 | 241) && spec(pid).page == key
}

pub(crate) struct DockPairing {
    spec: &'static Spec,
    edition: u32,
    layout: u32,
    name: String,
    state: PairingState,
    multi_pairing: bool,
    /// 应用当前已登记的设备（`product_id`, `edition_id`），对应源 `Es` 的比对列表。
    known_devices: Vec<(u32, u32)>,
    dongle: bool,
    original_dongle: bool,
    preview: bool,
    alert: Option<String>,
    modal: Option<Entity<DockDialog>>,
}
struct PreviewDialogRequested;
impl EventEmitter<PreviewDialogRequested> for DockPairing {}

/// 配对文案里的设备名被点击。源用 `Es(peer, devices)` 命中应用设备列表时把名字渲染成
/// `.deviceNameLink`（`cursor:pointer;text-decoration:underline`，hover `#44d62c`），
/// 点击 `z(e)` 切到该设备。
pub(crate) struct DeviceLinkRequested {
    pub(crate) product_id: u32,
    pub(crate) edition_id: u32,
}
impl EventEmitter<DeviceLinkRequested> for DockPairing {}
impl DockPairing {
    pub(crate) fn new(device: &Device) -> Self {
        Self {
            spec: spec(device.product_id),
            edition: device.edition_id,
            layout: device.layout_id,
            name: device
                .product_name
                .get(&i18n::locale().to_lowercase())
                .to_owned(),
            state: PairingState::default(),
            multi_pairing: false,
            known_devices: Vec::new(),
            dongle: false,
            original_dongle: false,
            preview: false,
            alert: None,
            modal: None,
        }
    }
    /// 刷新应用设备列表（数量或成员变化才通知）。
    pub(crate) fn set_known_devices(&mut self, devices: Vec<(u32, u32)>, cx: &mut Context<Self>) {
        if self.known_devices != devices {
            self.known_devices = devices;
            cx.notify();
        }
    }

    pub(crate) fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(modal) = self.modal.take() {
            modal.update(cx, |modal, cx| modal.close(window, cx));
        }
        self.alert = None;
        cx.notify();
    }
    fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.multi_pairing {
            if self.dongle {
                self.alert = Some("配对工具暂不可用。".into());
                cx.notify();
            }
            return;
        }
        if self.preview {
            cx.emit(PreviewDialogRequested);
            return;
        }
        self.dismiss(window, cx);
        self.modal = Some(DockDialog::open(
            self.spec,
            self.edition,
            self.layout,
            self.name.clone(),
            self.state.clone(),
            self.preview,
            window,
            cx,
        ));
        cx.notify();
    }
    fn page(&self, cx: &Context<Self>) -> AnyElement {
        let dual = self.spec.dual();
        let title = if dual || self.multi_pairing {
            "HYPER_SPEED_MULTI_DEVICE_PAIRING"
        } else {
            "HYPERPOLLING_WIRELESS"
        };
        let tips = if dual {
            "MOUSE_DOCK_MULTI_DEVICE_DUALINK_PROPERTIES_TOOLTIP"
        } else if self.multi_pairing {
            "HYPER_SPEED_DUALINK_PROPERTIES_TOOLTIP"
        } else {
            "MULTI__DUALINK_PROPERTIES_TOOLTIP"
        };
        let paired = self.state.peers();
        // `W = DeviceInfo.showBothDevicesConnectedWarning && we(pairedInfo)`：鼠标与
        // 键盘同时在底座上时，源把轮询率说明整行 `V = !W && …` 去掉，改为显示上面那条提示。
        let both_devices_capped =
            dual && self.spec.flag("showBothDevicesConnectedWarning") && paired.len() == 2;
        let mut content = h_flex()
            .items_start()
            .gap(surface::css(if dual && !paired.is_empty() {
                10.
            } else {
                20.
            }))
            .child(
                img(self.spec.asset(if dual || self.multi_pairing {
                    "icon-multideviceparing"
                } else {
                    "icon-multidevicepairing2"
                }))
                .size(surface::css(if dual { 40. } else { 44. }))
                .flex_shrink_0(),
            );
        if paired.is_empty() || self.multi_pairing {
            content = content.child(
                gpui_kit::base::Button::new("dock-open-pairing")
                    .disabled(self.multi_pairing && !self.dongle)
                    .accessibility_label(self.spec.text("OPEN_PAIRING_UTILITY"))
                    .h(surface::css(44.))
                    .text_size(surface::css(14.))
                    .underline()
                    .text_color(cx.theme().foreground)
                    .hover(|s| s.text_color(cx.theme().primary))
                    .focus_visible(|s| s.border_1().border_color(cx.theme().primary))
                    .when(self.multi_pairing && !self.dongle, |s| s.opacity(0.45))
                    .child(self.spec.text("OPEN_PAIRING_UTILITY"))
                    .on_click(cx.listener(|this, _, window, cx| this.open(window, cx))),
            );
        } else {
            content = content
                .child(
                    v_flex()
                        .flex_1()
                        .min_w_0()
                        .gap(surface::css(4.))
                        .children(paired.iter().map(|peer| {
                            // 源 `Es(e, devices)`：只有设备名能在应用设备列表里按
                            // `productId`+`editionId` 命中时才是链接。
                            let linked = self.known_devices.iter().any(|(pid, edition)| {
                                *pid == peer.product_id && *edition == peer.edition
                            });
                            let name = SharedString::from(peer.name.clone());
                            let link = |name: SharedString, product_id: u32, edition_id: u32| {
                                div()
                                    .id(SharedString::from(format!(
                                        "dock-device-link-{product_id}-{edition_id}"
                                    )))
                                    .cursor_pointer()
                                    .underline()
                                    // `.deviceNameLink{cursor:pointer;text-decoration:underline}` +
                                    // `.hyperpolling-span-hover:hover{color:#44d62c}`。
                                    .hover(|style| style.text_color(rgb(0x44d62c)))
                                    .on_click(cx.listener(move |_, _, _, cx| {
                                        cx.emit(DeviceLinkRequested {
                                            product_id,
                                            edition_id,
                                        });
                                    }))
                                    .child(name)
                            };
                            let text = if dual && paired.len() > 1 {
                                // `<lane>: <name>`，名字可能是链接。
                                let prefix = format!("{}: ", self.spec.text(peer.lane.key()));
                                h_flex()
                                    .child(SharedString::from(prefix))
                                    .when(linked, |row| {
                                        row.child(link(name.clone(), peer.product_id, peer.edition))
                                    })
                                    .when(!linked, |row| row.child(name.clone()))
                                    .into_any_element()
                            } else if dual {
                                // `SEAMLESS_AUTO_PAIRING_PAIRED_DESCRIPTION` 里的
                                // `{{deviceName}}` 替换成纯文本或链接。
                                let sentence =
                                    self.spec.text("SEAMLESS_AUTO_PAIRING_PAIRED_DESCRIPTION");
                                let (prefix, suffix) = sentence
                                    .split_once("{{deviceName}}")
                                    .map(|(prefix, suffix)| (prefix.to_owned(), suffix.to_owned()))
                                    .unwrap_or((sentence, String::new()));
                                h_flex()
                                    .child(SharedString::from(prefix))
                                    .when(linked, |row| {
                                        row.child(link(name.clone(), peer.product_id, peer.edition))
                                    })
                                    .when(!linked, |row| row.child(name.clone()))
                                    .child(SharedString::from(suffix))
                                    .into_any_element()
                            } else if linked {
                                link(name.clone(), peer.product_id, peer.edition).into_any_element()
                            } else {
                                div().child(name.clone()).into_any_element()
                            };
                            div().text_size(surface::css(14.)).child(text)
                        }))
                        .when(!both_devices_capped, |view| {
                            // `.pairedContent{font-size:12px;padding-top:10px}` +
                            // `.pollingRateInfo{color:#999;margin-top:10px}`。
                            view.child(
                                div()
                                    .mt(surface::css(10.))
                                    .text_size(surface::css(12.))
                                    .text_color(Colors::warning_text())
                                    .child(
                                        self.spec.text(
                                            "CONFIGURE_POLLING_RATE_DEVICE_DISCONNECTED_TEXT",
                                        ),
                                    ),
                            )
                        }),
                )
                .child(
                    command(
                        "dock-page-unpair",
                        self.spec.text("UNPAIR"),
                        false,
                        false,
                        cx,
                    )
                    .kind(CommandKind::PageUnpair(dual))
                    .mt(surface::css(if dual { 9. } else { 0. }))
                    .on_click(cx.listener(|this, _, window, cx| this.open(window, cx))),
                );
        }
        let panel = surface::panel_with_control(
            self.spec.text(title),
            surface::asset_button(
                "dock-pairing-tips",
                "synapse/help-default.svg",
                self.spec.text(tips),
                cx,
            ),
            cx,
        )
        .child(content)
        .when(self.multi_pairing && !self.dongle, |v| {
            v.child(surface::note(
                self.spec.text(
                    self.spec
                        .config_key("launchUtilityInfoKey")
                        .unwrap_or("ENABLE_LAUNCH_PAIRING_UTILITY_INFO"),
                ),
                cx,
            ))
        })
        // `DeviceInfo.showBothDevicesConnectedWarning && we(pairedInfo)`：只有
        // 鼠标与键盘都在底座上时才提示轮询率下降。
        .when(both_devices_capped, |v| {
            v.child(warning(
                self.spec,
                "MOUSE_DOCK_PRO_DUAL_DEVICE_POLLING_RATE_WARNING",
                &self.name,
                12.,
            ))
        })
        .when(self.original_dongle, |v| {
            v.child(warning(
                self.spec,
                "MOUSE_DOCK_ORIGINAL_DONGLE_WARNING",
                &self.name,
                12.,
            ))
        })
        .when_some(self.alert.clone(), |v, alert| {
            v.child(surface::note(alert, cx))
        });
        v_flex()
            .w_full()
            .child(surface::product_banner(
                self.spec.product_id,
                self.edition,
                self.layout,
                cx,
            ))
            .child(surface::page_columns().child(surface::page_column(panel)))
            .into_any_element()
    }
}
impl Render for DockPairing {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w_full()
            .child(self.page(cx))
            .children(self.modal.clone())
    }
}
fn command(
    id: impl Into<ElementId>,
    label: String,
    primary: bool,
    disabled: bool,
    cx: &App,
) -> SourceCommand {
    let id = id.into();
    let base = gpui_kit::base::Button::new(id.clone())
        .accessibility_label(label.clone())
        .disabled(disabled)
        .h(surface::css(28.))
        .min_w(surface::css(90.))
        .px(surface::css(5.))
        .rounded(surface::css(2.))
        .flex()
        .items_center()
        .justify_center()
        .text_size(surface::css(12.))
        .font_family("Roboto")
        .line_height(surface::css(14.))
        .bg(if primary {
            cx.theme().primary
        } else {
            Colors::secondary()
        })
        .text_color(if primary {
            Colors::primary_text()
        } else {
            Colors::secondary_text()
        })
        .focus_visible(|s| s.border_1().border_color(cx.theme().foreground))
        .child(label.to_uppercase());
    SourceCommand {
        id,
        base,
        primary,
        disabled,
        kind: CommandKind::Choice,
    }
}

#[derive(Clone, Copy)]
enum CommandKind {
    Choice,
    Scan,
    Unpair(bool),
    PageUnpair(bool),
}
#[derive(IntoElement)]
struct SourceCommand {
    id: ElementId,
    base: gpui_kit::base::Button,
    primary: bool,
    disabled: bool,
    kind: CommandKind,
}
impl SourceCommand {
    fn kind(mut self, kind: CommandKind) -> Self {
        self.kind = kind;
        self
    }
    fn on_click(mut self, handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.base = self.base.on_click(handler);
        self
    }
}
impl Styled for SourceCommand {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}
impl RenderOnce for SourceCommand {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        use gpui_kit::base::motion::{self, Easing, Transition};
        let hovered = window.use_keyed_state((self.id.clone(), "hover"), cx, |_, _| false);
        let active = !self.disabled && *hovered.read(cx);
        let opacity = match self.kind {
            CommandKind::Choice if self.primary => {
                if active {
                    0.7
                } else {
                    1.
                }
            }
            CommandKind::Choice => {
                if active {
                    1.
                } else {
                    0.7
                }
            }
            CommandKind::PageUnpair(_) => {
                if active {
                    1.
                } else {
                    0.8
                }
            }
            _ => 1.,
        };
        let opacity = if matches!(self.kind, CommandKind::Choice) {
            motion::transition(
                (self.id, "opacity"),
                opacity,
                Transition::new(std::time::Duration::from_millis(300)).easing(Easing::Ease),
                window,
                cx,
            )
        } else {
            opacity
        };
        self.base
            .opacity(if self.disabled { 0.5 } else { opacity })
            .when(matches!(self.kind, CommandKind::Choice), |b| {
                b.border_1()
                    .border_color(cx.theme().title_bar)
                    .rounded(surface::css(3.))
            })
            .when(matches!(self.kind, CommandKind::Unpair(_)), |b| {
                b.bg(if active {
                    Colors::unpair_hover()
                } else {
                    Colors::secondary()
                })
            })
            .when(
                matches!(self.kind, CommandKind::Unpair(false)) && !self.disabled,
                |b| b.active(|s| s.bg(Colors::unpair_pressed())),
            )
            .when(matches!(self.kind, CommandKind::PageUnpair(false)), |b| {
                b.min_w(surface::css(0.))
                    .bg(Colors::card())
                    .border_1()
                    .border_color(Colors::border())
                    .text_color(cx.theme().foreground)
                    .rounded(surface::css(3.))
            })
            .when(matches!(self.kind, CommandKind::PageUnpair(true)), |b| {
                b.px(surface::css(16.))
                    .py(surface::css(6.))
                    .line_height(surface::css(14.))
            })
            .on_hover(window.listener_for(&hovered, |hovered, value, _, cx| {
                *hovered = *value;
                cx.notify();
            }))
    }
}
/// `.HyperPollingWirelessMouseDock_dongleWarning` 与 `_bothDevicesPollingCapped`
/// 共用同一条形态：20px 信息图标 + `gap:10px` + `margin-top:20px`，正文 12px `#999`。
fn warning(spec: &Spec, key: &str, name: &str, size: f32) -> Div {
    h_flex()
        .items_start()
        .gap(surface::css(10.))
        .mt(surface::css(20.))
        .child(warning_icon(spec))
        .child(
            div()
                .text_size(surface::css(size))
                .text_color(Colors::warning_text())
                .child(spec.text(key).replace("{{deviceName}}", name)),
        )
}
/// `.Duallink_bothDevicesConnectedWarning`：配对工具弹层里的同形提示，但它用的是
/// `margin:50px auto 0;max-width:520px`、20px 图标与 14px/17px `#ccc` 正文。
fn dialog_warning(spec: &Spec, key: &str, name: &str) -> Div {
    h_flex()
        .items_start()
        .gap(surface::css(10.))
        .mt(surface::css(50.))
        .mx_auto()
        .max_w(surface::css(520.))
        .child(warning_icon(spec))
        .child(
            div()
                .text_size(surface::css(14.))
                .line_height(surface::css(17.))
                .text_color(Colors::dialog_warning_text())
                .child(spec.text(key).replace("{{deviceName}}", name)),
        )
}
fn warning_icon(spec: &Spec) -> AnyElement {
    img(spec.asset("icon_info_solid"))
        .size(surface::css(20.))
        .flex_shrink_0()
        .into_any_element()
}

#[derive(IntoElement)]
struct PairingSpinner {
    id: ElementId,
    asset: SharedString,
}
fn spinner(spec: &Spec, id: impl Into<ElementId>) -> PairingSpinner {
    PairingSpinner {
        id: id.into(),
        asset: spec.asset("icon-progress_spinner"),
    }
}
impl RenderOnce for PairingSpinner {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        // HyperPollingWireless / Duallink ::before: 24px SVG in a 26px
        // container, rotation 0..360deg, 1s linear infinite, no state mutation.
        let icon = svg()
            .path(self.asset)
            .size(surface::css(24.))
            .text_color(cx.theme().primary);
        div().size(surface::css(26.)).child(if cx.reduce_motion() {
            icon.into_any_element()
        } else {
            icon.with_animation(
                self.id,
                Animation::new(std::time::Duration::from_secs(1))
                    .repeat()
                    .with_easing(linear),
                |icon, phase| icon.with_transformation(Transformation::rotate(percentage(phase))),
            )
            .into_any_element()
        })
    }
}
