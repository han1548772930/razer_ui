use super::*;
use crate::ui::scroll::SourceScrollable as _;

pub(super) struct DockDialog {
    pub(super) spec: &'static Spec,
    pub(super) state: PairingState,
    edition: u32,
    layout: u32,
    pub(super) name: String,
    focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    open: bool,
    pub(super) preview: bool,
    pub(super) inline: bool,
    pub(super) alert: Option<String>,
    pub(super) last_request: Option<(String, serde_json::Value)>,
}
impl DockDialog {
    pub(super) fn open(
        spec: &'static Spec,
        edition: u32,
        layout: u32,
        name: String,
        state: PairingState,
        preview: bool,
        window: &mut Window,
        cx: &mut App,
    ) -> Entity<Self> {
        let return_focus = window.focused(cx);
        let modal = cx.new(|cx| Self {
            spec,
            state,
            edition,
            layout,
            name,
            focus: cx.focus_handle(),
            return_focus,
            open: true,
            preview,
            inline: false,
            alert: (!preview).then(|| "暂时无法读取配对信息。".into()),
            last_request: None,
        });
        modal.read(cx).focus.clone().focus(window, cx);
        modal
    }
    pub(super) fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.open {
            return;
        }
        self.open = false;
        self.last_request = None;
        if let Some(focus) = self.return_focus.take() {
            focus.focus(window, cx);
        }
        cx.notify();
    }
    fn escape(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(channel) = self
            .state
            .channels
            .iter_mut()
            .find(|c| c.status == Status::ConfirmUnpair)
        {
            channel.status = Status::Paired;
            cx.notify();
            return;
        }
        self.close(window, cx);
    }
    fn request(&mut self, kind: &str, payload: serde_json::Value, cx: &mut Context<Self>) -> bool {
        if !self.preview {
            self.alert = Some("配对服务暂不可用，请稍后重试。".into());
            cx.notify();
            return false;
        }
        self.alert = None;
        self.last_request = Some((kind.into(), payload));
        cx.notify();
        true
    }
    fn scan(&mut self, lane: Lane, cx: &mut Context<Self>) {
        if !self.state.modifiable(lane, self.spec.dual()) {
            return;
        }
        if self.request(
            "DUALLINK_SCAN_DEVICE",
            PairingState::scan_payload(lane, self.spec.dual()),
            cx,
        ) {
            let channel = self.state.channel_mut(lane);
            channel.status = Status::Scanning;
            channel.candidates.clear();
            channel.selected = None;
        }
    }
    fn pair(&mut self, lane: Lane, cx: &mut Context<Self>) {
        if !self.state.modifiable(lane, self.spec.dual()) {
            return;
        }
        let Some(peer) = self.state.selected(lane).cloned() else {
            return;
        };
        // Dongle 713 needs the source's additional continuation dialog. Keep
        // this gate explicit until that feature is implemented; never bypass it.
        if self.spec.dual() && peer.dongle_id == Some(713) {
            self.alert = Some("此设备需要额外的配对确认，暂未接入。".into());
            cx.notify();
            return;
        }
        let payload = serde_json::json!({"mode":1,"device":{"productId":peer.product_id,"dongleId":peer.dongle_id,"category":peer.lane.key(),"editionId":peer.edition,"layoutId":peer.layout,"productName":{"en":peer.name}}});
        if self.request("DUALLINK_BIND_DEVICE", payload, cx) {
            self.state.channel_mut(lane).status = Status::Pairing;
        }
    }
    fn unpair(&mut self, lane: Lane, cx: &mut Context<Self>) {
        if !self.state.modifiable(lane, self.spec.dual())
            || self.state.channel(lane).status != Status::ConfirmUnpair
        {
            return;
        }
        let Some(payload) = self.state.unpair_payload(lane, self.spec.dual()) else {
            return;
        };
        if self.request("DUALLINK_UNBIND_DEVICE", payload, cx) {
            self.state.channel_mut(lane).status = Status::Unpairing;
        }
    }
    fn channel(&self, lane: Lane, cx: &Context<Self>) -> AnyElement {
        let channel = self.state.channel(lane);
        let status = channel.status;
        let dual = self.spec.dual();
        let enabled = self.state.modifiable(lane, dual);
        let mut card = v_flex()
            .w(surface::css(if dual { 250. } else { 510. }))
            .h(surface::css(210.))
            .flex_shrink_0()
            .px(surface::css(if dual { 10. } else { 0. }))
            .rounded(surface::css(5.))
            .bg(Colors::card())
            .border_2()
            .border_color(if channel.peer.is_some() {
                cx.theme().primary
            } else {
                Colors::card()
            })
            .child(
                div()
                    .w_full()
                    .pt(surface::css(10.))
                    .h(surface::css(28.))
                    .text_center()
                    .text_color(Colors::category())
                    .child(self.spec.text(lane.key())),
            );
        if status == Status::ConfirmUnpair {
            let text = if dual {
                self.spec
                    .text("MOUSE_DOCK_UNPAIR_CONFIRM_TEXT")
                    .replace("{{deviceName}}", &self.name)
            } else {
                self.spec.text("HYPERPOLLING_WIRELESS_UNPAIR_CONFIRM_TEXT")
            };
            return card
                .child(
                    v_flex()
                        .w(surface::css(210.))
                        .mx_auto()
                        .mt(surface::css(10.))
                        .py(surface::css(18.))
                        .px(surface::css(10.))
                        .border_1()
                        .border_color(Colors::warning())
                        .rounded(surface::css(3.))
                        .gap(surface::css(9.))
                        .text_center()
                        .child(text)
                        .child(
                            h_flex()
                                .justify_center()
                                .gap(surface::css(10.))
                                .child(
                                    command(
                                        (ElementId::from("dock-cancel-unpair"), lane.key()),
                                        self.spec.text("CANCEL"),
                                        false,
                                        false,
                                        cx,
                                    )
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            this.state.channel_mut(lane).status = Status::Paired;
                                            cx.notify();
                                        },
                                    )),
                                )
                                .child(
                                    command(
                                        (ElementId::from("dock-confirm-unpair"), lane.key()),
                                        self.spec.text("CONFIRM"),
                                        true,
                                        !enabled,
                                        cx,
                                    )
                                    .on_click(
                                        cx.listener(move |this, _, _, cx| this.unpair(lane, cx)),
                                    ),
                                ),
                        ),
                )
                .into_any_element();
        }
        if status == Status::Scanned && channel.candidates.len() > 1 {
            card = card.child(
                v_flex()
                    .w_full()
                    .h(surface::css(144.))
                    .child(
                        div()
                            .my(surface::css(10.))
                            .text_center()
                            .child(self.spec.text("DUALLINK_SCAN_DEVICE_FOUND")),
                    )
                    .child(
                        div()
                            .id((ElementId::from("dock-candidates"), lane.key()))
                            .h(surface::css(104.))
                            .scrollable_y()
                            .children(channel.candidates.iter().map(|peer| {
                                let selected = channel.selected.as_ref() == Some(&peer.id);
                                let id = peer.id.clone();
                                gpui_kit::base::Radio::new(SharedString::from(format!(
                                    "dock-candidate-{}-{}",
                                    lane.key(),
                                    id
                                )))
                                .checked(selected)
                                .disabled(!enabled)
                                .accessibility_label(peer.name.clone())
                                .flex()
                                .items_center()
                                .gap(surface::css(8.))
                                .min_h(surface::css(20.))
                                .mb(surface::css(10.))
                                .focus_visible(|s| s.border_1().border_color(cx.theme().primary))
                                .child(
                                    div()
                                        .size(surface::css(14.))
                                        .border_1()
                                        .border_color(cx.theme().foreground)
                                        .rounded_full()
                                        .when(selected, |s| s.bg(cx.theme().primary)),
                                )
                                .child(peer.name.to_uppercase())
                                .on_change({
                                    let entity = cx.weak_entity();
                                    move |_, _, _, cx| {
                                        let _ = entity.update(cx, |this, cx| {
                                            this.state.channel_mut(lane).selected =
                                                Some(id.clone());
                                            cx.notify();
                                        });
                                    }
                                })
                            })),
                    ),
            );
        } else {
            let mut image = v_flex()
                .relative()
                .h(surface::css(120.))
                .w_full()
                .items_center()
                .justify_center()
                .gap(surface::css(10.));
            if let Some(peer) = channel
                .peer
                .as_ref()
                .filter(|_| status != Status::Unpairing)
            {
                if let Some(path) =
                    crate::resources::dashboard_image(peer.product_id, peer.edition, peer.layout)
                {
                    image = image.child(
                        img(path)
                            .absolute()
                            .inset_0()
                            .size_full()
                            .object_fit(ObjectFit::Contain),
                    );
                }
                if matches!(status, Status::Paired | Status::JustPaired) {
                    image = image.child(
                        command(
                            (ElementId::from("dock-unpair"), lane.key()),
                            self.spec.text("UNPAIR"),
                            false,
                            !enabled,
                            cx,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if this.state.modifiable(lane, this.spec.dual()) {
                                this.state.channel_mut(lane).status = Status::ConfirmUnpair;
                                cx.notify();
                            }
                        })),
                    );
                }
            } else {
                image = image.child(
                    img(self.spec.asset(if lane == Lane::Mouse {
                        "icon_category_mouse"
                    } else {
                        "icon_category_keyboard"
                    }))
                    .size(surface::css(44.)),
                );
                image = if matches!(
                    status,
                    Status::Scanning | Status::Pairing | Status::Unpairing
                ) {
                    image.child(
                        img(self.spec.asset("icon-progress_spinner")).size(surface::css(26.)),
                    )
                } else {
                    image.child(
                        command(
                            (ElementId::from("dock-scan"), lane.key()),
                            self.spec.text(if status == Status::Scanned {
                                "RESCAN"
                            } else {
                                "ADD"
                            }),
                            true,
                            !enabled,
                            cx,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| this.scan(lane, cx))),
                    )
                };
            }
            card = card.child(image);
        }
        let tag = match status {
            Status::Scanning => Some(("SCANNING", false)),
            Status::Pairing => Some(("PAIRING", false)),
            Status::Unpairing => Some(("UNPAIRING", false)),
            Status::PairFailed => Some(("PAIRING_FAILED", true)),
            Status::Scanned if channel.candidates.is_empty() => Some(("NO_DEVICE_FOUND", true)),
            _ => None,
        };
        if let Some((key, error)) = tag {
            card = card.child(
                div()
                    .text_center()
                    .text_color(if error {
                        Colors::warning()
                    } else {
                        cx.theme().primary
                    })
                    .child(self.spec.text(key)),
            );
        } else if status == Status::Scanned && channel.candidates.len() > 1 {
            card = card.child(
                h_flex()
                    .justify_center()
                    .gap(surface::css(10.))
                    .child(
                        command(
                            (ElementId::from("dock-cancel-selection"), lane.key()),
                            self.spec.text("CANCEL"),
                            false,
                            false,
                            cx,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.request("DUALLINK_BIND_INFO", serde_json::json!({}), cx);
                        })),
                    )
                    .child(
                        command(
                            (ElementId::from("dock-pair-selection"), lane.key()),
                            self.spec.text("PAIR"),
                            true,
                            !enabled || self.state.selected(lane).is_none(),
                            cx,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| this.pair(lane, cx))),
                    ),
            );
        } else if let Some(peer) = channel.peer.as_ref() {
            card = card.child(div().text_center().child(peer.name.to_uppercase()).when(
                status == Status::UnpairFailed,
                |v| {
                    v.child(
                        div()
                            .text_color(Colors::warning())
                            .child(self.spec.text("UNPAIRING_FAILED")),
                    )
                },
            ));
        } else {
            card = card.child(
                gpui_kit::base::Link::new((ElementId::from("dock-compatible"), lane.key()))
                    .href(if dual {
                        "https://www.razer.com/technology/razer-hyperspeed-wireless"
                    } else {
                        "https://www.razer.com/technology/razer-hyperpolling"
                    })
                    .open_with(|url, _, _, cx| cx.open_url(url))
                    .text_center()
                    .underline()
                    .cursor_pointer()
                    .accessibility_label(self.spec.text("VIEW_COMPATIBLE_DEVICES"))
                    .hover(|s| s.text_color(cx.theme().primary))
                    .focus_visible(|s| s.border_1().border_color(cx.theme().primary))
                    .child(self.spec.text("VIEW_COMPATIBLE_DEVICES")),
            );
        }
        card.into_any_element()
    }
    pub(super) fn body(&self, cx: &Context<Self>) -> AnyElement {
        let dual = self.spec.dual();
        if self
            .state
            .channels
            .iter()
            .all(|c| c.status == Status::Loading)
        {
            return v_flex()
                .min_h(surface::css(220.))
                .items_center()
                .justify_center()
                .when_some(self.alert.clone(), |v, a| v.child(surface::note(a, cx)))
                .when(self.alert.is_none(), |v| {
                    v.child(if dual { "loading" } else { "" }).child(
                        img(self.spec.asset("icon-progress_spinner")).size(surface::css(26.)),
                    )
                })
                .into_any_element();
        }
        let mut body = v_flex()
            .w_full()
            .min_h(surface::css(if dual { 0. } else { 685. }))
            .items_center()
            .text_size(surface::css(14.))
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .w(surface::css(790.))
                    .max_w_full()
                    .px(surface::css(if dual { 0. } else { 20. }))
                    .mt(surface::css(20.))
                    .mb(surface::css(10.))
                    .text_center()
                    .child(self.spec.text(if dual {
                        "MOUSE_DOCK_DUALINK_PROPERTIES_TOOLTIP"
                    } else {
                        "HYPERPOLLING_WIRELESS_HEADER"
                    })),
            );
        if dual {
            body = body.child(
                v_flex()
                    .items_center()
                    .w(surface::css(600.))
                    .child(
                        div()
                            .size(surface::css(80.))
                            .mb(surface::css(10.))
                            .when_some(
                                crate::resources::dashboard_image(
                                    self.spec.product_id,
                                    self.edition,
                                    self.layout,
                                ),
                                |v, path| {
                                    v.child(img(path).size_full().object_fit(ObjectFit::Contain))
                                },
                            ),
                    )
                    .child(self.name.to_uppercase())
                    .child(
                        h_flex()
                            .w(surface::css(520.))
                            .h(surface::css(30.))
                            .mt(surface::css(10.))
                            .mb(surface::css(13.))
                            .children([Lane::Keyboard, Lane::Mouse].map(|lane| {
                                let channel = self.state.channel(lane);
                                let connected = channel.peer.is_some();
                                let connecting =
                                    matches!(channel.status, Status::Scanning | Status::Pairing)
                                        || (channel.status == Status::Scanned
                                            && channel.candidates.len() > 1);
                                div().flex_1().h_full().when(connected || connecting, |v| {
                                    v.child(
                                        img(self.spec.asset(&format!(
                                            "icon_marching_ants_{}_{}",
                                            if connected { "connected" } else { "connecting" },
                                            if lane == Lane::Keyboard {
                                                "right_to_left"
                                            } else {
                                                "left_to_right"
                                            }
                                        )))
                                        .size_full(),
                                    )
                                })
                            })),
                    ),
            );
        }
        body = body.child(
            h_flex()
                .justify_center()
                .gap(surface::css(20.))
                .mt(surface::css(if dual { 0. } else { 10. }))
                .when(dual, |v| v.child(self.channel(Lane::Keyboard, cx)))
                .child(self.channel(Lane::Mouse, cx)),
        );
        if dual && self.state.peers().len() == 2 {
            body = body.child(div().max_w(surface::css(520.)).mt(surface::css(30.)).child(
                warning(
                    self.spec,
                    "MOUSE_DOCK_BOTH_DEVICES_PAIRING_UTILITY_WARNING",
                    &self.name,
                    14.,
                    cx,
                ),
            ));
        }
        let statuses = if dual {
            vec![
                self.state.channel(Lane::Keyboard).status,
                self.state.channel(Lane::Mouse).status,
            ]
        } else {
            vec![self.state.channel(Lane::Mouse).status]
        };
        let success = if statuses.contains(&Status::Unpaired) {
            Some("UNPAIRING_COMPLETE")
        } else if dual && statuses.contains(&Status::JustPaired) {
            Some("PAIRING_COMPLETE")
        } else {
            None
        };
        if let Some(key) = success {
            body = body.child(
                div()
                    .mt(surface::css(20.))
                    .text_color(cx.theme().primary)
                    .child(self.spec.text(key)),
            );
        } else if statuses.iter().any(|s| {
            matches!(
                s,
                Status::Ready | Status::Scanning | Status::PairFailed | Status::UnpairFailed
            ) || (dual && *s == Status::Scanned)
        }) {
            let notes = [
                "HYPER_SPEED_MULTI_DEVICE_DONGLE_DUALLINK_NOTE_ONE",
                "DUALLINK_NOTE_TWO",
                "DUALLINK_NOTE_THREE",
                if dual {
                    "DUALLINK_NOTE_FOUR_MOUSE_DOCK"
                } else {
                    "HYPERPOLLING_WIRELESS_NOTE_FOUR"
                },
                "DUALLINK_NOTE_FIVE",
            ];
            body = body.child(
                h_flex()
                    .w(surface::css(if dual { 680. } else { 714. }))
                    .min_h(surface::css(85.))
                    .mt(surface::css(20.))
                    .px(surface::css(12.))
                    .child(
                        img(self.spec.asset(if dual {
                            "icon_mousemat_ensure"
                        } else {
                            "icon_mouse_ensure"
                        }))
                        .size(surface::css(60.))
                        .flex_shrink_0(),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .ml(surface::css(if dual { 25. } else { 38. }))
                            .text_color(cx.theme().muted_foreground)
                            .children(notes.map(|key| {
                                h_flex()
                                    .items_start()
                                    .gap(surface::css(8.))
                                    .child("•")
                                    .child(self.spec.text(key))
                            })),
                    ),
            );
        } else if !dual
            && self.state.channel(Lane::Mouse).candidates.len() > 1
            && statuses.contains(&Status::Scanned)
        {
            body = body.child(
                div()
                    .mt(surface::css(20.))
                    .text_center()
                    .text_color(cx.theme().primary)
                    .child(self.spec.text("RADIO_SELECT_TIPS")),
            );
        }
        body.when_some(self.alert.clone(), |v, a| {
            v.child(surface::note(a, cx).mt(surface::css(20.)))
        })
        .into_any_element()
    }
}
impl Render for DockDialog {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.open {
            return div().into_any_element();
        }
        if self.inline {
            return self.body(cx);
        }
        let height = (window.viewport_size().height - window.rem_size() * (100. / 16.)).max(px(0.));
        let panel = v_flex()
            .id("dock-pairing-modal")
            .occlude()
            .w(surface::css(850.))
            .max_w_full()
            .h(height)
            .bg(Colors::panel())
            .rounded_t(surface::css(5.))
            .child(
                h_flex()
                    .relative()
                    .justify_center()
                    .h(surface::css(36.))
                    .flex_shrink_0()
                    .border_b_1()
                    .border_color(Colors::border())
                    .font_family(if self.spec.dual() {
                        "RazerF5"
                    } else {
                        "Roboto"
                    })
                    .text_size(surface::css(if self.spec.dual() { 16. } else { 14. }))
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        self.spec
                            .text(if self.spec.dual() {
                                "HYPER_SPEED_MULTI_DEVICE_PAIRING_UTILITY"
                            } else {
                                "PAIRING_UTILITY"
                            })
                            .to_uppercase(),
                    )
                    .child(
                        surface::keymap_close_button("dock-modal-close", "关闭", window, cx)
                            .absolute()
                            .right_0()
                            .top_0()
                            .on_click(cx.listener(|this, _, window, cx| this.close(window, cx))),
                    ),
            )
            .child(
                div()
                    .id("dock-modal-scroll")
                    .flex_1()
                    .min_h_0()
                    .scrollable_both()
                    .child(self.body(cx)),
            );
        gpui_kit::base::Dialog::new(cx)
            .focus_handle(self.focus.clone())
            .close_on_backdrop_press(false)
            .on_ok(|_, _, _| false)
            .on_close(cx.listener(|this, _, window, cx| this.escape(window, cx)))
            .backdrop(div().absolute().inset_0().bg(Colors::backdrop()))
            .popup(
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_end()
                    .justify_center()
                    .child(panel),
            )
            .into_any_element()
    }
}
