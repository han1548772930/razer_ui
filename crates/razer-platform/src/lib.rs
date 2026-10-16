//! Host operations, topology invalidation and read-only installed-file discovery.
#[path = "camera/camera_apps.rs"]
pub mod camera_apps;
#[path = "camera/camera_preview.rs"]
pub mod camera_preview;
#[path = "observation/device_changes.rs"]
pub mod device_changes;
#[path = "file_dialog.rs"]
pub mod file_dialog;
#[path = "observation/foreground_monitor.rs"]
pub mod foreground_monitor;
#[path = "observation/game_monitor.rs"]
pub mod game_monitor;
#[path = "input/global_shortcuts.rs"]
pub mod global_shortcuts;
#[path = "input/keyboard_capture.rs"]
pub mod keyboard_capture;
#[path = "input/keyboard_layout.rs"]
pub mod keyboard_layout;
#[path = "input/keyboard_layout_monitor.rs"]
pub mod keyboard_layout_monitor;
#[path = "input/macro_geometry.rs"]
pub mod macro_geometry;
#[path = "native_paths.rs"]
pub mod native_paths;
pub mod media;
#[path = "observation/session_changes.rs"]
pub mod session_changes;
#[path = "system.rs"]
pub mod system;
pub mod startup;
#[path = "input/wheel_scroll.rs"]
pub mod wheel_scroll;
#[path = "windows_service_status.rs"]
pub mod windows_service_status;

#[path = "platform/mod.rs"]
mod platform;
