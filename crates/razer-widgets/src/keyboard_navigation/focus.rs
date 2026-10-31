use super::KeyboardNavigation;
use crate::theme::KeyboardGuideColors as Colors;
use gpui_kit::base::Root;
use gpui_kit::base::input::{InputBaseState, InputModeKind};
use gpui_kit::*;
use std::{cell::RefCell, rc::Rc};

/// Directional keys retained by the focused control's own source behavior.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FocusBehavior {
    Spatial,
    LocalVertical,
    LocalArrows,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FocusRing {
    #[default]
    Standard,
    Options,
    InsetOuter,
    Toolbar,
    Drawer,
    Tabs,
    None,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Direction {
    Left,
    Right,
    Up,
    Down,
}

impl Direction {
    fn from_key(key: &str) -> Option<Self> {
        match key {
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            "up" => Some(Self::Up),
            "down" => Some(Self::Down),
            _ => None,
        }
    }
}

impl FocusBehavior {
    fn handles(self, direction: Direction) -> bool {
        self == Self::LocalArrows
            || self == Self::LocalVertical && matches!(direction, Direction::Up | Direction::Down)
    }
}

struct Target {
    focus: FocusHandle,
    bounds: Bounds<Pixels>,
    behavior: FocusBehavior,
    painted: bool,
}

#[derive(Default)]
pub(super) struct FocusFrame {
    targets: Vec<Target>,
}

pub(super) type SharedFrame = Rc<RefCell<FocusFrame>>;

pub(super) struct Registration {
    frame: SharedFrame,
    index: usize,
    primary: bool,
}

impl Registration {
    pub(super) fn painted(&self, bounds: Bounds<Pixels>, visible: bool) {
        let mut frame = self.frame.borrow_mut();
        let target = &mut frame.targets[self.index];
        if self.primary {
            target.painted = visible && (bounds.size.width != px(0.) || bounds.size.height != px(0.));
        } else if !visible {
            target.painted = false;
        }
    }

    pub(super) fn paints_ring(&self) -> bool {
        self.primary && self.frame.borrow().targets[self.index].painted
    }
}

pub(super) fn register_target(
    focus: &FocusHandle,
    bounds: Bounds<Pixels>,
    behavior: FocusBehavior,
    enabled: bool,
    window: &Window,
    cx: &App,
) -> Option<Registration> {
    let frame = frame(window, cx).filter(|_| enabled)?;
    let mut state = frame.borrow_mut();
    let previous = state.targets.iter().position(|target| target.focus == *focus);
    let index = previous.unwrap_or(state.targets.len());
    if previous.is_none() {
        state.targets.push(Target { focus: focus.clone(), bounds, behavior, painted: false });
    }
    drop(state);
    Some(Registration { frame, index, primary: previous.is_none() })
}

fn frame(window: &Window, cx: &App) -> Option<SharedFrame> {
    let root = window.root::<Root>().flatten()?;
    let owner = root.read(cx).plugin::<KeyboardNavigation>()?;
    Some(owner.read(cx).focus_frame.clone())
}

pub fn using_keyboard(window: &Window, cx: &App) -> bool {
    window.root::<Root>().flatten()
        .and_then(|root| root.read(cx).plugin::<KeyboardNavigation>())
        .is_some_and(|owner| owner.read(cx).keyboard)
}

/// Register the actual control handle and its resolved frame geometry.
/// Call after composing the control, with its current enabled state.
pub fn focus_target(
    element: impl IntoElement,
    focus: &FocusHandle,
    behavior: FocusBehavior,
    enabled: bool,
) -> FocusTarget {
    FocusTarget {
        child: element.into_any_element(),
        focus: focus.clone(),
        behavior,
        enabled,
        ring: FocusRing::Standard,
        radii: Corners::default(),
        opacity: 1.,
        registration: None,
    }
}

pub struct FocusTarget {
    child: AnyElement,
    focus: FocusHandle,
    behavior: FocusBehavior,
    enabled: bool,
    ring: FocusRing,
    radii: Corners<Pixels>,
    opacity: f32,
    registration: Option<Registration>,
}

impl FocusTarget {
    pub fn ring(mut self, ring: FocusRing) -> Self {
        self.ring = ring;
        self
    }

    pub fn radius(mut self, radius: Pixels) -> Self {
        self.radii = radius.into();
        self
    }

    pub fn radii(mut self, radii: Corners<Pixels>) -> Self {
        self.radii = radii;
        self
    }

    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity;
        self
    }
}

impl IntoElement for FocusTarget {
    type Element = Self;
    fn into_element(self) -> Self { self }
}

impl Element for FocusTarget {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> { None }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> { None }

    fn request_layout(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }

    fn prepaint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), window: &mut Window, cx: &mut App) {
        self.registration = register_target(&self.focus, bounds, self.behavior, self.enabled, window, cx);
        self.child.prepaint(window, cx);
    }

    fn paint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, bounds: Bounds<Pixels>, _: &mut (), _: &mut (), window: &mut Window, cx: &mut App) {
        if let Some(registration) = &self.registration {
            registration.painted(bounds, true);
        }
        self.child.paint(window, cx);
        if self.registration.as_ref().is_some_and(Registration::paints_ring)
            && self.focus.is_focused(window) && using_keyboard(window, cx) {
            paint_ring(self.ring, bounds, self.radii, self.opacity, window);
        }
    }
}

pub(super) fn paint_ring(ring: FocusRing, bounds: Bounds<Pixels>, radii: Corners<Pixels>, opacity: f32, window: &mut Window) {
    let unit = window.rem_size() / 16.;
    let black = Colors::focus_inner().opacity(opacity);
    let white = Colors::focus_outer().opacity(opacity);
    let mut stroke = |bounds: Bounds<Pixels>, width: f32, color: Hsla, radii: Corners<Pixels>| {
        window.paint_quad(outline(bounds, color, BorderStyle::Solid)
            .border_widths(unit * width).corner_radii(radii));
    };
    match ring {
        FocusRing::Standard => {
            stroke(bounds.dilate(unit * 4.), 4., white, radii.map(|radius| *radius + unit * 4.));
            stroke(bounds.dilate(unit * 2.), 2., black, radii.map(|radius| *radius + unit * 2.));
        }
        FocusRing::Options => {
            stroke(bounds.dilate(unit * 2.), 2., black, radii.map(|radius| *radius + unit * 2.));
            stroke(bounds, 2., white, radii);
        }
        FocusRing::InsetOuter | FocusRing::Tabs => {
            let radii = if ring == FocusRing::Tabs { (unit * 14.).into() } else { radii };
            stroke(bounds.dilate(unit * 4.), 4., black, radii.map(|radius| *radius + unit * 4.));
            stroke(bounds.dilate(unit * 2.), 2., white, radii.map(|radius| *radius + unit * 2.));
            stroke(bounds, 2., black, radii);
        }
        FocusRing::Toolbar => {
            stroke(bounds, 4., white, radii);
            stroke(bounds, 2., black, radii);
        }
        FocusRing::Drawer => {
            stroke(bounds.dilate(unit * 2.), 2., black, radii.map(|radius| *radius + unit * 2.));
            stroke(bounds, 2., white, radii);
        }
        FocusRing::None => {}
    }
}

fn directional_score(current: Bounds<Pixels>, candidate: Bounds<Pixels>, direction: Direction) -> Option<Pixels> {
    let delta = candidate.center() - current.center();
    let (ahead, perpendicular) = match direction {
        Direction::Right if candidate.left() >= current.right() - px(1.) => (delta.x, delta.y.abs()),
        Direction::Left if candidate.right() <= current.left() + px(1.) => (-delta.x, delta.y.abs()),
        Direction::Down if candidate.top() >= current.bottom() - px(1.) => (delta.y, delta.x.abs()),
        Direction::Up if candidate.bottom() <= current.top() + px(1.) => (-delta.y, delta.x.abs()),
        _ => return None,
    };
    (ahead > px(0.)).then_some(ahead + perpendicular * 2.)
}

pub(super) fn navigate(event: &KeyDownEvent, window: &mut Window, cx: &mut App) {
    let Some(direction) = Direction::from_key(&event.keystroke.key) else { return; };
    let Some(focus) = window.focused(cx) else { return; };
    let Some(frame) = frame(window, cx) else { return; };
    let state = frame.borrow();
    let Some(current) = state.targets.iter().find(|target| target.painted && target.focus == focus) else { return; };
    if current.behavior.handles(direction) { return; }
    let mut selected = None;
    for candidate in state.targets.iter().filter(|target| target.painted) {
        if current.focus.contains(&candidate.focus, window) || candidate.focus.contains(&current.focus, window) { continue; }
        if let Some(score) = directional_score(current.bounds, candidate.bounds, direction)
            && selected.as_ref().is_none_or(|(best, _)| score < *best) {
            selected = Some((score, candidate.focus.clone()));
        }
    }
    drop(state);
    if let Some((_, target)) = selected {
        target.focus(window, cx);
        cx.stop_propagation();
    }
}

/// Use the completed frame after a dropdown closes. The source keeps the
/// trigger focused at the first/last boundary instead of wrapping around.
pub fn focus_adjacent_to(trigger: &FocusHandle, backwards: bool, window: &mut Window, cx: &mut App) {
    let is_tab_target = frame(window, cx).is_some_and(|frame| frame.borrow().targets.iter().any(|target|
        target.painted && target.focus == *trigger && target.focus.tab_stop && target.focus.tab_index >= 0));
    if !is_tab_target {
        trigger.focus(window, cx);
        return;
    }
    // The rendered Tab table also contains text inputs and composed Kit controls.
    // GPUI publishes focus changes at the next draw, so this synchronous boundary
    // lookup exposes only the final target to focus subscribers.
    window.blur(cx);
    if backwards { window.focus_next(cx); } else { window.focus_prev(cx); }
    let boundary = window.focused(cx);
    trigger.focus(window, cx);
    if boundary.as_ref().is_some_and(|boundary| boundary != trigger) {
        if backwards { window.focus_prev(cx); } else { window.focus_next(cx); }
    }
}

pub(super) fn scope(element: impl IntoElement, frame: SharedFrame) -> impl IntoElement {
    FrameScope { child: element.into_any_element(), frame }
}

struct FrameScope { child: AnyElement, frame: SharedFrame }
impl IntoElement for FrameScope {
    type Element = Self;
    fn into_element(self) -> Self { self }
}
impl Element for FrameScope {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> { None }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> { None }
    fn request_layout(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, window: &mut Window, cx: &mut App) -> (LayoutId, ()) {
        (self.child.request_layout(window, cx), ())
    }
    fn prepaint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, _: Bounds<Pixels>, _: &mut (), window: &mut Window, cx: &mut App) {
        self.frame.borrow_mut().targets.clear();
        self.child.prepaint(window, cx);
    }
    fn paint(&mut self, _: Option<&GlobalElementId>, _: Option<&InspectorElementId>, _: Bounds<Pixels>, _: &mut (), _: &mut (), window: &mut Window, cx: &mut App) {
        self.child.paint(window, cx);
    }
}

#[cfg(test)]
mod tests {
    use super::{Direction, FocusBehavior, directional_score};
    use gpui_kit::{Bounds, point, px, size};

    #[test]
    fn source_directional_score_rejects_overlap_and_weights_perpendicular_distance() {
        let rect = |x, y, width, height| Bounds::new(point(px(x), px(y)), size(px(width), px(height)));
        let current = rect(10., 10., 20., 20.);
        assert_eq!(directional_score(current, rect(40., 10., 20., 20.), Direction::Right), Some(px(30.)));
        assert_eq!(directional_score(current, rect(40., 25., 20., 20.), Direction::Right), Some(px(60.)));
        assert_eq!(directional_score(current, rect(28., 10., 20., 20.), Direction::Right), None);
        assert_eq!(directional_score(current, rect(29., 10., 20., 20.), Direction::Right), Some(px(19.)));
        assert_eq!(directional_score(current, rect(-20., 10., 20., 20.), Direction::Left), Some(px(30.)));
        assert_eq!(directional_score(current, rect(10., -20., 20., 20.), Direction::Up), Some(px(30.)));
        assert_eq!(directional_score(current, rect(10., 40., 20., 20.), Direction::Down), Some(px(30.)));
        assert_eq!(directional_score(current, rect(-30., 10., 20., 20.), Direction::Right), None);
    }

    #[test]
    fn local_controls_keep_only_the_direction_keys_their_source_owns() {
        for direction in [Direction::Left, Direction::Right, Direction::Up, Direction::Down] {
            assert!(FocusBehavior::LocalArrows.handles(direction));
            assert!(!FocusBehavior::Spatial.handles(direction));
            assert_eq!(FocusBehavior::LocalVertical.handles(direction), matches!(direction, Direction::Up | Direction::Down));
        }
    }
}
