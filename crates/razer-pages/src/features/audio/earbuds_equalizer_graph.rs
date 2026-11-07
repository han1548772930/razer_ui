//! Compact earbud EQ: local pointer preview, one submission on global release.
use super::*;
use gpui_kit::base::motion::{self, Transition};
use razer_device::earbuds_equalizer::{Preset, Setting};
use razer_widgets::{
    keyboard_navigation::Button as SourceButton, theme::AudioEqualizerColors as C,
};
use std::{cell::Cell, rc::Rc, time::Duration};

// Chart.js fits the authored 1030x300 canvas to .line's 1002px width,
// retaining its aspect ratio and flooring the resulting height.
const WIDTH: f32 = 1002.;
const HEIGHT: f32 = 291.;
const FREQUENCIES: [&str; 10] = [
    "31Hz", "63Hz", "125Hz", "250Hz", "500Hz", "1kHz", "2kHz", "4kHz", "8kHz", "16kHz",
];
const GROUPS: [(&str, f32, std::ops::Range<usize>); 6] = [
    ("SUBBASS", 119., 0..1),
    ("BASS", 235., 1..3),
    ("LOW_MIDS", 160., 3..5),
    ("MID_RANGE", 225., 5..7),
    ("UPPER_MID", 147., 7..8),
    ("TREBLE", 194., 8..10),
];

#[derive(Default)]
pub(super) struct State {
    hovered: Option<usize>,
    group: Option<usize>,
    hover_value: Option<i8>,
    point_hovered: bool,
    bubble: Option<(usize, i8)>,
    drag: Option<Drag>,
}

struct Drag {
    band: usize,
    setting: Setting,
    revision: u64,
}

/// Ip.checkXValue leaves gaps between its narrow hit columns.
fn band_at(x: f32) -> Option<usize> {
    (0..10).find(|&band| {
        let start = 92.55 * band as f32 - 2. + 17. * band as f32;
        (start..=start + 17.).contains(&x)
    })
}

/// The source deliberately uses 276 for hover and 291 for dragging, with 13 steps.
fn level_at(y: f32, dragging: bool) -> i8 {
    let height = if dragging { 291. } else { 276. };
    (6. - ((y - 8.) / (height / 13.)).round()).clamp(-6., 6.) as i8
}

fn point_at(bounds: Bounds<Pixels>, band: usize, level: f32) -> Point<Pixels> {
    bounds.origin
        + point(
            bounds.size.width * (9. + (WIDTH - 18.) * band as f32 / 9.) / WIDTH,
            bounds.size.height * (10. + (HEIGHT - 20.) * (6. - level) / 12.) / HEIGHT,
        )
}

impl AudioProductWorkspace {
    fn preview_earbuds_eq(&mut self, x: f32, y: f32, cx: &mut Context<Self>) {
        if self.earbuds_equalizer_disabled() {
            self.earbuds.eq_graph = State::default();
            return;
        }
        let setting = self.earbuds_equalizer_setting().ok();
        let graph = &mut self.earbuds.eq_graph;
        if let Some(drag) = &mut graph.drag {
            drag.setting.bands[drag.band] = level_at(y, true);
            drag.setting.preset = Preset::Custom;
            graph.bubble = Some((drag.band, drag.setting.bands[drag.band]));
        } else {
            graph.hovered = band_at(x);
            graph.group = None;
            graph.hover_value = Some(level_at(y, false));
            graph.point_hovered = graph.hovered.is_some_and(|band| {
                setting.is_some_and(|setting| {
                    let dx = x - (9. + (WIDTH - 18.) * band as f32 / 9.);
                    let dy =
                        y - (10. + (HEIGHT - 20.) * (6. - f32::from(setting.bands[band])) / 12.);
                    // Chart.js point radius 7 plus its default hitRadius 1.
                    dx * dx + dy * dy < 64.
                })
            });
            if graph.point_hovered {
                graph.bubble = graph
                    .hovered
                    .zip(setting)
                    .map(|(band, setting)| (band, setting.bands[band]));
            }
        }
        cx.notify();
    }

    fn begin_earbuds_eq_drag(&mut self, cx: &mut Context<Self>) {
        if self.earbuds_equalizer_disabled() {
            return;
        }
        let graph = &self.earbuds.eq_graph;
        let (Some(band), Some(level), Ok(mut setting)) = (
            graph.hovered,
            graph.hover_value,
            self.earbuds_equalizer_setting(),
        ) else {
            return;
        };
        if setting.bands[band] != level {
            setting.bands[band] = level;
            setting.preset = Preset::Custom;
        }
        self.earbuds.eq_graph.drag = Some(Drag {
            band,
            setting,
            revision: self.earbuds.eq_revision,
        });
        self.earbuds.eq_graph.point_hovered = true;
        self.earbuds.eq_graph.bubble = Some((band, setting.bands[band]));
        cx.notify();
    }

    fn finish_earbuds_eq_drag(
        &mut self,
        inside: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(mut drag) = self.earbuds.eq_graph.drag.take() else {
            return;
        };
        if !inside {
            self.earbuds.eq_graph.hovered = None;
            self.earbuds.eq_graph.point_hovered = false;
        }
        if self.page != "TAB_SOUND"
            || self.earbuds_equalizer_disabled()
            || self.earbuds.eq_revision != drag.revision
        {
            cx.notify();
            return;
        }
        // windowMouseUp sends custom even if the initial click kept the same level.
        drag.setting.preset = Preset::Custom;
        self.submit_earbuds_eq_curve(drag.setting, window, cx);
    }

    pub(super) fn submit_earbuds_eq_curve(
        &mut self,
        setting: Setting,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.earbuds_equalizer_disabled() || setting.validate().is_err() {
            return;
        }
        let Some(eq) = self.spec.equalizers.first() else {
            return;
        };
        let key = eq.key.clone();
        self.earbuds.eq_revision = self.earbuds.eq_revision.wrapping_add(1);
        let state = &mut self.draft["equalizers"][&key];
        state["selected"] = json!(setting.preset);
        state["bands"] = json!(setting.bands);
        state["presets"][setting.preset.key()] = json!(setting.bands);
        self.sync(window, cx);
        self.request_earbuds_equalizer(&key, cx);
        cx.notify();
    }

    fn zero_earbuds_eq(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Ok(mut setting) = self.earbuds_equalizer_setting() {
            setting.bands = [0; 10];
            setting.preset = Preset::Custom;
            self.earbuds.eq_graph = State::default();
            self.submit_earbuds_eq_curve(setting, window, cx);
        }
    }

    pub(super) fn earbuds_eq_chart(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let disabled = self.earbuds_equalizer_disabled();
        let graph = &self.earbuds.eq_graph;
        let setting = graph
            .drag
            .as_ref()
            .map(|drag| drag.setting)
            .or_else(|| self.earbuds_equalizer_setting().ok());
        let hovered = graph.hovered;
        let hovered_group = graph.group;
        let dragging = graph.drag.as_ref().map(|drag| drag.band);
        let point_hovered = graph.point_hovered;
        let animated_bands = setting.map(|setting| {
            std::array::from_fn::<_, 10, _>(|band| {
                motion::transition(
                    (ElementId::from(("earbuds-eq-band", band)), "level"),
                    f32::from(setting.bands[band]),
                    Transition::new(Duration::from_millis(150))
                        .ease(|progress| 1. - (1. - progress).powi(4)),
                    window,
                    cx,
                )
            })
        });
        let bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
        let measured = bounds.clone();
        let movement = bounds.clone();
        let owner = cx.entity().downgrade();
        let drawing = canvas(
            move |bounds, _, _| measured.set(bounds),
            move |bounds, _, window, _| {
                let scale = f32::from(bounds.size.width) / WIDTH;
                for band in 0..10 {
                    let highlighted = hovered == Some(band)
                        || (hovered.is_none()
                            && hovered_group.is_some_and(|group| GROUPS[group].2.contains(&band)));
                    let x = bounds.left()
                        + bounds.size.width * (9. + band as f32 * (1000. - 16.) / 9.) / WIDTH;
                    let mut path = PathBuilder::stroke(px(2. * scale));
                    path.move_to(point(x, bounds.top() + px(3. * scale)));
                    path.line_to(point(x, bounds.top() + px(279. * scale)));
                    if let Ok(path) = path.build() {
                        window.paint_path(
                            path,
                            if highlighted { C::selected() } else { C::column() },
                        );
                    }
                    window.paint_quad(quad(
                        Bounds::new(
                            point(x - px(3. * scale), bounds.top() + px(138. * scale)),
                            size(px(6. * scale), px(6. * scale)),
                        ),
                        px(3. * scale),
                        C::zero(),
                        px(0.),
                        C::zero(),
                        BorderStyle::Solid,
                    ));
                }
                if let Some(bands) = animated_bands {
                    let mut line = PathBuilder::stroke(px(2. * scale));
                    for (band, level) in bands.into_iter().enumerate() {
                        let point = point_at(bounds, band, level);
                        if band == 0 {
                            line.move_to(point);
                        } else {
                            line.line_to(point);
                        }
                    }
                    if let Ok(line) = line.build() {
                        window.paint_path(line, C::selected());
                    }
                    for (band, level) in bands.into_iter().enumerate() {
                        let center = point_at(bounds, band, level);
                        let color = if dragging == Some(band) {
                            C::pressed()
                        } else if hovered == Some(band) && point_hovered {
                            C::hover()
                        } else {
                            C::selected()
                        };
                        window.paint_quad(quad(
                            Bounds::new(
                                center - point(px(7. * scale), px(7. * scale)),
                                size(px(14. * scale), px(14. * scale)),
                            ),
                            px(7. * scale),
                            color,
                            px(2. * scale),
                            C::selected(),
                            BorderStyle::Solid,
                        ));
                    }
                }
                // GPUI owns these frame listeners. The page owns the drag and clears it on unmount.
                let release = owner.clone();
                window.on_mouse_event(move |event: &MouseUpEvent, phase, window, cx| {
                    if phase.capture() && event.button == MouseButton::Left {
                        let _ = release.update(cx, |this, cx| {
                            this.finish_earbuds_eq_drag(
                                bounds.contains(&event.position),
                                window,
                                cx,
                            )
                        });
                    }
                });
                window.on_mouse_event(move |event: &MouseMoveEvent, phase, _, cx| {
                    if !phase.capture() {
                        return;
                    }
                    let _ = owner.update(cx, |this, cx| {
                        if this.earbuds.eq_graph.drag.is_some() {
                            this.preview_earbuds_eq(
                                f32::from(event.position.x - bounds.left()) / scale,
                                f32::from(event.position.y - bounds.top()) / scale,
                                cx,
                            );
                        }
                    });
                });
            },
        )
        .size_full();
        let mut plot = div()
            .id("earbuds-eq-chart")
            .test_support()
            .relative()
            .flex_none()
            .ml(surface::css(50.))
            .mt(surface::css(15.))
            .w(surface::css(WIDTH))
            .h(surface::css(HEIGHT))
            .child(drawing)
            .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                let bounds = movement.get();
                if bounds.size.width <= px(0.) || this.earbuds.eq_graph.drag.is_some() {
                    return;
                }
                let scale = f32::from(bounds.size.width) / WIDTH;
                this.preview_earbuds_eq(
                    f32::from(event.position.x - bounds.left()) / scale,
                    f32::from(event.position.y - bounds.top()) / scale,
                    cx,
                );
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| this.begin_earbuds_eq_drag(cx)),
            )
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                if !*hovered && this.earbuds.eq_graph.drag.is_none() {
                    this.earbuds.eq_graph.hovered = None;
                    this.earbuds.eq_graph.point_hovered = false;
                    cx.notify();
                }
            }));
        let bubble_opacity = surface::fade_opacity(
            "earbuds-eq-bubble",
            if point_hovered { 1. } else { 0. },
            200,
            window,
            cx,
        );
        if bubble_opacity > 0.
            && let Some((band, level)) = graph.bubble
        {
            plot = plot.child(
                div()
                    .id("earbuds-eq-bubble")
                    .absolute()
                    .opacity(bubble_opacity)
                    .left(surface::css(109.55 * band as f32 - 32.))
                    .top(surface::css(291. * (6. - f32::from(level)) / 13.))
                    .w(surface::css(26.))
                    .h(surface::css(20.))
                    .rounded(surface::css(3.))
                    .bg(C::selected())
                    .text_color(C::bubble_text())
                    .text_size(surface::css(12.))
                    .line_height(surface::css(20.))
                    .text_center()
                    .child(if level > 0 {
                        format!("+{level}")
                    } else {
                        level.to_string()
                    }),
            );
        }
        plot = plot.children(FREQUENCIES.into_iter().enumerate().map(|(band, label)| {
            div()
                .absolute()
                .left(surface::css(1. + band as f32 * (1000. - 16.) / 9.))
                .top(surface::css(279.))
                .w(surface::css(16.))
                .text_center()
                .child(label)
        }));
        let groups = h_flex()
            .w(surface::css(1080.))
            .ml(surface::css(20.))
            .gap(surface::css(2.))
            .mt(surface::css(10.))
            .children(
                GROUPS
                    .iter()
                    .enumerate()
                    .map(|(group, (label, width, range))| {
                        let highlighted = graph.group == Some(group)
                            || graph.hovered.is_some_and(|band| range.contains(&band));
                        div()
                            .id(("earbuds-eq-group", group))
                            .w(surface::css(*width))
                            .p(surface::css(5.))
                            .bg(if highlighted { C::group_hover() } else { C::column() })
                            .text_color(if highlighted { C::selected() } else { C::text() })
                            .text_center()
                            .child(t(label).to_uppercase())
                            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                                if !this.earbuds_equalizer_disabled()
                                    && this.earbuds.eq_graph.drag.is_none()
                                {
                                    this.earbuds.eq_graph.group = hovered.then_some(group);
                                    this.earbuds.eq_graph.hovered = None;
                                    cx.notify();
                                }
                            }))
                    }),
            );
        let zero = SourceButton::new("earbuds-eq-zero")
            .disabled(disabled)
            .accessibility_label(t("RESET"))
            .group("earbuds-eq-zero")
            .size(surface::css(20.))
            .opacity(0.85)
            .child(
                div()
                    .relative()
                    .size_full()
                    .child(img("synapse/audio-mixer-eq-reset.svg").size_full())
                    .child(
                        img("synapse/audio-mixer-eq-reset-hover.svg")
                            .absolute()
                            .inset_0()
                            .size_full()
                            .invisible()
                            .group_hover("earbuds-eq-zero", |style| style.visible()),
                    )
                    .child(
                        img("synapse/audio-mixer-eq-reset-active.svg")
                            .absolute()
                            .inset_0()
                            .size_full()
                            .invisible()
                            .group_active("earbuds-eq-zero", |style| style.visible()),
                    ),
            )
            .on_click(cx.listener(|this, _, window, cx| this.zero_earbuds_eq(window, cx)));
        v_flex()
            .text_size(surface::css(12.))
            .text_color(C::text())
            .child(
                h_flex()
                    .items_start()
                    .child(plot)
                    .child(
                        v_flex()
                            .ml(surface::css(43.))
                            .h(surface::css(307.))
                            .mt(surface::css(5.))
                            .justify_between()
                            .children(
                                ["+6dB", "+3dB", "0dB", "-3dB", "-6dB"]
                                    .map(|label| div().child(label)),
                            ),
                    )
                    .child(
                        div()
                            .h(surface::css(297.))
                            .mt(surface::css(10.))
                            .flex()
                            .items_center()
                            .child(zero),
                    ),
            )
            .child(groups)
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::{band_at, level_at};

    #[test]
    fn source_columns_have_gaps_and_hover_and_drag_have_distinct_steps() {
        assert_eq!(band_at(-2.), Some(0));
        assert_eq!(band_at(15.), Some(0));
        assert_eq!(band_at(16.), None);
        assert_eq!(band_at(110.), Some(1));
        assert_eq!(band_at(990.), Some(9));
        assert_eq!(level_at(18.7, false), 5);
        assert_eq!(level_at(18.7, true), 6);
        assert_eq!(level_at(160., false), -1);
        assert_eq!(level_at(160., true), -1);
        assert_eq!(level_at(-100., false), 6);
        assert_eq!(level_at(900., true), -6);
    }
}
