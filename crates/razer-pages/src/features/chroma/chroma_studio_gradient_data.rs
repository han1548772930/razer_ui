//! Current 9170:U3 / 9220:A stop conversion and 3690 Canvas2D sampling.
use super::*;

#[derive(Clone, Deserialize)]
pub(super) struct GradientDefinition {
    pub(super) presets: Vec<Vec<GradientPoint>>,
    pub(super) min_stops: usize,
    pub(super) max_stops: usize,
}

#[derive(Clone, Deserialize)]
pub(super) struct GradientPoint {
    pub(super) stop: f32,
    pub(super) rgb: Option<[u8; 3]>,
}

#[derive(Clone, PartialEq)]
pub(super) struct Stop {
    pub(super) id: u64,
    pub(super) position: f32,
    pub(super) color: Option<u32>,
}

pub(super) fn pack([r, g, b]: [u8; 3]) -> u32 {
    (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
}

pub(super) fn serialize(stops: &[Stop]) -> Value {
    let mut stops = stops.to_vec();
    stops.sort_by(|a, b| a.position.total_cmp(&b.position));
    Value::Array(
        stops
            .iter()
            .map(|stop| {
                let mut value = serde_json::json!({"Stop": (stop.position * 100.).round() as u32});
                if let Some(color) = stop.color {
                    value["Color"] = color.into();
                }
                value
            })
            .collect(),
    )
}

/// sRGB interpolation with premultiplied alpha, matching Canvas2D's
/// transparent stops. A sampled transparent pixel has zero RGB bytes.
pub(super) fn sample(stops: &[Stop], position: f32) -> Rgba {
    let color = |stop: &Stop| stop.color.map(rgb).unwrap_or_else(|| rgba(0));
    let Some(first) = stops.first() else {
        return rgba(0);
    };
    let right = stops.partition_point(|stop| stop.position <= position);
    if right == 0 {
        return color(first);
    }
    if right == stops.len() {
        return color(&stops[right - 1]);
    }
    let left = &stops[right - 1];
    let end = &stops[right];
    let amount = (position - left.position) / (end.position - left.position);
    let a = color(left);
    let b = color(end);
    let alpha = a.a + (b.a - a.a) * amount;
    let channel = |x: f32, y: f32| {
        if alpha == 0. {
            0.
        } else {
            (x * a.a + (y * b.a - x * a.a) * amount) / alpha
        }
    };
    Rgba {
        r: channel(a.r, b.r),
        g: channel(a.g, b.g),
        b: channel(a.b, b.b),
        a: alpha,
    }
}

pub(super) fn bar(stops: Vec<Stop>) -> impl IntoElement {
    canvas(
        |_, _, _| (),
        move |bounds, _, window, _| {
            let width = f32::from(bounds.size.width);
            if width <= 0. {
                return;
            }
            // Device-scaled columns keep the Canvas2D preview and insertion
            // sampling in the same sRGB space, including transparent intervals.
            let scale = window.scale_factor();
            let columns = (width * scale).ceil() as usize;
            window.with_content_mask(Some(ContentMask { bounds }), |window| {
                for x in 0..columns {
                    let start = x as f32 / scale;
                    window.paint_quad(fill(
                        Bounds::new(
                            bounds.origin + point(px(start), px(0.)),
                            size(px(1. / scale), bounds.size.height),
                        ),
                        sample(&stops, (start + 0.5 / scale) / width),
                    ));
                }
            });
        },
    )
    .size_full()
}
