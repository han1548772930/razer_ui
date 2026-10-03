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
    dongle: bool,
    original_dongle: bool,
    preview: bool,
    alert: Option<String>,
    modal: Option<Entity<DockDialog>>,
}
struct PreviewDialogRequested;
impl EventEmitter<PreviewDialogRequested> for DockPairing {}
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
            dongle: false,
            original_dongle: false,
            preview: false,
            alert: None,
            modal: None,
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
                            div()
                                .text_size(surface::css(14.))
                                .child(if dual && paired.len() > 1 {
                                    format!("{}: {}", self.spec.text(peer.lane.key()), peer.name)
                                } else if dual {
                                    self.spec
                                        .text("SEAMLESS_AUTO_PAIRING_PAIRED_DESCRIPTION")
                                        .replace("{{deviceName}}", &peer.name)
                                } else {
                                    peer.name.clone()
                                })
                        }))
                        .when(!dual, |view| {
                            view.child(
                                div()
                                    .mt(surface::css(10.))
                                    .text_size(surface::css(12.))
                                    .text_color(cx.theme().muted_foreground)
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
                self.spec.text("ENABLE_LAUNCH_PAIRING_UTILITY_INFO"),
                cx,
            ))
        })
        .when(dual && paired.len() == 2, |v| {
            v.child(warning(
                self.spec,
                "MOUSE_DOCK_PRO_DUAL_DEVICE_POLLING_RATE_WARNING",
                &self.name,
                12.,
                cx,
            ))
        })
        .when(self.original_dongle, |v| {
            v.child(warning(
                self.spec,
                "MOUSE_DOCK_ORIGINAL_DONGLE_WARNING",
                &self.name,
                12.,
                cx,
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
fn warning(spec: &Spec, key: &str, name: &str, size: f32, cx: &App) -> Div {
    h_flex()
        .items_start()
        .gap(surface::css(10.))
        .mt(surface::css(20.))
        .child(
            img(spec.asset("icon_info_solid"))
                .size(surface::css(20.))
                .flex_shrink_0(),
        )
        .child(
            div()
                .text_size(surface::css(size))
                .text_color(cx.theme().muted_foreground)
                .child(spec.text(key).replace("{{deviceName}}", name)),
        )
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
