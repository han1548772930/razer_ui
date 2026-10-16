//! Common source calculateKeyConfigPopupLeft / keyboard dot-bg position.
//! Current functions for all 72 products: keyboard-mapping-position-current.json.gz.
use super::*;
use std::cell::RefCell;

type Regions = Rc<RefCell<BTreeMap<String, (&'static razer_assets::KeyboardKey, Bounds<Pixels>)>>>;

#[derive(Deserialize)]
struct TwoTapPosition {
    product_id: u32,
    guards_hidden_target: bool,
}
fn two_tap_position(product_id: u32) -> Option<&'static TwoTapPosition> {
    static DATA: OnceLock<Vec<TwoTapPosition>> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!("keyboard_mapping_position_data.json"))
            .expect("audited current TwoTap geometry")
    })
    .iter()
    .find(|row| row.product_id == product_id)
}

pub(super) struct PopupPosition {
    child: AnyElement,
    input: String,
    region: Option<String>,
    regions: Regions,
    button_regions: Rc<RefCell<BTreeMap<String, Bounds<Pixels>>>>,
    viewport: Rc<Cell<Bounds<Pixels>>>,
    scroll: ScrollHandle,
    from_panel: bool,
    drawer_open: bool,
    drawer_width: f32,
    two_tap: Option<&'static TwoTapPosition>,
}
impl PopupPosition {
    pub(super) fn new(
        workspace: &KeyboardProductWorkspace,
        edit: &Edit,
        child: AnyElement,
    ) -> Self {
        Self {
            child,
            input: text(&edit.button["inputID"]),
            region: workspace.analog_gamepad.mapping.popup_region.clone(),
            regions: workspace.analog_gamepad.floating.key_bounds.clone(),
            button_regions: workspace.analog_gamepad.mapping.button_regions.clone(),
            viewport: workspace.analog_gamepad.mapping.viewport_bounds.clone(),
            scroll: workspace.analog_gamepad.mapping.page_scroll.clone(),
            from_panel: edit.from_panel,
            drawer_open: workspace.button_drawer_open,
            drawer_width: keypad_panel::width(workspace.spec.product_id),
            two_tap: (edit.button["inputType"] == "AnalogInput")
                .then(|| two_tap_position(workspace.spec.product_id))
                .flatten(),
        }
    }
}
impl IntoElement for PopupPosition {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for PopupPosition {
    type RequestLayoutState = LayoutId;
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, LayoutId) {
        let child = self.child.request_layout(window, cx);
        (
            window.request_layout(
                Style {
                    position: Position::Absolute,
                    display: Display::Flex,
                    ..Style::default()
                },
                [child],
                cx,
            ),
            child,
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        child: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let viewport = self.viewport.get();
        let drawer = if self.drawer_open {
            surface::css(self.drawer_width).to_pixels(window.rem_size())
        } else {
            px(0.)
        };
        // Source clientWidth excludes the popup's 1px borders.
        let width = (window.layout_bounds(*child).size.width - px(2.)).max(px(0.));
        let regions = self.regions.borrow();
        let target = self
            .region
            .as_ref()
            .and_then(|id| regions.get(id))
            .or_else(|| regions.values().find(|(key, _)| key.id == self.input))
            .map(|(_, rect)| *rect)
            .or_else(|| self.button_regions.borrow().get(&self.input).copied());
        let hidden_target = self
            .two_tap
            .is_some_and(|source| source.guards_hidden_target)
            && target
                .is_none_or(|target| target.size.width == px(0.) || target.size.height == px(0.));
        let mut left = if self.from_panel && self.drawer_open {
            drawer
        } else if hidden_target {
            // The fourteen guarded source callers return before drawer clamping.
            px(0.)
        } else if let Some(target) = target {
            let key_left = target.left() - viewport.left();
            let scroll_left = px(0.) - self.scroll.offset().x;
            if target.right() - viewport.left() + scroll_left + width
                < viewport.size.width
                    - surface::css(if self.two_tap.is_some() { 0. } else { 20. })
                        .to_pixels(window.rem_size())
            {
                target.right() - viewport.left()
            } else {
                key_left - width
            }
        } else {
            surface::css(if self.drawer_open { self.drawer_width } else { 20. })
                .to_pixels(window.rem_size())
        };
        // The source reads actual drawer clientWidth for a panel click, but
        // retains a literal 230px clamp for physical-key popup positions.
        if self.drawer_open && !self.from_panel && left < drawer && !hidden_target {
            left = surface::css(230.).to_pixels(window.rem_size());
        } else if left < px(0.) {
            left = px(0.);
        }
        // Source body top36, and its absolute header sits at top:-36.
        // Native PopupSize contains both; place its header at the wrapper
        // origin. Drawer remains fixed, physical keys subtract scrollTop.
        let scroll = if self.from_panel {
            px(0.)
        } else {
            self.scroll.offset().y
        };
        let origin = point(viewport.left() + left, viewport.top() + scroll);
        window.with_element_offset(origin - bounds.origin, |window| {
            self.child.prepaint(window, cx)
        });
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut LayoutId,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.child.paint(window, cx);
    }
}
