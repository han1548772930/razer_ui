//! Audited device workspace. Domain edits and retained controls are owned here.
//! The previous free-function pages are no longer compiled into the app.
mod audio_page;
mod controls;
pub use controls::Choice;
mod customize_drawer;
mod customize_page;
mod device_pages;
mod help_page;
mod keyboard_controls;
mod lighting_color;
mod lighting_input;
pub mod macro_inputs;
pub mod macro_library;
pub mod module_service;
mod sensitivity;
pub use razer_model::settings;
// Keep the pure native encoder and its strict input validation available for
// the eventual transport. The source UI has no standalone diagnostic button;
// the removed local "apply" control was disabled and never sent a request.
#[allow(dead_code)]
pub mod shortcut_engine;
pub mod shortcuts;
mod workspace;
pub use workspace::{DeviceWorkspace, WorkspaceEvent};

mod product_surface;
mod product_workspace;
mod source_controls;
mod source_help;
mod source_workspace;
pub use product_workspace::ProductWorkspace;
pub use source_controls::{
    ReceiverCategory, ReceiverDeviceRequested, ReceiverDevicesObservation, ReceiverOperation,
    ReceiverPairingEvent, ReceiverPairingIntent, ReceiverPairingObservation, ReceiverPeer,
    ReceiverProgress,
};
pub mod gamepad_products;
pub mod keyboard_products;
pub mod mouse_polling;
pub mod mouse_products;

/// Entry availability only; adapter completeness is audited separately.
pub fn has_product_workspace(pid: u32) -> bool {
    !crate::nav::Tab::for_product(pid).is_empty() || razer_catalog::registered(pid).is_some()
}

mod armory_product;
mod audio_products;
pub use audio_products::{
    AudioVolumeCompletion, AudioVolumeOperation, AudioVolumeReply, AudioVolumeRequest,
};
pub use audio_products::{OledRuntimeObservation, OledRuntimeRequested, StreamMixerObservation};
pub use keyboard_products::SnapTapObservation;
pub use mouse_products::ScrollWheelObservation;

mod system_products;

mod accessory_system_products;
pub mod aether_strip;
pub mod automation;
pub mod chroma_product;
pub mod chroma_studio;
pub mod display_mode_roots;
pub mod dock_pairing;
pub use dock_pairing::{DockPairingEvent, DockPairingObservation};
pub mod hue;
pub mod wired_argb;
pub mod wireless_argb;
