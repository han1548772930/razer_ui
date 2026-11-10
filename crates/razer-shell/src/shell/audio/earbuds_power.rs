//! Global sleep saves and their real device operations share the shell save queue.
use super::*;
use razer_pages::features::EarbudsPowerRequested;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use uuid::Uuid;

#[derive(Default)]
pub(super) struct State {
    registered: bool,
    workers: BTreeMap<Uuid, (Arc<AtomicBool>, Task<()>)>,
    lanes: BTreeMap<String, Lane<Submission>>,
}
struct Lane<T> {
    busy: bool,
    pending: std::collections::VecDeque<T>,
}
impl<T> Default for Lane<T> {
    fn default() -> Self {
        Self {
            busy: false,
            pending: Default::default(),
        }
    }
}
impl<T> Lane<T> {
    fn next(&mut self, current: impl Fn(&T) -> bool, prepared: impl Fn(&T) -> bool) -> Option<T> {
        if self.busy {
            return None;
        }
        while let Some(request) = self.pending.front() {
            if !current(request) {
                self.pending.pop_front();
                continue;
            }
            if !prepared(request) {
                return None;
            }
            self.busy = true;
            return self.pending.pop_front();
        }
        None
    }
    fn finished(&mut self) {
        self.busy = false;
    }
}
struct Submission {
    workspace: Entity<ProductWorkspace>,
    request: EarbudsPowerRequested,
    preparation: Preparation,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Preparation {
    WaitingForSave,
    Ready,
}
impl Drop for State {
    fn drop(&mut self) {
        for (active, _) in self.workers.values() {
            active.store(false, Ordering::Release);
        }
        for lane in self.lanes.values() {
            for submission in &lane.pending {
                submission.request.guard().store(false, Ordering::Release);
            }
        }
    }
}
pub(super) struct LocalSave {
    workspace: Entity<ProductWorkspace>,
    identity: String,
    request: EarbudsPowerRequested,
}
impl LocalSave {
    pub(super) fn owner(&self) -> (&str, WeakEntity<ProductWorkspace>) {
        (&self.identity, self.workspace.downgrade())
    }
}

#[cfg(test)]
mod tests {
    use super::Lane;
    use razer_device::earbuds_power::Setting;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    #[test]
    fn fifo_holds_later_complete_settings_until_first_device_cleanup_finishes() {
        let first = Setting::new(false, 900);
        let second = Setting::new(true, 300);
        let mut lane = Lane::default();
        lane.pending.extend([first, second]);
        assert_eq!(lane.next(|_| true, |_| true), Some(first));
        // Completion and cleanup have not arrived, even if a later save or
        // ready callback has already completed on another worker.
        assert_eq!(lane.next(|_| true, |_| true), None);
        lane.finished();
        assert_eq!(lane.next(|_| true, |_| true), Some(second));
        assert_eq!(lane.next(|_| true, |_| true), None);
        lane.finished();
        assert_eq!(lane.next(|_| true, |_| true), None);
    }

    #[test]
    fn failure_releases_fifo_and_reconnect_skips_cancelled_queued_settings() {
        let old = Arc::new(AtomicBool::new(true));
        let current = Arc::new(AtomicBool::new(true));
        let first = Setting::new(true, 900);
        let stale = Setting::new(false, 2700);
        let replacement = Setting::new(true, 3600);
        let mut lane = Lane::default();
        lane.pending
            .extend([(first, old.clone()), (stale, old.clone())]);
        let valid = |value: &(Setting, Arc<AtomicBool>)| value.1.load(Ordering::Acquire);
        assert_eq!(lane.next(valid, |_| true).unwrap().0, first);
        old.store(false, Ordering::Release);
        lane.pending.push_back((replacement, current));
        assert!(lane.next(valid, |_| true).is_none());
        // Failed/cancelled operations use the same terminal cleanup boundary.
        lane.finished();
        assert_eq!(lane.next(valid, |_| true).unwrap().0, replacement);
        assert!(lane.pending.is_empty());
    }

    #[test]
    fn independent_physical_power_owners_do_not_block_each_other() {
        let mut first = Lane::default();
        let mut second = Lane::default();
        first.pending.push_back(Setting::new(true, 300));
        second.pending.push_back(Setting::new(false, 900));
        assert!(first.next(|_| true, |_| true).is_some());
        assert!(second.next(|_| true, |_| true).is_some());
        assert!(first.busy && second.busy);
    }
    #[test]
    fn pending_save_cannot_be_overtaken_by_a_ready_profile_restore() {
        use super::Preparation;
        let first = Setting::new(true, 300);
        let restore = Setting::new(false, 900);
        let mut lane = Lane::default();
        lane.pending.extend([
            (first, Preparation::WaitingForSave),
            (restore, Preparation::Ready),
        ]);
        let ready = |request: &(Setting, Preparation)| request.1 == Preparation::Ready;
        assert!(lane.next(|_| true, ready).is_none());
        assert_eq!(lane.pending.len(), 2);
        lane.pending.front_mut().unwrap().1 = Preparation::Ready;
        assert_eq!(lane.next(|_| true, ready).unwrap().0, first);
        assert!(lane.next(|_| true, ready).is_none());
        lane.finished();
        assert_eq!(lane.next(|_| true, ready).unwrap().0, restore);
    }
}
impl AppShell {
    pub(super) fn request_earbuds_power(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: EarbudsPowerRequested,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(page) = workspace.read(cx).audio_mixer_page(cx) else {
            return;
        };
        if !page.read(cx).earbuds_power_request_current(&request) {
            return;
        }
        if self.close_requested || request.needs_persistence() && self.storage_error.is_some() {
            page.update(cx, |page, cx| {
                page.finish_earbuds_power(
                    &request,
                    Err("Earbud global power persistence is unavailable".into()),
                    cx,
                )
            });
            return;
        }
        if !request.needs_persistence() {
            self.submit_earbuds_power(workspace, request, Preparation::Ready, cx);
            return;
        }
        self.submit_earbuds_power(
            workspace.clone(),
            request.clone(),
            Preparation::WaitingForSave,
            cx,
        );
        let save = LocalSave {
            identity: workspace.read(cx).identity(cx),
            workspace,
            request,
        };
        let file = self.workspace_file(
            self.devices
                .iter()
                .map(|workspace| workspace.read(cx).saved_snapshot(cx))
                .collect(),
            cx,
        );
        self.pending_saves.push_back(self.prepare_save(
            window.window_handle(),
            file,
            Some(ProductSave::EarbudsPower(save)),
            cx,
        ));
        self.start_pending_save(cx);
    }
    pub(super) fn rebase_earbuds_power_save(
        &self,
        save: &mut PreparedSave,
        cx: &App,
    ) -> anyhow::Result<()> {
        let Some(ProductSave::EarbudsPower(local)) = &save.product else {
            return Ok(());
        };
        anyhow::ensure!(
            local
                .workspace
                .read(cx)
                .audio_mixer_page(cx)
                .is_some_and(|page| page.read(cx).earbuds_power_request_current(&local.request)),
            "Earbud global power save belongs to an inactive connection"
        );
        let setting = local
            .request
            .setting()
            .ok_or_else(|| anyhow::anyhow!("Power read cannot save a preference"))?;
        anyhow::ensure!(
            local.request.needs_persistence(),
            "Profile power restore cannot overwrite global preferences"
        );
        let mut found = false;
        save.file.devices = self
            .devices
            .iter()
            .map(|workspace| {
                let mut saved = workspace.read(cx).saved_snapshot(cx);
                if workspace.read(cx).identity(cx) == local.identity {
                    anyhow::ensure!(!found, "Earbud global power owner is duplicated");
                    found = true;
                    razer_storage::earbuds_profiles::merge_power(&mut saved, setting)?;
                }
                Ok(saved)
            })
            .collect::<anyhow::Result<_>>()?;
        anyhow::ensure!(found, "Earbud global power owner was removed");
        Ok(())
    }
    pub(super) fn finish_earbuds_power_save(
        &mut self,
        save: LocalSave,
        succeeded: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(page) = save.workspace.read(cx).audio_mixer_page(cx) else {
            self.resolve_earbuds_power_save(save.identity, save.request.id(), false, cx);
            return;
        };
        if !succeeded {
            page.update(cx, |page, cx| {
                page.finish_earbuds_power(
                    &save.request,
                    Err("Earbud global power save failed; no device setting was sent".into()),
                    cx,
                )
            });
            self.resolve_earbuds_power_save(save.identity, save.request.id(), false, cx);
            return;
        }
        let accepted = page.update(cx, |page, _| {
            page.mark_earbuds_power_persisted(&save.request)
        });
        self.resolve_earbuds_power_save(save.identity, save.request.id(), accepted, cx);
    }
    fn resolve_earbuds_power_save(
        &mut self,
        key: String,
        request_id: Uuid,
        accepted: bool,
        cx: &mut Context<Self>,
    ) {
        if let Some(lane) = self.earbuds_power.lanes.get_mut(&key) {
            if accepted {
                if let Some(request) = lane
                    .pending
                    .iter_mut()
                    .find(|request| request.request.id() == request_id)
                {
                    request.preparation = Preparation::Ready;
                }
            } else {
                lane.pending
                    .retain(|request| request.request.id() != request_id);
            }
        }
        self.start_earbuds_power_lane(key, cx);
    }
    fn submit_earbuds_power(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: EarbudsPowerRequested,
        preparation: Preparation,
        cx: &mut Context<Self>,
    ) {
        let key = workspace.read(cx).identity(cx);
        self.earbuds_power
            .lanes
            .entry(key.clone())
            .or_default()
            .pending
            .push_back(Submission {
                workspace,
                request,
                preparation,
            });
        self.start_earbuds_power_lane(key, cx);
    }
    fn start_earbuds_power_lane(&mut self, key: String, cx: &mut Context<Self>) {
        let Some(lane) = self.earbuds_power.lanes.get_mut(&key) else {
            return;
        };
        let submission = lane.next(
            |submission| {
                submission
                    .workspace
                    .read(cx)
                    .audio_mixer_page(cx)
                    .is_some_and(|page| {
                        page.read(cx)
                            .earbuds_power_request_current(&submission.request)
                    })
            },
            |submission| submission.preparation == Preparation::Ready,
        );
        let Some(submission) = submission else {
            if !lane.busy && lane.pending.is_empty() {
                self.earbuds_power.lanes.remove(&key);
            }
            return;
        };
        self.begin_earbuds_power(submission.workspace, submission.request, key, cx);
    }
    fn finish_earbuds_power_lane(&mut self, key: String, cx: &mut Context<Self>) {
        if let Some(lane) = self.earbuds_power.lanes.get_mut(&key) {
            lane.finished();
        }
        self.start_earbuds_power_lane(key, cx);
    }
    fn begin_earbuds_power(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: EarbudsPowerRequested,
        key: String,
        cx: &mut Context<Self>,
    ) {
        let Some(page) = workspace.read(cx).audio_mixer_page(cx) else {
            self.finish_earbuds_power_lane(key, cx);
            return;
        };
        if !page.read(cx).earbuds_power_request_current(&request) {
            self.finish_earbuds_power_lane(key, cx);
            return;
        }
        let device = workspace.read(cx).device(cx);
        if self.close_requested
            || request.scope().product_id() != device.product_id
            || request.scope().container_id() != device.device_container_id
        {
            page.update(cx, |page, cx| {
                page.finish_earbuds_power(
                    &request,
                    Err("No current earbud power device observation".into()),
                    cx,
                )
            });
            self.finish_earbuds_power_lane(key, cx);
            return;
        }
        self.with_earbuds_session(
            workspace.clone(),
            request.guard(),
            None,
            cx,
            move |result, shell, cx| match result {
                Ok(lease) => shell.submit_ready_earbuds_power(workspace, request, lease, key, cx),
                Err(error) => {
                    page.update(cx, |page, cx| {
                        page.finish_earbuds_power(&request, Err(error), cx)
                    });
                    shell.finish_earbuds_power_lane(key, cx);
                }
            },
        );
    }
    fn submit_ready_earbuds_power(
        &mut self,
        workspace: Entity<ProductWorkspace>,
        request: EarbudsPowerRequested,
        lease: super::earbuds_events::Lease,
        key: String,
        cx: &mut Context<Self>,
    ) {
        let Some(page) = workspace.read(cx).audio_mixer_page(cx) else {
            self.finish_earbuds_power_lane(key, cx);
            return;
        };
        if !page.read(cx).earbuds_power_request_current(&request) {
            self.finish_earbuds_power_lane(key, cx);
            return;
        }
        if !self.earbuds_power.registered {
            self.earbuds_power.registered = true;
            self.register_exit_cleanup(|shell, _| {
                let lanes = std::mem::take(&mut shell.earbuds_power.lanes);
                for lane in lanes.values() {
                    for submission in &lane.pending {
                        submission.request.guard().store(false, Ordering::Release);
                    }
                }
                let workers = std::mem::take(&mut shell.earbuds_power.workers);
                for (active, _) in workers.values() {
                    active.store(false, Ordering::Release);
                }
                async move {
                    for (_, (_, task)) in workers {
                        task.await;
                    }
                }
            });
        }
        let id = request.id();
        let active = request.guard();
        let worker_active = active.clone();
        let operation = request.operation();
        let connection = workspace.read(cx).connection_revision();
        let identity = workspace.read(cx).identity(cx);
        let task = cx.spawn(async move |shell, cx| {
            let completion = cx
                .background_spawn(async move {
                    let completion = razer_service::earbuds_auto_pause::device(
                        lease.route().clone(),
                        operation,
                        worker_active,
                    );
                    drop(lease);
                    completion
                })
                .await;
            let _ = shell.update(cx, |shell, cx| {
                let current = shell.devices.contains(&workspace)
                    && workspace.read(cx).connection_revision() == connection
                    && workspace.read(cx).identity(cx) == identity
                    && shell
                        .device_observations
                        .iter()
                        .any(|route| route.matches(workspace.read(cx).device(cx)));
                let result = match (completion.result, completion.cleanup_failure) {
                    (Ok(report), None) => Ok(report),
                    (Ok(mut report), Some(error)) => {
                        report.write = razer_device::earbuds_protocol40::WriteOutcome::Failed(
                            format!("Earbud power cleanup failed: {error}"),
                        );
                        Ok(report)
                    }
                    (Err(error), None) => Err(error),
                    (Err(error), Some(cleanup)) => Err(format!("{error}; cleanup: {cleanup}")),
                };
                let result = if current {
                    result
                } else {
                    Err(format!(
                        "Earbud power connection changed during the operation{}",
                        result
                            .err()
                            .map(|error| format!("; {error}"))
                            .unwrap_or_default()
                    ))
                };
                page.update(cx, |page, cx| {
                    page.finish_earbuds_power(&request, result, cx)
                });
                shell.earbuds_power.workers.remove(&id);
                shell.finish_earbuds_power_lane(key, cx);
            });
        });
        self.earbuds_power.workers.insert(id, (active, task));
    }
}
