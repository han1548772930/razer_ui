//! CSS filter functions operate on straight sRGB channels, in source order.

/// The filter functions used by acquired local page styles. Percentages retain
/// the source integer values; hue rotation is measured in degrees.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ImageFilter {
    Brightness(u16),
    Saturate(u16),
    Invert(u8),
    Sepia(u8),
    HueRotate(i16),
    Contrast(u16),
}

fn matrix(rgb: [f64; 3], rows: [[f64; 3]; 3]) -> [f64; 3] {
    rows.map(|row| row.into_iter().zip(rgb).map(|(a, b)| a * b).sum())
}

fn filter_rgb(mut rgb: [f64; 3], filters: &[ImageFilter]) -> [f64; 3] {
    for filter in filters {
        rgb = match *filter {
            ImageFilter::Brightness(percent) => rgb.map(|v| v * f64::from(percent) / 100.),
            ImageFilter::Contrast(percent) => {
                rgb.map(|v| (v - 0.5) * f64::from(percent) / 100. + 0.5)
            }
            ImageFilter::Invert(percent) => {
                let amount = f64::from(percent.min(100)) / 100.;
                rgb.map(|v| v * (1. - amount) + (1. - v) * amount)
            }
            ImageFilter::Sepia(percent) => {
                let amount = f64::from(percent.min(100)) / 100.;
                let sepia = matrix(
                    rgb,
                    [
                        [0.393, 0.769, 0.189],
                        [0.349, 0.686, 0.168],
                        [0.272, 0.534, 0.131],
                    ],
                );
                std::array::from_fn(|i| rgb[i] * (1. - amount) + sepia[i] * amount)
            }
            ImageFilter::Saturate(percent) => {
                let s = f64::from(percent) / 100.;
                matrix(
                    rgb,
                    [
                        [0.213 + 0.787 * s, 0.715 - 0.715 * s, 0.072 - 0.072 * s],
                        [0.213 - 0.213 * s, 0.715 + 0.285 * s, 0.072 - 0.072 * s],
                        [0.213 - 0.213 * s, 0.715 - 0.715 * s, 0.072 + 0.928 * s],
                    ],
                )
            }
            ImageFilter::HueRotate(degrees) => {
                let (s, c) = f64::from(degrees).to_radians().sin_cos();
                matrix(
                    rgb,
                    [
                        [
                            0.213 + 0.787 * c - 0.213 * s,
                            0.715 - 0.715 * c - 0.715 * s,
                            0.072 - 0.072 * c + 0.928 * s,
                        ],
                        [
                            0.213 - 0.213 * c + 0.143 * s,
                            0.715 + 0.285 * c + 0.140 * s,
                            0.072 - 0.072 * c - 0.283 * s,
                        ],
                        [
                            0.213 - 0.213 * c - 0.787 * s,
                            0.715 - 0.715 * c + 0.715 * s,
                            0.072 + 0.928 * c + 0.072 * s,
                        ],
                    ],
                )
            }
        };
        // CSS filter primitives clamp their result before the next function.
        rgb = rgb.map(|v| v.clamp(0., 1.));
    }
    rgb
}

pub(super) fn filter_bgra(pixels: &mut [u8], filters: &[ImageFilter]) -> Result<(), String> {
    if pixels.len() % 4 != 0 {
        return Err("Invalid filtered image pixel length".into());
    }
    for pixel in pixels.chunks_exact_mut(4) {
        let rgb = filter_rgb(
            [pixel[2], pixel[1], pixel[0]].map(|v| f64::from(v) / 255.),
            filters,
        );
        for (channel, value) in [2, 1, 0].into_iter().zip(rgb) {
            pixel[channel] = (value * 255.).round() as u8;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn css_filters_brightness_preserves_alpha_and_bgra_channel_order() {
        let mut pixels = [73, 73, 253, 255, 255, 255, 255, 128, 0, 80, 200, 0];
        filter_bgra(&mut pixels, &[ImageFilter::Brightness(120)]).unwrap();
        assert_eq!(
            pixels,
            [88, 88, 255, 255, 255, 255, 255, 128, 0, 96, 240, 0]
        );
        assert!(filter_bgra(&mut [0, 0, 0], &[]).is_err());
    }

    #[test]
    fn css_filters_source_ready_play_sequence_recolors_all_rgb_equally() {
        let filters = [
            ImageFilter::Brightness(0),
            ImageFilter::Saturate(100),
            ImageFilter::Invert(76),
            ImageFilter::Sepia(65),
            ImageFilter::Saturate(531),
            ImageFilter::HueRotate(73),
            ImageFilter::Brightness(95),
            ImageFilter::Contrast(92),
        ];
        let mut pixels = [204, 204, 204, 255, 3, 40, 170, 80];
        filter_bgra(&mut pixels, &filters).unwrap();
        // Independent CSS filter matrix evaluation, rounded once at output.
        assert_eq!(pixels, [110, 233, 59, 255, 110, 233, 59, 80]);
    }

    #[test]
    fn css_filters_order_clamping_and_identity_are_distinct() {
        let mut first = [32, 64, 128, 17];
        let mut second = first;
        filter_bgra(
            &mut first,
            &[ImageFilter::Brightness(200), ImageFilter::Invert(100)],
        )
        .unwrap();
        filter_bgra(
            &mut second,
            &[ImageFilter::Invert(100), ImageFilter::Brightness(200)],
        )
        .unwrap();
        assert_eq!(first, [191, 127, 0, 17]);
        assert_eq!(second, [255, 255, 254, 17]);
        let mut original = [13, 65, 212, 101];
        filter_bgra(
            &mut original,
            &[
                ImageFilter::Brightness(100),
                ImageFilter::Saturate(100),
                ImageFilter::Invert(0),
                ImageFilter::Sepia(0),
                ImageFilter::HueRotate(360),
                ImageFilter::Contrast(100),
            ],
        )
        .unwrap();
        assert_eq!(original, [13, 65, 212, 101]);
    }
}
