//! Retained current PID70 native mapping owner. Publication acknowledgements
//! and callback diagnostics never replace the optimistic source profile.
use super::mapping_callbacks_current::{self as callbacks, Callback, ScanCode};
use super::*;
use razer_device::mouse_current_dpi::DpiPair;
use razer_service::mapping::device::{Reply, Session};
use std::time::Duration;
impl EventEmitter<crate::features::MappingCallbackRequested> for MouseProductWorkspace {}

#[derive(Default)]
pub(super) struct State {
    owner: Option<Session>,
    stopping: bool,
    callbacks_closed: bool,
    task: Option<Task<()>>,
    container: String,
    connection_generation: u64,
    epoch: u64,
    revision: u64,
    published: Option<MousePollingScope>,
    requested: Option<MousePollingScope>,
    diagnostic: Option<String>,
    clutches: Vec<(ScanCode, (u16, u16))>,
    otfs: bool,
    native_otfs: Option<bool>,
    inter_otfs: bool,
    inter_otfs_applied: bool,
    pending_mode: Option<bool>,
    document: Option<Value>,
}
impl State {
    pub(super) fn stop(&mut self) {
        self.owner = None;
        self.stopping = false;
        self.callbacks_closed = false;
        self.task = None;
        self.epoch = self.epoch.wrapping_add(1);
        self.published = None;
        self.requested = None;
        self.container.clear();
        self.clutches.clear();
        self.otfs = false;
        self.native_otfs = None;
        self.inter_otfs = false;
        self.inter_otfs_applied = false;
        self.pending_mode = None;
        self.document = None;
    }

    fn accepts_events(&self, epoch: u64, scope: Option<MousePollingScope>) -> bool {
        self.epoch == epoch
            && !self.stopping
            && !self.callbacks_closed
            && self.published.is_some()
            && self.published == scope
    }

    pub(super) fn needs_dpi_restore(&self) -> bool {
        !self.clutches.is_empty()
    }
    pub(super) fn close_callback_admission(&mut self) {
        self.callbacks_closed = true;
    }
    pub(super) fn finish_dpi_restore(&mut self) {
        self.clutches.clear();
    }
}

impl MouseProductWorkspace {
    pub fn set_legacy_mapping_connection_generation(
        &mut self,
        container: Option<String>,
        generation: u64,
    ) {
        if self.spec.product_id != 70 {
            return;
        }
        self.legacy_dpi
            .observe_mapping_connection(container.as_ref().map(|container| {
                callbacks::Connection {
                    container: container.clone(),
                    generation,
                }
            }));
        if self.legacy_mapping.container.as_str() != container.as_deref().unwrap_or("")
            || self.legacy_mapping.connection_generation != generation
        {
            self.legacy_mapping.stop();
            self.legacy_mapping.container = container.unwrap_or_default();
            self.legacy_mapping.connection_generation = generation;
            self.dpi_editing_observed = None;
        }
    }

    pub fn cancel_mapping_service_exit(&mut self) {
        self.legacy_mapping.stop();
        self.receiver_mapping.stop();
    }

    pub fn finish_mapping_service(&mut self) {
        self.receiver_mapping.finish();
        self.legacy_mapping.stopping = true;
        if let Some(owner) = &self.legacy_mapping.owner {
            if let Err(error) = owner.request_stop_after_pending() {
                self.legacy_mapping.diagnostic = Some(error);
            }
        }
    }

    pub fn publish_legacy_mapping(
        &mut self,
        container: String,
        document: Value,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id != 70 || self.legacy_mapping.stopping {
            return;
        }
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        if !self.legacy_mapping.container.is_empty() && self.legacy_mapping.container != container {
            self.legacy_mapping.stop();
        }
        if self
            .legacy_mapping
            .published
            .or(self.legacy_mapping.requested)
            .is_some_and(|accepted| accepted != scope)
        {
            self.legacy_mapping.published = None;
        }
        if self.legacy_mapping.owner.is_none() {
            let (owner, replies) = match Session::start(70, container.clone()) {
                Ok(owner) => owner,
                Err(error) => {
                    self.legacy_mapping.diagnostic = Some(format!("{error:#}"));
                    return;
                }
            };
            self.legacy_mapping.container = container;
            self.legacy_mapping.epoch = self.legacy_mapping.epoch.wrapping_add(1);
            let epoch = self.legacy_mapping.epoch;
            self.legacy_mapping.owner = Some(owner);
            self.legacy_mapping.task = Some(cx.spawn(async move |this, cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(50))
                        .await;
                    let mut batch = Vec::new();
                    let disconnected = loop {
                        match replies.try_recv() {
                            Ok(reply) => batch.push(reply),
                            Err(std::sync::mpsc::TryRecvError::Empty) => break false,
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => break true,
                        }
                    };
                    let mut stopped = false;
                    if this
                        .update(cx, |this, cx| {
                            if this.legacy_mapping.epoch != epoch { stopped = true; return; }
                            for reply in batch {
                                match reply {
                                    Reply::Ready => {}
                                    Reply::Published {
                                        revision, result, ..
                                    } => {
                                        if revision == this.legacy_mapping.revision
                                            && this.legacy_mapping.requested == this.mouse_polling_scope(cx) {
                                            match result {
                                                Ok(()) => {
                                                    this.legacy_mapping.published = this.legacy_mapping.requested.take();
                                                    this.legacy_mapping.diagnostic = None;
                                                    if let Some(enabled) =
                                                        this.legacy_mapping.pending_mode.take()
                                                    {
                                                        let changed = this.legacy_mapping.inter_otfs_applied != enabled;
                                                        this.legacy_mapping.inter_otfs_applied =
                                                            enabled;
                                                        this.legacy_mapping.otfs =
                                                            this.legacy_mapping.native_otfs == Some(true)
                                                                || enabled;
                                                        if changed {
                                                            this.dpi_editing_observed = Some(
                                                                !this.legacy_mapping.otfs
                                                                    && this.legacy_mapping.clutches.is_empty(),
                                                            );
                                                        }
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
                                        for event in events {
                                            if !this.legacy_mapping.accepts_events(epoch, this.mouse_polling_scope(cx))
                                                || !this.legacy_mapping_matches_profile() { break; }
                                            this.legacy_mapping_event(event, cx);
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
                            if disconnected && !stopped {
                                this.legacy_mapping.owner = None;
                                this.legacy_mapping.published = None;
                                this.legacy_mapping.diagnostic = Some("Mapping reply channel disconnected before cleanup confirmation".into());
                                stopped = true;
                            }
                            cx.notify();
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
        match callbacks::parse(event, self.spec.min_dpi as u16, self.spec.max_dpi as u16) {
            Ok(callback) => self.apply_legacy_mapping_callback(callback, cx),
            Err(error) => {
                self.legacy_mapping.diagnostic = Some(format!("Mapping callback: {error:#}"));
                cx.notify();
            }
        }
    }

    fn legacy_mapping_matches_profile(&self) -> bool {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct DocumentIdentity {
            active_profile: String,
        }
        #[derive(Deserialize)]
        struct ProfileIdentity {
            guid: String,
        }
        self.legacy_mapping.document.as_ref().and_then(|document| {
            let document: DocumentIdentity = serde_json::from_value(document.clone()).ok()?;
            let profile: ProfileIdentity = serde_json::from_value(self.draft.clone()).ok()?;
            Some(document.active_profile == profile.guid)
        }) == Some(true)
    }

    fn apply_legacy_mapping_callback(&mut self, callback: Callback, cx: &mut Context<Self>) {
        let generation = self.legacy_mapping.connection_generation;
        match callback {
            Callback::Mode(enabled)
            | Callback::Otfs {
                enabled,
                interdevice: false,
            } => {
                self.legacy_mapping.native_otfs = Some(enabled);
                self.legacy_mapping.otfs = enabled || self.legacy_mapping.inter_otfs_applied;
                self.dpi_editing_observed =
                    Some(!self.legacy_mapping.otfs && self.legacy_mapping.clutches.is_empty());
            }
            Callback::Stage(action) => self.legacy_stage_event(action.assignment(), cx),
            Callback::ClutchDown { key, dpi } => self.legacy_clutch_event(key, Some(dpi), cx),
            Callback::ClutchUp(key) => self.legacy_clutch_event(key, None, cx),
            Callback::Value { up } => self.legacy_otfs_value(up, cx),
            Callback::Otfs {
                enabled,
                interdevice: true,
            } => self.set_interdevice_otfs(enabled, cx),
            Callback::Navigate { output, lightpac } => {
                if let Some(scope) = self.mouse_polling_scope(cx) {
                    cx.emit(MouseLegacyProfileNavigate { scope, output });
                }
                if let Some(raw) = lightpac {
                    cx.emit(crate::features::MappingCallbackRequested::new_scoped(
                        raw, generation,
                    ));
                }
            }
            Callback::Forward(raw) => cx.emit(
                crate::features::MappingCallbackRequested::new_scoped(raw, generation),
            ),
            Callback::Ignored => return,
        }
        cx.notify();
    }
    pub fn consume_interdevice_mapping(&mut self, raw: Value, cx: &mut Context<Self>) -> bool {
        if self.spec.product_id != 70
            || self.legacy_mapping.stopping
            || self.legacy_mapping.callbacks_closed
        {
            return false;
        }
        match callbacks::parse_raw(raw, self.spec.min_dpi as u16, self.spec.max_dpi as u16) {
            Ok(callback) => {
                // The source ready observer retains held OTFS for an owner
                // whose ordinary document has not been published yet.
                if self.legacy_mapping.owner.is_none() && !matches!(callback, Callback::Otfs { .. })
                {
                    return false;
                }
                self.apply_legacy_mapping_callback(callback, cx);
                true
            }
            Err(error) => {
                self.legacy_mapping.diagnostic = Some(format!("Interdevice callback: {error:#}"));
                cx.notify();
                false
            }
        }
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

    fn legacy_clutch_event(
        &mut self,
        scancode: ScanCode,
        dpi: Option<DpiPair>,
        cx: &mut Context<Self>,
    ) {
        if let Some(dpi) = dpi {
            let dpi = (dpi.x, dpi.y);
            self.legacy_mapping.clutches.push((scancode, dpi));
            self.dpi_editing_observed = Some(false);
            self.submit_legacy_temporary_dpi(dpi, false, cx);
        } else {
            let stack = &mut self.legacy_mapping.clutches;
            if stack.is_empty() {
                return;
            }
            let index = stack.iter().position(|(key, _)| key == &scancode);
            let should_restore = stack.len() == 1 || index == Some(stack.len() - 1);
            let index = index.unwrap_or(stack.len() - 1);
            stack.remove(index);
            let restores_profile = stack.is_empty();
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
                    self.submit_legacy_temporary_dpi(dpi, restores_profile, cx);
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
