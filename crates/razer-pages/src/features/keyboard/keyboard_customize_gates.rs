//! Factory-default Customize is its own mounted component, not an editor.
use super::*;

impl KeyboardProductWorkspace {
    pub(super) fn customize_factory_root(&self) -> bool {
        self.factory_default_profile
            && matches!(
                self.spec.product_id,
                642 | 678 | 679 | 688 | 719 | 720 | 721 | 740 | 741 | 742 | 746 | 756
            )
    }

    pub(super) fn factory_customize_widget(&self, cx: &Context<Self>) -> AnyElement {
        // PID742 8995.54ec5817.chunk.js module15916 mounts this container
        // after config-block/dim-corner, inside the same config-wrapper.
        surface::panel_with_help(
            t("FACTORY_DEFAULT_PROFILE_TITTLE"),
            surface::help_control(
                "keyboard-customize-factory-help",
                t("FACTORY_DEFAULT_PROFILE_TOOLTIP"),
            ),
            cx,
        )
        .child(FactoryDescription)
        .into_any_element()
    }

    pub(super) fn customize_controls_disabled(&self) -> bool {
        self.customize_factory_root()
            || self.spec.product_id == 756 && self.observed_bluetooth() == Some(true)
    }

    pub(super) fn customize(&self, window: &Window, cx: &Context<Self>) -> AnyElement {
        let contents = if self.customize_factory_root() {
            self.keyboard_image(false, cx)
        } else {
            self.customize_contents(window, cx)
        };
        div()
            .relative()
            .child(contents)
            .when(
                self.spec.product_id == 756
                    && self.calibration_v2.normal_mode
                    && self.observed_bluetooth() == Some(true),
                |page| page.child(lighting_gates::BleWarning),
            )
            .into_any_element()
    }
}

#[derive(IntoElement)]
struct FactoryDescription;
impl RenderOnce for FactoryDescription {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        let icon = f32::from(window.rem_size() / 16. * 14.);
        let html = t("FACTORY_DEFAULT_PROFILE_WARNING_DESC").replace(
            "{{dots}}",
            &format!("<img src=\"factory-profile-dots\" width=\"{icon}\" height=\"{icon}\" />"),
        );
        gpui_kit::base::TextView::html("keyboard-customize-factory-description", html)
            .image_source(|_| "synapse/profile-more.svg".into())
            .selectable(false)
            .scrollable(false)
    }
}
