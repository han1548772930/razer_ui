//! Original ordinary mouse row's unstyled secondary marker and fixed tooltip.
//! Its absent selector does not authorize borrowing the Keypad marker artwork.
use super::*;

#[derive(Deserialize)]
struct Metadata {
    product_id: u32,
    presence_field: String,
    assignment_field: String,
    default_assignment: String,
    label: String,
    type_line_height: Option<f32>,
    name_line_height: Option<f32>,
}
fn metadata(pid: u32) -> &'static Metadata {
    static DATA: OnceLock<Vec<Metadata>> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("mouse_sidebar_secondary_current_data.json"))
            .expect("actual mouse secondary row policy")
    })
    .iter()
    .find(|p| p.product_id == pid)
    .expect("audited ordinary mouse row")
}

#[derive(Default)]
pub(super) struct State {
    shown: Rc<RefCell<BTreeSet<usize>>>,
    positions: RefCell<BTreeMap<usize, Rc<Cell<Bounds<Pixels>>>>>,
    trigger_bounds: Rc<RefCell<BTreeMap<usize, Bounds<Pixels>>>>,
    tip_bounds: Rc<RefCell<BTreeMap<usize, Bounds<Pixels>>>>,
}
impl State {
    pub(super) fn retain(&self, mounted: &BTreeSet<usize>) {
        self.shown.borrow_mut().retain(|ix| mounted.contains(ix));
        self.positions
            .borrow_mut()
            .retain(|ix, _| mounted.contains(ix));
        self.trigger_bounds
            .borrow_mut()
            .retain(|ix, _| mounted.contains(ix));
        self.tip_bounds
            .borrow_mut()
            .retain(|ix, _| mounted.contains(ix));
    }
}
pub(super) fn marker(
    workspace: &MouseProductWorkspace,
    button: &Value,
    ix: usize,
    disabled: bool,
    cx: &Context<MouseProductWorkspace>,
) -> Option<AnyElement> {
    let metadata = metadata(workspace.spec.product_id);
    // A present null satisfies the source's !== undefined guard.
    if button.get(&metadata.presence_field).is_none()
        || button[&metadata.assignment_field] == metadata.default_assignment
    {
        return None;
    }
    // The actual ordinary row receives hyperShiftOn, without isHyperShiftOn
    // or isSystemDevice. Those separate optional-prop branches stay inactive.
    let kind = if truthy(&button[&metadata.assignment_field]) {
        t(&text(&button[&metadata.assignment_field])).to_string()
    } else {
        String::new()
    };
    let value = button
        .get(format!("{}Value", metadata.assignment_field))
        .map(text)
        .unwrap_or_default();
    let state = &workspace.sidebar_current_state.secondary;
    let position = state.positions.borrow_mut().entry(ix).or_default().clone();
    Some(
        Marker {
            ix,
            owner: cx.entity().downgrade(),
            metadata,
            label: t(&metadata.label).to_string(),
            kind,
            value: t(&value).to_string(),
            shown: state.shown.borrow().contains(&ix),
            disabled,
            position,
            trigger_bounds: state.trigger_bounds.clone(),
            tip_bounds: state.tip_bounds.clone(),
        }
        .into_any_element(),
    )
}

#[derive(IntoElement)]
struct Marker {
    ix: usize,
    owner: WeakEntity<MouseProductWorkspace>,
    metadata: &'static Metadata,
    label: String,
    kind: String,
    value: String,
    shown: bool,
    disabled: bool,
    position: Rc<Cell<Bounds<Pixels>>>,
    trigger_bounds: Rc<RefCell<BTreeMap<usize, Bounds<Pixels>>>>,
    tip_bounds: Rc<RefCell<BTreeMap<usize, Bounds<Pixels>>>>,
}
fn tip_surface(metadata: &Metadata, label: &str, kind: &str, value: &str) -> Div {
    div()
        .max_w(surface::css(300.))
        .min_h(surface::css(75.))
        .bg(PanelColors::tooltip())
        .border_1()
        .border_color(Colors::line())
        .text_color(Colors::text())
        .text_center()
        // `.customize .key-tip { padding: initial }` overrides 8px 10px.
        .child(
            v_flex()
                // The actual ancestry is logo-twoKeyMapping, not twoKeyMapping;
                // the latter's gray 14px secondaryFunction rule does not match.
                .child(div().child(format!(" {label}")))
                .child(
                    div()
                        .text_size(surface::css(12.))
                        .when_some(metadata.type_line_height, |view, line| {
                            view.line_height(surface::css(line))
                        })
                        .child(kind.to_uppercase()),
                )
                .child(
                    div()
                        .text_size(surface::css(18.))
                        .when_some(metadata.name_line_height, |view, line| {
                            view.line_height(surface::css(line))
                        })
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(capitalize(value)),
                ),
        )
}
fn hover(
    owner: &WeakEntity<MouseProductWorkspace>,
    ix: usize,
    entered: bool,
    tip: bool,
    size: Size<Pixels>,
    window: &Window,
    cx: &mut App,
) {
    let _ = owner.update(cx, |workspace, cx| {
        let state = &workspace.sidebar_current_state.secondary;
        let other = if tip {
            &state.trigger_bounds
        } else {
            &state.tip_bounds
        };
        // The fixed tooltip remains a DOM descendant in React. Preserve its
        // enter/leave ancestry when GPUI moves it into a deferred layer.
        let inside = entered
            || state.shown.borrow().contains(&ix)
                && other
                    .borrow()
                    .get(&ix)
                    .is_some_and(|rect| rect.contains(&window.mouse_position()));
        let mut repositioned = false;
        if entered
            && !tip
            && let Some(rect) = state.trigger_bounds.borrow().get(&ix)
            && let Some(position) = state.positions.borrow().get(&ix)
        {
            let next = Bounds::new(
                point(
                    rect.left() + surface::css(20.).to_pixels(window.rem_size()) - size.width / 2.,
                    rect.top() - surface::css(2.).to_pixels(window.rem_size()) - size.height,
                ),
                Size::default(),
            );
            repositioned = next != position.get();
            position.set(next);
        }
        let changed = if inside {
            state.shown.borrow_mut().insert(ix)
        } else {
            state.shown.borrow_mut().remove(&ix)
        };
        if changed || repositioned {
            cx.notify();
        }
    });
}
impl RenderOnce for Marker {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let mut measurement =
            tip_surface(self.metadata, &self.label, &self.kind, &self.value).into_any_element();
        let size = measurement.layout_as_root(
            size(AvailableSpace::MaxContent, AvailableSpace::MinContent),
            window,
            cx,
        );
        let opacity = gpui_kit::base::motion::transition(
            ElementId::from((
                ElementId::from(("mouse-secondary-opacity", self.ix)),
                SharedString::from(self.owner.entity_id().to_string()),
            )),
            if self.shown { 1. } else { 0. },
            gpui_kit::base::motion::Transition::new(Duration::from_millis(300))
                .easing(gpui_kit::base::motion::Easing::Ease),
            window,
            cx,
        );
        let ix = self.ix;
        let trigger_owner = self.owner.clone();
        let tip_owner = self.owner;
        let triggers = self.trigger_bounds;
        let tips = self.tip_bounds;
        let tooltip = tip_surface(self.metadata, &self.label, &self.kind, &self.value)
            .id(("mouse-sidebar-secondary-tip", ix))
            .w(size.width)
            .opacity(opacity)
            .when(!self.shown, |tip| tip.invisible())
            .on_prepaint(move |rect, _, _| {
                tips.borrow_mut().insert(ix, rect);
            })
            .on_hover(move |entered, window, cx| {
                hover(&tip_owner, ix, *entered, true, size, window, cx);
            });
        // No icon, width, height, margins or absolute offsets are attached:
        // all 76 current main stylesheets lack this marker's selector.
        div()
            .id(("mouse-sidebar-secondary-marker", ix))
            .on_prepaint(move |rect, _, _| {
                triggers.borrow_mut().insert(ix, rect);
            })
            .when(!self.disabled, |marker| {
                marker.on_hover(move |entered, window, cx| {
                    hover(&trigger_owner, ix, *entered, false, size, window, cx);
                })
            })
            .child(
                deferred(Layer::new(
                    LayerKind::Secondary(self.position),
                    tooltip.into_any_element(),
                ))
                .with_priority(999),
            )
    }
}
