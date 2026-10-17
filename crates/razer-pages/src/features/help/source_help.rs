//! Retained Help UI from each product's mounted current source component.
use gpui_kit::base::{Button as BaseButton, ElementExt as _};
use gpui_kit::component::button::Button;
use gpui_kit::component::spinner::Spinner;
use gpui_kit::component::*;
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_i18n as i18n;
use razer_model::model::Device;
use razer_widgets::surface;
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
    time::Duration,
};
#[path = "source_help_firmware.rs"]
mod firmware;
#[path = "source_help_inline.rs"]
mod inline;
#[path = "source_help_popup_layout.rs"]
mod popup_layout;
pub(super) use popup_layout::HelpScrollRequested;
#[path = "source_help_camo.rs"]
mod camo;
#[path = "source_help_reset.rs"]
mod reset_overlay;
#[path = "source_help_runtime_versions.rs"]
mod runtime_versions;
#[path = "source_help_system.rs"]
mod system;
#[path = "source_help_versions.rs"]
mod versions;
pub use camo::{HelpCamoCommand, HelpCamoObservation, HelpCamoRequested};
pub use runtime_versions::{HelpRuntimeVersionsObservation, HelpRuntimeVersionsRequest};
pub use system::{HelpSystemInfoCommand, HelpSystemInfoRequested};

#[derive(Deserialize)]
struct Help {
    product_id: u32,
    pages: Vec<HelpPage>,
}
#[derive(Deserialize)]
struct HelpPage {
    offset: usize,
    serial: bool,
    firmware: bool,
    view_more: bool,
    reset: bool,
    oled_reset: bool,
    reset_title: String,
    obm: bool,
    // DeviceInfo selects the message; the actual mounted callback selects
    // the command. Those decisions differ on several current products.
    #[serde(default)]
    reset_callback: Option<HelpResetCallback>,
    firmware_reset: Option<serde_json::Value>,
    system_info: bool,
    tutorial: bool,
    thx_instructions: bool,
    camo: bool,
    #[serde(default)]
    inline_confirmation: bool,
    #[serde(default)]
    popup_kind: Option<String>,
    #[serde(default)]
    popup_message: Option<String>,
    #[serde(default)]
    cooldown_ms: Option<u64>,
    #[serde(default)]
    obm_reset_during_ble: bool,
    #[serde(default)]
    plain_columns: bool,
    #[serde(default)]
    support_only: bool,
    #[serde(default)]
    wait_for_obm_reset_completion: bool,
    #[serde(default)]
    show_factory_reset_overlay: bool,
    #[serde(default)]
    disable_factory_reset_for_ble: bool,
}
#[derive(Deserialize)]
struct HelpResetCallback {
    action: String,
    payload: bool,
    #[serde(default)]
    payload_value: Option<serde_json::Value>,
}
fn records() -> &'static [Help] {
    static RECORDS: OnceLock<Vec<Help>> = OnceLock::new();
    RECORDS.get_or_init(|| read_source_help_catalog())
}

/// Internet-only Help mounts have no local destination after their links are removed.
pub(super) fn is_internet_only(product_id: u32, offset: usize) -> bool {
    records()
        .iter()
        .find(|help| help.product_id == product_id)
        .and_then(|help| help.pages.iter().find(|page| page.offset == offset))
        .is_some_and(|page| page.support_only)
}

pub(super) struct SourceHelp {
    device: Device,
    mounted_serial: String,
    localized_product_names: Option<BTreeMap<String, String>>,
    page_offset: Option<usize>,
    copied_serial: bool,
    view_more: bool,
    copy_task: Option<Task<()>>,
    runtime_versions: runtime_versions::State,
    camo: camo::State,
    system_info: system::State,
    confirmation_generation: u64,
    reset_generation: u64,
    pending_reset: bool,
    reset_cooldown: bool,
    reset_cooldown_task: Option<Task<()>>,
    reset_error: Option<String>,
    inline_reset: bool,
    popup_layout: popup_layout::State,
    reset_overlay_acknowledged: bool,
    reset_overlay_focus: FocusHandle,
    observed_ble: Option<bool>,
    reset_source_action: &'static str,
    reset_source_payload: bool,
    reset_source_payload_value: Option<&'static serde_json::Value>,
    reset_oled_obm: Option<bool>,
    observed_oled_resetting: bool,
    audio_restart: bool,
    audio_reset_generations: BTreeSet<u64>,
    audio_reset_tasks: BTreeMap<u64, Task<()>>,
}
#[derive(Clone, Copy, Debug)]
pub struct HelpResetRequest {
    generation: u64,
    audio_streams: bool,
    source_action: Option<&'static str>,
    source_payload: bool,
    source_payload_value: Option<&'static serde_json::Value>,
    oled_reset_obm: Option<bool>,
    firmware_hardware_reset: bool,
    oled_factory_reset: bool,
    oled_source_clone_failure: bool,
}
/// Actual query identity accompanies the raw source document separately.
/// Transport fields must never be injected into the original storage document.
pub struct HelpResetOutcome {
    pub source_document: serde_json::Value,
    pub serial_number: String,
}
impl HelpResetRequest {
    /// PID 1383's Help broadcasts this independently of its resetObm/BLE branch.
    pub fn oled_factory_reset_message(self) -> Option<serde_json::Value> {
        self.oled_factory_reset
            .then(|| serde_json::json!({"type":"ON_OLED_FACTORY_RESET_DEVICE"}))
    }
    /// The current static initializer retains a lazy function inside emote.src.
    /// It cannot be structured-cloned as ON_RESET_OLED; never replace it with
    /// guessed media or claim the original reset submission succeeded.
    pub fn oled_source_clone_failure(self) -> bool {
        self.oled_source_clone_failure
    }
    /// FirmwareFactoryReset's callback runs after Help's resetDevice. This
    /// second command uses the original middleware channel message format.
    pub fn hardware_reset_message(self) -> Option<serde_json::Value> {
        self.firmware_hardware_reset
            .then(|| serde_json::json!({"msgName":"uiRequest", "msgData":"resetHardware"}))
    }
    /// JSON-equivalent BroadcastChannel command; JS `timerTick: undefined`
    /// has no serialized field. Native transport bytes remain separate.
    pub fn source_message(self) -> Option<serde_json::Value> {
        if self.oled_reset_obm.is_some() {
            return None;
        }
        match self.source_action {
            Some("ON_RESET_OBM") => Some(serde_json::json!({
                "type": "ON_RESET_OBM",
                "payload": {},
            })),
            Some("ON_RESET_DEVICE") => Some(if self.source_payload {
                serde_json::json!({"type":"ON_RESET_DEVICE", "payload":self.source_payload_value.cloned().unwrap_or_else(|| serde_json::json!({}))})
            } else {
                serde_json::json!({"type":"ON_RESET_DEVICE"})
            }),
            _ => None,
        }
    }
    pub fn source_action(self) -> Option<&'static str> {
        self.source_action
    }
    /// Mounted audio owners create a fresh default profile. The owner must
    /// generate its source hostname-based name/GUID and submit ON_ADD_PROFILE;
    /// this request alone is not a device reset or successful local profile.
    pub fn creates_default_profile(self) -> bool {
        self.source_action == Some("ADD_PROFILE")
    }
    /// Build the original command from actual owner observations. `guid` must
    /// be a freshly generated UUID v4, matching the source crypto helper.
    pub fn default_profile_message(
        self,
        computer_name: &str,
        profiles: &[serde_json::Value],
        guid: &str,
    ) -> Option<serde_json::Value> {
        if !self.creates_default_profile() {
            return None;
        }
        let base = if computer_name.is_empty() {
            "Default".to_owned()
        } else {
            format!("{computer_name}-Default")
        };
        let mut name = base.clone();
        // Preserve AM's original limit and its final unchecked suffix 100.
        for index in 1..=100 {
            if !profiles.iter().any(|profile| profile["name"] == name) {
                break;
            }
            name = format!("{base} {index}");
        }
        Some(serde_json::json!({
            "type": "ON_ADD_PROFILE",
            "payload": {"newProfile": {"name": name, "guid": guid}},
        }))
    }
    /// Actual mounted PID 691 resetOled thunk argument. The OLED reducer
    /// resolves emote resources before middleware submission; this is not a
    /// BroadcastChannel ON_RESET_OBM command.
    pub fn oled_reset_obm(self) -> Option<bool> {
        self.oled_reset_obm
    }
    pub fn generation(self) -> u64 {
        self.generation
    }
    pub fn is_audio_streams(self) -> bool {
        self.audio_streams
    }
}
pub(super) enum HelpResetEvent {
    Requested(HelpResetRequest),
    Canceled(HelpResetRequest),
}
impl EventEmitter<HelpResetEvent> for SourceHelp {}
impl EventEmitter<super::source_help_owner::HelpOwnerOpenRequested> for SourceHelp {}
impl SourceHelp {
    /// The source unmounts Help when switching back to a product tab. Retained
    /// GPUI state must cancel the corresponding callbacks and popover as well.
    pub(super) fn leave_page(&mut self, cx: &mut Context<Self>) {
        if self.pending_reset {
            cx.emit(HelpResetEvent::Canceled(HelpResetRequest {
                generation: self.reset_generation,
                audio_streams: false,
                source_action: Some(self.reset_source_action),
                source_payload: self.reset_source_payload,
                source_payload_value: self.reset_source_payload_value,
                oled_reset_obm: self.reset_oled_obm,
                firmware_hardware_reset: false,
                oled_factory_reset: false,
                oled_source_clone_failure: false,
            }));
        }
        self.copy_task = None;
        self.runtime_versions.unmount();
        self.camo.unmount();
        self.system_info.unmount();
        self.copied_serial = false;
        self.view_more = false;
        self.inline_reset = false;
        self.pending_reset = false;
        self.reset_cooldown = false;
        self.reset_cooldown_task = None;
        self.reset_error = None;
        self.confirmation_generation = self.confirmation_generation.wrapping_add(1);
        self.reset_generation = self.reset_generation.wrapping_add(1);
        self.page_offset = None;
        self.popup_layout.unmount();
        self.cancel_audio_resets(cx);
        cx.notify();
    }
    pub(super) fn new(device: Device, cx: &mut Context<Self>) -> Self {
        Self {
            mounted_serial: device.serial_number.clone(),
            device,
            page_offset: None,
            copied_serial: false,
            view_more: false,
            copy_task: None,
            runtime_versions: runtime_versions::State::new(cx.entity_id()),
            localized_product_names: None,
            camo: camo::State::new(cx.entity_id()),
            system_info: system::State::new(cx.entity_id()),
            confirmation_generation: 0,
            reset_generation: 0,
            pending_reset: false,
            reset_cooldown: false,
            reset_cooldown_task: None,
            reset_error: None,
            inline_reset: false,
            popup_layout: popup_layout::State::default(),
            reset_overlay_acknowledged: false,
            reset_overlay_focus: cx.focus_handle(),
            observed_ble: None,
            reset_source_action: "ON_RESET_DEVICE",
            reset_source_payload: false,
            reset_source_payload_value: None,
            reset_oled_obm: None,
            observed_oled_resetting: false,
            audio_restart: false,
            audio_reset_generations: BTreeSet::new(),
            audio_reset_tasks: BTreeMap::new(),
        }
    }
    pub(super) fn set_device(&mut self, device: &Device, cx: &mut Context<Self>) {
        let camo_serial_changed = self.metadata().is_some_and(|(_, page)| page.camo)
            && self.device.serial_number != device.serial_number
            && self.device.product_id == device.product_id
            && self.device.device_container_id == device.device_container_id;
        if self.device.product_id != device.product_id
            || (self.device.serial_number != device.serial_number && !camo_serial_changed)
            || self.device.device_container_id != device.device_container_id
        {
            self.mounted_serial = device.serial_number.clone();
            if self.pending_reset {
                cx.emit(HelpResetEvent::Canceled(HelpResetRequest {
                    generation: self.reset_generation,
                    audio_streams: false,
                    source_action: Some(self.reset_source_action),
                    source_payload: self.reset_source_payload,
                    source_payload_value: self.reset_source_payload_value,
                    oled_reset_obm: self.reset_oled_obm,
                    firmware_hardware_reset: false,
                    oled_factory_reset: false,
                    oled_source_clone_failure: false,
                }));
            }
            self.copy_task = None;
            self.localized_product_names = None;
            self.runtime_versions.unmount();
            self.camo.unmount();
            self.system_info.unmount();
            self.copied_serial = false;
            self.view_more = false;
            self.reset_generation = self.reset_generation.wrapping_add(1);
            self.confirmation_generation = self.confirmation_generation.wrapping_add(1);
            self.pending_reset = false;
            self.reset_cooldown = false;
            self.reset_cooldown_task = None;
            self.reset_error = None;
            self.inline_reset = false;
            self.observed_ble = None;
            self.observed_oled_resetting = false;
            self.cancel_audio_resets(cx);
        }
        if self.device.product_id != device.product_id {
            self.popup_layout.unmount();
            self.page_offset = None;
        }
        self.device = device.clone();
        if camo_serial_changed {
            self.camo_serial_changed(&device.serial_number, cx);
        }
        cx.notify();
    }
    pub(super) fn set_page(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.page_offset != Some(offset) {
            if self.pending_reset {
                cx.emit(HelpResetEvent::Canceled(HelpResetRequest {
                    generation: self.reset_generation,
                    audio_streams: false,
                    source_action: Some(self.reset_source_action),
                    source_payload: self.reset_source_payload,
                    source_payload_value: self.reset_source_payload_value,
                    oled_reset_obm: self.reset_oled_obm,
                    firmware_hardware_reset: false,
                    oled_factory_reset: false,
                    oled_source_clone_failure: false,
                }));
            }
            self.page_offset = Some(offset);
            self.mounted_serial = self.device.serial_number.clone();
            self.runtime_versions.unmount();
            self.view_more = false;
            self.reset_generation = self.reset_generation.wrapping_add(1);
            self.confirmation_generation = self.confirmation_generation.wrapping_add(1);
            self.pending_reset = false;
            self.reset_cooldown = false;
            self.reset_cooldown_task = None;
            self.reset_error = None;
            self.inline_reset = false;
            self.mount_inline_popup_layout(cx);
            self.mount_camo(cx);
            self.mount_system_info(cx);
            self.cancel_audio_resets(cx);
            cx.notify();
        }
    }
    fn metadata(&self) -> Option<(&'static Help, &'static HelpPage)> {
        let help = records()
            .iter()
            .find(|h| h.product_id == self.device.product_id)?;
        let page = if let Some(offset) = self.page_offset {
            help.pages.iter().find(|p| p.offset == offset)?
        } else {
            help.pages.first()?
        };
        Some((help, page))
    }
    fn copy_serial(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.copied_serial {
            return;
        }
        let serial = if self.metadata().is_some_and(|(_, page)| page.camo) {
            self.camo_serial()
        } else if self.retains_mounted_serial() {
            &self.mounted_serial
        } else {
            &self.device.serial_number
        };
        cx.write_to_clipboard(ClipboardItem::new_string(serial.to_owned()));
        self.copied_serial = true;
        self.copy_task = Some(cx.spawn_in(window, async move |view, cx| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            let _ = view.update(cx, |view, cx| {
                view.copied_serial = false;
                cx.notify();
            });
        }));
        cx.notify();
    }
    fn show_reset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some((_, page)) = self.metadata() else {
            return;
        };
        if !page.reset && !(page.oled_reset && !self.device.use_ble) {
            return;
        }
        if self.reset_cooldown || self.observed_oled_resetting || self.factory_reset_disabled(page)
        {
            return;
        }
        if page.inline_confirmation {
            if self.inline_reset {
                return;
            }
            self.inline_reset = true;
            self.confirmation_generation = self.confirmation_generation.wrapping_add(1);
            if page.firmware_reset.is_some() {
                // FirmwareFactoryReset conditionally mounts its popup with
                // active=true; the source runs the initial 50ms overflow check.
                self.mount_inline_popup_layout(cx);
            } else {
                self.activate_inline_popup_layout(cx);
            }
            cx.notify();
            return;
        }
        // These current Help classes' confirmDel emits ON_RESET_DEVICE.
        // Other reset variants (OBM/OLED/firmware) remain separately audited.
        let local_reset = matches!(self.device.product_id, 164 | 241);
        let title = self.device.display_name();
        let message = if page.oled_reset && !self.device.use_ble {
            i18n::t(&page.reset_title)
        } else if let Some(reset) = &page.firmware_reset {
            i18n::t(
                reset
                    .get("modalMsg")
                    .and_then(|v| v.as_str())
                    .unwrap_or("FACTORY_RESET_MSG"),
            )
        } else {
            i18n::t(if page.obm {
                "FACTORY_RESET_MSG"
            } else {
                "FACTORY_RESET_MSG_NO_OBM_DEVICE"
            })
        };
        self.confirmation_generation = self.confirmation_generation.wrapping_add(1);
        let generation = self.confirmation_generation;
        let owner = cx.weak_entity();
        window.open_dialog(cx, move |dialog, _, _cx| {
            dialog
                .title(razer_assets::resource_scope(title.clone()))
                .child(razer_assets::resource_scope(message.clone()))
                .on_close({
                    let owner = owner.clone();
                    move |_, _, cx| {
                        let _ = owner.update(cx, |view, _| {
                            if view.confirmation_generation == generation {
                                view.confirmation_generation =
                                    view.confirmation_generation.wrapping_add(1);
                            }
                        });
                    }
                })
                .footer(
                    razer_assets::resource_scope(h_flex()
                        .justify_end()
                        .gap_3()
                        .child(
                            Button::new("source-help-reset-cancel")
                                .label(i18n::t("CANCEL"))
                                .on_click({
                                    let owner = owner.clone();
                                    move |_, window, cx| {
                                        let _ = owner.update(cx, |view, _| {
                                            if view.confirmation_generation == generation {
                                                view.confirmation_generation =
                                                    view.confirmation_generation.wrapping_add(1);
                                            }
                                        });
                                        window.close_dialog(cx);
                                    }
                                }),
                        )
                        .child(
                            Button::new("source-help-reset-confirm")
                                .label(i18n::t("RESET"))
                                .disabled(!local_reset)
                                .on_click({
                                    let owner = owner.clone();
                                    move |_, window, cx| {
                                        let accepted = owner
                                            .update(cx, |view, cx| {
                                                if !local_reset
                                                    || view.confirmation_generation != generation
                                                {
                                                    return false;
                                                }
                                                view.pending_reset = true;
                                                view.reset_error = None;
                                                view.reset_generation =
                                                    view.reset_generation.wrapping_add(1);
                                                let request = HelpResetRequest {
                                                    generation: view.reset_generation,
                                                    audio_streams: false,
                                                    source_action: Some("ON_RESET_DEVICE"),
                                                    source_payload: false,
                                                    source_payload_value: None,
                                                    oled_reset_obm: None,
                                                    firmware_hardware_reset: false,
                                                    oled_factory_reset: false,
                                                    oled_source_clone_failure: false,
                                                };
                                                view.reset_cooldown = true;
                                                // Original Help resetDevice re-enables after two
                                                // seconds independently of middleware completion.
                                                view.reset_cooldown_task = Some(cx.spawn_in(
                                                    window,
                                                    async move |view, cx| {
                                                        cx.background_executor()
                                                            .timer(Duration::from_secs(2))
                                                            .await;
                                                        let _ = view.update(cx, |view, cx| {
                                                            if view.reset_generation
                                                                == request.generation
                                                            {
                                                                view.reset_cooldown = false;
                                                                cx.notify();
                                                            }
                                                        });
                                                    },
                                                ));
                                                cx.emit(HelpResetEvent::Requested(request));
                                                cx.notify();
                                                true
                                            })
                                            .unwrap_or(false);
                                        if accepted {
                                            window.close_dialog(cx);
                                        }
                                    }
                                }),
                        )),
                )
        });
    }
    pub(super) fn reset_matches(&self, request: HelpResetRequest) -> bool {
        if request.audio_streams {
            return self.audio_reset_generations.contains(&request.generation);
        }
        self.pending_reset && self.reset_generation == request.generation
    }
    fn cancel_audio_resets(&mut self, cx: &mut Context<Self>) {
        for generation in std::mem::take(&mut self.audio_reset_generations) {
            cx.emit(HelpResetEvent::Canceled(HelpResetRequest {
                generation,
                audio_streams: true,
                source_action: None,
                source_payload: false,
                source_payload_value: None,
                oled_reset_obm: None,
                firmware_hardware_reset: false,
                oled_factory_reset: false,
                oled_source_clone_failure: false,
            }));
        }
        self.audio_reset_tasks.clear();
        self.audio_restart = false;
    }
    fn restart_audio(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.device.product_id != 1342 {
            return;
        }
        // Current ug.resetAudio sets the spinner, delays 1000 ms, invokes its
        // actual resetAudio prop and removes the spinner without awaiting IO.
        // Repeated clicks each retain their own original timer/request.
        self.audio_restart = true;
        self.reset_generation = self.reset_generation.wrapping_add(1);
        let request = HelpResetRequest {
            generation: self.reset_generation,
            audio_streams: true,
            source_action: None,
            source_payload: false,
            source_payload_value: None,
            oled_reset_obm: None,
            firmware_hardware_reset: false,
            oled_factory_reset: false,
            oled_source_clone_failure: false,
        };
        self.audio_reset_generations.insert(request.generation);
        let task = cx.spawn_in(window, async move |view, cx| {
            cx.background_executor().timer(Duration::from_secs(1)).await;
            let _ = view.update(cx, |view, cx| {
                if !view.reset_matches(request) {
                    return;
                }
                view.audio_restart = false;
                if let Some(task) = view.audio_reset_tasks.remove(&request.generation) {
                    task.detach();
                }
                cx.emit(HelpResetEvent::Requested(request));
                cx.notify();
            });
        });
        self.audio_reset_tasks.insert(request.generation, task);
        cx.notify();
    }
    pub(super) fn accept_source_serial(&mut self, serial: &str, cx: &mut Context<Self>) {
        if self.device.serial_number != serial {
            self.device.serial_number = serial.to_owned();
            self.camo_serial_changed(serial, cx);
            if !self.retains_mounted_serial() {
                self.copy_task = None;
                self.copied_serial = false;
            }
        }
    }
    pub(super) fn finish_reset(
        &mut self,
        request: HelpResetRequest,
        error: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if !self.reset_matches(request) {
            return;
        }
        if request.audio_streams {
            self.audio_reset_generations.remove(&request.generation);
            // This action does not replace profiles or claim recovered audio.
            cx.notify();
            return;
        }
        self.pending_reset = false;
        self.reset_error = error;
        cx.notify();
    }
}

fn help_button(id: &'static str, label: String, disabled: bool, cx: &App) -> BaseButton {
    BaseButton::new(id)
        .accessibility_label(label.clone())
        .child(label)
        .disabled(disabled)
        .flex()
        .items_center()
        .justify_center()
        .bg(cx.theme().button)
        .text_color(cx.theme().button_foreground)
        .styles(|style| style.disabled(|style| style.opacity(0.3)))
        .h(surface::css(27.))
        .min_w(surface::css(100.))
        .border_1()
        .border_color(cx.theme().title_bar)
        .rounded(surface::css(3.))
        .px(surface::css(12.))
        .py_0()
        .text_size(surface::css(12.))
        .when(!disabled, |button| button.hover(|style| style.opacity(0.8)))
        .focus_visible(|style| style.border_color(cx.theme().primary))
}
fn help_panel(title: &str, block_column: bool, cx: &App) -> Div {
    // Source widget-col is a block container. Adjacent 10px card margins
    // collapse to 10px. GPUI flex layout needs one explicit margin per card;
    // the column supplies the final 10px bottom margin.
    surface::panel(i18n::t(title), cx).when(block_column, |panel| panel.mb_0())
}
fn audio_setup_action(id: &'static str, key: &str, section: &'static str, cx: &App) -> BaseButton {
    let label = i18n::t(key);
    BaseButton::new(id)
        .accessibility_label(label.clone())
        .disabled(!cfg!(windows))
        .self_start()
        .p_0()
        .mb(surface::css(10.))
        .text_color(cx.theme().foreground)
        .underline()
        .focus_visible(|style| style.text_color(cx.theme().primary))
        .child(label)
        .on_click(move |_, _, _| {
            let result = match section {
                "sound" => razer_platform::system::open_sound_settings(),
                "apps-volume" => razer_platform::system::open_apps_volume_settings(),
                _ => return,
            };
            // The original native helper is a void dispatch, without a result
            // dialog or success state. Keep failures separate from UI state.
            if let Err(error) = result {
                eprintln!("[help-mixer] msSettings({section}) failed: {error:#}");
            }
        })
}
fn audio_setup_list(keys: &[&str], id: &'static str, section: &'static str, cx: &App) -> Div {
    // ug renders ol/li, not numbered paragraphs. Browser UA ol margins remain
    // a separate acceptance gap; only its current stylesheet is projected here.
    v_flex()
        .pl(surface::css(15.))
        .children(keys.iter().enumerate().map(|(ix, key)| {
            let row = div().relative().child(
                div()
                    .absolute()
                    .left(surface::css(-15.))
                    .child(format!("{}.", ix + 1)),
            );
            if ix == 0 {
                row.child(audio_setup_action(id, key, section, cx))
                    .into_any_element()
            } else {
                row.mb(surface::css(10.))
                    .child(i18n::t(key))
                    .into_any_element()
            }
        }))
}
impl Render for SourceHelp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some((_, page)) = self.metadata() else {
            // Missing source metadata remains an implementation gap; the
            // original interface has no reverse-engineering status paragraph.
            return div().into_any_element();
        };
        // Enabled only by independently audited mounted Help callers and CSS.
        // Retained local cards keep their source column spacing.
        let exact_help = page.plain_columns;
        let reset_disabled = self.factory_reset_disabled(page);
        let oled_popup = matches!(page.popup_kind.as_deref(), Some("oled" | "audio-oled"))
            && !self.observed_ble.unwrap_or(self.device.use_ble);
        let legacy_help = page.popup_kind.as_deref() == Some("legacy-delete");
        if page.support_only {
            return div().into_any_element();
        }
        let mut left = v_flex()
            .gap(surface::css(if exact_help { 0. } else { 20. }))
            .when(exact_help, |column| column.pb(surface::css(10.)));
        if page.system_info {
            if matches!(
                self.device.product_id,
                694 | 695 | 696 | 709 | 710 | 711 | 736 | 737
            ) {
                left = left.child(self.system_info_panel(exact_help, cx));
            }
        }
        if page.tutorial {
            let supported = super::source_help_owner::supports_tutorial(self.device.product_id);
            left = left.child(
                surface::panel(
                    super::source_help_owner::message(self.device.product_id, "TUTORIAL"),
                    cx,
                )
                .when(exact_help, |panel| panel.mb_0())
                .child(super::source_help_owner::message(
                    self.device.product_id,
                    "TUTORIAL_DESC",
                ))
                .child(
                    help_button(
                        "source-help-tutorial",
                        super::source_help_owner::message(self.device.product_id, "VIEW_AGAIN")
                            .to_uppercase(),
                        !supported,
                        cx,
                    )
                    .bg(razer_widgets::theme::HelpOwnerColors::button())
                    .text_color(razer_widgets::theme::HelpOwnerColors::plain_button_text())
                    .border_color(razer_widgets::theme::HelpOwnerColors::button_border())
                    .line_height(surface::css(14.))
                    .py(surface::css(6.))
                    .mt(surface::css(20.))
                    .on_click(cx.listener(move |_, _, _, cx| {
                        if supported {
                            cx.emit(super::source_help_owner::HelpOwnerOpenRequested(
                                super::source_help_owner::HelpOwnerOverlay::Tutorial,
                            ));
                        }
                    })),
                ),
            );
        }
        if page.thx_instructions {
            let supported = super::source_help_owner::supports_thx(self.device.product_id);
            left = left.child(
                surface::panel(
                    super::source_help_owner::message(self.device.product_id, "THX_SPATIAL_AUDIO"),
                    cx,
                )
                .when(exact_help, |panel| panel.mb_0())
                .child(super::source_help_owner::message(
                    self.device.product_id,
                    "THX_SPATIAL_AUDIO_NOTICE",
                ))
                .child(
                    BaseButton::new("source-help-thx")
                        .accessibility_label(super::source_help_owner::message(
                            self.device.product_id,
                            "SHOW_ME_HOW",
                        ))
                        .disabled(!supported)
                        .p_0()
                        .self_start()
                        .underline()
                        .text_color(razer_widgets::theme::HelpOwnerColors::text())
                        .hover(|style| {
                            style.text_color(razer_widgets::theme::HelpOwnerColors::accent())
                        })
                        .mt(surface::css(17.))
                        .child(super::source_help_owner::message(
                            self.device.product_id,
                            "SHOW_ME_HOW",
                        ))
                        .on_click(cx.listener(move |_, _, _, cx| {
                            if supported {
                                cx.emit(super::source_help_owner::HelpOwnerOpenRequested(
                                    super::source_help_owner::HelpOwnerOverlay::ThxInstructions,
                                ));
                            }
                        })),
                ),
            );
        }
        if page.camo && self.camo.installed() {
            left = left.child(self.camo_panel(exact_help, cx));
        }
        let audio_support = self.device.product_id == 1342;
        if audio_support {
            // Keep local setup/troubleshooting on the left and reset on the
            // right, with the original block-card margins.
            left = v_flex()
                .pb(surface::css(10.))
                .child(
                    help_panel("SETTING_IT_UP", exact_help, cx)
                        .child(i18n::t("SETTING_IT_UP_DES"))
                        .child(
                            div()
                                .mt(surface::css(20.))
                                .mb(surface::css(20.))
                                .child(i18n::t("SETTING_IT_UP_BASIC").to_uppercase()),
                        )
                        .child(audio_setup_list(
                            &[
                                "SETTING_IT_UP_BASIC_1",
                                "SETTING_IT_UP_BASIC_2",
                                "SETTING_IT_UP_BASIC_3",
                            ],
                            "source-help-sound-settings",
                            "sound",
                            cx,
                        ))
                        .child(
                            div()
                                .mt(surface::css(20.))
                                .mb(surface::css(20.))
                                .child(i18n::t("SETTING_IT_UP_ADVANCED").to_uppercase()),
                        )
                        .child(audio_setup_list(
                            &[
                                "SETTING_IT_UP_ADVANCED_1",
                                "SETTING_IT_UP_ADVANCED_2",
                                "SETTING_IT_UP_ADVANCED_3",
                                "SETTING_IT_UP_ADVANCED_4",
                            ],
                            "source-help-volume-settings",
                            "apps-volume",
                            cx,
                        )),
                )
                .child(
                    help_panel("AUDIO_TROUBLESHOOTING", exact_help, cx)
                        .child(i18n::t("AUDIO_TROUBLESHOOTING_DES"))
                        .child(
                            h_flex()
                                .child(
                                    help_button(
                                        "source-help-restart-audio",
                                        i18n::t("AUDIO_TROUBLESHOOTING_BUTTON"),
                                        false,
                                        cx,
                                    )
                                    .mt(surface::css(20.))
                                    .px(surface::css(20.))
                                    .pt(surface::css(7.))
                                    .pb(surface::css(8.))
                                    .on_click(cx.listener(
                                        |this, _, window, cx| this.restart_audio(window, cx),
                                    )),
                                )
                                .when(self.audio_restart, |row| {
                                    row.child(
                                        div()
                                            .relative()
                                            .top(surface::css(10.))
                                            .child(Spinner::new().small()),
                                    )
                                }),
                        ),
                );
        }
        if page.firmware_reset.is_some() {
            left = left.child(self.firmware_reset_panel(cx));
        }
        if !audio_support
            && page.firmware_reset.is_none()
            && (page.reset || page.oled_reset && !self.device.use_ble)
        {
            left = left.child(
                help_panel("FACTORY_RESET", exact_help, cx)
                    .id("source-help-reset-panel")
                    .relative()
                    .child(i18n::t(&page.reset_title))
                    .child(
                        v_flex()
                            .relative()
                            .items_start()
                            .child(
                                help_button(
                                    "source-help-reset",
                                    i18n::t("RESET"),
                                    self.reset_cooldown
                                        || self.observed_oled_resetting
                                        || reset_disabled,
                                    cx,
                                )
                                .self_start()
                                .when(exact_help, |button| {
                                    button.mt(surface::css(20.)).line_height(surface::css(14.))
                                })
                                .when(legacy_help, |button| {
                                    button
                                        .mt(surface::css(10.))
                                        .mr(surface::css(10.))
                                        .bg(razer_widgets::theme::SourceHelpPopupColors::legacy_button())
                                        .text_color(razer_widgets::theme::SourceHelpPopupColors::legacy_button_text())
                                        .px_0()
                                        .pt(surface::css(6.))
                                        .pb(surface::css(7.))
                                })
                                .on_click(
                                    cx.listener(|this, _, window, cx| this.show_reset(window, cx)),
                                ),
                            )
                            .when(
                                page.inline_confirmation && !oled_popup && !legacy_help,
                                |holder| holder.child(self.reset_popup(cx)),
                            ),
                    )
                    .when(oled_popup || legacy_help, |panel| {
                        panel.child(self.reset_popup(cx))
                    })
                    .when(reset_disabled, |panel| {
                        panel.opacity(0.3).tooltip(|window, cx| {
                            gpui_kit::component::tooltip::Tooltip::new(i18n::t(
                                "REQUIRES_WIRED_OR_HYPERSPEED_CONNECTION",
                            ))
                            .build(window, cx)
                        })
                    })
                    .when(
                        page.inline_confirmation
                            && (self.reset_cooldown || self.observed_oled_resetting),
                        |panel| panel.child(Spinner::new().small()),
                    ),
            );
        }
        let serial = if self.retains_mounted_serial() {
            &self.mounted_serial
        } else {
            &self.device.serial_number
        }
        .clone();
        let mut right = v_flex()
            .gap(surface::css(if exact_help { 0. } else { 20. }))
            .when(exact_help, |column| column.pb(surface::css(10.)));
        if audio_support {
            right = right.child(
                help_panel("FACTORY_RESET", exact_help, cx)
                    .id("source-help-reset-panel")
                    .relative()
                    .child(i18n::t(&page.reset_title))
                    .child(
                        help_button(
                            "source-help-reset",
                            i18n::t("RESET"),
                            self.reset_cooldown,
                            cx,
                        )
                        .self_start()
                        .mt(surface::css(20.))
                        .mr(surface::css(10.))
                        .px(surface::css(20.))
                        .pt(surface::css(7.))
                        .pb(surface::css(8.))
                        .on_click(cx.listener(|this, _, window, cx| this.show_reset(window, cx))),
                    )
                    .when(page.inline_confirmation, |panel| {
                        panel.child(self.reset_popup(cx))
                    })
                    .when(self.reset_cooldown, |panel| {
                        panel.child(Spinner::new().small())
                    }),
            );
        }
        if page.serial && page.camo {
            right = right.child(self.camo_serial_panel(exact_help, cx));
        } else if page.serial {
            right = right.child(
                help_panel("SERIAL_NUM", exact_help, cx)
                    .child(format!("{} {serial}", i18n::t("SERIAL")))
                    .child(
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
                        .self_start()
                        .when(exact_help, |button| {
                            button.mt(surface::css(20.)).line_height(surface::css(14.))
                        })
                        .when(legacy_help, |button| {
                            button
                                .mt(surface::css(10.))
                                .mr(surface::css(10.))
                                .bg(razer_widgets::theme::SourceHelpPopupColors::legacy_button())
                                .text_color(
                                    razer_widgets::theme::SourceHelpPopupColors::legacy_button_text(
                                    ),
                                )
                                .px_0()
                                .pt(surface::css(6.))
                                .pb(surface::css(7.))
                        })
                        .on_click(cx.listener(|this, _, window, cx| this.copy_serial(window, cx))),
                    ),
            );
        }
        let firmware = self.device.current_firmware_version();
        if page.firmware && !firmware.is_empty() {
            let mut panel = help_panel(
                if audio_support {
                    "FIRMWARE_VERSION_HEADER"
                } else {
                    "DEVICE_HEADER"
                },
                exact_help,
                cx,
            )
            .child(format!(
                "{}: {firmware}",
                i18n::t(if audio_support {
                    "CURRENT_VERSION"
                } else {
                    "FIRMWARE_VERSION"
                })
            ));
            if page.view_more {
                if self.view_more {
                    if let Some(version) = versions::ui_version(self.device.product_id) {
                        panel = panel.child(
                            div()
                                .mt(surface::css(10.))
                                .child(format!("{}: {version}", i18n::t("UI_VERSION"))),
                        );
                    }
                    for (label, version) in self.runtime_versions.rows() {
                        panel = panel.child(
                            div()
                                .mt(surface::css(10.))
                                .child(format!("{}: {version}", i18n::t(label))),
                        );
                    }
                }
                let label = i18n::t(if self.view_more { "VIEW_LESS" } else { "VIEW_MORE" });
                panel = panel.child(
                    BaseButton::new("source-help-versions")
                        .accessibility_label(label.clone())
                        .self_start()
                        .flex()
                        .items_center()
                        .gap(surface::css(4.))
                        .px_0()
                        .py(surface::css(15.))
                        .hover(|style| style.text_color(cx.theme().primary))
                        .child(div().underline().child(label))
                        .child(
                            svg()
                                .path(if self.view_more {
                                    "synapse/help-less.svg"
                                } else {
                                    "synapse/help-more.svg"
                                })
                                .size(surface::css(20.)),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.view_more = !this.view_more;
                            cx.notify();
                        })),
                );
            }
            right = right.child(panel);
        }
        let help_bounds = self.popup_layout.help_bounds.clone();
        let unit = self.popup_layout.unit.clone();
        let viewport_height = self.popup_layout.viewport_height.clone();
        surface::page_columns()
            .relative()
            .when(page.inline_confirmation, |columns| {
                columns.on_prepaint(move |bounds, window, _| {
                    help_bounds.set(bounds);
                    unit.set(window.rem_size() / 16.);
                    viewport_height.set(window.viewport_size().height);
                })
            })
            .when_some(self.popup_layout.reserved_height, |columns, height| {
                columns.h(height)
            })
            .when(exact_help, |columns| columns.gap_0())
            .child(surface::page_column(left))
            .child(surface::page_column(right))
            .when(
                page.show_factory_reset_overlay && self.reset_cooldown,
                |columns| columns.child(self.factory_reset_overlay(window, cx)),
            )
            .into_any_element()
    }
}

// Generated catalog reader: razer-pages/source_help_data.
fn read_source_help_catalog() -> Vec<Help> {
    fn decode_0(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<std::vec::Vec<Help>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Help> {
        let value = match r.tag()? {
            0 => Help {
                product_id: r.child(decode_2)?,
                pages: r.child(decode_3)?,
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
    ) -> anyhow::Result<std::vec::Vec<HelpPage>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_4)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<HelpPage> {
        let value = match r.tag()? {
            0 => HelpPage {
                offset: r.child(decode_5)?,
                serial: r.child(decode_6)?,
                firmware: r.child(decode_6)?,
                view_more: r.child(decode_6)?,
                reset: r.child(decode_6)?,
                oled_reset: r.child(decode_6)?,
                reset_title: r.child(decode_7)?,
                obm: r.child(decode_6)?,
                reset_callback: r.child(decode_8)?,
                firmware_reset: None,
                system_info: r.child(decode_6)?,
                tutorial: r.child(decode_6)?,
                thx_instructions: r.child(decode_6)?,
                camo: r.child(decode_6)?,
                inline_confirmation: r.child(decode_6)?,
                popup_kind: None,
                popup_message: None,
                cooldown_ms: None,
                obm_reset_during_ble: r.child(decode_6)?,
                plain_columns: r.child(decode_6)?,
                support_only: Default::default(),
                wait_for_obm_reset_completion: Default::default(),
                show_factory_reset_overlay: Default::default(),
                disable_factory_reset_for_ble: Default::default(),
            },
            1 => HelpPage {
                offset: r.child(decode_5)?,
                serial: r.child(decode_6)?,
                firmware: r.child(decode_6)?,
                view_more: r.child(decode_6)?,
                reset: r.child(decode_6)?,
                oled_reset: r.child(decode_6)?,
                reset_title: r.child(decode_7)?,
                obm: r.child(decode_6)?,
                reset_callback: r.child(decode_8)?,
                firmware_reset: None,
                system_info: r.child(decode_6)?,
                tutorial: r.child(decode_6)?,
                thx_instructions: r.child(decode_6)?,
                camo: r.child(decode_6)?,
                inline_confirmation: r.child(decode_6)?,
                popup_kind: r.child(decode_10)?,
                popup_message: None,
                cooldown_ms: r.child(decode_11)?,
                obm_reset_during_ble: r.child(decode_6)?,
                plain_columns: r.child(decode_6)?,
                support_only: Default::default(),
                wait_for_obm_reset_completion: Default::default(),
                show_factory_reset_overlay: Default::default(),
                disable_factory_reset_for_ble: Default::default(),
            },
            2 => HelpPage {
                offset: r.child(decode_5)?,
                serial: r.child(decode_6)?,
                firmware: r.child(decode_6)?,
                view_more: r.child(decode_6)?,
                reset: r.child(decode_6)?,
                oled_reset: r.child(decode_6)?,
                reset_title: r.child(decode_7)?,
                obm: r.child(decode_6)?,
                reset_callback: r.child(decode_8)?,
                firmware_reset: None,
                system_info: r.child(decode_6)?,
                tutorial: r.child(decode_6)?,
                thx_instructions: r.child(decode_6)?,
                camo: r.child(decode_6)?,
                inline_confirmation: r.child(decode_6)?,
                popup_kind: r.child(decode_10)?,
                popup_message: None,
                cooldown_ms: None,
                obm_reset_during_ble: Default::default(),
                plain_columns: r.child(decode_6)?,
                support_only: Default::default(),
                wait_for_obm_reset_completion: Default::default(),
                show_factory_reset_overlay: Default::default(),
                disable_factory_reset_for_ble: Default::default(),
            },
            3 => HelpPage {
                offset: r.child(decode_5)?,
                serial: r.child(decode_6)?,
                firmware: r.child(decode_6)?,
                view_more: r.child(decode_6)?,
                reset: r.child(decode_6)?,
                oled_reset: r.child(decode_6)?,
                reset_title: r.child(decode_7)?,
                obm: r.child(decode_6)?,
                reset_callback: r.child(decode_8)?,
                firmware_reset: None,
                system_info: r.child(decode_6)?,
                tutorial: r.child(decode_6)?,
                thx_instructions: r.child(decode_6)?,
                camo: r.child(decode_6)?,
                inline_confirmation: r.child(decode_6)?,
                popup_kind: None,
                popup_message: None,
                cooldown_ms: None,
                obm_reset_during_ble: r.child(decode_6)?,
                plain_columns: r.child(decode_6)?,
                support_only: Default::default(),
                wait_for_obm_reset_completion: r.child(decode_6)?,
                show_factory_reset_overlay: r.child(decode_6)?,
                disable_factory_reset_for_ble: r.child(decode_6)?,
            },
            4 => HelpPage {
                offset: r.child(decode_5)?,
                serial: r.child(decode_6)?,
                firmware: r.child(decode_6)?,
                view_more: r.child(decode_6)?,
                reset: r.child(decode_6)?,
                oled_reset: r.child(decode_6)?,
                reset_title: r.child(decode_7)?,
                obm: r.child(decode_6)?,
                reset_callback: None,
                firmware_reset: None,
                system_info: r.child(decode_6)?,
                tutorial: r.child(decode_6)?,
                thx_instructions: r.child(decode_6)?,
                camo: r.child(decode_6)?,
                inline_confirmation: Default::default(),
                popup_kind: None,
                popup_message: None,
                cooldown_ms: None,
                obm_reset_during_ble: Default::default(),
                plain_columns: r.child(decode_6)?,
                support_only: r.child(decode_6)?,
                wait_for_obm_reset_completion: Default::default(),
                show_factory_reset_overlay: Default::default(),
                disable_factory_reset_for_ble: Default::default(),
            },
            5 => HelpPage {
                offset: r.child(decode_5)?,
                serial: r.child(decode_6)?,
                firmware: r.child(decode_6)?,
                view_more: r.child(decode_6)?,
                reset: r.child(decode_6)?,
                oled_reset: r.child(decode_6)?,
                reset_title: r.child(decode_7)?,
                obm: r.child(decode_6)?,
                reset_callback: r.child(decode_8)?,
                firmware_reset: None,
                system_info: r.child(decode_6)?,
                tutorial: r.child(decode_6)?,
                thx_instructions: r.child(decode_6)?,
                camo: r.child(decode_6)?,
                inline_confirmation: r.child(decode_6)?,
                popup_kind: r.child(decode_10)?,
                popup_message: None,
                cooldown_ms: None,
                obm_reset_during_ble: r.child(decode_6)?,
                plain_columns: r.child(decode_6)?,
                support_only: Default::default(),
                wait_for_obm_reset_completion: Default::default(),
                show_factory_reset_overlay: Default::default(),
                disable_factory_reset_for_ble: Default::default(),
            },
            6 => HelpPage {
                offset: r.child(decode_5)?,
                serial: r.child(decode_6)?,
                firmware: r.child(decode_6)?,
                view_more: r.child(decode_6)?,
                reset: r.child(decode_6)?,
                oled_reset: r.child(decode_6)?,
                reset_title: r.child(decode_7)?,
                obm: r.child(decode_6)?,
                reset_callback: r.child(decode_8)?,
                firmware_reset: None,
                system_info: r.child(decode_6)?,
                tutorial: r.child(decode_6)?,
                thx_instructions: r.child(decode_6)?,
                camo: r.child(decode_6)?,
                inline_confirmation: r.child(decode_6)?,
                popup_kind: r.child(decode_10)?,
                popup_message: r.child(decode_10)?,
                cooldown_ms: r.child(decode_11)?,
                obm_reset_during_ble: r.child(decode_6)?,
                plain_columns: r.child(decode_6)?,
                support_only: Default::default(),
                wait_for_obm_reset_completion: Default::default(),
                show_factory_reset_overlay: Default::default(),
                disable_factory_reset_for_ble: Default::default(),
            },
            7 => HelpPage {
                offset: r.child(decode_5)?,
                serial: r.child(decode_6)?,
                firmware: r.child(decode_6)?,
                view_more: r.child(decode_6)?,
                reset: r.child(decode_6)?,
                oled_reset: r.child(decode_6)?,
                reset_title: r.child(decode_7)?,
                obm: r.child(decode_6)?,
                reset_callback: r.child(decode_8)?,
                firmware_reset: r.child(decode_13)?,
                system_info: r.child(decode_6)?,
                tutorial: r.child(decode_6)?,
                thx_instructions: r.child(decode_6)?,
                camo: r.child(decode_6)?,
                inline_confirmation: r.child(decode_6)?,
                popup_kind: r.child(decode_10)?,
                popup_message: r.child(decode_10)?,
                cooldown_ms: None,
                obm_reset_during_ble: Default::default(),
                plain_columns: r.child(decode_6)?,
                support_only: Default::default(),
                wait_for_obm_reset_completion: Default::default(),
                show_factory_reset_overlay: Default::default(),
                disable_factory_reset_for_ble: Default::default(),
            },
            8 => HelpPage {
                offset: r.child(decode_5)?,
                serial: r.child(decode_6)?,
                firmware: r.child(decode_6)?,
                view_more: r.child(decode_6)?,
                reset: r.child(decode_6)?,
                oled_reset: r.child(decode_6)?,
                reset_title: r.child(decode_7)?,
                obm: r.child(decode_6)?,
                reset_callback: None,
                firmware_reset: None,
                system_info: r.child(decode_6)?,
                tutorial: r.child(decode_6)?,
                thx_instructions: r.child(decode_6)?,
                camo: r.child(decode_6)?,
                inline_confirmation: Default::default(),
                popup_kind: None,
                popup_message: None,
                cooldown_ms: None,
                obm_reset_during_ble: Default::default(),
                plain_columns: Default::default(),
                support_only: Default::default(),
                wait_for_obm_reset_completion: Default::default(),
                show_factory_reset_overlay: Default::default(),
                disable_factory_reset_for_ble: Default::default(),
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<usize> {
        let value = match r.tag()? {
            0 => r.usize()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_8(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<HelpResetCallback>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_9)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_9(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<HelpResetCallback> {
        let value = match r.tag()? {
            0 => HelpResetCallback {
                action: r.child(decode_7)?,
                payload: r.child(decode_6)?,
                payload_value: None,
            },
            1 => HelpResetCallback {
                action: r.child(decode_7)?,
                payload: r.child(decode_6)?,
                payload_value: r.child(decode_13)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_10(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<String>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_7)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_11(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<u64>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_12)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_12(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u64> {
        let value = match r.tag()? {
            0 => r.u64()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_13(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<serde_json::Value>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_14)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_14(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<serde_json::Value> {
        let value = match r.tag()? {
            0 => r.external_value()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_SOURCE_HELP_DATA,
        0,
        "06682498f4d2c498098c5ed303f2be14d00780bc4f503270959d22c5575ddf22",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod source_help_data_catalog_tests {
    #[test]
    fn packaged_catalog_source_help_data_decodes() {
        let _ = super::read_source_help_catalog();
    }
}
// End generated catalog reader: razer-pages/source_help_data.
