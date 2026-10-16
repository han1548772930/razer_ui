//! Current owner-mounted PID 1453 Power tutorial and PID 1398 THX instructions.
//! This entity outlives Help tab changes, matching the original product owner.
use gpui_kit::base::Button;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_i18n as i18n;
use razer_widgets::{surface, theme::HelpOwnerColors as Colors};
use serde_json::Value;
use std::{collections::BTreeMap, sync::OnceLock, time::Duration};
#[path = "source_help_analog.rs"]
mod analog;
#[path = "../workspace/source_owner_smart_switch.rs"]
mod smart_switch;
#[path = "../workspace/source_owner_whats_new.rs"]
mod whats_new;
#[derive(serde::Deserialize)]
struct Messages {
    product_id: u32,
    locales: BTreeMap<String, BTreeMap<String, String>>,
}
pub(super) fn message(product_id: u32, key: &str) -> String {
    if let Some(text) = smart_switch::message(product_id, key) {
        return text;
    }
    if let Some(text) = analog::message(product_id, key) {
        return text;
    }
    static DATA: OnceLock<Vec<Messages>> = OnceLock::new();
    let data = DATA.get_or_init(|| read_source_help_owner_catalog());
    let locale = i18n::locale();
    data.iter()
        .find(|row| row.product_id == product_id)
        .and_then(|row| {
            row.locales
                .iter()
                .find(|(name, _)| name.eq_ignore_ascii_case(&locale))
                .and_then(|(_, values)| values.get(key))
                .or_else(|| row.locales.get("en").and_then(|values| values.get(key)))
        })
        .cloned()
        .unwrap_or_else(|| i18n::t(key))
}
pub(super) fn supports_thx(product_id: u32) -> bool {
    matches!(product_id, 1398 | 1401 | 1427 | 2638 | 2641 | 4124 | 4126)
}
pub(super) fn supports_product_names(product_id: u32) -> bool {
    matches!(
        product_id,
        162 | 585 | 691 | 1342 | 1383 | 2660 | 2664 | 2668 | 2689 | 778
    )
}
pub(super) fn supports_tutorial(product_id: u32) -> bool {
    product_id == 1453 || analog::supports(product_id)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelpOwnerOverlay {
    Tutorial,
    ThxInstructions,
    WhatsNew,
    SmartSwitch,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct HelpOwnerOpenRequested(pub HelpOwnerOverlay);
pub(super) struct HelpOwnerPageRequested(pub &'static str);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HelpOwnerCommand {
    Observe,
    DismissTutorial,
    DismissWhatsNew,
    SetSimultaneousAudio,
    SetAudioEnhancements,
    SetAudioSound,
    OpenSoundVolume,
    OpenAudioProperties,
    OpenWindowsSoundSettings,
}
#[derive(Clone, Debug)]
pub struct HelpOwnerRequested {
    owner: EntityId,
    generation: u64,
    product_id: u32,
    command: HelpOwnerCommand,
    simultaneous_mode: Option<Value>,
    audio_messages: Vec<Value>,
    audio_revision: u64,
}
impl HelpOwnerRequested {
    pub fn owner(&self) -> EntityId {
        self.owner
    }
    pub fn product_id(&self) -> u32 {
        self.product_id
    }
    pub fn product_names_source_product_id(&self) -> Option<u32> {
        match self.product_id {
            162 => Some(163),
            585 | 691 | 2664 | 778 => Some(self.product_id),
            _ => None,
        }
    }
    pub fn command(&self) -> HelpOwnerCommand {
        self.command
    }
    pub fn simultaneous_audio_message(&self) -> Option<Value> {
        if self.command != HelpOwnerCommand::SetSimultaneousAudio {
            return None;
        }
        let action = smart_switch::mutation(self.product_id)?;
        Some(serde_json::json!({"type":action,"payload":{"mode":self.simultaneous_mode}}))
    }
    pub fn audio_enhancement_messages(&self) -> &[Value] {
        &self.audio_messages
    }
    pub fn audio_sound_messages(&self) -> &[Value] {
        &self.audio_messages
    }
    pub fn audio_revision(&self) -> u64 {
        self.audio_revision
    }
    pub fn whats_new_storage_key(&self) -> Option<&'static str> {
        whats_new::storage_key(self.product_id)
    }
    pub fn tutorial_storage_key(&self) -> Option<String> {
        if self.product_id == 1453 {
            Some(format!("{}_OnboardingTutorialDismissed", self.product_id))
        } else {
            analog::storage_key(self.product_id)
        }
    }
    pub fn tutorial_storage_write(&self) -> Option<(String, Value)> {
        (self.command == HelpOwnerCommand::DismissTutorial)
            .then(|| {
                self.tutorial_storage_key()
                    .map(|key| (key, Value::Bool(true)))
            })
            .flatten()
    }
    pub fn tutorial_storage_writes(&self) -> Vec<(String, Value)> {
        if self.command != HelpOwnerCommand::DismissTutorial {
            return Vec::new();
        }
        if analog::supports(self.product_id) {
            analog::storage_writes(self.product_id)
        } else {
            self.tutorial_storage_write().into_iter().collect()
        }
    }
    pub fn host_focus_actions(&self) -> Option<[Value; 2]> {
        (self.product_id == 1453 && self.command == HelpOwnerCommand::Observe).then(|| {
            [
                serde_json::json!({"action":"getFocusTab", "windowName":"synapse"}),
                serde_json::json!({"action":"getWindowStatus"}),
            ]
        })
    }
    /// Original QT.gv caller invokes the parameterized host msSettings adapter.
    pub fn ms_settings_section(&self) -> Option<&'static str> {
        (self.command == HelpOwnerCommand::OpenWindowsSoundSettings).then_some("sound")
    }
}

pub(super) struct SourceHelpOwner {
    owner: EntityId,
    product_id: u32,
    generation: u64,
    audio_revision: u64,
    mounted: bool,
    active: bool,
    tutorial_shown: bool,
    whats_new_shown: bool,
    smart_switch_shown: bool,
    smart_switch_tip_hovered: bool,
    simultaneous_mode: Option<Value>,
    whats_new_dismissed: Option<bool>,
    analog_step: usize,
    instructions_shown: bool,
    tutorial_dismissed: Option<bool>,
    host_active: Option<bool>,
    host_focus_generation: u64,
    host_focus_task: Option<Task<()>>,
    // The source reducer initializes this to "". Never substitute a model name.
    render_device_name: String,
}
impl EventEmitter<HelpOwnerRequested> for SourceHelpOwner {}
impl EventEmitter<HelpOwnerPageRequested> for SourceHelpOwner {}
impl SourceHelpOwner {
    pub(super) fn new(product_id: u32, cx: &mut Context<Self>) -> Self {
        Self {
            owner: cx.entity_id(),
            product_id,
            generation: 0,
            audio_revision: 0,
            mounted: false,
            active: true,
            tutorial_shown: false,
            whats_new_shown: false,
            smart_switch_shown: false,
            smart_switch_tip_hovered: false,
            simultaneous_mode: smart_switch::initial_mode(product_id),
            whats_new_dismissed: None,
            analog_step: 1,
            instructions_shown: false,
            tutorial_dismissed: None,
            host_active: None,
            host_focus_generation: 0,
            host_focus_task: None,
            render_device_name: String::new(),
        }
    }
    pub(super) fn smart_switch_page(&self) -> Option<&'static str> {
        smart_switch::page(self.product_id)
    }
    pub(super) fn mount(&mut self, cx: &mut Context<Self>) {
        if self.mounted
            || !(self.product_id == 1453
                || supports_thx(self.product_id)
                || supports_product_names(self.product_id))
        {
            return;
        }
        self.mounted = true;
        cx.emit(self.request(HelpOwnerCommand::Observe));
    }
    pub(super) fn set_active(&mut self, active: bool, cx: &mut Context<Self>) {
        if self.active != active {
            self.active = active;
            cx.notify();
        }
    }
    pub(super) fn request(&self, command: HelpOwnerCommand) -> HelpOwnerRequested {
        HelpOwnerRequested {
            owner: self.owner,
            generation: self.generation,
            product_id: self.product_id,
            command,
            simultaneous_mode: self.simultaneous_mode.clone(),
            audio_messages: Vec::new(),
            audio_revision: self.audio_revision,
        }
    }
    pub(super) fn request_audio_enhancements(&self, messages: Vec<Value>, cx: &mut Context<Self>) {
        let mut request = self.request(HelpOwnerCommand::SetAudioEnhancements);
        request.audio_messages = messages;
        cx.emit(request);
    }
    pub(super) fn request_audio_sound(&mut self, messages: Vec<Value>, cx: &mut Context<Self>) {
        // Timer milliseconds can repeat; a monotonic owner revision also guards
        // same-tick edits and initial observations overtaken by a new request.
        self.audio_revision = self.audio_revision.wrapping_add(1);
        let mut request = self.request(HelpOwnerCommand::SetAudioSound);
        request.audio_messages = messages;
        cx.emit(request);
    }
    pub(super) fn request_sound_windows(&self, properties: bool, cx: &mut Context<Self>) {
        cx.emit(self.request(if properties {
            HelpOwnerCommand::OpenAudioProperties
        } else {
            HelpOwnerCommand::OpenSoundVolume
        }));
    }
    pub(super) fn accepts(&self, request: &HelpOwnerRequested) -> bool {
        request.owner == self.owner
            && request.generation == self.generation
            && request.product_id == self.product_id
            && self.mounted
    }
    pub(super) fn accepts_audio_sound(&self, request: &HelpOwnerRequested) -> bool {
        self.accepts(request) && request.audio_revision == self.audio_revision
    }
    pub(super) fn show(&mut self, overlay: HelpOwnerOverlay, cx: &mut Context<Self>) {
        match (self.product_id, overlay) {
            (pid, HelpOwnerOverlay::Tutorial) if supports_tutorial(pid) => {
                self.tutorial_shown = true;
                self.whats_new_shown = false;
            }
            (pid, HelpOwnerOverlay::ThxInstructions) if supports_thx(pid) => {
                self.instructions_shown = true
            }
            _ => return,
        }
        cx.notify();
    }
    pub(super) fn observe_whats_new_dismissal(
        &mut self,
        request: &HelpOwnerRequested,
        value: Option<Value>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts(request)
            || whats_new::storage_key(self.product_id).is_none()
            || !matches!(
                request.command,
                HelpOwnerCommand::Observe | HelpOwnerCommand::DismissWhatsNew
            )
        {
            return false;
        }
        self.whats_new_dismissed = Some(value.as_ref().is_some_and(js_truthy));
        // Original mount-only effect reads both preferences. Closing Tutorial
        // does not rerun that effect, nor does its generic WhatsNew key replace
        // this separate CompetitiveMode preference.
        if request.command == HelpOwnerCommand::Observe {
            self.whats_new_shown = self.tutorial_dismissed == Some(true)
                && self.whats_new_dismissed == Some(false)
                && !self.tutorial_shown;
        }
        cx.notify();
        true
    }
    pub(super) fn observe_tutorial_dismissal(
        &mut self,
        request: &HelpOwnerRequested,
        value: Option<Value>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts(request)
            || !supports_tutorial(self.product_id)
            || !matches!(
                request.command,
                HelpOwnerCommand::Observe | HelpOwnerCommand::DismissTutorial
            )
        {
            return false;
        }
        self.tutorial_dismissed = Some(value.as_ref().is_some_and(js_truthy));
        if analog::supports(self.product_id)
            && request.command == HelpOwnerCommand::Observe
            && self.tutorial_dismissed == Some(false)
        {
            self.tutorial_shown = true;
            self.whats_new_shown = false;
            cx.notify();
        }
        self.auto_open(cx);
        true
    }
    pub(super) fn observe_render_device_name(
        &mut self,
        request: &HelpOwnerRequested,
        name: String,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts(request)
            || !supports_thx(self.product_id)
            || request.command != HelpOwnerCommand::Observe
        {
            return false;
        }
        self.render_device_name = name;
        cx.notify();
        true
    }
    /// Source focus helper combines getFocusTab and getWindowStatus, then
    /// coalesces changes for 400 ms. window_name is observed window.name.
    pub(super) fn observe_host_focus(
        &mut self,
        request: &HelpOwnerRequested,
        focus_tab: &str,
        window_name: &str,
        is_visible: Option<bool>,
        is_minimized: Option<bool>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts(request)
            || self.product_id != 1453
            || request.command != HelpOwnerCommand::Observe
        {
            return false;
        }
        let active = is_visible != Some(false)
            && is_minimized != Some(true)
            && (focus_tab == window_name || (focus_tab.is_empty() && window_name == "synapse"));
        self.debounce_host_active(active, cx);
        true
    }
    /// Original emitInitial catch falls back to !document.hidden.
    pub(super) fn observe_initial_host_failure(
        &mut self,
        request: &HelpOwnerRequested,
        document_hidden: bool,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts(request)
            || self.product_id != 1453
            || request.command != HelpOwnerCommand::Observe
        {
            return false;
        }
        self.debounce_host_active(!document_hidden, cx);
        true
    }
    /// Source changeActiveTab/activeTabChanged and focus(true) are comparisons
    /// only. They deliberately do not consult window visibility/minimization.
    pub(super) fn observe_active_tab(
        &mut self,
        request: &HelpOwnerRequested,
        focus_tab: &str,
        window_name: &str,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts(request)
            || self.product_id != 1453
            || request.command != HelpOwnerCommand::Observe
        {
            return false;
        }
        self.debounce_host_active(focus_tab == window_name, cx);
        true
    }
    fn debounce_host_active(&mut self, active: bool, cx: &mut Context<Self>) {
        self.host_focus_generation = self.host_focus_generation.wrapping_add(1);
        let generation = self.host_focus_generation;
        self.host_focus_task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor()
                .timer(Duration::from_millis(400))
                .await;
            let _ = this.update(cx, |this, cx| {
                if this.host_focus_generation != generation {
                    return;
                }
                if this.host_active != Some(active) {
                    this.host_active = Some(active);
                    this.auto_open(cx);
                }
            });
        }));
    }
    fn auto_open(&mut self, cx: &mut Context<Self>) {
        if self.product_id == 1453
            && self.tutorial_dismissed == Some(false)
            && self.host_active == Some(true)
            && !self.tutorial_shown
        {
            self.tutorial_shown = true;
            self.whats_new_shown = false;
            cx.notify();
        }
    }
    fn close(&mut self, overlay: HelpOwnerOverlay, cx: &mut Context<Self>) {
        match overlay {
            HelpOwnerOverlay::Tutorial if self.tutorial_shown => {
                self.tutorial_shown = false;
                // Await the actual storage receipt. A dispatched request is
                // neither a successful persistence result nor a device write.
                self.tutorial_dismissed = None;
                cx.emit(self.request(HelpOwnerCommand::DismissTutorial));
            }
            HelpOwnerOverlay::SmartSwitch if self.smart_switch_shown => {
                self.smart_switch_shown = false;
                self.smart_switch_tip_hovered = false;
            }
            HelpOwnerOverlay::WhatsNew if self.whats_new_shown => {
                self.whats_new_shown = false;
                self.whats_new_dismissed = None;
                cx.emit(self.request(HelpOwnerCommand::DismissWhatsNew));
            }
            HelpOwnerOverlay::ThxInstructions if self.instructions_shown => {
                self.instructions_shown = false
            }
            _ => return,
        }
        cx.notify();
    }
    fn power_content(&self) -> Div {
        v_flex()
            .text_color(Colors::text())
            .child(
                heading(self.product_id, "POWER_ONBOARDING_TUTORIAL_HEADING").mb(surface::css(20.)),
            )
            .child(
                v_flex()
                    .mb(surface::css(10.))
                    .child(
                        div()
                            .font_family("RazerF5")
                            .text_size(surface::css(14.))
                            .line_height(surface::css(14.))
                            .text_color(Colors::accent())
                            .mb(surface::css(10.))
                            .child(
                                message(self.product_id, "POWER_ONBOARDING_TUTORIAL_SUBTITLE")
                                    .to_uppercase(),
                            ),
                    )
                    .child(
                        div().child(message(self.product_id, "POWER_ONBOARDING_TUTORIAL_BODY_1")),
                    ),
            )
            .child(
                div()
                    .mb(surface::css(10.))
                    .child(message(self.product_id, "POWER_ONBOARDING_TUTORIAL_BODY_2")),
            )
    }
    fn instruction_content(&self, cx: &Context<Self>) -> Div {
        v_flex()
            .gap(surface::css(20.))
            .child(heading(self.product_id, "WINDOWS_SPATIAL_AUDIO"))
            .child(div().child(message(self.product_id, "BEST_EXPERIENCE_HINT")))
            .child(
                h_flex()
                    .items_start()
                    .child(format!("1. {}", message(self.product_id, "GO_TO")))
                    .child(
                        Button::new("help-owner-windows-sound")
                            .accessibility_label(message(self.product_id, "WINDOWS_SOUND_SETTINGS"))
                            .p_0()
                            .ml(surface::css(4.))
                            .underline()
                            .text_color(Colors::text())
                            .child(message(self.product_id, "WINDOWS_SOUND_SETTINGS"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                cx.emit(this.request(HelpOwnerCommand::OpenWindowsSoundSettings))
                            })),
                    )
                    .child(
                        svg()
                            .path("synapse/help-owner-sound-external.svg")
                            .size(surface::css(16.))
                            .ml(surface::css(4.))
                            .flex_shrink_0(),
                    ),
            )
            .child(instruction_line(
                self.product_id,
                "2. ",
                "STEP_SELECT_OUTPUT_DEVICE",
                format!("\"{}\"", self.render_device_name),
                "STEP_SELECT_OUTPUT_DEVICE_2",
            ))
            .child(instruction_line(
                self.product_id,
                "3. ",
                "STEP_SELECT_SPATIAL_SOUND_OFF",
                format!("\"{}\"", message(self.product_id, "SPATIAL_SOUND")),
                "STEP_SELECT_SPATIAL_SOUND_OFF_2",
            ))
    }
    fn overlay(
        &self,
        overlay: HelpOwnerOverlay,
        window: &Window,
        cx: &Context<Self>,
    ) -> AnyElement {
        let tutorial = overlay == HelpOwnerOverlay::Tutorial;
        let asset = if tutorial {
            "synapse/help-owner-1453.webp"
        } else {
            "synapse/help-owner-1398.webp"
        };
        let size = window.viewport_size();
        let host_tab = window.rem_size() / 16. * 42.;
        let height = (size.height - host_tab).max(px(0.));
        let close = Button::new(if tutorial {
            "help-owner-tutorial-close"
        } else {
            "help-owner-thx-close"
        })
        .accessibility_label(message(self.product_id, "CLOSE"))
        .absolute()
        .top(surface::css(10.))
        .right(surface::css(10.))
        .size(surface::css(24.))
        .p_0()
        .when(tutorial, |button| {
            button
                .rounded(surface::css(2.))
                .hover(|style| style.bg(Colors::close_hover()))
        })
        .child(
            svg()
                .path("synapse/help-owner-close.svg")
                .size(surface::css(24.)),
        )
        .on_click(cx.listener(move |this, _, _, cx| this.close(overlay, cx)));
        let button = Button::new(if tutorial {
            "help-owner-tutorial-done"
        } else {
            "help-owner-thx-done"
        })
        .accessibility_label(message(self.product_id, "CLOSE"))
        .h(surface::css(27.))
        .min_w(surface::css(90.))
        .p_0()
        .rounded(surface::css(3.))
        .text_size(surface::css(12.))
        .bg(if tutorial {
            Colors::accent()
        } else {
            Colors::button()
        })
        .text_color(if tutorial {
            Colors::button_text()
        } else {
            Colors::plain_button_text()
        })
        .when(tutorial, |button| {
            button.hover(|style| style.bg(Colors::button_hover()))
        })
        .active(move |style| {
            if tutorial {
                style.bg(Colors::button_pressed()).opacity(1.)
            } else {
                style.opacity(0.3)
            }
        })
        .child(message(self.product_id, "CLOSE").to_uppercase())
        .on_click(cx.listener(move |this, _, _, cx| this.close(overlay, cx)));
        let card = h_flex()
            .id(if tutorial {
                "help-owner-power-tutorial"
            } else {
                "help-owner-thx-instructions"
            })
            .relative()
            .w(surface::css(800.))
            .min_h(surface::css(420.))
            .items_stretch()
            .bg(Colors::background())
            .text_color(Colors::text())
            .rounded(surface::css(3.))
            .shadow(vec![BoxShadow {
                color: Colors::accent(),
                offset: point(px(0.), px(0.)),
                blur_radius: px(0.),
                spread_radius: window.rem_size() / 16. * 1.5,
                inset: false,
            }])
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .occlude()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .relative()
                    .flex_shrink_0()
                    .w(surface::css(320.))
                    .min_h(surface::css(if tutorial { 420. } else { 380. }))
                    .m(surface::css(1.))
                    .when(!tutorial, |video| {
                        video.mx_0().my(surface::css(20.)).max_h(surface::css(420.))
                    })
                    .child(
                        image_cache(razer_assets::resource_image_cache())
                            .absolute()
                            .inset_0()
                            .child(
                                img(asset)
                                    .id("help-owner-video")
                                    .size_full()
                                    .object_fit(ObjectFit::Cover),
                            ),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .justify_between()
                    .p(surface::css(50.))
                    .child(if tutorial {
                        self.power_content()
                    } else {
                        self.instruction_content(cx)
                    })
                    .child(h_flex().self_start().child(button)),
            )
            .child(close);
        // Original backdrop has pointer-events:none; the card alone occludes.
        deferred(
            anchored()
                .anchor(Anchor::TopLeft)
                .position(point(px(0.), host_tab))
                .child(
                    div().w(size.width).h(height).bg(Colors::shade()).child(
                        deferred(CenteredAt {
                            center: point(size.width / 2., host_tab + height * 0.45),
                            child: card.into_any_element(),
                        })
                        .priority(106),
                    ),
                ),
        )
        .priority(105)
        .into_any_element()
    }
}
impl Render for SourceHelpOwner {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .when(self.active && self.tutorial_shown, |root| {
                if analog::supports(self.product_id) {
                    root.child(self.analog_overlay(window, cx))
                } else {
                    root.child(self.overlay(HelpOwnerOverlay::Tutorial, window, cx))
                }
            })
            .when(self.active && self.whats_new_shown, |root| {
                root.child(self.whats_new_overlay(window, cx))
            })
            .when(self.active && self.smart_switch_shown, |root| {
                root.child(self.smart_switch_overlay(window, cx))
            })
            .when(self.active && self.instructions_shown, |root| {
                root.child(self.overlay(HelpOwnerOverlay::ThxInstructions, window, cx))
            })
    }
}
fn heading(product_id: u32, key: &str) -> Div {
    div()
        .font_family("Roboto")
        .text_size(surface::css(20.))
        .line_height(surface::css(20.))
        .child(message(product_id, key).to_uppercase())
}
fn instruction_line(
    product_id: u32,
    prefix: &str,
    before: &str,
    highlight: String,
    after: &str,
) -> Div {
    let before = format!("{prefix}{} ", message(product_id, before));
    let start = before.len();
    let end = start + highlight.len();
    div().child(
        StyledText::new(format!(
            "{before}{highlight} {}",
            message(product_id, after)
        ))
        .with_highlights([(
            start..end,
            HighlightStyle {
                color: Some(Colors::accent()),
                ..Default::default()
            },
        )]),
    )
}
fn js_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::Number(v) => v.as_f64().is_some_and(|v| v != 0.),
        Value::String(v) => !v.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}
/// CSS translate(-50%,-50%) centers measured content without viewport snapping.
struct CenteredAt {
    center: Point<Pixels>,
    child: AnyElement,
}
impl IntoElement for CenteredAt {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for CenteredAt {
    type RequestLayoutState = LayoutId;
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        let child = self.child.request_layout(window, cx);
        let layout = window.request_layout(
            Style {
                position: Position::Absolute,
                display: Display::Flex,
                ..Default::default()
            },
            [child],
            cx,
        );
        (layout, child)
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let size = window.layout_bounds(*child).size;
        let origin = point(
            self.center.x - size.width / 2.,
            self.center.y - size.height / 2.,
        );
        window.with_element_offset(origin - bounds.origin, |window| {
            self.child.prepaint(window, cx);
        });
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut LayoutId,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}

// Generated catalog reader: razer-pages/source_help_owner_data.
fn read_source_help_owner_catalog() -> Vec<Messages> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<Messages>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Messages> {
        let value = match r.tag()? {
            0 => Messages {
                product_id: r.child(decode_2)?,
                locales: r.child(decode_3)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u32> {
        let value = match r.tag()? {
            0 => r.u32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
    > {
        let value = match r.tag()? {
            0 => r.map(decode_4, decode_5)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, String>> {
        let value = match r.tag()? {
            0 => r.map(decode_4, decode_4)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_SOURCE_HELP_OWNER_DATA,
        0,
        "68dd4b1bfd9676cdbdd12145b92fcb594839897a7eaf99636af753d52a90f968",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod source_help_owner_data_catalog_tests {
    #[test]
    fn packaged_catalog_source_help_owner_data_decodes() {
        let _ = super::read_source_help_owner_catalog();
    }
}
// End generated catalog reader: razer-pages/source_help_owner_data.
