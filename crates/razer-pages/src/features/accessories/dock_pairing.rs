//! Product 164/241 pairing pages. Pairing observations belong to the session,
//! independently of the SourceControls lighting profile owned by the workspace.
use gpui_kit::component::*;
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_i18n as i18n;
use razer_model::model::Device;
use razer_widgets::surface;
use razer_widgets::theme::DockPairingColors as Colors;
use serde::Deserialize;
use std::{collections::BTreeMap, sync::OnceLock};

#[path = "dock_pairing/context.rs"]
mod context;
#[path = "dock_pairing/dialog.rs"]
mod dialog;
#[path = "dock_pairing/multi.rs"]
mod multi;
#[path = "dock_pairing/widget.rs"]
mod widget;
pub use widget::DualLinkWidget;
#[path = "dock_pairing/observation.rs"]
mod observation;
#[path = "dock_pairing/preview.rs"]
mod preview;
#[path = "dock_pairing/state.rs"]
mod state;
use dialog::DockDialog;
pub use observation::{DockPairingEvent, DockPairingObservation};
pub use preview::open_preview;
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
        .get_or_init(|| read_dock_pairing_catalog())
        .iter()
        .find(|s| s.product_id == pid)
        .expect("supported dock product")
}
impl Spec {
    fn dual(&self) -> bool {
        self.config["canPairTwoDevices"].as_bool() == Some(true)
    }
    fn legacy(&self) -> bool {
        self.flag("legacyDualLink")
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
pub fn supports_page(pid: u32, key: &str) -> bool {
    matches!(pid, 164 | 241) && spec(pid).page == key
}

pub struct DockPairing {
    spec: &'static Spec,
    edition: u32,
    layout: u32,
    name: String,
    state: PairingState,
    active: bool,
    multi_pairing: bool,
    device_info: serde_json::Value,
    pairing_cache: serde_json::Value,
    /// 应用当前已登记的设备（`product_id`, `edition_id`），对应源 `Es` 的比对列表。
    known_devices: Vec<(u32, u32)>,
    device_context: context::DeviceContext,
    dongle: bool,
    original_dongle: bool,
    preview: bool,
    alert: Option<String>,
    modal: Option<Entity<DockDialog>>,
    modal_subscriptions: Vec<Subscription>,
}
struct PreviewDialogRequested;
impl EventEmitter<PreviewDialogRequested> for DockPairing {}

/// 241 配对文案里的设备名被点击。源 `Es(peer, devices)` 允许 peer 的 edition 为
/// 缺省/0；此事件已解析成命中设备的真实 edition。`z(e)` 切到该设备；164 的名称
/// 另走打开配对工具的 `g`，不发此事件。
pub struct DeviceLinkRequested {
    pub product_id: u32,
    pub edition_id: u32,
}
impl EventEmitter<DeviceLinkRequested> for DockPairing {}
/// Current 164/6776 useR -> 1398/WU opens the source multipairing root.
#[derive(Clone)]
pub struct DockPairingUtilityRequested {
    device_info: serde_json::Value,
}
impl DockPairingUtilityRequested {
    pub fn device_info(&self) -> &serde_json::Value {
        &self.device_info
    }
}
impl EventEmitter<DockPairingUtilityRequested> for DockPairing {}
impl EventEmitter<DockPairingEvent> for DockPairing {}
impl DockPairing {
    pub fn new(device: &Device) -> Self {
        Self {
            spec: spec(device.product_id),
            edition: device.edition_id,
            layout: device.layout_id,
            name: device
                .product_name
                .get(&i18n::locale().to_lowercase())
                .to_owned(),
            state: PairingState::default(),
            active: false,
            multi_pairing: false,
            device_info: multi::device_info(spec(device.product_id), device),
            pairing_cache: serde_json::Value::Null,
            known_devices: Vec::new(),
            device_context: context::DeviceContext::default(),
            dongle: false,
            original_dongle: false,
            preview: false,
            alert: None,
            modal: None,
            modal_subscriptions: Vec::new(),
        }
    }
    /// Only the verified, installed provision flag selects this source branch.
    pub fn set_multi_pairing_feature(&mut self, enabled: bool, cx: &mut Context<Self>) {
        let enabled = enabled && self.spec.flag("isMultiPairingSupported");
        if self.multi_pairing == enabled {
            return;
        }
        self.multi_pairing = enabled;
        if enabled && self.state.active.is_some() {
            cx.emit(self.state.cancel());
        }
        if let Some(modal) = self.modal.take() {
            let handle = modal.read(cx).window_handle();
            self.modal_subscriptions.clear();
            cx.spawn(async move |owner, cx| {
                let _ = handle.update(cx, |_, window, cx| {
                    let event = modal.update(cx, |modal, cx| modal.finish_close(window, cx));
                    if let Some(event) = event {
                        let _ = owner.update(cx, |_, cx| cx.emit(event));
                    }
                });
            })
            .detach();
        }
        self.begin_read(cx);
        cx.notify();
    }
    /// Cache records are source presentation, independently of hardware peers.
    pub fn set_pairing_cache(&mut self, records: serde_json::Value, cx: &mut Context<Self>) {
        if self.pairing_cache != records {
            self.pairing_cache = records;
            cx.notify();
        }
    }
    /// 刷新应用设备列表（数量或成员变化才通知）。
    pub fn set_known_devices(&mut self, devices: Vec<(u32, u32)>, cx: &mut Context<Self>) {
        if self.known_devices != devices {
            self.known_devices = devices;
            self.begin_read(cx);
            cx.notify();
        }
    }

    /// Current Ps uses real valid/runtime devices and the local profile store
    /// for distinct predicates. No cached profile is a hardware observation.
    pub fn set_device_context(&mut self, devices: &[Device], cx: &mut Context<Self>) {
        let mut owner_changed = false;
        if let Some(device) = devices.iter().find(|device| {
            device.product_id == self.spec.product_id
                && self.device_info["deviceContainerId"].as_str()
                    == Some(device.device_container_id.as_str())
                && self.device_info["serialNumber"].as_str() == Some(device.serial_number.as_str())
        }) {
            let info = multi::device_info(self.spec, device);
            let dongle = multi::is_dongle(device);
            owner_changed = self.device_info != info || self.dongle != dongle;
            self.device_info = info;
            self.dongle = dongle;
        }
        let context = context::DeviceContext::from_devices(devices);
        if owner_changed || self.device_context != context {
            self.known_devices = context.valid_devices();
            self.device_context = context;
            self.begin_read(cx);
            cx.notify();
        }
    }

    /// Current Ps reads on mount and when validDevices changes. Only the
    /// active page or its pairing dialog owns the current request generation.
    pub fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        if self.active == active {
            return;
        }
        self.active = active;
        if active {
            self.begin_read(cx);
        } else if self.state.active.is_some() {
            let event = self.state.cancel();
            cx.emit(event);
        }
        cx.notify();
    }

    fn begin_read(&mut self, cx: &mut Context<Self>) {
        if !self.active || self.preview || self.modal.is_some() || self.multi_pairing {
            return;
        }
        let event = self
            .state
            .request("DUALLINK_BIND_INFO", serde_json::json!({}));
        cx.emit(event);
    }

    pub fn dismiss(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(modal) = self.modal.take() {
            let cancel = modal.update(cx, |modal, cx| modal.finish_close(window, cx));
            self.modal_subscriptions.clear();
            if let Some(event) = cancel {
                cx.emit(event);
            }
        }
        self.modal_subscriptions.clear();
        self.alert = None;
        cx.notify();
    }
    pub fn observe_pairing(
        &mut self,
        observation: DockPairingObservation,
        cx: &mut Context<Self>,
    ) -> bool {
        if let Some(modal) = &self.modal {
            let observed = modal.update(cx, |view, cx| {
                view.observe_pairing(observation.clone(), cx)
                    .then(|| view.state.clone())
            });
            if let Some(mut state) = observed {
                // Only the dialog owns local intents; the parent displays peers.
                state.pending = None;
                state.active = None;
                state.recovery = None;
                state.success_close = None;
                self.state = state;
                cx.notify();
                return true;
            }
        }
        let (changed, event, error) = self.state.observe(observation, self.spec.dual());
        if changed {
            self.alert = error;
            if let Some(event) = event {
                cx.emit(event);
            }
            cx.notify();
        }
        changed
    }
    fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.multi_pairing {
            if self.dongle {
                cx.emit(DockPairingUtilityRequested {
                    device_info: self.device_info.clone(),
                });
            }
            return;
        }
        if self.preview {
            cx.emit(PreviewDialogRequested);
            return;
        }
        self.dismiss(window, cx);
        if self.state.active.is_some() {
            let event = self.state.cancel();
            cx.emit(event);
        }
        let modal = DockDialog::open(
            self.spec,
            self.edition,
            self.layout,
            self.name.clone(),
            self.state.clone(),
            self.preview,
            window,
            cx,
        );
        self.modal_subscriptions = vec![
            cx.subscribe(&modal, |_, _, event: &DockPairingEvent, cx| {
                cx.emit(event.clone());
            }),
            cx.subscribe(&modal, |this, _, _: &dialog::DockDialogClosed, cx| {
                this.modal = None;
                this.modal_subscriptions.clear();
                this.begin_read(cx);
                cx.notify();
            }),
        ];
        modal.update(cx, |view, cx| view.begin_read(cx));
        self.modal = Some(modal);
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
        let cached =
            multi::cached_peers(&self.pairing_cache, &self.device_info, &self.device_context);
        let mut paired = if self.multi_pairing {
            cached.iter().take(1).collect()
        } else {
            self.state.peers()
        };
        // 241/Ps renders both Z(mouse), Z(keyboard) and q(mouse), q(keyboard)
        // in that order. Channel storage uses keyboard first for the dialog.
        if dual {
            paired.sort_by_key(|peer| match peer.lane {
                Lane::Mouse => 0,
                Lane::Keyboard => 1,
            });
        }
        // `W = DeviceInfo.showBothDevicesConnectedWarning && we(pairedInfo)`：鼠标与
        // 键盘同时在底座上时，源把轮询率说明整行 `V = !W && …` 去掉，改为显示上面那条提示。
        let both_devices_capped =
            dual && self.spec.flag("showBothDevicesConnectedWarning") && paired.len() == 2;
        let any_connected = paired.iter().any(|peer| {
            if self.multi_pairing {
                self.known_devices
                    .iter()
                    .any(|(pid, _)| *pid == peer.product_id || peer.dongle_id == Some(*pid))
            } else {
                self.device_context.connected(peer, self.spec.product_id)
            }
        });
        let category_rows = dual && paired.len() > 1 && (both_devices_capped || any_connected);
        let show_polling = !dual
            || !both_devices_capped
                && paired.iter().any(|peer| {
                    self.device_context.has_polling_profile(peer)
                        && (!any_connected
                            || self.device_context.connected(peer, self.spec.product_id))
                });
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
        if paired.is_empty() {
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
            let description = v_flex()
                .when(dual, |view| view.flex_1())
                .when(!dual, |view| view.w(surface::css(369.)).flex_shrink_0())
                .min_w_0()
                .children(paired.iter().map(|peer| {
                    // 241/Es treats a missing/zero edition as a wildcard.
                    // Resolve the real target edition before emitting the
                    // shell event, whose device lookup is deliberately exact.
                    let target = self.known_devices.iter().copied().find(|(pid, edition)| {
                        peer.product_id != 0
                            && *pid == peer.product_id
                            && peer
                                .edition
                                .is_none_or(|value| value == 0 || *edition == value)
                    });
                    let linked = target.is_some();
                    let name = SharedString::from(if dual && !category_rows {
                        source_title_case(&peer.name)
                    } else {
                        peer.name.clone()
                    });
                    let link = |name: SharedString, product_id: u32, edition_id: u32| {
                        gpui_kit::base::Button::new(SharedString::from(format!(
                            "dock-device-link-{product_id}-{edition_id}"
                        )))
                        .accessibility_label(name.clone())
                        .cursor_pointer()
                        .underline()
                        // `.deviceNameLink{cursor:pointer;text-decoration:underline}` +
                        // `.hyperpolling-span-hover:hover{color:#44d62c}`。
                        .hover(|style| style.text_color(cx.theme().primary))
                        .focus_visible(|style| style.border_1().border_color(cx.theme().primary))
                        .on_click(cx.listener(move |_, _, _, cx| {
                            cx.emit(DeviceLinkRequested {
                                product_id,
                                edition_id,
                            });
                        }))
                        .child(name)
                    };
                    let text = if category_rows {
                        // `<lane>: <name>`，名字可能是链接。
                        let prefix = format!("{}: ", self.spec.text(peer.lane.key()));
                        h_flex()
                            .child(SharedString::from(prefix))
                            .when(linked, |row| {
                                let (product_id, edition_id) = target.expect("linked device");
                                row.child(link(name.clone(), product_id, edition_id))
                            })
                            .when(!linked, |row| row.child(name.clone()))
                            .into_any_element()
                    } else if dual {
                        // `SEAMLESS_AUTO_PAIRING_PAIRED_DESCRIPTION` 里的
                        // `{{deviceName}}` 替换成纯文本或链接。
                        let sentence = self.spec.text("SEAMLESS_AUTO_PAIRING_PAIRED_DESCRIPTION");
                        let (prefix, suffix) = sentence
                            .split_once("{{deviceName}}")
                            .map(|(prefix, suffix)| (prefix.to_owned(), suffix.to_owned()))
                            .unwrap_or((sentence, String::new()));
                        h_flex()
                            .child(SharedString::from(prefix))
                            .when(linked, |row| {
                                let (product_id, edition_id) = target.expect("linked device");
                                row.child(link(name.clone(), product_id, edition_id))
                            })
                            .when(!linked, |row| row.child(name.clone()))
                            .child(SharedString::from(suffix))
                            .into_any_element()
                    } else {
                        // 164/Re's pairedTitle uses onClick:g, which opens
                        // its pairing utility. It never calls the 241
                        // device-navigation callback, even for known peers.
                        let disabled = if self.multi_pairing {
                            !self.dongle
                        } else {
                            !any_connected
                        };
                        let name_button =
                            gpui_kit::base::Button::new("dock-paired-name-open-pairing")
                                .disabled(disabled)
                                .underline()
                                .text_color(if !disabled {
                                    cx.theme().foreground
                                } else {
                                    Colors::warning_text()
                                })
                                .hover(|style| style.text_color(cx.theme().primary))
                                .accessibility_label(name.clone())
                                .focus_visible(|style| {
                                    style.border_1().border_color(cx.theme().primary)
                                })
                                .child(name.clone())
                                .on_click(cx.listener(|this, _, window, cx| this.open(window, cx)));
                        h_flex()
                            .when(self.multi_pairing, |row| {
                                row.child(format!("{}\u{a0}", self.spec.text("PAIRED_WITH")))
                            })
                            .child(name_button)
                            .into_any_element()
                    };
                    div()
                        .text_size(surface::css(14.))
                        .when(dual, |row| row.line_height(surface::css(17.)))
                        .child(text)
                }))
                .when(show_polling, |view| {
                    // `.pairedContent{font-size:12px;padding-top:10px}` +
                    // `.pollingRateInfo{color:#999;margin-top:10px}`。
                    if !dual && any_connected {
                        let target = paired.first().and_then(|peer| {
                            self.known_devices
                                .iter()
                                .copied()
                                .find(|(pid, _)| *pid == peer.product_id)
                        });
                        view.child(
                            gpui_kit::base::Button::new("dock-polling-navigation")
                                .mt(surface::css(10.))
                                .text_size(surface::css(12.))
                                .text_color(Colors::dialog_warning_text())
                                .hover(|style| style.text_color(cx.theme().primary))
                                .disabled(target.is_none())
                                .child(self.spec.text("CONFIGURE_4000_POLLING_RATE"))
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    if let Some((product_id, edition_id)) = target {
                                        cx.emit(DeviceLinkRequested {
                                            product_id,
                                            edition_id,
                                        });
                                    }
                                })),
                        )
                    } else {
                        view.child(
                            div()
                                .mt(surface::css(10.))
                                .text_size(surface::css(12.))
                                .text_color(Colors::warning_text())
                                .child(self.spec.text(if dual && any_connected {
                                    "MOUSE_DOCK_PAIRING_POLLING_RATE_INFO"
                                } else {
                                    "CONFIGURE_POLLING_RATE_DEVICE_DISCONNECTED_TEXT"
                                })),
                        )
                    }
                });
            let unpair = command(
                "dock-page-unpair",
                self.spec.text("UNPAIR"),
                false,
                self.multi_pairing && !self.dongle,
                cx,
            )
            .kind(CommandKind::PageUnpair(dual))
            .mt(surface::css(if dual { 9. } else { 0. }))
            .on_click(cx.listener(|this, _, window, cx| this.open(window, cx)));
            // 241 nests description + action in pairedContentGroup. The icon
            // gap is 10px, while that group's action gap is 20px. Both dock
            // pairInfoBox rules reserve 20px below the complete row.
            content = if dual {
                content.items_center().mb(surface::css(20.)).child(
                    h_flex()
                        .flex_1()
                        .min_w_0()
                        .items_center()
                        .gap(surface::css(20.))
                        .child(description)
                        .child(unpair),
                )
            } else {
                content
                    .gap_0()
                    .justify_between()
                    .mb(surface::css(20.))
                    .child(description)
                    .child(unpair)
            };
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
        .when(
            self.multi_pairing && !self.dongle && paired.is_empty(),
            |v| {
                v.child(surface::note(
                    self.spec.text(
                        self.spec
                            .config_key("launchUtilityInfoKey")
                            .unwrap_or("ENABLE_LAUNCH_PAIRING_UTILITY_INFO"),
                    ),
                    cx,
                ))
            },
        )
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
        .when(
            self.original_dongle
                || dual
                    && self.spec.flag("showOriginalDongleConnectedWarning")
                    && self
                        .device_context
                        .original_dongle(&paired, self.spec.product_id),
            |v| {
                v.child(warning(
                    self.spec,
                    "MOUSE_DOCK_ORIGINAL_DONGLE_WARNING",
                    &self.name,
                    12.,
                ))
            },
        );
        v_flex()
            .w_full()
            // Current 164/241 roots unset BodyWrapper's min-width while
            // retaining main CSS's 10px 20px 20px content inset.
            .min_w_0()
            .pt(surface::css(10.))
            .px(surface::css(20.))
            .pb(surface::css(20.))
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

/// Current Ps/q lowers the label and applies ASCII /\b\w/g capitalization.
fn source_title_case(value: &str) -> String {
    let mut boundary = true;
    value
        .to_lowercase()
        .chars()
        .map(|character| {
            let word = character.is_ascii_alphanumeric() || character == '_';
            let result = if boundary && word {
                character.to_ascii_uppercase()
            } else {
                character
            };
            boundary = !word;
            result
        })
        .collect()
}
impl Render for DockPairing {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .w_full()
            .child(self.page(cx))
            .children(self.modal.clone())
    }
}
fn peer_image(peer: &Peer) -> Option<&'static str> {
    // Current useYn defaults an absent keyboard layout to one, mouse to
    // zero. An explicit zero remains zero and keeps its unavailable branch.
    let layout = peer
        .layout
        .unwrap_or(if peer.lane == Lane::Keyboard { 1 } else { 0 });
    let identity = (peer.product_id, peer.edition?, layout);
    let current = match identity {
        (164, 0, 0) => Some("synapse/dock-dashboard-164-0-0.png"),
        (241, 0, 0) => Some("synapse/dock-dashboard-241-0-0.png"),
        (756, 0, 1) => Some("synapse/dock-dashboard-756-0-1.png"),
        _ => None,
    };
    if current.is_some() {
        return current;
    }
    razer_assets::dashboard_image_exact(identity.0, identity.1, identity.2)
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
                b.h_auto()
                    .flex_shrink_0()
                    .self_start()
                    .px(surface::css(16.))
                    .pt(surface::css(7.))
                    .pb(surface::css(6.))
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

// Generated catalog reader: razer-pages/dock_pairing_data.
fn read_dock_pairing_catalog() -> Vec<Spec> {
    fn decode_0(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<std::vec::Vec<Spec>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Spec> {
        let value = match r.tag()? {
            0 => Spec {
                product_id: r.child(decode_2)?,
                page: r.child(decode_3)?,
                config: r.child(decode_4)?,
                translations: r.child(decode_5)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u32> {
        let value = match r.tag()? {
            0 => r.u32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<serde_json::Value> {
        let value = match r.tag()? {
            0 => r.external_value()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
    > {
        let value = match r.tag()? {
            0 => r.map(decode_3, decode_6)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, String>> {
        let value = match r.tag()? {
            0 => r.map(decode_3, decode_3)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_DOCK_PAIRING_DATA,
        0,
        "a802214ea98a300a3f74ab68807eca5b540b00461e76cf1f35e4e13ace6b4953",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod dock_pairing_data_catalog_tests {
    #[test]
    fn packaged_catalog_dock_pairing_data_decodes() {
        let _ = super::read_dock_pairing_catalog();
    }
}
// End generated catalog reader: razer-pages/dock_pairing_data.
