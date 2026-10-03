//! Audited device workspace. Domain edits and retained controls are owned here.
//! The previous free-function pages are no longer compiled into the app.
mod audio_page;
mod controls;
pub(crate) use controls::Choice;
mod customize_drawer;
mod customize_page;
mod device_pages;
mod help_page;
mod keyboard_controls;
mod lighting_color;
mod lighting_input;
mod sensitivity;
pub mod settings;
pub(crate) mod shortcut_engine;
pub(crate) mod shortcuts;
mod workspace;
pub use workspace::{DeviceWorkspace, WorkspaceEvent};

mod product_workspace;
mod source_controls;
mod source_help;
mod source_workspace;
pub(crate) use product_workspace::ProductWorkspace;
pub(crate) mod gamepad_products;
pub(crate) mod keyboard_products;
pub(crate) mod mouse_products;

/// Entry availability only; adapter completeness is audited separately.
pub(crate) fn has_product_workspace(pid: u32) -> bool {
    !crate::nav::Tab::for_product(pid).is_empty() || crate::product::registered(pid).is_some()
}

mod audio_products;

mod system_products;

mod accessory_system_products;
pub(crate) mod dock_pairing;
pub(crate) mod hue;
