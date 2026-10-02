//! IM's SVG geometry drives both painting and native Button activation.
//! GPUI's rectangular layout bounds remain useful for focus/accessibility; they
//! are never used as the final pointer hit region for a keyboard key.
use crate::resources::{KeyboardGeometry, KeyboardKey, KeyboardPathCommand};
use gpui_kit::base::{ElementExt as _, TestSupportExt as _};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::{
    App, Bounds, ClickEvent, FillOptions, FillRule, Hsla, InteractiveElement, IntoElement,
    MouseButton, ParentElement, PathBuilder, PathStyle, Pixels, Point, RenderOnce, SharedString,
    StatefulInteractiveElement, Styled, Window, canvas, div, point, prelude::FluentBuilder as _,
    px,
};
use std::{cell::Cell, rc::Rc};

impl KeyboardKey {
    pub(crate) fn contains_window_point(
        &self,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
    ) -> bool {
        if bounds.size.width <= px(0.) || bounds.size.height <= px(0.) {
            return false;
        }
        self.geometry.contains_source([
            self.bounds[0]
                + f32::from(position.x - bounds.left()) / f32::from(bounds.size.width)
                    * self.bounds[2],
            self.bounds[1]
                + f32::from(position.y - bounds.top()) / f32::from(bounds.size.height)
                    * self.bounds[3],
        ])
    }

    fn accepts_click(&self, event: &ClickEvent, bounds: Bounds<Pixels>) -> bool {
        match event {
            // Keep the real Button's Enter/Space and accessibility activation.
            ClickEvent::Keyboard(_) => true,
            ClickEvent::Mouse(event) => {
                self.contains_window_point(event.down.position, bounds)
                    && self.contains_window_point(event.up.position, bounds)
            }
            ClickEvent::Touch(event) => self.contains_window_point(event.position, bounds),
        }
    }

    fn path(&self, bounds: Bounds<Pixels>, stroke: bool) -> PathBuilder {
        let scale_x = f32::from(bounds.size.width) / self.bounds[2];
        let scale_y = f32::from(bounds.size.height) / self.bounds[3];
        let project = |p: [f32; 2]| {
            bounds.origin
                + point(
                    px((p[0] - self.bounds[0]) * scale_x),
                    px((p[1] - self.bounds[1]) * scale_y),
                )
        };
        let mut path = if stroke {
            // .svg-key:hover/.active use a 2px stroke in the source viewBox.
            PathBuilder::stroke(px(2. * scale_x))
        } else {
            PathBuilder::fill().with_style(PathStyle::Fill(
                FillOptions::default().with_fill_rule(FillRule::NonZero),
            ))
        };
        match &self.geometry {
            KeyboardGeometry::Path { commands } => {
                for command in commands {
                    match *command {
                        KeyboardPathCommand::Move(to) => path.move_to(project(to)),
                        KeyboardPathCommand::Line(to) => path.line_to(project(to)),
                        KeyboardPathCommand::Curve([a, b, to]) => {
                            path.cubic_bezier_to(project(to), project(a), project(b));
                        }
                        KeyboardPathCommand::Close => path.close(),
                    }
                }
            }
            KeyboardGeometry::Circle { center, radius } => {
                let right = project([center[0] + radius, center[1]]);
                let left = project([center[0] - radius, center[1]]);
                let radii = point(px(radius * scale_x), px(radius * scale_y));
                path.move_to(right);
                path.arc_to(radii, px(0.), false, true, left);
                path.arc_to(radii, px(0.), false, true, right);
                path.close();
            }
            KeyboardGeometry::Rect { origin, size } => {
                path.move_to(project(*origin));
                path.line_to(project([origin[0] + size[0], origin[1]]));
                path.line_to(project([origin[0] + size[0], origin[1] + size[1]]));
                path.line_to(project([origin[0], origin[1] + size[1]]));
                path.close();
            }
        }
        path
    }
}

impl KeyboardGeometry {
    pub(crate) fn contains_source(&self, position: [f32; 2]) -> bool {
        match self {
            Self::Circle { center, radius } => {
                (position[0] - center[0]).powi(2) + (position[1] - center[1]).powi(2)
                    <= radius.powi(2)
            }
            Self::Rect { origin, size } => {
                position[0] >= origin[0]
                    && position[0] <= origin[0] + size[0]
                    && position[1] >= origin[1]
                    && position[1] <= origin[1] + size[1]
            }
            Self::Path { commands } => {
                let mut crossings = Crossings::new(position);
                let mut previous = [0., 0.];
                let mut start = previous;
                for command in commands {
                    match *command {
                        KeyboardPathCommand::Move(to) => {
                            previous = to;
                            start = to;
                        }
                        KeyboardPathCommand::Line(to) => {
                            crossings.line(previous, to);
                            previous = to;
                        }
                        KeyboardPathCommand::Curve([a, b, to]) => {
                            crossings.cubic([previous, a, b, to]);
                            previous = to;
                        }
                        KeyboardPathCommand::Close => {
                            crossings.line(previous, start);
                            previous = start;
                        }
                    }
                }
                crossings.on_edge || crossings.winding != 0
            }
        }
    }
}

// SVG uses nonzero winding. Solve each cubic over its monotonic y intervals;
// using the actual curve avoids polygon tolerances at rounded key boundaries.
struct Crossings {
    point: [f64; 2],
    winding: i32,
    on_edge: bool,
}
impl Crossings {
    const EPSILON: f64 = 0.00001;

    fn new(point: [f32; 2]) -> Self {
        Self {
            point: point.map(f64::from),
            winding: 0,
            on_edge: false,
        }
    }

    fn line(&mut self, from: [f32; 2], to: [f32; 2]) {
        let from = from.map(f64::from);
        let to = to.map(f64::from);
        let [x, y] = self.point;
        let cross = (x - from[0]) * (to[1] - from[1]) - (y - from[1]) * (to[0] - from[0]);
        let length = (to[0] - from[0]).hypot(to[1] - from[1]);
        if cross.abs() <= Self::EPSILON * length.max(1.)
            && x >= from[0].min(to[0]) - Self::EPSILON
            && x <= from[0].max(to[0]) + Self::EPSILON
            && y >= from[1].min(to[1]) - Self::EPSILON
            && y <= from[1].max(to[1]) + Self::EPSILON
        {
            self.on_edge = true;
        }
        if (from[1] <= y && y < to[1]) || (to[1] <= y && y < from[1]) {
            let intersect_x = from[0] + (y - from[1]) * (to[0] - from[0]) / (to[1] - from[1]);
            if intersect_x > x {
                self.winding += if to[1] > from[1] { 1 } else { -1 };
            }
        }
    }

    fn cubic(&mut self, points: [[f32; 2]; 4]) {
        let points = points.map(|point| point.map(f64::from));
        let at = |t: f64| -> [f64; 2] {
            let u = 1. - t;
            [0, 1].map(|axis| {
                u.powi(3) * points[0][axis]
                    + 3. * u * u * t * points[1][axis]
                    + 3. * u * t * t * points[2][axis]
                    + t.powi(3) * points[3][axis]
            })
        };
        let [x, y] = self.point;
        if points
            .iter()
            .all(|point| (point[1] - y).abs() <= Self::EPSILON)
        {
            let extrema = cubic_extrema(points.map(|point| point[0]));
            let xs = extrema.into_iter().map(|t| at(t)[0]).collect::<Vec<_>>();
            let minimum = xs.iter().copied().fold(f64::INFINITY, f64::min);
            let maximum = xs.iter().copied().fold(f64::NEG_INFINITY, f64::max);
            self.on_edge |= minimum <= x && x <= maximum;
            return;
        }
        let extrema = cubic_extrema(points.map(|point| point[1]));
        for &t in &extrema {
            let p = at(t);
            self.on_edge |= (p[0] - x).abs() <= Self::EPSILON && (p[1] - y).abs() <= Self::EPSILON;
        }
        for range in extrema.windows(2) {
            let mut low = range[0];
            let mut high = range[1];
            let from = at(low)[1];
            let to = at(high)[1];
            if !((from <= y && y < to) || (to <= y && y < from)) {
                continue;
            }
            for _ in 0..40 {
                let mid = (low + high) / 2.;
                if (at(mid)[1] < y) == (to > from) {
                    low = mid;
                } else {
                    high = mid;
                }
            }
            let intersect_x = at((low + high) / 2.)[0];
            self.on_edge |= (intersect_x - x).abs() <= Self::EPSILON;
            if intersect_x > x {
                self.winding += if to > from { 1 } else { -1 };
            }
        }
    }
}

fn cubic_extrema([p0, p1, p2, p3]: [f64; 4]) -> Vec<f64> {
    let a = -p0 + 3. * p1 - 3. * p2 + p3;
    let b = 2. * (p0 - 2. * p1 + p2);
    let c = p1 - p0;
    let mut values = vec![0., 1.];
    let mut push = |t: f64| {
        if t > 0. && t < 1. {
            values.push(t);
        }
    };
    if a.abs() < 1e-12 {
        if b.abs() >= 1e-12 {
            push(-c / b);
        }
    } else {
        let discriminant = b * b - 4. * a * c;
        if discriminant >= 0. {
            push((-b - discriminant.sqrt()) / (2. * a));
            push((-b + discriminant.sqrt()) / (2. * a));
        }
    }
    values.sort_by(f64::total_cmp);
    values.dedup_by(|a, b| (*a - *b).abs() < 1e-12);
    values
}

type ClickHandler = Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
type HoverHandler = Rc<dyn Fn(&bool, &mut Window, &mut App)>;

#[derive(IntoElement)]
pub(crate) struct KeyRegion {
    key: &'static KeyboardKey,
    fill: Hsla,
    stroke: Hsla,
    selected: bool,
    hovered: bool,
    disabled: bool,
    on_click: ClickHandler,
    on_hover: HoverHandler,
}
impl KeyRegion {
    pub(crate) fn new(
        key: &'static KeyboardKey,
        fill: Hsla,
        stroke: Hsla,
        selected: bool,
        hovered: bool,
        on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
        on_hover: impl Fn(&bool, &mut Window, &mut App) + 'static,
    ) -> Self {
        Self {
            key,
            fill,
            stroke,
            selected,
            hovered,
            disabled: false,
            on_click: Rc::new(on_click),
            on_hover: Rc::new(on_hover),
        }
    }

    /// Mapping policy can disable a source-enabled key, for example in Hypershift.
    pub(crate) fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl RenderOnce for KeyRegion {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let key = self.key;
        let enabled = key.enabled && !self.disabled;
        let id = SharedString::from(format!("keyboard-input-{}", key.id));
        let focus = window
            .use_keyed_state(id.clone(), cx, |_, cx| cx.focus_handle())
            .read(cx)
            .clone();
        let active = enabled && (self.selected || self.hovered || focus.is_focused(window));
        let pointer_focus = focus.clone();
        let measured = Rc::new(Cell::new(Bounds::<Pixels>::default()));
        let for_measure = measured.clone();
        let for_move = measured.clone();
        let for_enter = measured.clone();
        let for_down = measured.clone();
        let hovered = self.on_hover.clone();
        let enter = self.on_hover;
        let on_click = self.on_click;
        div()
            .id(SharedString::from(format!("keyboard-hit-{}", key.id)))
            .test_support()
            .relative()
            .size_full()
            .on_prepaint(move |bounds, _, _| for_measure.set(bounds))
            .on_mouse_move(move |event, window, cx| {
                hovered(
                    &(enabled && key.contains_window_point(event.position, for_move.get())),
                    window,
                    cx,
                );
            })
            .on_hover(move |inside, window, cx| {
                enter(
                    &(enabled
                        && *inside
                        && key.contains_window_point(window.mouse_position(), for_enter.get())),
                    window,
                    cx,
                );
            })
            .when(enabled && self.hovered, |this| {
                this.tooltip(move |window, cx| Tooltip::new(key.label.clone()).build(window, cx))
            })
            .child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        if self.fill.a > 0. {
                            if let Ok(path) = key.path(bounds, false).build() {
                                window.paint_path(path, self.fill);
                            }
                        }
                        if active {
                            if let Ok(path) = key.path(bounds, true).build() {
                                window.paint_path(path, self.stroke);
                            }
                        }
                    },
                )
                .absolute()
                .size_full(),
            )
            .child(
                gpui_kit::base::Button::new(id)
                    .track_focus(&focus)
                    .accessibility_label(key.label.clone())
                    .selected(self.selected)
                    .disabled(!enabled)
                    .size_full()
                    .on_mouse_down(MouseButton::Left, move |event, window, cx| {
                        if enabled && key.contains_window_point(event.position, for_down.get()) {
                            window.focus(&pointer_focus, cx);
                        }
                        // Suppress rectangular automatic focus, without stopping events:
                        // the neighboring shaped Button can still claim its own point.
                        window.prevent_default();
                    })
                    .on_click(move |event, window, cx| {
                        if enabled && key.accepts_click(event, measured.get()) {
                            on_click(event, window, cx);
                            // SVG gives an overlapping point to the topmost actual shape.
                            // Rejected bounding-box corners/holes keep bubbling unchanged.
                            cx.stop_propagation();
                        }
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::{KeyRegion, KeyboardGeometry, KeyboardKey, KeyboardPathCommand};
    use crate::features::DeviceWorkspace;
    use gpui_kit::component::{Root, Theme};
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{
        AppContext, Bounds, Context, InteractiveElement, IntoElement, ParentElement, Render,
        StatefulInteractiveElement, Styled, TestAppContext, Window, div, point, px, rgb, size,
    };
    use std::{cell::Cell, rc::Rc, sync::OnceLock};

    fn key(id: &str) -> &'static KeyboardKey {
        layout_key(1, id)
    }

    fn layout_key(layout: u32, id: &str) -> &'static KeyboardKey {
        crate::resources::keyboard_keys_for_layout(layout)
            .iter()
            .find(|key| key.id == id)
            .unwrap()
    }

    #[test]
    fn iso_and_japanese_enter_cutouts_are_not_active_keys() {
        for layout in [3, 4, 6, 7, 10, 12, 15, 16, 17, 18] {
            let enter = layout_key(layout, "KEY_ENTER");
            assert!(
                enter.geometry.contains_source([435., 155.]),
                "layout {layout}"
            );
            assert!(
                enter.geometry.contains_source([450., 180.]),
                "layout {layout}"
            );
            // This point is inside Enter's bounding box, below its upper arm.
            assert!(
                !enter.geometry.contains_source([435., 180.]),
                "layout {layout}"
            );
            let neighbor = if layout == 12 {
                "KEY_BACKSLASH"
            } else {
                "KEY_NON_US_POUND"
            };
            assert!(
                layout_key(layout, neighbor)
                    .geometry
                    .contains_source([435., 180.]),
                "layout {layout}"
            );
            assert!(
                enter.geometry.contains_source([438., 180.]),
                "layout {layout}"
            );
            assert!(
                !enter.geometry.contains_source([437.99, 180.]),
                "layout {layout}"
            );
        }
        assert_eq!(crate::resources::keyboard_keys_for_layout(12).len(), 122);
        assert_ne!(
            layout_key(1, "KEY_ENTER").bounds,
            layout_key(2, "KEY_ENTER").bounds
        );
    }

    fn ring() -> &'static KeyboardKey {
        static KEY: OnceLock<KeyboardKey> = OnceLock::new();
        KEY.get_or_init(|| KeyboardKey {
            id: "test-ring".into(),
            label: "Ring".into(),
            enabled: true,
            functions: Vec::new(),
            bounds: [0., 0., 100., 100.],
            geometry: KeyboardGeometry::Path {
                // Opposite winding makes the inner contour an SVG hole.
                commands: vec![
                    KeyboardPathCommand::Move([0., 0.]),
                    KeyboardPathCommand::Line([100., 0.]),
                    KeyboardPathCommand::Line([100., 100.]),
                    KeyboardPathCommand::Line([0., 100.]),
                    KeyboardPathCommand::Close,
                    KeyboardPathCommand::Move([25., 25.]),
                    KeyboardPathCommand::Line([25., 75.]),
                    KeyboardPathCommand::Line([75., 75.]),
                    KeyboardPathCommand::Line([75., 25.]),
                    KeyboardPathCommand::Close,
                ],
            },
        })
    }

    #[test]
    fn original_cubic_circle_and_rect_edges_are_not_bounding_rectangles() {
        let side = key("DKM_KB_SIDE1");
        assert!(!side.geometry.contains_source([35.1, 100.6]));
        assert!(side.geometry.contains_source([40., 110.]));
        assert!(side.geometry.contains_source([35., 120.]));
        assert!(!side.geometry.contains_source([34.99, 120.]));
        let enter = key("KEY_ENTER");
        assert!(!enter.geometry.contains_source([413.1, 168.8]));
        assert!(enter.geometry.contains_source([438., 180.]));
        assert!(enter.geometry.contains_source([413., 180.]));
        assert!(!enter.geometry.contains_source([412.99, 180.]));
        let circle = key("DKM_KBMK_04");
        assert!(!circle.geometry.contains_source([609.6, 101.]));
        assert!(circle.geometry.contains_source([617.5, 108.9]));
        assert!(circle.geometry.contains_source([625.5, 108.9]));
        assert!(!circle.geometry.contains_source([625.51, 108.9]));
        let rectangle = key("DKM_KB_SIDE2");
        assert!(rectangle.geometry.contains_source([35., 142.5]));
        assert!(!rectangle.geometry.contains_source([34.99, 162.]));
        assert!(ring().geometry.contains_source([10., 50.]));
        assert!(!ring().geometry.contains_source([50., 50.]));
        assert!(ring().geometry.contains_source([25., 50.]));
    }

    #[test]
    fn source_paths_paint_and_hit_the_same_points_at_different_scales() {
        for scale in [0.75, 1., 1.25, 2.] {
            for key in [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 15, 16, 17, 18]
                .into_iter()
                .flat_map(crate::resources::keyboard_keys_for_layout)
            {
                let bounds = Bounds::new(
                    point(px(17.), px(29.)),
                    size(px(key.bounds[2] * scale), px(key.bounds[3] * scale)),
                );
                assert!(key.path(bounds, false).build().is_ok(), "{}", key.id);
                assert!(key.path(bounds, true).build().is_ok(), "{}", key.id);
                for fraction in [[0.01, 0.01], [0.5, 0.5], [0.99, 0.99]] {
                    let source = [
                        key.bounds[0] + key.bounds[2] * fraction[0],
                        key.bounds[1] + key.bounds[3] * fraction[1],
                    ];
                    let screen = bounds.origin
                        + point(
                            bounds.size.width * fraction[0],
                            bounds.size.height * fraction[1],
                        );
                    assert_eq!(
                        key.contains_window_point(screen, bounds),
                        key.geometry.contains_source(source)
                    );
                }
            }
        }
    }

    struct Overlap {
        key: &'static KeyboardKey,
        shaped: Rc<Cell<usize>>,
        underneath: Rc<Cell<usize>>,
        parent: Rc<Cell<usize>>,
    }
    impl Render for Overlap {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let shaped = self.shaped.clone();
            let underneath = self.underneath.clone();
            let parent = self.parent.clone();
            div().size_full().child(
                div()
                    .id("overlap-parent")
                    .relative()
                    .w(px(100.))
                    .h(px(100.))
                    .on_click(move |_, _, _| parent.set(parent.get() + 1))
                    .child(
                        gpui_kit::base::Button::new("underneath")
                            .absolute()
                            .size_full()
                            .on_click(move |_, _, _| underneath.set(underneath.get() + 1)),
                    )
                    .child(KeyRegion::new(
                        self.key,
                        rgb(0x44d62c).into(),
                        rgb(0x44d62c).into(),
                        false,
                        false,
                        move |_, _, _| shaped.set(shaped.get() + 1),
                        |_, _, _| {},
                    )),
            )
        }
    }

    #[gpui_kit::test]
    fn rejected_corners_and_holes_bubble_to_real_overlapping_buttons(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        for (key, outside, inside) in [
            (
                key("DKM_KBMK_04"),
                point(px(1.), px(1.)),
                point(px(50.), px(50.)),
            ),
            (ring(), point(px(50.), px(50.)), point(px(10.), px(50.))),
        ] {
            let shaped = Rc::new(Cell::new(0));
            let underneath = Rc::new(Cell::new(0));
            let parent = Rc::new(Cell::new(0));
            let handle = cx.open_window(size(px(240.), px(200.)), |window, cx| {
                let view = cx.new(|_| Overlap {
                    key,
                    shaped: shaped.clone(),
                    underneath: underneath.clone(),
                    parent: parent.clone(),
                });
                Root::new(view, window, cx)
            });
            let id = gpui_kit::SharedString::from(format!("keyboard-input-{}", key.id));
            cx.update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                window.click_at(id.clone(), outside, cx);
                assert_eq!((shaped.get(), underneath.get(), parent.get()), (0, 1, 1));
                window.click_at(id.clone(), inside, cx);
                assert_eq!((shaped.get(), underneath.get(), parent.get()), (1, 1, 1));
                assert_eq!(window.find(id.clone()).focused(), Some(true));
                window.press("space", cx);
                assert_eq!(shaped.get(), 2);
            })
            .unwrap();
        }
    }

    #[gpui_kit::test]
    fn real_keyboard_uses_source_geometry_at_scaled_default_and_all_known_layouts(
        cx: &mut TestAppContext,
    ) {
        cx.update(gpui_kit::init);
        for (index, layout) in [
            0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 15, 16, 17, 18, 999,
        ]
        .into_iter()
        .enumerate()
        {
            let scale = if index % 2 == 0 { 1. } else { 1.25 };
            cx.update(|cx| Theme::update(cx, |theme| theme.font_size = px(16. * scale)));
            let handle = cx.open_window(size(px(1280. * scale), px(960. * scale)), |window, cx| {
                let mut device = crate::demo::demo_keyboard();
                device.layout_id = layout;
                let view = cx.new(|cx| DeviceWorkspace::new(device, true, window, cx));
                Root::new(view, window, cx)
            });
            cx.update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                if layout == 999 {
                    assert!(window.try_find("keyboard-input-KEY_ENTER").is_none());
                    return;
                }
                window.click_at("keyboard-input-KEY_ENTER", point(px(0.1), px(0.1)), cx);
                assert_eq!(
                    window.find("keyboard-input-KEY_ENTER").selected(),
                    Some(false)
                );
                window.click("keyboard-input-KEY_ENTER", cx);
                assert_eq!(
                    window.find("keyboard-input-KEY_ENTER").selected(),
                    Some(true)
                );
                window.click_at("keyboard-input-DKM_KBMK_04", point(px(0.1), px(0.1)), cx);
                assert_eq!(
                    window.find("keyboard-input-DKM_KBMK_04").selected(),
                    Some(false)
                );
                assert_eq!(
                    window.find("keyboard-input-KEY_ENTER").selected(),
                    Some(true)
                );
                window.click("keyboard-input-DKM_KBMK_04", cx);
                assert_eq!(
                    window.find("keyboard-input-DKM_KBMK_04").selected(),
                    Some(true)
                );
                assert_eq!(
                    window.find("keyboard-input-KEY_ENTER").selected(),
                    Some(false)
                );
                if [3, 4, 6, 7, 10, 12, 15, 16, 17, 18].contains(&layout) {
                    let enter = layout_key(layout, "KEY_ENTER");
                    let bounds = window.find("keyboard-input-KEY_ENTER").bounds();
                    let notch = point(
                        bounds.size.width * ((435. - enter.bounds[0]) / enter.bounds[2]),
                        bounds.size.height * ((180. - enter.bounds[1]) / enter.bounds[3]),
                    );
                    window.click_at("keyboard-input-KEY_ENTER", notch, cx);
                    assert_eq!(
                        window.find("keyboard-input-KEY_ENTER").selected(),
                        Some(false)
                    );
                    assert_eq!(
                        window.find("keyboard-input-DKM_KBMK_04").selected(),
                        Some(false)
                    );
                    // The source notch belongs to the neighboring key. Rejecting
                    // Enter's rectangle must let that actual shape receive the click.
                    let neighbor = if layout == 12 {
                        "keyboard-input-KEY_BACKSLASH"
                    } else {
                        "keyboard-input-KEY_NON_US_POUND"
                    };
                    assert_eq!(window.find(neighbor).selected(), Some(true));
                    window.click("keyboard-input-DKM_KBMK_04", cx);
                }
            })
            .unwrap();
            if layout != 999 {
                cx.simulate_window_resize(handle.into(), size(px(1440. * scale), px(960. * scale)));
                cx.update_window(handle.into(), |_, window, cx| {
                    window.render_frame(cx);
                    window.click_at("keyboard-input-KEY_ENTER", point(px(0.1), px(0.1)), cx);
                    assert_eq!(
                        window.find("keyboard-input-DKM_KBMK_04").selected(),
                        Some(true)
                    );
                    window.click("keyboard-input-KEY_ENTER", cx);
                    assert_eq!(
                        window.find("keyboard-input-KEY_ENTER").selected(),
                        Some(true)
                    );
                })
                .unwrap();
            }
        }
    }
}
