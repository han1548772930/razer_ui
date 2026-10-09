//! Source-verified mouse system-properties capability and presentation.
use super::*;
use gpui_kit::{base::Button as BaseButton, component::WindowExt as _};
use razer_platform::system;

#[cfg(test)]
#[path = "mouse_properties_tests.rs"]
mod tests;

#[derive(Deserialize)]
pub(super) struct PropertiesSpec {
    product_id: u32,
    title: String,
    tooltip: String,
    action: String,
    legacy_icon: String,
    windows_11_icon: String,
    icon_width: f32,
    icon_height: f32,
    icon_margin: f32,
    font_size: f32,
    line_height: f32,
    foreground: u32,
    hover_foreground: u32,
    active_opacity: f32,
}

impl PropertiesSpec {
    pub(super) fn icon(&'static self, windows_11: bool) -> &'static str {
        if windows_11 {
            &self.windows_11_icon
        } else {
            &self.legacy_icon
        }
    }
}

pub(super) fn source_spec(product_id: u32) -> Option<&'static PropertiesSpec> {
    static SPECS: OnceLock<Vec<PropertiesSpec>> = OnceLock::new();
    SPECS
        .get_or_init(|| {
            serde_json::from_str(include_str!("mouse_properties_data.json"))
                .expect("validated current mouse properties capabilities")
        })
        .iter()
        .find(|spec| spec.product_id == product_id)
}

impl MouseProductWorkspace {
    pub(super) fn mouse_properties(&self, cx: &Context<Self>) -> AnyElement {
        let Some(spec) = source_spec(self.spec.product_id) else {
            return div().into_any_element();
        };
        surface::panel_with_control(
            t(&spec.title),
            surface::help_control("mouse-properties-help", t(&spec.tooltip)),
            cx,
        )
        .child(
            h_flex()
                .items_center()
                .child(
                    img(self.properties_icon.expect("source properties icon"))
                        .w(surface::css(spec.icon_width))
                        .h(surface::css(spec.icon_height))
                        .mr(surface::css(spec.icon_margin))
                        .flex_shrink_0(),
                )
                .child(
                    BaseButton::new("mouse-properties")
                        .accessibility_label(t(&spec.action))
                        .h(surface::css(spec.line_height))
                        .p_0()
                        .text_size(surface::css(spec.font_size))
                        .line_height(surface::css(spec.line_height))
                        .text_color(rgb(spec.foreground))
                        .underline()
                        .cursor_pointer()
                        .hover(move |style| style.text_color(rgb(spec.hover_foreground)))
                        .active(move |style| style.opacity(spec.active_opacity))
                        .child(t(&spec.action))
                        .on_click(|_, window, cx| {
                            // Existing local OS command. Never call/acknowledge the
                            // original vendor DLL operation or alter the local profile.
                            if let Err(error) = system::open(system::Properties::Mouse) {
                                window.push_notification(format!("无法打开系统属性：{error}"), cx);
                            }
                        }),
                ),
        )
        .into_any_element()
    }
}
