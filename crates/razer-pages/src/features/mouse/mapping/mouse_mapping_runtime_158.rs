//! PID158 software publication and callback lifetime. A native publication
//! receipt never acknowledges an onboard profile write or supplies device DPI.
use super::*;
use razer_device::mouse_dpi_stages::DpiStagesDraft;
use razer_service::mapping::device::{Reply, Session};
use std::time::Duration;

use super::mapping_callbacks_current as callbacks;
use callbacks::{Callback, ClutchRelease, Connection, DpiRuntime};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Publication {
    revision: u64,
    scope: MousePollingScope,
    inter_otfs: bool,
}

#[derive(Default)]
pub(super) struct State {
    owner: Option<Session>,
    pump: Option<Task<()>>,
    connection: Option<Connection>,
    epoch: u64,
    revision: u64,
    requested: Option<Publication>,
    published: Option<Publication>,
    document: Option<Value>,
    profile_guid: Option<String>,
    dpi: DpiRuntime,
    stopping: bool,
    callbacks_closed: bool,
    pub(super) error: Option<String>,
}

impl State {
    pub(super) fn stop(&mut self) {
        // Session cancellation is nonblocking. The worker keeps its target
        // lifecycle lock until restoration, removal and transport release end.
        self.owner = None;
        self.pump = None;
        self.epoch = self.epoch.wrapping_add(1);
        self.connection = None;
        self.requested = None;
        self.published = None;
        self.document = None;
        self.profile_guid = None;
        self.dpi = DpiRuntime::default();
        self.stopping = false;
        self.callbacks_closed = false;
    }

    fn install_connection(&mut self, next: Option<Connection>) -> bool {
        if self.connection == next {
            return false;
        }
        self.stop();
        self.connection = next;
        self.error = None;
        true
    }

    pub(super) fn finish(&mut self) {
        self.stopping = true;
        if let Some(owner) = &self.owner {
            if let Err(error) = owner.request_stop_after_pending() {
                self.error = Some(error);
            }
        }
    }

    pub(super) fn close_callback_admission(&mut self) {
        self.callbacks_closed = true;
    }

    fn admitted(&self, document: &Value, scope: MousePollingScope) -> bool {
        self.document.as_ref() == Some(document)
            && self.error.is_none()
            && self.requested.or(self.published).is_some_and(|request| {
                request.scope == scope && request.inter_otfs == self.dpi.inter_otfs_requested
            })
    }

    fn acknowledge(
        &mut self,
        revision: u64,
        result: Result<(), String>,
        scope: Option<MousePollingScope>,
    ) {
        let Some(requested) = self
            .requested
            .filter(|request| request.revision == revision)
        else {
            return;
        };
        self.requested = None;
        if Some(requested.scope) != scope {
            return;
        }
        match result {
            Ok(()) => {
                self.published = Some(requested);
                self.dpi.inter_otfs_applied = requested.inter_otfs;
                self.error = None;
            }
            Err(error) => self.fail(error),
        }
    }

    fn fail(&mut self, error: String) {
        self.error = Some(error);
        self.requested = None;
        self.published = None;
    }

    fn accepts_events(
        &self,
        epoch: u64,
        publication: Option<Publication>,
        scope: Option<MousePollingScope>,
    ) -> bool {
        !self.stopping
            && !self.callbacks_closed
            && self.epoch == epoch
            && publication.is_some()
            && self
                .published
                .is_some_and(|accepted| Some(accepted.scope) == scope)
            && publication.is_some_and(|publication| Some(publication.scope) == scope)
    }

    pub(super) fn needs_dpi_restore(&self) -> bool {
        self.dpi.clutched()
    }
    pub(super) fn finish_dpi_restore(&mut self) {
        self.dpi.clear_clutches();
    }
}

impl MouseProductWorkspace {
    pub fn set_receiver_mapping_connection_generation(
        &mut self,
        container: Option<String>,
        generation: u64,
    ) {
        if self.spec.product_id != 158 {
            return;
        }
        let next = container.map(|container| Connection {
            container,
            generation,
        });
        self.current_dpi.observe_mapping_connection(next.clone());
        if self.receiver_mapping.install_connection(next) {
            self.dpi_editing_observed = None;
        }
    }

    pub fn set_receiver_mapping_connection(&mut self, container: Option<String>) {
        if self.spec.product_id != 158
            || self
                .receiver_mapping
                .connection
                .as_ref()
                .map(|key| &key.container)
                == container.as_ref()
        {
            return;
        }
        let generation = self
            .receiver_mapping
            .connection
            .as_ref()
            .map_or(0, |key| key.generation);
        self.set_receiver_mapping_connection_generation(container, generation);
    }

    pub fn publish_receiver_mapping(
        &mut self,
        container: String,
        document: Value,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id != 158 || self.receiver_mapping.stopping {
            return;
        }
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct DocumentIdentity {
            active_profile: String,
        }
        let profile = match serde_json::from_value::<DocumentIdentity>(document.clone()) {
            Ok(identity) if !identity.active_profile.is_empty() => identity.active_profile,
            _ => {
                self.receiver_mapping_failure(
                    "Mapping document has no active profile identity".into(),
                    cx,
                );
                return;
            }
        };
        self.set_receiver_mapping_connection(Some(container.clone()));
        if self.receiver_mapping.owner.is_some() && self.receiver_mapping.admitted(&document, scope)
        {
            return;
        }
        if self
            .receiver_mapping
            .published
            .or(self.receiver_mapping.requested)
            .is_some_and(|publication| publication.scope != scope)
        {
            self.receiver_mapping.dpi.clear_clutches();
            self.receiver_mapping.published = None;
        }
        self.receiver_mapping.document = Some(document);
        self.receiver_mapping.profile_guid = Some(profile);
        if self.receiver_mapping.owner.is_none() {
            let (owner, replies) = match Session::start(158, container) {
                Ok(value) => value,
                Err(error) => {
                    self.receiver_mapping_failure(format!("{error:#}"), cx);
                    return;
                }
            };
            self.receiver_mapping.epoch = self.receiver_mapping.epoch.wrapping_add(1);
            let epoch = self.receiver_mapping.epoch;
            self.receiver_mapping.owner = Some(owner);
            self.receiver_mapping.pump = Some(cx.spawn(async move |view, cx| {
                loop {
                    cx.background_executor().timer(Duration::from_millis(50)).await;
                    let mut batch = Vec::new();
                    let disconnected = loop {
                        match replies.try_recv() {
                            Ok(reply) => batch.push(reply),
                            Err(std::sync::mpsc::TryRecvError::Empty) => break false,
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => break true,
                        }
                    };
                    if !view.update(cx, |this, cx| {
                        if this.receiver_mapping.epoch != epoch { return false; }
                        for reply in batch {
                            match reply {
                                Reply::Ready => {}
                                Reply::Published { revision, result, .. } => {
                                    let scope = this.mouse_polling_scope(cx);
                                    let previous_mode = this.receiver_mapping.dpi.inter_otfs_applied;
                                    this.receiver_mapping.acknowledge(revision, result, scope);
                                    if previous_mode != this.receiver_mapping.dpi.inter_otfs_applied {
                                        this.receiver_mapping_editing();
                                    }
                                    cx.notify();
                                }
                                Reply::Events(events) => {
                                    let publication = this.receiver_mapping.published;
                                    for event in events {
                                        // A callback may switch profile or republish. Recheck
                                        // the complete owner identity before its successor.
                                        if !this.receiver_mapping.accepts_events(epoch, publication, this.mouse_polling_scope(cx))
                                            || !this.receiver_mapping_matches_profile() { break; }
                                        this.receiver_mapping_event(event, cx);
                                    }
                                }
                                Reply::Failed(error) => {
                                    this.receiver_mapping.owner = None;
                                    this.receiver_mapping_failure(error, cx);
                                }
                                Reply::Stopped(result) => {
                                    this.receiver_mapping.owner = None;
                                    this.receiver_mapping.requested = None;
                                    this.receiver_mapping.published = None;
                                    if let Err(error) = result { this.receiver_mapping_failure(error, cx); }
                                    cx.notify();
                                    return false;
                                }
                            }
                        }
                        if disconnected {
                            this.receiver_mapping.owner = None;
                            this.receiver_mapping_failure("Mapping reply channel disconnected before cleanup confirmation".into(), cx);
                            false
                        } else { true }
                    }).unwrap_or(false) { break; }
                }
            }));
        }
        self.publish_receiver_mapping_current(scope, cx);
    }

    fn publish_receiver_mapping_current(
        &mut self,
        scope: MousePollingScope,
        cx: &mut Context<Self>,
    ) {
        let Some(ordinary) = self.receiver_mapping.document.as_ref() else {
            return;
        };
        let temporary = self.receiver_mapping.dpi.inter_otfs_requested;
        let document = if temporary {
            match razer_device::mouse_receiver_mapping::interdevice_otfs_document(ordinary) {
                Ok(document) => document,
                Err(error) => {
                    self.receiver_mapping_failure(format!("{error:#}"), cx);
                    return;
                }
            }
        } else {
            ordinary.clone()
        };
        let restore = temporary.then(|| ordinary.clone());
        self.receiver_mapping.revision = self.receiver_mapping.revision.wrapping_add(1);
        let revision = self.receiver_mapping.revision;
        self.receiver_mapping.requested = Some(Publication {
            revision,
            scope,
            inter_otfs: temporary,
        });
        if let Some(owner) = &self.receiver_mapping.owner {
            if let Err(error) = owner.publish_with_restore(revision, document, None, restore) {
                self.receiver_mapping_failure(error, cx);
            }
        }
    }

    fn receiver_mapping_failure(&mut self, error: String, cx: &mut Context<Self>) {
        eprintln!("[mouse158-mapping] {error}");
        self.receiver_mapping.fail(error);
        cx.notify();
    }

    fn receiver_mapping_editing(&mut self) {
        let dpi = &self.receiver_mapping.dpi;
        self.dpi_editing_observed = Some(!dpi.otfs() && !dpi.clutched());
    }

    fn receiver_mapping_matches_profile(&self) -> bool {
        #[derive(Deserialize)]
        struct Identity {
            guid: String,
        }
        serde_json::from_value::<Identity>(self.draft.clone())
            .ok()
            .is_some_and(|profile| {
                self.receiver_mapping.profile_guid.as_deref() == Some(profile.guid.as_str())
            })
    }

    fn receiver_mapping_profile(&self) -> Option<DpiStagesDraft> {
        #[derive(Deserialize)]
        struct Profile {
            #[serde(rename = "dpiStages")]
            stages: Stages,
        }
        #[derive(Deserialize)]
        struct Stages {
            enable: bool,
            active: usize,
            stages: Vec<razer_device::mouse_dpi_stages::DpiStage>,
        }
        if !self.current_dpi_profile_known() {
            return None;
        }
        let profile: Profile = serde_json::from_value(self.draft.clone()).ok()?;
        Some(DpiStagesDraft {
            enabled: profile.stages.enable,
            active_stage: profile.stages.active,
            stages: profile.stages.stages,
        })
    }

    fn receiver_mapping_event(&mut self, event: Value, cx: &mut Context<Self>) {
        match callbacks::parse(event, self.spec.min_dpi as u16, self.spec.max_dpi as u16) {
            Ok(callback) => self.apply_receiver_mapping_callback(callback, cx),
            Err(error) => {
                self.receiver_mapping.error = Some(format!("Mapping callback: {error:#}"));
                cx.notify();
            }
        }
    }

    pub fn consume_receiver_interdevice_mapping(
        &mut self,
        raw: Value,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.spec.product_id != 158
            || self.receiver_mapping.stopping
            || self.receiver_mapping.callbacks_closed
        {
            return false;
        }
        match callbacks::parse_raw(raw, self.spec.min_dpi as u16, self.spec.max_dpi as u16) {
            Ok(callback) => {
                // MW60168 R retains held interdevice OTFS until the target is ready.
                if self.receiver_mapping.owner.is_none()
                    && !matches!(callback, Callback::Otfs { .. })
                {
                    return false;
                }
                self.apply_receiver_mapping_callback(callback, cx);
                true
            }
            Err(error) => {
                self.receiver_mapping.error = Some(format!("Interdevice callback: {error:#}"));
                cx.notify();
                false
            }
        }
    }

    fn apply_receiver_mapping_callback(&mut self, callback: Callback, cx: &mut Context<Self>) {
        let generation = self
            .receiver_mapping
            .connection
            .as_ref()
            .map_or(0, |connection| connection.generation);
        match callback {
            Callback::Mode(enabled) => {
                self.receiver_mapping.dpi.native_otfs = Some(enabled);
                self.receiver_mapping_editing();
            }
            Callback::Otfs {
                enabled,
                interdevice,
            } => {
                if interdevice {
                    self.receiver_mapping.dpi.inter_otfs_requested = enabled;
                    if self.receiver_mapping.owner.is_some() {
                        if let Some(scope) = self.mouse_polling_scope(cx) {
                            self.publish_receiver_mapping_current(scope, cx);
                        }
                    }
                } else {
                    self.receiver_mapping.dpi.native_otfs = Some(enabled);
                    self.receiver_mapping_editing();
                }
            }
            Callback::ClutchDown { key, dpi } => {
                if self.receiver_dpi_callbacks_blocked() {
                    return;
                }
                self.receiver_mapping.dpi.push_clutch(key, dpi);
                self.receiver_mapping_editing();
                self.submit_receiver_callback_dpi(Some(dpi), false, true, cx);
            }
            Callback::ClutchUp(key) => {
                match self.receiver_mapping.dpi.release_clutch(&key) {
                    ClutchRelease::Unchanged => {}
                    ClutchRelease::Temporary(dpi) => {
                        self.submit_receiver_callback_dpi(Some(dpi), false, true, cx)
                    }
                    ClutchRelease::Profile => {
                        self.submit_receiver_callback_dpi(None, false, true, cx)
                    }
                }
                self.receiver_mapping_editing();
            }
            Callback::Stage(action) => {
                if self.receiver_mapping.dpi.clutched() && !self.receiver_mapping.dpi.otfs() {
                    return;
                }
                let Some(profile) = self.receiver_mapping_profile() else {
                    return;
                };
                let Some(next) = callbacks::next_stage(&profile, action) else {
                    return;
                };
                if let Some(active) = self.draft.pointer_mut(self.spec.active_path()) {
                    *active = json!(next);
                    self.submit_receiver_callback_dpi(None, true, true, cx);
                    cx.emit(MouseProductChanged);
                }
            }
            Callback::Value { up } => {
                if !self.receiver_mapping.dpi.otfs() {
                    return;
                }
                let Some(profile) = self.receiver_mapping_profile() else {
                    return;
                };
                let Some(index) = profile.active_stage.checked_sub(1) else {
                    return;
                };
                let Some(row) = profile.stages.get(index) else {
                    return;
                };
                let x = callbacks::otfs_value(
                    row.x,
                    up,
                    self.spec.dpi_step as u16,
                    self.spec.min_dpi as u16,
                    self.spec.max_dpi as u16,
                );
                let y = callbacks::otfs_value(
                    row.y,
                    up,
                    self.spec.dpi_step as u16,
                    self.spec.min_dpi as u16,
                    self.spec.max_dpi as u16,
                );
                if let Some(row) = self
                    .draft
                    .pointer_mut(self.spec.stages_path())
                    .and_then(Value::as_array_mut)
                    .and_then(|rows| rows.get_mut(index))
                {
                    row[self.spec.dpi_axis(0)] = json!(x);
                    row[self.spec.dpi_axis(1)] = json!(y);
                    // Combined OTFS/clutch states persist stages without
                    // overwriting the held temporary hardware DPI.
                    self.submit_receiver_callback_dpi(
                        None,
                        true,
                        !self.receiver_mapping.dpi.clutched(),
                        cx,
                    );
                    cx.emit(MouseProductChanged);
                }
            }
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
}

#[cfg(test)]
#[path = "mouse_mapping_runtime_158_tests.rs"]
mod tests;
