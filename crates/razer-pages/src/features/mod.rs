//! Audited device workspace. Domain edits and retained controls are owned here.
//! The previous free-function pages are no longer compiled into the app.
#[path = "audio/audio_page.rs"]
mod audio_page;
#[path = "workspace/controls.rs"]
mod controls;
#[path = "mapping/mapping_device_runtime.rs"]
pub(crate) mod mapping_device_runtime;
#[path = "workspace/source_catalog.rs"]
mod source_catalog;
pub use mapping_device_runtime::MappingCallbackRequested;
#[path = "mapping/mapping_key_capture_runtime.rs"]
mod mapping_key_capture_runtime;
#[path = "mapping/mapping_text_editor.rs"]
mod mapping_text_editor;
#[path = "mapping/source_key.rs"]
pub(crate) mod source_key;
pub fn active_mapping_workers() -> usize {
    mapping_device_runtime::active_workers() + mapping_key_capture_runtime::active_workers()
}
pub use controls::Choice;
#[path = "workspace/adjustment_prompt.rs"]
mod adjustment_prompt;
#[path = "customize/customize_drawer.rs"]
mod customize_drawer;
#[path = "customize/customize_page.rs"]
mod customize_page;
#[path = "workspace/device_pages.rs"]
mod device_pages;
#[path = "help/help_page.rs"]
mod help_page;
#[path = "keyboard/keyboard_controls.rs"]
mod keyboard_controls;
#[path = "lighting/lighting_color.rs"]
mod lighting_color;
#[path = "lighting/lighting_input.rs"]
mod lighting_input;
#[path = "mapping/macro_inputs.rs"]
pub mod macro_inputs;
#[path = "macros/macro_library.rs"]
pub mod macro_library;
#[path = "module_service.rs"]
pub mod module_service;
#[path = "mouse/dpi/sensitivity.rs"]
mod sensitivity;
pub use razer_model::settings;
// Keep the pure native encoder and its strict input validation available for
// the eventual transport. The source UI has no standalone diagnostic button;
// the removed local "apply" control was disabled and never sent a request.
#[allow(dead_code)]
#[path = "shortcuts/shortcut_engine.rs"]
pub mod shortcut_engine;
#[path = "shortcuts/shortcuts.rs"]
pub mod shortcuts;
#[path = "workspace/workspace.rs"]
mod workspace;
pub use workspace::{DeviceWorkspace, WorkspaceEvent};

#[path = "workspace/product_surface.rs"]
mod product_surface;
#[path = "workspace/product_workspace.rs"]
mod product_workspace;
#[path = "workspace/source_controls.rs"]
mod source_controls;
#[path = "help/source_help.rs"]
mod source_help;
#[path = "help/source_help_owner.rs"]
mod source_help_owner;
pub use source_help::{HelpCamoCommand, HelpCamoObservation, HelpCamoRequested};
pub use source_help::{HelpResetOutcome, HelpResetRequest};
pub use source_help::{HelpRuntimeVersionsObservation, HelpRuntimeVersionsRequest};
pub use source_help::{HelpSystemInfoCommand, HelpSystemInfoRequested};
pub use source_help_owner::{HelpOwnerCommand, HelpOwnerOverlay, HelpOwnerRequested};
#[path = "workspace/source_workspace.rs"]
mod source_workspace;
pub use product_workspace::ProductWorkspace;
pub use source_controls::{
    HyperfluxOperation, HyperfluxRequested, ReceiverBrightnessReadRequested,
    ReceiverBrightnessRequested, ReceiverCategory, ReceiverDeviceRequested,
    ReceiverDevicesObservation, ReceiverOperation, ReceiverPairingEvent, ReceiverPairingIntent,
    ReceiverPairingObservation, ReceiverPeer, ReceiverProgress, RipsawMonitoringRequested,
};
#[path = "gamepad/mod.rs"]
pub mod gamepad_products;
#[path = "keyboard/mod.rs"]
pub mod keyboard_products;
#[path = "mouse/dpi/mouse_dpi_matcher.rs"]
pub mod mouse_dpi_matcher;
#[path = "mouse/polling/mouse_polling.rs"]
pub mod mouse_polling;
#[path = "mouse/mod.rs"]
pub mod mouse_products;

/// Entry availability only; adapter completeness is audited separately.
pub fn has_product_workspace(pid: u32) -> bool {
    !crate::nav::Tab::for_product(pid).is_empty() || razer_catalog::registered(pid).is_some()
}

pub fn has_local_product_page(pid: u32, key: &str) -> bool {
    if !razer_catalog::registered(pid)
        .and_then(|product| product.primary_navigation())
        .is_some_and(|navigation| {
            navigation.pages().iter().any(|page| {
                page.role() != razer_catalog::ProductPageRole::Help && page.kind().key() == key
            })
        })
    {
        return false;
    }
    let existing = crate::nav::Tab::for_product(pid);
    if !existing.is_empty() {
        return crate::nav::Tab::from_arg(key).is_some_and(|tab| existing.contains(&tab));
    }
    if wireless_argb::supports_page(pid, key)
        || wired_argb::supports_page(pid, key)
        || aether_strip::supports_page(pid, key)
        || (pid == 784 && key == "TAB_LIGHTING")
        || automation::supports_page(pid, key)
        || dock_pairing::supports_page(pid, key)
    {
        return true;
    }
    if pid == 769 {
        return key == "HOME";
    }
    if mouse_products::source_product(pid).is_some() {
        return matches!(
            key,
            "TAB_CUSTOMIZE"
                | "TAB_PERFORMANCE"
                | "TAB_POWER"
                | "TAB_LIGHTING"
                | "TAB_CALIBRATION"
                | "TAB_SCROLLING"
                | "ADVANCED"
        );
    }
    if keyboard_products::source_product(pid).is_some() {
        return matches!(
            key,
            "TAB_CUSTOMIZE" | "TAB_LIGHTING" | "ACTUATION" | "TAB_CALIBRATION" | "TAB_POWER"
        ) || (key == "OLED" && source_controls::supports_page(pid, key));
    }
    if gamepad_products::source_product(pid).is_some() {
        return matches!(
            key,
            "TAB_CUSTOMIZE"
                | "TRIGGERS"
                | "THUMBSTICKS"
                | "TAB_LIGHTING"
                | "TAB_POWER"
                | "TAB_CALIBRATION"
        );
    }
    if audio_products::source_product(pid).is_some() {
        return audio_products::supports_page(pid, key) || source_controls::supports_page(pid, key);
    }
    if system_products::source_product(pid).is_some() {
        return matches!(
            key,
            "TAB_PERFORMANCE"
                | "TAB_BATTERY"
                | "TAB_DISPLAY"
                | "TAB_LIGHTING"
                | "TAB_SOUND"
                | "TAB_CUSTOMIZE"
        );
    }
    if accessory_system_products::source_product(pid).is_some() {
        return matches!(
            (pid, key),
            (3858 | 3880, "TAB_GAMING" | "TAB_COLOR" | "TAB_DISPLAY")
                | (3900 | 3893 | 3907, "TAB_PERFORMANCE")
                | (3921, "TAB_CUSTOMIZE")
        ) || (!accessory_system_products::supports_page(pid, key)
            && source_controls::supports_page(pid, key));
    }
    source_controls::supports_page(pid, key)
}

#[path = "accessories/armory_product.rs"]
mod armory_product;
#[path = "audio/mixer/audio_mixer.rs"]
pub mod audio_mixer;
#[path = "audio/mod.rs"]
mod audio_products;
pub use audio_products::{
    AudioMixerCompletion, AudioMixerOperation, AudioMixerReply, AudioMixerRequest,
    AudioProductWorkspace, PresetShortcutBinding, PresetShortcutRequest,
    SeirenAudioCaptureRequested, SeirenAudioPlaybackRequested, SeirenEqRequested,
    SeirenMicrophoneSetupSeen, SeirenStreamMixerCaptureChanged, SeirenStreamMixerObservation,
    SeirenStreamMixerRequested,
};
pub use audio_products::{
    AudioVolumeCompletion, AudioVolumeOperation, AudioVolumeReply, AudioVolumeRequest,
    LeviathanObservation, LeviathanOperation, LeviathanRequest,
};
pub use audio_products::{OledRuntimeObservation, OledRuntimeRequested, StreamMixerObservation};
pub use keyboard_products::SnapTapObservation;
pub use mouse_products::ScrollWheelObservation;

#[path = "system/mod.rs"]
mod system_products;

#[path = "accessories/accessory_system_products.rs"]
mod accessory_system_products;
#[path = "accessories/aether_strip.rs"]
pub mod aether_strip;
#[path = "automation.rs"]
pub mod automation;
#[path = "chroma/chroma_product.rs"]
pub mod chroma_product;
#[path = "chroma/chroma_studio.rs"]
pub mod chroma_studio;
#[path = "workspace/display_mode_roots.rs"]
pub mod display_mode_roots;
#[path = "accessories/dock_pairing.rs"]
pub mod dock_pairing;
pub use dock_pairing::{DockPairingEvent, DockPairingObservation, DockPairingUtilityRequested};
#[path = "accessories/hue.rs"]
pub mod hue;
#[path = "accessories/wired_argb.rs"]
pub mod wired_argb;
#[path = "accessories/wireless_argb.rs"]
pub mod wireless_argb;
