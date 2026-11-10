//! Current ordinary mouse software publication and callback lifetime. A publication
//! receipt never acknowledges an onboard profile write or supplies device DPI.
use super::*;
use razer_device::mouse_dpi_stages::DpiStagesDraft;
use razer_service::mapping::device::{Reply, Session};
use std::time::Duration;

use super::mapping_callbacks_current as callbacks;
use callbacks::{Callback, ClutchRelease, Connection, DpiRuntime};
use crate::features::ReceiverMappingPublication;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Publication {
    revision: u64,
    scope: MousePollingScope,
    inter_otfs: bool,
}

impl EventEmitter<ReceiverMappingPublication> for MouseProductWorkspace {}

#[derive(Clone, Copy, PartialEq, Eq)]
struct SharedOwner {
    id: uuid::Uuid,
    physical_product: u32,
}

#[derive(Default)]
pub(super) struct State {
    owner: Option<Session>,
    shared_owner: Option<SharedOwner>,
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
    otfs_request: Option<uuid::Uuid>,
}

impl State {
    pub(super) fn otfs_document(&self) -> Option<&Value> {
        self.document.as_ref()
    }
    pub(super) fn otfs_error(&mut self, error: String) {
        self.error = Some(error);
    }
    pub(super) fn otfs_mode(&mut self, enabled: bool) -> bool {
        self.dpi.inter_otfs_requested = enabled;
        self.dpi.inter_otfs_applied = enabled;
        !self.dpi.otfs() && !self.dpi.clutched()
    }
    pub(super) fn replace_otfs_document(&mut self, document: Value) {
        self.document = Some(document);
    }
    pub(super) fn stop(&mut self) {
        // Session cancellation is nonblocking. The worker keeps its target
        // lifecycle lock until restoration, removal and transport release end.
        self.owner = None;
        self.shared_owner = None;
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
        self.otfs_request = None;
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
    pub fn shared_mapping_matches(&self, physical_product: u32, container: &str, generation: u64) -> bool {
        self.receiver_mapping.shared_owner.is_some_and(|owner| owner.physical_product == physical_product)
            && self.receiver_mapping.connection.as_ref().is_some_and(|connection| {
                connection.container == container && connection.generation == generation
            })
    }

    pub fn shared_mapping_connection_matches(&self, container: &str, generation: u64) -> bool {
        self.receiver_mapping.shared_owner.is_some()
            && self.receiver_mapping.connection.as_ref().is_some_and(|connection| {
                connection.container == container && connection.generation == generation
            })
    }

    pub fn configure_shared_mapping(&mut self, owner: uuid::Uuid, physical_product: u32, container: String, generation: u64) {
        let shared = SharedOwner { id: owner, physical_product };
        if self.receiver_mapping.shared_owner != Some(shared)
            || !self.shared_mapping_matches(physical_product, &container, generation)
        {
            self.receiver_mapping.stop();
            self.mouse_otfs.reset();
            let connection = Connection { container, generation };
            self.current_dpi.observe_mapping_connection(Some(connection.clone()));
            self.receiver_mapping.connection = Some(connection);
            self.receiver_mapping.shared_owner = Some(shared);
            self.receiver_mapping.error = None;
        }
    }

    pub fn observe_shared_mapping_publication(&mut self, owner: uuid::Uuid, revision: u64, result: Result<(), String>, cx: &mut Context<Self>) -> bool {
        if !self.receiver_mapping.shared_owner.is_some_and(|shared| shared.id == owner) {
            return false;
        }
        let otfs = (revision == self.receiver_mapping.revision).then(|| self.receiver_mapping.otfs_request.take()).flatten();
        let scope = self.mouse_polling_scope(cx);
        let otfs_result = result.clone();
        self.receiver_mapping.acknowledge(revision, result, scope);
        if let Some(id) = otfs {
            let current = self.receiver_mapping.published.is_some_and(|publication| publication.revision == revision && Some(publication.scope) == scope);
            self.finish_mouse_otfs_publication(id, otfs_result.and_then(|()| {
                if current { Ok(()) } else { Err("OTFS publication scope changed".into()) }
            }), cx);
        }
        self.receiver_mapping_editing();
        cx.notify();
        true
    }

    pub fn observe_shared_mapping_event(&mut self, owner: uuid::Uuid, event: Value, cx: &mut Context<Self>) -> bool {
        if !self.receiver_mapping.shared_owner.is_some_and(|shared| shared.id == owner)
            || !self.receiver_mapping.accepts_events(self.receiver_mapping.epoch, self.receiver_mapping.published, self.mouse_polling_scope(cx))
            || !self.receiver_mapping_matches_profile()
        { return false; }
        self.receiver_mapping_event(event, cx);
        true
    }

    pub fn release_shared_mapping(&mut self, owner: uuid::Uuid, error: Option<String>, cx: &mut Context<Self>) {
        if self.receiver_mapping.shared_owner.is_some_and(|shared| shared.id == owner) {
            self.receiver_mapping.stop();
            self.mouse_otfs.stop_pending();
            self.receiver_mapping.error = error;
            self.dpi_editing_observed = None;
            cx.notify();
        }
    }

    pub fn set_receiver_mapping_connection_generation(
        &mut self,
        container: Option<String>,
        generation: u64,
    ) {
        if !razer_device::mouse_receiver_mapping::supports_ordinary(self.mapping_source_product_id()) {
            return;
        }
        let next = container.map(|container| Connection {
            container,
            generation,
        });
        self.current_dpi.observe_mapping_connection(next.clone());
        if self.receiver_mapping.install_connection(next) {
            self.mouse_otfs.reset();
            self.dpi_editing_observed = None;
        }
    }

    pub fn set_receiver_mapping_connection(&mut self, container: Option<String>) {
        if !razer_device::mouse_receiver_mapping::supports_ordinary(self.mapping_source_product_id())
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
        if !razer_device::mouse_receiver_mapping::supports_ordinary(self.mapping_source_product_id()) || self.receiver_mapping.stopping {
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
        if self.receiver_mapping.shared_owner.is_some() {
            self.publish_receiver_mapping_current(scope, cx);
            return;
        }
        if self.mouse_otfs.is_pending() && self.receiver_mapping.owner.is_some() {
            return;
        }
        if self.receiver_mapping.owner.is_none() {
            let (owner, replies) = match Session::start(self.spec.product_id, container) {
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
                                    let otfs_result = result.clone();
                                    let otfs_id = (revision == this.receiver_mapping.revision)
                                        .then(|| this.receiver_mapping.otfs_request.take()).flatten();
                                    let scope = this.mouse_polling_scope(cx);
                                    let previous_mode = this.receiver_mapping.dpi.inter_otfs_applied;
                                    this.receiver_mapping.acknowledge(revision, result, scope);
                                    if let Some(id) = otfs_id {
                                        let current = this.receiver_mapping.published.is_some_and(|publication| Some(publication.scope) == scope);
                                        this.finish_mouse_otfs_publication(id, if current { otfs_result } else { Err("OTFS publication scope changed".into()) }, cx);
                                    }
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
                                    this.mouse_otfs.stop_pending();
                                    this.receiver_mapping.owner = None;
                                    this.receiver_mapping_failure(error, cx);
                                }
                                Reply::Stopped(result) => {
                                    this.mouse_otfs.stop_pending();
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
                            this.mouse_otfs.stop_pending();
                            this.receiver_mapping.owner = None;
                            this.receiver_mapping_failure("Mapping reply channel disconnected before cleanup confirmation".into(), cx);
                            false
                        } else { true }
                    }).unwrap_or(false) { break; }
                }
            }));
        }
        if let Some(document) = self.receiver_mapping.document.as_ref() {
            if let Err(error) = self.mouse_otfs.record_publication(document) {
                self.receiver_mapping_failure(error, cx);
                return;
            }
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
        let document = ordinary.clone();
        self.receiver_mapping.revision = self.receiver_mapping.revision.wrapping_add(1);
        let revision = self.receiver_mapping.revision;
        self.receiver_mapping.requested = Some(Publication {
            revision,
            scope,
            inter_otfs: temporary,
        });
        if let Some(owner) = self.receiver_mapping.shared_owner {
            cx.emit(ReceiverMappingPublication::new(owner.id, self.mapping_source_product_id(), revision, document));
            return;
        }
        if let Some(owner) = &self.receiver_mapping.owner {
            if let Err(error) = owner.publish(revision, document, None) {
                self.receiver_mapping_failure(error, cx);
            }
        }
    }

    fn receiver_mapping_failure(&mut self, error: String, cx: &mut Context<Self>) {
        eprintln!("[mouse158-mapping] {error}");
        self.receiver_mapping.fail(error);
        cx.notify();
    }

    pub(super) fn publish_receiver_otfs_saved(
        &mut self,
        request: &super::mouse_otfs::MouseOtfsRequested,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(scope) = self.mouse_polling_scope(cx) else {
            self.finish_mouse_otfs_publication(
                request.id(),
                Err("OTFS profile scope unavailable".into()),
                cx,
            );
            return;
        };
        self.receiver_mapping.document = Some(request.document().clone());
        self.receiver_mapping.dpi.inter_otfs_requested = enabled;
        self.receiver_mapping.otfs_request = Some(request.id());
        self.publish_receiver_mapping_current(scope, cx);
        if self.receiver_mapping.requested.is_none()
            || self.receiver_mapping.owner.is_none() && self.receiver_mapping.shared_owner.is_none()
        {
            self.receiver_mapping.otfs_request = None;
            self.finish_mouse_otfs_publication(
                request.id(),
                Err("OTFS mapping owner unavailable".into()),
                cx,
            );
        }
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
        if !razer_device::mouse_receiver_mapping::supports_ordinary(self.mapping_source_product_id())
            || self.receiver_mapping.stopping
            || self.receiver_mapping.callbacks_closed
        {
            return false;
        }
        match callbacks::parse_raw(raw, self.spec.min_dpi as u16, self.spec.max_dpi as u16) {
            Ok(callback) => {
                // MW60168 R retains held interdevice OTFS until the target is ready.
                if self.receiver_mapping.owner.is_none()
                    && self.receiver_mapping.shared_owner.is_none()
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
                    self.request_interdevice_otfs(enabled, cx);
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
