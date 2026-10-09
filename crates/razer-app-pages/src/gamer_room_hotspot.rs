//! gamer_room_hotspot_animation.53dd5566.svg: recovered by current content
//! fingerprint, then parsed as XML. The native SVG decoder does not run SMIL.
use super::*;
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Deserialize)]
struct Track {
    duration_seconds: f64,
    key_times: [f32; 4],
    values: [f32; 4],
    key_splines: [[f32; 4]; 3],
}
impl Track {
    fn sample(&self, elapsed: f64) -> f32 {
        let progress = (elapsed.rem_euclid(self.duration_seconds) / self.duration_seconds) as f32;
        let segment = self
            .key_times
            .windows(2)
            .position(|times| progress < times[1])
            .unwrap_or(2);
        let local = (progress - self.key_times[segment])
            / (self.key_times[segment + 1] - self.key_times[segment]);
        let [x1, y1, x2, y2] = self.key_splines[segment];
        let eased = Easing::CubicBezier { x1, y1, x2, y2 }.sample(local);
        self.values[segment] + (self.values[segment + 1] - self.values[segment]) * eased
    }
}

#[derive(Deserialize)]
struct Spec {
    width: f32,
    height: f32,
    view_box: [f32; 4],
    center: [f32; 2],
    stroke_rgb: [u8; 3],
    stroke_opacity: f32,
    radius: Track,
    stroke_width: Track,
    opacity: Track,
}
fn spec() -> &'static Spec {
    static SPEC: OnceLock<Spec> = OnceLock::new();
    SPEC.get_or_init(|| {
        serde_json::from_str(include_str!("gamer_room_hotspot_data.json"))
            .expect("audited current Gamer Room hotspot timeline")
    })
}

#[derive(IntoElement)]
pub struct HotspotPulse {
    id: ElementId,
    base: Stateful<Div>,
}
impl HotspotPulse {
    pub fn new(id: &'static str) -> Self {
        let spec = spec();
        Self {
            id: id.into(),
            base: div()
                .id(id)
                .relative()
                .w(surface::css(spec.width))
                .h(surface::css(spec.height))
                .flex_shrink_0()
                .overflow_hidden(),
        }
    }
}
impl Styled for HotspotPulse {
    fn style(&mut self) -> &mut StyleRefinement {
        self.base.style()
    }
}
impl RenderOnce for HotspotPulse {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let source = spec();
        let now = cx.background_executor().now();
        let started = window.use_keyed_state((self.id, "smil-start"), cx, |_, _| now);
        // Local reduced-motion support pauses at the SVG's first values; it
        // does not introduce a guessed midpoint or change the normal timeline.
        let elapsed = if cx.reduce_motion() {
            0.
        } else {
            window.request_animation_frame();
            now.saturating_duration_since(*started.read(cx))
                .as_secs_f64()
        };
        let radius = source.radius.sample(elapsed);
        let stroke = source.stroke_width.sample(elapsed);
        let opacity = source.opacity.sample(elapsed) * source.stroke_opacity;
        let [r, g, b] = source.stroke_rgb;
        let color: Hsla = rgb((u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)).into();
        self.base.child(
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    if opacity <= 0. {
                        return;
                    }
                    let scale = (bounds.size.width / source.view_box[2])
                        .min(bounds.size.height / source.view_box[3]);
                    let offset = point(
                        (bounds.size.width - scale * source.view_box[2]) / 2.,
                        (bounds.size.height - scale * source.view_box[3]) / 2.,
                    );
                    let center = bounds.origin
                        + offset
                        + point(
                            scale * (source.center[0] - source.view_box[0]),
                            scale * (source.center[1] - source.view_box[1]),
                        );
                    let radius = scale * radius;
                    // Two exact SVG elliptical arcs retain a centered fractional
                    // stroke. A CSS border would snap its width to device pixels.
                    let mut path = PathBuilder::stroke(scale * stroke);
                    path.move_to(center + point(radius, px(0.)));
                    path.arc_to(
                        point(radius, radius),
                        px(0.),
                        false,
                        true,
                        center - point(radius, px(0.)),
                    );
                    path.arc_to(
                        point(radius, radius),
                        px(0.),
                        false,
                        true,
                        center + point(radius, px(0.)),
                    );
                    path.close();
                    if let Ok(path) = path.build() {
                        window.paint_path(path, color.opacity(opacity));
                    }
                },
            )
            .size_full(),
        )
    }
}
