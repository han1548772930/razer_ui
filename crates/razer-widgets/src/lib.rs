//! Shared semantic presentation primitives.
pub mod keyboard_navigation;
mod window_resources;

/// Register presentation facilities after GPUI Kit initialization and before
/// opening windows. Every Base Root then owns the same resource-loading policy.
pub fn init(cx: &mut gpui_kit::App) {
    window_resources::init(cx);
    keyboard_navigation::init(cx);
}

#[path = "app_introduction_banner.rs"]
pub mod app_introduction_banner;
#[path = "attribute_tip.rs"]
pub mod attribute_tip;
#[path = "battery/battery.rs"]
pub mod battery;
#[path = "game_tile.rs"]
pub mod game_tile;
#[path = "hover_tip.rs"]
pub mod hover_tip;
#[path = "keyboard_capture.rs"]
pub mod keyboard_capture;
#[path = "module_button.rs"]
pub mod module_button;
#[path = "scroll/scroll.rs"]
pub mod scroll;
#[path = "settings_button.rs"]
pub mod settings_button;
#[path = "source_alert/source_alert.rs"]
pub mod source_alert;
pub mod source_input;
#[path = "source_link.rs"]
pub mod source_link;
#[path = "source_slider.rs"]
pub mod source_slider;
pub mod source_text;
#[path = "source_tooltip/source_tooltip.rs"]
pub mod source_tooltip;
#[path = "stepper.rs"]
pub mod stepper;
#[path = "surface/surface.rs"]
pub mod surface;
#[path = "synapse_select.rs"]
pub mod synapse_select;
#[path = "theme.rs"]
pub mod theme;
#[path = "tutorial/tutorial.rs"]
pub mod tutorial;
#[path = "tutorial/tutorial_media.rs"]
pub mod tutorial_media;
