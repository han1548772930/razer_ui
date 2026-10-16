//! Top-view Customize surfaces from independently mounted current source.
//! Each supported cycle branch retains its own source coordinates and artwork.
use super::*;
use crate::keyboard_geometry::KeyboardKeyExt as _;
use gpui_kit::base::Button as BaseButton;
use gpui_kit::base::ElementExt as _;
use gpui_kit::base::motion::Easing;
use razer_widgets::theme::MouseCustomizeColors as Colors;
use std::time::{Duration, Instant};
use std::{cell::Cell, rc::Rc};

fn physical_cover_contains(bounds: Bounds<Pixels>, position: Point<Pixels>, rounded: bool) -> bool {
    let width = f32::from(bounds.size.width);
    let height = f32::from(bounds.size.height);
    if width <= 0. || height <= 0. {
        return false;
    }
    let x = f32::from(position.x - bounds.left()) / width;
    let y = f32::from(position.y - bounds.top()) / height;
    if rounded {
        // The current 231 source mounts border-radius:50% native buttons.
        (x - 0.5).powi(2) + (y - 0.5).powi(2) <= 0.25
    } else {
        // 143's canvas circlePointCollision uses a strict 30x30 rectangle.
        x > 0. && x < 1. && y > 0. && y < 1.
    }
}

pub(super) struct AttachmentState {
    // A failed source getter publishes undefined. Keep it distinct from a
    // successful NoSidePadConnected status, whose numeric value is zero.
    status: Option<u32>,
    puzzle: u32,
    changed: bool,
    animation: Option<Instant>,
    light_from: f32,
    light_animation: Option<Instant>,
    epoch: u64,
    svg_current: u32,
    svg_transition: Option<u32>,
    svg_closing: bool,
    svg_animation: Option<Instant>,
    svg_target: u32,
    side_draw_repeat: bool,
}
impl Default for AttachmentState {
    fn default() -> Self {
        // These four current reducers start with sidepadLayout:3; the parent
        // mount calls checkEnum(3), replacing its constructor puzzleNumm:0.
        Self {
            status: Some(3),
            puzzle: 3,
            changed: false,
            animation: None,
            light_from: 0.,
            light_animation: None,
            epoch: 0,
            svg_current: 3,
            svg_transition: None,
            svg_closing: false,
            svg_animation: None,
            svg_target: 3,
            side_draw_repeat: false,
        }
    }
}
impl AttachmentState {
    fn connected(&self) -> bool {
        // The four raster parents use the strict source test 0 !== status.
        self.status != Some(0)
    }

    fn svg_module(status: Option<u32>) -> u32 {
        // 231's aA(undefined) starts at zero, unlike the raster parents that
        // retain their last positive puzzleNumm after a failed getter.
        match status {
            Some(4) => 2,
            Some(value) => value,
            None => 0,
        }
    }

    fn reset(&mut self) {
        let epoch = self.epoch.wrapping_add(1);
        *self = Self {
            epoch,
            ..Self::default()
        };
    }

    fn light_opacity(&self) -> f32 {
        let target = if self.connected() { 0. } else { 1. };
        let Some(start) = self.light_animation else {
            return target;
        };
        let delay = if self.connected() { 0.6 } else { 0. };
        let phase = ((start.elapsed().as_secs_f32() - delay) / 0.5).clamp(0., 1.);
        self.light_from + (target - self.light_from) * phase
    }
}

fn observed_sidepad_status(snapshot: &Value) -> Option<Option<u32>> {
    snapshot.get("sidepadLayout").and_then(|value| {
        if value.is_null() {
            Some(None)
        } else {
            value
                .as_u64()
                .and_then(|value| u32::try_from(value).ok())
                .map(Some)
        }
    })
}

#[cfg(test)]
mod runtime_artwork_tests {
    use super::{AttachmentState, observed_sidepad_status};
    use serde_json::json;
    use std::time::Instant;

    // The glob imports above also carry GPUI's `test` macro; name the
    // built-in attribute so this remains an ordinary Rust test.
    use std::prelude::v1::test;
    #[test]
    fn query_failure_keeps_unknown_distinct_from_zero_and_reducer_initial_value() {
        assert_eq!(observed_sidepad_status(&json!({})), None);
        assert_eq!(
            observed_sidepad_status(&json!({"sidepadLayout": null})),
            Some(None)
        );
        assert_eq!(
            observed_sidepad_status(&json!({"sidepadLayout": 0})),
            Some(Some(0))
        );
        assert_eq!(
            observed_sidepad_status(&json!({"sidepadLayout": 4})),
            Some(Some(4))
        );
        let mut state = AttachmentState::default();
        assert_eq!(state.status, Some(3));
        state.status = None;
        assert!(state.connected());
        assert_eq!(AttachmentState::svg_module(state.status), 0);
        state.status = Some(0);
        assert!(!state.connected());
        assert_eq!(AttachmentState::svg_module(Some(4)), 2);
    }

    #[test]
    fn scope_cleanup_invalidates_old_animation_and_restores_source_initial_state() {
        let mut state = AttachmentState {
            status: Some(1),
            puzzle: 1,
            changed: true,
            epoch: 7,
            animation: Some(Instant::now()),
            svg_transition: Some(1),
            svg_animation: Some(Instant::now()),
            ..AttachmentState::default()
        };
        state.reset();
        assert_eq!(state.epoch, 8);
        assert_eq!(state.status, Some(3));
        assert_eq!(state.puzzle, 3);
        assert!(!state.changed);
        assert!(state.animation.is_none());
        assert!(state.svg_transition.is_none());
        assert!(state.svg_animation.is_none());
    }
}

fn source_keyframes(values: &[f32; 5], phase: f32, easing: Easing) -> f32 {
    let phase = phase.clamp(0., 1.) * 4.;
    let index = (phase.floor() as usize).min(3);
    values[index] + (values[index + 1] - values[index]) * easing.sample(phase - index as f32)
}

#[derive(Deserialize)]
struct Surface {
    product_id: u32,
    #[serde(default)]
    group_index: usize,
    shapes: Vec<Shape>,
    controls: Vec<Control>,
    images: Vec<Artwork>,
    #[serde(default)]
    prefer_svg: bool,
    #[serde(default)]
    cycle: Option<Cycle>,
    #[serde(default)]
    cycle_images: Vec<Artwork>,
    #[serde(default)]
    image_top: f32,
    #[serde(default = "default_image_left")]
    image_left: f32,
    #[serde(default = "default_image_width")]
    image_width: f32,
    #[serde(default)]
    image_height: Option<f32>,
    #[serde(default)]
    edition_zero_fallback: bool,
    #[serde(default = "default_footer_margin_top")]
    footer_margin_top: f32,
    #[serde(default = "default_footer_margin_bottom")]
    footer_margin_bottom: f32,
    #[serde(default)]
    footer_armory_height: bool,
    #[serde(default)]
    separate_cycle_canvas: bool,
    #[serde(default)]
    cycle_canvas_when_active: bool,
    #[serde(default)]
    control_mode: Option<String>,
    #[serde(default = "default_line_y_start")]
    line_y_start: f32,
    #[serde(default)]
    attachment_layers: bool,
    #[serde(default)]
    attachment_images: Vec<Artwork>,
    #[serde(default)]
    attachment_svg: bool,
    #[serde(default)]
    thumbnails: Vec<Artwork>,
    #[serde(default)]
    thumbnail_check: Vec<Artwork>,
    #[serde(default)]
    shape_mode: Option<String>,
    #[serde(default)]
    circle_mapped_active: bool,
    #[serde(default)]
    circle_hover_fill: bool,
    #[serde(default)]
    label_top: f32,
    #[serde(default)]
    physical_circle_covers: Vec<CircleCover>,
    #[serde(default)]
    svg_button_foreground: Vec<Artwork>,
    #[serde(default)]
    svg_path_hits: Vec<SourcePathHit>,
}
fn default_line_y_start() -> f32 {
    16.
}
fn default_image_left() -> f32 {
    235.
}
fn default_image_width() -> f32 {
    300.
}
fn default_image_height() -> f32 {
    340.
}
fn default_footer_margin_top() -> f32 {
    20.
}
fn default_footer_margin_bottom() -> f32 {
    10.
}
#[derive(Clone, Deserialize)]
struct Shape {
    index: usize,
    x: f32,
    y: f32,
    left: bool,
    #[serde(default)]
    active_x: Option<f32>,
    #[serde(default)]
    active_y: Option<f32>,
    #[serde(default)]
    selected_cycle_x: Option<f32>,
    #[serde(default)]
    selected_cycle_y: Option<f32>,
    #[serde(default)]
    line_y: Option<f32>,
    #[serde(default)]
    target_dot_requires_cycle_trigger: bool,
    #[serde(default)]
    target_dot_requires_cycle_display: bool,
    #[serde(default)]
    hide_target_when_cycle: bool,
    #[serde(default)]
    cycle_line_length_adjust: f32,
    #[serde(default)]
    hover_index: Option<usize>,
    #[serde(default)]
    repeat_line_y: Option<f32>,
}
#[derive(Deserialize)]
struct Cycle {
    index: usize,
    left: f32,
    top: f32,
    initial: CyclePosition,
    hover: CyclePosition,
    leave: CyclePosition,
    leave_selected: CyclePosition,
    click_previous_selected: Option<CyclePosition>,
    deselect: Option<CyclePosition>,
    same_selected_update: Option<CyclePosition>,
    #[serde(default)]
    select_update: Option<CyclePosition>,
    #[serde(default)]
    render_idle: Option<CyclePosition>,
    #[serde(default)]
    render_active: Option<CyclePosition>,
    #[serde(default)]
    deselect_when_empty: bool,
    #[serde(default)]
    match_field: Option<String>,
    #[serde(default)]
    match_value: Option<String>,
    #[serde(default)]
    edition_zero_fallback: bool,
    #[serde(default)]
    visual_hover_index: Option<usize>,
    #[serde(default)]
    physical_cover: Option<PhysicalCover>,
    #[serde(default = "default_image_width")]
    width: f32,
    #[serde(default = "default_image_height")]
    height: f32,
    #[serde(default)]
    fixed_edition: Option<u32>,
}
#[derive(Deserialize)]
struct PhysicalCover {
    left: f32,
    top: f32,
    width: f32,
    height: f32,
}
#[derive(Deserialize)]
struct CircleCover {
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    index: usize,
    hover_index: usize,
    #[serde(default)]
    rounded: bool,
    #[serde(default)]
    require_enabled: bool,
}
#[derive(Clone, Copy, Deserialize)]
pub(super) struct CyclePosition {
    display: bool,
    x: f32,
    y: f32,
}
#[derive(Clone, Deserialize)]
struct Control {
    index: usize,
    #[serde(default)]
    button_index: Option<usize>,
    side: String,
    #[serde(default)]
    column_side: Option<String>,
    #[serde(default)]
    margin_top: f32,
}
#[derive(Deserialize)]
struct Artwork {
    edition: u32,
    request: String,
    asset: String,
}
#[derive(Deserialize)]
struct SourcePathHit {
    index: usize,
    left: f32,
    top: f32,
    width: f32,
    height: f32,
    require_enabled: bool,
    paint: bool,
    key: razer_assets::KeyboardKey,
    #[serde(default)]
    svg_state: Option<super::svg_button_motion::SvgButtonPresentation>,
}
fn source_surface(pid: u32, group_index: usize) -> Option<&'static Surface> {
    static SURFACES: OnceLock<Vec<Surface>> = OnceLock::new();
    SURFACES
        .get_or_init(|| {
            serde_json::from_str(include_str!("mouse_customize_current_data.json"))
                .expect("current mounted mouse geometry")
        })
        .iter()
        .find(|surface| surface.product_id == pid && surface.group_index == group_index)
}

impl MouseProductWorkspace {
    pub(super) fn customize_footer_margins(&self) -> (f32, f32) {
        source_surface(self.spec.product_id, self.group_ix)
            .map(|surface| (surface.footer_margin_top, surface.footer_margin_bottom))
            .unwrap_or((20., 10.))
    }

    fn current_cycle_selected(&self, input: Option<&str>) -> bool {
        let Some(cycle) = source_surface(self.spec.product_id, self.group_ix)
            .and_then(|surface| surface.cycle.as_ref())
        else {
            return false;
        };
        self.spec
            .groups
            .get(self.group_ix)
            .and_then(|group| group["buttonList"].as_array())
            .is_some_and(|buttons| {
                buttons.iter().any(|button| {
                    button["inputID"].as_str() == input
                        && input.is_some()
                        && cycle.match_value.as_deref().map_or_else(
                            || {
                                buttons.get(cycle.index).is_some_and(|cycle_button| {
                                    cycle_button["inputID"] == button["inputID"]
                                })
                            },
                            |value| {
                                button[cycle.match_field.as_deref().unwrap_or("buttonKey")].as_str()
                                    == Some(value)
                            },
                        )
                })
            })
    }

    /// The source updates retained cycle coordinates on events, including
    /// distinct deselection coordinates. Rendering never changes this state.
    pub(super) fn set_current_customize_input(&mut self, input: Option<String>, clicked: bool) {
        if input.is_none() {
            self.mapping_current_state.clear();
            // These current parents mount their popup only while activeButton
            // exists. Their search and menu children unmount on deselection.
            // 162's persistent component has a different lifecycle.
            if matches!(self.spec.product_id, 199 | 208) {
                self.mapping_menu_current_state = mapping_menu_current::State::default();
            }
        }
        if self.group_ix > 0 {
            self.customize_attachment.side_draw_repeat = true;
        }
        if let Some(cycle) = source_surface(self.spec.product_id, self.group_ix)
            .and_then(|surface| surface.cycle.as_ref())
        {
            let previous = self.current_cycle_selected(self.mapping_input.as_deref());
            let selected = self.current_cycle_selected(input.as_deref());
            let mut position = self.customize_cycle.unwrap_or(cycle.initial);
            if clicked && previous {
                if let Some(update) = cycle.click_previous_selected {
                    position = update;
                }
            }
            if input.is_none() && (previous || (cycle.deselect_when_empty && position.display)) {
                if let Some(update) = cycle.deselect {
                    position = update;
                }
            }
            if selected && !previous {
                if let Some(update) = cycle.select_update {
                    position = update;
                }
            }
            if previous && selected && !position.display {
                if let Some(update) = cycle.same_selected_update {
                    position = update;
                }
            }
            self.customize_cycle = Some(position);
        }
        self.mapping_input = input;
    }

    fn hover_current_customize(&mut self, index: usize, hovered: bool) {
        if self.group_ix > 0 {
            self.customize_attachment.side_draw_repeat = true;
        }
        if let Some(cycle) = source_surface(self.spec.product_id, self.group_ix)
            .and_then(|surface| surface.cycle.as_ref())
        {
            if hovered && index == cycle.index {
                self.customize_cycle = Some(cycle.hover);
            }
            if !hovered {
                self.customize_cycle = Some(
                    if self.current_cycle_selected(self.mapping_input.as_deref()) {
                        cycle.leave_selected
                    } else {
                        cycle.leave
                    },
                );
            }
        }
        self.customize_hover = hovered.then_some(index);
    }

    /// Actual device/customize observation; edition zero is the source reducer's
    /// initial value. Unknown editions retain an absent image rather than using
    /// another product color or a fabricated hardware observation.
    pub fn observe_mouse_customize(&mut self, snapshot: &Value, cx: &mut Context<Self>) {
        if source_surface(self.spec.product_id, 0)
            .is_some_and(|surface| surface.attachment_layers || surface.attachment_svg)
            && let Some(status) = observed_sidepad_status(snapshot)
            && status != self.customize_attachment.status
        {
            let light_from = self.customize_attachment.light_opacity();
            let prior_connected = self.customize_attachment.connected();
            let connected = status != Some(0);
            let state = &mut self.customize_attachment;
            let svg = source_surface(self.spec.product_id, 0)
                .is_some_and(|surface| surface.attachment_svg);
            if svg {
                let previous = AttachmentState::svg_module(state.status);
                let next = AttachmentState::svg_module(status);
                if previous != next && next != state.svg_current {
                    state.svg_closing = next == 0;
                    state.svg_current = if next == 0 { next } else { previous };
                    state.svg_transition = Some(if next == 0 { previous } else { next });
                    state.svg_target = next;
                    state.svg_animation = Some(Instant::now());
                }
            }
            if prior_connected != connected {
                state.light_from = light_from;
                state.light_animation = Some(Instant::now());
            }
            // Source class changes restart animations; changing one nonzero
            // panel to another only replaces src, retaining the same class.
            if !state.changed || prior_connected != connected {
                state.animation = Some(Instant::now());
            }
            state.status = status;
            if svg {
                state.puzzle = AttachmentState::svg_module(status);
            } else if let Some(status) = status.filter(|status| *status > 0) {
                state.puzzle = status;
            }
            state.changed = true;
            state.epoch = state.epoch.wrapping_add(1);
            let epoch = state.epoch;
            cx.spawn(async move |view, cx| {
                let start = Instant::now();
                while start.elapsed() < Duration::from_millis(1150) {
                    cx.background_executor()
                        .timer(Duration::from_millis(16))
                        .await;
                    let running = view.update(cx, |view, cx| {
                        if view.customize_attachment.epoch != epoch {
                            return false;
                        }
                        if !view.active || view.page != "TAB_CUSTOMIZE" || view.group_ix != 0 {
                            let state = &mut view.customize_attachment;
                            state.changed = false;
                            state.animation = None;
                            state.light_animation = None;
                            state.svg_current = AttachmentState::svg_module(state.status);
                            state.svg_transition = None;
                            state.svg_animation = None;
                            return false;
                        }
                        let state = &mut view.customize_attachment;
                        if svg
                            && state.svg_animation.is_some_and(|start| {
                                start.elapsed()
                                    >= Duration::from_millis(if state.svg_closing {
                                        900
                                    } else {
                                        250
                                    })
                            })
                        {
                            state.svg_current = state.svg_target;
                            state.svg_transition = None;
                            state.svg_animation = None;
                        }
                        cx.notify();
                        true
                    });
                    if !matches!(running, Ok(true)) {
                        break;
                    }
                }
            })
            .detach();
        }
        if let (Some(target), Some(fields)) = (
            self.mouse_customize_observation.as_object_mut(),
            snapshot.as_object(),
        ) {
            target.extend(
                fields
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone())),
            );
        }
        self.observe_mouse_runtime_lighting(snapshot);
        self.sync_receiver_widget_mount(cx);
        cx.notify();
    }

    /// Scope teardown is the source component unmount, separate from a failed
    /// getter. Cancel old animation work and restore source reducer defaults.
    pub fn reset_mouse_customize_runtime(&mut self, cx: &mut Context<Self>) {
        self.mouse_customize_observation = json!({});
        self.customize_attachment.reset();
        self.reset_mouse_runtime_lighting();
        self.sync_receiver_widget_mount(cx);
        cx.notify();
    }

    pub(super) fn customize_current_surface(
        &self,
        buttons: &[Value],
        unavailable: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let surface = source_surface(self.spec.product_id, self.group_ix)?;
        if surface
            .shapes
            .iter()
            .any(|shape| shape.index >= buttons.len())
        {
            return None;
        }
        let edition = self.mouse_customize_observation["editionId"]
            .as_u64()
            .unwrap_or(0) as u32;
        let image = surface
            .images
            .iter()
            .filter(|image| {
                image.edition == edition && (!surface.prefer_svg || image.request.ends_with(".svg"))
            })
            .max_by_key(|image| if image.request.contains("3x") { 2 } else { 1 })
            .or_else(|| {
                surface
                    .edition_zero_fallback
                    .then(|| {
                        surface
                            .images
                            .iter()
                            .filter(|image| {
                                image.edition == 0
                                    && (!surface.prefer_svg || image.request.ends_with(".svg"))
                            })
                            .max_by_key(|image| if image.request.contains("3x") { 2 } else { 1 })
                    })
                    .flatten()
            });
        let selected = buttons
            .iter()
            .position(|button| button["inputID"].as_str() == self.mapping_input.as_deref());
        let hovered = self.customize_hover;
        let cycle_position = surface.cycle.as_ref().map(|cycle| {
            let position = self.customize_cycle.unwrap_or(cycle.initial);
            if position.display {
                cycle.render_active.unwrap_or(position)
            } else {
                cycle.render_idle.unwrap_or(position)
            }
        });
        let cycle_active = cycle_position.is_some_and(|position| position.display)
            || surface
                .cycle
                .as_ref()
                .and_then(|cycle| cycle.visual_hover_index)
                .is_some_and(|index| self.customize_hover == Some(index));
        let cycle_index = surface.cycle.as_ref().map(|cycle| cycle.index);
        let cycle_edition = surface
            .cycle
            .as_ref()
            .and_then(|cycle| cycle.fixed_edition)
            .unwrap_or(edition);
        let cycle_image = surface
            .cycle_images
            .iter()
            .filter(|image| image.edition == cycle_edition)
            .max_by_key(|image| if image.request.contains("3x") { 2 } else { 1 })
            .or_else(|| {
                surface
                    .cycle
                    .as_ref()
                    .filter(|cycle| cycle.edition_zero_fallback)
                    .and_then(|_| {
                        surface
                            .cycle_images
                            .iter()
                            .filter(|image| image.edition == 0)
                            .max_by_key(|image| if image.request.contains("3x") { 2 } else { 1 })
                    })
            });
        let cycle_selected = self.current_cycle_selected(self.mapping_input.as_deref());
        let hyper = self.hypershift;
        let enabled = buttons
            .iter()
            .map(|button| !unavailable && button["isEnabled"].as_bool().unwrap_or(false))
            .collect::<Vec<_>>();
        let line_y_start = surface.line_y_start;
        let circle_mode = surface.shape_mode.as_deref() == Some("circle");
        let repeated = self.customize_attachment.side_draw_repeat;
        let create_lines = |shapes: Vec<Shape>| {
            let enabled = enabled.clone();
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    let scale = f32::from(window.rem_size()) / 16.;
                    let point_at =
                        |x: f32, y: f32| bounds.origin + point(px(x * scale), px(y * scale));
                    let dot = |x: f32, y: f32, color: Hsla, window: &mut Window| {
                        window.paint_quad(PaintQuad {
                            corner_radii: Corners::all(px(2.5 * scale)),
                            ..fill(
                                Bounds::new(
                                    point_at(x - 2.5, y - 2.5),
                                    size(px(5. * scale), px(5. * scale)),
                                ),
                                color,
                            )
                        });
                    };
                    let mut left_row = 0;
                    let mut right_row = 0;
                    for shape in &shapes {
                        if circle_mode {
                            continue;
                        }
                        let row = if shape.left {
                            let row = left_row;
                            left_row += 1;
                            row
                        } else {
                            let row = right_row;
                            right_row += 1;
                            row
                        };
                        let x = if shape.left { 3. } else { 307. };
                        let y = if repeated {
                            shape.repeat_line_y.or(shape.line_y)
                        } else {
                            shape.line_y
                        }
                        .unwrap_or(line_y_start + row as f32 * 50.);
                        let active = selected == Some(shape.index)
                            || hovered == Some(shape.hover_index.unwrap_or(shape.index));
                        let (target_x, target_y) = if cycle_index == Some(shape.index) {
                            cycle_position
                                .map(|position| (position.x, position.y))
                                .unwrap_or((shape.x, shape.y))
                        } else if cycle_selected && shape.selected_cycle_x.is_some() {
                            (
                                shape.selected_cycle_x.unwrap_or(shape.x),
                                shape.selected_cycle_y.unwrap_or(shape.y),
                            )
                        } else {
                            (
                                if active {
                                    shape.active_x.unwrap_or(shape.x)
                                } else {
                                    shape.x
                                },
                                if active {
                                    shape.active_y.unwrap_or(shape.y)
                                } else {
                                    shape.y
                                },
                            )
                        };
                        let target_x = target_x + 7.5;
                        let target_y = target_y + 2.5;
                        let color = if !enabled[shape.index] {
                            Colors::disabled_line()
                        } else if active {
                            if hyper {
                                Colors::hypershift()
                            } else {
                                Colors::selected()
                            }
                        } else {
                            Colors::line()
                        };
                        let mut path = PathBuilder::stroke(px(scale));
                        path.move_to(point_at(x, y));
                        path.line_to(point_at(
                            x + if shape.left { 53. } else { -53. }
                                + if cycle_active {
                                    shape.cycle_line_length_adjust
                                } else {
                                    0.
                                },
                            y,
                        ));
                        if !(shape.hide_target_when_cycle && cycle_active) {
                            path.line_to(point_at(target_x, target_y));
                        }
                        if let Ok(path) = path.build() {
                            window.paint_path(path, color);
                        }
                        dot(x, y, color, window);
                        if !(shape.hide_target_when_cycle && cycle_active)
                            && (!shape.target_dot_requires_cycle_display || cycle_active)
                            && (!shape.target_dot_requires_cycle_trigger
                                || hovered == Some(shape.index)
                                || cycle_selected)
                        {
                            dot(target_x, target_y, color, window);
                        }
                        if active {
                            let mut ring = PathBuilder::stroke(px(scale));
                            for step in 0..=32 {
                                let angle = step as f32 / 32. * std::f32::consts::TAU;
                                let position = point_at(
                                    target_x + angle.cos() * 5.5,
                                    target_y + angle.sin() * 5.5,
                                );
                                if step == 0 {
                                    ring.move_to(position);
                                } else {
                                    ring.line_to(position);
                                }
                            }
                            if let Ok(ring) = ring.build() {
                                window.paint_path(ring, color);
                            }
                        }
                    }
                },
            )
            .absolute()
            .left(surface::css(230.))
            .w(surface::css(310.))
            .h(surface::css(340.))
        };
        let separate_cycle_canvas =
            surface.separate_cycle_canvas && (!surface.cycle_canvas_when_active || cycle_active);
        let mut base_lines = Some(create_lines(
            surface
                .shapes
                .iter()
                .filter(|shape| !separate_cycle_canvas || cycle_index != Some(shape.index))
                .cloned()
                .collect(),
        ));
        let cycle_lines = separate_cycle_canvas.then(|| {
            create_lines(
                surface
                    .shapes
                    .iter()
                    .filter(|shape| cycle_index == Some(shape.index))
                    .cloned()
                    .collect(),
            )
        });
        let block = div()
            .relative()
            .w(surface::css(770.))
            .h(surface::css(340.))
            .mx_auto()
            .children(self.customize_attachment_layers(surface, edition, false))
            .children(image.map(|image| {
                img(SharedString::from(image.asset.clone()))
                    .absolute()
                    .left(surface::css(surface.image_left))
                    .top(surface::css(surface.image_top))
                    .w(surface::css(surface.image_width))
                    .when_some(surface.image_height, |image, height| {
                        image.h(surface::css(height))
                    })
                    .when(surface.image_height.is_none(), |image| {
                        image.max_h(surface::css(340.))
                    })
                    .object_fit(ObjectFit::Contain)
            }))
            .children(self.customize_attachment_layers(surface, edition, true))
            .children(if separate_cycle_canvas {
                base_lines.take()
            } else {
                None
            })
            .when(cycle_active, |block| {
                block.children(
                    surface
                        .cycle
                        .as_ref()
                        .zip(cycle_image)
                        .map(|(cycle, image)| {
                            img(SharedString::from(image.asset.clone()))
                                .absolute()
                                .left(surface::css(cycle.left))
                                .top(surface::css(cycle.top))
                                .w(surface::css(cycle.width))
                                .h(surface::css(cycle.height))
                                .object_fit(ObjectFit::Contain)
                        }),
                )
            })
            .children(if separate_cycle_canvas {
                cycle_lines
            } else {
                base_lines.take()
            })
            .children(self.customize_source_circles(surface, buttons, cx))
            .children(self.customize_source_paths(surface, buttons, window, cx))
            .children(surface.svg_button_foreground.iter().map(|image| {
                img(SharedString::from(image.asset.clone()))
                    .absolute()
                    .left(surface::css(surface.image_left))
                    .top(surface::css(surface.image_top))
                    .w(surface::css(surface.image_width))
                    .h(surface::css(surface.image_height.unwrap_or(340.)))
                    .object_fit(ObjectFit::Contain)
            }))
            .children(surface.physical_circle_covers.iter().filter_map(|cover| {
                let button = buttons.get(cover.index)?;
                if cover.require_enabled
                    && (button["isEnabled"] == false || button["disabled"] == true)
                {
                    return None;
                }
                let input = buttons.get(cover.index)?["inputID"].as_str()?.to_owned();
                let hover_index = cover.hover_index;
                let rounded = cover.rounded;
                let bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
                let measured = bounds.clone();
                let moved = bounds.clone();
                let entered = bounds.clone();
                Some(
                    div()
                        .id(SharedString::from(format!(
                            "mouse-hidpi-source-circle-{}",
                            cover.index
                        )))
                        .absolute()
                        .left(surface::css(cover.left))
                        .top(surface::css(cover.top))
                        .w(surface::css(cover.width))
                        .h(surface::css(cover.height))
                        .when(cover.rounded, |cover| cover.rounded_full())
                        .on_prepaint(move |area, _, _| measured.set(area))
                        .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                            let inside =
                                physical_cover_contains(moved.get(), event.position, rounded);
                            if inside {
                                this.hover_current_customize(hover_index, true);
                            } else if this.customize_hover == Some(hover_index) {
                                this.hover_current_customize(hover_index, false);
                            }
                            cx.notify();
                        }))
                        .on_hover(cx.listener(move |this, hovered: &bool, window, cx| {
                            let inside = *hovered
                                && physical_cover_contains(
                                    entered.get(),
                                    window.mouse_position(),
                                    rounded,
                                );
                            if inside {
                                this.hover_current_customize(hover_index, true);
                            } else if this.customize_hover == Some(hover_index) {
                                this.hover_current_customize(hover_index, false);
                            }
                            cx.notify();
                        }))
                        .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                            let accepts = match event {
                                ClickEvent::Keyboard(_) => rounded,
                                ClickEvent::Mouse(event) => {
                                    physical_cover_contains(
                                        bounds.get(),
                                        event.down.position,
                                        rounded,
                                    ) && physical_cover_contains(
                                        bounds.get(),
                                        event.up.position,
                                        rounded,
                                    )
                                }
                                ClickEvent::Touch(event) => {
                                    physical_cover_contains(bounds.get(), event.position, rounded)
                                }
                            };
                            if !accepts {
                                return;
                            }
                            this.open_current_mouse_mapping(input.clone(), window, cx);
                        })),
                )
            }))
            .child(self.customize_current_labels(surface, buttons, true, unavailable, cx))
            .child(self.customize_current_labels(surface, buttons, false, unavailable, cx))
            .children(surface.cycle.as_ref().and_then(|cycle| {
                cycle.physical_cover.as_ref().map(|cover| {
                    let index = cycle.index;
                    div()
                        .id("mouse-current-physical-cycle-cover")
                        .absolute()
                        .left(surface::css(cover.left))
                        .top(surface::css(cover.top))
                        .w(surface::css(cover.width))
                        .h(surface::css(cover.height))
                        .rounded_full()
                        .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                            this.hover_current_customize(index, *hovered);
                            cx.notify();
                        }))
                })
            }));
        let content = div()
            .w_full()
            .child(
                div()
                    .relative()
                    .w_full()
                    .min_w(surface::css(770.))
                    .max_w(surface::css(1220.))
                    .mx_auto()
                    .h(surface::css(340.))
                    .child(surface::dot_background(cx))
                    .child(block),
            )
            .child(self.customize_source_thumbnails(surface, cx))
            .child(
                h_flex()
                    .items_center()
                    .justify_center()
                    .mt(surface::css(surface.footer_margin_top))
                    .mb(surface::css(surface.footer_margin_bottom))
                    .when(
                        surface.footer_armory_height
                            && self.mouse_customize_observation["displayMode"] == "armory",
                        |row| row.h(surface::css(140.)),
                    )
                    .text_size(surface::css(14.))
                    .line_height(surface::css(14.))
                    .children(
                        (self.mouse_customize_observation["displayMode"] != "macro").then(|| {
                            div()
                                .mx(surface::css(5.))
                                .flex_shrink_0()
                                .child(self.current_sidebar_toggle(cx))
                        }),
                    )
                    .child(
                        div()
                            .mx(surface::css(5.))
                            .flex_shrink_0()
                            .child(self.customize_current_hypershift(cx)),
                    )
                    .child(div().mx(surface::css(5.)).flex_shrink_0().child(
                        surface::hypershift_help_control_for_product(
                            self.spec.product_id,
                            "mouse-current-hypershift-help",
                            t("HYPERSHIFT_TOOLTIP"),
                        ),
                    )),
            );
        let content = content.child(self.receiver_indicator_panel(cx));
        Some(self.current_sidebar_surface(buttons, unavailable, content, window, cx))
    }

    fn customize_source_paths(
        &self,
        surface: &'static Surface,
        buttons: &[Value],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        surface
            .svg_path_hits
            .iter()
            .filter_map(|hit| {
                let button = buttons.get(hit.index)?;
                let hover_allowed = !hit.require_enabled || button["isEnabled"] != false;
                if !hover_allowed && hit.svg_state.is_none() {
                    return None;
                }
                let click_allowed =
                    hover_allowed && (!hit.require_enabled || button["disabled"] != true);
                let input = button["inputID"].as_str()?.to_owned();
                let index = hit.index;
                let selected = button["isFocused"] == true
                    || (self.mapping_input.is_some()
                        && self.mapping_input.as_deref() == Some(input.as_str()));
                let hovered = self.customize_hover == Some(index);
                let disabled = button["isMapped"] == true && button["assignmentValue"] == "DISABLE";
                let color = if self.hypershift {
                    Colors::hypershift()
                } else {
                    Colors::selected()
                };
                let fill = if !hit.paint {
                    Colors::transparent()
                } else if selected {
                    color.opacity(0.5)
                } else if disabled {
                    Hsla::from(rgb(0xc8323c)).opacity(0.5)
                } else {
                    Colors::transparent()
                };
                let stroke = if hit.paint && (selected || hovered) {
                    color.opacity(if selected || disabled { 0.5 } else { 1. })
                } else {
                    Colors::transparent()
                };
                let svg_paint = hit.svg_state.as_ref().map(|presentation| {
                    let owner = ElementId::from(("mouse-source-svg", cx.entity_id()));
                    let id = ElementId::NamedChild(
                        owner.into(),
                        SharedString::from(format!(
                            "{}-{}-{input}",
                            self.spec.product_id, self.group_ix
                        )),
                    );
                    presentation.sample(
                        id,
                        selected,
                        hovered,
                        self.hypershift,
                        button["isEnabled"] == false,
                        window,
                        cx,
                    )
                });
                let key = &hit.key;
                let bounds = Rc::new(Cell::new(Bounds::<Pixels>::default()));
                let measured = bounds.clone();
                let moved = bounds.clone();
                let entered = bounds.clone();
                Some(
                    div()
                        .id(SharedString::from(key.id.clone()))
                        .absolute()
                        .left(surface::css(hit.left))
                        .top(surface::css(hit.top))
                        .w(surface::css(hit.width))
                        .h(surface::css(hit.height))
                        .opacity(svg_paint.as_ref().map_or(1., |paint| paint.opacity()))
                        .on_prepaint(move |area, _, _| measured.set(area))
                        .when(hover_allowed, |path| {
                            path.on_mouse_move(cx.listener(
                                move |this, event: &MouseMoveEvent, _, cx| {
                                    let inside =
                                        key.contains_window_point(event.position, moved.get());
                                    if inside {
                                        if this.customize_hover != Some(index) {
                                            this.hover_current_customize(index, true);
                                            cx.notify();
                                        }
                                    } else if this.customize_hover == Some(index) {
                                        this.hover_current_customize(index, false);
                                        cx.notify();
                                    }
                                },
                            ))
                            .on_hover(cx.listener(move |this, inside: &bool, window, cx| {
                                let inside = *inside
                                    && key.contains_window_point(
                                        window.mouse_position(),
                                        entered.get(),
                                    );
                                if inside {
                                    this.hover_current_customize(index, true);
                                    cx.notify();
                                } else if this.customize_hover == Some(index) {
                                    this.hover_current_customize(index, false);
                                    cx.notify();
                                }
                            }))
                            .on_click(cx.listener(
                                move |this, event: &ClickEvent, window, cx| {
                                    if !click_allowed
                                        || matches!(event, ClickEvent::Keyboard(_))
                                        || !key.accepts_click(event, bounds.get())
                                    {
                                        return;
                                    }
                                    this.open_current_mouse_mapping(input.clone(), window, cx);
                                    cx.stop_propagation();
                                    cx.notify();
                                },
                            ))
                        })
                        .child(
                            canvas(
                                |_, _, _| (),
                                move |area, _, window, _| {
                                    if let Some(paint) = &svg_paint {
                                        paint.paint(key, area, window);
                                        return;
                                    }
                                    if fill.a > 0.
                                        && let Ok(path) = key.path(area, false).build()
                                    {
                                        window.paint_path(path, fill);
                                    }
                                    if stroke.a > 0.
                                        && let Ok(path) = key.path(area, true).build()
                                    {
                                        window.paint_path(path, stroke);
                                    }
                                },
                            )
                            .absolute()
                            .size_full(),
                        )
                        .into_any_element(),
                )
            })
            .collect()
    }

    fn customize_source_thumbnails(&self, surface: &Surface, cx: &Context<Self>) -> AnyElement {
        h_flex()
            .w_full()
            .justify_center()
            .children(surface.thumbnails.iter().filter_map(|image| {
                let name = image.request.rsplit('/').next()?;
                let index = name
                    .strip_prefix("thumbview-")?
                    .strip_suffix(".png")?
                    .parse::<usize>()
                    .ok()?;
                let selected = self.group_ix == index;
                // A missing field uses the initial device reducer; an explicit
                // null is the getter's undefined payload and has no plug mark.
                let status =
                    observed_sidepad_status(&self.mouse_customize_observation).unwrap_or(Some(3));
                let plugged = match self.spec.product_id {
                    103 | 147 => {
                        matches!((status, index), (Some(1), 1) | (Some(2), 2) | (Some(3), 3))
                    }
                    143 | 167 | 231 => {
                        matches!((status, index), (Some(1), 1) | (Some(4), 2) | (Some(3), 3))
                    }
                    _ => false,
                };
                Some(
                    div()
                        .id(SharedString::from(format!("mouse-source-view-{index}")))
                        .relative()
                        .w(surface::css(50.))
                        .h(surface::css(50.))
                        .mx(surface::css(5.))
                        .border_2()
                        .border_color(if selected {
                            if self.hypershift {
                                Colors::hypershift()
                            } else {
                                Colors::selected()
                            }
                        } else {
                            rgb(0x5d5d5d).into()
                        })
                        .hover(|style| style.bg(rgba(0xffffff1a)))
                        .child(
                            img(SharedString::from(image.asset.clone()))
                                .size_full()
                                .object_fit(ObjectFit::Cover),
                        )
                        .children(surface.thumbnail_check.first().filter(|_| plugged).map(
                            |image| {
                                img(SharedString::from(image.asset.clone()))
                                    .absolute()
                                    .right(surface::css(3.))
                                    .bottom(surface::css(3.))
                                    .w(surface::css(16.))
                                    .h(surface::css(16.))
                            },
                        ))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if this.group_ix == index {
                                return;
                            }
                            this.set_current_customize_input(None, false);
                            this.group_ix = index;
                            this.customize_hover = None;
                            this.customize_cycle = None;
                            this.customize_attachment.side_draw_repeat = false;
                            this.mapping_assignment = None;
                            cx.notify();
                        })),
                )
            }))
            .into_any_element()
    }

    fn customize_source_circles(
        &self,
        surface: &Surface,
        buttons: &[Value],
        _cx: &Context<Self>,
    ) -> Vec<AnyElement> {
        if surface.shape_mode.as_deref() != Some("circle") {
            return Vec::new();
        }
        surface
            .shapes
            .iter()
            .map(|shape| {
                let button = &buttons[shape.index];
                let hovered = self.customize_hover == Some(shape.index);
                let selected = self.mapping_input.as_deref() == button["inputID"].as_str()
                    && self.mapping_input.is_some();
                let active = selected
                    || hovered
                    || (surface.circle_mapped_active && button["isMapped"] == true);
                let color = if active {
                    if self.hypershift {
                        Colors::hypershift()
                    } else {
                        Colors::selected()
                    }
                } else {
                    Colors::line()
                };
                div()
                    .absolute()
                    .left(surface::css(230. + shape.x - 1.))
                    .top(surface::css(shape.y - 1.))
                    .w(surface::css(32.))
                    .h(surface::css(32.))
                    .border_2()
                    .border_color(color)
                    .rounded_full()
                    .bg(rgb(if hovered && surface.circle_hover_fill {
                        0x292929
                    } else {
                        0x111111
                    }))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(rgb(0xcccccc))
                    .text_size(surface::css(12.))
                    .child(
                        button["counter"]
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| {
                                button["counter"]
                                    .as_i64()
                                    .map(|value| value.to_string())
                                    .unwrap_or_default()
                            }),
                    )
                    .into_any_element()
            })
            .collect()
    }

    fn customize_attachment_layers(
        &self,
        surface: &Surface,
        edition: u32,
        foreground: bool,
    ) -> Vec<AnyElement> {
        if surface.attachment_svg {
            return if foreground {
                self.customize_svg_attachment(surface)
            } else {
                Vec::new()
            };
        }
        if !surface.attachment_layers {
            return Vec::new();
        }
        let state = &self.customize_attachment;
        let elapsed = state
            .animation
            .map(|start| start.elapsed().as_secs_f32())
            .unwrap_or(0.);
        let connected = state.connected();
        let button_opacity = if state.changed {
            if connected {
                (elapsed / 1.).clamp(0., 1.)
            } else {
                let phase = (elapsed / 0.7).clamp(0., 1.);
                let (from, to, local) = if phase < 0.25 {
                    (1., 0.5, phase / 0.25)
                } else if phase < 0.5 {
                    (0.5, 0.75, (phase - 0.25) / 0.25)
                } else {
                    (0.75, 0., (phase - 0.5) / 0.5)
                };
                from + (to - from) * Easing::EaseInOut.sample(local)
            }
        } else if connected {
            1.
        } else {
            0.
        };
        let (quarter_opacity, quarter_left) = if elapsed < 0.25 {
            let phase = (elapsed / 0.25).clamp(0., 1.);
            if connected {
                (
                    source_keyframes(&[0.1, 0.3, 0.75, 1., 1.], phase, Easing::EaseIn),
                    source_keyframes(&[-25., -15., -10., -3., 0.], phase, Easing::EaseIn),
                )
            } else {
                (
                    source_keyframes(&[0., 0.25, 0.5, 0.75, 1.], phase, Easing::EaseIn),
                    0.,
                )
            }
        } else if connected {
            (
                source_keyframes(
                    &[1., 1., 0.5, 0.3, 0.],
                    ((elapsed - 0.25) / 0.6).clamp(0., 1.),
                    Easing::EaseOut,
                ),
                0.,
            )
        } else {
            let phase = ((elapsed - 0.25) / 0.65).clamp(0., 1.);
            (
                source_keyframes(&[1., 1., 1., 1., 0.], phase, Easing::EaseOut),
                source_keyframes(&[0., 0., 0., -30., -30.], phase, Easing::EaseOut),
            )
        };
        let stems = if foreground {
            vec![("light-off".to_owned(), state.light_opacity(), 0.)]
        } else {
            // z0 layer, z1 sidepad, z2 transient quarter. The main z2 image
            // follows this group, matching its later position in source DOM.
            let mut layers = vec![
                ("layer".to_owned(), 1., 0.),
                (format!("button{}", state.puzzle), button_opacity, 0.),
            ];
            if state.changed {
                layers.push((
                    format!("quarterFade-{}", state.puzzle),
                    quarter_opacity,
                    quarter_left,
                ));
            }
            layers
        };
        stems
            .into_iter()
            .filter_map(|(stem, opacity, left)| {
                let suffix = format!("/{stem}-3x.png");
                let fallback = format!("/{stem}-1x.png");
                let image = surface
                    .attachment_images
                    .iter()
                    .find(|image| image.edition == edition && image.request.ends_with(&suffix))
                    .or_else(|| {
                        surface.attachment_images.iter().find(|image| {
                            image.edition == edition && image.request.ends_with(&fallback)
                        })
                    })?;
                Some(
                    div()
                        .absolute()
                        .left(surface::css(surface.image_left + left))
                        .top(surface::css(surface.image_top))
                        .w(surface::css(300.))
                        .h(surface::css(340.))
                        .opacity(opacity)
                        .child(
                            img(SharedString::from(image.asset.clone()))
                                .size_full()
                                .object_fit(ObjectFit::Contain),
                        )
                        .into_any_element(),
                )
            })
            .collect()
    }

    fn customize_svg_attachment(&self, surface: &Surface) -> Vec<AnyElement> {
        let state = &self.customize_attachment;
        let (module, opacity, left) = if let Some(module) = state.svg_transition {
            let elapsed = state
                .svg_animation
                .map(|start| start.elapsed().as_secs_f32())
                .unwrap_or(0.);
            if state.svg_closing {
                let phase = ((elapsed - 0.25) / 0.65).clamp(0., 1.);
                (
                    module,
                    source_keyframes(&[1., 1., 1., 1., 0.], phase, Easing::EaseOut),
                    source_keyframes(&[0., 0., 0., -30., -30.], phase, Easing::EaseOut),
                )
            } else {
                let phase = (elapsed / 0.25).clamp(0., 1.);
                (
                    module,
                    source_keyframes(&[0.1, 0.3, 0.75, 1., 1.], phase, Easing::EaseIn),
                    source_keyframes(&[-25., -15., -10., -3., 0.], phase, Easing::EaseIn),
                )
            }
        } else {
            (state.svg_current, 1., 0.)
        };
        let keys = match module {
            1 => 2,
            2 => 6,
            3 => 12,
            _ => return Vec::new(),
        };
        let stem = format!("/side-module-{keys}.");
        let Some(image) = surface
            .attachment_images
            .iter()
            .find(|image| image.edition == 0 && image.request.contains(&stem))
        else {
            return Vec::new();
        };
        vec![
            div()
                .absolute()
                .left(surface::css(surface.image_left + left))
                .top(surface::css(surface.image_top))
                .w(surface::css(200.))
                .h(surface::css(320.))
                .opacity(opacity)
                .child(img(SharedString::from(image.asset.clone())).size_full())
                .into_any_element(),
        ]
    }

    fn customize_current_labels(
        &self,
        surface: &Surface,
        buttons: &[Value],
        left: bool,
        unavailable: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        // 211 mounts filtered button props in original array order. Its idx is
        // the combined label position, while button_index owns the real action.
        let controls = if surface.control_mode.as_deref() == Some("button_pos") {
            let left_count = buttons
                .iter()
                .filter(|button| button["buttonPos"] == "left")
                .count();
            ["left", "right"]
                .into_iter()
                .flat_map(|side| {
                    buttons
                        .iter()
                        .enumerate()
                        .filter(move |(_, button)| button["buttonPos"] == side)
                        .enumerate()
                        .map(move |(row, (button_index, _))| Control {
                            index: row + if side == "right" { left_count } else { 0 },
                            button_index: Some(button_index),
                            side: side.to_owned(),
                            column_side: None,
                            margin_top: 0.,
                        })
                })
                .collect::<Vec<_>>()
        } else {
            surface.controls.clone()
        };
        let mut column = v_flex()
            .absolute()
            .top(surface::css(surface.label_top))
            .w(surface::css(235.));
        column = if left {
            column
                .right(surface::css(535.))
                .pr(surface::css(15.))
                .items_end()
        } else {
            column
                .left(surface::css(535.))
                .pl(surface::css(15.))
                .items_start()
        };
        column
            .children(
                controls
                    .iter()
                    .filter(|control| {
                        (control.column_side.as_deref().unwrap_or(&control.side) == "left") == left
                    })
                    .map(|control| {
                        // 218's last label has idx:8 but receives buttonList[9]. The
                        // original button prop owns its action; idx is only the row label.
                        let index = control.button_index.unwrap_or(control.index);
                        let button = &buttons[index];
                        let input = button["inputID"].as_str().unwrap_or_default().to_owned();
                        let enabled =
                            !unavailable && button["isEnabled"].as_bool().unwrap_or(false);
                        let active = self.mapping_input.as_deref() == Some(input.as_str());
                        let value = button["assignmentValue"].as_str().unwrap_or("DEFAULT");
                        let measured_input = input.clone();
                        let anchors = self.mapping_menu_current_state.anchors.clone();
                        BaseButton::new(SharedString::from(format!(
                            "mouse-{}-input-{input}",
                            self.spec.product_id
                        )))
                        .accessibility_label(t(value))
                        .on_prepaint(move |bounds, _, _| {
                            anchors.borrow_mut().insert(measured_input.clone(), bounds);
                        })
                        .h(surface::css(30.))
                        .mb(surface::css(20.))
                        .mt(surface::css(control.margin_top))
                        .px(surface::css(10.))
                        .py_0()
                        .when(left, |button| button.mr(surface::css(10.)))
                        .when(!left, |button| button.ml(surface::css(10.)))
                        .rounded(surface::css(3.))
                        .text_size(surface::css(14.))
                        .line_height(surface::css(17.))
                        .text_color(if enabled {
                            Colors::text()
                        } else {
                            Colors::disabled_text()
                        })
                        .bg(if active {
                            Colors::selected_surface()
                        } else {
                            Colors::transparent()
                        })
                        .disabled(!enabled)
                        .hover(|style| style.bg(Colors::hover()))
                        .active(|style| style.bg(Colors::selected_surface()))
                        .child(t(value))
                        .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                            this.hover_current_customize(index, *hovered);
                            cx.notify();
                        }))
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                if this.mapping_input.as_deref() == Some(input.as_str()) {
                                    return;
                                }
                                this.open_current_mouse_mapping(input.clone(), window, cx);
                            },
                        ))
                    }),
            )
            .into_any_element()
    }

    fn customize_current_hypershift(&self, cx: &Context<Self>) -> AnyElement {
        let hyper = self.hypershift;
        BaseButton::new("mouse-current-hypershift")
            .role(Role::Switch)
            .selected(hyper)
            .accessibility_label(t("HYPERSHIFT"))
            .h(surface::css(36.))
            .p(surface::css(5.))
            .border_1()
            .border_color(Colors::line())
            .rounded(surface::css(18.))
            .bg(Colors::selected_surface())
            .hover(|style| style.border_color(Colors::selected()))
            .active(|style| style.bg(Colors::pressed()))
            .child(h_flex().children([false, true].into_iter().map(|layer| {
                let active = layer == hyper;
                div()
                    .px(surface::css(10.))
                    .pt(surface::css(6.))
                    .pb(surface::css(5.))
                    .h(surface::css(24.))
                    .when(!layer, |segment| segment.mr(surface::css(5.)))
                    .rounded(surface::css(12.))
                    .bg(if active {
                        if layer {
                            Colors::hypershift()
                        } else {
                            Colors::selected()
                        }
                    } else {
                        Colors::transparent()
                    })
                    .text_color(if active {
                        Colors::selected_text()
                    } else {
                        Colors::text()
                    })
                    .child(t(if layer { "HYPERSHIFT" } else { "STANDARD" }))
            })))
            .on_click(cx.listener(|this, _, _, cx| {
                this.hypershift = !this.hypershift;
                this.set_current_customize_input(None, false);
                this.mapping_assignment = None;
                this.customize_hover = None;
                cx.notify();
            }))
            .into_any_element()
    }
}
