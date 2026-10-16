//! Current 3595 Ph({}) -> dgq -> ON_SET_CAMERA_MIC_SETTINGS custom handler.
//! Switches stage reducer values; DEVICE_REBOOT submits SetAudioFunction.
//! Source optimism is separate from the validated KS/flash/reboot receipt.
use super::*;
use razer_device::{
    camera_processing_family::{
        CameraFamilyProcessingRequest as Request, CameraFamilyProcessingValue as MicValue,
        validate_camera_family_processing_progress,
    },
    camera_submission::{
        CameraCommand, CameraCommandOutcome, CameraScope, CameraSubmissionPlan,
        CameraSubmissionProgress, compile_camera_edit,
    },
};
use razer_widgets::theme::CameraProductColors as Colors;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

#[derive(Clone, Copy, Debug)]
enum Operation {
    Read,
    Reboot([bool; 2]),
}
impl Operation {
    fn plan(self) -> anyhow::Result<CameraSubmissionPlan> {
        Ok(match self {
            Self::Read => CameraSubmissionPlan {
                commands: vec![CameraCommand::FamilyProcessing(Request::GetAudioFunction)],
            },
            Self::Reboot([adaptive_noise_reduction, crystal_clear_voice]) => compile_camera_edit(
                3595,
                "apply-microphone",
                &serde_json::json!({
                    "deviceMic": { "adaptiveNoiseReduction": adaptive_noise_reduction, "crystalClearVoice": crystal_clear_voice }
                }),
            )?,
        })
    }
}

#[derive(Default)]
pub(super) struct State {
    scope: Option<CameraScope>,
    values: [bool; 2],
    dirty: bool,
    generation: u64,
    jobs: BTreeMap<u64, Arc<AtomicBool>>,
    // Diagnostics do not become source reducer state or a synthetic success.
    observation: Option<[u8; 2]>,
    diagnostic: Option<String>,
}
impl Drop for State {
    fn drop(&mut self) {
        for canceled in self.jobs.values() {
            canceled.store(true, Ordering::Release);
        }
    }
}
impl State {
    fn set_scope(&mut self, scope: Option<CameraScope>) -> bool {
        if self.scope == scope {
            return false;
        }
        for canceled in self.jobs.values() {
            canceled.store(true, Ordering::Release);
        }
        self.scope = scope;
        self.values = [false; 2];
        self.dirty = false;
        self.observation = None;
        self.diagnostic = None;
        true
    }
    fn edit(&mut self, key: &str, value: bool) -> bool {
        let ix = match key {
            "3595:adaptiveNoiseReduction" => 0,
            "3595:crystalClearVoice" => 1,
            _ => return false,
        };
        self.values[ix] = value;
        self.dirty = true;
        true
    }
    fn reboot(&mut self) -> Operation {
        // Current dgq clears isMicDirty before its optional broadcast. A native
        // failure does not roll this source state back or invent a confirmation.
        self.dirty = false;
        Operation::Reboot(self.values)
    }
    fn finish(
        &mut self,
        scope: &CameraScope,
        operation: Operation,
        result: &Result<CameraSubmissionProgress, String>,
    ) {
        if self.scope.as_ref() != Some(scope) {
            return;
        }
        self.diagnostic = match result {
            Err(error) => Some(error.clone()),
            Ok(progress) => progress.error.clone(),
        };
        match operation {
            Operation::Reboot(values) => {
                // Current taskMaker catches failures and still broadcasts the
                // submitted values. bC6 does not change isMicDirty, including
                // when another switch edit happened while this call ran.
                self.values = values;
            }
            Operation::Read => {
                if let Ok(progress) = result {
                    for outcome in &progress.outcomes {
                        if let CameraCommandOutcome::FamilyProcessing(p) = outcome
                            && validate_camera_family_processing_progress(
                                3595,
                                Request::GetAudioFunction,
                                p,
                            )
                            .is_ok()
                            && let Some(MicValue::AudioFunction {
                                adaptive_noise_reduction,
                                crystal_clear_voice,
                            }) = p.value
                        {
                            self.observation =
                                Some([adaptive_noise_reduction, crystal_clear_voice]);
                            self.values = [adaptive_noise_reduction != 0, crystal_clear_voice != 0];
                        }
                    }
                }
            }
        }
    }
}

impl SourceControls {
    fn sync_camera_microphone(&mut self) {
        if self.spec.product_id == 3595 {
            self.draft["deviceMic"] = serde_json::json!({
                "adaptiveNoiseReduction": self.camera_mic.values[0],
                "crystalClearVoice": self.camera_mic.values[1],
            });
        }
    }
    pub(super) fn restore_camera_microphone(&mut self) {
        // MIC belongs to the device reducer, not to the selected camera profile.
        // A saved local profile cannot substitute for GetAudioFunction.
        self.sync_camera_microphone();
    }
    pub(super) fn set_camera_microphone_scope(
        &mut self,
        scope: Option<CameraScope>,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id != 3595 || !self.camera_mic.set_scope(scope) {
            return;
        }
        self.sync_camera_microphone();
        let Some(handle) = self.camera_runtime.window else {
            return;
        };
        let owner = cx.entity().downgrade();
        cx.defer(move |cx| {
            let _ = cx.update_window(handle, |_, window, cx| {
                let _ = owner.update(cx, |this, cx| {
                    this.start_camera_microphone(Operation::Read, window, cx)
                });
            });
        });
    }
    pub(super) fn edit_camera_microphone(
        &mut self,
        key: &str,
        value: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.spec.product_id != 3595
            || !value
                .as_bool()
                .is_some_and(|value| self.camera_mic.edit(key, value))
        {
            return false;
        }
        self.sync_camera_microphone();
        self.sync(window, cx);
        cx.emit(SourceControlsChanged);
        cx.notify();
        true
    }
    fn reboot_camera_microphone(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.camera_settings_blocked() || !self.camera_mic.dirty {
            return;
        }
        let operation = self.camera_mic.reboot();
        self.start_camera_microphone(operation, window, cx);
        cx.notify();
    }
    fn start_camera_microphone(
        &mut self,
        operation: Operation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(scope) = self.camera_mic.scope.clone() else {
            self.camera_mic.diagnostic =
                Some("camera microphone has no active observed hardware scope".into());
            eprintln!(
                "camera microphone diagnostic: {}",
                self.camera_mic.diagnostic.as_deref().unwrap()
            );
            return;
        };
        // This custom middleware handler is outside RzTaskManager. Do not
        // coalesce it with image tasks or automatically save on switch edits.
        let plan = match operation.plan().and_then(|plan| {
            plan.validate(scope.product_id)?;
            Ok(plan)
        }) {
            Ok(plan) => plan,
            Err(error) => {
                self.camera_mic.diagnostic = Some(error.to_string());
                return;
            }
        };
        let signal = Arc::new(AtomicBool::new(false));
        let canceled = signal.clone();
        let (send, receive) = mpsc::sync_channel(1);
        let job_scope = scope.clone();
        let worker = match thread::Builder::new()
            .name("camera-microphone-ipc".into())
            .spawn(move || {
                let result = camera_runtime::execute_job(job_scope, plan, signal, None);
                let _ = send.send(result);
            }) {
            Ok(worker) => worker,
            Err(error) => {
                self.camera_mic
                    .finish(&scope, operation, &Err(error.to_string()));
                eprintln!("camera microphone worker: {error}");
                self.sync_camera_microphone();
                return;
            }
        };
        self.camera_mic.generation = self.camera_mic.generation.wrapping_add(1);
        let generation = self.camera_mic.generation;
        self.camera_mic.jobs.insert(generation, canceled.clone());
        cx.spawn_in(window, async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(40))
                    .await;
                let result = match receive.try_recv() {
                    Ok(result) => result,
                    Err(mpsc::TryRecvError::Empty) => {
                        if view.upgrade().is_none() {
                            canceled.store(true, Ordering::Release);
                        }
                        continue;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        Err("camera microphone worker disconnected".into())
                    }
                };
                let _ = worker.join();
                let _ = view.update_in(cx, |this, window, cx| {
                    this.camera_mic.jobs.remove(&generation);
                    if !canceled.load(Ordering::Acquire)
                        && this.camera_mic.scope.as_ref() == Some(&scope)
                    {
                        this.camera_mic.finish(&scope, operation, &result);
                        if let Some(error) = &this.camera_mic.diagnostic {
                            eprintln!("camera microphone transport diagnostic: {error}");
                        }
                        this.sync_camera_microphone();
                        this.sync(window, cx);
                        cx.emit(SourceControlsChanged);
                        cx.notify();
                    }
                });
                break;
            }
        })
        .detach();
    }
    pub(super) fn render_camera_microphone(
        &self,
        page: &'static PageSpec,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let disabled = self.camera_settings_blocked();
        v_flex()
            .id("3595:microphone-settings")
            .text_color(Colors::text())
            .children(
                page.sections
                    .iter()
                    .flat_map(|section| &section.controls)
                    .map(|control| {
                        v_flex()
                            .child(
                                div().mb(surface::css(10.)).child(
                                    self.render_camera_microphone_switch(control, disabled, cx),
                                ),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .my(surface::css(20.))
                                    .border_1()
                                    .border_color(Colors::divider())
                                    .rounded(surface::css(2.))
                                    .when(disabled, |divider| divider.opacity(0.3)),
                            )
                    }),
            )
            .when(self.camera_mic.dirty, |column| {
                column.child(
                    h_flex()
                        .items_center()
                        .mb(surface::css(10.))
                        .when(disabled, |row| row.opacity(0.3))
                        // Byte-identical to current 3595 icon_warning.6c0cd78b.svg;
                        // the shared pack already retains these original bytes.
                        .child(
                            img("synapse/onboard-transfer-warning.svg")
                                .size(surface::css(20.))
                                .flex_shrink_0(),
                        )
                        .child(
                            v_flex()
                                .ml(surface::css(10.))
                                .text_size(surface::css(14.))
                                .line_height(surface::css(16.))
                                .child(razer_i18n::t("DEVICE_REBOOT_DESCRIPTION"))
                                .child(
                                    gpui_kit::base::Button::new("3595:microphone-reboot")
                                        .accessibility_label(razer_i18n::t("DEVICE_REBOOT"))
                                        .disabled(disabled)
                                        .self_start()
                                        .p_0()
                                        .text_color(Colors::focus())
                                        .underline()
                                        .when(!disabled, |button| button.cursor_pointer())
                                        .focus_visible(|button| {
                                            button.border_1().border_color(Colors::focus())
                                        })
                                        .child(razer_i18n::t("DEVICE_REBOOT"))
                                        .on_click(cx.listener(|this, _, window, cx| {
                                            this.reboot_camera_microphone(window, cx)
                                        })),
                                ),
                        ),
                )
            })
            .into_any_element()
    }
    fn render_camera_microphone_switch(
        &self,
        control: &ControlSpec,
        disabled: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let key = control.key.clone();
        h_flex()
            .items_center()
            .text_size(surface::css(14.))
            .line_height(surface::css(16.))
            .child(
                div()
                    .when(disabled, |label| label.opacity(0.3))
                    .child(razer_i18n::t(&control.label)),
            )
            .child(
                div().ml(surface::css(10.)).child(
                    surface::SynapseSwitch::new(SharedString::from(key.clone()))
                        .accessibility_label(razer_i18n::t(&control.label))
                        .checked(
                            self.value(control)
                                .and_then(Value::as_bool)
                                .unwrap_or(false),
                        )
                        .disabled(disabled)
                        .on_change(cx.listener(move |this, value, window, cx| {
                            this.edit(&key, Value::Bool(*value), window, cx)
                        })),
                ),
            )
            .when_some(control.tooltip.as_ref(), |row, text| {
                let help = if disabled {
                    // Current disabled container suppresses tooltip interaction.
                    div()
                        .size(surface::css(14.))
                        .bg(camera_theme::help())
                        .rounded(surface::css(7.))
                        .opacity(0.3)
                        .child(img("synapse/camera-section-help.svg").size_full())
                        .into_any_element()
                } else {
                    camera_sections::help(
                        SharedString::from(format!("{}:help", control.key)),
                        razer_i18n::t(text),
                        false,
                    )
                };
                row.child(div().ml(surface::css(10.)).child(help))
            })
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{Operation, State};
    use razer_device::camera_submission::CameraScope;
    use razer_device::{
        camera_extensions::CameraKsCallOutcome,
        camera_focus_family::CameraFamilyFocusTransport,
        camera_processing_family::{
            CameraFamilyProcessingRequest, execute_camera_family_processing,
        },
        camera_submission::{CameraCommandOutcome, CameraSubmissionProgress},
    };
    use std::prelude::v1::test;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    fn scope(id: &str) -> CameraScope {
        CameraScope {
            product_id: 3595,
            container_id: id.into(),
        }
    }
    #[test]
    fn microphone_source_switches_stage_both_values_and_reboot_clears_dirty_optimistically() {
        let mut state = State::default();
        state.set_scope(Some(scope("camera-a")));
        assert!(state.edit("3595:adaptiveNoiseReduction", true));
        assert!(state.dirty);
        assert_eq!(state.values, [true, false]);
        let Operation::Reboot(values) = state.reboot() else {
            panic!("missing reboot")
        };
        assert!(!state.dirty);
        assert_eq!(values, [true, false]);
        state.finish(
            &scope("camera-a"),
            Operation::Reboot(values),
            &Err("simulated device rejection".into()),
        );
        assert!(!state.dirty);
        assert_eq!(state.values, values);
        assert!(state.diagnostic.is_some() && state.observation.is_none());
    }
    #[test]
    fn microphone_source_broadcast_keeps_a_later_edit_dirty_and_old_device_receipts_are_ignored() {
        let mut state = State::default();
        state.set_scope(Some(scope("camera-a")));
        state.edit("3595:adaptiveNoiseReduction", true);
        let operation = state.reboot();
        state.edit("3595:crystalClearVoice", true);
        state.finish(
            &scope("camera-a"),
            operation,
            &Err("simulated failure".into()),
        );
        assert!(state.dirty);
        assert_eq!(state.values, [true, false]);
        state.set_scope(Some(scope("camera-b")));
        state.finish(&scope("camera-a"), operation, &Err("old device".into()));
        assert_eq!(state.values, [false, false]);
        assert!(!state.dirty && state.diagnostic.is_none());
    }
    #[test]
    fn microphone_source_initializes_from_a_validated_actual_read_and_failed_reads_preserve_state()
    {
        struct MemoryMic;
        impl CameraFamilyFocusTransport for MemoryMic {
            fn begin_topology(&mut self) -> anyhow::Result<u32> {
                Ok(1)
            }
            fn begin_node(&mut self, _: u32) -> anyhow::Result<()> {
                Ok(())
            }
            fn ks_property(
                &mut self,
                header: &[u8; 32],
                data: &mut [u8; 8],
            ) -> anyhow::Result<CameraKsCallOutcome> {
                if header[20] == 2 {
                    assert_eq!(*data, [0xc0, 0x0e, 0xa1, 0, 0, 0, 0, 0]);
                } else {
                    *data = [0, 0, 0, 0, 1, 0, 0, 0];
                }
                Ok(CameraKsCallOutcome {
                    hresult: 0,
                    bytes_returned: 8,
                })
            }
            fn end_node(&mut self) {}
            fn end_topology(&mut self) {}
            fn wait_ms(&mut self, _: u32) -> anyhow::Result<()> {
                Ok(())
            }
        }
        let receipt = execute_camera_family_processing(
            &mut MemoryMic,
            3595,
            CameraFamilyProcessingRequest::GetAudioFunction,
            &|| Ok(()),
        );
        let mut state = State::default();
        state.set_scope(Some(scope("camera-a")));
        state.finish(
            &scope("camera-a"),
            Operation::Read,
            &Ok(CameraSubmissionProgress {
                completed: true,
                outcomes: vec![CameraCommandOutcome::FamilyProcessing(receipt.clone())],
                ..Default::default()
            }),
        );
        assert_eq!(state.values, [false, true]);
        assert_eq!(state.observation, Some([0, 1]));
        state.finish(
            &scope("camera-a"),
            Operation::Read,
            &Err("simulated getter failure".into()),
        );
        assert_eq!(state.values, [false, true]);
        let mut forged = receipt;
        forged.exchanges[0].command.bytes[2] = 0xa0;
        forged.value = Some(super::MicValue::AudioFunction {
            adaptive_noise_reduction: 1,
            crystal_clear_voice: 0,
        });
        state.finish(
            &scope("camera-a"),
            Operation::Read,
            &Ok(CameraSubmissionProgress {
                completed: true,
                outcomes: vec![CameraCommandOutcome::FamilyProcessing(forged)],
                ..Default::default()
            }),
        );
        assert_eq!(state.values, [false, true]);
    }
    #[test]
    fn microphone_device_replacement_and_drop_cancel_outstanding_workers() {
        let mut state = State::default();
        state.set_scope(Some(scope("camera-a")));
        let canceled = Arc::new(AtomicBool::new(false));
        state.jobs.insert(1, canceled.clone());
        assert!(!state.set_scope(Some(scope("camera-a"))));
        assert!(!canceled.load(Ordering::Acquire));
        state.set_scope(Some(scope("camera-b")));
        assert!(canceled.load(Ordering::Acquire));
        let canceled = Arc::new(AtomicBool::new(false));
        state.jobs.insert(2, canceled.clone());
        drop(state);
        assert!(canceled.load(Ordering::Acquire));
    }
}
