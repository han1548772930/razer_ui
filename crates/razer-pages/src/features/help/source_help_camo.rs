//! Local Camo installation/launch and camera serial lifecycle.
use super::*;
use razer_device::camera_serial::CameraSerialProgress;
use razer_model::model::CameraSerialObservation;
use razer_platform::camera_apps::CamoInstallation;
use razer_widgets::theme::HelpCamoColors as Colors;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelpCamoCommand {
    Observe,
    GetSerialNumber,
    /// Original bound setLaunchCamoApp(), distinct from the download URL.
    LaunchInstalled,
}
#[derive(Clone, Debug)]
pub struct HelpCamoRequested {
    owner: EntityId,
    generation: u64,
    product_id: u32,
    real_product_id: u32,
    container_id: String,
    edition_id: u32,
    layout_id: u32,
    page_offset: usize,
    command: HelpCamoCommand,
    cancelled: Arc<AtomicBool>,
}
impl HelpCamoRequested {
    pub fn product_id(&self) -> u32 {
        self.product_id
    }
    pub fn page_offset(&self) -> usize {
        self.page_offset
    }
    pub fn owner(&self) -> EntityId {
        self.owner
    }
    pub fn command(&self) -> &HelpCamoCommand {
        &self.command
    }
    pub fn container_id(&self) -> &str {
        &self.container_id
    }
    pub fn cancellation(&self) -> Arc<AtomicBool> {
        self.cancelled.clone()
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
    pub fn serial_message(&self) -> Option<serde_json::Value> {
        matches!(self.command, HelpCamoCommand::GetSerialNumber)
            .then(|| serde_json::json!({"type":"ON_GET_SERIAL_NUMBER"}))
    }
    /// Source advancedWebcamReducer preserves Date.now() as timerTick and
    /// discards the bound thunk's "CAMO" payload for this specific action.
    pub fn middleware_message(&self, timer_tick: u64) -> Option<serde_json::Value> {
        match self.command {
            HelpCamoCommand::GetSerialNumber => self.serial_message(),
            HelpCamoCommand::LaunchInstalled => Some(serde_json::json!({
                "type":"ON_LAUNCH_CAMO_STUDIO", "timerTick":timer_tick, "payload":{}
            })),
            _ => None,
        }
    }
}
pub struct HelpCamoObservation {
    request: HelpCamoRequested,
    result: Outcome,
}
enum Outcome {
    Installation(Result<CamoInstallation, String>),
    Serial(Result<CameraSerialProgress, String>),
    Launch(Result<(), String>),
}
impl HelpCamoObservation {
    pub fn installation(request: HelpCamoRequested, installation: CamoInstallation) -> Self {
        Self {
            request,
            result: Outcome::Installation(Ok(installation)),
        }
    }
    pub fn request(&self) -> &HelpCamoRequested {
        &self.request
    }
    pub fn serial(request: HelpCamoRequested, result: Result<CameraSerialProgress, String>) -> Self {
        Self { request, result: Outcome::Serial(result) }
    }
    pub fn launched(request: HelpCamoRequested, result: Result<(), String>) -> Self {
        Self { request, result: Outcome::Launch(result) }
    }
    pub fn failed(request: HelpCamoRequested, error: String) -> Self {
        let result = match request.command {
            HelpCamoCommand::Observe => Outcome::Installation(Err(error)),
            HelpCamoCommand::GetSerialNumber => Outcome::Serial(Err(error)),
            HelpCamoCommand::LaunchInstalled => Outcome::Launch(Err(error)),
        };
        Self { request, result }
    }
    pub(in crate::features) fn serial_result(&self) -> Option<&Result<CameraSerialProgress, String>> {
        match &self.result {
            Outcome::Serial(result) => Some(result),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HelpCamoInstallationStatus {
    Unobserved,
    Loading,
    Observed(CamoInstallation),
    Failed(String),
}

pub(super) struct State {
    owner: EntityId,
    generation: u64,
    serial: String,
    invalid_serial: bool,
    loading_serial: bool,
    installation: HelpCamoInstallationStatus,
    launch: Option<Result<(), String>>,
    serial_error: Option<String>,
    cancelled: Arc<AtomicBool>,
}
impl State {
    pub(super) fn new(owner: EntityId) -> Self {
        Self {
            owner,
            generation: 0,
            serial: String::new(),
            invalid_serial: false,
            loading_serial: false,
            installation: HelpCamoInstallationStatus::Unobserved,
            launch: None,
            serial_error: None,
            cancelled: Arc::new(AtomicBool::new(true)),
        }
    }
    pub(super) fn unmount(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        self.generation = self.generation.wrapping_add(1);
        self.serial.clear();
        self.invalid_serial = false;
        self.loading_serial = false;
        self.installation = HelpCamoInstallationStatus::Unobserved;
        self.launch = None;
        self.serial_error = None;
    }
    pub(super) fn installed(&self) -> bool {
        matches!(&self.installation, HelpCamoInstallationStatus::Observed(observation)
            if observation.camera_available() == Ok(true))
    }
    fn validate(&mut self, serial: &str) {
        self.invalid_serial = !valid_serial(serial);
        self.serial = if self.invalid_serial {
            "NOSERIALNUMBER".into()
        } else {
            serial.into()
        };
    }

    fn observe_serial(&mut self, observation: &CameraSerialObservation) {
        self.serial_error = observation.error().map(str::to_owned);
        self.loading_serial = matches!(observation, CameraSerialObservation::Reading);
        if !self.loading_serial {
            self.validate(observation.serial().unwrap_or(""));
        }
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}
fn valid_serial(serial: &str) -> bool {
    let text = serial.encode_utf16().collect::<Vec<_>>();
    text.len() == 15
        && text[10..15]
            .iter()
            .all(|ch| (*ch as u8).is_ascii_digit() && *ch < 128)
        && text[2..6]
            .iter()
            .all(|ch| (*ch as u8).is_ascii_digit() && *ch < 128)
        && text[6] < 128
        && (text[6] as u8).is_ascii_alphabetic()
        && text[9] < 128
        && (text[9] as u8).is_ascii_digit()
        && !text[6..10]
            .iter()
            .any(|ch| matches!(*ch, 10 | 13 | 0x2028 | 0x2029))
}
// CSS ease is cubic-bezier(.25,.1,.25,1); GPUI's default is different.
pub(super) fn source_ease(progress: f32) -> f32 {
    let cubic = |t: f32, a: f32, b: f32| {
        3. * (1. - t).powi(2) * t * a + 3. * (1. - t) * t.powi(2) * b + t.powi(3)
    };
    let (mut low, mut high) = (0., 1.);
    for _ in 0..20 {
        let middle = (low + high) / 2.;
        if cubic(middle, 0.25, 0.25) < progress {
            low = middle;
        } else {
            high = middle;
        }
    }
    cubic((low + high) / 2., 0.1, 1.)
}
impl EventEmitter<HelpCamoRequested> for SourceHelp {}
impl SourceHelp {
    pub(super) fn camo_request(&self, command: HelpCamoCommand) -> Option<HelpCamoRequested> {
        let offset = self.page_offset?;
        let (_, page) = self.metadata()?;
        (page.camo && page.offset == offset && !self.camo.cancelled.load(Ordering::Acquire)).then(|| HelpCamoRequested {
            owner: self.camo.owner,
            generation: self.camo.generation,
            product_id: self.device.product_id,
            real_product_id: self.device.real_product_id,
            container_id: self.device.device_container_id.clone(),
            edition_id: self.device.edition_id,
            layout_id: self.device.layout_id,
            page_offset: page.offset,
            command,
            cancelled: self.camo.cancelled.clone(),
        })
    }
    pub(in crate::features) fn accepts_camo_request(&self, request: &HelpCamoRequested) -> bool {
        self.camo_request(HelpCamoCommand::Observe)
            .is_some_and(|current| {
                current.owner == request.owner
                    && current.generation == request.generation
                    && current.product_id == request.product_id
                    && current.real_product_id == request.real_product_id
                    && current.container_id == request.container_id
                    && current.edition_id == request.edition_id
                    && current.layout_id == request.layout_id
                    && current.page_offset == request.page_offset
                    && !request.is_cancelled()
                    && Arc::ptr_eq(&current.cancelled, &request.cancelled)
            })
    }
    pub(super) fn mount_camo(&mut self, cx: &mut Context<Self>) {
        self.camo.unmount();
        if self.page_offset.is_none() || !self.metadata().is_some_and(|(_, page)| page.camo) {
            return;
        }
        self.camo.cancelled = Arc::new(AtomicBool::new(false));
        if let Some(request) = self.camo_request(HelpCamoCommand::Observe) {
            self.camo.observe_serial(&self.device.dashboard.readonly_camera_serial);
            self.camo.installation = HelpCamoInstallationStatus::Loading;
            cx.emit(request);
        }
    }
    fn retry_camo_serial(&mut self, cx: &mut Context<Self>) {
        if self.camo.loading_serial {
            return;
        }
        let Some(request) = self.camo_request(HelpCamoCommand::GetSerialNumber) else {
            return;
        };
        self.camo.loading_serial = true;
        self.camo.serial_error = None;
        cx.emit(request);
        cx.notify();
    }
    pub(super) fn camo_serial_changed(&mut self, serial: &str, cx: &mut Context<Self>) {
        if self.camo_request(HelpCamoCommand::Observe).is_none() {
            return;
        }
        self.camo.validate(serial);
        self.camo.loading_serial = false;
        cx.notify();
    }
    pub(super) fn camo_observation_changed(&mut self, cx: &mut Context<Self>) {
        if self.camo_request(HelpCamoCommand::Observe).is_some() {
            self.camo.observe_serial(&self.device.dashboard.readonly_camera_serial);
            cx.notify();
        }
    }
    pub(in crate::features) fn observe_camo(
        &mut self,
        observation: HelpCamoObservation,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts_camo_request(&observation.request) {
            return false;
        }
        match observation.result {
            Outcome::Installation(result) if observation.request.command == HelpCamoCommand::Observe => {
                self.camo.installation = match result {
                    Ok(installed) => HelpCamoInstallationStatus::Observed(installed),
                    Err(error) => HelpCamoInstallationStatus::Failed(error),
                };
            }
            Outcome::Serial(result) if observation.request.command == HelpCamoCommand::GetSerialNumber => {
                let serial = match result {
                    Ok(progress) => CameraSerialObservation::Read(progress),
                    Err(error) => CameraSerialObservation::Failed(error),
                };
                self.camo.observe_serial(&serial);
            }
            Outcome::Launch(result) if observation.request.command == HelpCamoCommand::LaunchInstalled => {
                self.camo.launch = Some(result);
            }
            _ => return false,
        }
        cx.notify();
        true
    }
    pub(super) fn camo_serial(&self) -> &str {
        &self.camo.serial
    }
    fn camo_shimmer(&self) -> AnyElement {
        div()
            .w(surface::css(100.))
            .h(surface::css(20.))
            .ml(surface::css(5.))
            .rounded(surface::css(5.))
            .bg(Colors::skeleton())
            .relative()
            .overflow_hidden()
            .child(
                h_flex()
                    .absolute()
                    .top_0()
                    .w(surface::css(100.))
                    .h_full()
                    .child(div().w(surface::css(50.)).h_full().bg(linear_gradient(
                        90.,
                        linear_color_stop(Colors::transparent(), 0.),
                        linear_color_stop(Colors::skeleton(), 1.),
                    )))
                    .child(div().w(surface::css(50.)).h_full().bg(linear_gradient(
                        90.,
                        linear_color_stop(Colors::skeleton(), 0.),
                        linear_color_stop(Colors::transparent(), 1.),
                    )))
                    .with_animation(
                        SharedString::from(format!("camo-serial-shimmer-{}", self.camo.generation)),
                        Animation::new(Duration::from_millis(1200))
                            .repeat()
                            .with_easing(linear),
                        |bar, phase| bar.left(surface::css(-100. + 200. * phase)),
                    ),
            )
            .into_any_element()
    }
    pub(super) fn camo_panel(&self, exact: bool, cx: &Context<Self>) -> AnyElement {
        help_panel("CAMO_STUDIO", exact, cx)
            .child(
                BaseButton::new("source-help-camo-launch")
                    .accessibility_label(i18n::t("CAMO_STUDIO_LAUNCH"))
                    .p_0()
                    .self_start()
                    .text_color(cx.theme().primary)
                    .hover(|style| style.text_color(cx.theme().foreground))
                    .child(i18n::t("CAMO_STUDIO_LAUNCH"))
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.camo.installed() {
                            if let Some(request) =
                                this.camo_request(HelpCamoCommand::LaunchInstalled)
                            {
                                cx.emit(request);
                            }
                        }
                    })),
            )
            .into_any_element()
    }
    pub(super) fn camo_serial_panel(&self, exact: bool, cx: &Context<Self>) -> AnyElement {
        let mut row = h_flex().child(i18n::t("SERIAL"));
        if self.camo.loading_serial {
            row = row.child(self.camo_shimmer());
        } else {
            row = row.child(format!(" {}", self.camo.serial));
        }
        let mut panel = help_panel("SERIAL_NUM", exact, cx).child(row);
        if self.camo.invalid_serial {
            panel = panel
                .child(
                    help_button(
                        "source-help-camo-serial-retry",
                        i18n::t("RETRY"),
                        self.camo.loading_serial,
                        cx,
                    )
                    .mt(surface::css(20.))
                    .min_w(surface::css(100.))
                    .px(surface::css(6.))
                    .bg(Colors::retry())
                    .text_color(Colors::retry_text())
                    .hover(|style| style.opacity(0.8))
                    .on_click(cx.listener(|this, _, _, cx| this.retry_camo_serial(cx))),
                )
                .child(
                    div()
                        .mt(surface::css(20.))
                        .child(i18n::t("ERROR_SERIALNUMBER_DESC")),
                );
        } else {
            panel = panel.child(
                help_button(
                    "source-help-copy",
                    i18n::t(if self.copied_serial {
                        "COPIED_SERIAL"
                    } else {
                        "COPY_SERIAL"
                    }),
                    self.copied_serial,
                    cx,
                )
                .mt(surface::css(20.))
                .on_click(cx.listener(|this, _, window, cx| this.copy_serial(window, cx))),
            );
        }
        panel.into_any_element()
    }
}
