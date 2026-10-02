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
