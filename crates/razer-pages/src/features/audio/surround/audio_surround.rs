//! Retained Surround interaction state, shared by the current 1392/1442/3942 roots.
use super::*;
use razer_device::audio_surround::{
    Change, Channel, Delay, Endpoint, Equalizer, Identity, Product, Volume,
    observed::ObservationReply, presentation::Controls,
};
use razer_device::simple_audio_volume::AudioVolumeReadResult;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[path = "audio_surround_tutorial.rs"]
mod tutorial;
#[path = "audio_surround_view.rs"]
mod view;

#[derive(Debug, Clone)]
pub enum SurroundOperation {
    Open,
    Change(Change),
    Master { mute: bool, volume: u8 },
}
#[derive(Clone)]
pub struct SurroundRequest {
    identity: Identity,
    alive: Arc<AtomicBool>,
    revision: u64,
    operation: SurroundOperation,
}
impl SurroundRequest {
    pub fn identity(&self) -> &Identity {
        &self.identity
    }
    pub fn guard(&self) -> Arc<AtomicBool> {
        self.alive.clone()
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn operation(&self) -> &SurroundOperation {
        &self.operation
    }
    pub fn is_current(&self) -> bool {
        self.alive.load(Ordering::Acquire)
    }
}

pub struct SurroundObservation {
    reply: ObservationReply,
    endpoints: Vec<Endpoint>,
    master: Option<AudioVolumeReadResult>,
    failure: Option<String>,
}
impl SurroundObservation {
    pub fn new(
        reply: ObservationReply,
        endpoints: Vec<Endpoint>,
        master: Option<AudioVolumeReadResult>,
        failure: Option<String>,
    ) -> Self {
        Self {
            reply,
            endpoints,
            master,
            failure,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Control {
    Master,
    FrontVolume,
    RearVolume,
    Delay,
    FrontLow,
    FrontMid,
    FrontHigh,
    RearLow,
    RearMid,
    RearHigh,
}
impl Control {
    fn limits(self) -> (f32, f32) {
        match self {
            Self::Master | Self::FrontVolume | Self::RearVolume => (0., 100.),
            Self::Delay => (-100., 100.),
            _ => (-6., 6.),
        }
    }
    fn eq(self) -> Option<(Channel, usize)> {
        match self {
            Self::FrontLow => Some((Channel::Front, 0)),
            Self::FrontMid => Some((Channel::Front, 1)),
            Self::FrontHigh => Some((Channel::Front, 2)),
            Self::RearLow => Some((Channel::Rear, 0)),
            Self::RearMid => Some((Channel::Rear, 1)),
            Self::RearHigh => Some((Channel::Rear, 2)),
            _ => None,
        }
    }
}

#[derive(Default)]
pub(super) struct State {
    identity: Option<Identity>,
    alive: Option<Arc<AtomicBool>>,
    generation: u64,
    revision: u64,
    controls: Controls,
    connected: bool,
    endpoints: Vec<Endpoint>,
    master: Option<AudioVolumeReadResult>,
    failure: Option<String>,
    pending: BTreeMap<u64, SurroundOperation>,
    preview: BTreeMap<Control, SliderEdit>,
    slider_tasks: BTreeMap<Control, Task<()>>,
    sliders: BTreeMap<Control, Entity<SliderState>>,
    select: Option<Entity<SelectState<Vec<Choice>>>>,
    selected: Option<String>,
    choosing_front: bool,
    signal_requested: bool,
    tutorial: Option<usize>,
    syncing: bool,
}
#[derive(Clone, Copy)]
struct SliderEdit {
    value: f32,
    submitted: Option<u64>,
}
impl State {
    fn actual(&self, control: Control) -> Option<f32> {
        if !self.connected {
            return None;
        }
        match control {
            Control::Master => self.master.as_ref().map(|m| f32::from(m.volume)),
            Control::FrontVolume => self
                .controls
                .volume(Channel::Front)
                .value()
                .map(|v| f32::from(v.percent())),
            Control::RearVolume => self
                .controls
                .volume(Channel::Rear)
                .value()
                .map(|v| f32::from(v.percent())),
            Control::Delay => self
                .controls
                .delay()
                .value()
                .map(|v| f32::from(v.milliseconds())),
            _ => control.eq().and_then(|(channel, band)| {
                self.controls
                    .equalizer(channel)
                    .value()
                    .map(|v| f32::from(v.bands()[band]))
            }),
        }
    }
    fn accept(
        &mut self,
        identity: &Identity,
        completed: Option<u64>,
        result: Result<SurroundObservation, String>,
    ) -> Option<bool> {
        if self.identity.as_ref() != Some(identity)
            || result
                .as_ref()
                .is_ok_and(|value| value.reply.identity() != identity)
        {
            return None;
        }
        if let Some(revision) = completed {
            self.pending.remove(&revision);
            self.preview
                .retain(|_, edit| edit.submitted != Some(revision));
        }
        let previous_endpoints = self.endpoints.clone();
        let previous_values = self
            .preview
            .keys()
            .map(|control| (*control, self.actual(*control)))
            .collect::<BTreeMap<_, _>>();
        match result {
            Ok(observation) => {
                let service = observation.reply.service();
                self.connected = service.connected;
                self.controls = Controls::from_service(service);
                self.master = observation
                    .master
                    .filter(|v| v.result && v.volume <= 100 && service.connected);
                self.failure = observation.failure.or_else(|| service.failure.clone());
                self.endpoints = if service.connected {
                    observation.endpoints
                } else {
                    Vec::new()
                };
            }
            Err(error) => {
                self.controls = Controls::default();
                self.connected = false;
                self.master = None;
                self.endpoints.clear();
                self.failure = Some(error);
            }
        }
        if !self.connected {
            self.preview.clear();
            self.slider_tasks.clear();
            self.pending.clear();
            self.selected = None;
            self.choosing_front = false;
            self.signal_requested = false;
        } else {
            // The source slider's curValue effect replaces its temporary state
            // when the incoming value changes, including during a drag.
            let changed = previous_values
                .into_iter()
                .filter_map(|(control, value)| {
                    (value != self.actual(control) || value.is_none()).then_some(control)
                })
                .collect::<Vec<_>>();
            for control in changed {
                self.preview.remove(&control);
            }
        }
        Some(previous_endpoints != self.endpoints)
    }
}
impl Drop for State {
    fn drop(&mut self) {
        if let Some(alive) = &self.alive {
            alive.store(false, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use razer_device::audio_surround::observed::ServiceObservation;
    use razer_device::audio_surround::protocol::{
        EndpointProperties, EndpointVolumeState, EndpointsState,
    };
    use std::prelude::v1::test;

    fn identity(generation: u64) -> Identity {
        Identity::new(
            Product::Clio,
            "physical-speaker".into(),
            "page".into(),
            generation,
        )
        .unwrap()
    }
    fn observation(id: &Identity, connected: bool) -> SurroundObservation {
        let mut service = ServiceObservation::default();
        service.connected = connected;
        service.front_endpoint = Some("front".into());
        service.observations.endpoints = Some(EndpointsState {
            state_header: None,
            endpoints_properties: vec![EndpointProperties {
                endpoint_id: "front".into(),
                friendly_name: "Front".into(),
                thx_endpoint_id: 0,
                form_factor: 1,
                wave_format_extensible: Vec::new(),
            }],
        });
        service.observations.volumes.insert(
            "front".into(),
            EndpointVolumeState {
                state_header: None,
                volume: 0.63,
                muted: false,
            },
        );
        SurroundObservation::new(
            ObservationReply::new(id.clone(), service),
            vec![Endpoint::new("candidate".into(), "Front speaker".into()).unwrap()],
            Some(AudioVolumeReadResult {
                result: true,
                reason: String::new(),
                device_id: "master".into(),
                muted: true,
                volume: 37,
            }),
            None,
        )
    }
    fn state() -> State {
        let mut state = State::default();
        state.identity = Some(identity(2));
        state.accept(&identity(2), None, Ok(observation(&identity(2), true)));
        state.pending.insert(
            10,
            SurroundOperation::Master {
                mute: false,
                volume: 80,
            },
        );
        state.preview.insert(
            Control::Master,
            SliderEdit {
                value: 80.,
                submitted: Some(10),
            },
        );
        state
    }
    #[test]
    fn surround_replaced_owner_and_wrong_reply_do_not_complete_current_edits() {
        let mut state = state();
        assert!(
            state
                .accept(&identity(1), Some(10), Ok(observation(&identity(1), true)))
                .is_none()
        );
        assert!(
            state
                .accept(&identity(2), Some(10), Ok(observation(&identity(1), true)))
                .is_none()
        );
        assert!(state.pending.contains_key(&10));
        assert_eq!(state.preview[&Control::Master].value, 80.);
    }
    #[test]
    fn surround_completion_preserves_other_drag_and_uses_real_readback() {
        let mut state = state();
        state.preview.insert(
            Control::FrontVolume,
            SliderEdit {
                value: 32.,
                submitted: None,
            },
        );
        assert_eq!(
            state.accept(&identity(2), Some(10), Ok(observation(&identity(2), true))),
            Some(false)
        );
        assert!(!state.preview.contains_key(&Control::Master));
        assert_eq!(state.preview[&Control::FrontVolume].value, 32.);
        assert_eq!(state.master.as_ref().unwrap().volume, 37);
        assert!(state.master.as_ref().unwrap().muted);
    }
    #[test]
    fn surround_old_completion_does_not_clear_a_new_drag_on_same_control() {
        let mut state = state();
        state.preview.insert(
            Control::Master,
            SliderEdit {
                value: 51.,
                submitted: None,
            },
        );
        state.accept(&identity(2), Some(10), Ok(observation(&identity(2), true)));
        assert_eq!(state.preview[&Control::Master].value, 51.);
    }
    #[test]
    fn surround_changed_incoming_value_overrides_drag_as_source_cur_value_effect() {
        let mut state = state();
        state.preview.insert(
            Control::Master,
            SliderEdit {
                value: 51.,
                submitted: None,
            },
        );
        let mut incoming = observation(&identity(2), true);
        incoming.master.as_mut().unwrap().volume = 24;
        state.accept(&identity(2), None, Ok(incoming));
        assert!(!state.preview.contains_key(&Control::Master));
        assert_eq!(state.actual(Control::Master), Some(24.));
    }
    #[test]
    fn surround_failure_and_disconnect_clear_temporaries_without_fallback() {
        for result in [
            Err("Read failed".into()),
            Ok(observation(&identity(2), false)),
        ] {
            let mut state = state();
            state.endpoints = vec![Endpoint::new("old".into(), "Old speaker".into()).unwrap()];
            assert_eq!(state.accept(&identity(2), None, result), Some(true));
            assert!(!state.connected);
            assert!(state.master.is_none());
            assert!(state.preview.is_empty());
            assert!(state.pending.is_empty());
            assert!(state.endpoints.is_empty());
            assert_eq!(state.controls, Controls::default());
        }
    }
}

impl EventEmitter<SurroundRequest> for AudioProductWorkspace {}
impl AudioProductWorkspace {
    pub(super) fn initialize_surround(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if Product::try_from(self.spec.product_id).is_err() {
            return;
        }
        if let Some(demo) = &self.demo {
            self.subscriptions
                .push(cx.observe(demo, |_, _, cx| cx.notify()));
        }
        for control in [
            Control::Master,
            Control::FrontVolume,
            Control::RearVolume,
            Control::Delay,
            Control::FrontLow,
            Control::FrontMid,
            Control::FrontHigh,
            Control::RearLow,
            Control::RearMid,
            Control::RearHigh,
        ] {
            let (min, max) = control.limits();
            let state = cx.new(|_| SliderState::new().min(min).max(max).step(1.));
            self.subscriptions.push(cx.subscribe(
                &state,
                move |this, _, event: &SliderEvent, cx| {
                    if this.surround.syncing {
                        return;
                    }
                    match event {
                        SliderEvent::Change(v) => {
                            this.surround.preview.insert(
                                control,
                                SliderEdit {
                                    value: v.start(),
                                    submitted: None,
                                },
                            );
                            let debounce = match control {
                                Control::Master => Some(50),
                                Control::FrontVolume | Control::RearVolume => Some(120),
                                _ => None,
                            };
                            if let Some(milliseconds) = debounce {
                                this.queue_surround_slider(control, v.start(), milliseconds, cx);
                            }
                            cx.notify();
                        }
                        SliderEvent::Release(v) => {
                            this.submit_surround_slider(control, v.start(), cx)
                        }
                    }
                },
            ));
            self.surround.sliders.insert(control, state);
        }
        let select = cx.new(|cx| SelectState::new(Vec::<Choice>::new(), None, window, cx));
        self.subscriptions.push(cx.subscribe(
            &select,
            |this, _, event: &SelectEvent<Vec<Choice>>, cx| {
                if this.surround.syncing {
                    return;
                }
                if let SelectEvent::Confirm(Some(id)) = event {
                    this.surround.selected = Some(id.clone());
                    if !this.surround.choosing_front {
                        this.submit_surround_front(false, cx);
                    }
                    cx.notify();
                }
            },
        ));
        self.surround.select = Some(select);
    }
    pub fn set_surround_active(&mut self, active: bool, container: &str, cx: &mut Context<Self>) {
        let Ok(product) = Product::try_from(self.spec.product_id) else {
            return;
        };
        let same = self
            .surround
            .identity
            .as_ref()
            .is_some_and(|id| id.container_id() == container);
        if active && same {
            return;
        }
        if let Some(alive) = self.surround.alive.take() {
            alive.store(false, Ordering::Release);
        }
        self.surround.identity = None;
        self.surround.controls = Controls::default();
        self.surround.connected = false;
        self.surround.master = None;
        self.surround.endpoints.clear();
        self.surround.preview.clear();
        self.surround.slider_tasks.clear();
        self.surround.pending.clear();
        self.surround.choosing_front = false;
        self.surround.signal_requested = false;
        self.surround.failure = None;
        if active {
            self.surround.generation = self.surround.generation.wrapping_add(1);
            match Identity::new(
                product,
                container.to_owned(),
                format!("surround-page-{}", cx.entity_id()),
                self.surround.generation,
            ) {
                Ok(identity) => {
                    self.surround.identity = Some(identity);
                    self.surround.alive = Some(Arc::new(AtomicBool::new(true)));
                    self.dispatch_surround(SurroundOperation::Open, cx);
                }
                Err(error) => self.surround.failure = Some(format!("{error:#}")),
            }
        }
        cx.notify();
    }
    pub fn surround_request_current(&self, request: &SurroundRequest) -> bool {
        request.is_current() && self.surround.identity.as_ref() == Some(request.identity())
    }
    fn dispatch_surround(&mut self, operation: SurroundOperation, cx: &mut Context<Self>) {
        let (Some(identity), Some(alive)) = (&self.surround.identity, &self.surround.alive) else {
            return;
        };
        if !alive.load(Ordering::Acquire) {
            return;
        }
        self.surround.revision = self.surround.revision.wrapping_add(1);
        let request = SurroundRequest {
            identity: identity.clone(),
            alive: alive.clone(),
            revision: self.surround.revision,
            operation: operation.clone(),
        };
        self.surround.pending.insert(request.revision, operation);
        cx.emit(request);
        cx.notify();
    }
    fn submit_surround_slider(&mut self, control: Control, value: f32, cx: &mut Context<Self>) {
        if self.surround.actual(control).is_none() {
            return;
        }
        let operation = match control {
            Control::Master => self
                .surround
                .master
                .as_ref()
                .map(|m| SurroundOperation::Master {
                    mute: m.muted,
                    volume: value as u8,
                }),
            Control::FrontVolume | Control::RearVolume => {
                Volume::new(value as u8).ok().map(|volume| {
                    SurroundOperation::Change(Change::Volume {
                        channel: if control == Control::FrontVolume {
                            Channel::Front
                        } else {
                            Channel::Rear
                        },
                        volume,
                    })
                })
            }
            Control::Delay => Delay::new(value as i16)
                .ok()
                .map(|delay| SurroundOperation::Change(Change::Delay(delay))),
            _ => control.eq().and_then(|(channel, band)| {
                let mut gains = self
                    .surround
                    .pending
                    .values()
                    .rev()
                    .find_map(|operation| match operation {
                        SurroundOperation::Change(Change::Equalizer {
                            channel: pending_channel,
                            equalizer,
                        }) if *pending_channel == channel => Some(equalizer.bands()),
                        _ => None,
                    })
                    .or_else(|| {
                        self.surround
                            .controls
                            .equalizer(channel)
                            .value()
                            .map(|eq| eq.bands())
                    })?;
                gains[band] = value as i8;
                Equalizer::new(gains).ok().map(|equalizer| {
                    SurroundOperation::Change(Change::Equalizer { channel, equalizer })
                })
            }),
        };
        if let Some(operation) = operation {
            self.dispatch_surround(operation, cx);
            self.surround.preview.insert(
                control,
                SliderEdit {
                    value,
                    submitted: Some(self.surround.revision),
                },
            );
        } else {
            self.surround.preview.remove(&control);
        }
    }
    fn queue_surround_slider(
        &mut self,
        control: Control,
        value: f32,
        milliseconds: u64,
        cx: &mut Context<Self>,
    ) {
        let identity = self.surround.identity.clone();
        let timer = cx
            .background_executor()
            .timer(std::time::Duration::from_millis(milliseconds));
        let task = cx.spawn(async move |this, cx| {
            timer.await;
            let _ = this.update(cx, |this, cx| {
                if this.surround.identity == identity {
                    this.submit_surround_slider(control, value, cx);
                }
            });
        });
        self.surround.slider_tasks.insert(control, task);
    }
    fn submit_surround_front(&mut self, enable: bool, cx: &mut Context<Self>) {
        let endpoint = self
            .surround
            .endpoints
            .iter()
            .find(|e| Some(e.id()) == self.surround.selected.as_deref())
            .cloned();
        if let Some(endpoint) = endpoint {
            self.dispatch_surround(
                SurroundOperation::Change(Change::FrontSpeaker(endpoint)),
                cx,
            );
            self.surround.choosing_front = false;
            if enable {
                self.dispatch_surround(SurroundOperation::Change(Change::Enabled(true)), cx);
            }
        }
    }
    pub(super) fn leave_surround_page(&mut self, cx: &mut Context<Self>) {
        if self.spec.product_id == 3942 {
            self.surround.tutorial = None;
        }
        self.surround.choosing_front = false;
        self.surround.signal_requested = false;
        if self.surround.identity.is_some() {
            self.dispatch_surround(SurroundOperation::Change(Change::TestSignal(false)), cx);
        }
    }
    pub fn finish_surround(
        &mut self,
        identity: &Identity,
        completed: Option<u64>,
        result: Result<SurroundObservation, String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(endpoints_changed) = self.surround.accept(identity, completed, result) else {
            return;
        };
        if endpoints_changed {
            self.surround.syncing = true;
            if let Some(select) = &self.surround.select {
                let items = self
                    .surround
                    .endpoints
                    .iter()
                    .map(|e| Choice::new(e.id(), e.name().to_owned()))
                    .collect();
                select.update(cx, |select, cx| select.set_items(items, window, cx));
            }
            self.surround.syncing = false;
        }
        self.sync_surround_controls(window, cx);
        cx.notify();
    }
    fn surround_value(&self, control: Control) -> Option<f32> {
        let actual = self.surround.actual(control)?;
        Some(
            self.surround
                .preview
                .get(&control)
                .map_or(actual, |edit| edit.value),
        )
    }
    fn sync_surround_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.surround.syncing = true;
        for (control, state) in &self.surround.sliders {
            if !self.surround.preview.contains_key(control) {
                if let Some(value) = self.surround_value(*control) {
                    state.update(cx, |state, cx| state.set_value(value, window, cx));
                }
            }
        }
        if !self.surround.choosing_front && self.surround.pending.is_empty() {
            self.surround.selected = self.surround.controls.front().map(|e| e.id().to_owned());
            if let Some(select) = &self.surround.select {
                select.update(cx, |select, cx| {
                    if let Some(id) = &self.surround.selected {
                        select.set_selected_value(id, window, cx);
                    } else {
                        select.set_selected_index(None, window, cx);
                    }
                });
            }
        }
        self.surround.syncing = false;
    }
}
