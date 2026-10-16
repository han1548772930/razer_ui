//! Device workspaces, product pages and editor controllers.
#[path = "features/mod.rs"]
pub mod features;
#[path = "keyboard_geometry.rs"]
pub mod keyboard_geometry;
#[path = "nav.rs"]
pub mod nav;
#[cfg(test)]
#[path = "../testdata/devices.rs"]
mod test_devices;
