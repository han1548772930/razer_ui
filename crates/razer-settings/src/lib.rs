//! Settings and live connection views. Host navigation is emitted as events.
mod data;
#[path = "products.rs"]
pub mod products;
#[path = "runtime_page.rs"]
pub mod runtime_page;
#[path = "preferences/settings_page.rs"]
pub mod settings_page;
#[path = "preferences/settings_systray_action.rs"]
pub mod settings_systray_action;
#[path = "window/settings_window.rs"]
pub mod settings_window;
