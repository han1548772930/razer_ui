//! Live Ea modal state from independently parsed current 740/746 chunks.
//! Host capture requests are explicit; neither keyboard input nor ADC success is synthesized.
use super::*;
use std::time::Instant;

#[derive(Clone, Copy, Debug)]
pub enum LegacyCalibrationHostCommand {
    /// Subscribe/start foreground monitor, disableMapping, enable device input redirect, subscribe inputredirect, INIT locally.
    Mount,
    /// Foreground resumed: disableMapping, ensure device input redirect, subscribe inputredirect, INIT locally.
    Initialize,
    /// Reset locally, enableMapping, release input redirect, unsubscribe, then ON_STOP_CALIBRATION.
    Cleanup,
    /// Unsubscribe/stop foreground monitor and perform Cleanup if initialized.
    Unmount,
}
#[derive(Clone, Debug)]
pub struct KeyboardLegacyCalibrationHostRequested {
    pub product_id: u32,
    pub command: LegacyCalibrationHostCommand,
}
impl EventEmitter<KeyboardLegacyCalibrationHostRequested> for KeyboardProductWorkspace {}

#[derive(Deserialize)]
struct CaptureData {
    product_id: u32,
    keys: Vec<Value>,
}
fn capture_keys(pid: u32) -> &'static [Value] {
    static DATA: OnceLock<Vec<CaptureData>> = OnceLock::new();
    &DATA
        .get_or_init(|| {
            serde_json::from_str(include_str!("keyboard_calibration_capture_data.json"))
                .expect("current 740/746 scan-code literals")
        })
        .iter()
        .find(|row| row.product_id == pid)
        .expect("supported calibration product")
        .keys
}
pub(super) struct Live {
    owner: WeakEntity<KeyboardProductWorkspace>,
    buttons: Vec<Value>,
    step: u8,
    error: bool,
    loading: bool,
    completed: bool,
    verify_top_once: bool,
    selected: Option<Value>,
    capture: Value,
    alternative: Value,
    initialized: bool,
    last_input: Instant,
    timer: Option<Task<()>>,
}
impl Live {
    fn new(owner: WeakEntity<KeyboardProductWorkspace>, buttons: Vec<Value>) -> Self {
        Self {
            owner,
            buttons,
            step: 1,
            error: false,
            loading: false,
            completed: false,
            verify_top_once: true,
            selected: None,
            capture: Value::Null,
            alternative: Value::Null,
            initialized: false,
            last_input: Instant::now(),
            timer: None,
        }
    }
    fn reset(&mut self) {
        self.step = 1;
        self.error = false;
        self.loading = false;
        self.completed = false;
        self.verify_top_once = true;
        self.selected = None;
        self.capture = Value::Null;
        self.alternative = Value::Null;
        self.timer = None;
    }
    pub(super) fn sample(&self) -> Sample {
        if self.error {
            return Sample::Failure;
        }
        match self.step {
            1 => {
                if self.selected.is_some() {
                    Sample::KeySelected
                } else {
                    Sample::SelectKey
                }
            }
            2 => {
                if self.loading {
                    Sample::CalibrateBottom
                } else {
                    Sample::PressKey
                }
            }
            3 => {
                if self.completed {
                    Sample::ReleaseKey
                } else {
                    Sample::VerifyBottom
                }
            }
            _ => {
                if self.loading {
                    Sample::CalibrateTop
                } else {
                    Sample::Success
                }
            }
        }
    }
    pub(super) fn key_label(&self) -> Option<String> {
        self.selected
            .as_ref()
            .and_then(|v| v["defaultValue"].as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    }
    pub(super) fn step(&self) -> u8 {
        self.step
    }
    pub(super) fn completed(&self) -> bool {
        self.completed
    }
    pub(super) fn loading(&self) -> bool {
        self.loading
    }
    pub(super) fn captured(&self) -> bool {
        self.capture["name"].as_str().is_some()
    }
    pub(super) fn disabled(&self) -> bool {
        self.loading
            || self.step == 1
                && self.selected.as_ref().is_none_or(|v| {
                    ["defaultValue", "buttonKey", "inputID"]
                        .iter()
                        .all(|key| v[*key].as_str().is_none_or(str::is_empty))
                })
    }
    fn input_id(&self) -> Option<String> {
        self.selected
            .as_ref()
            .and_then(|key| {
                key["inputID"]
                    .as_str()
                    .or_else(|| key["buttonKey"].as_str())
            })
            .map(str::to_owned)
    }
    fn observe_result(&mut self, success: bool) -> bool {
        self.error = !success;
        match self.step {
            3 => {
                if success {
                    self.loading = false;
                    self.completed = true;
                } else {
                    self.step -= 1;
                    self.loading = false;
                    self.completed = false;
                }
            }
            4 => {
                if success && self.verify_top_once {
                    self.loading = false;
                    self.completed = true;
                    self.verify_top_once = false;
                    return true;
                } else if !success {
                    self.step = 3;
                    self.loading = false;
                    self.completed = false;
                }
            }
            _ => {
                self.loading = false;
                self.completed = false;
            }
        }
        false
    }
    fn redirect(&mut self, pid: u32, input: &str) {
        let Ok(input) = serde_json::from_str::<Value>(input) else {
            return;
        };
        // Source Pause sequence retains preceding E1 Ctrl instead of the fake NumLock fragment.
        if self.capture["scancode"] == 29
            && self.capture["outputFlag"] == 4
            && input["flag"] == 0
            && input["scancode"] == 69
        {
            return;
        }
        self.last_input = Instant::now();
        let flag = input["flag"].as_i64();
        let scancode = input["scancode"].as_i64();
        if flag.is_none() || scancode.is_none() {
            return;
        }
        let extra = json!({"name":"KEY_FN","inputID":"KEY_FN","type":"","scancode":126,"flag":0,"pageID":"7","HID":"231"});
        let keys = capture_keys(pid)
            .iter()
            .chain(std::iter::once(&extra))
            .collect::<Vec<_>>();
        let matches = |key: &&Value, offset: i64| {
            let expected = key["outputFlag"]
                .as_i64()
                .or_else(|| key["flag"].as_i64())
                .unwrap_or(0)
                + offset;
            if input["type"] == "razerKey" && Some(expected) == flag {
                key["name"] == "KEY_FN"
            } else {
                key["scancode"].as_i64() == scancode && Some(expected) == flag
            }
        };
        let normal = keys
            .iter()
            .copied()
            .filter(|key| matches(key, 0))
            .collect::<Vec<_>>();
        if let Some(key) = keys.iter().copied().find(|key| matches(key, 1)) {
            self.capture = key.clone();
            self.capture["outputFlag"] = input["flag"].clone();
            self.capture["scancode"] = input["scancode"].clone();
        } else if let Some(key) = normal.first() {
            self.capture = (*key).clone();
            self.alternative = normal.get(1).map_or(Value::Null, |v| (*v).clone());
        }
        if self.step > 1 || self.error {
            return;
        }
        let mut names = Vec::new();
        for key in [&self.capture, &self.alternative] {
            for field in ["inputID", "name"] {
                if let Some(value) = key[field].as_str() {
                    names.push(value);
                }
            }
            match key["inputID"].as_str() {
                Some("KEY_LEFT_GUI" | "KEY_WINDOWS") => {
                    names.extend(["KEY_LEFT_GUI", "KEY_WINDOWS"])
                }
                Some("KEY_RO") => names.extend(["KEY_RO", "KEY_BEFORE_LEFT_SHIFT"]),
                Some("KEY_KATAKANA_HIRAGANA") => {
                    names.extend(["KEY_KATAKANA_HIRAGANA", "KEY_BEFORE_RIGHT_ALT"])
                }
                Some("KEY_HENKAN") => names.extend(["KEY_HENKAN", "KEY_AFTER_SPACE"]),
                Some("KEY_MUHENKAN") => names.extend(["KEY_MUHENKAN", "KEY_BEFORE_SPACE"]),
                _ => {}
            }
        }
        if let Some(button) = self.buttons.iter().find(|button| {
            button["inputType"] == "AnalogInput"
                && ["buttonKey", "inputID"].iter().any(|field| {
                    button[*field]
                        .as_str()
                        .is_some_and(|value| names.contains(&value))
                })
        }) {
            let mut button = button.clone();
            if button["buttonKey"] == "KEY_FN" {
                button["defaultValue"] = json!("Fn");
            }
            self.selected = Some(button);
        }
    }
}
impl CalibrationModal {
    pub(super) fn attach_live(
        &mut self,
        owner: WeakEntity<KeyboardProductWorkspace>,
        buttons: Vec<Value>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.live = Some(Live::new(owner, buttons));
        self.host(LegacyCalibrationHostCommand::Mount, cx);
        self.start_live_timer(window, cx);
    }
    fn host(&self, command: LegacyCalibrationHostCommand, cx: &mut Context<Self>) {
        let Some(live) = &self.live else { return };
        let owner = live.owner.clone();
        let product_id = self.spec.product_id;
        cx.defer(move |cx| {
            let _ = owner.update(cx, |_, cx| {
                cx.emit(KeyboardLegacyCalibrationHostRequested {
                    product_id,
                    command,
                })
            });
        });
    }
    fn middleware(&self, action: &'static str, payload: Option<Value>, cx: &mut Context<Self>) {
        let Some(live) = &self.live else { return };
        let owner = live.owner.clone();
        cx.defer(move |cx| {
            let _ = owner.update(cx, |this, cx| {
                let mut message = json!({"type":action});
                if let Some(payload) = payload {
                    message["payload"] = payload;
                }
                this.request_actuation_message(message, cx);
            });
        });
    }
    pub(super) fn close_live(&mut self, cx: &mut Context<Self>) {
        if self.live.is_some() {
            self.host(LegacyCalibrationHostCommand::Unmount, cx);
            self.live = None;
        }
    }
    fn start_live_timer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(live) = &mut self.live else { return };
        live.last_input = Instant::now();
        live.timer = Some(cx.spawn_in(window, async move |this, cx| {
            loop {
                cx.background_executor().timer(Duration::from_secs(1)).await;
                let active = this
                    .update_in(cx, |this, window, cx| {
                        let Some(live) = &this.live else { return false };
                        if !this.open || live.step > 3 {
                            return false;
                        }
                        if live.last_input.elapsed() > Duration::from_secs(15) {
                            this.close(window, cx);
                            false
                        } else {
                            true
                        }
                    })
                    .unwrap_or(false);
                if !active {
                    break;
                }
            }
        }));
    }
    pub(super) fn advance_live(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(live) = &mut self.live else { return };
        if live.disabled() {
            return;
        }
        if live.step > 3 {
            self.close(window, cx);
            return;
        }
        if live.error {
            live.reset();
            self.middleware("ON_STOP_CALIBRATION", None, cx);
            cx.notify();
            return;
        }
        let Some(input) = live.input_id() else { return };
        let action = match live.step {
            1 => {
                live.step = 2;
                live.capture = Value::Null;
                live.completed = false;
                live.loading = true;
                "ON_START_BOTTOM_ANALOG_CALIBRATION"
            }
            2 => {
                live.step = 3;
                "ON_VERIFY_BOTTOM_ANALOG_CALIBRATION"
            }
            3 if live.completed => {
                live.step = 4;
                live.capture = Value::Null;
                live.completed = false;
                live.loading = true;
                live.timer = None;
                "ON_START_TOP_ANALOG_CALIBRATION"
            }
            _ => return,
        };
        if live.step <= 3 {
            self.start_live_timer(window, cx);
        }
        self.middleware(action, Some(json!([input])), cx);
        cx.notify();
    }
}
impl KeyboardProductWorkspace {
    pub fn observe_legacy_calibration_capture(
        &mut self,
        initialized: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(modal) = &self.calibration_modal {
            modal.update(cx, |modal, cx| {
                let Some(live) = &mut modal.live else { return };
                live.initialized = initialized;
                if initialized {
                    live.reset();
                    modal.start_live_timer(window, cx);
                } else {
                    live.reset();
                }
                cx.notify();
            });
        }
    }
    pub fn observe_legacy_calibration_input(
        &mut self,
        input: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(modal) = &self.calibration_modal {
            modal.update(cx, |modal, cx| {
                let Some(live) = &mut modal.live else { return };
                if !live.initialized {
                    return;
                }
                live.redirect(modal.spec.product_id, input);
                if live.step <= 3 && live.timer.is_none() {
                    modal.start_live_timer(window, cx);
                }
                cx.notify();
            });
        }
    }
    pub fn observe_legacy_calibration(
        &mut self,
        action: &str,
        payload: &Value,
        cx: &mut Context<Self>,
    ) -> bool {
        if action == "MW_UPDATE_ANALOG_CALIBRATED_KEY_LIST_TO_UI" {
            let Some(keys) = payload["calibratedKeys"].as_array() else {
                return false;
            };
            self.calibrated_keys = keys.clone();
            cx.notify();
            return true;
        }
        let success = match action {
            "MW_UPDATE_BOTTOM_ANALOG_CALIBRATION_STATUS_TO_UI"
            | "MW_UPDATE_TOP_ANALOG_CALIBRATION_STATUS_TO_UI" => payload.as_bool(),
            "MW_UPDATE_ANALOG_ADC_VERIFICATION_STATUS_TO_UI" => payload["isSuccess"].as_bool(),
            _ => return false,
        };
        let Some(success) = success else { return false };
        if let Some(modal) = &self.calibration_modal {
            modal.update(cx, |modal, cx| {
                let Some(live) = &mut modal.live else { return };
                if !live.initialized {
                    return;
                }
                if live.observe_result(success)
                    && let Some(id) = live.input_id()
                {
                    modal.middleware("ON_VERIFY_TOP_ANALOG_CALIBRATION", Some(json!([id])), cx);
                }
                cx.notify();
            });
        }
        true
    }
    pub fn observe_legacy_calibration_foreground(
        &mut self,
        name: &str,
        host_active: bool,
        cx: &mut Context<Self>,
    ) {
        if let Some(modal) = &self.calibration_modal {
            modal.update(cx, |modal, cx| {
                let Some(live) = &mut modal.live else { return };
                let active = host_active && name == "RazerAppEngine.exe";
                if active && !live.initialized {
                    modal.host(LegacyCalibrationHostCommand::Initialize, cx);
                } else if !active && live.initialized {
                    live.reset();
                    live.initialized = false;
                    modal.host(LegacyCalibrationHostCommand::Cleanup, cx);
                }
                cx.notify();
            });
        }
    }
}
