//! Static reconstruction of current PID190 mapping caller and MapMouse.
//! Evidence: main.81b09779.js gO @4544837; 316.f2f64e83.chunk.js
//! module5316 Ut @31579; MapMouse.cf968564.chunk.js module9119.
//! Main CSS popup @107609/111485, head @113322, menu @135424,
//! body @111940. No vendor JS is evaluated. Saves below are LOCAL DRAFTS.
//! Native setMappingList -> profile/service submission/ack remains unconnected.
use super::*;
use razer_widgets::source_tooltip::SourceTooltip;
use std::time::Duration;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

#[derive(Clone)]
pub(crate) enum Next {
    Open(String),
    Hyper,
    Close,
    Panel,
    PanelOpen(String),
    Tab(String),
    History(bool),
}
pub enum MouseMappingNavigation {
    Page(String),
    History(bool),
}
impl EventEmitter<MouseMappingNavigation> for MouseProductWorkspace {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseMappingSaveRejected {
    UnavailableObservation,
    IneligibleDraft,
    MissingActiveInput,
    UnsupportedFunction,
    InvalidKeyboard,
    PrimaryClickRequired,
}
impl EventEmitter<MouseMappingSaveRejected> for MouseProductWorkspace {}

#[derive(Default)]
pub(crate) struct State {
    pub panel_open: bool,
    pub panel_filter: u8,
    pub panel_filter_open: bool,
    pub from_panel: bool,
    pub panel_scroll: ScrollHandle,
    pub body_scroll: ScrollHandle,
    pub surface_bounds: Rc<Cell<Bounds<Pixels>>>,
    pub anchors: Rc<RefCell<BTreeMap<String, Bounds<Pixels>>>>,
    pub panel_tooltip: Option<String>,
    pub panel_tooltip_anchor: Rc<Cell<Bounds<Pixels>>>,
    pub outer_body_bounds: Rc<Cell<Bounds<Pixels>>>,
    pub menu_bounds: Rc<Cell<Bounds<Pixels>>>,
    pub menu_height: Option<Pixels>,
    pub panel_dropdown: DropdownGeometry,
    pub option_dropdown: DropdownGeometry,
    pub group_dropdown: DropdownGeometry,
    pub dropdown_activation: Option<Subscription>,
    pub function: String,
    pub value: Value,
    pub original: Value,
    pub changed: bool,
    pub can_save: bool,
    pub expanded: bool,
    pub option_open: bool,
    pub pending: Option<Next>,
    pub primary_error: bool,
    pub input: Option<Entity<InputState>>,
    pub turbo: Option<Entity<SliderState>>,
    pub turbo_number: Option<Entity<InputState>>,
    pub turbo_speed: Value,
    pub turbo_typed: bool,
    pub turbo_repeat: Option<Task<()>>,
    pub syncing_controls: bool,
    pub focus: Option<FocusHandle>,
    pub recording: bool,
    pub group_open: bool,
    pub include_modifiers: bool,
    pub selected_symbol: Option<String>,
    /// None means the caller's dependency producers have not supplied observations.
    /// This is separate from an observed source canSave=false.
    pub missing_observation: Option<&'static str>,
    pub subscriptions: Vec<Subscription>,
}
impl State {
    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }
    fn clear_editor(&mut self) {
        let mut replacement = Self::default();
        replacement.panel_open = self.panel_open;
        replacement.panel_filter = self.panel_filter;
        replacement.panel_scroll = self.panel_scroll.clone();
        replacement.body_scroll = self.body_scroll.clone();
        replacement.surface_bounds = self.surface_bounds.clone();
        replacement.anchors = self.anchors.clone();
        replacement.outer_body_bounds = self.outer_body_bounds.clone();
        replacement.panel_dropdown = self.panel_dropdown.clone();
        replacement.dropdown_activation = self.dropdown_activation.take();
        *self = replacement;
    }
    fn source_save_eligibility(&self) -> Option<bool> {
        self.missing_observation.is_none().then_some(self.can_save)
    }
}

#[derive(Clone, Default)]
pub(super) struct DropdownGeometry {
    pub trigger: Rc<Cell<Bounds<Pixels>>>,
    pub options: Rc<Cell<Bounds<Pixels>>>,
    pub scroll: ScrollHandle,
}

/// Current 4355 -> 5035 row tooltip. Unlike shared drop-tips it has no fade.
pub(super) struct BindingTooltip {
    content: AnyElement,
    surface: Rc<Cell<Bounds<Pixels>>>,
    row: Rc<Cell<Bounds<Pixels>>>,
}

/// gO.renderSaveAlert uses an absolute full main-container backdrop and a
/// fixed dialog: left50%-width/2, top50%-height, including the border/padding.
struct ConfirmationLayer {
    backdrop: AnyElement,
    dialog: AnyElement,
}

#[derive(Default)]
struct AlertButtonInteraction {
    hovered: bool,
    pressed: bool,
}

fn alert_button(id: &'static str, window: &mut Window, cx: &mut App) -> BaseButton {
    let interaction =
        window.use_keyed_state((id, 0u32), cx, |_, _| AlertButtonInteraction::default());
    let state = interaction.read(cx);
    let target = if state.pressed {
        0.6
    } else if state.hovered {
        0.8
    } else {
        1.
    };
    let opacity = gpui_kit::base::motion::transition(
        (id, "opacity"),
        target,
        gpui_kit::base::motion::Transition::new(Duration::from_millis(300))
            .easing(gpui_kit::base::motion::Easing::Ease),
        window,
        cx,
    );
    BaseButton::new(id)
        .opacity(opacity)
        .on_hover(
            window.listener_for(&interaction, |state, hovered: &bool, _, cx| {
                state.hovered = *hovered;
                cx.notify();
            }),
        )
        .on_mouse_down(
            MouseButton::Left,
            window.listener_for(&interaction, |state, _, _, cx| {
                state.pressed = true;
                cx.notify();
            }),
        )
        .on_mouse_up(
            MouseButton::Left,
            window.listener_for(&interaction, |state, _, _, cx| {
                state.pressed = false;
                cx.notify();
            }),
        )
        .on_mouse_up_out(
            MouseButton::Left,
            window.listener_for(&interaction, |state, _, _, cx| {
                state.pressed = false;
                cx.notify();
            }),
        )
}
impl IntoElement for ConfirmationLayer {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for ConfirmationLayer {
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
        let backdrop = self.backdrop.request_layout(window, cx);
        let dialog = self.dialog.request_layout(window, cx);
        (
            window.request_layout(
                Style {
                    position: Position::Absolute,
                    ..Style::default()
                },
                [backdrop, dialog],
                cx,
            ),
            dialog,
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        dialog: &mut LayoutId,
        window: &mut Window,
        cx: &mut App,
    ) {
        let measured = window.layout_bounds(*dialog).size;
        let viewport = window.viewport_size();
        window.with_element_offset(point(px(0.), px(0.)) - bounds.origin, |window| {
            self.backdrop.prepaint(window, cx)
        });
        let left = (viewport.width - measured.width) / 2.;
        let top = viewport.height / 2. - measured.height;
        window.with_element_offset(point(left, top) - bounds.origin, |window| {
            self.dialog.prepaint(window, cx)
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
        self.backdrop.paint(window, cx);
        self.dialog.paint(window, cx);
    }
}
impl BindingTooltip {
    pub(super) fn new(text: String, state: &State, window: &Window) -> Self {
        let width = (surface::label_width(&text, 14., window) + 22.).min(358.);
        Self {
            content: div()
                .w(surface::css(width))
                .px(surface::css(10.))
                .py(surface::css(8.))
                .border_1()
                .border_color(rgb(0x5d5d5d))
                .bg(rgb(0x000000))
                .text_color(rgb(0xcccccc))
                .text_size(surface::css(14.))
                .line_height(surface::css(16.))
                .whitespace_normal()
                .child(text)
                .into_any_element(),
            surface: state.surface_bounds.clone(),
            row: state.panel_tooltip_anchor.clone(),
        }
    }
}
impl IntoElement for BindingTooltip {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for BindingTooltip {
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
        let child = self.content.request_layout(window, cx);
        (
            window.request_layout(
                Style {
                    position: Position::Absolute,
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
        let unit = window.rem_size() / 16.;
        let height = window.layout_bounds(*child).size.height;
        let mut top = self.row.get().top() + unit * 50.;
        if top + height > window.viewport_size().height {
            top -= height + unit * 50.;
        }
        let left = self.surface.get().left() + unit * 61.;
        window.with_element_offset(point(left, top) - bounds.origin, |window| {
            self.content.prepaint(window, cx)
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
        self.content.paint(window, cx);
    }
}

/// Shared 7734 dropdown options, isolated from the scrolling function body.
pub(super) struct DropdownLayer {
    content: AnyElement,
    geometry: DropdownGeometry,
    height: f32,
    expected_height: f32,
}
impl DropdownLayer {
    pub(super) fn new(
        content: AnyElement,
        geometry: DropdownGeometry,
        height: f32,
        expected_height: f32,
    ) -> Self {
        Self {
            content,
            geometry,
            height,
            expected_height,
        }
    }
}
impl IntoElement for DropdownLayer {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for DropdownLayer {
    type RequestLayoutState = ();
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
    ) -> (LayoutId, ()) {
        let child = self.content.request_layout(window, cx);
        (
            window.request_layout(
                Style {
                    position: Position::Absolute,
                    ..Style::default()
                },
                [child],
                cx,
            ),
            (),
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let anchor = self.geometry.trigger.get();
        let unit = window.rem_size() / 16.;
        // 7734 compares bottom+2+min(180,25*n+2) with window.innerHeight.
        let up =
            anchor.bottom() + unit * (2. + self.expected_height) > window.viewport_size().height;
        let top = if up {
            anchor.top() - unit * (self.height + 1.)
        } else {
            anchor.top() + unit * 28.
        };
        window.with_element_offset(point(anchor.left(), top) - bounds.origin, |window| {
            self.content.prepaint(window, cx)
        });
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.content.paint(window, cx);
    }
}

// The current shared caller measures actual button rectangles every animation
// frame. This layer reads rectangles captured during the same GPUI prepaint,
// so resizing and scrolling do not rely on guessed canvas/label coordinates.
pub(super) struct PopupLayer {
    content: AnyElement,
    surface: Rc<Cell<Bounds<Pixels>>>,
    anchors: Rc<RefCell<BTreeMap<String, Bounds<Pixels>>>>,
    input: String,
    from_panel: bool,
    panel_open: bool,
    body_scroll: ScrollHandle,
}
impl PopupLayer {
    pub(super) fn new(content: AnyElement, state: &State, input: &str) -> Self {
        Self {
            content,
            surface: state.surface_bounds.clone(),
            anchors: state.anchors.clone(),
            input: input.into(),
            from_panel: state.from_panel,
            panel_open: state.panel_open,
            body_scroll: state.body_scroll.clone(),
        }
    }
}
impl IntoElement for PopupLayer {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for PopupLayer {
    type RequestLayoutState = ();
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
    ) -> (LayoutId, ()) {
        let child = self.content.request_layout(window, cx);
        (
            window.request_layout(
                Style {
                    position: Position::Absolute,
                    ..Style::default()
                },
                [child],
                cx,
            ),
            (),
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let root = self.surface.get();
        let unit = window.rem_size() / 16.;
        let key = if self.from_panel {
            format!("panel:{}", self.input)
        } else {
            self.input.clone()
        };
        let anchor = self.anchors.borrow().get(&key).copied().unwrap_or(root);
        let popup_width = unit * 292.;
        // calculateKeyConfigPopupLeft: drawer origin uses measured width;
        // image origin chooses right of the label if the viewport has space,
        // otherwise left of it, then clamps the left edge / drawer overlap.
        let left = if self.panel_open && self.from_panel {
            root.left() + unit * 230.
        } else {
            let candidate =
                if anchor.right() + popup_width < window.viewport_size().width - unit * 20. {
                    anchor.right()
                } else {
                    anchor.left() - popup_width
                };
            if self.panel_open && candidate < root.left() + unit * 230. {
                root.left() + unit * 230.
            } else {
                candidate.max(px(0.))
            }
        };
        // The source top-view caller sets 36-scrollTop; drawer origin stays36.
        // The header is positioned -36 above this body in the source CSS.
        let top = root.top()
            + unit * 36.
            + if self.from_panel {
                px(0.)
            } else {
                self.body_scroll.offset().y
            };
        window.with_element_offset(point(left, top) - bounds.origin, |window| {
            self.content.prepaint(window, cx)
        });
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        self.content.paint(window, cx);
    }
}

// Current190 main module6114 literal fields (no downloaded code evaluated).
// `symbol:*` is UI selection identity only. MapKeyboard.getMappingData
// resolves a modifier-bearing row through the FIRST Rk keyCode match and
// adds KEY_LEFT_SHIFT; only the resulting real inputID reaches the draft.
const KEYS: &[(&str, &str, &str)] = &[
    ("KEY_A", "A", "Alphanumeric"),
    ("KEY_B", "B", "Alphanumeric"),
    ("KEY_C", "C", "Alphanumeric"),
    ("KEY_D", "D", "Alphanumeric"),
    ("KEY_E", "E", "Alphanumeric"),
    ("KEY_F", "F", "Alphanumeric"),
    ("KEY_G", "G", "Alphanumeric"),
    ("KEY_H", "H", "Alphanumeric"),
    ("KEY_I", "I", "Alphanumeric"),
    ("KEY_J", "J", "Alphanumeric"),
    ("KEY_K", "K", "Alphanumeric"),
    ("KEY_L", "L", "Alphanumeric"),
    ("KEY_M", "M", "Alphanumeric"),
    ("KEY_N", "N", "Alphanumeric"),
    ("KEY_O", "O", "Alphanumeric"),
    ("KEY_P", "P", "Alphanumeric"),
    ("KEY_Q", "Q", "Alphanumeric"),
    ("KEY_R", "R", "Alphanumeric"),
    ("KEY_S", "S", "Alphanumeric"),
    ("KEY_T", "T", "Alphanumeric"),
    ("KEY_U", "U", "Alphanumeric"),
    ("KEY_V", "V", "Alphanumeric"),
    ("KEY_W", "W", "Alphanumeric"),
    ("KEY_X", "X", "Alphanumeric"),
    ("KEY_Y", "Y", "Alphanumeric"),
    ("KEY_Z", "Z", "Alphanumeric"),
    ("KEY_0", "0", "Alphanumeric"),
    ("KEY_1", "1", "Alphanumeric"),
    ("KEY_2", "2", "Alphanumeric"),
    ("KEY_3", "3", "Alphanumeric"),
    ("KEY_4", "4", "Alphanumeric"),
    ("KEY_5", "5", "Alphanumeric"),
    ("KEY_6", "6", "Alphanumeric"),
    ("KEY_7", "7", "Alphanumeric"),
    ("KEY_8", "8", "Alphanumeric"),
    ("KEY_9", "9", "Alphanumeric"),
    ("KEY_F1", "F1", "Function"),
    ("KEY_F2", "F2", "Function"),
    ("KEY_F3", "F3", "Function"),
    ("KEY_F4", "F4", "Function"),
    ("KEY_F5", "F5", "Function"),
    ("KEY_F6", "F6", "Function"),
    ("KEY_F7", "F7", "Function"),
    ("KEY_F8", "F8", "Function"),
    ("KEY_F9", "F9", "Function"),
    ("KEY_F10", "F10", "Function"),
    ("KEY_F11", "F11", "Function"),
    ("KEY_F12", "F12", "Function"),
    ("KEY_F13", "F13", "Function"),
    ("KEY_F14", "F14", "Function"),
    ("KEY_F15", "F15", "Function"),
    ("KEY_F16", "F16", "Function"),
    ("KEY_F17", "F17", "Function"),
    ("KEY_F18", "F18", "Function"),
    ("KEY_F19", "F19", "Function"),
    ("KEY_F20", "F20", "Function"),
    ("KEY_F21", "F21", "Function"),
    ("KEY_F22", "F22", "Function"),
    ("KEY_F23", "F23", "Function"),
    ("KEY_F24", "F24", "Function"),
    ("KEY_NUMPAD_NUM_LOCK", "Num Lock", "Numpad"),
    ("KEY_NUMPAD_0", "Num 0", "Numpad"),
    ("KEY_NUMPAD_1", "Num 1", "Numpad"),
    ("KEY_NUMPAD_2", "Num 2", "Numpad"),
    ("KEY_NUMPAD_3", "Num 3", "Numpad"),
    ("KEY_NUMPAD_4", "Num 4", "Numpad"),
    ("KEY_NUMPAD_5", "Num 5", "Numpad"),
    ("KEY_NUMPAD_6", "Num 6", "Numpad"),
    ("KEY_NUMPAD_7", "Num 7", "Numpad"),
    ("KEY_NUMPAD_8", "Num 8", "Numpad"),
    ("KEY_NUMPAD_9", "Num 9", "Numpad"),
    ("KEY_NUMPAD_SLASH", "Num /", "Numpad"),
    ("KEY_NUMPAD_ASTERISK", "Num *", "Numpad"),
    ("KEY_NUMPAD_DASH", "Num -", "Numpad"),
    ("KEY_NUMPAD_PLUS", "Num +", "Numpad"),
    ("KEY_NUMPAD_PERIOD", "Num .", "Numpad"),
    ("KEY_NUMPAD_ENTER", "Num Enter", "Numpad"),
    ("KEY_UP_ARROW", "Up", "Navigation"),
    ("KEY_DOWN_ARROW", "Down", "Navigation"),
    ("KEY_LEFT_ARROW", "Left", "Navigation"),
    ("KEY_RIGHT_ARROW", "Right", "Navigation"),
    ("KEY_PAGE_UP", "Page Up", "Navigation"),
    ("KEY_PAGE_DOWN", "Page Down", "Navigation"),
    ("KEY_BACKSPACE", "Backspace", "Navigation"),
    ("KEY_TAB", "Tab", "Navigation"),
    ("KEY_SPACEBAR", "Space", "Navigation"),
    ("KEY_ENTER", "Enter", "Navigation"),
    ("KEY_ESC", "Esc", "Navigation"),
    ("KEY_INSERT", "Insert", "Navigation"),
    ("KEY_HOME", "Home", "Navigation"),
    ("KEY_DELETE", "Delete", "Navigation"),
    ("KEY_END", "End", "Navigation"),
    ("KEY_PRINT_SCREEN", "Print Screen", "Navigation"),
    ("KEY_PAUSE", "Pause", "Navigation"),
    ("KEY_CANCEL", "Invalid/cancel", ""),
    ("KEY_APPLICATION", "Menu", "Navigation"),
    ("KEY_LEFT_CTRL", "Ctrl", "Modifiers"),
    ("KEY_RIGHT_CTRL", "Right Ctrl", "Modifiers"),
    ("KEY_LEFT_SHIFT", "Shift", "Modifiers"),
    ("KEY_RIGHT_SHIFT", "Right Shift", "Modifiers"),
    ("KEY_LEFT_ALT", "Alt", "Modifiers"),
    ("KEY_RIGHT_ALT", "Right Alt", "Modifiers"),
    ("KEY_LEFT_GUI", "Windows", "Modifiers"),
    ("KEY_CAPS_LOCK", "Caps Lock", "Modifiers"),
    ("KEY_SCROLL_LOCK", "Scroll Lock", "Modifiers"),
    ("KEY_NUMPAD_NUM_LOCK", "Num Lock", "Modifiers"),
    ("KEY_TILDE", "`", "Symbols"),
    ("symbol:~", "~", "Symbols"),
    ("symbol:!", "!", "Symbols"),
    ("symbol:@", "@", "Symbols"),
    ("symbol:#", "#", "Symbols"),
    ("symbol:$", "$", "Symbols"),
    ("symbol:%", "%", "Symbols"),
    ("symbol:^", "^", "Symbols"),
    ("symbol:&", "&", "Symbols"),
    ("symbol:*", "*", "Symbols"),
    ("symbol:(", "(", "Symbols"),
    ("symbol:)", ")", "Symbols"),
    ("KEY_HYPEN", "-", "Symbols"),
    ("KEY_EQUAL", "=", "Symbols"),
    ("symbol:_", "_", "Symbols"),
    ("symbol:+", "+", "Symbols"),
    ("KEY_OPEN_SQUARE_BRACKET", "[", "Symbols"),
    ("KEY_CLOSE_SQUARE_BRACKET", "]", "Symbols"),
    ("KEY_BACKSLASH", "\\", "Symbols"),
    ("symbol:{", "{", "Symbols"),
    ("symbol:}", "}", "Symbols"),
    ("symbol:|", "|", "Symbols"),
    ("KEY_SEMICOLON", ";", "Symbols"),
    ("KEY_APOSTROPHE", "'", "Symbols"),
    ("symbol::", ":", "Symbols"),
    ("symbol:\"", "\"", "Symbols"),
    ("KEY_COMMA", ",", "Symbols"),
    ("KEY_PERIOD", ".", "Symbols"),
    ("KEY_SLASH", "/", "Symbols"),
    ("symbol:<", "<", "Symbols"),
    ("symbol:>", ">", "Symbols"),
    ("symbol:?", "?", "Symbols"),
    ("KEY_YEN", "KEY_YEN", ""),
    ("KEY_RO", "KEY_BEFORE_LEFT_SHIFT", ""),
    ("KEY_MUHENKAN", "KEY_BEFORE_SPACE", ""),
    ("KEY_HENKAN", "KEY_AFTER_SPACE", ""),
    ("KEY_KATAKANA_HIRAGANA", "KEY_BEFORE_RIGHT_ALT", ""),
    ("KEY_NON_US_BACKSLASH", "\\", ""),
];
const SHIFTED: &[(&str, &str)] = &[
    ("symbol:~", "KEY_TILDE"),
    ("symbol:!", "KEY_1"),
    ("symbol:@", "KEY_2"),
    ("symbol:#", "KEY_3"),
    ("symbol:$", "KEY_4"),
    ("symbol:%", "KEY_5"),
    ("symbol:^", "KEY_6"),
    ("symbol:&", "KEY_7"),
    ("symbol:*", "KEY_8"),
    ("symbol:(", "KEY_9"),
    ("symbol:)", "KEY_0"),
    ("symbol:_", "KEY_HYPEN"),
    ("symbol:+", "KEY_EQUAL"),
    ("symbol:{", "KEY_OPEN_SQUARE_BRACKET"),
    ("symbol:}", "KEY_CLOSE_SQUARE_BRACKET"),
    ("symbol:|", "KEY_BACKSLASH"),
    ("symbol::", "KEY_SEMICOLON"),
    ("symbol:\"", "KEY_APOSTROPHE"),
    ("symbol:<", "KEY_COMMA"),
    ("symbol:>", "KEY_PERIOD"),
    ("symbol:?", "KEY_SLASH"),
];

// module9267 ne + oe. DeviceInfo.extendSupportMappings admits all four oe.
const MOUSE: &[(&str, &str)] = &[
    ("Click", "LEFT_CLICK"),
    ("Menu", "RIGHT_CLICK"),
    ("ScrollButton", "SCROLL_CLICK"),
    ("DoubleClick", "DOUBLE_CLICK"),
    ("ScrollUp", "SCROLL_UP"),
    ("ScrollDown", "SCROLL_DOWN"),
    ("Previous", "MOUSE_BUTTON_4"),
    ("Next", "MOUSE_BUTTON_5"),
    ("ScrollLeft", "SCROLL_LEFT"),
    ("ScrollRight", "SCROLL_RIGHT"),
    ("RepeatScrollUp", "REPEAT_SCROLL_UP"),
    ("RepeatScrollDown", "REPEAT_SCROLL_DOWN"),
    ("RepeatScrollLeft", "REPEAT_SCROLL_LEFT"),
    ("RepeatScrollRight", "REPEAT_SCROLL_RIGHT"),
];
const MEDIA: &[(&str, &str)] = &[
    ("VolumeDown", "VOLUME_DOWN"),
    ("VolumeUp", "VOLUME_UP"),
    ("MuteVolume", "MUTE_VOLUME"),
    ("MicVolumeUp", "MIC_VOLUME_UP"),
    ("MicVolumeDown", "MIC_VOLUME_DOWN"),
    ("MuteMic", "MUTE_MIC"),
    ("MuteAll", "MUTE_ALL"),
    ("Play", "PLAY_PAUSE"),
    ("PrevTrack", "PREVIOUS_TRACK"),
    ("NextTrack", "NEXT_TRACK"),
];
const WINDOWS: &[(&str, &str)] = &[
    ("Calculator", "LAUNCH_CALCULATOR"),
    ("MSPaint", "LAUNCH_MSPAINT"),
    ("Notepad", "LAUNCH_NOTEPAD"),
    ("Snipping_Tool", "LAUNCH_SNIPPING_TOOL"),
    ("LaunchTaskManager", "LAUNCH_TASK_MANAGER"),
    ("MSCopilot", "LAUNCH_WINDOWS_COPILOT"),
    ("User_Directory", "OPEN_USER_DIRECTORY"),
    ("PowerUserMenu", "OPEN_SYSTEM_UTILITY"),
    ("ShowDesktop", "SHOW_DESKTOP"),
    ("CycleApps", "CYCLE_APPS"),
    ("SwitchApps", "SWITCH_APPS"),
    ("CloseApp", "CLOSE_APP"),
    ("Cut", "CUT"),
    ("Copy", "COPY"),
    ("Paste", "PASTE"),
    ("Mail", "MAIL"),
    ("ThisPC", "THIS_PC"),
    ("Refresh", "REFRESH"),
    ("DisplayBrightnessUp", "DISPLAY_BRIGHTNESS_UP"),
    ("DisplayBrightnessDown", "DISPLAY_BRIGHTNESS_DOWN"),
    ("File_Explorer", "OPEN_FILE_EXPLORER"),
    ("LockComputer", "LOCK_COMPUTER"),
    ("WindowsZoomIn", "WINDOWS_ZOOM_IN"),
    ("WindowsZoomOut", "WINDOWS_ZOOM_OUT"),
    ("OfficeZoomIn", "OFFICE_ZOOM_IN"),
    ("OfficeZoomOut", "OFFICE_ZOOM_OUT"),
];
const SENSITIVITY: &[(&str, &str)] = &[
    ("DPI_Clutch", "SENSITIVITY_CLUTCH"),
    ("DPI_Up", "SENSITIVITY_STAGE_UP"),
    ("DPI_Down", "SENSITIVITY_STAGE_DOWN"),
    ("DPI_OnTheFly", "ON_THE_FLY_SENSITIVITY"),
    ("DPI_CycleUp", "CYCLE_UP_SENSITIVITY"),
    ("DPI_CycleDown", "CYCLE_DOWN_SENSITIVITY"),
];
fn group(function: &str) -> Option<&'static str> {
    Some(match function {
        "MOUSE_FUNCTION" => "mouseGroup",
        "MULTIMEDIA" => "multimediaGroup",
        "WINDOWS_SHORTCUT" => "win8ShortcutsGroup",
        "SENSITIVITY" => "sensitivityGroup",
        "KEYBOARD_FUNCTION" => "keyboardGroup",
        "TEXT_FUNCTION" => "textBlockGroup",
        "LAUNCH_PROGRAM" => "launchGroup",
        "RAZER_HYPERSHIFT" => "hyperShiftGroup",
        "DISABLE" => "disableGroup",
        "DEFAULT" => "defaultGroup",
        _ => return None,
    })
}
fn function(mapping: &Value) -> &'static str {
    match mapping["outputType"].as_str().unwrap_or("") {
        "mouseGroup" => "MOUSE_FUNCTION",
        "multimediaGroup" => "MULTIMEDIA",
        "win8ShortcutsGroup" => "WINDOWS_SHORTCUT",
        "sensitivityGroup" => "SENSITIVITY",
        "keyboardGroup" => "KEYBOARD_FUNCTION",
        "textBlockGroup" => "TEXT_FUNCTION",
        "launchGroup" => "LAUNCH_PROGRAM",
        "hyperShiftGroup" => "RAZER_HYPERSHIFT",
        "disableGroup" => "DISABLE",
        "macroGroup" => "MACRO",
        "profileNavigationGroup" => "SWITCH_PROFILE",
        "lightPacGroup" => "SWITCH_LIGHTING",
        "interDeviceGroup" => "INTERDEVICE",
        "aiLauncherGroup" => "AI_LAUNCHER",
        _ => "DEFAULT",
    }
}
fn defaults(function: &str) -> Value {
    match function {
        "MOUSE_FUNCTION" => json!({"mouseAssignment":"Click"}),
        "MULTIMEDIA" => json!({"multimediaAssignment":"VolumeDown","useTurbo":true}),
        "WINDOWS_SHORTCUT" => json!({"windowsShortcutAssignment":"Calculator"}),
        "SENSITIVITY" => {
            json!({"sensitivityAssignment":"DPI_Clutch","x":800,"y":800,"independent":false})
        }
        "KEYBOARD_FUNCTION" => json!({"keyGroup":0,"modifiers":[],"key":""}),
        "TEXT_FUNCTION" => json!({"text":""}),
        "LAUNCH_PROGRAM" => json!({"url":""}),
        _ => json!({}),
    }
}
fn icon(function: &str, active: bool) -> String {
    let name = match function {
        "DEFAULT" => "default",
        "KEYBOARD_FUNCTION" => "keyboard",
        "MOUSE_FUNCTION" => "mouse",
        "SENSITIVITY" => "sensitivity",
        "MACRO" => "macro",
        "INTERDEVICE" => "interdevice",
        "SWITCH_PROFILE" => "profile",
        "SWITCH_LIGHTING" => "lighting",
        "RAZER_HYPERSHIFT" => "hypershift",
        "LAUNCH_PROGRAM" => "launch",
        "MULTIMEDIA" => "multimedia",
        "WINDOWS_SHORTCUT" => "windows",
        "TEXT_FUNCTION" => "text",
        "DISABLE" => "disable",
        "AI_LAUNCHER" => "ai",
        _ => "launch",
    };
    format!(
        "synapse/mapping-{name}{}.{}",
        if active { "-active" } else { "" },
        if name == "lighting" { "png" } else { "svg" }
    )
}

impl MouseProductWorkspace {
    pub(crate) fn mapping_190_body_bounds_handle(&self) -> Rc<Cell<Bounds<Pixels>>> {
        self.mapping_190_state.outer_body_bounds.clone()
    }

    /// Semantic adapter for 5316.windowClick. Callers must supply source target
    /// and direct-parent classes, rather than treating every outer click alike.
    pub(crate) fn mapping_190_document_click(
        &mut self,
        target: &[&str],
        parent: &[&str],
        ancestors: &[&str],
        inside_mapping: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.spec.product_id != 190 || self.mapping_input.is_none() || inside_mapping {
            return false;
        }
        // Outer navigation already owns its Save continuation for this click.
        if self.mapping_190_state.pending.is_some() {
            return false;
        }
        if ancestors
            .iter()
            .any(|classes| classes.contains("raw-text") && classes.contains("option"))
        {
            return false;
        }
        const WHITELIST: &[&str] = &[
            "interdevice-icon-show",
            "config-btns-wrapper",
            "tip-special-button-text",
            "drawer-btn",
            "svg-key",
            "main-container",
            "body-wrapper",
            "key",
            "config-btn",
            "save-close",
            "preventing-closing-mapping",
            "mapping-button",
            "secondary-function",
            "config-ctx",
            "button-description",
        ];
        if WHITELIST
            .iter()
            .any(|class| target.contains(class) || parent.contains(class))
        {
            return false;
        }
        self.mapping_190_request(Next::Close, window, cx);
        true
    }

    pub(super) fn mapping_190_toggle_dropdown(
        &mut self,
        id: &'static str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let state = &mut self.mapping_190_state;
        let open = match id {
            "panel" => &mut state.panel_filter_open,
            "group" => &mut state.group_open,
            _ => &mut state.option_open,
        };
        *open = !*open;
        if state.dropdown_activation.is_none() {
            state.dropdown_activation =
                Some(cx.observe_window_activation(window, |this, window, cx| {
                    if !window.is_window_active() {
                        this.mapping_190_state.panel_filter_open = false;
                        this.mapping_190_state.option_open = false;
                        this.mapping_190_state.group_open = false;
                        cx.notify();
                    }
                }));
        }
        cx.notify();
    }

    pub(super) fn mapping_190_source_dropdown(
        &self,
        id: &'static str,
        label: String,
        selected: usize,
        options: Vec<AnyElement>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let state = &self.mapping_190_state;
        let (open, geometry, priority) = match id {
            "panel" => (state.panel_filter_open, state.panel_dropdown.clone(), 150),
            "group" => (state.group_open, state.group_dropdown.clone(), 104),
            _ => (state.option_open, state.option_dropdown.clone(), 104),
        };
        let expected = (options.len() as f32 * 25. + 2.).min(180.);
        let height = gpui_kit::base::motion::transition(
            ElementId::from((
                ElementId::from(SharedString::from(format!("mouse-190-{id}-options-height"))),
                SharedString::from(cx.entity_id().to_string()),
            )),
            if open { 180. } else { 0. },
            gpui_kit::base::motion::Transition::new(Duration::from_millis(200))
                .easing(gpui_kit::base::motion::Easing::Ease),
            window,
            cx,
        );
        let height = if open { height.min(expected) } else { 0. };
        let rotation = gpui_kit::base::motion::transition(
            ElementId::from((
                ElementId::from(SharedString::from(format!("mouse-190-{id}-options-icon"))),
                SharedString::from(cx.entity_id().to_string()),
            )),
            if open { 180. } else { 0. },
            gpui_kit::base::motion::Transition::new(Duration::from_millis(300))
                .easing(gpui_kit::base::motion::Easing::Ease),
            window,
            cx,
        );
        let trigger = geometry.trigger.clone();
        let outside_geometry = geometry.clone();
        let bounds = geometry.options.clone();
        let width = geometry.trigger.get().size.width;
        let options_surface = v_flex()
            .id(SharedString::from(format!("mouse-190-{id}-source-options")))
            .w(if width > px(0.) {
                width
            } else {
                window.rem_size() * (210. / 16.)
            })
            .h(surface::css(height))
            .overflow_y_scroll()
            .track_scroll(&geometry.scroll)
            .bg(rgb(0x000000))
            .when(open, |el| el.border_1().border_color(rgb(0x515151)))
            .on_prepaint(move |rect, _, _| bounds.set(rect))
            .children(if open { options } else { Vec::new() });
        // Selection is scrolled into view when opening, not on every scroll frame.
        div()
            .relative()
            .w_full()
            .h(surface::css(27.))
            .child(
                BaseButton::new(SharedString::from(format!("mouse-190-{id}-source-trigger")))
                    .w_full()
                    .h(surface::css(27.))
                    .px(surface::css(5.))
                    .py_0()
                    .border_1()
                    .border_color(rgb(if open { 0x44d62c } else { 0x515151 }))
                    .bg(rgb(0x111111))
                    .text_color(rgb(0xcccccc))
                    .on_prepaint(move |rect, _, _| trigger.set(rect))
                    .child(
                        div()
                            .w_full()
                            .pr(surface::css(24.))
                            .text_ellipsis()
                            .text_left()
                            .child(label),
                    )
                    .child(
                        svg()
                            .path("synapse/automation-icon_expand.svg")
                            .text_color(rgb(0x999999))
                            .absolute()
                            .right(surface::css(9.5))
                            .top(surface::css(7.5))
                            .size(surface::css(10.))
                            .with_transformation(Transformation::rotate(radians(
                                rotation.to_radians(),
                            ))),
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        let geometry = match id {
                            "panel" => &this.mapping_190_state.panel_dropdown,
                            "group" => &this.mapping_190_state.group_dropdown,
                            _ => &this.mapping_190_state.option_dropdown,
                        };
                        geometry.scroll.scroll_to_top_of_item(selected);
                        this.mapping_190_toggle_dropdown(id, window, cx);
                    }))
                    .on_mouse_up_out(
                        MouseButton::Left,
                        cx.listener(move |this, event: &MouseUpEvent, _, cx| {
                            if outside_geometry.options.get().contains(&event.position) {
                                return;
                            }
                            let open = match id {
                                "panel" => &mut this.mapping_190_state.panel_filter_open,
                                "group" => &mut this.mapping_190_state.group_open,
                                _ => &mut this.mapping_190_state.option_open,
                            };
                            if *open {
                                *open = false;
                                cx.notify();
                            }
                        }),
                    ),
            )
            .when(height > 0., |el| {
                el.child(
                    deferred(DropdownLayer::new(
                        options_surface.into_any_element(),
                        geometry,
                        height,
                        expected,
                    ))
                    .with_priority(priority),
                )
            })
            .into_any_element()
    }
    pub(super) fn mapping_190_button(&self, input: &str) -> Option<&Value> {
        self.spec.groups[0]["buttonList"]
            .as_array()?
            .iter()
            .find(|b| b["inputID"] == input)
    }
    pub(super) fn mapping_190_current(&self, input: &str) -> Option<&Value> {
        self.draft["mappings"].as_array()?.iter().find(|m| {
            m["inputID"] == input && m["isHyperShift"].as_bool().unwrap_or(false) == self.hypershift
        })
    }
    pub(super) fn mapping_190_label(&self, button: &Value) -> SharedString {
        let input = button["inputID"].as_str().unwrap_or("");
        let Some(mapping) = self.mapping_190_current(input) else {
            return t(button["defaultValue"].as_str().unwrap_or("DEFAULT")).into();
        };
        let fun = function(mapping);
        let value = group(fun).map(|g| &mapping[g]);
        if let Some(v) = value {
            for (key, choices) in [
                ("mouseAssignment", MOUSE),
                ("multimediaAssignment", MEDIA),
                ("windowsShortcutAssignment", WINDOWS),
                ("sensitivityAssignment", SENSITIVITY),
            ] {
                if let Some((_, label)) = choices.iter().find(|(id, _)| v[key] == *id) {
                    return t(label).into();
                }
            }
            for key in ["text", "key", "path", "url"] {
                if let Some(s) = v[key].as_str() {
                    return s.to_owned().into();
                }
            }
        }
        t(fun).into()
    }
    pub(super) fn mapping_190_assignment_label(&self, input: &str) -> String {
        self.mapping_190_current(input)
            .map(|mapping| {
                let fun = function(mapping);
                t(if fun == "MOUSE_FUNCTION" {
                    "MOUSE"
                } else {
                    fun
                })
            })
            .unwrap_or_else(|| t("MOUSE"))
    }
    // gO.updateMappingsToButtons: the last standard left-click is protected,
    // including a remapped Click on another button. HyperShift is independent.
    pub(super) fn mapping_190_enabled(&self, button: &Value) -> bool {
        if self.hypershift {
            return self
                .mapping_190_current(button["inputID"].as_str().unwrap_or(""))
                .is_none_or(|m| m["outputType"] != "hyperShiftGroup");
        }
        let buttons = self.spec.groups[0]["buttonList"]
            .as_array()
            .expect("190 buttons");
        let click = |b: &Value| {
            self.mapping_190_current(b["inputID"].as_str().unwrap_or(""))
                .map_or(b["defaultValue"] == "LEFT_CLICK", |m| {
                    m["outputType"] == "mouseGroup" && m["mouseGroup"]["mouseAssignment"] == "Click"
                })
        };
        !click(button) || buttons.iter().filter(|b| click(b)).count() > 1
    }
    pub(super) fn mapping_190_request(
        &mut self,
        next: Next,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if matches!(&next, Next::Open(input) | Next::PanelOpen(input) if self.mapping_input.as_ref() == Some(input))
        {
            // Both mounted image and drawer callers ignore the active button.
            return;
        }
        if self.mapping_190_state.changed && self.mapping_input.is_some() {
            self.mapping_190_state.pending = Some(next);
            cx.notify();
        } else {
            self.mapping_190_next(next, window, cx);
        }
    }
    pub fn defer_mapping_page(
        &mut self,
        key: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.spec.product_id != 190 || !self.mapping_190_state.changed {
            return false;
        }
        self.mapping_190_request(Next::Tab(key.into()), window, cx);
        true
    }
    pub fn defer_mapping_history(
        &mut self,
        forward: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.spec.product_id != 190 || !self.mapping_190_state.changed {
            return false;
        }
        self.mapping_190_request(Next::History(forward), window, cx);
        true
    }
    fn mapping_190_next(&mut self, next: Next, window: &mut Window, cx: &mut Context<Self>) {
        let from_panel = self.mapping_190_state.from_panel;
        self.mapping_190_state.clear_editor();
        match next {
            Next::Close => self.mapping_input = None,
            Next::Panel => {
                if self.mapping_190_state.panel_open && from_panel {
                    self.mapping_input = None;
                }
                self.mapping_190_state.panel_open = !self.mapping_190_state.panel_open;
                // Panel-origin selection becomes an image selection on closing.
                self.mapping_190_state.from_panel = false;
                if let Some(input) = self.mapping_input.clone() {
                    self.mapping_190_reopen(input, false, window, cx);
                }
            }
            Next::Tab(key) => {
                self.mapping_input = None;
                cx.emit(MouseMappingNavigation::Page(key));
            }
            Next::History(forward) => {
                self.mapping_input = None;
                cx.emit(MouseMappingNavigation::History(forward));
            }
            Next::Hyper => {
                self.hypershift = !self.hypershift;
                self.mapping_input = None;
                self.customize_hover = None;
            }
            Next::Open(input) => self.mapping_190_reopen(input, false, window, cx),
            Next::PanelOpen(input) => self.mapping_190_reopen(input, true, window, cx),
        }
        cx.notify();
    }
    fn mapping_190_reopen(
        &mut self,
        input: String,
        from_panel: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(button) = self.mapping_190_button(&input) else {
            return;
        };
        if !self.mapping_190_enabled(button) {
            return;
        }
        let current = self
            .mapping_190_current(&input)
            .cloned()
            .unwrap_or(Value::Null);
        self.mapping_input = Some(input);
        self.mapping_190_state.from_panel = from_panel;
        self.mapping_190_state.original = current.clone();
        self.mapping_190_select_function(function(&current), window, cx);
        self.mapping_190_state.can_save = false;
        cx.notify();
    }
    fn mapping_190_select_function(
        &mut self,
        fun: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let state = &mut self.mapping_190_state;
        state.function = fun.to_owned();
        state.value = if function(&state.original) == fun {
            group(fun)
                .and_then(|g| state.original.get(g))
                .cloned()
                .unwrap_or_else(|| defaults(fun))
        } else {
            defaults(fun)
        };
        state.changed = false; // source changeFunction resets isMappingChanged.
        state.can_save = group(fun).is_some()
            && fun != function(&state.original)
            && !matches!(
                fun,
                "KEYBOARD_FUNCTION" | "TEXT_FUNCTION" | "LAUNCH_PROGRAM"
            );
        state.option_open = false;
        state.expanded = false;
        state.menu_height = None;
        state.primary_error = false;
        state.subscriptions.clear();
        state.input = None;
        state.turbo = None;
        state.turbo_number = None;
        state.turbo_repeat = None;
        state.turbo_typed = false;
        state.missing_observation = match fun {
            "MACRO" => Some("macroReducer.macroList/macroAvailable/installedModules"),
            "INTERDEVICE" => Some("validDevices/profile/sensitivity/keymap producers"),
            "SWITCH_PROFILE" => {
                Some("profileReducer.profiles/lightingEffectsReducer.chromaProfiles")
            }
            "SWITCH_LIGHTING" => Some("chroma installation/profile observation producers"),
            "AI_LAUNCHER" => Some("getAiLauncher/isInstalled/engine list producers"),
            _ => None,
        };
        state.focus = Some(cx.focus_handle());
        state.recording = false;
        state.group_open = false;
        state.selected_symbol = if state.value["keyGroup"] == 6
            && state.value["modifiers"].as_array().is_some_and(|m| {
                m.iter()
                    .any(|m| m == "KEY_LEFT_SHIFT" || m == "KEY_RIGHT_SHIFT")
            }) {
            SHIFTED
                .iter()
                .find(|(_, base)| state.value["key"] == *base)
                .map(|(symbol, _)| symbol.to_string())
        } else {
            None
        };
        state.include_modifiers = state.value["modifiers"]
            .as_array()
            .is_some_and(|m| !m.is_empty());
        if fun == "SENSITIVITY"
            && self.mapping_input.as_deref() == Some("CycleUpSensitivityStages")
            && state.value["sensitivityAssignment"] == "DPI_Clutch"
        {
            state.value["sensitivityAssignment"] = json!("DPI_Up");
        }
        if matches!(fun, "TEXT_FUNCTION" | "LAUNCH_PROGRAM") {
            let key = if fun == "TEXT_FUNCTION" {
                "text"
            } else if state.value.get("path").is_some() {
                "path"
            } else {
                "url"
            };
            let value = state.value[key].as_str().unwrap_or("").to_owned();
            let input = cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(value)
                    .validate(move |value, _| key != "text" || value.encode_utf16().count() <= 250)
            });
            let subscription = cx.subscribe(&input, move |this, input, event: &InputEvent, cx| {
                if !matches!(event, InputEvent::Change) {
                    return;
                }
                let value = input.read(cx).value().to_string();
                this.mapping_190_state.value = json!({key: value});
                this.mapping_190_state.changed = true;
                this.mapping_190_state.can_save = !value.is_empty();
                cx.notify();
            });
            self.mapping_190_state.input = Some(input);
            self.mapping_190_state.subscriptions.push(subscription);
        }
        if matches!(fun, "MOUSE_FUNCTION" | "KEYBOARD_FUNCTION") {
            let value = self.mapping_190_state.value["turboMode"]["keysPerSecond"]
                .as_f64()
                .unwrap_or(7.) as f32;
            self.mapping_190_state.turbo_speed = json!(value as i64);
            let slider = cx.new(|_| {
                SliderState::new()
                    .min(1.)
                    .max(20.)
                    .step(1.)
                    .default_value(value)
            });
            let number = cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value((value as u8).to_string())
                    .mask_pattern(MaskPattern::None)
                    .validate(|text, _| {
                        let digits = text.strip_prefix('-').unwrap_or(text);
                        digits.len() <= 2 && digits.bytes().all(|b| b.is_ascii_digit())
                    })
            });
            let number_subscription = cx.subscribe_in(
                &number,
                window,
                |this, input, event: &InputEvent, window, cx| {
                    if this.mapping_190_state.syncing_controls {
                        return;
                    }
                    let text = input.read(cx).value().to_string();
                    if matches!(event, InputEvent::Change) {
                        // 4230.handleChange sends the accepted STRING live. It
                        // permits "" and "-"; parseInput belongs to handleBlur.
                        this.mapping_190_state.turbo_typed = true;
                        this.mapping_190_turbo_value(json!(text), false, window, cx);
                    } else if matches!(event, InputEvent::Blur | InputEvent::PressEnter { .. }) {
                        let value = text.parse::<i64>().unwrap_or(0).clamp(1, 20);
                        this.mapping_190_state.turbo_typed = false;
                        this.mapping_190_turbo_value(json!(value), true, window, cx);
                    }
                },
            );
            let subscription = cx.subscribe_in(
                &slider,
                window,
                |this, _, event: &SliderEvent, window, cx| {
                    if this.mapping_190_state.syncing_controls {
                        return;
                    }
                    let value = match event {
                        SliderEvent::Change(v) | SliderEvent::Release(v) => v.start(),
                    };
                    this.mapping_190_state.syncing_controls = true;
                    if let Some(input) = &this.mapping_190_state.turbo_number {
                        input.update(cx, |input, cx| {
                            input.set_value((value.round() as u8).to_string(), window, cx)
                        });
                    }
                    this.mapping_190_state.syncing_controls = false;
                    if !matches!(event, SliderEvent::Release(_)) {
                        cx.notify();
                        return;
                    }
                    this.mapping_190_state.turbo_typed = false;
                    this.mapping_190_turbo_value(json!(value.round() as u8), true, window, cx);
                },
            );
            self.mapping_190_state.turbo = Some(slider);
            self.mapping_190_state.turbo_number = Some(number);
            self.mapping_190_state.subscriptions.push(subscription);
            self.mapping_190_state
                .subscriptions
                .push(number_subscription);
        }
        cx.notify();
    }
    fn mapping_190_save(&mut self, cx: &mut Context<Self>) -> bool {
        if self.mapping_190_state.source_save_eligibility() != Some(true) {
            cx.emit(if self.mapping_190_state.missing_observation.is_some() {
                MouseMappingSaveRejected::UnavailableObservation
            } else {
                MouseMappingSaveRejected::IneligibleDraft
            });
            return false;
        }
        let Some(input) = self.mapping_input.clone() else {
            cx.emit(MouseMappingSaveRejected::MissingActiveInput);
            return false;
        };
        let fun = self.mapping_190_state.function.clone();
        let Some(output) = group(&fun) else {
            cx.emit(MouseMappingSaveRejected::UnsupportedFunction);
            return false;
        };
        let mut value = self.mapping_190_state.value.clone();
        if value["turboMode"]["isTurbo"] == true {
            // Save activation normally follows input Blur. Normalize the
            // snapshot as well for keyboard activation during retained focus.
            value["turboMode"]["keysPerSecond"] = json!(self.mapping_190_turbo_numeric());
        }
        if fun == "KEYBOARD_FUNCTION"
            && !value["key"].as_str().is_some_and(|key| {
                KEYS.iter()
                    .any(|(id, _, _)| *id == key && !id.starts_with("symbol:"))
            })
        {
            cx.emit(MouseMappingSaveRejected::InvalidKeyboard);
            return false;
        }
        let mut entries = self.draft["mappings"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let reset_both_layers = entries.iter().any(|m| {
            m["inputID"] == input
                && m["isHyperShift"] != true
                && matches!(
                    m["outputType"].as_str(),
                    Some("hyperShiftGroup" | "disableGroup")
                )
        });
        entries.retain(|m| {
            m["inputID"] != input
                || (m["isHyperShift"].as_bool().unwrap_or(false) != self.hypershift
                    && m["outputType"] != "hyperShiftGroup")
        });
        if fun != "DEFAULT" {
            let mut mapping = json!({"inputID":input,"inputType":"MouseInput","isHyperShift":self.hypershift,"outputType":output});
            if !matches!(fun.as_str(), "RAZER_HYPERSHIFT" | "DISABLE") {
                mapping[output] = value;
            }
            entries.push(mapping.clone());
            if fun == "RAZER_HYPERSHIFT" && !self.hypershift {
                entries.retain(|m| m["inputID"] != input || m["isHyperShift"] != true);
                mapping["isHyperShift"] = json!(true);
                entries.push(mapping);
            }
        } else if reset_both_layers && !self.hypershift {
            entries.retain(|m| m["inputID"] != input || m["isHyperShift"] != true);
        }
        // Never accept a profile without a standard Click. Source normally
        // prevents selecting the last Click; guard stale drafts as well.
        let buttons = self.spec.groups[0]["buttonList"]
            .as_array()
            .expect("190 buttons");
        let has_click = buttons.iter().any(|button| {
            entries
                .iter()
                .find(|m| m["inputID"] == button["inputID"] && m["isHyperShift"] != true)
                .map_or(button["defaultValue"] == "LEFT_CLICK", |m| {
                    m["mouseGroup"]["mouseAssignment"] == "Click"
                })
        });
        if !has_click {
            self.mapping_190_state.primary_error = true;
            cx.emit(MouseMappingSaveRejected::PrimaryClickRequired);
            cx.notify();
            return false;
        }
        self.draft["mappings"] = json!(entries);
        self.mapping_190_state.changed = false;
        cx.emit(MouseProductChanged);
        true
    }
    fn mapping_190_options(&self) -> (&'static str, Vec<(&'static str, &'static str)>) {
        match self.mapping_190_state.function.as_str() {
            "MOUSE_FUNCTION" => ("mouseAssignment", MOUSE.to_vec()),
            "MULTIMEDIA" => ("multimediaAssignment", MEDIA.to_vec()),
            "WINDOWS_SHORTCUT" => ("windowsShortcutAssignment", WINDOWS.to_vec()),
            "KEYBOARD_FUNCTION" => {
                let group = self.mapping_190_state.value["keyGroup"]
                    .as_u64()
                    .unwrap_or(0) as usize;
                let kind = [
                    "Key Recording",
                    "Alphanumeric",
                    "Function",
                    "Numpad",
                    "Navigation",
                    "Modifiers",
                    "Symbols",
                ]
                .get(group)
                .copied()
                .unwrap_or("Key Recording");
                (
                    "key",
                    KEYS.iter()
                        .filter(|(_, _, k)| *k == kind)
                        .map(|(id, label, _)| (*id, *label))
                        .collect(),
                )
            }
            "SENSITIVITY" => {
                let cycle = self.mapping_input.as_deref() == Some("CycleUpSensitivityStages");
                (
                    "sensitivityAssignment",
                    SENSITIVITY
                        .iter()
                        .copied()
                        .filter(|(id, _)| {
                            !(cycle && matches!(*id, "DPI_Clutch" | "DPI_OnTheFly")
                                || self.hypershift && *id == "DPI_OnTheFly")
                        })
                        .collect(),
                )
            }
            _ => ("", vec![]),
        }
    }
    fn mapping_190_dropdown(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let (key, options) = self.mapping_190_options();
        let state = &self.mapping_190_state;
        let selected = if key == "key" {
            state
                .selected_symbol
                .as_deref()
                .or_else(|| state.value[key].as_str())
                .unwrap_or("")
        } else {
            state.value[key].as_str().unwrap_or("")
        };
        let label = options
            .iter()
            .find(|(id, _)| *id == selected)
            .map(|(_, label)| *label)
            .unwrap_or_else(|| options.first().map(|(_, l)| *l).unwrap_or(""));
        let selected_index = options
            .iter()
            .position(|(id, _)| *id == selected)
            .unwrap_or(0);
        let items = options
            .into_iter()
            .map(|(id, label)| {
                BaseButton::new(SharedString::from(format!("mouse-190-option-{id}")))
                    .w_full()
                    .h(surface::css(25.))
                    .min_h(surface::css(25.))
                    .flex_shrink_0()
                    .px(surface::css(4.))
                    .py_0()
                    .text_color(rgb(if selected == id { 0x44d62c } else { 0xcccccc }))
                    .bg(rgb(0x000000))
                    .hover(|style| style.bg(rgba(0xffffff1a)))
                    .child(div().w_full().text_ellipsis().text_left().child(t(label)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.mapping_190_state.value[key] = json!(id);
                        if key == "key" {
                            this.mapping_190_state.selected_symbol = None;
                            if let Some((symbol, base)) =
                                SHIFTED.iter().find(|(symbol, _)| *symbol == id)
                            {
                                this.mapping_190_state.selected_symbol = Some(symbol.to_string());
                                this.mapping_190_state.value["key"] = json!(base);
                                let mut modifiers = this.mapping_190_state.value["modifiers"]
                                    .as_array()
                                    .cloned()
                                    .unwrap_or_default();
                                modifiers
                                    .retain(|m| m != "KEY_RIGHT_SHIFT" && m != "KEY_LEFT_SHIFT");
                                modifiers.push(json!("KEY_LEFT_SHIFT"));
                                this.mapping_190_state.value["modifiers"] = json!(modifiers);
                            }
                        }
                        if key == "multimediaAssignment" {
                            this.mapping_190_state
                                .value
                                .as_object_mut()
                                .expect("mapping group")
                                .remove("useTurbo");
                            if matches!(id, "VolumeUp" | "VolumeDown") {
                                this.mapping_190_state.value["useTurbo"] = json!(true);
                            }
                        }
                        this.mapping_190_state.changed = true;
                        this.mapping_190_state.can_save = true;
                        this.mapping_190_state.option_open = false;
                        cx.notify();
                    }))
                    .into_any_element()
            })
            .collect();
        div()
            .w_full()
            .mb(surface::css(10.))
            .child(self.mapping_190_source_dropdown(
                "option",
                t(label).to_string(),
                selected_index,
                items,
                window,
                cx,
            ))
            .into_any_element()
    }
    fn mapping_190_turbo_disabled(&self) -> bool {
        matches!(
            self.mapping_input.as_deref(),
            Some("ScrollUp" | "ScrollDown" | "CycleUpSensitivityStages")
        )
    }
    fn mapping_190_turbo_numeric(&self) -> i64 {
        let speed = &self.mapping_190_state.turbo_speed;
        speed
            .as_i64()
            .or_else(|| speed.as_str().and_then(|v| v.parse::<i64>().ok()))
            .unwrap_or(0)
            .clamp(1, 20)
    }
    fn mapping_190_turbo_value(
        &mut self,
        value: Value,
        canonical: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !matches!(
            self.mapping_190_state.function.as_str(),
            "MOUSE_FUNCTION" | "KEYBOARD_FUNCTION"
        ) {
            return;
        }
        let changed = self.mapping_190_state.turbo_speed != value;
        self.mapping_190_state.turbo_speed = value.clone();
        if self.mapping_190_state.value["turboMode"]["isTurbo"] == true {
            self.mapping_190_state.value["turboMode"]["keysPerSecond"] = value.clone();
        }
        let keyboard_ready = self.mapping_190_state.value["key"]
            .as_str()
            .is_some_and(|key| !key.is_empty());
        // MapMouse.changeTurboSpeed always enables save/dirty. MapKeyboard
        // only calls enableSave when speed changed and keyValue is nonempty.
        if self.mapping_190_state.function == "MOUSE_FUNCTION" || changed && keyboard_ready {
            self.mapping_190_state.changed = true;
            self.mapping_190_state.can_save = true;
        }
        self.mapping_190_state.syncing_controls = true;
        if canonical {
            if let Some(input) = &self.mapping_190_state.turbo_number {
                input.update(cx, |input, cx| {
                    input.set_value(
                        value
                            .as_str()
                            .map(ToOwned::to_owned)
                            .unwrap_or_else(|| value.to_string()),
                        window,
                        cx,
                    )
                });
            }
        }
        let numeric = self.mapping_190_turbo_numeric();
        if let Some(slider) = &self.mapping_190_state.turbo {
            slider.update(cx, |slider, cx| {
                slider.set_value(numeric as f32, window, cx)
            });
        }
        self.mapping_190_state.syncing_controls = false;
        if canonical && matches!(numeric, 1 | 20) {
            self.mapping_190_state.turbo_repeat = None;
        }
        cx.notify();
    }
    fn mapping_190_turbo_step(
        &mut self,
        delta: i64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.mapping_190_state.value["turboMode"]["isTurbo"] != true {
            return false;
        }
        let text = self
            .mapping_190_state
            .turbo_number
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_else(|| self.mapping_190_state.turbo_speed.to_string());
        let current = text.parse::<i64>().unwrap_or(0);
        // Current module4230 volumeUp uses JS + on its handleChange string:
        // typing "7" then increment yields "71", clamped to 20. Decrement
        // coerces through subtraction. Keep this verified behavior.
        let next = if delta > 0 && self.mapping_190_state.turbo_typed {
            format!("{text}1").parse::<i64>().unwrap_or(0)
        } else {
            current + delta
        };
        let value = next.clamp(1, 20);
        self.mapping_190_state.turbo_typed = false;
        self.mapping_190_turbo_value(json!(value), true, window, cx);
        !matches!(value, 1 | 20)
    }
    fn mapping_190_turbo_repeat(
        &mut self,
        delta: i64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.mapping_190_state.turbo_repeat = None;
        if !self.mapping_190_turbo_step(delta, window, cx) {
            return;
        }
        // Current4230 handleContinuous uses setInterval(...,300); unmount,
        // release, leaving the arrow, or a numeric boundary cancels it.
        let task = cx.spawn_in(window, async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(300))
                    .await;
                let more = view
                    .update_in(cx, |view, window, cx| {
                        view.mapping_190_turbo_step(delta, window, cx)
                    })
                    .unwrap_or(false);
                if !more {
                    break;
                }
            }
        });
        self.mapping_190_state.turbo_repeat = Some(task);
    }
    fn mapping_190_requires_synapse(&self) -> bool {
        let state = &self.mapping_190_state;
        // Current9119.checkRequiredSynapse, not the unused getIsRequireSynapse:
        // DeviceInfo.isOBMDevice=true; R/D overrides are not exported by8193.
        // 9937.lYZ minus OBM x excludes all repeat scrolls and ScrollUp/Down.
        match state.function.as_str() {
            "MOUSE_FUNCTION" => {
                matches!(
                    state.value["mouseAssignment"].as_str(),
                    Some("ScrollLeft" | "ScrollRight")
                ) || state.value["mouseAssignment"] == "DoubleClick"
                    && state.value["turboMode"]["isTurbo"] == true
            }
            // MapMedia.changeOption uses9937.lYZ; source190 OBM supports the
            // remaining multimedia options. Source400 applies8193.P, which
            // excludes every sensitivity label except ON_THE_FLY_SENSITIVITY.
            "MULTIMEDIA" => matches!(
                state.value["multimediaAssignment"].as_str(),
                Some("MicVolumeUp" | "MicVolumeDown" | "MuteMic" | "MuteAll")
            ),
            "SENSITIVITY" => state.value["sensitivityAssignment"] == "DPI_OnTheFly",
            // Ut.getRequireSynapse/notReqSynapseConfig and these chunks pass
            // enableSave(...,...,true). This means host dependence, not Save
            // success or an observed running service.
            "WINDOWS_SHORTCUT" | "TEXT_FUNCTION" | "LAUNCH_PROGRAM" => true,
            _ => false,
        }
    }
    fn mapping_190_synapse_hint(&self) -> AnyElement {
        let text = t("REQUIRES_RAZER_SYNAPSE");
        let content = if let Some(prefix) = text.strip_suffix("Razer Synapse") {
            v_flex()
                .child(prefix.to_owned())
                .child("Razer Synapse")
                .into_any_element()
        } else {
            div().child(text).into_any_element()
        };
        h_flex()
            .relative()
            .items_center()
            .w(surface::css(210.))
            .h(surface::css(44.))
            .mt(surface::css(10.))
            .mb(surface::css(20.))
            .border_1()
            .border_color(rgb(0x707070))
            .rounded(surface::css(3.))
            .child(
                img("synapse/mapping-190-requires-synapse.svg")
                    .size(surface::css(24.))
                    .ml(surface::css(10.))
                    .flex_shrink_0(),
            )
            .child(
                div()
                    .w(surface::css(142.))
                    .mx(surface::css(5.))
                    .text_color(rgb(0x707070))
                    .text_size(surface::css(14.))
                    .line_height(surface::css(14.))
                    .flex_shrink_0()
                    .child(content),
            )
            .child(
                div().mr(surface::css(10.)).flex_shrink_0().child(
                    SourceTooltip::new(
                        "mouse-190-requires-synapse-help",
                        t("REQUIRES_RAZER_SYNAPSE_TOOLTIP"),
                        250.,
                    )
                    .trigger(|hovered, _, _| {
                        BaseButton::new("mouse-190-requires-synapse-trigger")
                            .size(surface::css(14.))
                            .p_0()
                            .rounded_full()
                            .bg(if hovered {
                                rgba(0xffffff4d)
                            } else {
                                rgb(0x4a4a4a)
                            })
                            .accessibility_label(t("REQUIRES_RAZER_SYNAPSE_TOOLTIP"))
                            .child(img("synapse/automation-tooltip_questionmark.svg").size_full())
                            .into_any_element()
                    }),
                ),
            )
            .into_any_element()
    }
    fn mapping_190_keyboard(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let state = &self.mapping_190_state;
        let key_group = state.value["keyGroup"].as_u64().unwrap_or(0) as usize;
        let groups = [
            "KB_KEY_RECORDING",
            "KB_ALPHANUMERIC",
            "KB_FUNCTION",
            "KB_NUMPAD",
            "KB_NAVIGATION",
            "KB_MODIFIERS",
            "KB_SYMBOLS",
        ];
        let group_options = groups
            .into_iter()
            .enumerate()
            .map(|(index, label)| {
                BaseButton::new(SharedString::from(format!("mouse-190-keygroup-{index}")))
                    .w_full()
                    .h(surface::css(25.))
                    .min_h(surface::css(25.))
                    .flex_shrink_0()
                    .px(surface::css(4.))
                    .py_0()
                    .text_color(rgb(if key_group == index {
                        0x44d62c
                    } else {
                        0xcccccc
                    }))
                    .bg(rgb(0x000000))
                    .hover(|style| style.bg(rgba(0xffffff1a)))
                    .child(div().w_full().text_ellipsis().text_left().child(t(label)))
                    .into_any_element()
            })
            .collect();
        let mut body =
            v_flex()
                .w_full()
                .mb(surface::css(20.))
                .child(self.mapping_190_source_dropdown(
                    "group",
                    t(groups[key_group.min(6)]).to_string(),
                    key_group,
                    group_options,
                    window,
                    cx,
                ));
        if key_group == 0 {
            let key = state.value["key"].as_str().unwrap_or("");
            let label: SharedString = if key.is_empty() {
                t("TEXT_CLICK_TO_RECORD").into()
            } else {
                state.value["modifiers"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .chain(std::iter::once(key))
                    .map(|id| {
                        KEYS.iter()
                            .find(|(k, _, _)| *k == id)
                            .map(|(_, label, _)| *label)
                            .unwrap_or(id)
                    })
                    .collect::<Vec<_>>()
                    .join(" + ")
                    .into()
            };
            let mut recorder = div().id("mouse-190-key-recorder");
            if let Some(focus) = &state.focus {
                recorder = recorder.track_focus(focus);
            }
            body = body.child(
                recorder
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        if !this.mapping_190_state.recording || event.is_held {
                            return;
                        }
                        cx.stop_propagation();
                        let Some(key) =
                            crate::features::workspace::canonical_key(&event.keystroke.key)
                        else {
                            return;
                        };
                        if !KEYS.iter().any(|(id, _, _)| *id == key) {
                            return;
                        }
                        let held = event.keystroke.modifiers;
                        let modifiers = [
                            (held.control, "KEY_LEFT_CTRL"),
                            (held.alt, "KEY_LEFT_ALT"),
                            (held.shift, "KEY_LEFT_SHIFT"),
                            (held.platform, "KEY_LEFT_GUI"),
                        ]
                        .into_iter()
                        .filter(|(on, id)| *on && *id != key)
                        .map(|(_, id)| id)
                        .collect::<Vec<_>>();
                        this.mapping_190_state.value["key"] = json!(key);
                        this.mapping_190_state.value["modifiers"] = json!(modifiers);
                        this.mapping_190_state.changed = true;
                        this.mapping_190_state.can_save = true;
                        this.mapping_190_state.recording = false;
                        cx.notify();
                    }))
                    .child(
                        BaseButton::new("mouse-190-record-key")
                            .w_full()
                            .h(surface::css(27.))
                            .border_1()
                            .border_color(rgb(if state.recording { 0x44d62c } else { 0x5d5d5d }))
                            .child(label)
                            .on_click(cx.listener(|this, _, window, cx| {
                                this.mapping_190_state.recording = true;
                                if let Some(focus) = &this.mapping_190_state.focus {
                                    window.focus(focus, cx)
                                }
                                cx.notify();
                            })),
                    ),
            );
        } else {
            body = body.child(self.mapping_190_dropdown(window, cx));
            if key_group != 5 {
                body = body.child(
                    Checkbox::new("mouse-190-include-modifier")
                        .label(t("TEXT_INCLUDE_MODIFIER"))
                        .checked(state.include_modifiers)
                        .on_click(cx.listener(|this, on: &bool, _, cx| {
                            this.mapping_190_state.include_modifiers = *on;
                            if !on {
                                this.mapping_190_state.value["modifiers"] = json!([]);
                            }
                            this.mapping_190_state.changed = true;
                            this.mapping_190_state.can_save = true;
                            cx.notify();
                        })),
                );
                if state.include_modifiers {
                    for (family, left, right) in [
                        ("SHIFT", "KEY_LEFT_SHIFT", Some("KEY_RIGHT_SHIFT")),
                        ("CTRL", "KEY_LEFT_CTRL", Some("KEY_RIGHT_CTRL")),
                        ("ALT", "KEY_LEFT_ALT", Some("KEY_RIGHT_ALT")),
                        ("WIN", "KEY_LEFT_GUI", None),
                    ] {
                        body =
                            body.child(h_flex().justify_between().children(
                                std::iter::once(left).chain(right).map(|id| {
                                    let selected = state.value["modifiers"]
                                        .as_array()
                                        .is_some_and(|m| m.iter().any(|m| m == id));
                                    BaseButton::new(SharedString::from(format!(
                                        "mouse-190-modifier-{id}"
                                    )))
                                    .w(surface::css(100.))
                                    .h(surface::css(27.))
                                    .selected(selected)
                                    .child(family)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let mut modifiers =
                                            this.mapping_190_state.value["modifiers"]
                                                .as_array()
                                                .cloned()
                                                .unwrap_or_default();
                                        let remove = modifiers.iter().any(|m| m == id);
                                        let family = id
                                            .strip_prefix("KEY_LEFT_")
                                            .or_else(|| id.strip_prefix("KEY_RIGHT_"))
                                            .unwrap_or(id);
                                        modifiers.retain(|m| {
                                            m.as_str().is_none_or(|m| !m.ends_with(family))
                                        });
                                        if !remove {
                                            modifiers.push(json!(id));
                                        }
                                        this.mapping_190_state.value["modifiers"] =
                                            json!(modifiers);
                                        this.mapping_190_state.changed = true;
                                        this.mapping_190_state.can_save = true;
                                        cx.notify();
                                    }))
                                }),
                            ));
                    }
                }
            }
        }
        body.into_any_element()
    }
    fn mapping_190_body(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let state = &self.mapping_190_state;
        let mut body = v_flex()
            .w(surface::css(250.))
            .p(surface::css(20.))
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .child(
                div()
                    .text_color(rgb(0x44d62c))
                    .font_family("RazerF5")
                    .text_size(surface::css(16.))
                    .mb(surface::css(20.))
                    .child(t(&state.function)),
            );
        if state.function != "KEYBOARD_FUNCTION" && !self.mapping_190_options().1.is_empty() {
            body = body.child(self.mapping_190_dropdown(window, cx));
        }
        match state.function.as_str() {
            "DEFAULT" => {
                let label = self
                    .mapping_input
                    .as_deref()
                    .and_then(|i| self.mapping_190_button(i))
                    .and_then(|b| b["defaultValue"].as_str())
                    .unwrap_or("DEFAULT");
                body = body.child(
                    div()
                        .border_1()
                        .border_color(rgb(0x5d5d5d))
                        .p(surface::css(5.))
                        .mb(surface::css(20.))
                        .child(t(label)),
                );
            }
            "RAZER_HYPERSHIFT" => {
                body = body.child(
                    div()
                        .mb(surface::css(20.))
                        .text_color(rgb(0x707070))
                        .child(t("RAZER_HYPERSHIFT_MSG1"))
                        .child(
                            div()
                                .mt(surface::css(20.))
                                .child(t("RAZER_HYPERSHIFT_MSG2")),
                        ),
                )
            }
            "KEYBOARD_FUNCTION" => body = body.child(self.mapping_190_keyboard(window, cx)),
            "TEXT_FUNCTION" | "LAUNCH_PROGRAM" => {
                if state.function == "LAUNCH_PROGRAM" {
                    body = body.child(
                        h_flex()
                            .gap(surface::css(8.))
                            .mb(surface::css(10.))
                            .children([("path", "PROGRAM"), ("url", "WEBSITE")].into_iter().map(
                                |(key, label)| {
                                    BaseButton::new(SharedString::from(format!(
                                        "mouse-190-launch-{key}"
                                    )))
                                    .child(t(label))
                                    .selected(state.value.get(key).is_some())
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.mapping_190_state.value = json!({key:""});
                                        this.mapping_190_state.original["launchGroup"] =
                                            this.mapping_190_state.value.clone();
                                        this.mapping_190_select_function(
                                            "LAUNCH_PROGRAM",
                                            window,
                                            cx,
                                        );
                                    }))
                                },
                            )),
                    );
                }
                if let Some(input) = &state.input {
                    body = body.child(Input::new(input).w_full());
                }
                if state.function == "TEXT_FUNCTION" {
                    body = body.child(div().mt(surface::css(5.)).text_color(rgb(0x999999)).child(
                        format!(
                                "{}/250",
                                state.value["text"]
                                    .as_str()
                                    .unwrap_or("")
                                    .encode_utf16()
                                    .count()
                            ),
                    ));
                }
            }
            "SENSITIVITY" if state.value["sensitivityAssignment"] == "DPI_Clutch" => {
                for axis in ["x", "y"] {
                    if axis == "y" && state.value["independent"] != true {
                        continue;
                    }
                    let current = state.value[axis].as_i64().unwrap_or(800);
                    body = body.child(
                        h_flex()
                            .gap(surface::css(5.))
                            .mb(surface::css(10.))
                            .child(axis.to_uppercase())
                            .child(current.to_string())
                            .children([(-1, "?"), (1, "+")].into_iter().map(|(delta, label)| {
                                BaseButton::new(SharedString::from(format!(
                                    "mouse-190-clutch-{axis}-{delta}"
                                )))
                                .child(label)
                                .on_click(cx.listener(
                                    move |this, _, _, cx| {
                                        let value = (this.mapping_190_state.value[axis]
                                            .as_i64()
                                            .unwrap_or(800)
                                            + delta)
                                            .clamp(100, 45000);
                                        this.mapping_190_state.value[axis] = json!(value);
                                        if axis == "x"
                                            && this.mapping_190_state.value["independent"] != true
                                        {
                                            this.mapping_190_state.value["y"] = json!(value);
                                        }
                                        this.mapping_190_state.changed = true;
                                        this.mapping_190_state.can_save = true;
                                        cx.notify();
                                    },
                                ))
                            })),
                    );
                }
                body = body.child(
                    Checkbox::new("mouse-190-clutch-xy")
                        .label("X-Y")
                        .checked(state.value["independent"] == true)
                        .on_click(cx.listener(|this, checked: &bool, _, cx| {
                            this.mapping_190_state.value["independent"] = json!(*checked);
                            this.mapping_190_state.changed = true;
                            this.mapping_190_state.can_save = true;
                            cx.notify();
                        })),
                );
            }
            // Observations for remaining functions are absent; inspect
            // State.missing_observation instead of presenting a source disable.
            _ => {}
        }
        if matches!(
            state.function.as_str(),
            "MOUSE_FUNCTION" | "KEYBOARD_FUNCTION"
        ) && !self.mapping_190_turbo_disabled()
        {
            let enabled = state.value["turboMode"]["isTurbo"] == true;
            body=body.child(Checkbox::new("mouse-190-mapping-turbo").label(t("ENABLE_TURBO")).checked(enabled)
                .on_click(cx.listener(|this,on:&bool,_,cx|{
                    this.mapping_190_state.turbo_repeat=None;
                    if *on{this.mapping_190_state.value["turboMode"]=json!({"isTurbo":true,"keysPerSecond":this.mapping_190_state.turbo_speed});}
                    else{this.mapping_190_state.value.as_object_mut().expect("mapping value").remove("turboMode");}
                    if this.mapping_190_state.function=="MOUSE_FUNCTION"||this.mapping_190_state.value["key"].as_str().is_some_and(|key|!key.is_empty()){
                        this.mapping_190_state.changed=true;this.mapping_190_state.can_save=true;
                    }
                    cx.notify();
                })));
            if enabled {
                if let Some(slider) = &state.turbo {
                    body = body
                        .child(div().mt(surface::css(10.)).child(t("SPEED_PER_SEC")))
                        .children(state.turbo_number.as_ref().map(|number| {
                            div()
                                .relative()
                                .w(surface::css(60.))
                                .h(surface::css(27.))
                                .border_1()
                                .border_color(rgb(0x5d5d5d))
                                .on_key_down(cx.listener(
                                    |this, event: &KeyDownEvent, window, cx| {
                                        match event.keystroke.key.as_str() {
                                            "up" => {
                                                this.mapping_190_turbo_step(1, window, cx);
                                                cx.stop_propagation();
                                            }
                                            "down" => {
                                                this.mapping_190_turbo_step(-1, window, cx);
                                                cx.stop_propagation();
                                            }
                                            "enter" | "escape" => {
                                                window.blur(cx);
                                                cx.stop_propagation();
                                            }
                                            _ => {}
                                        }
                                    },
                                ))
                                .on_scroll_wheel(cx.listener(
                                    |this, event: &ScrollWheelEvent, window, cx| {
                                        let focused = this
                                            .mapping_190_state
                                            .turbo_number
                                            .as_ref()
                                            .is_some_and(|input| {
                                                input.focus_handle(cx).is_focused(window)
                                            });
                                        if !focused {
                                            return;
                                        }
                                        cx.stop_propagation();
                                        let delta = event.delta.pixel_delta(window.line_height()).y;
                                        if delta > px(0.) {
                                            this.mapping_190_turbo_step(1, window, cx);
                                        } else if delta < px(0.) {
                                            this.mapping_190_turbo_step(-1, window, cx);
                                        }
                                    },
                                ))
                                .child(
                                    Input::new(number)
                                        .appearance(false)
                                        .bordered(false)
                                        .focus_bordered(false)
                                        .w_full()
                                        .h(surface::css(25.))
                                        .p_0()
                                        .pl(surface::css(5.))
                                        .pr(surface::css(18.)),
                                )
                                .child(
                                    v_flex()
                                        .absolute()
                                        .right_0()
                                        .top_0()
                                        .w(surface::css(18.))
                                        .children([(1, "up"), (-1, "down")].into_iter().map(
                                            |(delta, direction)| {
                                                BaseButton::new(SharedString::from(format!(
                                                    "mouse-190-turbo-{direction}"
                                                )))
                                                .w_full()
                                                .h(surface::css(12.))
                                                .p_0()
                                                .child(
                                                    img(SharedString::from(format!(
                                                        "synapse/stepper-{direction}.svg"
                                                    )))
                                                    .w(surface::css(8.))
                                                    .h(surface::css(4.)),
                                                )
                                                .disabled(
                                                    state.turbo_speed.as_i64()
                                                        == Some(if delta > 0 { 20 } else { 1 }),
                                                )
                                                .when(
                                                    state.turbo_speed.as_i64()
                                                        == Some(if delta > 0 { 20 } else { 1 }),
                                                    |button| button.opacity(0.3),
                                                )
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    cx.listener(move |this, _, window, cx| {
                                                        this.mapping_190_turbo_repeat(
                                                            delta, window, cx,
                                                        )
                                                    }),
                                                )
                                                .on_mouse_up(
                                                    MouseButton::Left,
                                                    cx.listener(|this, _, _, _| {
                                                        this.mapping_190_state.turbo_repeat = None
                                                    }),
                                                )
                                                .on_mouse_up_out(
                                                    MouseButton::Left,
                                                    cx.listener(|this, _, _, _| {
                                                        this.mapping_190_state.turbo_repeat = None
                                                    }),
                                                )
                                                .on_hover(cx.listener(|this, on: &bool, _, _| {
                                                    if !on {
                                                        this.mapping_190_state.turbo_repeat = None;
                                                    }
                                                }))
                                                .on_click(cx.listener(
                                                    move |this, event: &ClickEvent, window, cx| {
                                                        if !matches!(event, ClickEvent::Mouse(_)) {
                                                            this.mapping_190_turbo_step(
                                                                delta, window, cx,
                                                            );
                                                        }
                                                    },
                                                ))
                                            },
                                        )),
                                )
                        }))
                        .child(Slider::new(slider))
                        .child(h_flex().justify_between().child("1").child("20"));
                }
            }
        }
        if self.mapping_190_requires_synapse() {
            body = body.child(self.mapping_190_synapse_hint());
        }
        body.child(
            h_flex()
                .mt(surface::css(20.))
                .gap(surface::css(10.))
                .child(
                    Button::new("mouse-190-mapping-cancel")
                        .label(t("CANCEL"))
                        .outline()
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.mapping_190_next(Next::Close, window, cx)
                        })),
                )
                .child(
                    Button::new("mouse-190-mapping-save")
                        .label(t("SAVE"))
                        .primary()
                        .disabled(state.source_save_eligibility() != Some(true))
                        .on_click(cx.listener(|this, _, window, cx| {
                            if this.mapping_190_save(cx) {
                                this.mapping_190_next(Next::Close, window, cx)
                            }
                        })),
                ),
        )
        .into_any_element()
    }
    pub(super) fn mapping_190_drawer(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(input) = self.mapping_input.as_deref() else {
            return div().into_any_element();
        };
        let button = self.mapping_190_button(input).expect("active190 button");
        let state = &self.mapping_190_state;
        let expanded = state.expanded;
        let height =
            (f32::from(window.viewport_size().height) * 16. / f32::from(window.rem_size()) - 140.)
                .clamp(310., 570.);
        let menu_width = gpui_kit::base::motion::transition(
            ElementId::from(("mouse-190-menu-width", cx.entity_id())),
            if expanded { 250. } else { 40. },
            gpui_kit::base::motion::Transition::new(Duration::from_millis(200))
                .easing(gpui_kit::base::motion::Easing::Ease),
            window,
            cx,
        );
        let menu_bounds = state.menu_bounds.clone();
        let menu = v_flex()
            .id("mouse-190-functions")
            .absolute()
            .left_0()
            .top_0()
            .w(surface::css(menu_width))
            .h_full()
            .overflow_hidden()
            .when(expanded, |menu| menu.overflow_y_scroll())
            .bg(rgb(0x222222))
            .on_prepaint(move |bounds, _, _| menu_bounds.set(bounds))
            .on_hover(cx.listener(|this, on: &bool, window, cx| {
                this.mapping_190_state.expanded = *on;
                let state = &mut this.mapping_190_state;
                state.menu_height = None;
                let menu = state.menu_bounds.get().size.height;
                let body = state.outer_body_bounds.get().size.height;
                if *on
                    && menu > px(0.)
                    && body > px(0.)
                    && menu + window.rem_size() * (46. / 16.) > body
                {
                    state.menu_height = Some(body - window.rem_size() * (46. / 16.));
                }
                // Current5316 stores keyMappingStyle/isShowScroll but its mounted
                // render never reads either. Only CSS :hover enables overflow.
                cx.notify();
            }))
            .children(
                button["functionList"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .filter(|f| !self.hypershift || *f != "RAZER_HYPERSHIFT")
                    .map(|fun| {
                        let fun = fun.to_owned();
                        let active = state.function == fun;
                        BaseButton::new(SharedString::from(format!("mouse-190-function-{fun}")))
                            .w_full()
                            .h(surface::css(40.))
                            .p_0()
                            .bg(if active { rgb(0x111111) } else { rgb(0x222222) })
                            .text_color(rgb(if active { 0x44d62c } else { 0xcccccc }))
                            .hover(|s| s.bg(rgb(0x383838)))
                            .child(
                                h_flex()
                                    .w(surface::css(250.))
                                    .flex_shrink_0()
                                    .child(
                                        img(SharedString::from(icon(&fun, active)))
                                            .size(surface::css(40.)),
                                    )
                                    .child(div().child(t(&fun))),
                            )
                            .on_click(cx.listener(move |this, _, window, cx| {
                                this.mapping_190_select_function(&fun, window, cx)
                            }))
                    }),
            );
        div()
            .id("mouse-190-keymap")
            .relative()
            .occlude()
            .w(surface::css(292.))
            .h(surface::css(height))
            .bg(rgb(0x111111))
            .border_1()
            .border_color(rgb(0x5d5d5d))
            .rounded(surface::css(5.))
            .child(
                deferred(
                    h_flex()
                        .absolute()
                        .top(surface::css(-36.))
                        .h(surface::css(36.))
                        .w_full()
                        .items_center()
                        .justify_center()
                        .bg(rgb(0x222222))
                        .border_b_1()
                        .border_color(rgb(0x5d5d5d))
                        .text_color(rgb(0x999999))
                        .child(t(button["defaultValue"].as_str().unwrap_or("")))
                        .child(
                            BaseButton::new("mouse-190-mapping-close")
                                .absolute()
                                .right_0()
                                .top_0()
                                .size(surface::css(36.))
                                .p_0()
                                .child(img("synapse/mapping-close.svg").size(surface::css(20.)))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.mapping_190_request(Next::Close, window, cx)
                                })),
                        ),
                )
                .with_priority(106),
            )
            .child(
                div()
                    .relative()
                    .h_full()
                    .child(
                        v_flex()
                            .id("mouse-190-function-body")
                            .ml(surface::css(40.))
                            .w(surface::css(250.))
                            .h_full()
                            .overflow_y_scroll()
                            .child(self.mapping_190_body(window, cx)),
                    )
                    .child(deferred(menu).with_priority(105)),
            )
            .into_any_element()
    }
    pub(super) fn mapping_190_confirmation(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let open = self.mapping_190_state.pending.is_some();
        // gO mounts the backdrop even when hidden. Prime the persistent opacity
        // channel every frame; visibility:hidden hides immediately on close.
        let opacity = gpui_kit::base::motion::transition(
            ElementId::from(("mouse-190-save-alert-opacity", cx.entity_id())),
            if open { 1. } else { 0. },
            gpui_kit::base::motion::Transition::new(Duration::from_millis(100))
                .easing(gpui_kit::base::motion::Easing::Linear),
            window,
            cx,
        );
        if !open {
            return None;
        }
        let discard = t("DONT_SAVE");
        let long_discard = discard.encode_utf16().count() > 9;
        let dialog = v_flex()
            .absolute()
            .left_0()
            .top_0()
            .w(surface::css(400.))
            .px(surface::css(30.))
            .py(surface::css(20.))
            .bg(rgb(0x111111))
            .border_1()
            .border_color(rgb(0x44d62c))
            .rounded(surface::css(5.))
            .text_center()
            .font_family("Roboto")
            .text_color(rgb(0xcccccc))
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .opacity(opacity)
            .child(
                BaseButton::new("mouse-190-confirm-close")
                    .absolute()
                    .right(surface::css(8.))
                    .top(surface::css(8.))
                    .size(surface::css(20.))
                    .p_0()
                    .bg(rgba(0))
                    .child(img("synapse/mapping-close.svg").size_full())
                    .on_click(cx.listener(|this, _, _, cx| {
                        // dismissSave(true) preserves the outer dirty observation.
                        this.mapping_190_state.pending = None;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .w_full()
                    .mb(surface::css(20.))
                    .font_family("RazerF5")
                    .text_color(rgb(0x44d62c))
                    .text_size(surface::css(16.))
                    .line_height(surface::css(19.))
                    .child(t("SAVE_REMAPPED_BUTTON_HEADER").to_uppercase()),
            )
            .child(div().w_full().child(t("SAVE_REMAPPED_BUTTON_MSG1")))
            // Two source <br> nodes after localized inline text create one
            // blank17px line between the paragraphs.
            .child(div().h(surface::css(17.)).flex_shrink_0())
            .child(div().w_full().child(t("SAVE_REMAPPED_BUTTON_MSG2")))
            .child(
                h_flex()
                    .mr(surface::css(-10.))
                    .mt(surface::css(20.))
                    .justify_center()
                    .child(
                        alert_button("mouse-190-confirm-discard", window, cx)
                            .min_w(surface::css(100.))
                            .h(surface::css(27.))
                            .p_0()
                            .pt(surface::css(6.))
                            .pb(surface::css(7.))
                            .pl(surface::css(if long_discard { 16. } else { 6. }))
                            .pr(surface::css(if long_discard { 16. } else { 10. }))
                            .mr(surface::css(10.))
                            .border_1()
                            .border_color(rgb(0x000000))
                            .rounded(surface::css(3.))
                            .bg(rgb(0x707070))
                            .text_color(rgb(0xffffff))
                            .text_size(surface::css(12.))
                            .line_height(surface::css(14.))
                            .child(discard.to_uppercase())
                            .on_click(cx.listener(|this, _, window, cx| {
                                if let Some(next) = this.mapping_190_state.pending.take() {
                                    this.mapping_190_next(next, window, cx);
                                }
                            })),
                    )
                    .child(
                        alert_button("mouse-190-confirm-save", window, cx)
                            .min_w(surface::css(100.))
                            .h(surface::css(27.))
                            .p_0()
                            .pt(surface::css(6.))
                            .pb(surface::css(7.))
                            .pl(surface::css(6.))
                            .pr(surface::css(10.))
                            .mr(surface::css(10.))
                            .border_1()
                            .border_color(rgb(0x000000))
                            .rounded(surface::css(3.))
                            .bg(rgb(0x44d62c))
                            .text_color(rgb(0x000000))
                            .text_size(surface::css(12.))
                            .line_height(surface::css(14.))
                            .child(t("SAVE").to_uppercase())
                            // gO Save has no disabled class or canSave predicate.
                            .on_click(cx.listener(|this, _, window, cx| {
                                if this.mapping_190_save(cx) {
                                    if let Some(next) = this.mapping_190_state.pending.take() {
                                        this.mapping_190_next(next, window, cx);
                                    }
                                }
                            })),
                    ),
            )
            .into_any_element();
        let backdrop = div()
            .w(window.viewport_size().width)
            .h(window.viewport_size().height)
            .occlude()
            .bg(rgba(0x00000080))
            .opacity(opacity)
            .into_any_element();
        Some(ConfirmationLayer { backdrop, dialog }.into_any_element())
    }
}
