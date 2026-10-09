//! 1382 MapAudio state and controls. Source receipts: control-pod-audio-current-evidence.json.
use super::*;
use gpui_kit::base::Radio;
use gpui_kit::base::motion::{self, Easing, Transition};
use razer_ipc::ServiceClient;
use razer_ipc::ServiceRequest;
use razer_widgets::theme::ControlPodAudioColors as Colors;
use std::time::Duration;
#[path = "control_pod_audio_warning.rs"]
mod warning;

#[derive(Deserialize)]
struct OptionSpec {
    id: String,
    content: String,
}
#[derive(Deserialize)]
struct Data {
    modes: Vec<OptionSpec>,
    playback: Vec<OptionSpec>,
    equalizer: Vec<OptionSpec>,
    presets: Vec<OptionSpec>,
    inputs: Vec<Value>,
    labels: BTreeMap<String, String>,
}

/// Only current observed `commonReducer.validDevices` records belong here.
/// Catalogue, demo, HID and saved profile records are not this observation.
#[derive(Clone, Deserialize)]
#[serde(transparent)]
pub struct RuntimeAudioDevice(BTreeMap<String, Value>);
fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("control_pod_audio_data.json"))
            .expect("audited Control Pod audio")
    })
}

impl AudioProductWorkspace {
    pub fn pod_editor_dirty(&self, cx: &App) -> bool {
        self.pod_audio_editor
            .as_ref()
            .is_some_and(|(_, editor)| editor.read(cx).changed)
    }

    pub fn observe_pod_runtime_devices(
        &mut self,
        devices: Vec<RuntimeAudioDevice>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id != 1382 {
            return;
        }
        self.pod_runtime_devices = devices.clone();
        if let Some((_, editor)) = &self.pod_audio_editor {
            editor.update(cx, |editor, cx| {
                editor.valid_devices = devices;
                editor.project_equalizer_devices(window, cx);
                cx.notify();
            });
        }
    }
    pub fn defer_pod_navigation(
        &mut self,
        navigation: AudioNavigation,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some((_, editor)) = self.pod_audio_editor.clone() else {
            return false;
        };
        if editor.update(cx, |editor, cx| {
            editor.request_departure(Departure::Navigation(navigation), window, cx)
        }) {
            return true;
        }
        self.pod_audio_editor = None;
        self.pod_audio_subscription = None;
        cx.notify();
        false
    }
    pub(super) fn has_pod_audio_editor(&self, path: &str) -> bool {
        self.spec.product_id == 1382
            && path.ends_with("/outputType")
            && self.draft.pointer(path).is_some_and(|v| v == "audioGroup")
            && path
                .strip_prefix("/profile/podMappings/")
                .and_then(|p| p.strip_suffix("/outputType"))
                .is_some_and(|id| data().inputs.iter().any(|v| v["inputID"] == id))
    }
    pub(super) fn open_pod_audio(
        &mut self,
        path: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.has_pod_audio_editor(path) || !self.control(path).is_some_and(|c| self.enabled(c))
        {
            return;
        }
        if let Some((current_path, editor)) = &self.pod_audio_editor {
            if current_path == path {
                return;
            }
            if editor.clone().update(cx, |editor, cx| {
                editor.request_departure(Departure::Input(path.to_owned()), window, cx)
            }) {
                return;
            }
        }
        let base = path
            .strip_suffix("/outputType")
            .expect("checked mapping path");
        let input = base.rsplit('/').next().unwrap();
        let group_path = format!("{base}/audioGroup");
        let original = self
            .draft
            .pointer(&group_path)
            .cloned()
            .unwrap_or(json!({}));
        let baseline = original.clone();
        let assignment_path = path.to_owned();
        let devices = self.pod_runtime_devices.clone();
        let editor = cx.new(|cx| AudioEditor::new(input, original, devices, window, cx));
        self.pod_audio_subscription =
            Some(
                cx.subscribe_in(&editor, window, move |this, emitter, event, window, cx| {
                    if !this
                        .pod_audio_editor
                        .as_ref()
                        .is_some_and(|(_, current)| current.entity_id() == emitter.entity_id())
                    {
                        return;
                    }
                    let mut departure = None;
                    match event {
                        EditorEvent::Save(mapping) | EditorEvent::SaveThen(mapping, _) => {
                            // Preserve the native profile's other local assignment drafts.
                            // The source-shaped request is never sent to a device here.
                            if this.has_pod_audio_editor(&assignment_path)
                                && this
                                    .control(&assignment_path)
                                    .is_some_and(|c| this.enabled(c))
                                && this.draft.pointer(&group_path) == Some(&baseline)
                            {
                                this.edit(&group_path, mapping["audioGroup"].clone(), window, cx);
                                if let EditorEvent::SaveThen(_, destination) = event {
                                    departure = Some(destination.clone());
                                }
                            }
                        }
                        EditorEvent::Cancel => {}
                    }
                    this.pod_audio_editor = None;
                    this.pod_audio_subscription = None;
                    if let Some(destination) = departure {
                        match destination {
                            Departure::Input(path) => this.open_pod_audio(&path, window, cx),
                            Departure::Navigation(navigation) => cx.emit(navigation),
                        }
                    }
                    cx.notify();
                }),
            );
        self.pod_audio_editor = Some((path.into(), editor));
        cx.notify();
    }
    pub(super) fn restore_pod_audio(&mut self, saved: &Value) {
        // Generic merge_known only restores fields present in the initial schema;
        // advanced payloads are optional and must survive a local draft reload.
        for input in &data().inputs {
            let Some(id) = input["inputID"].as_str() else {
                continue;
            };
            let path = format!("/profile/podMappings/{id}/audioGroup");
            let Some(group) = saved.pointer(&path).filter(|g| g.is_object()) else {
                continue;
            };
            let valid_mode = data()
                .modes
                .iter()
                .any(|v| g_string(group, "audioMode") == Some(v.id.as_str()));
            let valid_action = data()
                .playback
                .iter()
                .chain(&data().equalizer)
                .any(|v| g_string(group, "audioAssignment") == Some(v.id.as_str()));
            if valid_mode && valid_action {
                if let Some(target) = self.draft.pointer_mut(&path) {
                    *target = group.clone();
                }
            }
        }
    }
}
fn g_string<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

pub(super) enum EditorEvent {
    Save(Value),
    SaveThen(Value, Departure),
    Cancel,
}
#[derive(Clone)]
pub(super) enum Departure {
    Input(String),
    Navigation(AudioNavigation),
}

pub(super) struct AudioEditor {
    input: Value,
    original: Value,
    mode: usize,
    playback: usize,
    equalizer: usize,
    preset: usize,
    first_id: Value,
    second_id: Value,
    first_device: Value,
    second_device: Value,
    speakers: Vec<Value>,
    valid_devices: Vec<RuntimeAudioDevice>,
    eq_speakers: Vec<Value>,
    has_eq_devices: bool,
    eq_device_index: usize,
    eq_effect_counts: Option<(usize, usize)>,
    can_save: bool,
    changed: bool,
    loading: bool,
    read_error: Option<String>,
    save_error: Option<String>,
    modes: Entity<SelectState<Vec<Choice>>>,
    presets: Entity<SelectState<Vec<Choice>>>,
    first: Entity<SelectState<Vec<DeviceChoice>>>,
    second: Entity<SelectState<Vec<DeviceChoice>>>,
    eq_devices: Entity<SelectState<Vec<DeviceChoice>>>,
    subscriptions: Vec<Subscription>,
    warning: Option<warning::WarningState>,
}
impl EventEmitter<EditorEvent> for AudioEditor {}

#[derive(Clone)]
struct DeviceChoice {
    key: String,
    name: SharedString,
    disabled: bool,
}
impl gpui_kit::component::select::SelectItem for DeviceChoice {
    type Value = String;
    fn title(&self) -> SharedString {
        self.name.clone()
    }
    fn value(&self) -> &String {
        &self.key
    }
    fn disabled(&self) -> bool {
        self.disabled
    }
}

impl AudioEditor {
    pub(super) fn new(
        input_id: &str,
        original: Value,
        valid_devices: Vec<RuntimeAudioDevice>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let input = data()
            .inputs
            .iter()
            .find(|v| v["inputID"] == input_id)
            .expect("source Control Pod input")
            .clone();
        let assignment = original["audioAssignment"].as_str().unwrap_or("");
        let playback = data().playback.iter().position(|v| v.id == assignment);
        let equalizer = data().equalizer.iter().position(|v| v.id == assignment);
        let mode = if equalizer.is_some() { 0 } else { 1 };
        let none = json!({"id": data().labels["x3E"], "name":"None", "content":"None"});
        let payload = if playback == Some(2) {
            &original["playbackPayload"]
        } else {
            &Value::Null
        };
        let first_id = payload.get("firstDeviceId").cloned().unwrap_or(json!(0));
        let second_id = payload
            .get("secondDeviceId")
            .filter(|v| truthy(v))
            .cloned()
            .unwrap_or(none["id"].clone());
        let first_device = payload
            .pointer("/playbackDevices/0")
            .cloned()
            .unwrap_or(json!({}));
        let second_device = payload
            .pointer("/playbackDevices/1")
            .cloned()
            .unwrap_or(none);
        let modes = cx.new(|cx| {
            SelectState::new(
                choices(&data().modes),
                Some(gpui_kit::component::IndexPath::new(mode)),
                window,
                cx,
            )
        });
        let presets = cx.new(|cx| {
            SelectState::new(
                choices(&data().presets),
                Some(gpui_kit::component::IndexPath::new(0)),
                window,
                cx,
            )
        });
        let first = cx.new(|cx| SelectState::new(Vec::<DeviceChoice>::new(), None, window, cx));
        let second = cx.new(|cx| SelectState::new(Vec::<DeviceChoice>::new(), None, window, cx));
        let eq_devices =
            cx.new(|cx| SelectState::new(Vec::<DeviceChoice>::new(), None, window, cx));
        let mut this = Self {
            input,
            original,
            mode,
            playback: playback.unwrap_or(0),
            equalizer: equalizer.unwrap_or(0),
            preset: 0,
            first_id,
            second_id,
            first_device,
            second_device,
            speakers: Vec::new(),
            valid_devices,
            eq_speakers: Vec::new(),
            has_eq_devices: false,
            eq_device_index: 0,
            eq_effect_counts: None,
            can_save: playback.is_none() && equalizer.is_none(),
            changed: false,
            loading: true,
            read_error: None,
            save_error: None,
            modes,
            presets,
            first,
            second,
            eq_devices,
            subscriptions: Vec::new(),
            warning: None,
        };
        this.subscriptions
            .push(cx.subscribe(&this.modes, |this, _, event, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    if let Some(mode) = data().modes.iter().position(|v| v.id == *value) {
                        // MapAudio's category callback uses the previous A value.
                        this.can_save = this.has_eq_devices || this.mode == 0;
                        this.mode = mode;
                        this.changed = true;
                        cx.notify();
                    }
                }
            }));
        this.subscriptions
            .push(cx.subscribe(&this.presets, |this, _, event, cx| {
                if let SelectEvent::Confirm(Some(value)) = event {
                    if let Some(preset) = data().presets.iter().position(|v| v.id == *value) {
                        this.preset = preset;
                        cx.notify();
                    }
                }
            }));
        this.subscriptions
            .push(cx.subscribe(&this.eq_devices, |this, _, event, cx| {
                if let SelectEvent::Confirm(Some(key)) = event {
                    if let Some(index) = this
                        .eq_speakers
                        .iter()
                        .position(|v| v["id"].to_string() == *key)
                    {
                        this.eq_device_index = index;
                        // Fe updates R only; it does not enable Save or set dirty.
                        cx.notify();
                    }
                }
            }));
        // The initial [w] effect runs even when w remains false.
        if this.mode == 0 {
            this.can_save = false;
            this.changed = true;
        }
        this.project_equalizer_devices(window, cx);
        for (first, state) in [(true, this.first.clone()), (false, this.second.clone())] {
            this.subscriptions
                .push(cx.subscribe(&state, move |this, _, event, cx| {
                    if let SelectEvent::Confirm(Some(key)) = event {
                        this.select_device(first, key, cx);
                    }
                }));
        }
        cx.spawn_in(window, async move |owner, cx| {
            let result = cx
                .background_spawn(async move {
                    // Worker startup, query and destruction all stay off the UI thread.
                    // No mutation API is used. This path is not executed by static validation.
                    let mut service = ServiceClient::spawn().map_err(|e| format!("{e:#}"))?;
                    service
                        .request(ServiceRequest::AudioDevices)
                        .map_err(|e| format!("{e:#}"))
                })
                .await;
            let _ = owner.update_in(cx, |this, window, cx| {
                this.loading = false;
                match result {
                    Ok(value) => {
                        if let Err(error) = this.observe_speakers(value, window, cx) {
                            this.read_error = Some(error);
                        }
                    }
                    Err(error) => this.read_error = Some(error),
                }
                cx.notify();
            });
        })
        .detach();
        this
    }

    fn observe_speakers(
        &mut self,
        value: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        // runtime_native already parses callback parameter 3. The JS wrapper's
        // {deviceList: JSON-string} envelope is not part of ServiceClient's result.
        let entries = value
            .as_array()
            .ok_or_else(|| "音频设备枚举返回格式不正确。".to_string())?;
        let mut speakers = entries
            .iter()
            .filter(|v| v["type"] == "speaker")
            .cloned()
            .collect::<Vec<_>>();
        for speaker in &mut speakers {
            speaker["content"] = speaker["name"].clone();
        }
        if self
            .original
            .pointer("/playbackPayload/firstDeviceId")
            .is_none_or(|v| !truthy(v))
            && !truthy(&self.first_id)
        {
            if let Some(first) = speakers.first() {
                self.first_id = first["id"].clone();
                self.first_device = first.clone();
                self.second_device =
                    json!({"id":data().labels["x3E"],"name":"None","content":"None"});
            }
        }
        if let Some(saved) = self
            .original
            .pointer("/playbackPayload/playbackDevices")
            .and_then(Value::as_array)
        {
            for device in saved {
                if device.get("id").is_some_and(|id| {
                    truthy(id)
                        && id != &json!(data().labels["x3E"])
                        && !speakers.iter().any(|s| s.get("id") == Some(id))
                }) {
                    let mut disconnected = device.clone();
                    disconnected["isDisconnected"] = json!(true);
                    disconnected["disabled"] = json!(true);
                    speakers.push(disconnected);
                }
            }
        }
        self.speakers = speakers;
        self.project_equalizer_devices(window, cx);
        let first_choices = self.device_choices(false);
        let second_choices = self.device_choices(true);
        self.first.update(cx, |state, cx| {
            state.set_items(first_choices, window, cx);
            state.set_selected_value(&self.first_id.to_string(), window, cx);
        });
        self.second.update(cx, |state, cx| {
            state.set_items(second_choices, window, cx);
            state.set_selected_value(&self.second_id.to_string(), window, cx);
        });
        Ok(())
    }

    fn device_choices(&self, second: bool) -> Vec<DeviceChoice> {
        let mut choices = Vec::new();
        if second {
            choices.push(DeviceChoice {
                key: json!(data().labels["x3E"]).to_string(),
                name: "None".into(),
                disabled: false,
            });
        }
        choices.extend(self.speakers.iter().map(|v| {
            DeviceChoice {
                key: v["id"].to_string(),
                name: v["content"]
                    .as_str()
                    .or_else(|| v["name"].as_str())
                    .unwrap_or("")
                    .to_owned()
                    .into(),
                disabled: v["disabled"] == true,
            }
        }));
        choices
    }
    fn select_device(&mut self, first: bool, key: &str, cx: &mut Context<Self>) {
        let device = if !first && key == json!(data().labels["x3E"]).to_string() {
            Some(json!({"id":data().labels["x3E"],"name":"None","content":"None"}))
        } else {
            self.speakers
                .iter()
                .find(|v| v["id"].to_string() == key && v["disabled"] != true)
                .cloned()
        };
        let Some(device) = device else {
            return;
        };
        let field = if first {
            "firstDeviceId"
        } else {
            "secondDeviceId"
        };
        self.can_save = self.original["playbackPayload"][field] != device["id"];
        if first {
            self.first_id = device["id"].clone();
            self.first_device = device;
        } else {
            self.second_id = device["id"].clone();
            self.second_device = device;
        }
        self.changed = true;
        cx.notify();
    }
    fn project_equalizer_devices(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // ze's dependency list is [validDevices.length, G.length], not contents.
        let counts = (self.valid_devices.len(), self.speakers.len());
        if self.eq_effect_counts == Some(counts) {
            return;
        }
        self.eq_effect_counts = Some(counts);
        let containers = self
            .valid_devices
            .iter()
            .filter(|d| {
                d.0.get("category").and_then(Value::as_str) == Some("AUDIO")
                    && matches!(
                        d.0.get("subCategory").and_then(Value::as_str),
                        Some("SPEAKER" | "SPEAKER_HEAD_CUSHION")
                    )
            })
            .map(|d| d.0.get("deviceContainerId"))
            .collect::<Vec<_>>();
        let available = !containers.is_empty();
        self.eq_speakers = self
            .speakers
            .iter()
            .filter(|s| containers.contains(&s.get("containerId")))
            .cloned()
            .collect();
        // ze writes !available first, then [w]'s subsequent effect writes available.
        // Only the final coherent native state is exposed; no fake acknowledgment.
        if available != self.has_eq_devices && self.mode == 0 {
            self.can_save = available;
            self.changed = true;
        }
        self.has_eq_devices = available;
        let items = endpoint_choices(&self.eq_speakers);
        let selected = self
            .eq_speakers
            .get(self.eq_device_index)
            .map(|v| v["id"].to_string());
        self.eq_devices.update(cx, |state, cx| {
            state.set_items(items, window, cx);
            if let Some(value) = selected {
                state.set_selected_value(&value, window, cx);
            } else {
                state.set_selected_index(None, window, cx);
            }
        });
    }

    fn mapping(&self) -> Result<Value, String> {
        let action = if self.mode == 0 {
            &data().equalizer[self.equalizer]
        } else {
            &data().playback[self.playback]
        };
        let mut group = json!({"audioMode":data().modes[self.mode].id,"audioAssignment":action.id});
        if self.mode == 0 && !self.eq_speakers.is_empty() {
            let selected = self
                .eq_speakers
                .get(self.eq_device_index)
                .ok_or_else(|| "所选音频设备已不可用，局部修改仍保留。".to_string())?;
            let device = self
                .valid_devices
                .iter()
                .find(|d| selected.get("containerId") == d.0.get("deviceContainerId"))
                .ok_or_else(|| "所选 Synapse 音频设备已不可用，局部修改仍保留。".to_string())?;
            let mut payload = json!({"razerAudioSpeakerIds":self.eq_speakers.iter().map(|v|v["id"].clone()).collect::<Vec<_>>()});
            if let Some(id) = selected.get("id") {
                payload["selectedPlaybackId"] = id.clone();
            }
            if let Some(pid) = device.0.get("productId") {
                payload["selectedSpeakerPID"] = pid.clone();
            }
            if action.id == "SpecificAudioEq" {
                if let Some(preset) = data().presets.get(self.preset) {
                    payload["specificEqValue"] = json!(preset.id);
                }
            }
            group["equalizerPayload"] = payload;
        }
        // Ee conditions playbackPayload on J, not on the active category A.
        if self.playback == 2 && !self.speakers.is_empty() {
            group["playbackPayload"] = json!({"firstDeviceId":self.first_id,"secondDeviceId":self.second_id,"playbackDevices":[self.first_device,self.second_device]});
        }
        Ok(
            json!({"outputType":"audioGroup","isHyperShift":false,"inputType":self.input["inputType"],"inputID":self.input["inputID"],"audioGroup":group}),
        )
    }

    fn action_radio(
        &self,
        option: &OptionSpec,
        ix: usize,
        eq: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let checked = if eq {
            self.equalizer == ix
        } else {
            self.playback == ix
        };
        let progress = motion::transition(
            SharedString::from(format!("pod-audio-radio-{}", option.id)),
            if checked { 1_f32 } else { 0. },
            Transition::new(Duration::from_millis(200)).easing(Easing::Ease),
            window,
            cx,
        );
        let owner = cx.entity().downgrade();
        Radio::new(SharedString::from(format!("pod-audio-{}", option.id)))
            .accessibility_label(t(&option.content))
            .checked(checked)
            .flex()
            .items_center()
            .mb(surface::css(10.))
            .mr(surface::css(20.))
            .text_size(surface::css(14.))
            .line_height(surface::css(20.))
            .child(
                div()
                    .size(surface::css(20.))
                    .flex_shrink_0()
                    .rounded_full()
                    .border_1()
                    .border_color(Colors::radio_border())
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .size(surface::css(10. * progress))
                            .rounded_full()
                            .bg(Colors::selected())
                            .opacity(progress),
                    ),
            )
            .child(div().ml(surface::css(10.)).child(t(&option.content)))
            .on_change(move |_, _, _, cx| {
                let _ = owner.update(cx, |this, cx| {
                    if eq {
                        this.equalizer = ix;
                    } else {
                        this.playback = ix;
                        this.can_save = true;
                        this.changed = true;
                    }
                    cx.notify();
                });
            })
            .into_any_element()
    }
}

fn endpoint_choices(endpoints: &[Value]) -> Vec<DeviceChoice> {
    endpoints
        .iter()
        .map(|v| DeviceChoice {
            key: v["id"].to_string(),
            name: v["content"]
                .as_str()
                .or_else(|| v["name"].as_str())
                .unwrap_or("")
                .to_owned()
                .into(),
            disabled: v["disabled"] == true,
        })
        .collect()
}

fn choices(options: &[OptionSpec]) -> Vec<Choice> {
    options
        .iter()
        .map(|v| Choice::new(v.id.clone(), t(&v.content)))
        .collect()
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::Number(v) => v.as_f64() != Some(0.),
        Value::String(v) => !v.is_empty(),
        _ => true,
    }
}

fn note(text: String, error: bool) -> Div {
    div()
        .text_size(surface::css(14.))
        .line_height(surface::css(17.))
        .mb(surface::css(20.))
        .text_color(if error {
            Colors::error()
        } else {
            Colors::secondary()
        })
        .child(text)
}

impl Render for AudioEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let mut body = v_flex()
            .w(surface::css(210.))
            .text_color(Colors::text())
            .mb(surface::css(30.))
            .child(
                h_flex()
                    .justify_between()
                    .mb(surface::css(20.))
                    .font_family("RazerF5")
                    .text_size(surface::css(16.))
                    .line_height(surface::css(19.))
                    .text_color(Colors::selected())
                    .child(t("AUDIO_FUNCTION"))
                    .child(
                        gpui_kit::base::Button::new("pod-audio-close")
                            .accessibility_label(t("CLOSE"))
                            .size(surface::css(20.))
                            .child(img("synapse/mapping-close.svg").size(surface::css(20.)))
                            .on_click(
                                cx.listener(|this, _, window, cx| this.request_close(window, cx)),
                            ),
                    ),
            )
            .child(
                surface::select(&self.modes)
                    .items(choices(&data().modes))
                    .w_full()
                    .mb(surface::css(20.)),
            );
        if self.mode == 0 {
            if self.has_eq_devices {
                body = body
                    .child(
                        div()
                            .text_size(surface::css(14.))
                            .line_height(surface::css(14.))
                            .mb(surface::css(10.))
                            .child(t(&data().labels["fYN"]).to_uppercase()),
                    )
                    .child(
                        surface::select(&self.eq_devices)
                            .items(endpoint_choices(&self.eq_speakers))
                            .w_full()
                            .mb(surface::css(10.)),
                    );
                for (ix, option) in data().equalizer.iter().enumerate() {
                    body = body.child(self.action_radio(option, ix, true, window, cx));
                }
                if self.equalizer == 2 {
                    body = body.child(
                        surface::select(&self.presets)
                            .items(choices(&data().presets))
                            .w_full()
                            .mb(surface::css(10.)),
                    );
                }
            } else {
                body = body.child(note(t(&data().labels["diY"]), true));
            }
            body = body.child(
                gpui_kit::base::Link::new("pod-audio-compatible")
                    .href("https://mysupport.razer.com/app/answers/detail/a_id/13138")
                    .open_with(|url, _, _, cx| cx.open_url(url))
                    .underline()
                    .text_size(surface::css(14.))
                    .line_height(surface::css(17.))
                    .text_color(Colors::text())
                    .child(t(&data().labels["RFv"])),
            );
        } else {
            body = body.child(note(t(&data().labels["uTW"]), false));
            for (ix, option) in data().playback.iter().enumerate() {
                body = body.child(self.action_radio(option, ix, false, window, cx));
            }
            if self.playback == 2 {
                body = body
                    .child(
                        surface::select(&self.first)
                            .items(self.device_choices(false))
                            .w_full()
                            .mb(surface::css(10.)),
                    )
                    .child(
                        surface::select(&self.second)
                            .items(self.device_choices(true))
                            .w_full()
                            .mb(surface::css(10.)),
                    );
                if self.speakers.iter().any(|v| v["isDisconnected"] == true) {
                    body = body.child(note(t(&data().labels["f_P"]), true));
                }
            }
        }
        if let Some(error) = &self.save_error {
            body = body.child(note(error.clone(), true));
        }
        if self.loading {
            body = body.child(surface::note("正在读取音频设备…", cx));
        }
        if let Some(error) = &self.read_error {
            body = body.child(surface::note(error.clone(), cx));
        }
        body.child(
            h_flex()
                .gap_2()
                .child(
                    Button::new("pod-audio-cancel")
                        .label(t("CANCEL"))
                        .on_click(cx.listener(|_, _, _, cx| cx.emit(EditorEvent::Cancel))),
                )
                .child(
                    Button::new("pod-audio-save")
                        .label(t("SAVE"))
                        .disabled(!self.can_save)
                        .on_click(cx.listener(|this, _, _, cx| {
                            if this.can_save {
                                match this.mapping() {
                                    Ok(mapping) => cx.emit(EditorEvent::Save(mapping)),
                                    Err(error) => {
                                        this.save_error = Some(error);
                                        cx.notify();
                                    }
                                }
                            }
                        })),
                ),
        )
        .when(self.warning.is_some(), |body| {
            body.child(self.render_warning(window, cx))
        })
    }
}
