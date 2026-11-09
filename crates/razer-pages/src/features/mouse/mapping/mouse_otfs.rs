//! Serial local OTFS transitions: retain bindings, save the projection, then publish.
use super::*;
use razer_device::mouse_otfs::Snapshot;
use std::collections::VecDeque;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use uuid::Uuid;

#[derive(Clone)]
pub struct MouseOtfsRequested {
    id: Uuid,
    document: Value,
    snapshot: Snapshot,
    temporary: bool,
    publish_required: bool,
    valid: Arc<AtomicBool>,
}
impl MouseOtfsRequested {
    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn document(&self) -> &Value {
        &self.document
    }
    pub fn is_valid(&self) -> bool {
        self.valid.load(Ordering::Acquire)
    }
    pub fn publication_required(&self) -> bool {
        self.publish_required
    }
    pub fn rebase(&mut self, current: &Value) -> anyhow::Result<()> {
        match razer_device::mouse_otfs::project(current, &self.snapshot, self.temporary)? {
            Some(document) => {
                self.document = document;
                self.publish_required = true;
            }
            None => {
                self.publish_required = false;
            }
        }
        Ok(())
    }
}
impl EventEmitter<MouseOtfsRequested> for MouseProductWorkspace {}

struct Pending {
    request: MouseOtfsRequested,
    restoring: bool,
    enabled: bool,
    publishing: bool,
}
#[derive(Default)]
pub(super) struct State {
    snapshot: Option<Snapshot>,
    pending: Option<Pending>,
    queued: VecDeque<bool>,
    publication_cache: razer_device::mouse_otfs::PublicationCache,
}
impl State {
    pub(super) fn reset(&mut self) {
        if let Some(pending) = &self.pending {
            pending.request.valid.store(false, Ordering::Release);
        }
        *self = Self::default();
    }
    pub(super) fn stop_pending(&mut self) {
        self.queued.clear();
        if let Some(pending) = self.pending.take() {
            pending.request.valid.store(false, Ordering::Release);
        }
    }
    pub(super) fn is_pending(&self) -> bool {
        self.pending.is_some()
    }
    pub(super) fn record_publication(&mut self, document: &Value) -> Result<bool, String> {
        self.publication_cache
            .admit(document)
            .map_err(|error| format!("{error:#}"))
    }
}

impl MouseProductWorkspace {
    pub(super) fn request_interdevice_otfs(&mut self, enabled: bool, cx: &mut Context<Self>) {
        self.mouse_otfs.queued.push_back(enabled);
        self.start_interdevice_otfs(cx);
    }
    fn start_interdevice_otfs(&mut self, cx: &mut Context<Self>) {
        if self.mouse_otfs.pending.is_some() {
            return;
        }
        let Some(enabled) = self.mouse_otfs.queued.pop_front() else {
            return;
        };
        let document = match self.spec.product_id {
            70 => self.legacy_mapping.otfs_document(),
            pid if razer_device::mouse_receiver_mapping::supports_ordinary(pid) => self.receiver_mapping.otfs_document(),
            _ => None,
        }
        .cloned();
        let Some(document) = document else {
            return;
        };
        let temporary = enabled && self.mouse_otfs.snapshot.is_none();
        if temporary {
            match Snapshot::capture(&document) {
                Ok(snapshot) => self.mouse_otfs.snapshot = Some(snapshot),
                Err(error) => {
                    self.otfs_failure(format!("{error:#}"));
                    return;
                }
            }
        }
        let Some(snapshot) = &self.mouse_otfs.snapshot else {
            self.finish_otfs_mode(enabled);
            self.start_interdevice_otfs(cx);
            return;
        };
        match razer_device::mouse_otfs::project(&document, snapshot, temporary) {
            Ok(Some(document)) => {
                let request = MouseOtfsRequested {
                    id: Uuid::new_v4(),
                    document,
                    snapshot: snapshot.clone(),
                    temporary,
                    publish_required: true,
                    valid: Arc::new(AtomicBool::new(true)),
                };
                self.mouse_otfs.pending = Some(Pending {
                    request: request.clone(),
                    restoring: !temporary,
                    enabled,
                    publishing: false,
                });
                cx.emit(request);
            }
            Ok(None) => {
                if !temporary {
                    self.mouse_otfs.snapshot = None;
                }
                self.finish_otfs_mode(enabled);
                self.start_interdevice_otfs(cx);
            }
            Err(error) => {
                self.otfs_failure(format!("{error:#}"));
                self.start_interdevice_otfs(cx);
            }
        }
    }
    fn otfs_failure(&mut self, message: String) {
        match self.spec.product_id {
            70 => self.legacy_mapping.otfs_error(message),
            pid if razer_device::mouse_receiver_mapping::supports_ordinary(pid) => self.receiver_mapping.otfs_error(message),
            _ => {}
        }
    }
    fn finish_otfs_mode(&mut self, enabled: bool) {
        let editable = match self.spec.product_id {
            70 => self.legacy_mapping.otfs_mode(enabled),
            pid if razer_device::mouse_receiver_mapping::supports_ordinary(pid) => self.receiver_mapping.otfs_mode(enabled),
            _ => return,
        };
        self.dpi_editing_observed = Some(editable);
    }
    pub fn accepts_mouse_otfs(&self, request: &MouseOtfsRequested) -> bool {
        self.mouse_otfs
            .pending
            .as_ref()
            .is_some_and(|pending| pending.request.id == request.id)
    }
    pub fn finish_mouse_otfs_save(
        &mut self,
        request: &MouseOtfsRequested,
        saved: bool,
        cx: &mut Context<Self>,
    ) {
        if !self.accepts_mouse_otfs(request) {
            return;
        }
        if self
            .mouse_otfs
            .pending
            .as_ref()
            .is_some_and(|pending| pending.publishing)
        {
            return;
        }
        if !saved {
            self.finish_mouse_otfs_publication(
                request.id,
                Err("OTFS profile persistence failed".into()),
                cx,
            );
            return;
        }
        if !request.publication_required() {
            self.finish_mouse_otfs_publication(request.id, Ok(()), cx);
            return;
        }
        self.mouse_otfs.pending.as_mut().unwrap().request = request.clone();
        self.mouse_otfs.pending.as_mut().unwrap().publishing = true;
        match self.spec.product_id {
            70 => self
                .legacy_mapping
                .replace_otfs_document(request.document.clone()),
            pid if razer_device::mouse_receiver_mapping::supports_ordinary(pid) => self
                .receiver_mapping
                .replace_otfs_document(request.document.clone()),
            _ => {}
        }
        let enabled = self.mouse_otfs.pending.as_ref().unwrap().enabled;
        match self.mouse_otfs.record_publication(request.document()) {
            Ok(false) => {
                self.finish_mouse_otfs_publication(request.id, Ok(()), cx);
                return;
            }
            Ok(true) => {}
            Err(error) => {
                self.finish_mouse_otfs_publication(request.id, Err(error), cx);
                return;
            }
        }
        match self.spec.product_id {
            70 => self.publish_legacy_otfs_saved(request, enabled, cx),
            pid if razer_device::mouse_receiver_mapping::supports_ordinary(pid) => self.publish_receiver_otfs_saved(request, enabled, cx),
            _ => {}
        }
    }
    pub(super) fn finish_mouse_otfs_publication(
        &mut self,
        id: Uuid,
        result: Result<(), String>,
        cx: &mut Context<Self>,
    ) {
        if !self
            .mouse_otfs
            .pending
            .as_ref()
            .is_some_and(|pending| pending.request.id == id)
        {
            return;
        }
        let pending = self.mouse_otfs.pending.take().unwrap();
        pending.request.valid.store(false, Ordering::Release);
        match result {
            Ok(()) => {
                if pending.restoring {
                    self.mouse_otfs.snapshot = None;
                }
                self.finish_otfs_mode(pending.enabled);
            }
            Err(error) => self.otfs_failure(error),
        }
        self.start_interdevice_otfs(cx);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::super::MouseProductWorkspace;
    use gpui_kit::component::Root;
    use gpui_kit::{AppContext, TestAppContext, px, size};
    use serde_json::json;

    #[gpui_kit::test]
    fn repeated_enable_restores_snapshot_and_failed_save_preserves_it(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        for pid in [70,89,96,104,112,115,140,145,148,158,161,178,185,190,192,194,196,222,224,229,239] {
            let mut owner = None;
            let handle = cx.open_window(size(px(1600.), px(1000.)), |window, cx| {
                let page = cx.new(|cx| MouseProductWorkspace::new(pid, window, cx));
                owner = Some(page.clone());
                Root::new(page, window, cx)
            });
            let owner = owner.unwrap();
            cx.update_window(handle.into(), |_, _, cx| {
                owner.update(cx, |page, cx| {
                    let profile = json!({"guid":"a","name":"A","mappings":[]});
                    let guids = (0..128).map(|i| format!("00000000-0000-4000-8000-{i:012}"));
                    let doc = if pid == 70 {
                        razer_device::mouse_legacy_mapping::prepare_document(
                            &[profile],
                            "a",
                            None,
                            guids,
                        )
                    } else {
                        razer_device::mouse_receiver_mapping::prepare_document_for(
                            pid,
                            &[profile],
                            "a",
                            None,
                            None,
                            guids,
                        )
                    }
                    .unwrap();
                    if pid == 70 {
                        page.legacy_mapping.replace_otfs_document(doc);
                    } else {
                        page.receiver_mapping.replace_otfs_document(doc);
                    }
                    page.request_interdevice_otfs(true, cx);
                    let first = page.mouse_otfs.pending.as_ref().unwrap().request.clone();
                    assert!(first.temporary);
                    let mut switched = first.document().clone();
                    let mut next_profile = switched["profiles"][0].clone();
                    next_profile["guid"] = json!("current");
                    next_profile["name"] = json!("Current");
                    next_profile["customField"] = json!(123);
                    switched["profiles"]
                        .as_array_mut()
                        .unwrap()
                        .push(next_profile);
                    switched["activeProfile"] = json!("current");
                    let mut rebased = first.clone();
                    rebased.rebase(&switched).unwrap();
                    assert_eq!(rebased.document()["activeProfile"], "current");
                    assert_eq!(rebased.document()["profiles"][1]["customField"], 123);
                    assert_eq!(rebased.document()["profiles"][0], switched["profiles"][0]);
                    page.request_interdevice_otfs(true, cx);
                    assert_eq!(page.mouse_otfs.queued.len(), 1);
                    page.finish_mouse_otfs_save(&first, false, cx);
                    let restore = page.mouse_otfs.pending.as_ref().unwrap().request.clone();
                    assert!(!restore.temporary);
                    assert!(page.mouse_otfs.snapshot.is_some());
                    page.finish_mouse_otfs_publication(first.id(), Ok(()), cx);
                    assert!(page.accepts_mouse_otfs(&restore));
                    page.finish_mouse_otfs_publication(
                        restore.id(),
                        Err("publish failed".into()),
                        cx,
                    );
                    assert!(page.mouse_otfs.snapshot.is_some());
                    page.request_interdevice_otfs(false, cx);
                    let mut retry = page.mouse_otfs.pending.as_ref().unwrap().request.clone();
                    let mut removed = retry.document().clone();
                    removed["activeProfile"] = json!("removed");
                    retry.rebase(&removed).unwrap();
                    assert!(!retry.publication_required());
                    page.finish_mouse_otfs_save(&retry, true, cx);
                    assert!(page.mouse_otfs.snapshot.is_none());
                    assert!(!retry.is_valid());
                    page.request_interdevice_otfs(true, cx);
                    let stale = page.mouse_otfs.pending.as_ref().unwrap().request.clone();
                    page.mouse_otfs.reset();
                    assert!(!stale.is_valid());
                    page.finish_mouse_otfs_save(&stale, true, cx);
                    assert!(page.mouse_otfs.pending.is_none());
                    page.request_interdevice_otfs(true, cx);
                    let successor = page.mouse_otfs.pending.as_ref().unwrap().request.clone();
                    assert_ne!(stale.id(), successor.id());
                    page.finish_mouse_otfs_publication(stale.id(), Ok(()), cx);
                    assert!(page.accepts_mouse_otfs(&successor));
                    page.mouse_otfs.pending.as_mut().unwrap().publishing = true;
                    page.finish_mouse_otfs_save(&successor, false, cx);
                    assert!(page.accepts_mouse_otfs(&successor));
                    page.mouse_otfs.stop_pending();
                    assert!(!successor.is_valid());
                    assert!(page.mouse_otfs.snapshot.is_some());
                    assert!(!page.mouse_otfs.is_pending());
                });
            })
            .unwrap();
        }
    }
}
