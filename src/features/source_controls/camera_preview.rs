//! Mounted preview source and Camo branches from the three current camera roots.
//! Device enumeration and media playback remain at the transport boundary.
use super::*;
use crate::ui::theme::CameraProductColors as Colors;

#[derive(Deserialize)]
pub(super) struct CameraPreviewSpec {
    activation: bool,
    promo_intro: String,
    promo_items: Vec<String>,
    promo_link: String,
    source_title: String,
    preview_off: String,
    enable_preview: String,
}

pub(super) struct CameraPreviewState {
    // The current reducer initializes this to null, not to true.
    enabled: Option<bool>,
    source: Entity<SelectState<Vec<Choice>>>,
}

impl CameraPreviewState {
    pub(super) fn new(window: &mut Window, cx: &mut Context<SourceControls>) -> Self {
        Self {
            enabled: None,
            // `cameraDeviceList: []`. Never synthesize a connected camera.
            source: cx.new(|cx| SelectState::new(Vec::new(), None, window, cx)),
        }
    }
}

impl SourceControls {
    fn preview_spec(&self) -> Option<&'static CameraPreviewSpec> {
        self.spec
            .pages
            .iter()
            .find_map(|page| page.camera_preview.as_ref())
    }

    fn set_camera_preview_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if let Some(state) = &mut self.camera_preview {
            state.enabled = Some(enabled);
            cx.notify();
        }
    }

    pub(super) fn render_camera_preview_actions(&self, cx: &mut Context<Self>) -> AnyElement {
        let enabled = self
            .camera_preview
            .as_ref()
            .is_some_and(|state| state.enabled == Some(true));
        h_flex()
            .ml_auto()
            .items_center()
            .child(
                gpui_kit::base::Button::new("camera-preview-refresh")
                    .accessibility_label(crate::i18n::t("REFRESH"))
                    .p_0()
                    .size(surface::css(20.))
                    .min_w(surface::css(20.))
                    .mr(surface::css(10.))
                    .cursor_pointer()
                    .hover(|button| button.opacity(0.7))
                    .focus_visible(|button| button.border_1().border_color(Colors::focus()))
                    .child(
                        Icon::default()
                            .path("synapse/camera-section-reset.svg")
                            .size(surface::css(20.))
                            .text_color(Colors::text())
                            .transform(Transformation::scale(size(-1., 1.))),
                    )
                    .on_click(cx.listener(|_, _, _, cx| {
                        // Source requests enumeration/reconnect. No source state changes the
                        // refresh icon, and no transport result is manufactured here.
                        cx.emit(SourceControlsPreviewRefreshRequested);
                    })),
            )
            .child(
                gpui_kit::base::Button::new("camera-preview-toggle")
                    .accessibility_label(
                        self.preview_spec()
                            .map_or(String::new(), |spec| crate::i18n::t(&spec.source_title)),
                    )
                    .p_0()
                    .size(surface::css(24.))
                    .min_w(surface::css(24.))
                    .cursor_pointer()
                    .hover(|button| button.opacity(0.7))
                    .focus_visible(|button| button.border_1().border_color(Colors::focus()))
                    .child(
                        img(if enabled {
                            "synapse/camera-preview-open.svg"
                        } else {
                            "synapse/camera-preview-close.svg"
                        })
                        .size_full()
                        .object_fit(ObjectFit::Contain),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_camera_preview_enabled(!enabled, cx)
                    })),
            )
            .into_any_element()
    }

    pub(super) fn render_camera_source_select(&self) -> AnyElement {
        let Some(state) = &self.camera_preview else {
            return div().into_any_element();
        };
        surface::select(&state.source)
            .id("camera-preview-source")
            .items(Vec::new())
            .placeholder("")
            .accessibility_label(
                self.preview_spec()
                    .map_or(String::new(), |spec| crate::i18n::t(&spec.source_title)),
            )
            .w_full()
            .into_any_element()
    }

    pub(super) fn render_camera_promo(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(spec) = self.preview_spec() else {
            return div().into_any_element();
        };
        let link_text = crate::i18n::t(&spec.promo_link);
        let link_content = || {
            h_flex()
                .items_center()
                .mr(surface::css(8.))
                .child(div().underline().child(link_text.clone()))
                .child(
                    Icon::default()
                        .path(if spec.activation {
                            "synapse/camera-preview-arrow.svg"
                        } else {
                            "synapse/camera-preview-external.svg"
                        })
                        .w(surface::css(if spec.activation { 6. } else { 24. }))
                        .h(surface::css(if spec.activation { 10. } else { 24. }))
                        .when(spec.activation, |icon| icon.ml(surface::css(5.)))
                        .when(!spec.activation, |icon| icon.mt(surface::css(-5.))),
                )
        };
        let link = if spec.activation {
            gpui_kit::base::Button::new("camera-camo-help")
                .accessibility_label(link_text.clone())
                .p_0()
                .text_size(surface::css(14.))
                .line_height(surface::css(30.))
                .text_color(Colors::text())
                .cursor_pointer()
                .hover(|button| button.text_color(Colors::focus()))
                .active(|button| button.opacity(0.7))
                .focus_visible(|button| button.border_1().border_color(Colors::focus()))
                .child(link_content())
                .on_click(cx.listener(|_, _, _, cx| cx.emit(SourceControlsHelpRequested)))
                .into_any_element()
        } else {
            gpui_kit::base::Link::new("camera-camo-website")
                .href("https://www.razer.com/software/camo")
                .accessibility_label(link_text.clone())
                .open_with(|url, _, _, cx| cx.open_url(url))
                .text_size(surface::css(14.))
                .line_height(surface::css(30.))
                .text_color(Colors::text())
                .cursor_pointer()
                .hover(|link| link.text_color(Colors::focus()))
                .active(|link| link.opacity(0.7))
                .child(link_content())
                .into_any_element()
        };
        v_flex()
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .child(
                div()
                    .mr(surface::css(5.))
                    .child(crate::i18n::t(&spec.promo_intro)),
            )
            // The source <ul> overrides only margin-top:5px. The other list
            // insets remain Chromium's default 40px padding / 1em bottom margin.
            .child(
                v_flex()
                    .mt(surface::css(5.))
                    .mb(surface::css(14.))
                    .pl(surface::css(40.))
                    .children(spec.promo_items.iter().map(|key| {
                        div()
                            .relative()
                            .child(div().absolute().right_full().child("• "))
                            .child(crate::i18n::t(key))
                    })),
            )
            .child(div().mt(surface::css(-10.)).child(link))
            .into_any_element()
    }

    pub(super) fn render_camera_preview_off(&self, cx: &mut Context<Self>) -> AnyElement {
        let Some(spec) = self.preview_spec() else {
            return div().into_any_element();
        };
        if self
            .camera_preview
            .as_ref()
            .is_some_and(|state| state.enabled == Some(true))
        {
            return div().into_any_element();
        }
        let label = crate::i18n::t(&spec.enable_preview).to_lowercase();
        // Source `.camera-reconnect` uses text-transform:capitalize after lowercasing.
        let label = label
            .split(' ')
            .map(|word| {
                let mut chars = word.chars();
                chars.next().map_or(String::new(), |first| {
                    first.to_uppercase().collect::<String>() + chars.as_str()
                })
            })
            .collect::<Vec<_>>()
            .join(" ");
        v_flex()
            .absolute()
            .inset_0()
            .items_center()
            .justify_center()
            .text_center()
            .text_size(surface::css(14.))
            .line_height(surface::css(16.))
            .child(
                img("synapse/camera-preview-unable.svg")
                    .size(surface::css(18.))
                    .mb(surface::css(6.)),
            )
            .child(crate::i18n::t(&spec.preview_off))
            .child(
                gpui_kit::base::Button::new("camera-preview-enable")
                    .p_0()
                    .mt(surface::css(16.))
                    .cursor_pointer()
                    .underline()
                    .accessibility_label(label.clone())
                    .hover(|button| button.text_color(Colors::focus()))
                    .focus_visible(|button| button.border_1().border_color(Colors::focus()))
                    .child(label)
                    .on_click(
                        cx.listener(|this, _, _, cx| this.set_camera_preview_enabled(true, cx)),
                    ),
            )
            .into_any_element()
    }
}
