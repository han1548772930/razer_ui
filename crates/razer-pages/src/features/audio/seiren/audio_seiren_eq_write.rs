//! Local storage, optimistic drafts and actual device readbacks have separate owners.
use super::*;
use razer_device::seiren_eq::{Change, Report};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone)]
pub struct SeirenEqRequested {
    owner: EntityId,
    epoch: u64,
    sequence: u64,
    settings: Value,
    change: Option<Change>,
    persist_before_device: bool,
    valid: Arc<AtomicBool>,
}
impl EventEmitter<SeirenEqRequested> for AudioProductWorkspace {}
impl SeirenEqRequested {
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn settings(&self) -> &Value {
        &self.settings
    }
    pub fn change(&self) -> Option<&Change> {
        self.change.as_ref()
    }
    pub fn persists_local_settings(&self) -> bool {
        self.persist_before_device
    }
    pub fn is_valid(&self) -> bool {
        self.valid.load(Ordering::Acquire)
    }
    pub fn guard(&self) -> Arc<AtomicBool> {
        self.valid.clone()
    }
    fn cancel(&self) {
        self.valid.store(false, Ordering::Release);
    }
}
#[derive(Default)]
pub(super) struct State {
    epoch: u64,
    sequence: u64,
    pending: Option<SeirenEqRequested>,
    queued: VecDeque<SeirenEqRequested>,
    observed: Option<Report>,
    error: Option<String>,
}
impl State {
    fn enqueue(&mut self, request: SeirenEqRequested) -> Option<SeirenEqRequested> {
        if self.pending.is_none() {
            self.pending = Some(request.clone());
            return Some(request);
        }
        let key = request.change.as_ref().map(Change::key);
        if let Some(index) = self
            .queued
            .iter()
            .position(|old| old.change.as_ref().map(Change::key) == key)
        {
            self.queued.remove(index).expect("queued request").cancel();
        }
        // Preset and band tasks share source state. Keep surviving requests in
        // commit order so a newer band cannot move before a preset reset, and
        // the serialized profile snapshot never regresses another EQ field.
        self.queued.push_back(request);
        None
    }
    fn cancel(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
        if let Some(pending) = &self.pending {
            pending.cancel();
        }
        for queued in self.queued.drain(..) {
            queued.cancel();
        }
        self.observed = None;
        self.error = None;
        // Keep the cancelled serialization slot until its worker actually ends.
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.cancel();
    }
}
impl AudioProductWorkspace {
    pub fn cancel_seiren_eq_profile(&mut self) {
        self.seiren_eq_write.cancel();
    }
    pub fn cancel_seiren_eq_connection(&mut self) {
        self.seiren_eq_write.cancel();
        self.seiren_eq.live = razer_device::seiren_mixer::LiveState::disconnected();
    }
    pub fn request_seiren_volume_read(&mut self, cx: &mut Context<Self>) {
        if matches!(self.spec.product_id, 1422 | 1446) {
            self.submit_seiren_eq_change(Change::ReadVolumes, cx);
        }
    }
    /// Dedicated device restore for verified user profile-change callers.
    /// Constructing or locally discarding a workspace must not call this path.
    pub fn request_seiren_profile_restore(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.spec.product_id, 1422 | 1446) {
            return;
        }
        let changes = (|| -> anyhow::Result<Vec<Change>> {
            let active = self
                .seiren_eq_active_payload()
                .ok_or_else(|| anyhow::anyhow!("Active local Seiren EQ preset is absent"))?;
            let mut changes = vec![
                Change::from_source("ON_SET_ACTIVE_PARAMETRIC_EQ", active.clone())?,
                Change::from_source("ON_SET_HIGH_PASS_FILTER", active["highPassFilter"].clone())?,
            ];
            // GenericFeatureManager restores feature/event insertion order:
            // ParametricEQ, Marie_FW effects, DialControl, MicAGC, Audio_StreamMixer.
            for (field, operation) in [
                ("mic_deesser", "ON_SET_MIC_DEESSER"),
                ("vocal_bass", "ON_SET_VOCAL_BASS"),
                ("vocal_exciter", "ON_SET_VOCAL_EXCITER"),
            ] {
                if let Some(effect) = self.draft.pointer(&format!("/device/eqEffects/{field}")) {
                    changes.push(Change::from_source(operation, effect.clone())?);
                }
            }
            if let Some(dial) = self.draft.pointer("/profile/dialControl") {
                let Change::DialControl(value) =
                    Change::from_source("ON_SET_DIAL_CONTROL", dial.clone())?
                else {
                    unreachable!("typed dial payload");
                };
                changes.push(Change::RestoreDialControl(value));
            }
            if let Some(agc) = self.draft.pointer("/device/micAgc") {
                changes.push(Change::from_source("ON_SET_MIC_AGC", agc.clone())?);
            }
            // The three aliases share the source Bs once-per-restore guard.
            // Restore the global monitor value once; NoSave mic/HP volumes are
            // read by runtime initialization and never replayed from a profile.
            if let Some(monitor) = self.draft.pointer("/device/micSideTone") {
                changes.push(Change::from_source(
                    "ON_SET_MIC_SIDE_TONE",
                    monitor.clone(),
                )?);
            }
            for change in &changes {
                razer_device::seiren_eq::validate_change(self.spec.product_id, change)?;
            }
            Ok(changes)
        })();
        match changes {
            Ok(changes) => {
                for change in changes {
                    self.submit_seiren_eq_request(change, false, cx);
                }
            }
            Err(error) => {
                self.seiren_eq_write.error = Some(format!("{error:#}"));
                cx.notify();
            }
        }
    }
    pub(super) fn submit_seiren_eq_payload(
        &mut self,
        operation: &str,
        mut payload: Value,
        cx: &mut Context<Self>,
    ) {
        if operation == "ON_SET_ACTIVE_PARAMETRIC_EQ" {
            let target = if payload.get("parametricEQ").is_some() {
                &mut payload["parametricEQ"]
            } else {
                &mut payload
            };
            for (source, target_key) in [
                ("vocal_bass", "vocalBass"),
                ("vocal_exciter", "vocalExciter"),
            ] {
                if target.get(target_key).is_none() {
                    if let Some(effect) = self.draft.pointer(&format!("/device/eqEffects/{source}"))
                    {
                        target[target_key] =
                            json!({"isEnabled":effect["isEnabled"],"value":effect["value"]});
                    }
                }
            }
        }
        match Change::from_source(operation, payload) {
            Ok(change) => self.submit_seiren_eq_change(change, cx),
            Err(error) => {
                self.seiren_eq_write.error = Some(format!("{error:#}"));
                cx.notify();
            }
        }
    }
    pub(super) fn submit_seiren_eq_change(&mut self, change: Change, cx: &mut Context<Self>) {
        let persist = change.persists_local_settings();
        self.submit_seiren_eq_request(change, persist, cx);
    }
    fn submit_seiren_eq_request(
        &mut self,
        change: Change,
        persist_before_device: bool,
        cx: &mut Context<Self>,
    ) {
        if let Err(error) = razer_device::seiren_eq::validate_change(self.spec.product_id, &change)
        {
            self.seiren_eq_write.error = Some(format!("{error:#}"));
            cx.notify();
            return;
        }
        self.seiren_eq_write.sequence = self.seiren_eq_write.sequence.wrapping_add(1);
        self.seiren_eq_write.error = None;
        let request = SeirenEqRequested {
            owner: cx.entity_id(),
            epoch: self.seiren_eq_write.epoch,
            sequence: self.seiren_eq_write.sequence,
            settings: self.snapshot(),
            change: Some(change),
            persist_before_device,
            valid: Arc::new(AtomicBool::new(true)),
        };
        if let Some(request) = self.seiren_eq_write.enqueue(request) {
            cx.emit(request);
        }
    }
    pub fn seiren_eq_request_matches(&self, request: &SeirenEqRequested, _cx: &App) -> bool {
        request.is_valid()
            && request.epoch == self.seiren_eq_write.epoch
            && self
                .seiren_eq_write
                .pending
                .as_ref()
                .is_some_and(|pending| {
                    pending.owner == request.owner
                        && pending.sequence == request.sequence
                        && pending.epoch == request.epoch
                        && Arc::ptr_eq(&pending.valid, &request.valid)
                })
    }
    pub fn finish_seiren_eq(
        &mut self,
        request: &SeirenEqRequested,
        observed: Option<Report>,
        cx: &mut Context<Self>,
    ) {
        if !self
            .seiren_eq_write
            .pending
            .as_ref()
            .is_some_and(|pending| {
                pending.owner == request.owner
                    && pending.sequence == request.sequence
                    && pending.epoch == request.epoch
                    && Arc::ptr_eq(&pending.valid, &request.valid)
            })
        {
            return;
        }
        if self.seiren_eq_request_matches(request, cx) {
            if let Some(levels) = observed.as_ref().and_then(Report::native_levels) {
                self.record_seiren_native_levels(levels, cx);
            }
            self.seiren_eq_write.observed = observed;
        }
        self.seiren_eq_write
            .pending
            .take()
            .expect("matching task")
            .cancel();
        while let Some(next) = self.seiren_eq_write.queued.pop_front() {
            if next.is_valid() && next.epoch == self.seiren_eq_write.epoch {
                self.seiren_eq_write.pending = Some(next.clone());
                cx.emit(next);
                break;
            }
            next.cancel();
        }
        cx.notify();
    }
    pub fn prepare_seiren_eq_exit(&mut self, _cx: &App) -> Option<SeirenEqRequested> {
        let latest = self
            .seiren_eq_write
            .queued
            .iter()
            .rev()
            .find(|request| request.persists_local_settings())
            .cloned()
            .or_else(|| {
                self.seiren_eq_write
                    .pending
                    .clone()
                    .filter(SeirenEqRequested::persists_local_settings)
            });
        let old_epoch = self.seiren_eq_write.epoch;
        self.seiren_eq_write.cancel();
        self.seiren_eq_write.pending = None;
        let mut latest = latest?;
        if latest.epoch != old_epoch {
            return None;
        }
        self.seiren_eq_write.sequence = self.seiren_eq_write.sequence.wrapping_add(1);
        latest.epoch = self.seiren_eq_write.epoch;
        latest.sequence = self.seiren_eq_write.sequence;
        latest.settings = self.snapshot();
        latest.change = None;
        latest.valid = Arc::new(AtomicBool::new(true));
        self.seiren_eq_write.pending = Some(latest.clone());
        Some(latest)
    }

    pub fn sync_seiren_audio_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.sync_seiren_eq(window, cx);
        self.sync_seiren_stream_mixer(window, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(sequence: u64, change: Change) -> SeirenEqRequested {
        SeirenEqRequested {
            owner: EntityId::from(1),
            epoch: 0,
            sequence,
            settings: json!({}),
            change: Some(change),
            persist_before_device: true,
            valid: Arc::new(AtomicBool::new(true)),
        }
    }
    #[::core::prelude::v1::test]
    fn queue_retains_other_effects_and_cancels_replaced_amounts() {
        let mut state = State::default();
        state.enqueue(request(
            1,
            Change::HighPass(razer_device::seiren_eq::Effect::new(true, 3)),
        ));
        let first = request(
            2,
            Change::VocalBass(razer_device::seiren_eq::Effect::new(true, 20)),
        );
        state.enqueue(first.clone());
        state.enqueue(request(
            3,
            Change::DeEsser(razer_device::seiren_eq::Effect::new(true, 40)),
        ));
        state.enqueue(request(
            4,
            Change::VocalBass(razer_device::seiren_eq::Effect::new(true, 80)),
        ));
        assert!(!first.is_valid());
        assert_eq!(state.queued.len(), 2);
        assert_eq!(state.queued[0].sequence(), 3);
        assert_eq!(state.queued[1].sequence(), 4);
    }
    #[::core::prelude::v1::test]
    fn cancellation_retains_inflight_slot_and_drop_invalidates_all_owned_requests() {
        let mut state = State::default();
        let pending = request(
            1,
            Change::HighPass(razer_device::seiren_eq::Effect::new(true, 3)),
        );
        state.enqueue(pending.clone());
        state.cancel();
        assert!(!pending.is_valid());
        assert!(state.pending.is_some());
        let queued = request(
            2,
            Change::VocalBass(razer_device::seiren_eq::Effect::new(true, 80)),
        );
        state.enqueue(queued.clone());
        drop(state);
        assert!(!queued.is_valid());
    }
}
