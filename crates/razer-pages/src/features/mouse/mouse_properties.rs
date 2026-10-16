//! Source-verified mouse system-properties capability and presentation.
use super::*;
use gpui_kit::{base::Button as BaseButton, component::WindowExt as _};
use razer_platform::system;
use razer_widgets::theme::MouseCustomizeColors;

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
    windows_11_supported: bool,
    column_side: String,
    icon_width: f32,
    icon_height: f32,
    icon_margin: f32,
    font_size: f32,
    line_height: f32,
    active_opacity: f32,
}

impl PropertiesSpec {
    pub(super) fn icon(&'static self, windows_11: bool) -> &'static str {
        if windows_11 && self.windows_11_supported {
            &self.windows_11_icon
        } else {
            &self.legacy_icon
        }
    }

    pub(super) fn in_left_column(&self) -> bool {
        self.column_side == "left"
    }
}

pub(super) fn source_spec(product_id: u32) -> Option<&'static PropertiesSpec> {
    static SPECS: OnceLock<Vec<PropertiesSpec>> = OnceLock::new();
    SPECS
        .get_or_init(|| crate::data::mouse_properties_data!())
        .iter()
        .find(|spec| spec.product_id == product_id)
}

impl MouseProductWorkspace {
    pub(super) fn mouse_properties(&self, cx: &Context<Self>) -> AnyElement {
        let Some(spec) = source_spec(self.spec.product_id) else {
            return div().into_any_element();
        };
        surface::panel_with_help(
            t(&spec.title),
            surface::help_control("mouse-properties-help", t(&spec.tooltip)),
            cx,
        )
        .when(self.spec.block_widget_columns(), |panel| panel.mb_0())
        .child(
            h_flex()
                .items_center()
                .child(
                    img(self.properties_icon.expect("source properties icon"))
                        .w(surface::css(spec.icon_width))
                        .h(surface::css(spec.icon_height))
                        .mr(surface::css(spec.icon_margin))
                        .flex_shrink_0()
                        .object_fit(ObjectFit::Contain),
                )
                .child(
                    BaseButton::new("mouse-properties")
                        .accessibility_label(t(&spec.action))
                        // `.img-text.flex > div { flex:auto }`: the source
                        // action occupies the remainder of the row. It has a
                        // line-height, not a fixed height, so translations can wrap.
                        .flex_grow(1.)
                        .flex_shrink(1.)
                        .justify_start()
                        .min_h(surface::css(spec.line_height))
                        .p_0()
                        .text_left()
                        .whitespace_normal()
                        .text_size(surface::css(spec.font_size))
                        .line_height(surface::css(spec.line_height))
                        .text_color(MouseCustomizeColors::text())
                        .underline()
                        .cursor_pointer()
                        .hover(|style| style.text_color(MouseCustomizeColors::selected()))
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
