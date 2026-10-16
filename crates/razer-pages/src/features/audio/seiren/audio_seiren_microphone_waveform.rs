//! Current UI7405 Zn: 250ms analyser snapshots in a three-second ring, the
//! initial 100ms sample gate, six-second dB history and smoothing in canvas Y.
use super::super::audio_seiren_eq::colors::Colors as C;
use super::*;

pub(super) struct State {
    samples: VecDeque<f64>,
    ring: VecDeque<f64>,
    sample_rate: u32,
    gathered: usize,
    history: VecDeque<(Instant, f64)>,
    peaks: VecDeque<(Instant, f64)>,
    offset: f64,
    smoothed: f64,
    snapshot_at: Option<Instant>,
    external: Option<f64>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            samples: VecDeque::new(),
            ring: VecDeque::new(),
            sample_rate: 0,
            gathered: 0,
            history: VecDeque::new(),
            peaks: VecDeque::new(),
            offset: 0.,
            smoothed: -60.,
            snapshot_at: None,
            external: None,
        }
    }
}
impl State {
    pub(super) fn set_external(&mut self, level: f32) {
        if level.is_finite() {
            self.external = Some(f64::from(level.clamp(0., 100.)));
        }
    }
    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }
    pub(super) fn observe(&mut self, frame: &CaptureFrame, gain_db: f64) {
        self.observe_at(frame, gain_db, Instant::now());
    }
    fn observe_at(&mut self, frame: &CaptureFrame, gain_db: f64, now: Instant) {
        if self.sample_rate != frame.capture_sample_rate {
            self.clear();
            self.sample_rate = frame.capture_sample_rate;
            self.snapshot_at = Some(now);
        }
        let gain = 10f64.powf(gain_db / 20.);
        self.samples.extend(
            frame
                .mono_pcm
                .iter()
                .map(|sample| f64::from(*sample) * gain),
        );
        if self.samples.len() > 2048 {
            self.samples.drain(..self.samples.len() - 2048);
        }
        if let Some(level) = frame.microphone_levels.map(|levels| levels[0]) {
            self.set_external(level);
        }
    }
    fn ready(&self) -> bool {
        self.sample_rate > 0 && self.gathered.min(self.ring.len()) >= self.sample_rate as usize / 10
    }
    pub(super) fn tick(&mut self) -> bool {
        self.tick_at(Instant::now())
    }
    fn tick_at(&mut self, now: Instant) -> bool {
        let Some(snapshot_at) = self.snapshot_at else {
            return false;
        };
        if now.saturating_duration_since(snapshot_at) >= Duration::from_millis(250) {
            // Like getFloatTimeDomainData, one timer callback snapshots the
            // latest analyser without inventing missed callbacks or samples.
            self.ring
                .extend(std::iter::repeat_n(0., 2048 - self.samples.len()));
            self.ring.extend(self.samples.iter().copied());
            let capacity = self.sample_rate as usize * 3;
            if self.ring.len() > capacity {
                self.ring.drain(..self.ring.len() - capacity);
            }
            self.gathered = self.gathered.saturating_add(2048);
            self.snapshot_at = Some(now);
        }
        if !self.ready() {
            return false;
        }
        let rms = (self
            .samples
            .iter()
            .map(|sample| sample * sample)
            .sum::<f64>()
            / 2048.)
            .sqrt();
        let pcm = (20. * rms.max(1e-10).log10()).clamp(-60., 0.);
        let (db, peaking) = if let Some(level) = self.external.filter(|level| *level > 0.) {
            let db = level / 100. * 60. - 60.;
            self.offset = 0.95 * self.offset + 0.05 * (db - pcm);
            (db, level >= 95.)
        } else {
            let adjusted = (pcm + self.offset).clamp(-60., 0.);
            self.smoothed = 0.9 * self.smoothed + 0.1 * adjusted;
            (self.smoothed, adjusted > -3.)
        };
        self.history.push_back((now, db));
        while self
            .history
            .front()
            .is_some_and(|(at, _)| now.duration_since(*at) >= Duration::from_secs(6))
        {
            self.history.pop_front();
        }
        if peaking && db > -10. {
            self.peaks.push_back((now, db));
        }
        while self
            .peaks
            .front()
            .is_some_and(|(at, _)| now.duration_since(*at) >= Duration::from_secs(3))
        {
            self.peaks.pop_front();
        }
        true
    }
    fn points(&self, muted: bool, now: Instant) -> Vec<(f64, f64)> {
        if muted {
            return Vec::new();
        }
        let values = self
            .history
            .iter()
            .map(|(at, db)| {
                (
                    1. - now.saturating_duration_since(*at).as_secs_f64() / 6.,
                    y(*db),
                )
            })
            .collect::<Vec<_>>();
        if values.len() <= 2 {
            return values;
        }
        // The source smooths mapped Y, not dB. These differ across -12/-18.
        let blurred =
            values
                .iter()
                .enumerate()
                .map(|(ix, (x, _))| {
                    let (sum, weight) = (ix.saturating_sub(9)..=(ix + 9).min(values.len() - 1))
                        .fold((0., 0.), |(sum, weight), other| {
                            let delta = other as f64 - ix as f64;
                            let w = (-delta * delta / (2. * 81.)).exp();
                            (sum + values[other].1 * w, weight + w)
                        });
                    (*x, sum / weight)
                })
                .collect::<Vec<_>>();
        blurred
            .iter()
            .enumerate()
            .map(|(ix, (x, _))| {
                let window = &blurred[ix.saturating_sub(5)..(ix + 6).min(blurred.len())];
                (
                    *x,
                    window.iter().map(|(_, y)| y).sum::<f64>() / window.len() as f64,
                )
            })
            .collect()
    }
}
fn y(db: f64) -> f64 {
    if db >= -12. {
        0.3 * (db / -12.)
    } else if db >= -18. {
        0.3 + 0.3 * ((db + 12.) / -6.)
    } else {
        0.6 + 0.4 * ((db + 18.) / -42.)
    }
}
fn line(window: &mut Window, a: Point<Pixels>, b: Point<Pixels>, color: Hsla, width: f32) {
    let mut path = PathBuilder::stroke(px(width));
    path.move_to(a);
    path.line_to(b);
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}

impl AudioProductWorkspace {
    pub(super) fn seiren_microphone_waveform(&self) -> AnyElement {
        let muted = self.draft["profile"]["micVolume"]["isEnabled"] != true;
        let now = Instant::now();
        let ready = self.seiren_mic.waveform.ready();
        let points = self.seiren_mic.waveform.points(muted, now);
        let peaks = self
            .seiren_mic
            .waveform
            .peaks
            .iter()
            .map(|(at, db)| {
                (
                    *db,
                    1. - now.saturating_duration_since(*at).as_secs_f32() / 3.,
                )
            })
            .collect::<Vec<_>>();
        let pid = self.spec.product_id;
        div()
            .relative()
            .w_full()
            .h(surface::css(100.))
            .bg(C::waveform())
            .overflow_hidden()
            .child(
                canvas(
                    move |_, _, _| {},
                    move |bounds, _, window, _| {
                        let scale = f32::from(window.rem_size()) / 16.;
                        let full_height = f32::from(bounds.size.height);
                        let width = (f32::from(bounds.size.width) - 25. * scale).max(1.);
                        let height = (full_height - 18. * scale).max(1.);
                        let point_at = |x: f64, y: f64| {
                            bounds.origin
                                + point(px(width * x as f32), px(10. * scale + height * y as f32))
                        };
                        if !ready {
                            line(
                                window,
                                bounds.origin + point(px(0.), px(full_height * 0.85)),
                                bounds.origin + point(px(width), px(full_height * 0.85)),
                                C::border().opacity(0.6),
                                scale,
                            );
                            return;
                        }
                        for db in [-60., -18., -12., -3.] {
                            line(
                                window,
                                point_at(0., y(db)),
                                point_at(1., y(db)),
                                if db == -3. {
                                    C::peak().opacity(0.7)
                                } else if db == -60. {
                                    C::border().opacity(0.5)
                                } else {
                                    C::selected().opacity(0.7)
                                },
                                if db == -60. { scale } else { 1.5 * scale },
                            );
                        }
                        line(
                            window,
                            point_at(1., 0.),
                            point_at(1., 1.),
                            C::border().opacity(0.3),
                            scale,
                        );
                        for db in [-3., -12., -18., -60.] {
                            line(
                                window,
                                point_at(1., y(db)) - point(px(5. * scale), px(0.)),
                                point_at(1., y(db)),
                                C::border().opacity(0.3),
                                scale,
                            );
                        }
                        let stroke = lyon_tessellation::StrokeOptions::default()
                            .with_line_width(if points.is_empty() { scale } else { 1.5 * scale })
                            .with_line_cap(lyon_tessellation::LineCap::Round)
                            .with_line_join(lyon_tessellation::LineJoin::Round);
                        let mut path =
                            PathBuilder::stroke(px(scale)).with_style(PathStyle::Stroke(stroke));
                        if points.is_empty() {
                            path.move_to(point_at(0., 1.));
                            path.line_to(point_at(1., 1.));
                        } else {
                            path.move_to(point_at(points[0].0, points[0].1));
                            for pair in points.windows(2) {
                                if points.len() > 2 {
                                    path.curve_to(
                                        point_at(
                                            (pair[0].0 + pair[1].0) / 2.,
                                            (pair[0].1 + pair[1].1) / 2.,
                                        ),
                                        point_at(pair[0].0, pair[0].1),
                                    );
                                } else {
                                    path.line_to(point_at(pair[1].0, pair[1].1));
                                }
                            }
                            if let Some(last) = points.last() {
                                path.line_to(point_at(last.0, last.1));
                            }
                        }
                        if let Ok(path) = path.build() {
                            window.paint_path(path, C::border());
                        }
                        for (db, opacity) in &peaks {
                            let center = point_at(1., y(*db)) - point(px(10. * scale), px(0.));
                            let radius = 4. * scale;
                            let mut dot = fill(
                                Bounds::new(
                                    center - point(px(radius), px(radius)),
                                    size(px(2. * radius), px(2. * radius)),
                                ),
                                C::waveform_peak().opacity(opacity.clamp(0., 1.) * 0.8),
                            );
                            dot.corner_radii = px(radius).into();
                            window.paint_quad(dot);
                        }
                    },
                )
                .size_full(),
            )
            .when(ready, |waveform| {
                waveform
                    .child(
                        div()
                            .absolute()
                            .left(surface::css(10.))
                            .top(surface::css(10. + 82. * y(-3.) as f32 - 17.))
                            .font_family("monospace")
                            .text_size(surface::css(12.))
                            .line_height(surface::css(12.))
                            .text_color(C::peak().opacity(0.9))
                            .child(text(pid, "hGj")),
                    )
                    .child(
                        div()
                            .absolute()
                            .left(surface::css(10.))
                            .top(surface::css(
                                10. + 82. * ((y(-18.) + y(-12.)) / 2.) as f32 - 8.,
                            ))
                            .font_family("monospace")
                            .text_size(surface::css(12.))
                            .line_height(surface::css(12.))
                            .text_color(C::selected().opacity(0.9))
                            .child(text(pid, "FJG")),
                    )
                    .child(
                        div()
                            .absolute()
                            .right(surface::css(30.))
                            .top(surface::css(10. + 82. * y(-3.) as f32 - 15.))
                            .font_family("monospace")
                            .text_size(surface::css(10.))
                            .line_height(surface::css(10.))
                            .text_color(C::text())
                            .child("dB"),
                    )
                    .children(
                        [(-3., "-3"), (-12., "-12"), (-18., "-18"), (-60., "-60")].map(
                            |(db, label)| {
                                div()
                                    .absolute()
                                    .right_0()
                                    .top(surface::css(
                                        10. + 82. * y(db) as f32
                                            + if db == -60. { -2. } else { -6. },
                                    ))
                                    .w(surface::css(20.))
                                    .text_size(surface::css(10.))
                                    .line_height(surface::css(10.))
                                    .font_family("monospace")
                                    .text_color(C::text())
                                    .child(label)
                            },
                        ),
                    )
            })
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // The glob above carries GPUI's `test` macro, which expands to `#[test]`.
    // Name the built-in attribute macro so these logic tests stay plain Rust.
    // The glob imports above also carry GPUI's `test` macro; name the
    // built-in attribute so this remains an ordinary Rust test.
    use std::prelude::v1::test;
    fn frame() -> CaptureFrame {
        CaptureFrame {
            capture_sample_rate: 48_000,
            output_sample_rate: None,
            mono_pcm: vec![0.1; 2048],
            microphone_levels: Some([50., 50.]),
            headphone_levels: None,
            endpoint_id: "simulated".into(),
            label: "simulated".into(),
            default_fallback: false,
            diagnostics: Vec::new(),
        }
    }
    #[test]
    fn microphone_waveform_uses_real_pcm_and_resets_calibration() {
        let mut state = State::default();
        let now = Instant::now();
        state.observe_at(&frame(), 0., now);
        for ms in [250, 500] {
            assert!(!state.tick_at(now + Duration::from_millis(ms)));
        }
        assert!(state.history.is_empty());
        assert!(state.tick_at(now + Duration::from_millis(750)));
        assert_eq!(state.history.back().unwrap().1, -30.);
        assert!(state.offset < 0.);
        state.clear();
        assert!(state.history.is_empty() && state.samples.is_empty() && state.ring.is_empty());
        assert_eq!(state.offset, 0.);
    }
    #[test]
    fn microphone_waveform_smooths_mapped_y_and_fades_real_peak_history() {
        let mut state = State::default();
        let now = Instant::now();
        for (ix, db) in [-60., -18., 0.].into_iter().enumerate() {
            state
                .history
                .push_back((now - Duration::from_millis(32 - ix as u64 * 16), db));
        }
        let points = state.points(false, now);
        assert!(points.iter().all(|(_, y)| (*y - 1.6 / 3.).abs() < 0.01));
        assert!(state.points(true, now).is_empty());
        let mut frame = frame();
        frame.microphone_levels = Some([100.; 2]);
        state.observe_at(&frame, 0., now);
        for ms in [250, 500, 750] {
            state.tick_at(now + Duration::from_millis(ms));
        }
        assert_eq!(state.peaks.len(), 1);
        state.set_external(50.);
        state.tick_at(now + Duration::from_millis(3750));
        assert!(state.peaks.is_empty());
        assert!(state.ring.len() <= 48_000 * 3);
    }
}
