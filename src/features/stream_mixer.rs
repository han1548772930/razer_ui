//! 3334/3337 yU: enable confirmation and playback endpoint selection.
//! Middleware observations never come from product/profile catalogues. Local
//! choices stay separate from observations and do not acknowledge device writes.
use super::*;
use crate::ui::theme::StreamMixerColors as Colors;
use gpui_kit::base::{Button as BaseButton, Dialog};
use gpui_kit::component::{select::SelectItem, tooltip::Tooltip};
use std::time::Instant;

const ENABLE: &str = "/device/streamMixerSettings/isStreamMixerEnabled";
const PLAYBACK: &str = "/device/streamMixerSettings/playbackMixDevice";
const PAGE: &str = "STREAM_MIXER_HEADER";

/// Typed projections of the three current MW reducer actions (see receipts).
/// A transport adapter must supply real observations; no publisher is connected yet.
#[derive(Clone)]
#[allow(dead_code)]
pub(crate) enum StreamMixerObservation {
    ActiveMixer(bool),
    PlaybackDevices(Vec<String>),
    PlaybackDevice(String),
    Unavailable(String),
}

#[derive(Clone)]
pub(super) struct DeviceChoice {
    name: String,
    disabled: bool,
}
impl SelectItem for DeviceChoice {
    type Value = String;
    fn title(&self) -> SharedString {
        self.name.clone().into()
    }
    fn value(&self) -> &String {
        &self.name
    }
    fn disabled(&self) -> bool {
        self.disabled
    }
}

struct Warning {
    focus: FocusHandle,
    return_focus: Option<FocusHandle>,
    opened: Instant,
}

pub(super) struct MixerState {
    devices: Option<Vec<String>>,
    active_mixer: Option<bool>,
    observed_device: Option<String>,
    local_selection: bool,
    select: Entity<SelectState<Vec<DeviceChoice>>>,
    warning: Option<Warning>,
    read_error: Option<String>,
}
impl MixerState {
    pub(super) fn new(pid: u32, window: &mut Window, cx: &mut App) -> Option<Self> {
        matches!(pid, 3334 | 3337).then(|| Self {
            devices: None,
            active_mixer: None,
            observed_device: None,
            local_selection: false,
            select: cx.new(|cx| SelectState::new(Vec::new(), None, window, cx)),
            warning: None,
            read_error: None,
        })
    }
}

impl AudioProductWorkspace {
    pub(super) fn subscribe_mixer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(state) = &self.mixer else {
            return;
        };
        self.subscriptions.push(cx.subscribe_in(
            &state.select,
            window,
            |this, _, event, window, cx| {
                if let SelectEvent::Confirm(Some(name)) = event {
                    this.select_mixer_playback(name, window, cx);
                }
            },
        ));
    }

    fn mixer_enabled(&self) -> bool {
        self.draft.pointer(ENABLE) == Some(&Value::Bool(true))
    }

    fn mixer_playback(&self) -> String {
        let Some(state) = &self.mixer else {
            return String::new();
        };
        if !state.local_selection {
            if let Some(name) = &state.observed_device {
                return name.clone();
            }
        }
        self.draft
            .pointer(PLAYBACK)
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned()
    }

    fn mixer_choices(&self) -> Vec<DeviceChoice> {
        let Some(state) = &self.mixer else {
            return Vec::new();
        };
        let selected = self.mixer_playback();
        let mut choices = state
            .devices
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|name| DeviceChoice {
                name: name.clone(),
                disabled: false,
            })
            .collect::<Vec<_>>();
        // yU appends even an empty missing name, never silently chooses another endpoint.
        if !choices.iter().any(|choice| choice.name == selected) {
            choices.push(DeviceChoice {
                name: selected,
                disabled: true,
            });
        }
        choices
    }

    pub(super) fn sync_mixer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let choices = self.mixer_choices();
        let selected = self.mixer_playback();
        if let Some(state) = &self.mixer {
            state.select.update(cx, |state, cx| {
                state.set_items(choices, window, cx);
                state.set_selected_value(&selected, window, cx);
            });
        }
    }

    fn select_mixer_playback(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        if self.syncing {
            return;
        }
        let available = self.mixer.as_ref().is_some_and(|s| {
            s.warning.is_none()
                && s.devices
                    .as_ref()
                    .is_some_and(|list| list.iter().any(|n| n == name))
        });
        // Recheck against the latest collection after a menu was opened. A
        // disconnected row and stale confirmation must never become a local edit.
        if self.page != PAGE || !self.mixer_enabled() || !available {
            self.sync_mixer(window, cx);
            return;
        }
        let was_local = self.mixer.as_ref().unwrap().local_selection;
        let value_changed = self.draft.pointer(PLAYBACK) != Some(&json!(name));
        self.mixer.as_mut().unwrap().local_selection = true;
        self.edit(PLAYBACK, json!(name), window, cx);
        if !was_local && !value_changed {
            // The local override itself changes the snapshot, even when its
            // value equals the reducer default. Do not lose that explicit choice.
            self.sync_mixer(window, cx);
            cx.emit(AudioProductChanged);
        }
    }

    pub(super) fn mixer_snapshot(&self, snapshot: &mut Value) {
        if self.mixer.as_ref().is_some_and(|s| !s.local_selection) {
            if let Some(fields) = snapshot
                .pointer_mut("/device/streamMixerSettings")
                .and_then(Value::as_object_mut)
            {
                fields.remove("playbackMixDevice");
            }
        }
    }

    pub(super) fn restore_mixer(
        &mut self,
        saved: Option<&Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.dismiss_mixer_warning(window, cx);
        if let Some(state) = self.mixer.as_mut() {
            state.local_selection = saved
                .and_then(|v| v.pointer(PLAYBACK))
                .is_some_and(Value::is_string);
        }
    }

    /// This is an observation boundary, not a query success or a write receipt.
    pub(crate) fn observe_stream_mixer(
        &mut self,
        observation: StreamMixerObservation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = self.mixer.as_mut() else {
            return;
        };
        match observation {
            StreamMixerObservation::ActiveMixer(active) => state.active_mixer = Some(active),
            StreamMixerObservation::PlaybackDevices(devices) => state.devices = Some(devices),
            StreamMixerObservation::PlaybackDevice(name) => state.observed_device = Some(name),
            StreamMixerObservation::Unavailable(error) => {
                state.devices = None;
                state.active_mixer = None;
                state.read_error = Some(error);
            }
        }
        if state.devices.is_some() && state.active_mixer.is_some() {
            state.read_error = None;
        }
        self.sync_mixer(window, cx);
        cx.notify();
    }

    pub(super) fn request_mixer_enable(
        &mut self,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.page != PAGE || self.mixer.is_none() {
            return;
        }
        let state = self.mixer.as_mut().unwrap();
        if state.warning.is_some() {
            return;
        }
        if enabled && state.active_mixer == Some(true) {
            let return_focus = window.focused(cx);
            let focus = cx.focus_handle();
            focus.focus(window, cx);
            state.warning = Some(Warning {
                focus,
                return_focus,
                opened: Instant::now(),
            });
            cx.notify();
        } else {
            self.edit(ENABLE, json!(enabled), window, cx);
        }
    }

    pub(super) fn dismiss_mixer_warning(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(warning) = self.mixer.as_mut().and_then(|s| s.warning.take()) {
            if let Some(focus) = warning.return_focus {
                focus.focus(window, cx);
            }
            cx.notify();
        }
    }

    fn confirm_mixer_enable(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.page != PAGE || !self.mixer.as_ref().is_some_and(|s| s.warning.is_some()) {
            return;
        }
        self.dismiss_mixer_warning(window, cx);
        self.edit(ENABLE, json!(true), window, cx);
    }

    pub(super) fn mixer_section(
        &self,
        section: &AudioSection,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let state = self.mixer.as_ref()?;
        if self.page != PAGE || section.title != PAGE {
            return None;
        }
        let enabled = self.mixer_enabled();
        Some(
            surface::panel_with_title_switch(
                t(PAGE),
                surface::SynapseSwitch::new("stream-mixer-enable")
                    .accessibility_label(t(PAGE))
                    .checked(enabled)
                    .on_change(cx.listener(|this, next: &bool, window, cx| {
                        this.request_mixer_enable(*next, window, cx)
                    })),
                div(),
                cx,
            )
            .child(
                div()
                    .opacity(if enabled { 1. } else { 0.3 })
                    .child(t("STREAM_MIXER_INFORMATION")),
            )
            .children(
                section
                    .controls
                    .iter()
                    .filter(|c| c.path != ENABLE)
                    .map(|c| self.render_control(c, cx)),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(if state.active_mixer.is_none() {
                        "尚未读取混音器状态；当前编辑为本地草稿。"
                    } else {
                        "当前编辑为本地草稿，尚未写入设备。"
                    }),
            )
            .when_some(state.read_error.as_ref(), |panel, error| {
                panel.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(error.clone()),
                )
            })
            .into_any_element(),
        )
    }

    pub(super) fn mixer_playback_selector(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(state) = &self.mixer else {
            return div().into_any_element();
        };
        let selected = self.mixer_playback();
        let disconnected = state
            .devices
            .as_ref()
            .is_some_and(|list| !list.contains(&selected));
        let enabled = self.mixer_enabled();
        v_flex()
            .gap_2()
            .child(
                h_flex()
                    .items_start()
                    .opacity(if enabled { 1. } else { 0.3 })
                    .when(disconnected, |row| {
                        row.child(
                            div()
                                .id("stream-mixer-offline")
                                .w(surface::css(27.))
                                .h(surface::css(20.))
                                .mt(surface::css(5.))
                                .mr(surface::css(-10.))
                                .when(enabled, |icon| {
                                    icon.tooltip(|window, cx| {
                                        Tooltip::new(t("DEVICE_NOT_FOUND_TIP")).build(window, cx)
                                    })
                                })
                                .child(
                                    img("synapse/stream-mixer-warning-icon.svg")
                                        .w(surface::css(20.))
                                        .h(surface::css(17.)),
                                ),
                        )
                    })
                    .child(
                        surface::select(&state.select)
                            .items(self.mixer_choices())
                            .accessibility_label(t("PLAYBACK_MIX"))
                            .disabled(
                                !enabled || state.devices.is_none() || state.warning.is_some(),
                            )
                            .flex_1()
                            .min_w_0(),
                    ),
            )
            .when(state.devices.is_none(), |column| {
                column.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child("尚未读取播放设备。"),
                )
            })
            .into_any_element()
    }

    pub(super) fn mixer_dialog(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if self.page != PAGE || self.mixer_enabled() {
            return None;
        }
        let warning = self.mixer.as_ref()?.warning.as_ref()?;
        let opacity = (warning.opened.elapsed().as_secs_f32() / 0.1).clamp(0., 1.);
        if opacity < 1. {
            window.request_animation_frame();
        }
        let action =
            |id, label: &str, primary, handler: fn(&mut Self, &mut Window, &mut Context<Self>)| {
                BaseButton::new(id)
                    .accessibility_label(t(label))
                    .w(surface::css(90.))
                    .h(surface::css(27.))
                    .rounded(surface::css(3.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .bg(if primary {
                        Colors::accent()
                    } else {
                        Colors::secondary()
                    })
                    .text_color(if primary {
                        Colors::button_text()
                    } else {
                        Colors::text()
                    })
                    .text_size(surface::css(12.))
                    .child(t(label).to_uppercase())
                    .on_click(cx.listener(move |this, _, window, cx| handler(this, window, cx)))
            };
        let panel = div()
            .relative()
            .occlude()
            .w(surface::css(620.))
            .h(surface::css(164.))
            .px(surface::css(30.))
            .py(surface::css(20.))
            .border_1()
            .border_color(Colors::warning())
            .rounded(surface::css(5.))
            .bg(Colors::panel())
            .text_color(Colors::text())
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .child(
                h_flex()
                    .justify_center()
                    .mb(surface::css(20.))
                    .text_color(Colors::warning())
                    .text_size(surface::css(16.))
                    .line_height(surface::css(16.8))
                    .child(
                        img("synapse/stream-mixer-warning.svg")
                            .size(surface::css(25.))
                            .mr(surface::css(10.)),
                    )
                    .child(t("STREAM_MIXER_DETECTED").to_uppercase()),
            )
            .child(
                div()
                    .text_center()
                    .line_height(surface::css(16.8))
                    .child(t("STREAM_MIXER_DETECTED_WARNING")),
            )
            .child(
                h_flex()
                    .justify_center()
                    .mt(surface::css(17.))
                    .gap(surface::css(20.))
                    .child(action(
                        "stream-mixer-cancel",
                        "CANCEL",
                        false,
                        Self::dismiss_mixer_warning,
                    ))
                    .child(action(
                        "stream-mixer-confirm",
                        "ENABLE",
                        true,
                        Self::confirm_mixer_enable,
                    )),
            );
        Some(
            Dialog::new(cx)
                .focus_handle(warning.focus.clone())
                .close_on_backdrop_press(false)
                .close_on_escape(false)
                // yU supplies only Cancel/Enable; no backdrop, Escape or Enter action.
                .on_cancel(|_, _, _| false)
                .on_ok(|_, _, _| false)
                .backdrop(
                    div()
                        .absolute()
                        .inset_0()
                        .bg(Colors::backdrop())
                        .opacity(opacity),
                )
                .popup(
                    div()
                        .absolute()
                        .left_0()
                        .bottom(relative(0.5))
                        .w(window.viewport_size().width)
                        .flex()
                        .justify_center()
                        .opacity(opacity)
                        .child(panel),
                )
                .into_any_element(),
        )
    }
}
