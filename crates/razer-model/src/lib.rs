//! Device/profile contracts and explicit preview fixtures; no UI or transport I/O.
pub mod chroma_studio;
mod controller_input;
pub mod domain;
pub mod host_window_focus;
pub mod mapping_requirements;
pub mod model;
pub mod product_preview;
mod runtime_setup;
pub mod settings;
pub mod source_identity;
pub mod source_names;
#[cfg(test)]
#[path = "../testdata/devices.rs"]
mod test_devices;

pub mod macro_library;
pub mod mapping;
pub mod module_service;
pub mod preferences;
pub mod shortcuts;
