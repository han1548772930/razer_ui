//! Device/profile contracts and explicit preview fixtures; no UI or transport I/O.
mod controller_input;
pub mod domain;
pub mod host_window_focus;
pub mod model;
pub mod product_preview;
pub mod settings;
pub mod source_identity;
pub mod source_names;
mod runtime_setup;
#[cfg(test)]
#[path = "../testdata/devices.rs"]
mod test_devices;
