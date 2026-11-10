//! Device-global Sound actions do not depend on a usable local profile.
use super::earbuds_auto_pause::EarbudsAutoPauseRequested;
use super::*;
use razer_device::earbuds_auto_pause::{Operation, Report};
use razer_device::earbuds_protocol40::WriteOutcome;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use uuid::Uuid;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Action {
    InputSource,
    ActivationCode,
}
impl Action {
    pub(super) fn operation(self) -> Operation {
        match self {
            Self::InputSource => Operation::SwitchInputSource,
            Self::ActivationCode => Operation::ActivationCode,
        }
    }
}

#[derive(Clone, Copy)]
struct Queued {
    action: Action,
    activation_generation: u64,
}

#[derive(Default)]
pub(super) struct State {
    identity: String,
    connection: u64,
    active: bool,
    scope: Option<Uuid>,
    scope_guard: Option<Arc<AtomicBool>>,
    pub(super) pending: Option<EarbudsAutoPauseRequested>,
    pending_action: Option<Queued>,
    queue: VecDeque<Queued>,
    activation_generation: u64,
    pub(super) input_popup: bool,
    pub(super) input_disabled_cursor: Option<Point<Pixels>>,
    pub(super) input_result: Option<Result<WriteOutcome, String>>,
}

impl State {
    pub(super) fn cancel(&mut self) {
        if let Some(guard) = self.scope_guard.take() {
            guard.store(false, Ordering::Release);
        }
        if let Some(request) = &self.pending {
            request.guard().store(false, Ordering::Release);
        }
        if self.pending.is_none() {
            self.pending_action = None;
        }
        self.queue.clear();
        self.scope = None;
        self.input_popup = false;
        self.input_disabled_cursor = None;
        self.input_result = None;
        self.activation_generation = self.activation_generation.wrapping_add(1);
    }
    pub(super) fn stop(&mut self) {
        self.cancel();
        self.active = false;
    }
    pub(super) fn active(&self) -> bool {
        self.active
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.cancel();
    }
}

impl AudioProductWorkspace {
    pub fn set_earbuds_sound_context(
        &mut self,
        device: &razer_model::model::Device,
        active: bool,
        connection: u64,
        cx: &mut Context<Self>,
    ) {
        if !razer_device::earbuds_protocol40::supports_auto_pause(self.spec.product_id) {
            return;
        }
        let active = active && device.product_id == self.spec.product_id;
        let identity = device.workspace_identity();
        if self.earbuds_sound.identity != identity
            || self.earbuds_sound.connection != connection
            || self.earbuds_sound.active != active
            || !active
        {
            self.earbuds_sound.cancel();
            self.earbuds_activation.clear_device();
        }
        self.earbuds_sound.identity = identity;
        self.earbuds_sound.connection = connection;
        self.earbuds_sound.active = active;
        cx.notify();
    }

    pub(super) fn request_earbuds_sound_action(&mut self, action: Action, cx: &mut Context<Self>) {
        if !self.earbuds_sound.active {
            return;
        }
        if action == Action::InputSource && !self.earbuds_connection.ready() {
            return;
        }
        if action == Action::ActivationCode {
            self.earbuds_sound.activation_generation =
                self.earbuds_sound.activation_generation.wrapping_add(1);
        }
        let queued = Queued {
            action,
            activation_generation: self.earbuds_sound.activation_generation,
        };
        self.earbuds_sound
            .queue
            .retain(|queued| queued.action != action);
        self.earbuds_sound.queue.push_back(queued);
        self.start_earbuds_sound_action(cx);
    }

    fn start_earbuds_sound_action(&mut self, cx: &mut Context<Self>) {
        if !self.earbuds_sound.active || self.earbuds_sound.pending.is_some() {
            return;
        }
        while let Some(queued) = self.earbuds_sound.queue.pop_front() {
            if queued.action == Action::InputSource && !self.earbuds_connection.ready() {
                self.earbuds_sound.input_result =
                    Some(Err("Earbud radio changed before input switching".into()));
                continue;
            }
            if self.earbuds_sound.scope.is_none() {
                self.earbuds_sound.scope = Some(Uuid::new_v4());
                self.earbuds_sound.scope_guard = Some(Arc::new(AtomicBool::new(true)));
            }
            if queued.action == Action::InputSource {
                self.earbuds_sound.input_result = None;
            }
            let request = EarbudsAutoPauseRequested::device_action(
                self.earbuds_sound.scope.expect("active Sound scope"),
                self.earbuds_sound
                    .scope_guard
                    .as_ref()
                    .expect("active Sound owner")
                    .clone(),
                queued.action,
            );
            self.earbuds_sound.pending = Some(request.clone());
            self.earbuds_sound.pending_action = Some(queued);
            cx.emit(request);
            break;
        }
    }

    pub(super) fn earbuds_sound_request_current(
        &self,
        request: &EarbudsAutoPauseRequested,
    ) -> bool {
        self.earbuds_sound.active
            && request.device_scope() == self.earbuds_sound.scope
            && request.guard().load(Ordering::Acquire)
            && self
                .earbuds_sound
                .scope_guard
                .as_ref()
                .is_some_and(|guard| guard.load(Ordering::Acquire))
            && self
                .earbuds_sound
                .pending
                .as_ref()
                .is_some_and(|pending| pending.id() == request.id())
    }

    pub(super) fn finish_earbuds_sound_action(
        &mut self,
        request: &EarbudsAutoPauseRequested,
        result: Result<Report, String>,
        cx: &mut Context<Self>,
    ) {
        if !self
            .earbuds_sound
            .pending
            .as_ref()
            .is_some_and(|pending| pending.id() == request.id())
        {
            return;
        }
        let current = self.earbuds_sound_request_current(request);
        self.earbuds_sound.pending = None;
        let queued = self
            .earbuds_sound
            .pending_action
            .take()
            .expect("Sound request action");
        if current {
            let result = result.and_then(|report| {
                if report.product_id != self.spec.product_id
                    || report.operation != request.operation()
                {
                    return Err("Earbud Sound response belongs to another request".into());
                }
                Ok(report)
            });
            match queued.action {
                Action::InputSource => {
                    self.earbuds_sound.input_result =
                        Some(result.and_then(|report| match report.write {
                            WriteOutcome::Confirmed => Ok(WriteOutcome::Confirmed),
                            WriteOutcome::Failed(error) => Err(error),
                            _ => Err("Input source reply omitted its write confirmation".into()),
                        }));
                }
                Action::ActivationCode
                    if queued.activation_generation == self.earbuds_sound.activation_generation =>
                {
                    self.earbuds_activation.code = Some(result.and_then(|report| {
                        if let WriteOutcome::Failed(error) = report.write {
                            return Err(error);
                        }
                        report
                            .activation_code
                            .ok_or_else(|| "Activation response omitted the code".into())
                    }));
                    self.earbuds_activation.loading = false;
                }
                Action::ActivationCode => {}
            }
        } else if self.earbuds_sound.active && request.device_scope() == self.earbuds_sound.scope {
            let error = result.err().unwrap_or_else(|| {
                "Earbud Sound operation was cancelled; its response is not current".into()
            });
            match queued.action {
                Action::InputSource => self.earbuds_sound.input_result = Some(Err(error)),
                Action::ActivationCode
                    if queued.activation_generation == self.earbuds_sound.activation_generation =>
                {
                    self.earbuds_activation.code = Some(Err(error));
                    self.earbuds_activation.loading = false;
                }
                Action::ActivationCode => {}
            }
        }
        self.start_earbuds_sound_action(cx);
        cx.notify();
    }

    pub(super) fn cancel_earbuds_activation(&mut self) {
        self.earbuds_activation.cancel();
        self.earbuds_activation.loading = false;
        self.earbuds_sound.activation_generation =
            self.earbuds_sound.activation_generation.wrapping_add(1);
        self.earbuds_sound
            .queue
            .retain(|queued| queued.action != Action::ActivationCode);
        if let Some(request) = self
            .earbuds_sound
            .pending
            .as_ref()
            .filter(|request| request.operation() == Operation::ActivationCode)
        {
            request.guard().store(false, Ordering::Release);
        }
    }
}
