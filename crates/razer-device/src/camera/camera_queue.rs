//! Current MW NORMAL_SKIPPABLE queues, one current task per source task name.
use crate::camera_submission::{CameraScope, CameraSubmissionPlan, CameraSubmissionProgress};
use serde_json::Value;
use std::collections::{BTreeMap, VecDeque};
#[derive(Clone, Debug)]
pub struct CameraQueuedIntent {
    pub generation: u64,
    pub epoch: u64,
    pub key: String,
    pub task_name: String,
    pub scope: CameraScope,
    pub plan: CameraSubmissionPlan,
    pub edit_revision: u64,
}
#[derive(Default)]
pub struct CameraIntentQueue {
    scope: Option<CameraScope>,
    epoch: u64,
    generation: u64,
    edit_revision: u64,
    current: BTreeMap<u64, CameraQueuedIntent>,
    ready: VecDeque<u64>,
    queued: VecDeque<CameraQueuedIntent>,
    pub skipped_tasks: u64,
    read_needed: bool,
    read_retry_deferred: bool,
    pub last_progress: Option<CameraSubmissionProgress>,
    pub error: Option<String>,
    pub last_edit_progress: Option<CameraSubmissionProgress>,
    pub observation_error: Option<String>,
}
impl CameraIntentQueue {
    pub fn scope(&self) -> Option<&CameraScope> {
        self.scope.as_ref()
    }
    pub fn pending(&self) -> bool {
        !self.current.is_empty()
    }
    pub fn has_waiting_intents(&self) -> bool {
        !self.ready.is_empty() || !self.queued.is_empty()
    }
    pub fn read_needed(&self) -> bool {
        self.read_needed
    }
    /// A failed current getter retains its refresh requirement, but completion
    /// must not immediately resubmit it. A later page/scope/edit event may retry.
    pub fn continue_after_completion(&self) -> bool {
        self.has_waiting_intents() || !self.read_retry_deferred
    }
    pub fn invalidate(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
        // Source page/profile changes do not call cancelAllTasks. Keep
        // already dispatched edits, while rejecting their stale UI receipts.
        self.queued.retain(|intent| intent.key != "read");
        let unstarted_reads: Vec<_> = self
            .ready
            .iter()
            .copied()
            .filter(|id| {
                self.current
                    .get(id)
                    .is_some_and(|intent| intent.key == "read")
            })
            .collect();
        self.ready.retain(|id| !unstarted_reads.contains(id));
        self.current.retain(|id, _| !unstarted_reads.contains(id));
        self.last_progress = None;
        self.error = None;
        self.last_edit_progress = None;
        self.observation_error = None;
        self.read_needed = true;
        self.read_retry_deferred = false;
    }
    pub fn set_scope(&mut self, scope: Option<CameraScope>) -> bool {
        if self.scope == scope {
            return false;
        }
        self.invalidate();
        // A replaced physical owner must not submit its not-yet-started work
        // through the newly selected device. In-flight jobs retain their scope.
        self.queued.clear();
        for id in self.ready.drain(..) {
            self.current.remove(&id);
        }
        self.scope = scope;
        true
    }
    pub fn edit(&mut self, key: String, plan: CameraSubmissionPlan) -> anyhow::Result<()> {
        let scope = self
            .scope
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("camera has no active observed hardware scope"))?;
        scope.validate()?;
        plan.validate(scope.product_id)?;
        let (prefix, name) = key
            .split_once(':')
            .map_or(("", key.as_str()), |(p, n)| (p, n));
        let exposure_tasks = if name == "reset-exposure" || name == "auto-exposure" {
            use crate::camera::{CameraControlProperty, StandardCameraProperty};
            use crate::camera_exposure_family::CameraFamilyExposureRequest as X;
            use crate::camera_extensions::CameraExtensionRequest as E;
            use crate::camera_submission::CameraCommand;
            let Some(CameraCommand::StandardWrite(setting)) = plan.commands.first() else {
                anyhow::bail!("camera exposure task omits its standard mode write");
            };
            anyhow::ensure!(
                setting.property
                    == StandardCameraProperty::CameraControl(CameraControlProperty::Exposure)
                    && setting.setting.value == 0,
                "invalid camera exposure mode task"
            );
            let automatic = setting.setting.is_auto;
            let valid_companions = match (scope.product_id, automatic, &plan.commands[1..]) {
                (
                    3594,
                    true,
                    [
                        CameraCommand::Extension(E::SetAutoExposureMetering { .. }),
                        CameraCommand::Extension(E::SetEvCompensation { .. }),
                    ],
                )
                | (
                    3594,
                    false,
                    [
                        CameraCommand::Extension(E::SetIsoSensitivity { .. }),
                        CameraCommand::Extension(E::SetShutterSpeed { .. }),
                    ],
                )
                | (
                    3595 | 3596,
                    true,
                    [
                        CameraCommand::FamilyExposure(X::SetAutoExposureMetering { .. }),
                        CameraCommand::FamilyExposure(X::SetEvCompensation { .. }),
                    ],
                )
                | (
                    3595,
                    false,
                    [
                        CameraCommand::FamilyExposure(X::SetIsoSensitivity { .. }),
                        CameraCommand::FamilyExposure(X::SetShutterSpeed { .. }),
                    ],
                ) => true,
                (
                    3596,
                    false,
                    [
                        CameraCommand::StandardWrite(manual),
                        CameraCommand::FamilyExposure(X::SetShutterSpeed { .. }),
                    ],
                ) => manual.property == setting.property && !manual.setting.is_auto,
                _ => false,
            };
            anyhow::ensure!(
                valid_companions
                    && (name != "reset-exposure"
                        || matches!(scope.product_id, 3594 | 3595) && automatic),
                "invalid camera exposure task sequence"
            );
            Some(if automatic {
                ["auto-exposure", "metering", "compensation"]
            } else if scope.product_id == 3596 {
                // N0J starts immediately; Wcp waits under the same name.
                ["auto-exposure", "shutter", "shutter"]
            } else {
                ["auto-exposure", "iso", "shutter"]
            })
        } else {
            None
        };
        if matches!(name, "pan-tilt" | "view-preset") {
            use crate::camera::{CameraControlProperty as C, StandardCameraProperty as P};
            use crate::camera_submission::CameraCommand;
            let expected = if name == "pan-tilt" {
                vec![C::Pan, C::Tilt]
            } else {
                vec![C::Zoom, C::Pan, C::Tilt]
            };
            anyhow::ensure!(plan.commands.len() == expected.len() && plan.commands.iter().zip(expected).all(|(command, property)| {
                matches!(command, CameraCommand::StandardWrite(setting) if setting.property == P::CameraControl(property))
            }), "invalid camera view task sequence");
        }
        self.edit_revision = self.edit_revision.wrapping_add(1);
        self.error = None;
        self.observation_error = None;
        self.read_needed = true;
        self.read_retry_deferred = false;
        if let Some(tasks) = exposure_tasks {
            // MW enqueues NORMAL_SKIPPABLE tasks with separate result
            // handlers. A failed metering task does not cancel compensation,
            // and each setting coalesces with later edits of that same field.
            for (task, command) in tasks.into_iter().zip(plan.commands) {
                let key = if prefix.is_empty() {
                    task.into()
                } else {
                    format!("{prefix}:{task}")
                };
                self.enqueue(
                    key,
                    CameraSubmissionPlan {
                        commands: vec![command],
                    },
                );
            }
        } else if matches!(name, "pan-tilt" | "view-preset") {
            // The current view task maker enqueues zoom, pan and tilt by name;
            // a focus or image-preset runner remains one compound task.
            use crate::camera::{CameraControlProperty as C, StandardCameraProperty as P};
            use crate::camera_submission::CameraCommand;
            for command in plan.commands {
                let task = match &command {
                    CameraCommand::StandardWrite(s) => match s.property {
                        P::CameraControl(C::Zoom) => "zoom",
                        P::CameraControl(C::Pan) => "pan",
                        P::CameraControl(C::Tilt) => "tilt",
                        _ => unreachable!("validated view command"),
                    },
                    _ => unreachable!("validated view command"),
                };
                self.enqueue(
                    if prefix.is_empty() {
                        task.into()
                    } else {
                        format!("{prefix}:{task}")
                    },
                    CameraSubmissionPlan {
                        commands: vec![command],
                    },
                );
            }
        } else {
            let key = if scope.product_id == 3596 && name == "manual-exposure" {
                if prefix.is_empty() {
                    "shutter".into()
                } else {
                    format!("{prefix}:shutter")
                }
            } else {
                key
            };
            self.enqueue(key, plan);
        }
        Ok(())
    }
    fn enqueue(&mut self, key: String, plan: CameraSubmissionPlan) {
        let name = key.split_once(':').map_or(key.as_str(), |(_, n)| n);
        let task_name = match name {
            "autofocus" | "focus" | "focus-tracking" | "focus-mode" | "focus-lighting" => {
                "autoFocus"
            }
            "auto-exposure" | "reset-exposure" => "autoExposure",
            "metering" => "autoExposureMetering",
            "manual-exposure" | "shutter" => "shutterSpeed",
            "image-preset" | "brightness" | "contrast" | "saturation" | "sharpness" | "gain"
            | "white-balance" | "auto-white-balance" => "imagePreset",
            "anti-flicker" => "antiFlicker",
            "mirror" => "mirrorVideo",
            "state2D" | "state3D" => "dynamicNoiseReduction",
            "hdr-mode" => "hdr",
            "auto-quality" | "quality-mode" | "manual-quality" | "mjpeg-quality" => "mjpegQuality",
            "watermark-position" => "watermark",
            "auto-framing" => "CAMERA_AUTOFRAMING",
            "shutter-guard" => "CAMERA_SHUTTER_LIMIT_CONTROL",
            "save-camera" => "advancedCamera",
            other => other,
        }
        .to_owned();
        self.generation = self.generation.wrapping_add(1);
        let intent = CameraQueuedIntent {
            generation: self.generation,
            epoch: self.epoch,
            key,
            task_name,
            scope: self.scope.clone().expect("validated camera scope"),
            plan,
            edit_revision: self.edit_revision,
        };
        if self
            .current
            .values()
            .any(|current| same_queue(current, &intent))
        {
            self.queued.push_back(intent);
        } else {
            // enqueueTask invokes run immediately. Reserve current before
            // another caller in the same dispatch can enqueue the same name.
            let before = self.queued.len();
            self.queued.retain(|queued| !same_queue(queued, &intent));
            self.skipped_tasks += (before - self.queued.len()) as u64;
            self.ready.push_back(intent.generation);
            self.current.insert(intent.generation, intent);
        }
    }
    pub fn read(&mut self, plan: CameraSubmissionPlan) -> anyhow::Result<()> {
        anyhow::ensure!(
            plan.commands
                .iter()
                .all(|c| matches!(c, crate::camera_submission::CameraCommand::StandardRead(_))),
            "camera observation intent may contain only getters"
        );
        if self.pending() || self.has_waiting_intents() {
            return Ok(());
        }
        let scope = self
            .scope
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("camera has no active scope"))?;
        scope.validate()?;
        plan.validate(scope.product_id)?;
        self.enqueue("read".into(), plan);
        self.read_needed = false;
        self.read_retry_deferred = false;
        Ok(())
    }
    pub fn next(&mut self) -> Option<CameraQueuedIntent> {
        if let Some(id) = self.ready.pop_front() {
            return self.current.get(&id).cloned();
        }
        let first = self.queued.iter().position(|intent| {
            !self
                .current
                .values()
                .any(|current| same_queue(current, intent))
        })?;
        let selected = self.queued[first].clone();
        let latest = self
            .queued
            .iter()
            .rposition(|intent| same_queue(&selected, intent))?;
        let intent = self.queued.remove(latest)?;
        let before = self.queued.len();
        self.queued.retain(|queued| !same_queue(queued, &intent));
        self.skipped_tasks += (before - self.queued.len()) as u64;
        self.current.insert(intent.generation, intent.clone());
        Some(intent)
    }
    pub fn finish(
        &mut self,
        intent: &CameraQueuedIntent,
        result: Result<CameraSubmissionProgress, String>,
    ) -> bool {
        if self.current.remove(&intent.generation).is_none() {
            return false;
        }
        let current = intent.epoch == self.epoch
            && self.scope.as_ref() == Some(&intent.scope)
            && (intent.key != "read" || intent.edit_revision == self.edit_revision);
        if current {
            match result {
                Ok(p) => {
                    if intent.key == "read" {
                        self.observation_error = p.error.clone();
                        self.read_needed = !p.completed || p.error.is_some();
                        self.read_retry_deferred = self.read_needed;
                    } else {
                        self.error = p.error.clone();
                        self.last_edit_progress = Some(p.clone());
                        self.read_needed = true;
                        self.read_retry_deferred = false;
                    }
                    self.last_progress = Some(p);
                }
                Err(e) => {
                    if intent.key == "read" {
                        self.observation_error = Some(e);
                        self.read_needed = true;
                        self.read_retry_deferred = true;
                    } else {
                        self.error = Some(e);
                        self.read_needed = true;
                        self.read_retry_deferred = false;
                    }
                }
            }
        }
        current
    }
}
fn same_queue(a: &CameraQueuedIntent, b: &CameraQueuedIntent) -> bool {
    a.task_name == b.task_name
        && a.scope
            .container_id
            .eq_ignore_ascii_case(&b.scope.container_id)
}

/// Merge observations for submission companions without changing local storage.
pub fn overlay_camera_observations(draft: &Value, observations: &BTreeMap<String, Value>) -> Value {
    let mut profile = draft.clone();
    for (path, value) in observations {
        if let Some(field) = profile.pointer_mut(path) {
            *field = value.clone();
        }
    }
    profile
}
/// Moving from a predefined image mode into Custom retains the current actual
/// companion values. A later explicit preset selection discards these overlays.
pub fn relocate_camera_image_observations(
    observations: &mut BTreeMap<String, Value>,
    from: usize,
    to: usize,
) {
    let prefix = format!("/image/{from}/dataSet/");
    let selected: Vec<_> = observations
        .iter()
        .filter_map(|(p, v)| {
            p.strip_prefix(&prefix).map(|suffix| {
                (
                    p.clone(),
                    format!("/image/{to}/dataSet/{suffix}"),
                    v.clone(),
                )
            })
        })
        .collect();
    for (old, new, value) in selected {
        observations.remove(&old);
        observations.insert(new, value);
    }
}
/// Clear the edited fields before compiling a compound caller. Untouched
/// observed fields continue to supply its parameters instead of stale defaults.
pub fn invalidate_camera_edit_observations(
    observations: &mut BTreeMap<String, Value>,
    key: &str,
    path: Option<&str>,
) {
    let name = key.split_once(':').map_or(key, |(_, n)| n);
    match name {
        "image-preset" => observations.retain(|p, _| !p.starts_with("/image/")),
        "view-preset" => observations.retain(|p, _| !p.starts_with("/camera/viewPresets/data/")),
        _ => {
            if let Some(path) = path {
                observations.remove(path);
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::{
        CameraSetting, StandardCameraProperty, StandardCameraSetting, VideoProcAmpProperty,
    };
    use crate::camera_submission::CameraCommand;
    fn scope() -> CameraScope {
        CameraScope {
            product_id: 3594,
            container_id: "{01234567-89ab-cdef-0123-456789abcdef}".into(),
        }
    }
    fn plan(value: i32) -> CameraSubmissionPlan {
        CameraSubmissionPlan {
            commands: vec![CameraCommand::StandardWrite(StandardCameraSetting {
                property: StandardCameraProperty::VideoProcAmp(VideoProcAmpProperty::Brightness),
                setting: CameraSetting {
                    value,
                    is_auto: false,
                },
            })],
        }
    }
    fn queue() -> CameraIntentQueue {
        let mut q = CameraIntentQueue::default();
        q.set_scope(Some(scope()));
        q
    }
    fn read_plan() -> CameraSubmissionPlan {
        CameraSubmissionPlan {
            commands: vec![CameraCommand::StandardRead(
                StandardCameraProperty::VideoProcAmp(VideoProcAmpProperty::Brightness),
            )],
        }
    }
    fn reset_plan() -> CameraSubmissionPlan {
        crate::camera_submission::compile_camera_edit(3594, "reset-exposure",
            &serde_json::json!({"camera":{"autoExposure":{"isEnabled":true,"metering":5,"compensation":0}}}))
            .unwrap()
    }
    #[test]
    fn camera_reset_tasks_keep_current_companions_and_reject_stale_scope_results() {
        let mut q = queue();
        q.edit("3594:reset-exposure".into(), reset_plan()).unwrap();
        let pending = q.next().unwrap();
        assert_eq!(pending.key, "3594:auto-exposure");
        let meter = CameraSubmissionPlan {
            commands: vec![CameraCommand::Extension(
                crate::camera_extensions::CameraExtensionRequest::SetAutoExposureMetering {
                    mode: 0,
                },
            )],
        };
        q.edit("3594:metering".into(), meter.clone()).unwrap();
        q.finish(&pending, Err("simulated auto-exposure failure".into()));
        let original_meter = q.next().unwrap();
        assert_eq!(original_meter.key, "3594:metering");
        let compensation = q.next().unwrap();
        assert_eq!(compensation.key, "3594:compensation");
        q.finish(&compensation, Err("simulated compensation failure".into()));
        q.finish(
            &original_meter,
            Err("simulated original metering failure".into()),
        );
        let updated = q.next().unwrap();
        assert_eq!(updated.plan, meter);
        q.finish(&updated, Err("simulated metering failure".into()));
        assert!(q.next().is_none());

        q.edit("3594:reset-exposure".into(), reset_plan()).unwrap();
        let old = q.next().unwrap();
        q.invalidate();
        assert!(!q.finish(&old, Err("old task canceled".into())));
        while let Some(dispatched) = q.next() {
            assert!(!q.finish(&dispatched, Ok(Default::default())));
        }
    }
    #[test]
    fn malformed_reset_task_groups_do_not_partially_enqueue() {
        let mut q = queue();
        q.edit("brightness".into(), plan(17)).unwrap();
        let mut reset = reset_plan();
        reset.commands.swap(1, 2);
        assert!(q.edit("3594:reset-exposure".into(), reset).is_err());
        let existing = q.next().unwrap();
        assert_eq!(existing.key, "brightness");
        q.finish(&existing, Err("simulated failure".into()));
        assert!(q.next().is_none());
    }
    #[test]
    fn camera_queue_keeps_inflight_slot_across_scope_change() {
        let mut q = queue();
        q.edit("brightness".into(), plan(1)).unwrap();
        let p = q.next().unwrap();
        q.set_scope(None);
        q.set_scope(Some(scope()));
        q.edit("brightness".into(), plan(2)).unwrap();
        assert!(q.next().is_none());
        assert!(!q.finish(
            &p,
            Ok(CameraSubmissionProgress {
                completed: true,
                ..Default::default()
            })
        ));
        assert!(q.last_progress.is_none());
        assert_eq!(q.next().unwrap().plan, plan(2));
    }
    #[test]
    fn camera_queue_coalesces_only_same_waiting_setting() {
        let mut q = queue();
        q.edit("brightness".into(), plan(1)).unwrap();
        let p = q.next().unwrap();
        q.edit("zoom".into(), plan(2)).unwrap();
        q.edit("brightness".into(), plan(3)).unwrap();
        q.edit("brightness".into(), plan(4)).unwrap();
        assert_eq!(q.queued.len(), 2);
        let contrast = q.next().unwrap();
        assert_eq!(contrast.key, "zoom");
        assert!(q.next().is_none());
        q.finish(&p, Ok(Default::default()));
        let next = q.next().unwrap();
        assert_eq!(next.plan, plan(4));
        assert_eq!(q.skipped_tasks, 1);
        q.finish(&contrast, Ok(Default::default()));
    }
    #[test]
    fn camera_queue_initial_read_cannot_publish_over_a_later_edit() {
        let mut q = queue();
        q.read(read_plan()).unwrap();
        let read = q.next().unwrap();
        q.edit("brightness".into(), plan(2)).unwrap();
        assert!(!q.finish(
            &read,
            Ok(CameraSubmissionProgress {
                completed: true,
                ..Default::default()
            })
        ));
        assert!(q.last_progress.is_none());
        assert!(q.read_needed());
        assert!(q.continue_after_completion());
        assert_eq!(q.next().unwrap().key, "brightness");
    }
    #[test]
    fn camera_queue_no_hardware_scope_never_submits_a_local_draft() {
        let mut q = CameraIntentQueue::default();
        assert!(q.edit("brightness".into(), plan(1)).is_err());
        assert!(q.next().is_none());
    }
    #[test]
    fn camera_queue_named_current_starts_at_enqueue_and_pending_only_skips_after_settle() {
        let mut q = queue();
        q.edit("autofocus".into(), plan(1)).unwrap();
        q.edit("focus".into(), plan(2)).unwrap();
        q.edit("focus-tracking".into(), plan(3)).unwrap();
        q.edit("zoom".into(), plan(4)).unwrap();
        let focus = q.next().unwrap();
        assert_eq!(focus.task_name, "autoFocus");
        assert_eq!(focus.plan, plan(1));
        let zoom = q.next().unwrap();
        assert_eq!(zoom.task_name, "zoom");
        assert!(q.next().is_none());
        assert!(q.finish(&focus, Err("current runner failed".into())));
        let latest = q.next().unwrap();
        assert_eq!(latest.plan, plan(3));
        assert_eq!(q.skipped_tasks, 1);
        assert!(q.pending());
        q.finish(&zoom, Ok(Default::default()));
        assert!(q.pending());
        q.finish(&latest, Ok(Default::default()));
        assert!(!q.pending());
    }
    #[test]
    fn camera_queue_x_exposure_preserves_started_value_then_latest_shared_shutter() {
        let mut q = CameraIntentQueue::default();
        let mut target = scope();
        target.product_id = 3596;
        q.set_scope(Some(target));
        let profile = serde_json::json!({"camera":{"autoExposure":{"isEnabled":false,"exposureValue":-7,"shutterSpeed":16666}}});
        let plan =
            crate::camera_submission::compile_camera_edit(3596, "auto-exposure", &profile).unwrap();
        q.edit("auto-exposure".into(), plan.clone()).unwrap();
        let mode = q.next().unwrap();
        let value = q.next().unwrap();
        assert_eq!(mode.task_name, "autoExposure");
        assert_eq!(value.task_name, "shutterSpeed");
        assert_eq!(value.plan.commands, vec![plan.commands[1].clone()]);
        q.edit(
            "manual-exposure".into(),
            crate::camera_submission::compile_camera_edit(3596, "manual-exposure", &profile)
                .unwrap(),
        )
        .unwrap();
        q.finish(&value, Err("value setter failed".into()));
        let latest = q.next().unwrap();
        assert_eq!(latest.plan.commands, value.plan.commands);
        assert_eq!(q.skipped_tasks, 1);
        assert!(q.current.contains_key(&mode.generation));
    }
    #[test]
    fn camera_queue_page_invalidation_keeps_dispatched_edits_without_publishing_stale_results() {
        let mut q = queue();
        q.edit("focus".into(), plan(1)).unwrap();
        let current = q.next().unwrap();
        q.edit("focus".into(), plan(2)).unwrap();
        q.invalidate();
        assert!(!q.finish(&current, Ok(Default::default())));
        let queued = q.next().unwrap();
        assert_eq!(queued.plan, plan(2));
        assert!(!q.finish(&queued, Ok(Default::default())));
        assert!(q.last_progress.is_none());
    }
    #[test]
    fn camera_queue_old_duplicate_completion_cannot_release_new_request() {
        let mut q = queue();
        q.edit("brightness".into(), plan(1)).unwrap();
        let old = q.next().unwrap();
        q.finish(&old, Ok(Default::default()));
        q.edit("brightness".into(), plan(2)).unwrap();
        let current = q.next().unwrap();
        assert!(!q.finish(&old, Ok(Default::default())));
        assert!(q.pending());
        assert!(q.finish(&current, Ok(Default::default())));
    }
    #[test]
    fn camera_queue_coalescing_keeps_the_latest_action_after_a_compound_preset() {
        let mut q = queue();
        q.edit("brightness".into(), plan(1)).unwrap();
        q.edit("image-preset".into(), plan(2)).unwrap();
        q.edit("brightness".into(), plan(3)).unwrap();
        let first = q.next().unwrap();
        assert_eq!(first.plan, plan(1));
        assert!(q.next().is_none());
        q.finish(&first, Ok(Default::default()));
        let preset = q.next().unwrap();
        // All image controls use the same actual source imagePreset name.
        assert_eq!(preset.key, "brightness");
        assert_eq!(preset.plan, plan(3));
        assert_eq!(q.skipped_tasks, 1);
        q.finish(&preset, Ok(Default::default()));
        assert!(q.next().is_none());
    }
    #[test]
    fn camera_queue_refresh_cannot_erase_a_partial_submission_error() {
        let mut q = queue();
        q.edit("brightness".into(), plan(1)).unwrap();
        let edit = q.next().unwrap();
        q.finish(
            &edit,
            Ok(CameraSubmissionProgress {
                error: Some("write response was lost".into()),
                ..Default::default()
            }),
        );
        q.read(read_plan()).unwrap();
        let read = q.next().unwrap();
        q.finish(
            &read,
            Ok(CameraSubmissionProgress {
                completed: true,
                ..Default::default()
            }),
        );
        assert_eq!(q.error.as_deref(), Some("write response was lost"));
        assert!(!q.last_edit_progress.as_ref().unwrap().completed);
        assert!(q.last_progress.as_ref().unwrap().completed);
    }
    #[test]
    fn camera_queue_getter_lane_rejects_writes() {
        let mut q = queue();
        assert!(q.read(plan(1)).is_err());
        assert!(q.read_needed());
        assert!(q.next().is_none());
    }
    #[test]
    fn camera_queue_failed_read_retains_refresh_without_immediate_retry() {
        let mut q = queue();
        q.read(read_plan()).unwrap();
        let read = q.next().unwrap();
        assert!(!q.read_needed());
        assert!(q.finish(&read, Err("simulated worker disconnected".into())));
        assert!(q.read_needed());
        assert!(!q.continue_after_completion());
        assert!(!q.pending());
        assert!(q.next().is_none());
        assert_eq!(
            q.observation_error.as_deref(),
            Some("simulated worker disconnected")
        );
        // A subsequent page/scope event can explicitly pump a getter again.
        q.read(read_plan()).unwrap();
        let retry = q.next().unwrap();
        assert!(q.finish(
            &retry,
            Ok(CameraSubmissionProgress {
                completed: true,
                ..Default::default()
            })
        ));
        assert!(!q.read_needed());
        assert!(q.continue_after_completion());
        assert!(q.observation_error.is_none());
    }
    #[test]
    fn camera_queue_partial_or_incomplete_getter_keeps_refresh_pending() {
        let observed = crate::camera::StandardCameraRead {
            property: StandardCameraProperty::VideoProcAmp(VideoProcAmpProperty::Brightness),
            observation: crate::camera::CameraLongObservation::from_native(17, 2, 0).unwrap(),
        };
        let remaining = CameraCommand::StandardRead(StandardCameraProperty::VideoProcAmp(
            VideoProcAmpProperty::Contrast,
        ));
        for progress in [
            CameraSubmissionProgress::default(),
            CameraSubmissionProgress {
                outcomes: vec![crate::camera_submission::CameraCommandOutcome::Read(
                    observed,
                )],
                unconfirmed: Some(remaining.clone()),
                error: Some("simulated getter response loss".into()),
                ..Default::default()
            },
        ] {
            let mut q = queue();
            let mut plan = read_plan();
            plan.commands.push(remaining.clone());
            q.read(plan).unwrap();
            let read = q.next().unwrap();
            assert!(q.finish(&read, Ok(progress.clone())));
            assert!(q.read_needed());
            assert!(!q.continue_after_completion());
            assert_eq!(q.last_progress.as_ref(), Some(&progress));
            assert!(!progress.completed);
        }
    }
    #[test]
    fn camera_queue_stale_getter_failure_cannot_block_new_scope_refresh() {
        let mut q = queue();
        q.read(read_plan()).unwrap();
        let old = q.next().unwrap();
        let mut new_scope = scope();
        new_scope.container_id = "{11234567-89ab-cdef-0123-456789abcdef}".into();
        q.set_scope(Some(new_scope.clone()));
        assert!(q.read_needed());
        assert!(!q.finish(&old, Err("old device disappeared".into())));
        assert!(q.read_needed());
        assert!(q.continue_after_completion());
        assert!(q.observation_error.is_none());
        q.read(read_plan()).unwrap();
        let current = q.next().unwrap();
        assert_eq!(current.scope, new_scope);
        assert!(!q.finish(&old, Err("duplicate old failure".into())));
        assert!(q.pending());
        assert!(!q.read_needed());
        assert!(q.observation_error.is_none());
    }
    #[test]
    fn camera_queue_stale_page_getter_cannot_clear_current_refresh() {
        let mut q = queue();
        q.read(read_plan()).unwrap();
        let old_page = q.next().unwrap();
        // Page/profile changes invalidate the epoch while retaining hardware.
        q.invalidate();
        assert!(!q.finish(
            &old_page,
            Ok(CameraSubmissionProgress {
                completed: true,
                ..Default::default()
            })
        ));
        assert!(q.read_needed());
        assert!(q.continue_after_completion());
        assert!(q.last_progress.is_none());
        q.read(read_plan()).unwrap();
        assert!(q.next().unwrap().epoch != old_page.epoch);
    }
    #[test]
    fn camera_queue_edit_after_getter_failure_runs_before_independent_refresh() {
        let mut q = queue();
        q.read(read_plan()).unwrap();
        let read = q.next().unwrap();
        q.finish(&read, Err("simulated getter failure".into()));
        q.edit("brightness".into(), plan(27)).unwrap();
        assert!(q.continue_after_completion());
        let edit = q.next().unwrap();
        assert_eq!(edit.key, "brightness");
        assert_eq!(edit.plan, plan(27));
        q.finish(&edit, Err("simulated lost write response".into()));
        assert!(q.read_needed());
        assert!(q.continue_after_completion());
        q.read(read_plan()).unwrap();
        assert_eq!(q.next().unwrap().key, "read");
        assert_eq!(q.error.as_deref(), Some("simulated lost write response"));
    }
    #[test]
    fn camera_observation_companion_uses_actual_white_balance_without_persisting_it() {
        let draft = serde_json::json!({"imageMode":0,"image":[{"dataSet":{"whiteBalance":{"value":3000,"isAuto":false}}}]});
        let mut actual = BTreeMap::from([
            (
                "/image/0/dataSet/whiteBalance/value".into(),
                Value::from(4700),
            ),
            (
                "/image/0/dataSet/whiteBalance/isAuto".into(),
                Value::Bool(true),
            ),
        ]);
        invalidate_camera_edit_observations(
            &mut actual,
            "IMAGE:auto-white-balance",
            Some("/image/0/dataSet/whiteBalance/isAuto"),
        );
        let p = overlay_camera_observations(&draft, &actual);
        let plan =
            crate::camera_submission::compile_camera_edit(3594, "IMAGE:auto-white-balance", &p)
                .unwrap();
        let CameraCommand::StandardWrite(write) = &plan.commands[0] else {
            panic!("expected white balance")
        };
        assert_eq!(
            write.setting,
            CameraSetting {
                value: 4700,
                is_auto: false
            }
        );
        assert_eq!(
            draft.pointer("/image/0/dataSet/whiteBalance/value"),
            Some(&Value::from(3000))
        );
    }
    #[test]
    fn camera_observation_custom_transition_preserves_untouched_actual_fields() {
        let mut actual = BTreeMap::from([
            ("/image/0/dataSet/brightness".into(), Value::from(10)),
            ("/image/0/dataSet/contrast".into(), Value::from(20)),
        ]);
        relocate_camera_image_observations(&mut actual, 0, 1);
        invalidate_camera_edit_observations(
            &mut actual,
            "IMAGE:brightness",
            Some("/image/1/dataSet/brightness"),
        );
        let draft = serde_json::json!({"image":[{"dataSet":{"brightness":1,"contrast":2}},{"dataSet":{"brightness":30,"contrast":2}}]});
        let p = overlay_camera_observations(&draft, &actual);
        assert_eq!(
            p.pointer("/image/1/dataSet/brightness"),
            Some(&Value::from(30))
        );
        assert_eq!(
            p.pointer("/image/1/dataSet/contrast"),
            Some(&Value::from(20))
        );
        assert_eq!(
            draft.pointer("/image/1/dataSet/contrast"),
            Some(&Value::from(2))
        );
        invalidate_camera_edit_observations(&mut actual, "IMAGE:image-preset", None);
        assert!(actual.is_empty());
    }
}
