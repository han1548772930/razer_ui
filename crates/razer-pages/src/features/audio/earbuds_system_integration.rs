//! Source Sound Properties card and its page-owned Windows icon observation.
use super::*;
use razer_widgets::{keyboard_navigation::Button, theme::AudioEnhancementColors as Colors};

pub(super) struct State {
    windows_11: bool,
    observed: bool,
    generation: u64,
    task: Option<Task<()>>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            windows_11: true,
            observed: false,
            generation: 0,
            task: None,
        }
    }
}

impl State {
    pub(super) fn cancel(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.task = None;
    }

    fn observe(&mut self, generation: u64, version: Option<&str>) -> bool {
        if self.generation != generation {
            return false;
        }
        self.windows_11 = version == Some("11");
        self.observed = true;
        true
    }
}

impl AudioProductWorkspace {
    pub(super) fn sync_earbuds_system_integration(&mut self, cx: &mut Context<Self>) {
        if !razer_device::earbuds_protocol40::supports_auto_pause(self.spec.product_id)
            || self.page != "TAB_SOUND"
        {
            self.earbuds_system.cancel();
            return;
        }
        if self.earbuds_system.observed || self.earbuds_system.task.is_some() {
            return;
        }
        let generation = self.earbuds_system.generation;
        self.earbuds_system.task = Some(cx.spawn(async move |owner, cx| {
            let version = cx
                .background_spawn(async { razer_platform::system::windows_major_version() })
                .await;
            let _ = owner.update(cx, |owner, cx| {
                if owner.page == "TAB_SOUND" && owner.earbuds_system.observe(generation, version) {
                    owner.earbuds_system.task = None;
                    cx.notify();
                }
            });
        }));
    }

    pub(super) fn earbuds_sound_properties(&self, cx: &mut Context<Self>) -> AnyElement {
        surface::panel_with_control(
            t("SOUND_PROPERTIES"),
            surface::help_control("earbuds-properties-help", t("SOUND_PROPERTIES_TOOLTIP")),
            cx,
        )
        .id("earbuds-sound-properties")
        .test_support()
        .child(
            h_flex()
                .items_center()
                .child(
                    img(if self.earbuds_system.windows_11 {
                        "synapse/windows-11.svg"
                    } else {
                        "synapse/earbuds-properties-windows.svg"
                    })
                    .size(surface::css(44.))
                    .mr(surface::css(20.)),
                )
                .child(
                    Button::new("earbuds-open-sound-properties")
                        .group("earbuds-properties-link")
                        .accessibility_label(t("OPEN_WINDOW_PROPERTIES_SOUND"))
                        .h(surface::css(44.))
                        .line_height(surface::css(44.))
                        .flex()
                        .items_center()
                        .self_start()
                        .text_color(Colors::text())
                        .text_size(surface::css(14.))
                        .underline()
                        .hover(|style| style.text_color(Colors::selected()))
                        .focus_visible(|style| style.text_color(Colors::selected()))
                        .active(|style| style.opacity(0.7))
                        .child(t("OPEN_WINDOW_PROPERTIES_SOUND"))
                        .child(
                            div()
                                .relative()
                                .size(surface::css(20.))
                                .ml(surface::css(4.))
                                .child(img("synapse/earbuds-properties-external.svg").size_full())
                                .child(
                                    img("synapse/earbuds-volume-external-active.svg")
                                        .absolute()
                                        .inset_0()
                                        .size_full()
                                        .opacity(0.)
                                        .group_hover("earbuds-properties-link", |style| {
                                            style.opacity(1.)
                                        }),
                                ),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(owner) = &this.source_owner {
                                owner.update(cx, |owner, cx| owner.request_sound_windows(true, cx));
                            }
                        })),
                ),
        )
        .into_any_element()
    }
}

#[cfg(test)]
#[path = "earbuds_system_integration_tests.rs"]
mod tests;
