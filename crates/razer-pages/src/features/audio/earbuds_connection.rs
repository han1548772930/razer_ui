//! The current radio snapshot gates earbud controls independently of profiles.
use super::*;
use razer_device::{earbuds_auto_pause::Report, earbuds_events::Notification};

#[derive(Default)]
pub(super) struct State {
    identity: String,
    generation: u64,
    active: bool,
    disconnected: Option<bool>,
    error: Option<String>,
    power_cursor: Option<Point<Pixels>>,
    microphone_cursor: Option<Point<Pixels>>,
}
impl State {
    pub(super) fn sync(
        &mut self,
        device: &razer_model::model::Device,
        active: bool,
        generation: u64,
    ) {
        let identity = device.workspace_identity();
        if self.identity != identity || self.generation != generation || !active {
            self.disconnected = None;
            self.error = None;
            self.power_cursor = None;
            self.microphone_cursor = None;
        }
        self.identity = identity;
        self.generation = generation;
        self.active = active;
    }
    pub(super) fn ready(&self) -> bool {
        self.active && self.disconnected == Some(false)
    }
    pub(super) fn disconnected(&self) -> bool {
        self.disconnected == Some(true)
    }
    pub(super) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
    pub(super) fn leave(&mut self) {
        self.power_cursor = None;
        self.microphone_cursor = None;
    }
    pub(super) fn stop(&mut self) {
        self.active = false;
        self.disconnected = None;
        self.error = None;
        self.leave();
    }
}

#[derive(Clone, Copy)]
pub(super) enum Card {
    Power,
    Microphone,
}
impl Card {
    fn id(self) -> &'static str {
        match self {
            Self::Power => "earbuds-power-radio",
            Self::Microphone => "earbuds-microphone-radio",
        }
    }
    fn cursor(self, state: &State) -> Option<Point<Pixels>> {
        match self {
            Self::Power => state.power_cursor,
            Self::Microphone => state.microphone_cursor,
        }
    }
    fn set_cursor(self, state: &mut State, cursor: Option<Point<Pixels>>) {
        match self {
            Self::Power => state.power_cursor = cursor,
            Self::Microphone => state.microphone_cursor = cursor,
        }
    }
}

impl AudioProductWorkspace {
    pub fn observe_earbuds_shared_connection(&mut self, report: &Report, cx: &mut Context<Self>) {
        if !matches!(
            report.operation,
            razer_device::earbuds_auto_pause::Operation::Read
                | razer_device::earbuds_auto_pause::Operation::Battery { .. }
        ) {
            return;
        }
        if report.product_id != self.spec.product_id || !self.earbuds_connection.active {
            return;
        }
        self.earbuds_connection.disconnected = match report.connected {
            Some(false) => Some(true),
            Some(true) if report.left_battery.is_some() && report.right_battery.is_some() => {
                Some(report.has_disconnected_earbud())
            }
            _ => None,
        };
        self.earbuds_connection.error = None;
        if self.earbuds_connection.ready() {
            self.resume_earbuds_power(cx);
            self.resume_earbuds_microphone(cx);
        } else {
            self.earbuds_power.cancel();
            self.cancel_earbuds_microphone();
        }
        cx.notify();
    }
    pub fn observe_earbuds_shared_connection_error(
        &mut self,
        error: String,
        cx: &mut Context<Self>,
    ) {
        self.earbuds_connection.disconnected = None;
        self.earbuds_connection.error = Some(error.clone());
        self.earbuds_sound.cancel();
        self.earbuds_sound.input_result = Some(Err(error.clone()));
        self.earbuds_activation.code = Some(Err(error.clone()));
        self.earbuds_activation.loading = false;
        self.finish_earbuds_power_monitor(error.clone(), cx);
        self.finish_earbuds_microphone_monitor(error, cx);
    }
    pub fn observe_earbuds_power_notification(
        &mut self,
        event: &Notification,
        cx: &mut Context<Self>,
    ) {
        if !self.earbuds_connection.active {
            return;
        }
        if let Notification::Connection { status, .. } = event {
            if *status == 0 {
                self.earbuds_sound.cancel();
                self.earbuds_activation.clear_device();
            }
            self.earbuds_connection.disconnected = if *status == 0 { Some(true) } else { None };
            self.earbuds_power.cancel();
            self.cancel_earbuds_microphone();
            if *status > 3 {
                self.earbuds_connection.error =
                    Some(format!("Unknown earbud connection status {status}"));
            }
        }
        cx.notify();
    }
    pub(super) fn earbuds_disconnected_surface(
        &self,
        card: Card,
        panel: AnyElement,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let disconnected = self.earbuds_connection.disconnected();
        div()
            .id(card.id())
            .test_support()
            .relative()
            .child(panel)
            .when(disconnected, |wrapper| {
                wrapper
                    .child(
                        div()
                            .id(SharedString::from(format!("{}-occluder", card.id())))
                            .absolute()
                            .inset_0()
                            .occlude()
                            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                                if !hovered {
                                    card.set_cursor(&mut this.earbuds_connection, None);
                                    cx.notify();
                                }
                            }))
                            .on_mouse_move(cx.listener(
                                move |this, event: &MouseMoveEvent, _, cx| {
                                    card.set_cursor(
                                        &mut this.earbuds_connection,
                                        Some(event.position),
                                    );
                                    cx.notify();
                                },
                            )),
                    )
                    .children(card.cursor(&self.earbuds_connection).map(|cursor| {
                        use razer_widgets::theme::AudioEqualizerColors as C;
                        razer_assets::deferred_resource(
                            super::earbuds_equalizer_view::CursorTip::new(
                                cursor,
                                div()
                                    .max_w(surface::css(320.))
                                    .px(surface::css(10.))
                                    .py(surface::css(8.))
                                    .border_1()
                                    .border_color(C::border())
                                    .bg(C::tip_surface())
                                    .text_color(C::text())
                                    .text_size(surface::css(14.))
                                    .line_height(surface::css(16.))
                                    .child(
                                        gpui_kit::base::Tooltip::new(SharedString::from(format!(
                                            "{}-tip",
                                            card.id()
                                        )))
                                        .child(t("LIMITED_FUNCTIONALITY_EARBUD")),
                                    ),
                            ),
                        )
                        .with_priority(999999)
                    }))
            })
            .into_any_element()
    }
}
