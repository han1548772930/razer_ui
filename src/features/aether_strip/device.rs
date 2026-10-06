//! Current Sg/Gl/ol device carousel and its remove/take-control surfaces.
use super::source_tip::{SourceTipItem, SourceTipWrap, TipAnchor};
use super::*;
use std::collections::BTreeSet;
use std::time::Duration;

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
    pub(super) fn sync_identify_ready(&mut self, cx: &mut Context<Self>) {
        // `ol` in the current 784 bundle clears the previous timeout first.
        // Power-off (and unknown power) disables Identify immediately; a
        // powered-on card becomes actionable only after the 500 ms timeout.
        self.identify_task = None;
        self.identify_ready = false;
        if self.observation.power_on != Some(true) {
            return;
        }
        self.identify_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(500))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.observation.power_on == Some(true) {
                    this.identify_ready = true;
                }
                this.identify_task = None;
                cx.notify();
            });
        }));
    }

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
            self.sync_identify_ready(cx);
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
            // `.carousel--item{align-items:center;display:flex;flex:none;
            //  flex-direction:column;height:212px;justify-content:center;padding:0 30px;
            //  position:relative;width:248px}`。
            let mut card = v_flex()
                .relative()
                .w(surface::css(248.))
                .h(surface::css(212.))
                .flex_shrink_0()
                .items_center()
                .justify_center()
                .px(surface::css(30.));
            // 卡片内部的 `.device`：`.carousel--item .device{opacity:.5}`、
            // `.device.active{opacity:1;padding-top:28px}`、
            // `.device:not(.active){margin-top:75px}`。
            let mut device = v_flex()
                .items_center()
                .when(!selected, |v| v.opacity(0.5).mt(surface::css(75.)))
                .when(selected, |v| v.pt(surface::css(28.)));
            let id = peer.id.clone();
            device = device.child(
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
                device = device.child(
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
                device = device.child(
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
            // 四个徽标在源码里是无条件渲染的，可见性由状态与悬停决定（`.device--badge{visibility:hidden}`）：
            // `.device.active .device--cta-power{bottom:54%;left:6%;visibility:visible}`、
            // `.device.active .device--cta-find{bottom:37%;left:6%;visibility:visible}`、
            // `.device.active .device--cta-del{right:10%;top:22%}` 且仅
            // `.device.active:hover .device--cta-del{visibility:visible}`
            // （`.device.active.offline`/`.device.active.busy` 时也可见）；
            // `.device.active.offline .device--cta-find,.device--cta-power{visibility:hidden}`、
            // `.device.active.busy .device--cta-find,.device--cta-power{visibility:visible}`；
            // 忙碌徽标 `.device.busy .device--cta-enable{visibility:visible;width:42px;height:27px;
            //  left:50%;top:calc(50% - 10px);transform:translate(-50%,-50%)}`，选中时改用
            //  `.device.active.busy .device--cta-enable{background-image:busy-btn-white;border-color:#fff;
            //  top:calc(50% - 8px)}` + `:hover{background-color:#707070}`；
            // 离线徽标 `.device.offline .device--badge-offline{visibility:visible}`
            //  （`.device.busy` 时隐藏；选中 `right:10%;top:34%`，否则 `right:25%;top:40%`）。
            // 这些徽标都是绝对定位，隐藏时不占位，所以本地按可见性直接决定是否渲染，效果与
            // `visibility` 一致，也避免隐藏元素仍可命中。
            let busy = locked;
            let power_find = selected && (busy || !offline);
            let remove = selected && (offline || busy);
            let card_group = SharedString::from(peer.id.clone());
            card = card.group(card_group.clone());
            if power_find {
                card = card
                    .child(
                        SourceTipItem::new(
                            "aether-power",
                            if power_off {
                                "power-off-btn"
                            } else {
                                "power-on-btn"
                            },
                            text("POWER_ON"),
                            observation.power_on.is_none(),
                            true,
                            TipAnchor::Half,
                        )
                        // `.device.active .device--cta-power{left:6%;bottom:54%}`（卡片 248×212）。
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
                        SourceTipItem::new(
                            "aether-find",
                            "indentify-btn",
                            text("IDENTIFY_TEXT"),
                            observation.power_on != Some(true) || !self.identify_ready,
                            true,
                            TipAnchor::Half,
                        )
                        // `.device.active .device--cta-find{left:6%;bottom:37%}`。
                        .absolute()
                        .left(surface::css(15.))
                        .bottom(surface::css(78.))
                        .on_click(cx.listener(|this, _, _, cx| {
                            if this.identify_ready {
                                this.request("ON_IDENTIFY_IOT", json!({}), cx)
                            }
                        })),
                    );
            }
            // `.device.active .device--cta-del{right:10%;top:22%}`，只有悬停（或离线/忙碌）时才显示。
            card = card.child(
                div()
                    .absolute()
                    .right(surface::css(25.))
                    .top(surface::css(47.))
                    .when(!remove, |v| {
                        v.invisible()
                            .group_hover(card_group.clone(), |v| v.visible())
                    })
                    .child(
                        SourceTipItem::new(
                            "aether-remove",
                            "close-btn",
                            text("REMOVE_DEVICE_TITLE"),
                            false,
                            true,
                            TipAnchor::Right,
                        )
                        .when(remove, |v| {
                            v.on_click(cx.listener(|this, _, window, cx| {
                                this.open_device_dialog(DialogKind::Remove, window, cx)
                            }))
                        }),
                    ),
            );
            if busy {
                // `.device.busy .device--cta-enable`（42×27、居中）与
                // `.device.active.busy` 的白色变体：`top:calc(50% - 10px)`/`calc(50% - 8px)`
                // 再 `translate(-50%,-50%)`，卡片高 212、按钮高 27。
                let active = selected;
                device = device.child(
                    SourceTipItem::new(
                        "aether-take-control",
                        if active { "busy-btn-white" } else { "busy-btn" },
                        text("LIGHTING_DEVICE_TAKE_CONTROL_DESC"),
                        false,
                        true,
                        TipAnchor::Center,
                    )
                    .absolute()
                    .top(surface::css(if active { 84.5 } else { 82.5 }))
                    .w(surface::css(42.))
                    .h(surface::css(27.))
                    .border_1()
                    .border_color(if active {
                        cx.theme().foreground
                    } else {
                        Colors::control_border().opacity(0.302)
                    })
                    .rounded(surface::css(3.))
                    .when(active, |v| v.with_hover_bg(Colors::control_border()))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.open_device_dialog(DialogKind::TakeControl, window, cx)
                    })),
                );
            }
            if offline && !busy {
                device = device.child(
                    img(asset("offline-badge"))
                        .absolute()
                        // `.device.active .device--badge-offline{right:10%;top:34%}`，非选中卡片是
                        // `.device.offline .device--badge-offline{right:25%;top:40%}`。
                        .right(surface::css(if selected { 25. } else { 62. }))
                        .top(surface::css(if selected { 72. } else { 84.8 }))
                        .size(surface::css(24.)),
                );
            }
            card.child(device)
        });
        // `.carousel--inner{align-items:flex-end;display:flex;margin:0 auto;overflow:hidden;
        //  padding:0 496px;scroll-behavior:smooth}`；只有一台设备时源码改用
        // `.carousel--inner.center{overflow:visible;padding:initial;width:fit-content}`。
        // 多台设备时选中项由源码 `H()` 居中：
        // `scrollLeft = carousel.scrollWidth / 5 * index`（`496 = 2×248`，`/5` 约一屏五张卡），
        // 挂载与切换都执行；`scroll-behavior:smooth` 没在源码里声明时长（UA 决定），
        // 所以本地按同一偏移量直接定位，不自造缓动或时长。
        let row = if self.peers.len() > 1 {
            let index = self
                .peers
                .iter()
                .position(|peer| peer.id == self.selected)
                .unwrap_or(0);
            let target = self.carousel_scroll.bounds().size.width / 5. * index as f32;
            if (self.carousel_scroll.offset().x - target).abs() > px(0.5) {
                self.carousel_scroll.set_offset(point(target, px(0.)));
            }
            h_flex()
                .id("aether-device-carousel")
                .w_full()
                .items_end()
                .h(surface::css(235.))
                .px(surface::css(496.))
                .overflow_scroll()
                // 源码容器是 `overflow:hidden`，没有滚动条。
                .scrollbar_width(px(0.))
                .track_scroll(&self.carousel_scroll)
                .children(cards)
                .into_any_element()
        } else {
            h_flex()
                .id("aether-device-carousel")
                .w_full()
                .justify_center()
                .items_end()
                .h(surface::css(235.))
                .children(cards)
                .into_any_element()
        };
        let mut carousel = v_flex()
            .relative()
            .w_full()
            .h(surface::css(300.))
            .mt(surface::css(-3.))
            .mb(surface::css(-14.))
            .child(surface::dot_background(cx))
            .child(row);
        if self.peers.len() > 1 {
            // `.indicator--container{align-items:center;display:flex;justify-content:center;
            //  margin-top:10px;position:relative;z-index:1}`，编号项装在
            // `.indicator--list{background:#111;border:1px solid #ccc;border-radius:40px;
            //  display:flex;margin-right:10px;padding:5px}` 里（`.indicator--item` 26×26、
            //  `margin-right:10px`、末项 0——本地用 `gap` 表达同一间距）。
            let items = self.peers.iter().enumerate().map(|(ix, peer)| {
                let id = peer.id.clone();
                let selected = self.selected == peer.id;
                let warning =
                    peer.observation.locked == Some(true) || peer.observation.online == Some(false);
                let accent = if warning {
                    Colors::warning()
                } else {
                    cx.theme().primary
                };
                let name = peer.name.clone();
                // `.indicator--item[tooltip]:before{right:auto;top:calc(100% + 10px);
                //  width:fit-content}`：编号项的提示框左边缘贴左边缘、下方 10px。
                let item = gpui_kit::base::Button::new((
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
                .on_click(cx.listener(move |this, _, window, cx| this.select_peer(&id, window, cx)))
                .into_any_element();
                SourceTipWrap::new(
                    (
                        ElementId::from("aether-device-indicator-tip"),
                        SharedString::from(peer.id.clone()),
                    ),
                    name,
                    TipAnchor::Start,
                    10.,
                    item,
                )
            });
            carousel = carousel.child(
                h_flex()
                    .relative()
                    .w_full()
                    .justify_center()
                    .mt(surface::css(10.))
                    .child(
                        h_flex()
                            .gap(surface::css(10.))
                            .p(surface::css(5.))
                            .bg(Colors::background())
                            .border_1()
                            .border_color(cx.theme().foreground)
                            .rounded(surface::css(40.))
                            .mr(surface::css(10.))
                            .children(items),
                    ),
            );
        }
        carousel.into_any_element()
    }
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
