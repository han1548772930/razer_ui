//! Current `qP/YL/WP` artwork and `ZP/FL/VP` 100-segment meters. Complete
//! per-product JSX, CSS and resource receipts: gamepad-main-audit/gamepad_triggers.
//! Cached images belong to the workspace; neutral initial paint is not a sample.
use super::GamepadProductWorkspace;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_i18n::t;
use razer_widgets::{surface, theme::GamepadTriggerColors as Colors};
use serde_json::Value;
use std::{fmt::Write, io::Cursor, sync::Arc};

const LEFT_PATH: &str = "M21 70.5001C15.4 71.7001 10.2409 73.0001 8 73.0001C-1 73.0001 0.5 64.5001 0.5 63.0001C0.5 61.5001 2.5 46.0001 4 40.0001C5.5 34.0001 11 18.5001 12.5 16.5001C14.3974 13.9702 22 5.50006 27 3.00006C32.2824 0.358849 42 6.10352e-05 45.5 1.50006C48 3.00006 52 9.00006 54 12.5001C56 16.0001 67 51.0001 67 58.5001C67 64.5001 62.5 65.0001 59.5 65.5001C57 65.9167 52 66.0001 50 66.5001C47.5746 67.1064 28 69.0001 21 70.5001Z";
const RIGHT_PATH: &str = "M46.4062 70.5001C52.0063 71.7001 57.1654 73.0001 59.4062 73.0001C68.4062 73.0001 66.9062 64.5001 66.9062 63.0001C66.9062 61.5001 64.9062 46.0001 63.4062 40.0001C61.9062 34.0001 56.4062 18.5001 54.9062 16.5001C53.0089 13.9702 45.4062 5.50006 40.4062 3.00006C35.1238 0.358849 25.4062 6.10352e-05 21.9062 1.50006C19.4062 3.00006 15.4062 9.00006 13.4062 12.5001C11.4062 16.0001 0.40625 51.0001 0.40625 58.5001C0.40625 64.5001 4.90625 65.0001 7.90625 65.5001C10.4062 65.9167 15.4062 66.0001 17.4062 66.5001C19.8316 67.1064 39.4062 69.0001 46.4062 70.5001Z";

#[derive(Clone, Copy, PartialEq)]
struct Meter {
    min: f64,
    max: f64,
    active: f64,
    opacity: f64,
}

impl Meter {
    fn from_state(settings: &Value, digital_mode: i64, input: f64, binary: bool) -> Self {
        let digital = settings["mode"].as_i64() == Some(digital_mode);
        let min = settings[if digital { "actuationPoint" } else { "startRange" }]
            .as_f64()
            .unwrap_or(0.);
        let max = if digital {
            100.
        } else {
            settings["endRange"].as_f64().unwrap_or(100.)
        };
        let rounded = (100. * input).ceil() / 100.;
        Self {
            min,
            max,
            active: if input >= 0.99 {
                100.
            } else if input <= 0. {
                0.
            } else {
                min + rounded * (max - min)
            },
            opacity: if binary {
                if input > 0. { 0.3 } else { 0. }
            } else {
                0.2 + rounded * 0.5
            },
        }
    }
}

struct Artwork {
    edition: u32,
    image: Arc<Image>,
    svg: Option<String>,
    width: f32,
    height: f32,
}

#[derive(Default)]
pub(super) struct TriggerVisualization {
    artwork: Option<Artwork>,
    meters: Option<[Arc<Image>; 2]>,
    overlays: Option<[Arc<Image>; 2]>,
    values: Option<[Meter; 2]>,
    error: Option<String>,
}

fn source_artwork(pid: u32, edition: u32, cx: &App) -> anyhow::Result<Artwork> {
    let is_svg = matches!(pid, 2676 | 2684 | 4144);
    let extension = if is_svg { "svg" } else { "png" };
    let path = |edition| format!("synapse/gamepad-trigger-device/{pid}/{edition}.{extension}");
    let bytes = crate::runtime_resources::load_edition_resource(edition, |edition| {
        cx.asset_source().load(&path(edition))
    })?
    .ok_or_else(|| anyhow::anyhow!("Missing current controller actuation artwork for {pid}"))?;
    decode_artwork(pid, edition, bytes.as_ref())
}

fn decode_artwork(pid: u32, edition: u32, bytes: &[u8]) -> anyhow::Result<Artwork> {
    let is_svg = matches!(pid, 2676 | 2684 | 4144);
    let (width, height, svg, format) = if is_svg {
        let svg = String::from_utf8(bytes.to_vec())?;
        let (width, height) = if pid == 4144 {
            (500., 500. * 185. / 479.)
        } else {
            (400., 170.)
        };
        (width, height, Some(svg), ImageFormat::Svg)
    } else {
        let (width, height) = image::ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()?
            .into_dimensions()?;
        (
            500.,
            500. * height as f32 / width as f32,
            None,
            ImageFormat::Png,
        )
    };
    Ok(Artwork {
        edition,
        image: Arc::new(Image::from_bytes(format, bytes.to_vec())),
        svg,
        width,
        height,
    })
}

fn image(svg: String) -> Arc<Image> {
    Arc::new(Image::from_bytes(ImageFormat::Svg, svg.into_bytes()))
}

fn meter_svg(meter: Meter, left: bool) -> String {
    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="160" height="170" viewBox="-70 -50 160 100"><g transform="{}">"#,
        if left { "translate(20 0) scale(-1 1)" } else { "" },
    );
    let step = std::f64::consts::PI / 100.;
    for index in 0..100 {
        let start = -std::f64::consts::FRAC_PI_2 + index as f64 * step + 0.005;
        let end = start + step - 0.01;
        let active = index as f64 + 1. <= meter.active;
        let inner = if active { 65. } else { 73. };
        let available = index as f64 + 1. >= meter.min && (index as f64) < meter.max;
        let fill = if !available {
            Colors::meter_excluded_svg()
        } else if active {
            Colors::accent_svg()
        } else {
            Colors::meter_available_svg()
        };
        write!(svg,
            r#"<path d="M {} {} A 83 83 0 0 1 {} {} L {} {} A {inner} {inner} 0 0 0 {} {} Z" fill="{fill}"/>"#,
            83. * start.cos(), 83. * start.sin(), 83. * end.cos(), 83. * end.sin(),
            inner * end.cos(), inner * end.sin(), inner * start.cos(), inner * start.sin(),
        ).expect("writing SVG string");
    }
    svg.push_str("</g></svg>");
    svg
}

fn overlay_svg(path: &str, opacity: f64) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="63.65" height="69.35" viewBox="0 0 67 73"><path d="{path}" fill="{}" fill-opacity="{opacity}"/></svg>"#,
        Colors::accent_svg()
    )
}

fn dynamic_svg(source: &str, values: [Meter; 2]) -> String {
    ["leftTrigger", "rightTrigger"]
        .into_iter()
        .zip(values)
        .fold(source.to_owned(), |svg, (class, meter)| {
            // All three audited originals have exactly these two opacity attributes.
            // Preserve the remaining original SVG bytes, including gradients and masks.
            svg.replace(
                &format!(r#"class="{class}" opacity="0.3""#),
                &format!(r#"class="{class}" opacity="{}""#, meter.opacity),
            )
        })
}

fn media_scale(width: f32, root_font: f32) -> f32 {
    match width * 16. / root_font.max(1.) {
        width if width <= 400. => 0.5,
        width if width <= 650. => 0.6,
        width if width <= 850. => 0.8,
        _ => 1.,
    }
}

impl GamepadProductWorkspace {
    pub(super) fn trigger_artwork_pending(&self) -> bool {
        self.page == "TRIGGERS" && self.trigger_visualization.artwork.is_none()
    }

    pub(super) fn refresh_trigger_visualization(&mut self, cx: &App) {
        if self.page != "TRIGGERS" || !self.spec.pages.iter().any(|page| page == "TRIGGERS") {
            return;
        }
        let state = &mut self.trigger_visualization;
        let edition_changed = state
            .artwork
            .as_ref()
            .is_none_or(|art| art.edition != self.edition_id);
        if edition_changed {
            match source_artwork(self.spec.product_id, self.edition_id, cx) {
                Ok(artwork) => {
                    state.artwork = Some(artwork);
                    state.error = None;
                }
                Err(error) => {
                    state.artwork = None;
                    state.error = Some(error.to_string());
                }
            }
        }
        let observed = self
            .controller_input_sample
            .as_ref()
            .map_or([0.; 2], |sample| sample.triggers());
        let binary = matches!(self.spec.product_id, 2676 | 2684);
        let values = ["leftTrigger", "rightTrigger"].map(|side| {
            Meter::from_state(
                &self.draft["controller"][side],
                self.spec.digital_mode,
                observed[usize::from(side == "rightTrigger")],
                binary,
            )
        });
        if state.values == Some(values) && !edition_changed {
            return;
        }
        state.meters = Some([
            image(meter_svg(values[0], true)),
            image(meter_svg(values[1], false)),
        ]);
        if let Some(artwork) = &mut state.artwork {
            if let Some(svg) = &artwork.svg {
                artwork.image = image(dynamic_svg(svg, values));
                state.overlays = None;
            } else {
                state.overlays = Some([
                    image(overlay_svg(LEFT_PATH, values[0].opacity)),
                    image(overlay_svg(RIGHT_PATH, values[1].opacity)),
                ]);
            }
        }
        state.values = Some(values);
    }

    pub(super) fn trigger_visualization(&self, window: &Window) -> AnyElement {
        let playstation = self.spec.info["isPlaystationController"].as_bool() == Some(true);
        let (left, right) = if playstation { ("L2", "R2") } else { ("LT", "RT") };
        let description = t("ACTUATION_DESC")
            .replace("{{left}}", left)
            .replace("{{right}}", right);
        let binary = matches!(self.spec.product_id, 2676 | 2684);
        let state = &self.trigger_visualization;
        let scale = media_scale(
            window.viewport_size().width.into(),
            window.rem_size().into(),
        );
        let height = state
            .artwork
            .as_ref()
            .map_or(170., |art| art.height.max(170.));
        let mut row = h_flex()
            .absolute()
            .left(relative(0.5))
            .ml(surface::css(-850. * scale / 2.))
            .top(surface::css(height * (1. - scale) / 2.))
            .w(surface::css(850. * scale))
            .h(surface::css(height * scale))
            .justify_center()
            .items_start()
            .when(binary, |row| row.items_center());
        if let Some(meters) = &state.meters {
            row = row.child(
                img(Arc::clone(&meters[0]))
                    .w(surface::css(160. * scale))
                    .h(surface::css(170. * scale))
                    .flex_shrink_0()
                    .bg(Colors::meter_background()),
            );
        }
        if let Some(artwork) = &state.artwork {
            let mut art = div()
                .relative()
                .flex_shrink_0()
                .w(surface::css(artwork.width * scale))
                .h(surface::css(artwork.height * scale))
                .child(img(Arc::clone(&artwork.image)).size_full());
            if let Some(overlays) = &state.overlays {
                let local_scale = if playstation { 1. } else { 0.95 };
                let width = 63.65 * local_scale;
                let height = 69.35 * local_scale;
                for (index, overlay) in overlays.iter().enumerate() {
                    let x = if index == 0 {
                        if playstation { 65. } else { 83. }
                    } else {
                        artwork.width - 63.65 - if playstation { 63. } else { 78. }
                    };
                    art = art.child(
                        img(Arc::clone(overlay))
                            .absolute()
                            .left(surface::css((x + (63.65 - width) / 2.) * scale))
                            .top(surface::css(
                                ((if playstation { 51. } else { 60. }) + (69.35 - height) / 2.)
                                    * scale,
                            ))
                            .w(surface::css(width * scale))
                            .h(surface::css(height * scale)),
                    );
                }
            }
            row = row.child(art);
        }
        if let Some(meters) = &state.meters {
            row = row.child(
                img(Arc::clone(&meters[1]))
                    .w(surface::css(160. * scale))
                    .h(surface::css(170. * scale))
                    .flex_shrink_0()
                    .bg(Colors::meter_background()),
            );
        }
        v_flex()
            .h(surface::css(255.))
            .flex_shrink_0()
            .my(surface::css(50.))
            .w_full()
            .items_center()
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .text_color(Colors::text())
            .text_center()
            .child(div().flex_shrink_0().child(description))
            .child(div().h(surface::css(17.)).flex_shrink_0())
            .child(
                div()
                    .relative()
                    .flex_shrink_0()
                    .self_stretch()
                    .min_w(surface::css(850.))
                    .mx(surface::css(if binary { 20. } else { 30. }))
                    .h(surface::css(height))
                    .mt(surface::css(if binary { 70. } else { 30. }))
                    .child(row),
            )
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::super::resource_fixtures as fixtures;
    use super::{Meter, decode_artwork, dynamic_svg, media_scale};
    use serde_json::json;
    #[test]
    fn live_trigger_range_rounding_and_binary_illustrations_follow_source() {
        let settings = json!({"mode":0,"startRange":10,"endRange":80,"actuationPoint":25});
        let neutral = Meter::from_state(&settings, 1, 0., false);
        assert_eq!(
            (neutral.min, neutral.max, neutral.active, neutral.opacity),
            (10., 80., 0., 0.2)
        );
        assert!((Meter::from_state(&settings, 1, 0.321, false).active - 33.1).abs() < 1e-10);
        assert_eq!(Meter::from_state(&settings, 1, 0.99, false).active, 100.);
        assert_eq!(Meter::from_state(&settings, 0, 0.5, false).active, 62.5);
        assert_eq!(Meter::from_state(&settings, 1, 0., true).opacity, 0.);
        assert_eq!(Meter::from_state(&settings, 1, 0.001, true).opacity, 0.3);
    }

    #[test]
    fn every_product_edition_has_its_manifest_entry_and_base_edition_fallback() {
        let entries = razer_assets::list_resources("synapse/gamepad-trigger-device/")
            .unwrap()
            .into_iter()
            .map(|key| key.to_string())
            .collect::<std::collections::BTreeSet<_>>();
        for (pid, editions) in [
            (2629, vec![0, 128]),
            (2636, vec![0, 128, 129]),
            (2647, vec![0, 128]),
            (2676, vec![0]),
            (2684, vec![0]),
            (4133, vec![0, 128, 129]),
            (4144, vec![0]),
        ] {
            for edition in editions.into_iter().chain([u32::MAX]) {
                let extension = if matches!(pid, 2676 | 2684 | 4144) {
                    "svg"
                } else {
                    "png"
                };
                let path =
                    |edition| format!("synapse/gamepad-trigger-device/{pid}/{edition}.{extension}");
                if edition != u32::MAX {
                    assert!(entries.contains(&path(edition)));
                }
                assert!(entries.contains(&path(0)));
            }
        }
        assert_eq!(media_scale(850., 16.), 0.8);
        assert_eq!(media_scale(1300., 32.), 0.6);
        assert_eq!(media_scale(400., 16.), 0.5);
    }

    #[test]
    fn original_svg_trigger_elements_accept_observations_without_resource_io() {
        for &(pid, source) in fixtures::GAMEPAD_TRIGGER_ELEMENTS {
            let art = decode_artwork(pid, 0, source.as_bytes()).unwrap();
            assert!(art.width > 0. && art.height > 0.);
            let meter = Meter {
                min: 0.,
                max: 100.,
                active: 50.,
                opacity: 0.125,
            };
            let changed = dynamic_svg(&art.svg.unwrap(), [meter; 2]);
            for side in ["leftTrigger", "rightTrigger"] {
                assert!(changed.contains(&format!(r#"class="{side}" opacity="0.125""#)));
            }
        }
    }
}
