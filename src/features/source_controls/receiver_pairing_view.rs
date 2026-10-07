//! Current 179/9473 re -> ie/oe/ne/ae/_e, with local-only operation intents.
use super::pairing_state::{ReceiverCategory as Category, Status};
use super::*;
use crate::ui::theme::DockPairingColors as Colors;
use gpui_kit::base::{Button as BaseButton, Link, Radio as BaseRadio};

#[derive(Deserialize)]
struct PairingText {
    translations: BTreeMap<String, BTreeMap<String, String>>,
}
pub(super) fn t(key: &str) -> String {
    static TEXT: OnceLock<PairingText> = OnceLock::new();
    let text = TEXT.get_or_init(|| {
        serde_json::from_str(include_str!("receiver_pairing_data.json"))
            .expect("audited 179 pairing locales")
    });
    let locale = crate::i18n::locale();
    text.translations
        .iter()
        .find(|(lang, _)| lang.eq_ignore_ascii_case(&locale))
        .and_then(|(_, values)| values.get(key))
        .or_else(|| {
            text.translations
                .get("en")
                .and_then(|values| values.get(key))
        })
        .cloned()
        .unwrap_or_else(|| crate::i18n::t(key))
}

impl EventEmitter<ReceiverPairingEvent> for SourceControls {}

#[derive(Clone, Copy)]
enum Action {
    Refresh,
    Scan(Category),
    Bind,
    CancelSelection,
    ConfirmUnpair,
    CancelUnpair,
    Unbind,
    DiscardIntent,
}

impl SourceControls {
    pub(super) fn open_receiver_pairing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(event) = self.receiver.page.suspend() {
            cx.emit(event);
        }
        self.sync_receiver_page_retry(cx);
        self.receiver.failure_recovery = None;
        self.receiver.success_close = None;
        self.receiver.pairing_window = Some(window.window_handle());
        self.receiver.return_focus = window.focused(cx);
        let event = self.receiver.pairing.open();
        self.focus.focus(window, cx);
        cx.emit(event);
        cx.notify();
    }
    pub(super) fn close_receiver_pairing(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.receiver.failure_recovery = None;
        self.receiver.success_close = None;
        self.receiver.pairing_window = None;
        let event = self.receiver.pairing.close();
        cx.emit(event);
        if let Some(focus) = self.receiver.return_focus.take() {
            focus.focus(window, cx);
        }
        if let Some(event) = self.receiver.page.begin() {
            cx.emit(event);
        }
        cx.notify();
    }
    /// Real publisher entry point. Opening, selecting and confirming do not call
    /// this function themselves and do not write the SourceControls draft.
    pub(crate) fn observe_receiver_pairing(
        &mut self,
        observation: ReceiverPairingObservation,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.spec.product_id != 179 {
            return false;
        }
        if self.receiver.page.observe(&observation) {
            self.sync_receiver_page_retry(cx);
            cx.notify();
            return true;
        }
        let (changed, event) = self.receiver.pairing.observe(observation.clone());
        if let Some(event) = event {
            cx.emit(event);
        }
        if changed {
            self.receiver.page.observe_dialog_result(&observation);
            self.sync_receiver_failure_recovery(cx);
            self.sync_receiver_success_close(cx);
            cx.notify();
        }
        changed
    }
    fn sync_receiver_success_close(&mut self, cx: &mut Context<Self>) {
        let ticket = self.receiver.pairing.success_close();
        if self
            .receiver
            .success_close
            .as_ref()
            .map(|(ticket, _)| *ticket)
            == ticket
        {
            return;
        }
        self.receiver.success_close = None;
        if let (Some(ticket), Some(handle)) = (ticket, self.receiver.pairing_window) {
            let task = cx.spawn(async move |owner, cx| {
                cx.background_executor().timer(ticket.delay()).await;
                let _ = handle.update(cx, |_, window, cx| {
                    let _ = owner.update(cx, |this, cx| {
                        if this.receiver.pairing.can_close_success(ticket) {
                            this.close_receiver_pairing(window, cx);
                        }
                    });
                });
            });
            self.receiver.success_close = Some((ticket, task));
        }
    }
    fn sync_receiver_failure_recovery(&mut self, cx: &mut Context<Self>) {
        let recovery = self.receiver.pairing.recovery();
        if self
            .receiver
            .failure_recovery
            .as_ref()
            .map(|(ticket, _)| *ticket)
            == recovery
        {
            return;
        }
        self.receiver.failure_recovery = None;
        if let Some(ticket) = recovery {
            let task = cx.spawn(async move |this, cx| {
                cx.background_executor().timer(ticket.delay()).await;
                let _ = this.update(cx, |this, cx| {
                    if this.receiver.pairing.recover_failure(ticket) {
                        this.receiver.failure_recovery = None;
                        cx.notify();
                    }
                });
            });
            self.receiver.failure_recovery = Some((ticket, task));
        }
    }
    fn receiver_pairing_action(&mut self, action: Action, cx: &mut Context<Self>) {
        let state = &mut self.receiver.pairing;
        let event = match action {
            Action::Refresh => state.refresh(),
            Action::Scan(category) => state.scan(category),
            Action::Bind => state.bind(),
            Action::CancelSelection => state.cancel_selection(),
            Action::ConfirmUnpair => {
                state.confirm_unpair();
                None
            }
            Action::CancelUnpair => state.cancel_unpair(),
            Action::Unbind => state.unbind(),
            Action::DiscardIntent => state.cancel_pending(),
        };
        if let Some(event) = event {
            cx.emit(event);
        }
        self.sync_receiver_failure_recovery(cx);
        self.sync_receiver_success_close(cx);
        cx.notify();
    }

    pub(super) fn receiver_pairing_body(&self, cx: &Context<Self>) -> AnyElement {
        let state = &self.receiver.pairing;
        if state.status == Status::Loading {
            return div()
                .id("receiver-pairing-loading")
                .test_support()
                .role(Role::Group)
                .aria_label(t("LOADING"))
                .flex()
                .h_full()
                .items_center()
                .justify_center()
                .child(ReceiverSpinner)
                .into_any_element();
        }
        let dual = state.status == Status::Ready;
        let mut cards = h_flex()
            .justify_center()
            .items_center()
            .mt(surface::css(10.));
        if dual {
            cards = cards
                .gap(surface::css(20.))
                .child(self.receiver_pairing_card(Some(Category::Keyboard), true, cx))
                .child(self.receiver_pairing_card(Some(Category::Mouse), true, cx));
        } else {
            // ie nests .hyperpolling-device-box inside .hyperpolling-devices-box.
            cards = cards.child(
                div()
                    .mt(surface::css(10.))
                    .child(self.receiver_pairing_card(state.category, false, cx)),
            );
        }
        let mut body = v_flex()
            .id("receiver-pairing-content")
            .test_support()
            .role(Role::Group)
            .aria_label(t("PAIRING_UTILITY"))
            .w_full()
            .min_w(surface::css(850.))
            .min_h(surface::css(685.))
            .flex_1()
            .bg(Colors::panel())
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .text_color(Colors::dialog_warning_text())
            .child(
                div()
                    .w(surface::css(790.))
                    .max_w_full()
                    .mx_auto()
                    .mt(surface::css(20.))
                    .mb(surface::css(10.))
                    .text_center()
                    .child(t("HYPERPOLLING_WIRELESS_DONGLE_HEADER")),
            )
            .child(
                v_flex()
                    .w(surface::css(600.))
                    .mx_auto()
                    .child(
                        v_flex()
                            .items_center()
                            .child(
                                img("synapse/receiver/uma-pairing.png")
                                    .w(surface::css(77.))
                                    .h(surface::css(60.))
                                    .mb(surface::css(10.)),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .text_center()
                                    .child(t("HYPERPOLLING_WIRELESS_DONGLE").to_uppercase()),
                            ),
                    )
                    .child(connection_line(state.status, cx))
                    .child(cards),
            );
        if !(dual && state.bound.is_empty()) {
            body = body.child(self.receiver_pairing_precautions(cx));
        }
        if let Some(operation) = state.failure {
            let message = match operation {
                ReceiverOperation::Bindings => "无法读取配对信息。",
                ReceiverOperation::Scan => "扫描设备失败。",
                ReceiverOperation::Bind => "配对失败。",
                ReceiverOperation::Unbind => "取消配对失败。",
            };
            body = body.child(
                h_flex()
                    .id("receiver-pairing-service-error")
                    .test_support()
                    .role(Role::Group)
                    .aria_label(message)
                    .mx_auto()
                    .mt(surface::css(20.))
                    .gap(surface::css(10.))
                    .text_color(Colors::warning())
                    .child(message)
                    .child(self.receiver_pairing_button(
                        "receiver-retry-binding-read",
                        "RETRY",
                        false,
                        false,
                        Action::Refresh,
                        cx,
                    )),
            );
        }
        // The shell services binding reads. Scan/bind/unbind remain unsent local
        // intents; neither a read in progress nor an intent is a device result.
        if let Some(intent) = &state.pending {
            let reading = matches!(intent, ReceiverPairingIntent::QueryBindings);
            let message = match intent {
                ReceiverPairingIntent::Scan(_) => "已准备扫描请求，尚未发送到设备。",
                ReceiverPairingIntent::Bind(_) => "已准备配对请求，尚未发送到设备。",
                ReceiverPairingIntent::Unbind(_) => "已准备取消配对请求，尚未发送到设备。",
                ReceiverPairingIntent::QueryBindings => "正在读取配对信息…",
                _ => "请求尚未发送到设备。",
            };
            body = body.child(
                h_flex()
                    .id(if reading {
                        "receiver-pairing-binding-read"
                    } else {
                        "receiver-pairing-local-intent"
                    })
                    .test_support()
                    .role(Role::Status)
                    .aria_label(message)
                    .mx_auto()
                    .mt(surface::css(20.))
                    .gap(surface::css(10.))
                    .text_color(Colors::warning_text())
                    .child(message)
                    .when(!reading, |view| {
                        view.child(self.receiver_pairing_button(
                            "receiver-discard-intent",
                            "CANCEL",
                            false,
                            false,
                            Action::DiscardIntent,
                            cx,
                        ))
                    }),
            );
        }
        body.into_any_element()
    }

    fn receiver_pairing_card(
        &self,
        category: Option<Category>,
        dual: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let state = &self.receiver.pairing;
        let status = state.status;
        let category_key = category.map(Category::key).unwrap_or("");
        let width = if dual { 250. } else { 510. };
        let card_id = format!("receiver-pairing-card-{category_key}");
        let mut inner = v_flex().relative().size_full().overflow_hidden().when(
            status != Status::Scanned,
            |view| {
                view.child(
                    div()
                        .id(SharedString::from(format!(
                            "receiver-category-{category_key}"
                        )))
                        .test_support()
                        .h(surface::css(28.))
                        .pt(surface::css(10.))
                        .w_full()
                        .text_size(surface::css(14.))
                        .line_height(surface::css(18.))
                        .text_color(Colors::category())
                        .text_center()
                        .child(if category_key.is_empty() {
                            String::new()
                        } else {
                            t(category_key).to_uppercase()
                        }),
                )
            },
        );
        if status == Status::Scanned && !state.candidates.is_empty() {
            inner = inner.child(self.receiver_candidate_list(cx));
        } else {
            inner = inner.child(self.receiver_pairing_device(category, cx));
        }
        let pending = state.pending.is_some();
        inner = match status {
            Status::Scanning | Status::Upgrading | Status::Pairing | Status::Unpairing => {
                let label = match status {
                    Status::Scanning => t("SCANNING"),
                    Status::Upgrading => "Upgrading".into(), // Literal in 9473/oe.
                    Status::Pairing => t("PAIRING"),
                    _ => t("UNPAIRING"),
                };
                inner.child(status_text(
                    "receiver-pairing-progress",
                    label,
                    cx.theme().primary,
                ))
            }
            Status::Scanned if state.candidates.len() > 1 => inner.child(
                h_flex()
                    .justify_center()
                    .gap(surface::css(10.))
                    .child(self.receiver_pairing_button(
                        "receiver-cancel-candidates",
                        "CANCEL",
                        false,
                        false,
                        Action::CancelSelection,
                        cx,
                    ))
                    .child(self.receiver_pairing_button(
                        "receiver-bind-selected",
                        "PAIR",
                        true,
                        pending,
                        Action::Bind,
                        cx,
                    )),
            ),
            Status::Scanned if state.candidates.is_empty() => inner.child(status_text(
                "receiver-no-candidates",
                t("NO_DEVICE_FOUND"),
                Colors::warning(),
            )),
            Status::PairFailed => inner.child(status_text(
                "receiver-pair-failed",
                t("PAIRING_FAILED"),
                Colors::warning(),
            )),
            Status::Paired | Status::UnpairFailed if !state.bound.is_empty() => {
                let peer = &state.bound[0];
                inner.child(
                    v_flex()
                        .id("receiver-paired-device-label")
                        .test_support()
                        .w_full()
                        .h(surface::css(60.))
                        .px(surface::css(12.))
                        .text_size(surface::css(14.))
                        .line_height(surface::css(16.))
                        .text_center()
                        .child(
                            peer.label(&crate::i18n::locale().to_lowercase())
                                .to_uppercase(),
                        )
                        .when(status == Status::UnpairFailed, |view| {
                            view.child(status_text(
                                "receiver-unpair-failed",
                                t("UNPAIRING_FAILED"),
                                Colors::warning(),
                            ))
                        }),
                )
            }
            _ => inner.child(
                Link::new(SharedString::from(format!(
                    "receiver-compatible-{category_key}"
                )))
                .href("https://www.razer.com/technology/razer-hyperpolling")
                .open_with(|url, _, _, cx| cx.open_url(url))
                .accessibility_label(t("VIEW_COMPATIBLE_DEVICES"))
                .w_full()
                .h(surface::css(50.))
                .text_center()
                .underline()
                .text_size(surface::css(14.))
                .line_height(surface::css(16.))
                .child(t("VIEW_COMPATIBLE_DEVICES")),
            ),
        };
        if status == Status::ConfirmUnpair {
            inner = inner.child(
                div()
                    .absolute()
                    .top(surface::css(10.))
                    .left(surface::css((width - 210.) / 2.))
                    .w(surface::css(210.))
                    .child(
                        v_flex()
                            .id("receiver-unpair-confirmation")
                            .test_support()
                            .role(Role::Group)
                            .aria_label(t("HYPERPOLLING_WIRELESS_DONGLE_UNPAIR_CONFIRM_TEXT"))
                            .bg(Colors::card())
                            .border_1()
                            .border_color(Colors::warning())
                            .rounded(surface::css(3.))
                            .pt(surface::css(18.))
                            .pb(surface::css(20.))
                            .child(
                                div()
                                    .w(surface::css(190.))
                                    .mx_auto()
                                    .mt(surface::css(3.))
                                    .mb(surface::css(9.))
                                    .text_size(surface::css(14.))
                                    .line_height(surface::css(17.))
                                    .text_center()
                                    .child(t("HYPERPOLLING_WIRELESS_DONGLE_UNPAIR_CONFIRM_TEXT")),
                            )
                            .child(
                                h_flex()
                                    .justify_center()
                                    .gap(surface::css(10.))
                                    .child(self.receiver_pairing_button(
                                        "receiver-cancel-unpair",
                                        "CANCEL",
                                        false,
                                        false,
                                        Action::CancelUnpair,
                                        cx,
                                    ))
                                    .child(self.receiver_pairing_button(
                                        "receiver-confirm-unpair",
                                        "CONFIRM",
                                        true,
                                        pending,
                                        Action::Unbind,
                                        cx,
                                    )),
                            ),
                    ),
            );
        }
        div()
            .id(SharedString::from(card_id))
            .test_support()
            .role(Role::Group)
            .aria_label(if category_key.is_empty() {
                t("PAIRING_UTILITY")
            } else {
                t(category_key)
            })
            .w(surface::css(width))
            .h(surface::css(210.))
            .flex_shrink_0()
            .rounded(surface::css(5.))
            .bg(Colors::card())
            .when(!state.bound.is_empty(), |view| {
                view.shadow(vec![BoxShadow {
                    color: cx.theme().primary,
                    offset: point(px(0.), px(0.)),
                    blur_radius: px(0.),
                    spread_radius: cx.theme().font_size * (2. / 16.),
                    inset: false,
                }])
            })
            .child(inner)
            .into_any_element()
    }

    fn receiver_pairing_device(
        &self,
        category: Option<Category>,
        cx: &Context<Self>,
    ) -> AnyElement {
        let state = &self.receiver.pairing;
        let mut region = div()
            .relative()
            .w_full()
            .h(surface::css(120.))
            .flex_shrink_0();
        if state.bound.is_empty() || state.status == Status::Unpairing {
            let category_key = category.map(Category::key).unwrap_or("");
            let mut placeholder = v_flex().pt(surface::css(20.)).items_center();
            if let Some(category) = category {
                placeholder = placeholder.child(
                    img(category_asset(category, false))
                        .size(surface::css(44.))
                        .mb(surface::css(10.)),
                );
            } else {
                placeholder = placeholder.child(div().h(surface::css(54.)));
            }
            if matches!(
                state.status,
                Status::Scanning | Status::Upgrading | Status::Pairing | Status::Unpairing
            ) {
                placeholder = placeholder.child(progress_spinner(cx));
            } else if let Some(category) = category {
                let disabled = state.pending.is_some()
                    || (state.status == Status::Ready
                        && state.firmware_version.as_deref().is_none_or(str::is_empty));
                let label = if state.status == Status::Scanned && state.candidates.is_empty() {
                    "RESCAN"
                } else {
                    "SELECT"
                };
                placeholder = placeholder.child(self.receiver_pairing_button(
                    SharedString::from(format!("receiver-select-{category_key}")),
                    label,
                    true,
                    disabled,
                    Action::Scan(category),
                    cx,
                ));
            }
            region = region.child(placeholder);
        } else if let Some(peer) = state.bound.first() {
            let image = peer.image_identity().and_then(peer_image);
            region = region.when_some(image, |view, asset| {
                view.child(img(asset).h_full().w_full().object_fit(ObjectFit::Contain))
            });
            if state.status == Status::Paired {
                region = region.child(
                    div()
                        .absolute()
                        .inset_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            BaseButton::new("receiver-open-unpair-confirmation")
                                .accessibility_label(t("UNPAIR"))
                                .h(surface::css(28.))
                                .px(surface::css(5.))
                                .rounded(surface::css(2.))
                                .line_height(surface::css(28.))
                                .text_size(surface::css(12.))
                                .bg(Colors::secondary())
                                .text_color(Colors::secondary_text())
                                .hover(|style| style.bg(Colors::unpair_hover()))
                                .active(|style| style.bg(Colors::unpair_pressed()))
                                .child(t("UNPAIR").to_uppercase())
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.receiver_pairing_action(Action::ConfirmUnpair, cx)
                                })),
                        ),
                );
            }
        }
        region.into_any_element()
    }

    fn receiver_candidate_list(&self, cx: &Context<Self>) -> AnyElement {
        let state = &self.receiver.pairing;
        let count = state.candidates.len();
        let title = if count <= 1 {
            String::new()
        } else {
            state.category.map_or_else(
                || count.to_string(),
                |category| {
                    format!(
                        "{count} {}",
                        t(if category == Category::Mouse {
                            "MICE_FOUND"
                        } else {
                            "KEYBOARDS_FOUND"
                        })
                        .to_uppercase()
                    )
                },
            )
        };
        let mut options = v_flex()
            .id("receiver-candidate-options")
            .test_support()
            .role(Role::Group)
            .aria_label(title.clone())
            .h(surface::css(104.))
            .w_full()
            .ml(surface::css(10.))
            .overflow_y_scroll()
            .gap(surface::css(16.));
        for (index, peer) in state.candidates.iter().enumerate() {
            let label = peer
                .label(&crate::i18n::locale().to_lowercase())
                .to_uppercase();
            let selected = index == state.selected;
            let identity = format!("receiver-candidate-{}-{index}", peer.identity());
            options = options.child(
                BaseRadio::new(SharedString::from(identity))
                    .accessibility_label(label.clone())
                    .checked(selected)
                    .disabled(state.pending.is_some())
                    .flex()
                    .items_center()
                    .gap(surface::css(10.))
                    .min_h(surface::css(20.))
                    .w_full()
                    .ml(surface::css(20.))
                    .child(
                        div()
                            .size(surface::css(20.))
                            .rounded_full()
                            .border_1()
                            .border_color(crate::ui::theme::ControlPodAudioColors::radio_border())
                            .flex()
                            .items_center()
                            .justify_center()
                            .flex_shrink_0()
                            .when(selected, |view| {
                                view.child(
                                    div()
                                        .size(surface::css(10.))
                                        .rounded_full()
                                        .bg(cx.theme().primary),
                                )
                            }),
                    )
                    .child(
                        div()
                            .text_size(surface::css(14.))
                            .line_height(surface::css(20.))
                            .child(label),
                    )
                    .on_change({
                        let entity = cx.weak_entity();
                        move |_, _, _, cx| {
                            let _ = entity.update(cx, |this, cx| {
                                this.receiver.pairing.select(index);
                                cx.notify();
                            });
                        }
                    }),
            );
        }
        v_flex()
            .relative()
            .left(surface::css(-6.))
            .w_full()
            .child(
                div()
                    .id("receiver-candidate-count")
                    .test_support()
                    .h(surface::css(20.))
                    .my(surface::css(16.))
                    .text_center()
                    .line_height(surface::css(20.))
                    .child(title),
            )
            .child(options)
            .into_any_element()
    }

    fn receiver_pairing_precautions(&self, cx: &Context<Self>) -> AnyElement {
        let state = &self.receiver.pairing;
        if matches!(
            state.status,
            Status::Ready
                | Status::Scanned
                | Status::Upgrading
                | Status::Scanning
                | Status::PairFailed
                | Status::UnpairFailed
        ) {
            let mut list = v_flex()
                .flex_1()
                .ml(surface::css(38.))
                .text_color(Colors::warning_text());
            for key in [
                "HYPER_SPEED_MULTI_DEVICE_DONGLE_DUALLINK_NOTE_ONE",
                "DUALLINK_NOTE_TWO",
                "DUALLINK_NOTE_THREE",
                "HYPERPOLLING_WIRELESS_DONGLE_NOTE_FOUR",
                "DUALLINK_NOTE_FIVE",
            ] {
                list = list.child(h_flex().gap(surface::css(8.)).child("•").child(t(key)));
            }
            return h_flex()
                .id("receiver-pairing-precautions")
                .test_support()
                .mx_auto()
                .mt(surface::css(20.))
                .h(surface::css(85.))
                .min_w(surface::css(520.))
                .max_w(surface::css(714.))
                .px(surface::css(12.))
                .when_some(state.category, |view, category| {
                    view.child(
                        img(category_asset(category, true))
                            .size(surface::css(60.))
                            .flex_shrink_0(),
                    )
                })
                .child(list)
                .into_any_element();
        }
        let message = if state.status == Status::Unpaired {
            Some("UNPAIRING_COMPLETE")
        } else if state.status == Status::Paired && !state.candidates.is_empty() {
            Some("PAIRING_COMPLETE")
        } else {
            None
        };
        div()
            .id("receiver-pairing-completion")
            .test_support()
            .w(surface::css(230.))
            .mx_auto()
            .mt(surface::css(20.))
            .text_center()
            .line_height(surface::css(20.))
            .text_color(cx.theme().primary)
            .children(message.map(t))
            .into_any_element()
    }

    fn receiver_pairing_button(
        &self,
        id: impl Into<ElementId>,
        key: &str,
        primary: bool,
        disabled: bool,
        action: Action,
        cx: &Context<Self>,
    ) -> BaseButton {
        BaseButton::new(id)
            .accessibility_label(t(key))
            .disabled(disabled)
            .min_w(surface::css(90.))
            .h(surface::css(28.))
            .px(surface::css(5.))
            .rounded(surface::css(3.))
            .text_size(surface::css(12.))
            .line_height(surface::css(28.))
            .text_center()
            .bg(if primary && !disabled {
                cx.theme().primary
            } else {
                Colors::secondary()
            })
            .text_color(if primary && !disabled {
                Colors::primary_text()
            } else {
                Colors::secondary_text()
            })
            .opacity(if disabled {
                0.6
            } else if primary {
                1.
            } else {
                0.7
            })
            .when(!disabled, |button| {
                button.hover(move |style| style.opacity(if primary { 0.7 } else { 1. }))
            })
            .focus_visible(|style| style.border_1().border_color(cx.theme().primary))
            .child(t(key).to_uppercase())
            .on_click(cx.listener(move |this, _, _, cx| this.receiver_pairing_action(action, cx)))
    }
}

fn category_asset(category: Category, precaution: bool) -> &'static str {
    match (category, precaution) {
        (Category::Keyboard, false) => "synapse/dock-164-icon_category_keyboard.svg",
        (Category::Mouse, false) => "synapse/dock-164-icon_category_mouse.svg",
        (Category::Keyboard, true) => "synapse/dock-164-icon_kb_ensure.svg",
        (Category::Mouse, true) => "synapse/dock-164-icon_mouse_ensure.svg",
    }
}
fn peer_image(identity: (u32, u32, u32)) -> Option<&'static str> {
    // 9473/ne retries AVIF -> PNG for the same three identity fields. The
    // general dashboard helper normalizes some layouts, which is not this path.
    const IMAGES: &[(u32, u32, u32, &str)] =
        include!("../../../assets/synapse/dashboard-images.rs");
    const ARMORY: &[(u32, u32, u32, &str)] =
        include!("../../../assets/synapse/armory-dashboard-images.rs");
    IMAGES
        .iter()
        .chain(ARMORY)
        .find_map(|&(pid, edition, layout, asset)| {
            ((pid, edition, layout) == identity).then_some(asset)
        })
}
fn status_text(id: &'static str, text: String, color: Hsla) -> AnyElement {
    div()
        .id(id)
        .test_support()
        .role(Role::Group)
        .aria_label(text.clone())
        .w_full()
        .text_center()
        .text_size(surface::css(14.))
        .line_height(surface::css(16.))
        .text_color(color)
        .child(text)
        .into_any_element()
}
fn progress_spinner(cx: &App) -> AnyElement {
    let frame = |phase: f32| {
        gpui_kit::component::Icon::default()
            .path("synapse/dock-164-icon-progress_spinner.svg")
            .size(surface::css(24.))
            .transform(Transformation::rotate(radians(
                phase * std::f32::consts::TAU,
            )))
    };
    let spinner = div()
        .id("receiver-operation-spinner")
        .test_support()
        .size(surface::css(26.));
    if cx.reduce_motion() {
        spinner.child(frame(0.)).into_any_element()
    } else {
        spinner
            .with_animation(
                "receiver-operation-spin",
                Animation::new(Duration::from_secs(1))
                    .repeat()
                    .with_easing(linear),
                move |view, phase| view.child(frame(phase)),
            )
            .into_any_element()
    }
}
fn connection_line(status: Status, cx: &App) -> AnyElement {
    let connecting = matches!(
        status,
        Status::Ready
            | Status::Upgrading
            | Status::Scanning
            | Status::Scanned
            | Status::Pairing
            | Status::ConfirmUnpair
            | Status::Unpairing
    );
    let asset = if status == Status::Paired {
        "synapse/dock-164-icon_marching_ants_master_line.svg"
    } else {
        "synapse/dock-164-icon_marching_ants_not_connected_top_to_bottom.svg"
    };
    let container = div()
        .id("receiver-pairing-connection-line")
        .test_support()
        .w_full()
        .h(surface::css(30.))
        .mt(surface::css(10.))
        .flex()
        .justify_center();
    if !connecting {
        return container
            .child(
                img(asset)
                    .w(surface::css(if status == Status::Paired {
                        260.
                    } else {
                        250.
                    }))
                    .h(surface::css(30.)),
            )
            .into_any_element();
    }
    let color = cx.theme().primary;
    let frame = move |phase: f32| {
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let unit = f32::from(bounds.size.height) / 30.;
                let offset = (1. - phase) * 22.;
                for n in -2..4 {
                    let start = (n as f32 * 22. - offset).max(0.);
                    let end = (n as f32 * 22. - offset + 2.).min(30.);
                    if end > start {
                        let mut path = PathBuilder::stroke(px(2. * unit));
                        path.move_to(point(bounds.center().x, bounds.top() + px(start * unit)));
                        path.line_to(point(bounds.center().x, bounds.top() + px(end * unit)));
                        if let Ok(path) = path.build() {
                            window.paint_path(path, color);
                        }
                    }
                }
            },
        )
        .w(surface::css(250.))
        .h(surface::css(30.))
    };
    if cx.reduce_motion() {
        container.child(frame(0.)).into_any_element()
    } else {
        container
            .with_animation(
                "receiver-connecting-line",
                Animation::new(Duration::from_millis(500))
                    .repeat()
                    .with_easing(linear),
                move |view, phase| view.child(frame(phase)),
            )
            .into_any_element()
    }
}
