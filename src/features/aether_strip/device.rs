//! Current Sg/Gl/ol device carousel and its remove/take-control surfaces.
use super::*;
use crate::ui::scroll::SourceScrollable as _;
use std::collections::BTreeSet;

#[derive(Clone)]
pub(super) struct Peer {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) observation: Observation,
}
impl Peer {
    pub(super) fn new(id: String, name: String) -> Self {
        Self {
            id,
            name,
            observation: Observation::default(),
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum DialogKind {
    Remove,
    TakeControl,
}

impl AetherStrip {
    fn select_peer(&mut self, id: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected == id {
            return;
        }
        let Some(peer) = self.peers.iter().find(|peer| peer.id == id).cloned() else {
            return;
        };
        self.request(
            "ON_CHANGE_UI_ON_SHOW",
            json!({"activeContainerId":peer.id}),
            cx,
        );
        if self.preview {
            self.dismiss(window, cx);
            self.selected = peer.id;
            self.observation = peer.observation;
            self.bends = state::distribute(self.observation.detected.unwrap_or(0), 1);
            self.sync_inputs(window, cx);
            cx.notify();
        }
    }
    fn start_name(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.observation.online != Some(true) {
            return;
        }
        let Some(peer) = self.peers.iter().find(|peer| peer.id == self.selected) else {
            return;
        };
        let name = peer.name.clone();
        self.rename_input.update(cx, |input, cx| {
            input.set_value(name, window, cx);
            input.focus(window, cx);
        });
        self.renaming = true;
        cx.notify();
    }
    pub(super) fn commit_name(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        if !self.renaming {
            return;
        }
        self.renaming = false;
        let name = self.rename_input.read(cx).value().trim().to_string();
        if !name.is_empty() {
            self.request("ON_NAME_IOT_DEVICE", json!({"name":name}), cx);
        }
        cx.notify();
    }
    pub(super) fn open_device_dialog(
        &mut self,
        kind: DialogKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dismiss(window, cx);
        let return_focus = window.focused(cx);
        let owner = cx.entity().downgrade();
        let peers = self
            .peers
            .iter()
            .filter(|peer| peer.observation.locked == Some(true))
            .cloned()
            .collect();
        let modal = cx.new(|cx| DeviceDialog {
            kind,
            peers,
            checked: BTreeSet::new(),
            owner,
            preview: self.preview,
            open: true,
            focus: cx.focus_handle(),
            return_focus,
            alert: None,
        });
        modal.read(cx).focus.clone().focus(window, cx);
        self.modal = Some(modal);
        cx.notify();
    }
    pub(super) fn device_carousel(&self, cx: &Context<Self>) -> AnyElement {
        let cards = self.peers.iter().map(|peer| {
            let selected = peer.id == self.selected;
            let observation = if selected {
                &self.observation
            } else {
                &peer.observation
            };
            let locked = observation.locked == Some(true);
            let offline = observation.online == Some(false);
            let power_off = observation.power_on == Some(false);
            let mut card = v_flex()
                .relative()
                .w(surface::css(248.))
                .h(surface::css(212.))
                .flex_shrink_0()
                .items_center()
                .justify_end()
                .px(surface::css(5.))
                .when(!selected, |v| v.opacity(0.5));
            let id = peer.id.clone();
            card = card.child(
                gpui_kit::base::Button::new((
                    ElementId::from("aether-device-image"),
                    SharedString::from(peer.id.clone()),
                ))
                .accessibility_label(peer.name.clone())
                .selected(selected)
                .p_0()
                .w(surface::css(if selected { 237. } else { 154.8 }))
                .h(surface::css(if selected { 132.72 } else { 86.688 }))
                .when(offline || locked || power_off, |v| v.opacity(0.5))
                .when(self.edition == 0, |v| {
                    v.child(
                        img("synapse/aether-prd-3x.png")
                            .size_full()
                            .object_fit(ObjectFit::Contain),
                    )
                })
                .focus_visible(|v| v.border_1().border_color(cx.theme().primary))
                .on_click(
                    cx.listener(move |this, _, window, cx| this.select_peer(&id, window, cx)),
                ),
            );
            if selected && self.renaming {
                card = card.child(
                    Input::new(&self.rename_input)
                        .w(surface::css(200.))
                        .h(surface::css(26.))
                        .small(),
                );
            } else {
                let name = if self.peers.len() == 1 && locked {
                    text("LIGHTING_DEVICE_TAKE_CONTROL_DESC_1")
                } else {
                    peer.name.clone()
                };
                card = card.child(
                    gpui_kit::base::Button::new((
                        ElementId::from("aether-device-name"),
                        SharedString::from(peer.id.clone()),
                    ))
                    .accessibility_label(name.clone())
                    .disabled(!selected || observation.online != Some(true))
                    .h(surface::css(26.))
                    .max_w(surface::css(240.))
                    .text_size(surface::css(if selected { 14. } else { 12. }))
                    .when(power_off, |v| v.opacity(0.5))
                    .child(div().truncate().child(name))
                    .on_click(cx.listener(|this, _, window, cx| this.start_name(window, cx))),
                );
            }
            if selected {
                if !offline {
                    card = card
                        .child(
                            card_action(
                                "aether-power",
                                if power_off {
                                    "power-off-btn"
                                } else {
                                    "power-on-btn"
                                },
                                text("POWER_ON"),
                                observation.power_on.is_none(),
                                cx,
                            )
                            .absolute()
                            .left(surface::css(15.))
                            .bottom(surface::css(114.))
                            .on_click(cx.listener(|this, _, _, cx| {
                                if let Some(power) = this.observation.power_on {
                                    this.request(
                                        "ON_SET_POWER_STATE_IOT",
                                        json!({"isPowerOn":!power}),
                                        cx,
                                    );
                                }
                            })),
                        )
                        .child(
                            card_action(
                                "aether-find",
                                "indentify-btn",
                                text("IDENTIFY_TEXT"),
                                observation.power_on != Some(true),
                                cx,
                            )
                            .absolute()
                            .left(surface::css(15.))
                            .bottom(surface::css(78.))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.request("ON_IDENTIFY_IOT", json!({}), cx)
                            })),
                        );
                }
                card = card.child(
                    card_action(
                        "aether-remove",
                        "close-btn",
                        text("REMOVE_DEVICE_TITLE"),
                        false,
                        cx,
                    )
                    .absolute()
                    .right(surface::css(25.))
                    .top(surface::css(47.))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_device_dialog(DialogKind::Remove, window, cx)
                    })),
                );
                if locked {
                    card = card.child(
                        card_action(
                            "aether-take-control",
                            "busy-btn-white",
                            text("LIGHTING_DEVICE_TAKE_CONTROL_DESC"),
                            false,
                            cx,
                        )
                        .absolute()
                        .top(surface::css(90.))
                        .w(surface::css(42.))
                        .h(surface::css(27.))
                        .border_1()
                        .border_color(cx.theme().foreground)
                        .rounded(surface::css(3.))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.open_device_dialog(DialogKind::TakeControl, window, cx)
                        })),
                    );
                } else if offline {
                    card = card.child(
                        img(asset("offline-badge"))
                            .absolute()
                            .right(surface::css(25.))
                            .top(surface::css(72.))
                            .size(surface::css(24.)),
                    );
                }
            }
            card
        });
        let mut carousel = v_flex()
            .relative()
            .w_full()
            .h(surface::css(300.))
            .mt(surface::css(-3.))
            .mb(surface::css(-14.))
            .child(surface::dot_background(cx))
            .child(
                h_flex()
                    .id("aether-device-carousel")
                    .w_full()
                    .justify_center()
                    .items_end()
                    .h(surface::css(235.))
                    .scrollable_both()
                    .children(cards),
            );
        if self.peers.len() > 1 {
            carousel =
                carousel.child(
                    h_flex()
                        .w_full()
                        .justify_center()
                        .gap(surface::css(10.))
                        .mt(surface::css(10.))
                        .children(self.peers.iter().enumerate().map(|(ix, peer)| {
                            let id = peer.id.clone();
                            let selected = self.selected == peer.id;
                            let warning = peer.observation.locked == Some(true)
                                || peer.observation.online == Some(false);
                            let accent = if warning {
                                Colors::warning()
                            } else {
                                cx.theme().primary
                            };
                            let name = peer.name.clone();
                            gpui_kit::base::Button::new((
                                ElementId::from("aether-device-indicator"),
                                SharedString::from(id.clone()),
                            ))
                            .accessibility_label(name.clone())
                            .selected(selected)
                            .size(surface::css(26.))
                            .rounded(surface::css(30.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(surface::css(14.))
                            .text_color(if selected {
                                Colors::background()
                            } else if warning {
                                Colors::warning()
                            } else {
                                cx.theme().foreground
                            })
                            .when(selected, |v| v.bg(accent))
                            .hover(move |v| v.bg(accent).text_color(Colors::background()))
                            .focus_visible(|v| v.border_1().border_color(cx.theme().foreground))
                            .child((ix + 1).to_string())
                            .tooltip(move |window, cx| {
                                tooltip::Tooltip::new(name.clone()).build(window, cx)
                            })
                            .on_click(cx.listener(
                                move |this, _, window, cx| this.select_peer(&id, window, cx),
                            ))
                        })),
                );
        }
        carousel.into_any_element()
    }
}
fn card_action(
    id: &'static str,
    name: &str,
    label: String,
    disabled: bool,
    cx: &App,
) -> gpui_kit::base::Button {
    gpui_kit::base::Button::new(id)
        .group(id)
        .accessibility_label(label.clone())
        .disabled(disabled)
        .size(surface::css(24.))
        .p_0()
        .rounded_full()
        .when(disabled, |v| v.opacity(0.3))
        .relative()
        .child(img(asset(name)).size_full())
        .when(name == "close-btn", |v| {
            v.child(
                img(asset("close-hovered-btn"))
                    .absolute()
                    .inset_0()
                    .size_full()
                    .opacity(0.)
                    .group_hover(id, |v| v.opacity(1.)),
            )
        })
        .focus_visible(|v| v.border_1().border_color(cx.theme().primary))
        .when(
            !disabled && ["power-on-btn", "power-off-btn", "indentify-btn"].contains(&name),
            |v| v.hover(|s| s.border_1().border_color(cx.theme().primary)),
        )
        .when(!disabled && name == "busy-btn-white", |v| {
            v.hover(|s| s.bg(Colors::control_border()))
        })
        .tooltip(move |window, cx| {
            tooltip::Tooltip::new(label.clone())
                .max_w(surface::css(250.))
                .build(window, cx)
        })
}

pub(super) struct DeviceDialog {
    kind: DialogKind,
    peers: Vec<Peer>,
    checked: BTreeSet<String>,
    owner: WeakEntity<AetherStrip>,
    preview: bool,
    open: bool,
    focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    alert: Option<String>,
}
impl DeviceDialog {
    pub(super) fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        self.open = false;
        if let Some(focus) = self.return_focus.take() {
            focus.focus(window, cx);
        }
        cx.notify();
    }
    fn confirm(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (action, payload) = match self.kind {
            DialogKind::Remove => ("ON_REMOVE_IOT", json!({})),
            DialogKind::TakeControl => ("ON_UNLOCK_IOT", json!({"unlockDevices":self.checked})),
        };
        let _ = self
            .owner
            .update(cx, |owner, cx| owner.request(action, payload, cx));
        if self.preview {
            self.close(window, cx);
        } else {
            self.alert = Some(i18n::t_or(
                "AETHER_SERVICE_UNAVAILABLE",
                "暂时无法连接灯带，请稍后重试。",
            ));
            cx.notify();
        }
    }
}
impl Render for DeviceDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().into_any_element();
        }
        let remove = self.kind == DialogKind::Remove;
        let mut body = v_flex()
            .id("aether-device-dialog-content")
            .occlude()
            .min_w(surface::css(300.))
            .max_w(surface::css(500.))
            .p(surface::css(20.))
            .bg(Colors::dialog())
            .border_1()
            .border_color(if remove {
                Colors::remove()
            } else {
                Colors::control_border()
            })
            .rounded(surface::css(3.))
            .text_size(surface::css(14.))
            .child(
                div()
                    .mb(surface::css(if remove { 10. } else { 25. }))
                    .when(remove, |v| {
                        v.text_center()
                            .text_color(Colors::remove())
                            .text_size(surface::css(16.))
                    })
                    .child(if remove {
                        text("REMOVE_IOT_DEVICE").replace("{{number}}", "1")
                    } else {
                        text("LIGHTING_DEVICE_TAKE_CONTROL_DESC_1")
                    }),
            )
            .child(
                div()
                    .mb(surface::css(if remove { 16. } else { 10. }))
                    .when(remove, |v| v.text_center())
                    .child(text(if remove {
                        "REMOVE_THIS_IOT_DEVICE_DESC"
                    } else {
                        "LIGHTING_DEVICE_TAKE_CONTROL_DESC_2"
                    })),
            );
        if !remove {
            for peer in &self.peers {
                let id = peer.id.clone();
                body = body.child(
                    checkbox::Checkbox::new((
                        ElementId::from("aether-unlock-choice"),
                        SharedString::from(id.clone()),
                    ))
                    .label(peer.name.clone())
                    .checked(self.checked.contains(&id))
                    .mb(surface::css(10.))
                    .on_click(cx.listener(move |this, checked, _, cx| {
                        if *checked {
                            this.checked.insert(id.clone());
                        } else {
                            this.checked.remove(&id);
                        }
                        cx.notify();
                    })),
                );
            }
        }
        body = body
            .when_some(self.alert.clone(), |v, alert| {
                v.child(surface::note(alert, cx).mb(surface::css(16.)))
            })
            .child(
                h_flex().justify_center().child(
                    gpui_kit::base::Button::new("aether-device-dialog-confirm")
                        .accessibility_label(text(if remove { "REMOVE" } else { "TAKE_CONTROL" }))
                        .bg(Colors::remove())
                        .text_color(Colors::background())
                        .border_1()
                        .border_color(Colors::backdrop())
                        .rounded(surface::css(3.))
                        .h(surface::css(27.))
                        .min_w(surface::css(90.))
                        .px(surface::css(12.))
                        .text_size(surface::css(12.))
                        .child(text(if remove { "REMOVE" } else { "TAKE_CONTROL" }).to_uppercase())
                        .hover(|v| v.opacity(0.8))
                        .focus_visible(|v| v.border_color(cx.theme().foreground))
                        .on_click(cx.listener(|this, _, window, cx| this.confirm(window, cx))),
                ),
            );
        let owner = cx.entity().downgrade();
        gpui_kit::base::Dialog::new(cx)
            .focus_handle(self.focus.clone())
            .on_ok(move |_, window, cx| {
                let _ = owner.update(cx, |this, cx| this.confirm(window, cx));
                false
            })
            .on_close(cx.listener(|this, _, window, cx| this.close(window, cx)))
            // Current sl.defaultProps.backdrop=false. Keep the native dismissal
            // hit region transparent instead of inventing a darkened surface.
            .backdrop(div().absolute().inset_0())
            .popup(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_start()
                    .justify_center()
                    .pt(surface::css(113.))
                    .child(body),
            )
            .into_any_element()
    }
}
