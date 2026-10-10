//! PID 70 qI/eO mount RI.A with numeric tips and endpoint tags;
//! neither lighting slider mounts a text input.
use super::*;
use razer_widgets::source_slider::SourceSlider;

impl MouseProductWorkspace {
    pub(super) fn lighting_slider_70(
        &self,
        path: &str,
        min_tag: &str,
        max_tag: &str,
        enabled: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let slider = &self.sliders[path];
        let minimum = slider.read(cx).min_value();
        let maximum = slider.read(cx).max_value();
        let value = self.number(path);
        div()
            .relative()
            .w_full()
            .h(surface::css(64.))
            .child(
                SourceSlider::new(slider, (value - minimum) / (maximum - minimum))
                    .enabled(enabled)
                    .tip(Some(format!("{value:.0}"))),
            )
            .child(
                div()
                    .absolute()
                    .bottom_0()
                    .w_full()
                    .opacity(if enabled { 1. } else { 0.3 })
                    .child(surface::slider_tags(min_tag, None, max_tag, None)),
            )
            .into_any_element()
    }
}
