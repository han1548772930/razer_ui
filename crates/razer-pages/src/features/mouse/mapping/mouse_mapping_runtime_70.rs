//! Retained current PID70 native mapping owner. Publication acknowledgements
//! and callback diagnostics never replace the optimistic source profile.
use super::*;
use crate::features::mapping_device_runtime::{Owner, Reply};
use std::time::Duration;
impl EventEmitter<crate::features::MappingCallbackRequested> for MouseProductWorkspace {}

#[derive(Default)]
pub(super) struct State {
    owner: Option<Owner>,
    task: Option<Task<()>>,
    container: String,
    revision: u64,
    published: Option<MousePollingScope>,
    requested: Option<MousePollingScope>,
    diagnostic: Option<String>,
    clutches: Vec<(Value, (u16, u16))>,
    otfs: bool,
    native_otfs: bool,
    inter_otfs: bool,
    inter_otfs_applied: bool,
    pending_mode: Option<bool>,
    document: Option<Value>,
}
impl State {
    pub(super) fn stop(&mut self) {
        self.owner = None;
        self.task = None;
        self.published = None;
        self.requested = None;
        self.container.clear();
        self.clutches.clear();
        self.otfs = false;
        self.native_otfs = false;
        self.inter_otfs = false;
        self.inter_otfs_applied = false;
        self.pending_mode = None;
        self.document = None;
    }
}

impl MouseProductWorkspace {
    pub fn publish_legacy_mapping(
        &mut self,
        container: String,
        document: Value,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id != 70 {
            return;
        }
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        if !self.legacy_mapping.container.is_empty() && self.legacy_mapping.container != container {
            self.legacy_mapping.stop();
        }
        if self.legacy_mapping.owner.is_none() {
            let (owner, replies) = match Owner::start(70, container.clone()) {
                Ok(owner) => owner,
                Err(error) => {
                    self.legacy_mapping.diagnostic = Some(format!("{error:#}"));
                    return;
                }
            };
            self.legacy_mapping.container = container;
            self.legacy_mapping.owner = Some(owner);
            self.legacy_mapping.task = Some(cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(50))
                        .await;
                    let replies: Vec<_> = replies.try_iter().collect();
                    let mut stopped = false;
                    if this
                        .update(cx, |this, cx| {
                            for reply in replies {
                                match reply {
                                    Reply::Ready => {}
                                    Reply::Published {
                                        revision, result, ..
                                    } => {
                                        if revision == this.legacy_mapping.revision {
                                            match result {
                                                Ok(()) => {
                                                    this.legacy_mapping.published =
                                                        this.legacy_mapping.requested;
                                                    this.legacy_mapping.diagnostic = None;
                                                    if let Some(enabled) =
                                                        this.legacy_mapping.pending_mode.take()
                                                    {
                                                        this.legacy_mapping.inter_otfs_applied =
                                                            enabled;
                                                        this.legacy_mapping.otfs =
                                                            this.legacy_mapping.native_otfs
                                                                || enabled;
                                                        this.dpi_editing_observed = Some(
                                                            !this.legacy_mapping.otfs
                                                                && this
                                                                    .legacy_mapping
                                                                    .clutches
                                                                    .is_empty(),
                                                        );
                                                        cx.notify();
                                                    }
                                                }
                                                Err(error) => {
                                                    this.legacy_mapping.published = None;
                                                    this.legacy_mapping.diagnostic = Some(error);
                                                    this.legacy_mapping.pending_mode = None;
                                                }
                                            }
                                        }
                                    }
                                    Reply::Events(events) => {
                                        if this.legacy_mapping.published
                                            == this.mouse_polling_scope(cx)
                                            && this.legacy_mapping.published.is_some()
                                        {
                                            for event in events {
                                                this.legacy_mapping_event(event, cx);
                                            }
                                        }
                                    }
                                    Reply::Failed(error) => {
                                        eprintln!("[mouse70-mapping] {error}");
                                        this.legacy_mapping.diagnostic = Some(error);
                                        this.legacy_mapping.owner = None;
                                        this.legacy_mapping.published = None;
                                        stopped = true;
                                    }
                                    Reply::Stopped(result) => {
                                        if let Err(error) = result {
                                            this.legacy_mapping.diagnostic = Some(error);
                                        }
                                        this.legacy_mapping.owner = None;
                                        this.legacy_mapping.published = None;
                                        stopped = true;
                                    }
                                }
                            }
                        })
                        .is_err()
                        || stopped
                    {
                        break;
                    }
                }
            }));
        }
        self.legacy_mapping.document = Some(document);
        self.publish_legacy_mapping_current(scope);
    }

    fn publish_legacy_mapping_current(&mut self, scope: MousePollingScope) {
        let Some(document) = self.legacy_mapping.document.as_ref() else {
            return;
        };
        let document = if self.legacy_mapping.inter_otfs {
            match razer_device::mouse_legacy_mapping::interdevice_otfs_document(document) {
                Ok(document) => document,
                Err(error) => {
                    self.legacy_mapping.diagnostic = Some(format!("{error:#}"));
                    return;
                }
            }
        } else {
            document.clone()
        };
        self.legacy_mapping.revision = self.legacy_mapping.revision.wrapping_add(1);
        self.legacy_mapping.requested = Some(scope);
        self.legacy_mapping.published = None;
        self.legacy_mapping.pending_mode = Some(self.legacy_mapping.inter_otfs);
        if let Some(owner) = &self.legacy_mapping.owner {
            if let Err(error) = owner.publish_with_restore(
                self.legacy_mapping.revision,
                document,
                None,
                self.legacy_mapping
                    .inter_otfs
                    .then(|| self.legacy_mapping.document.clone())
                    .flatten(),
            ) {
                self.legacy_mapping.diagnostic = Some(error);
                self.legacy_mapping.pending_mode = None;
            }
        }
    }

    fn legacy_mapping_event(&mut self, event: Value, cx: &mut Context<Self>) {
        let raw = &event["jsonEvent"];
        if event["source"] == "devicemodechanged" && event["eventType"] == 4 {
            let mode = event["event"]
                .as_str()
                .and_then(|text| serde_json::from_str::<Value>(text).ok())
                .unwrap_or_else(|| event["event"].clone());
            if let Some(enabled) = mode["isEnabled"].as_bool() {
                self.legacy_mapping.native_otfs = enabled;
                self.legacy_mapping.otfs = enabled || self.legacy_mapping.inter_otfs_applied;
                self.dpi_editing_observed =
                    Some(!self.legacy_mapping.otfs && self.legacy_mapping.clutches.is_empty());
                cx.notify();
            }
            return;
        }
        let output = match raw["output"].as_str() {
            Some(output) => match serde_json::from_str::<Value>(output) {
                Ok(output) => output,
                Err(_) => return,
            },
            None => return,
        };
        if matches!(output["type"].as_str(), Some("interdevice" | "lightpac")) {
            cx.emit(crate::features::MappingCallbackRequested::new(raw.clone()));
        } else if output["type"] == "sensitivity" {
            let assignment = output["sensitivityGroup"]["sensitivityAssignment"]
                .as_str()
                .unwrap_or_default();
            if matches!(
                assignment,
                "DPI_Up" | "DPI_Down" | "DPI_CycleUp" | "DPI_CycleDown"
            ) {
                self.legacy_stage_event(assignment, cx);
            } else if assignment == "DPI_Clutch" {
                let input = raw["input"]
                    .as_str()
                    .and_then(|text| serde_json::from_str::<Value>(text).ok())
                    .unwrap_or(Value::Null);
                self.legacy_clutch_event(&output, input["scancode"].clone(), cx);
            } else if matches!(assignment, "DPI_Value_Up" | "DPI_Value_Down") {
                self.legacy_otfs_value(assignment == "DPI_Value_Up", cx);
            } else if assignment == "DPI_OnTheFly" && raw["eventName"] == "interDevice" {
                self.set_interdevice_otfs(output["flag"] == 0, cx);
            }
        } else if matches!(
            output["type"].as_str(),
            Some("switchProfile" | "navigateProfile")
        ) {
            if let Some(scope) = self.mouse_polling_scope(cx) {
                cx.emit(MouseLegacyProfileNavigate { scope, output });
            }
        }
    }
    pub fn consume_interdevice_mapping(&mut self, raw: Value, cx: &mut Context<Self>) -> bool {
        if self.spec.product_id != 70 {
            return false;
        }
        let otfs = raw["output"]
            .as_str()
            .and_then(|value| serde_json::from_str::<Value>(value).ok())
            .is_some_and(|output| {
                output["type"] == "sensitivity"
                    && output["sensitivityGroup"]["sensitivityAssignment"] == "DPI_OnTheFly"
            });
        // R's ready observer replays held OTFS when the target becomes ready.
        // Retain that request until this target's ordinary document is available.
        if self.legacy_mapping.owner.is_none() && !otfs {
            return false;
        }
        self.legacy_mapping_event(json!({"jsonEvent":raw}), cx);
        true
    }

    fn set_interdevice_otfs(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if self.legacy_mapping.inter_otfs == enabled
            && (self.legacy_mapping.pending_mode.is_some()
                || self.legacy_mapping.inter_otfs_applied == enabled)
        {
            return;
        }
        self.legacy_mapping.inter_otfs = enabled;
        if self.legacy_mapping.owner.is_some() {
            if let Some(scope) = self.mouse_polling_scope(cx) {
                self.publish_legacy_mapping_current(scope);
            }
        }
    }

    fn legacy_clutch_event(&mut self, output: &Value, scancode: Value, cx: &mut Context<Self>) {
        if output["flag"] == 0 {
            let values = &output["sensitivityGroup"];
            let dpi = values["x"]
                .as_u64()
                .zip(values["y"].as_u64())
                .and_then(|(x, y)| Some((u16::try_from(x).ok()?, u16::try_from(y).ok()?)))
                .filter(|(x, y)| (100..=16000).contains(x) && (100..=16000).contains(y));
            let Some(dpi) = dpi else {
                return;
            };
            self.legacy_mapping.clutches.push((scancode, dpi));
            self.dpi_editing_observed = Some(false);
            self.submit_legacy_temporary_dpi(dpi, cx);
        } else {
            let stack = &mut self.legacy_mapping.clutches;
            if stack.is_empty() {
                return;
            }
            let index = stack.iter().position(|(key, _)| key == &scancode);
            let should_restore = stack.len() == 1 || index == Some(stack.len() - 1);
            let index = index.unwrap_or(stack.len() - 1);
            stack.remove(index);
            if should_restore {
                let dpi = stack.last().map(|(_, dpi)| *dpi).or_else(|| {
                    let stages = &self.draft["DPIStages"];
                    let index = stages["Active"].as_u64()?.checked_sub(1)? as usize;
                    let row = stages["Stages"]["DPIStage"].get(index)?;
                    Some((
                        u16::try_from(row["X"].as_u64()?).ok()?,
                        u16::try_from(row["Y"].as_u64()?).ok()?,
                    ))
                });
                if let Some(dpi) = dpi {
                    self.submit_legacy_temporary_dpi(dpi, cx);
                }
            }
            self.dpi_editing_observed =
                Some(!self.legacy_mapping.otfs && self.legacy_mapping.clutches.is_empty());
        }
        cx.notify();
    }

    fn legacy_otfs_value(&mut self, up: bool, cx: &mut Context<Self>) {
        // Current MW gt receives DeviceInfo.dpiStep=50, independently of the
        // text input's UI step=1. Both axes retain their difference.
        let Some(index) = self.draft["DPIStages"]["Active"]
            .as_u64()
            .and_then(|v| v.checked_sub(1))
        else {
            return;
        };
        let Some(row) = self.draft["DPIStages"]["Stages"]["DPIStage"].get_mut(index as usize)
        else {
            return;
        };
        let Some((x, y)) = row["X"].as_i64().zip(row["Y"].as_i64()) else {
            return;
        };
        let delta = if up { 50 } else { -50 };
        row["X"] = json!((x + delta).clamp(100, 16000));
        row["Y"] = json!((y + delta).clamp(100, 16000));
        self.legacy_dpi.refresh_controls = true;
        cx.emit(MouseProductChanged);
        self.submit_legacy_dpi_profile(true, cx);
        cx.notify();
    }

    /// Current MW yt preserves selection at boundaries and still persists and
    /// queues the active DPI. Disabled staging keeps its selected value.
    fn legacy_stage_event(&mut self, assignment: &str, cx: &mut Context<Self>) {
        let stages = &self.draft["DPIStages"];
        let active = stages["Active"].as_u64().unwrap_or(0);
        let Some(rows) = stages["Stages"]["DPIStage"].as_array() else {
            return;
        };
        let visible: Vec<_> = rows
            .iter()
            .enumerate()
            .filter(|(_, row)| row["Active"] == true)
            .map(|(index, _)| index as u64 + 1)
            .collect();
        let next = if stages["State"] == true {
            match assignment {
                "DPI_Up" => visible.iter().copied().find(|index| *index > active),
                "DPI_Down" => visible.iter().rev().copied().find(|index| *index < active),
                "DPI_CycleUp" => visible
                    .iter()
                    .copied()
                    .find(|index| *index > active)
                    .or(visible.first().copied()),
                "DPI_CycleDown" => visible
                    .iter()
                    .rev()
                    .copied()
                    .find(|index| *index < active)
                    .or(visible.last().copied()),
                _ => None,
            }
            .unwrap_or(active)
        } else {
            active
        };
        self.draft["DPIStages"]["Active"] = json!(next);
        cx.emit(MouseProductChanged);
        self.submit_legacy_dpi_profile(true, cx);
        cx.notify();
    }
}
