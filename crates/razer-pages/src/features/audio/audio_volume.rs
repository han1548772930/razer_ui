//! Current 1352 XU volume intents and real simple_service endpoint observations.
//! Default/restored local values never issue a setter.
use super::AudioProductWorkspace;
use gpui_kit::component::{ActiveTheme, WindowExt as _, h_flex};
use gpui_kit::*;
use razer_device::simple_audio_volume::{AudioVolumeReadResult, AudioVolumeWriteResult};
use razer_i18n::t;
use razer_widgets::{source_slider::SourceSlider, surface};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioVolumeOperation {
    Read,
    Write { mute: bool, volume: u8 },
}
#[derive(Clone, Copy, Debug)]
pub struct AudioVolumeRequest {
    pub generation: u64,
    pub operation: AudioVolumeOperation,
    epoch: u64,
    edit_revision: u64,
}
pub enum AudioVolumeReply {
    Read(AudioVolumeReadResult),
    Write(AudioVolumeWriteResult),
}
pub struct AudioVolumeCompletion {
    pub reply: AudioVolumeReply,
    pub warning: Option<String>,
    pub endpoint_current: bool,
}
#[derive(Default)]
pub(super) struct State {
    active: bool,
    epoch: u64,
    generation: u64,
    edit_revision: u64,
    pending: Option<AudioVolumeRequest>,
    queued: Option<AudioVolumeOperation>,
    needs_read: bool,
    observed: Option<AudioVolumeReadResult>,
    visible: Option<(bool, u8)>,
    preview: Option<u8>,
    error: Option<String>,
    warning: Option<String>,
    cancellation: Option<Arc<AtomicBool>>,
}
impl State {
    fn invalidate(&mut self) {
        if let Some(cancellation) = &self.cancellation {
            cancellation.store(true, Ordering::Release);
        }
        self.epoch = self.epoch.wrapping_add(1);
        self.queued = None;
        self.observed = None;
        self.visible = None;
        self.preview = None;
        self.error = None;
        self.warning = None;
        self.needs_read = self.active;
        // Keep the physical worker's slot until completion even after scope changes.
    }
}
impl EventEmitter<AudioVolumeRequest> for AudioProductWorkspace {}
impl AudioProductWorkspace {
    /// Current 1352 `XU` uses the widget's title switch, one range, and its
    /// OpenSoundVolume command. The descriptor's extra checkbox and repeated
    /// VOLUME_HEADER label are not mounted by that component.
    pub(super) fn source_volume_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let enabled = self
            .volume_enabled()
            .unwrap_or_else(|| self.draft["device"]["volume"]["isEnabled"] == true);
        let value = self
            .volume_preview()
            .or_else(|| self.volume_value())
            .map(f32::from)
            .unwrap_or_else(|| {
                self.draft["device"]["volume"]["value"]
                    .as_f64()
                    .unwrap_or(50.) as f32
            });
        let state = &self.sliders["/device/volume/value"];
        surface::panel_with_title_switch(
            t("VOLUME_HEADER"),
            surface::SynapseSwitch::new("audio-volume-switch")
                .checked(enabled)
                .accessibility_label(t("VOLUME_HEADER"))
                .on_change(cx.listener(|this, enabled, window, cx| {
                    this.toggle_volume(*enabled, window, cx);
                })),
            div()
                .absolute()
                .right(surface::css(10.))
                .top(surface::css(10.))
                .child(surface::help_control(
                    "audio-volume-help",
                    t("VOLUME_NOMMO_TOOLTIP"),
                )),
            cx,
        )
        .relative()
        .child(
            div()
                .relative()
                .child(
                    SourceSlider::new(state, value / 100.)
                        .tip(Some(format!("{value:.0}")))
                        .enabled(enabled),
                )
                .child(
                    h_flex()
                        .absolute()
                        .bottom(surface::css(-2.))
                        .w_full()
                        .justify_between()
                        .text_size(surface::css(14.))
                        .opacity(if enabled { 1. } else { 0.3 })
                        // XU passes explicit string labels to VU for this page.
                        .child("0")
                        .child("100"),
                ),
        )
        .child(
            gpui_kit::base::Button::new("audio-open-volume-mixer")
                .group("audio-open-volume-mixer")
                .accessibility_label(t("OPEN_WINDOW_VOLUME_MIXER"))
                .h(surface::css(44.))
                .line_height(surface::css(44.))
                .flex()
                .items_center()
                .self_start()
                .text_color(cx.theme().foreground)
                .text_size(surface::css(14.))
                .underline()
                .hover(|style| style.text_color(cx.theme().primary))
                .focus_visible(|style| style.text_color(cx.theme().primary))
                .child(t("OPEN_WINDOW_VOLUME_MIXER"))
                .child(
                    div()
                        .relative()
                        .size(surface::css(20.))
                        .ml(surface::css(4.))
                        .mb(surface::css(2.))
                        .child(img("synapse/audio-volume-external-sprite.svg#link").size_full())
                        .child(
                            img("synapse/audio-volume-external-sprite.svg#hover")
                                .absolute()
                                .inset_0()
                                .size_full()
                                .opacity(0.)
                                .group_hover("audio-open-volume-mixer", |style| style.opacity(1.)),
                        ),
                )
                .on_click(|_, window, cx| {
                    if let Err(error) =
                        razer_platform::system::open(razer_platform::system::Properties::Volume)
                    {
                        window.push_notification(format!("无法打开系统音量：{error}"), cx);
                    }
                }),
        )
        .into_any_element()
    }
    pub fn set_volume_active(&mut self, active: bool, cx: &mut Context<Self>) {
        let active = active && self.spec.product_id == 1352;
        if self.volume.active != active {
            self.volume.active = active;
            self.volume.invalidate();
        }
        self.request_volume_read(cx);
        cx.notify();
    }
    pub fn invalidate_volume(&mut self) {
        self.volume.invalidate();
    }
    pub fn volume_request_matches(&self, request: AudioVolumeRequest) -> bool {
        self.volume.pending.is_some_and(|pending| {
            pending.generation == request.generation && pending.operation == request.operation
        })
    }
    pub fn volume_request_current(&self, request: AudioVolumeRequest) -> bool {
        self.volume_request_matches(request)
            && self.volume.epoch == request.epoch
            && self.volume.active
    }
    pub fn volume_cancellation(&self, request: AudioVolumeRequest) -> Option<Arc<AtomicBool>> {
        self.volume_request_matches(request)
            .then(|| self.volume.cancellation.clone())
            .flatten()
    }
    fn dispatch_volume(&mut self, operation: AudioVolumeOperation, cx: &mut Context<Self>) {
        if self.spec.product_id != 1352 || !self.volume.active {
            return;
        }
        if self.volume.pending.is_some() {
            if matches!(operation, AudioVolumeOperation::Write { .. }) {
                self.volume.queued = Some(operation);
            }
            return;
        }
        self.volume.generation = self.volume.generation.wrapping_add(1);
        let request = AudioVolumeRequest {
            generation: self.volume.generation,
            operation,
            epoch: self.volume.epoch,
            edit_revision: self.volume.edit_revision,
        };
        self.volume.pending = Some(request);
        self.volume.cancellation = Some(Arc::new(AtomicBool::new(false)));
        self.volume.needs_read = false;
        self.volume.error = None;
        self.volume.warning = None;
        cx.emit(request);
    }
    pub(super) fn request_volume_read(&mut self, cx: &mut Context<Self>) {
        if self.volume.needs_read {
            self.dispatch_volume(AudioVolumeOperation::Read, cx);
        }
    }
    pub(super) fn preview_volume(&mut self, volume: u8, cx: &mut Context<Self>) {
        self.volume.edit_revision = self.volume.edit_revision.wrapping_add(1);
        self.volume.preview = Some(volume);
        cx.notify();
    }
    pub(super) fn volume_preview(&self) -> Option<u8> {
        self.volume.preview
    }
    pub(super) fn volume_active(&self) -> bool {
        self.volume.active
    }
    pub(super) fn volume_value(&self) -> Option<u8> {
        self.volume.visible.map(|(_, value)| value)
    }
    pub(super) fn volume_enabled(&self) -> Option<bool> {
        self.volume.visible.map(|(enabled, _)| enabled)
    }
    pub(super) fn commit_volume(&mut self, value: u8, window: &mut Window, cx: &mut Context<Self>) {
        if !self.volume.active {
            return;
        }
        self.volume.preview = None;
        // XU.changeValue derives enabled from non-zero; toggle preserves value.
        self.draft["device"]["volume"] = serde_json::json!({"isEnabled":value != 0,"value":value});
        self.volume.visible = Some((value != 0, value));
        self.sync(window, cx);
        cx.emit(super::AudioProductChanged);
        self.request_volume_write(cx);
    }
    pub(super) fn toggle_volume(
        &mut self,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.volume.active {
            return;
        }
        let Some(value) = self.volume_value().or_else(|| {
            self.draft
                .pointer("/device/volume/value")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| u8::try_from(value).ok())
        }) else {
            return;
        };
        self.volume.visible = Some((enabled, value));
        self.draft["device"]["volume"] = serde_json::json!({"isEnabled":enabled,"value":value});
        self.sync(window, cx);
        cx.emit(super::AudioProductChanged);
        self.request_volume_write(cx);
    }
    pub(super) fn request_volume_write(&mut self, cx: &mut Context<Self>) {
        if self.spec.product_id != 1352 || !self.volume.active {
            return;
        }
        let Some(volume) = self
            .draft
            .pointer("/device/volume/value")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| u8::try_from(value).ok())
            .filter(|value| *value <= 100)
        else {
            return;
        };
        let Some(enabled) = self
            .draft
            .pointer("/device/volume/isEnabled")
            .and_then(serde_json::Value::as_bool)
        else {
            return;
        };
        self.volume.edit_revision = self.volume.edit_revision.wrapping_add(1);
        self.volume.observed = None;
        self.volume.error = None;
        self.volume.warning = None;
        self.volume.needs_read = false;
        if let Some(cancellation) = &self.volume.cancellation {
            cancellation.store(true, Ordering::Release);
        }
        self.dispatch_volume(
            AudioVolumeOperation::Write {
                mute: !enabled,
                volume,
            },
            cx,
        );
    }
    pub fn finish_volume(
        &mut self,
        request: AudioVolumeRequest,
        result: Result<AudioVolumeCompletion, String>,
        scope_current: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.volume_request_matches(request) {
            return;
        }
        let mut current = self.volume_request_current(request);
        if !scope_current && request.epoch == self.volume.epoch {
            self.volume.invalidate();
            self.volume.needs_read = false;
            current = false;
            self.volume.error = Some(match request.operation {
                AudioVolumeOperation::Read => "音频连接已变化，当前音量未确认".into(),
                AudioVolumeOperation::Write { .. } => {
                    "音频连接已变化；设备可能已更新，当前状态未确认".into()
                }
            });
        }
        if current && scope_current {
            let result = result.and_then(|completion| {
                if self.volume.edit_revision == request.edit_revision {
                    self.volume.warning = completion.warning;
                }
                if completion.endpoint_current {
                    Ok(completion.reply)
                } else {
                    Err(match request.operation {
                        AudioVolumeOperation::Read => "扬声器连接发生变化，当前音量未确认".into(),
                        AudioVolumeOperation::Write { .. } => {
                            "音量设置已返回，但扬声器连接发生变化；设备可能已更新，当前状态未确认"
                                .into()
                        }
                    })
                }
            });
            match result {
                Ok(AudioVolumeReply::Read(reading))
                    if self.volume.edit_revision == request.edit_revision =>
                {
                    if reading.result && reading.volume <= 100 {
                        self.volume.observed = Some(reading.clone());
                        self.volume.visible = Some((!reading.muted, reading.volume));
                        if let Some(slider) = self.sliders.get("/device/volume/value") {
                            self.syncing = true;
                            slider.update(cx, |slider, cx| {
                                slider.set_value(f32::from(reading.volume), window, cx)
                            });
                            self.syncing = false;
                        }
                        // This read updates the visible reducer-equivalent state only.
                        // It is not a local save event or a hardware write.
                    } else {
                        self.volume.error = Some(format!("音量读取失败：{}", reading.reason));
                    }
                }
                Ok(AudioVolumeReply::Read(_)) => {} // User edits make an older read stale.
                Ok(AudioVolumeReply::Write(write))
                    if self.volume.edit_revision == request.edit_revision =>
                {
                    if let Some(reading) = write
                        .observation
                        .filter(|reading| reading.result && reading.volume <= 100)
                    {
                        self.volume.observed = Some(reading);
                    }
                    if !write.source_response.result {
                        self.volume.error =
                            Some(if write.volume_submitted || write.mute_submitted {
                                "音量或静音仅部分更新，请重新读取".into()
                            } else {
                                format!("音量设置失败：{}", write.source_response.reason)
                            });
                    } else if let Some(error) = write.observation_error {
                        self.volume.error = Some(format!("音量已提交，但回读失败：{error}"));
                    } else if let (AudioVolumeOperation::Write { mute, volume }, Some(reading)) =
                        (request.operation, &self.volume.observed)
                    {
                        if reading.muted != mute || reading.volume != volume {
                            self.volume.error = Some("音量回读与请求不同；保留当前本地选择".into());
                        }
                    } else {
                        self.volume.error = Some("音量已提交，但没有设备回读确认".into());
                    }
                }
                Ok(AudioVolumeReply::Write(_)) => {}
                Err(error) if self.volume.edit_revision == request.edit_revision => {
                    self.volume.error = Some(error);
                }
                Err(_) => {}
            }
        }
        self.volume.pending = None;
        self.volume.cancellation = None;
        if let Some(next) = self.volume.queued.take() {
            self.dispatch_volume(next, cx);
        } else {
            self.request_volume_read(cx);
        }
        cx.notify();
    }
}
