//! Current756/1874 useCn/pn/Nn/Tn: product master + opposite-category peer.
use super::*;

impl DockDialog {
    pub(in crate::features::dock_pairing) fn set_owner(
        &mut self,
        device: &Device,
        cx: &mut Context<Self>,
    ) {
        self.owner = Some(Peer {
            id: format!(
                "{}:{}:{}",
                device.product_id, device.device_container_id, device.serial_number
            ),
            name: device
                .product_name
                .get(&i18n::locale().to_lowercase())
                .to_owned(),
            product_id: device.product_id,
            dongle_id: self.spec.config["dongleId"]
                .as_u64()
                .and_then(|id| id.try_into().ok()),
            edition: Some(device.edition_id),
            // useCn keeps an absent connectedDeviceInfo keyboard layout;
            // useYn then selects layout one. The model's startup zero does
            // not establish a keyboard layout read.
            layout: (device.layout_id != 0 || self.spec.config["category"] != "KEYBOARD")
                .then_some(device.layout_id),
            lane: if self.spec.config["category"] == "KEYBOARD" {
                Lane::Keyboard
            } else {
                Lane::Mouse
            },
            payload: Some(multi::device_info(self.spec, device)),
        });
        cx.notify();
    }
    pub(super) fn legacy_remote_is_master(&self) -> bool {
        self.state
            .peers()
            .first()
            .and_then(|peer| peer.payload.as_ref())
            .is_some_and(|payload| payload["role"] == "master")
    }
    pub(super) fn display_peer(&self, lane: Lane) -> Option<&Peer> {
        if self.spec.legacy() && self.legacy_remote_is_master() {
            self.owner.as_ref()
        } else {
            self.state.channel(lane).peer.as_ref()
        }
    }
    pub(super) fn legacy_body(&self, cx: &Context<Self>) -> AnyElement {
        if self
            .state
            .channels
            .iter()
            .all(|channel| channel.status == Status::Loading)
        {
            return v_flex()
                .h_full()
                .flex_1()
                .min_h_0()
                .items_center()
                .justify_center()
                .child("loading")
                .into_any_element();
        }
        let peers = self.state.peers();
        let master = if self.legacy_remote_is_master() {
            peers.first().copied()
        } else {
            self.owner.as_ref()
        };
        let Some(master) = master else {
            return div().into_any_element();
        };
        let slave_lane = peers.first().map(|peer| peer.lane).unwrap_or(Lane::Mouse);
        let channel = self.state.channel(slave_lane);
        let connected = channel.peer.is_some();
        let connecting = matches!(channel.status, Status::Scanning | Status::Pairing)
            || channel.status == Status::Scanned && channel.candidates.len() > 1;
        let master_keyboard = master.lane == Lane::Keyboard;
        let master_card = v_flex()
            .w(surface::css(250.))
            .h(surface::css(210.))
            .px(surface::css(10.))
            .rounded(surface::css(5.))
            .bg(Colors::card())
            .shadow(vec![BoxShadow {
                color: cx.theme().primary,
                offset: point(Pixels::ZERO, Pixels::ZERO),
                blur_radius: Pixels::ZERO,
                spread_radius: cx.theme().font_size * (2. / 16.),
                inset: false,
            }])
            .child(
                div()
                    .pt(surface::css(10.))
                    .h(surface::css(28.))
                    .line_height(surface::css(18.))
                    .text_center()
                    .text_color(Colors::category())
                    .child(self.spec.text(master.lane.key())),
            )
            .child(
                div()
                    .h(surface::css(120.))
                    .w_full()
                    .when_some(peer_image(master), |view, path| {
                        view.child(img(path).size_full().object_fit(ObjectFit::Contain))
                    }),
            )
            .child(
                div()
                    .h(surface::css(60.))
                    .line_height(surface::css(16.))
                    .text_center()
                    .child(master.name.to_uppercase()),
            );
        let master_card = master_card.into_any_element();
        let slave_card = self.channel(slave_lane, cx);
        let (keyboard_card, mouse_card) = if master_keyboard {
            (master_card, slave_card)
        } else {
            (slave_card, master_card)
        };
        let line = |master_line: bool, right_to_left: bool| {
            let asset = if master_line {
                self.spec.asset("icon_marching_ants_master_line")
            } else {
                self.spec.asset(&format!(
                    "icon_marching_ants_{}_{}",
                    if connected { "connected" } else { "connecting" },
                    if right_to_left {
                        "right_to_left"
                    } else {
                        "left_to_right"
                    }
                ))
            };
            div()
                .relative()
                .flex_shrink_0()
                .w(surface::css(if master_line { 80. } else { 440. }))
                .h_full()
                .left(surface::css(if master_line {
                    if master_keyboard { 88. } else { -88. }
                } else if master_keyboard {
                    -78.
                } else {
                    0.
                }))
                .when(master_line || connected || connecting, |view| {
                    view.child(img(asset).size_full())
                })
        };
        let dongle = v_flex()
            .items_center()
            .w(relative(0.5))
            .relative()
            .left(surface::css(if master_keyboard { 10. } else { -10. }))
            .child(
                img("synapse/dock-756-dongle.png")
                    .size(surface::css(80.))
                    .mb(surface::css(10.))
                    .object_fit(ObjectFit::Contain),
            )
            .child(
                self.spec
                    .text("MULTI_DEVICE_HYPER_SPEED_DONGLE")
                    .to_uppercase(),
            )
            .child(format!("({})", master.name.to_uppercase()));
        let mut body = v_flex()
            .w_full()
            .items_center()
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .w(surface::css(790.))
                    .max_w_full()
                    .mt(surface::css(20.))
                    .mb(surface::css(10.))
                    .text_center()
                    .child(self.spec.text("HYPER_SPEED_MULTI_DEVICE_HEADER")),
            )
            .child(
                v_flex()
                    .w(surface::css(600.))
                    .items_center()
                    .child(
                        h_flex()
                            .w_full()
                            .items_center()
                            .px(surface::css(10.))
                            .when(!master_keyboard, |row| row.child(div().w(relative(0.5))))
                            .child(dongle)
                            .when(master_keyboard, |row| row.child(div().w(relative(0.5)))),
                    )
                    .child(
                        h_flex()
                            .h(surface::css(30.))
                            .mt(surface::css(10.))
                            .mb(surface::css(13.))
                            .child(line(master_keyboard, true))
                            .child(line(!master_keyboard, false)),
                    )
                    .child(
                        h_flex()
                            .gap(surface::css(20.))
                            .justify_center()
                            .child(keyboard_card)
                            .child(mouse_card),
                    ),
            );
        let key = match channel.status {
            Status::Unpaired => Some("UNPAIRING_COMPLETE"),
            Status::JustPaired => Some("PAIRING_COMPLETE"),
            _ => None,
        };
        if let Some(key) = key {
            body = body.child(
                div()
                    .mt(surface::css(20.))
                    .w(surface::css(520.))
                    .h(surface::css(20.))
                    .text_center()
                    .text_color(cx.theme().primary)
                    .child(self.spec.text(key)),
            );
        } else if matches!(
            channel.status,
            Status::Ready
                | Status::Scanning
                | Status::Scanned
                | Status::PairFailed
                | Status::UnpairFailed
        ) {
            body = body.child(
                h_flex()
                    .w(surface::css(520.))
                    .min_h(surface::css(85.))
                    .mt(surface::css(20.))
                    .px(surface::css(12.))
                    .child(
                        img(self.spec.asset(if master_keyboard {
                            "icon_mouse_ensure"
                        } else {
                            "icon_kb_ensure"
                        }))
                        .size(surface::css(60.))
                        .flex_shrink_0(),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .ml(surface::css(25.))
                            .text_color(cx.theme().muted_foreground)
                            .children(
                                [
                                    "HYPER_SPEED_MULTI_DEVICE_DONGLE_DUALLINK_NOTE_ONE",
                                    "DUALLINK_NOTE_TWO",
                                    "DUALLINK_NOTE_THREE",
                                    "DUALLINK_NOTE_FOUR",
                                    "DUALLINK_NOTE_FIVE",
                                ]
                                .map(|key| {
                                    h_flex()
                                        .items_start()
                                        .gap(surface::css(8.))
                                        .child("•")
                                        .child(self.spec.text(key))
                                }),
                            ),
                    ),
            );
        }
        body.into_any_element()
    }
}
