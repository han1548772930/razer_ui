//! Current keyboard BodyWidgets / WidgetCol geometry.
//! Complete original wrappers, mounted callers and CSS: keyboard-main-audit.json.
use super::*;

/// Pure geometry shared by the mounted image and its physical key regions.
/// Values are the current source SVG/CSS coordinates, before native zoom.
pub(super) struct KeyboardImageFrame {
    pub size: [f32; 2],
    pub source_scale: f32,
    pub source_config_frame: bool,
    pub customize_artwork: bool,
    pub image: Option<&'static str>,
    pub image_origin: [f32; 2],
    pub image_size: [f32; 2],
    pub keyboard_origin: [f32; 2],
}

impl KeyboardImageFrame {
    pub fn new(
        spec: &'static KeyboardProductSpec,
        observation: Option<SourceArtworkObservation>,
        customize: bool,
        factory: bool,
    ) -> Self {
        let keypad = matches!(spec.product_id, 555 | 580 | 747);
        let geometry = observation
            .and_then(|observation| source_artwork::geometry_for_surface(observation, customize));
        let [width, height] = geometry
            .map(|geometry| geometry.viewbox)
            .unwrap_or(spec.viewbox);
        // PID708 retains a 1400x760 viewBox; 8248.ccc25bc7.chunk.css
        // projects the image and every key region to 700x380 CSS pixels.
        let source_scale = if spec.product_id == 708 { 0.5 } else { 1. };
        let size = [width * source_scale, height * source_scale];
        let source_config_frame = customize && !keypad;
        let factory = source_config_frame && factory;
        let customize_artwork = source_config_frame && !factory;
        let artwork = observation.and_then(|observation| {
            if customize_artwork {
                source_artwork::customize_image_for_scale(observation, 1.)
            } else {
                source_artwork::base_image(observation)
            }
        });
        let [image_width, image_height] = artwork
            .map(|image| image.css_size)
            .unwrap_or(spec.image_size);
        let [mut image_width, image_height] =
            [image_width * source_scale, image_height * source_scale];
        let mut image = artwork.map(|image| image.asset).or_else(|| {
            (!source_artwork::has_source(spec.product_id) || observation.is_none())
                .then_some(spec.image.as_deref())
                .flatten()
        });
        if customize_artwork && spec.product_id == 691 && observation.is_none() {
            // The current producer excludes _LED/_Selection root groups
            // before injecting artwork alongside the native key regions.
            image = Some("synapse/keyboard-customize-691.svg");
        }
        let svg_image = image.is_some_and(|asset| asset.ends_with(".svg"));
        // Factory Customize mounts .box-img; normal Customize injects the
        // productSVG nodes into its parent SVG and must retain their origin.
        let image_top = if factory {
            match spec.product_id {
                678 | 679 | 688 | 708 | 719 | 742 => 33.,
                720 => 10.,
                721 => 55.,
                740 => 60.,
                741 => 35.,
                756 => 50.,
                _ => 0.,
            }
        } else {
            0.
        };
        if factory && spec.product_id == 721 {
            image_width = 720.;
        }
        let image_left = if svg_image && !factory {
            // PID721 injects a 640px SVG at x=0 in a 720px parent.
            0.
        } else {
            (size[0] - image_width) / 2. - if factory && spec.product_id == 746 { 4. } else { 0. }
        };
        let keyboard_origin = if customize_artwork {
            match spec.product_id {
                // The config wrapper selector overrides PID721's less
                // specific .keyboard-svg left declaration.
                721 => [40., 45.],
                739 => [4., 40.],
                740 => [0., 50.],
                728 | 746 => [0., -10.],
                756 => [0., 40.],
                _ => [0., 0.],
            }
        } else {
            [0., 0.]
        };
        Self {
            size,
            source_scale,
            source_config_frame,
            customize_artwork,
            image,
            image_origin: [image_left, image_top],
            image_size: [image_width, image_height],
            keyboard_origin,
        }
    }
}

pub(super) fn hypershift_help(product_id: u32) -> AnyElement {
    use razer_widgets::source_tooltip::HypershiftHelpLayout;

    let layout = match product_id {
        614 | 708 | 741 => HypershiftHelpLayout::new(362., 14.),
        721 | 728 | 740 | 742 | 746 | 756 => HypershiftHelpLayout::new(362., 20.),
        717 | 720 => HypershiftHelpLayout::new(300., 20.).narrow_top(20.),
        739 | 752 => HypershiftHelpLayout::new(362., 19.).narrow_top(26.),
        747 => HypershiftHelpLayout::new(362., 19.).narrow_top(19.),
        653 => {
            return surface::help_control("keyboard-hypershift-help", t("HYPERSHIFT_TOOLTIP"));
        }
        _ => HypershiftHelpLayout::new(300., 19.).narrow_top(26.),
    };
    surface::hypershift_help_control_with_layout(
        "keyboard-hypershift-help",
        t("HYPERSHIFT_TOOLTIP"),
        layout.line_height(16.),
    )
}

pub(super) fn columns() -> Div {
    // BodyWidgets keeps a wrapping row at every viewport width. In particular,
    // its media query adds column margins without introducing a vertical gap.
    h_flex()
        .w_full()
        .max_w(surface::css(1240.))
        .mx_auto()
        .items_start()
        .justify_center()
        .flex_wrap()
}

pub(super) fn column(content: impl IntoElement) -> KeyboardColumn {
    column_width(600., content)
}

pub(super) fn column_width(width: f32, content: impl IntoElement) -> KeyboardColumn {
    KeyboardColumn {
        width,
        content: content.into_any_element(),
    }
}

#[derive(IntoElement)]
pub(super) struct KeyboardColumn {
    width: f32,
    content: AnyElement,
}

impl RenderOnce for KeyboardColumn {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        let compact = surface::stacked_device_columns(
            f32::from(window.viewport_size().width),
            f32::from(window.rem_size()),
        );
        // `div.flex > div { flex:auto }` lets the 600px columns grow to 620px
        // inside a full 1240px body. Cards keep their source width and center.
        // The 756 calibration switch column has a source-specific 680px width.
        div()
            .w(surface::css(self.width))
            .min_w(surface::css(self.width))
            .flex_grow(1.)
            .flex_shrink(1.)
            .when(compact, |column| column.mx(surface::css(30.)))
            .child(
                div()
                    .w(surface::css(self.width))
                    .mx_auto()
                    .child(self.content),
            )
    }
}
