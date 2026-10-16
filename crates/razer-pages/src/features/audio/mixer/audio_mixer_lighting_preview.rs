//! Exact JSX SVG geometry from modules13832/16764/34758/76981/89038/40889/
//! 15860/6186; each template and source SHA is in the1342 resources manifest.
use super::*;
use std::sync::Arc;

const WORD: &str = include_str!(
    "../../../../../../assets/synapse/shared/source-unverified/2fa8128637e2a94fb101fac1e1da8440947f4aa6c44221b10d7cf15c65854232.svg"
);
const BASE: &str = include_str!(
    "../../../../../../assets/synapse/shared/source-unverified/77735323e07166ff550a0d045ee45a1f2588c4b07bae6855ce4a41ce066fc771.svg"
);
const WAVE: &str = include_str!(
    "../../../../../../assets/synapse/shared/source-unverified/35450f7f9018227a5ee60ec4569878b790bafb937bc7119b50c08f9762fc108f.svg"
);
const FIRE: &str = include_str!(
    "../../../../../../assets/synapse/shared/source-unverified/647c369bd068fc9c60130224a4ec3e5f77624917aca9b541fb4e1c2d2c8a312f.svg"
);
const FOREGROUND: &str = include_str!(
    "../../../../../../assets/synapse/shared/source-unverified/fcec9268bb2fc2bb97d46eaf749212a42ffe4adee0d46dbe0c0907eeda202740.svg"
);
const CHANNEL: &str = include_str!(
    "../../../../../../assets/synapse/shared/source-unverified/cdd0a25d068eac4f8e33fc7d5f2629f1cdf85b082220101d57a1608ccde65cc0.svg"
);
const MIC: &str = include_str!(
    "../../../../../../assets/synapse/shared/source-unverified/6c3bf0f4ba81e253cc5513d5ea2b16bc25bb42c78975e9b2400242562835fb3e.svg"
);
const BLEEP: &str = include_str!(
    "../../../../../../assets/synapse/shared/source-unverified/ab5a30324433a75312c88674964c1372cb82bf87e875049a2e12d127af62ab30.svg"
);

#[derive(Deserialize)]
struct ForegroundColor {
    channel: usize,
    fire: String,
    spectrum: String,
    wave: String,
    other: String,
}
fn foreground_colors() -> &'static [ForegroundColor] {
    static COLORS: OnceLock<Vec<ForegroundColor>> = OnceLock::new();
    COLORS.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../../../../assets/synapse/products/1342/lighting-foreground-colors.json"
        ))
        .expect("source fill expressions")
    })
}
fn source_color(value: &Value, fallback: &str) -> String {
    value
        .as_str()
        .filter(|s| {
            *s == "no-color"
                || s.strip_prefix('#')
                    .is_some_and(|hex| hex.len() == 6 && u32::from_str_radix(hex, 16).is_ok())
        })
        .unwrap_or(fallback)
        .into()
}
fn replace_colors(template: &str, colors: &[String]) -> String {
    colors
        .iter()
        .enumerate()
        .fold(template.to_owned(), |svg, (index, color)| {
            svg.replace(&format!("{{{{COLOR{index}}}}}"), color)
        })
}
impl AudioProductWorkspace {
    pub(super) fn refresh_mixer_lighting_previews(&mut self) {
        let mut images = Vec::new();
        let color = if self.lighting_enabled(4) {
            source_color(&self.lighting_setting(4)["color1"], "#ff18ff")
        } else {
            "#808080".into()
        };
        images.push(((4, 0), replace_colors(WORD, &[color])));
        let effect = self.lighting_effect_id(5);
        let settings = self.lighting_setting(5);
        let colors = (0..4)
            .map(|index| {
                source_color(
                    if effect == 2 {
                        &settings["color1"]
                    } else {
                        &settings["color1"][index]
                    },
                    "#FF00FF",
                )
            })
            .collect::<Vec<_>>();
        images.push((
            (5, 0),
            replace_colors(
                if effect == 4 {
                    WAVE
                } else if effect == 8 {
                    FIRE
                } else {
                    BASE
                },
                &colors,
            ),
        ));
        let mode = self.draft["device"]["fader"]["useMode"]
            .as_u64()
            .unwrap_or(0);
        let mut foreground = FOREGROUND.to_owned();
        for (index, fill) in foreground_colors().iter().enumerate() {
            let region = fill.channel as u32 + 6;
            let effect = if mode == 0 {
                self.lighting_effect_id(region)
            } else {
                1
            };
            let color = match effect {
                1 | 2 => source_color(&self.lighting_setting(region)["color1"], "#00ff00"),
                3 => fill.spectrum.clone(),
                4 => fill.wave.clone(),
                8 => fill.fire.clone(),
                _ => fill.other.clone(),
            };
            foreground = foreground.replace(&format!("{{{{FORE{index}}}}}"), &color);
        }
        images.push(((6, 0), foreground));
        for (region, indices) in [
            (16, &[0usize, 1, 2, 4, 6][..]),
            (32, &[0usize, 1][..]),
            (33, &[0usize, 1][..]),
        ] {
            for index in indices {
                let color = source_color(
                    &self.lighting_setting(region)["color1"][*index],
                    if *index % 2 == 0 { "#00ff00" } else { "#ff0000" },
                );
                let svg = if region == 33 {
                    BLEEP.replace("#ffffff", &color)
                } else {
                    replace_colors(if region == 16 { CHANNEL } else { MIC }, &[color])
                };
                images.push(((region, *index), svg));
            }
        }
        for (key, svg) in images {
            if self
                .mixer_lighting
                .previews
                .get(&key)
                .is_none_or(|(old, _)| old != &svg)
            {
                let image = Arc::new(Image::from_bytes(ImageFormat::Svg, svg.as_bytes().to_vec()));
                self.mixer_lighting.previews.insert(key, (svg, image));
            }
        }
    }
    pub(super) fn mixer_lighting_preview(&self, region: u32) -> AnyElement {
        if region == 10 {
            let colors = self.lighting_setting(region);
            return h_flex()
                .justify_around()
                .children((0..4).map(|index| {
                    let value = if self.lighting_enabled(region) {
                        source_color(&colors["color1"][index], "#ff18ff")
                    } else {
                        "#808080".into()
                    };
                    let color = value
                        .strip_prefix('#')
                        .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                        .map(rgb)
                        .map(Hsla::from)
                        .unwrap_or(C::text());
                    div()
                        .font_family("RazerF5")
                        .text_size(surface::css(20.))
                        .text_color(color)
                        .child(format!("CH {}", index + 1))
                }))
                .into_any_element();
        }
        if region == 5 && self.lighting_effect_id(5) == 13 {
            return div().into_any_element();
        }
        let Some((_, image)) = self.mixer_lighting.previews.get(&(region, 0)) else {
            return div().into_any_element();
        };
        let (width, height) = if region == 4 { (130., 20.) } else { (212., 160.) };
        let opacity = if region == 5 {
            self.draft["profile"]["brightness"][brightness_index(region)]["value"]
                .as_f64()
                .unwrap_or(100.) as f32
                / 100.
        } else {
            1.
        };
        div()
            .when(region == 4, |d| d.h(surface::css(28.)))
            .child(
                img(image.clone())
                    .w(surface::css(width))
                    .h(surface::css(height))
                    .opacity(opacity),
            )
            .into_any_element()
    }
    pub(super) fn mixer_lighting_button_preview(&self, region: u32, index: usize) -> AnyElement {
        let Some((_, image)) = self.mixer_lighting.previews.get(&(region, index)) else {
            return div().into_any_element();
        };
        let (width, height) = match region {
            16 => (25., 18.),
            32 => (20., 20.),
            _ => (150., 40.),
        };
        let opacity = if region == 33 {
            self.draft["profile"]["brightness"][brightness_index(region)]["value"]
                .as_f64()
                .unwrap_or(100.) as f32
                / 100.
        } else {
            1.
        };
        div()
            .when(region != 33, |d| {
                d.border_1()
                    .border_color(C::border())
                    .rounded(surface::css(3.))
                    .py(surface::css(10.))
                    .px(surface::css(if region == 16 { 23. } else { 65. }))
            })
            .child(
                img(image.clone())
                    .w(surface::css(width))
                    .h(surface::css(height))
                    .opacity(opacity),
            )
            .into_any_element()
    }
}
