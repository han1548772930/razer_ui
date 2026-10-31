//! Source dropdown presentations over Kit's select, popup and list behavior.
//!
//! SelectState remains the committed value and event source. The owner supplies its current
//! options because Kit 0.7 does not expose SelectState's delegate or its menu renderer.
use gpui_kit::base::motion::{self, Easing, Presence, Sequence, Transition};
use gpui_kit::base::{DeferredPopover, GlobalState, Popover, Popup, Select as BaseSelect};
use gpui_kit::component::{
    ActiveTheme, Disableable, ElementExt, IndexPath, Selectable, StyledExt,
    list::{List, ListDelegate, ListState},
    select::{SelectEvent, SelectItem, SelectState},
};
use gpui_kit::{prelude::FluentBuilder as _, *};

use super::{
    surface::css,
    theme::{AlexaColors, DropdownColors},
};
use crate::keyboard_navigation::{self, FocusBehavior, FocusRing, focus_target};
use std::time::Duration;
use std::{collections::BTreeMap, rc::Rc};
#[path = "synapse_select_pointer.rs"]
mod pointer;

/// Original module 7734 option adornments, keyed by the committed domain value.
/// Separators are presentation between options and never enter SelectState.
#[derive(Clone, Default, PartialEq)]
pub struct SelectOptionDecoration {
    prefix_image: Option<OptionImage>,
    content_image: Option<OptionImage>,
    tooltip_icon: Option<(SharedString, SharedString)>,
    separator_before: bool,
}

#[derive(Clone, PartialEq)]
struct OptionImage {
    path: SharedString,
    width: f32,
    height: f32,
}

impl SelectOptionDecoration {
    pub fn new() -> Self {
        Self::default()
    }

    /// The source calls its trailing slot image an option/label prefix.
    pub fn prefix_image(mut self, path: impl Into<SharedString>, width: f32, height: f32) -> Self {
        self.prefix_image = Some(OptionImage {
            path: path.into(),
            width,
            height,
        });
        self
    }

    /// Replace the option and selected text with the source's image content.
    /// The item title remains the accessible name and its value remains the ID.
    pub fn content_image(mut self, path: impl Into<SharedString>, width: f32, height: f32) -> Self {
        self.content_image = Some(OptionImage {
            path: path.into(),
            width,
            height,
        });
        self
    }

    /// Only this icon owns the source preset tooltip; the option text does not.
    pub fn tooltip_icon(
        mut self,
        path: impl Into<SharedString>,
        text: impl Into<SharedString>,
    ) -> Self {
        self.tooltip_icon = Some((path.into(), text.into()));
        self
    }

    pub fn separator_before(mut self, before: bool) -> Self {
        self.separator_before = before;
        self
    }
}

/// Source pointer-dropdown geometry, independent of committed selection.
#[derive(Clone)]
pub struct PointerSelectStyle {
    trigger_height: Rems,
    row_height: Rems,
    text_inset: Rems,
    text_trailing_inset: Rems,
    menu_padding_left: Rems,
}

impl Default for PointerSelectStyle {
    fn default() -> Self {
        Self {
            trigger_height: css(27.),
            row_height: css(25.),
            text_inset: css(5.),
            text_trailing_inset: css(19.),
            menu_padding_left: css(0.),
        }
    }
}

impl PointerSelectStyle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn trigger_height(mut self, height: Rems) -> Self {
        self.trigger_height = height;
        self
    }

    pub fn row_height(mut self, height: Rems) -> Self {
        self.row_height = height;
        self
    }

    pub fn text_insets(mut self, leading: Rems, trailing: Rems) -> Self {
        self.text_inset = leading;
        self.text_trailing_inset = trailing;
        self
    }

    pub fn menu_padding_left(mut self, padding: Rems) -> Self {
        self.menu_padding_left = padding;
        self
    }
}

/// The verified source branch that decides the menu's vertical direction.
#[derive(Clone, Default)]
pub enum PointerSelectPlacement {
    /// Mouse 190/229/239 flips only on viewport overflow.
    #[default]
    ViewportOverflow,
    /// Audio 1422 also flips when its 25px-row estimate equals the viewport.
    ViewportBoundary,
    /// Audio 1422 reads the actual body wrapper, then checks window overflow.
    WrapperBoundary(Rc<std::cell::Cell<Bounds<Pixels>>>),
}

#[derive(Clone, Copy, Default, PartialEq)]
enum Presentation {
    #[default]
    Synapse,
    Alexa,
}

impl Presentation {
    fn row_height(self) -> f32 {
        match self {
            Self::Synapse => 25.,
            // `.dropdown-item`: 14px * 1.36 line-height + 4px top/bottom padding.
            Self::Alexa => 14. * 1.36 + 8.,
        }
    }

    fn max_height(self) -> f32 {
        match self {
            Self::Synapse => 180.,
            Self::Alexa => 110.,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum SelectFocus {
    Outside,
    Trigger,
    Option(usize),
}

#[derive(Debug, PartialEq)]
enum SelectKeyRequest {
    Ignore,
    Toggle,
    Focus(usize),
    Confirm(usize),
    Tab { backwards: bool },
}

struct SelectKeyboard<'a, I> {
    presentation: Presentation,
    open: bool,
    disabled: bool,
    focus: SelectFocus,
    items: &'a [I],
}

impl<I: SelectItem> SelectKeyboard<'_, I> {
    fn request(&self, key: &str, shift: bool) -> SelectKeyRequest {
        if self.presentation != Presentation::Synapse
            || self.disabled
            || self.focus == SelectFocus::Outside
        {
            return SelectKeyRequest::Ignore;
        }
        match key {
            "up" | "down" => {
                let available = self
                    .items
                    .iter()
                    .enumerate()
                    .filter_map(|(ix, item)| (!item.disabled()).then_some(ix))
                    .collect::<Vec<_>>();
                // The source inspects mounted .option:not(.inactive) elements.
                // A closed menu has none, so its first arrow only clicks open.
                if !self.open || available.is_empty() {
                    return if self.focus == SelectFocus::Trigger {
                        SelectKeyRequest::Toggle
                    } else {
                        SelectKeyRequest::Ignore
                    };
                }
                let focused = match self.focus {
                    SelectFocus::Option(ix) => available.iter().position(|entry| *entry == ix),
                    _ => None,
                };
                let next = match (focused, key) {
                    (Some(ix), "down") => (ix + 1) % available.len(),
                    (Some(0), "up") | (None, "up") => available.len() - 1,
                    (Some(ix), "up") => ix - 1,
                    (None, _) => 0,
                    _ => unreachable!(),
                };
                SelectKeyRequest::Focus(available[next])
            }
            "enter" | "space" => match self.focus {
                SelectFocus::Trigger => SelectKeyRequest::Toggle,
                SelectFocus::Option(ix)
                    if self.open && self.items.get(ix).is_some_and(|item| !item.disabled()) =>
                {
                    SelectKeyRequest::Confirm(ix)
                }
                _ => SelectKeyRequest::Ignore,
            },
            // Unlike arrow navigation, source c counts inactive options too.
            "tab" if self.open && !self.items.is_empty() => {
                SelectKeyRequest::Tab { backwards: shift }
            }
            _ => SelectKeyRequest::Ignore,
        }
    }
}

struct TabRequest {
    generation: u64,
    backwards: bool,
}

pub fn select<I: SelectItem<Value = String> + 'static>(
    state: &Entity<SelectState<Vec<I>>>,
) -> SynapseSelect<I> {
    SynapseSelect {
        state: state.clone(),
        id: ("synapse-select", state.entity_id()).into(),
        items: Vec::new(),
        style: StyleRefinement::default(),
        disabled: false,
        label: None,
        placeholder: None,
        selected_text: None,
        presentation: Presentation::Synapse,
        on_trigger_click: None,
        pointer_only: false,
        pointer_decorations: BTreeMap::new(),
        pointer_style: PointerSelectStyle::default(),
        pointer_placement: PointerSelectPlacement::default(),
        allow_one_item: false,
    }
}

/// Alexa's `.dropdown-selector/.dropdown-razer-2`, using the same committed state.
pub fn select_alexa<I: SelectItem<Value = String> + 'static>(
    state: &Entity<SelectState<Vec<I>>>,
) -> SynapseSelect<I> {
    SynapseSelect {
        presentation: Presentation::Alexa,
        ..select(state)
    }
}

/// Current module 7734 pointer presentation, with the host's shared keyboard behavior.
/// Use only where the mounted source caller and that dependency are verified.
pub fn select_pointer<I: SelectItem<Value = String> + 'static>(
    state: &Entity<SelectState<Vec<I>>>,
) -> SynapseSelect<I> {
    SynapseSelect {
        pointer_only: true,
        ..select(state)
    }
}

#[derive(IntoElement)]
pub struct SynapseSelect<I: SelectItem<Value = String> + 'static> {
    state: Entity<SelectState<Vec<I>>>,
    id: ElementId,
    items: Vec<I>,
    style: StyleRefinement,
    disabled: bool,
    label: Option<SharedString>,
    placeholder: Option<SharedString>,
    selected_text: Option<SharedString>,
    presentation: Presentation,
    on_trigger_click: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
    pointer_only: bool,
    pointer_decorations: BTreeMap<String, SelectOptionDecoration>,
    pointer_style: PointerSelectStyle,
    pointer_placement: PointerSelectPlacement,
    allow_one_item: bool,
}

impl<I: SelectItem<Value = String> + 'static> SynapseSelect<I> {
    pub fn id(mut self, id: impl Into<ElementId>) -> Self {
        self.id = id.into();
        self
    }

    /// Supply the same current options used by the owning feature's SelectState.
    pub fn items(mut self, items: Vec<I>) -> Self {
        self.items = items;
        self
    }

    /// Use the source's pointer presentation with decorated, grouped options.
    /// Kit's ordinary List remains unchanged: it requires equal-height rows,
    /// while this source renders a separate 9px, non-selectable divider.
    pub fn pointer_option_decorations(
        mut self,
        decorations: BTreeMap<String, SelectOptionDecoration>,
    ) -> Self {
        self.pointer_only = true;
        self.pointer_decorations = decorations;
        self
    }

    pub fn pointer_style(mut self, style: PointerSelectStyle) -> Self {
        self.pointer_style = style;
        self
    }

    pub fn pointer_placement(mut self, placement: PointerSelectPlacement) -> Self {
        self.pointer_placement = placement;
        self
    }

    /// Source `allowOneItem` keeps a pointer dropdown usable with one option.
    pub fn allow_one_item(mut self, allow: bool) -> Self {
        self.allow_one_item = allow;
        self
    }

    pub fn accessibility_label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Text shown while no domain value is selected; never added to the menu.
    pub fn placeholder(mut self, placeholder: impl Into<SharedString>) -> Self {
        self.placeholder = Some(placeholder.into());
        self
    }

    /// Owner-supplied text for the committed value in the trigger only.
    ///
    /// Menu titles and selection remain controlled by `items` and SelectState.
    /// Without a committed value, the placeholder is still displayed. This
    /// preserves source selectors that retain a selected object's old label
    /// while the refreshed menu contains its new label.
    pub fn selected_text(mut self, text: impl Into<SharedString>) -> Self {
        self.selected_text = Some(text.into());
        self
    }

    /// Observe trigger activation, including source keyboard-generated clicks.
    /// Menu option clicks and state synchronization do not call this adapter.
    /// Kit's popover stops mouse-down propagation, so source document-click
    /// behavior belongs on the trigger itself.
    pub fn on_trigger_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_trigger_click = Some(Rc::new(handler));
        self
    }
}

impl<I: SelectItem<Value = String> + 'static> Disableable for SynapseSelect<I> {
    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl<I: SelectItem<Value = String> + 'static> Styled for SynapseSelect<I> {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

struct SynapseSelectView<I: SelectItem<Value = String> + 'static> {
    state: Entity<SelectState<Vec<I>>>,
    list: Entity<ListState<Options<I>>>,
    id: ElementId,
    style: StyleRefinement,
    disabled: bool,
    label: Option<SharedString>,
    placeholder: Option<SharedString>,
    selected_text: Option<SharedString>,
    presentation: Presentation,
    on_trigger_click: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
    pointer_only: bool,
    pointer_decorations: BTreeMap<String, SelectOptionDecoration>,
    pointer_style: PointerSelectStyle,
    pointer_placement: PointerSelectPlacement,
    pointer_scroll: ScrollHandle,
    pointer_position: Rc<std::cell::Cell<pointer::PointerPosition>>,
    allow_one_item: bool,
    pointer_bounds: Rc<std::cell::Cell<Bounds<Pixels>>>,
    open: bool,
    generation: u64,
    pending_tab: Option<TabRequest>,
    hovered: bool,
    width: Pixels,
    _popup_context: Option<DeferredPopover>,
    _state_observer: Subscription,
    _activation_observer: Subscription,
    _keyboard_subscription: Subscription,
}

impl<I: SelectItem<Value = String> + 'static> SynapseSelectView<I> {
    fn new(
        state: Entity<SelectState<Vec<I>>>,
        id: ElementId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let view = cx.entity().downgrade();
        let list = cx.new(|cx| {
            ListState::new(
                Options {
                    items: Vec::new(),
                    committed: None,
                    cursor: None,
                    presentation: Presentation::Synapse,
                    focus: BTreeMap::new(),
                    generation: 0,
                    view,
                },
                window,
                cx,
            )
        });
        let window_id = window.window_handle().window_id();
        let view = cx.entity().downgrade();
        let keyboard = cx.intercept_keystrokes(move |event, window, cx| {
            if window.window_handle().window_id() == window_id {
                let _ = view.update(cx, |view, cx| {
                    view.on_source_key(&event.keystroke, window, cx);
                });
            }
        });
        Self {
            _state_observer: cx.observe_in(&state, window, |view, state, window, cx| {
                let selected = state.read(cx).selected_value().cloned();
                // Current Synapse module 7734 componentDidUpdate closes only
                // when the committed value changes, not on unrelated redraws.
                let changed = view.list.read(cx).delegate().committed != selected;
                if changed && view.presentation == Presentation::Synapse {
                    view.set_open(false, window, cx);
                }
                view.list.update(cx, |list, cx| {
                    if list.delegate().committed != selected {
                        list.delegate_mut().committed = selected;
                        cx.notify();
                    }
                });
                cx.notify();
            }),
            // Current Synapse module 7734 installs/removes a window blur
            // listener with the dropdown. Retain its native equivalent here.
            _activation_observer: cx.observe_window_activation(window, |view, window, cx| {
                if !window.is_window_active() && view.presentation == Presentation::Synapse {
                    view.set_open(false, window, cx);
                }
            }),
            state,
            list,
            id,
            style: StyleRefinement::default(),
            disabled: false,
            label: None,
            placeholder: None,
            selected_text: None,
            presentation: Presentation::Synapse,
            on_trigger_click: None,
            pointer_only: false,
            pointer_decorations: BTreeMap::new(),
            pointer_style: PointerSelectStyle::default(),
            pointer_placement: PointerSelectPlacement::default(),
            pointer_scroll: ScrollHandle::new(),
            pointer_position: Rc::new(std::cell::Cell::new(pointer::PointerPosition::default())),
            allow_one_item: false,
            pointer_bounds: Rc::new(std::cell::Cell::new(Bounds::default())),
            open: false,
            generation: 0,
            pending_tab: None,
            hovered: false,
            width: px(0.),
            _popup_context: None,
            _keyboard_subscription: keyboard,
        }
    }

    fn is_disabled(&self, cx: &App) -> bool {
        self.disabled
            || (self.pointer_only
                && !self.allow_one_item
                && self.list.read(cx).delegate().items.len() <= 1)
    }

    fn current_focus(&self, window: &Window, cx: &App) -> SelectFocus {
        if self.state.focus_handle(cx).is_focused(window) {
            return SelectFocus::Trigger;
        }
        let options = self.list.read(cx).delegate();
        options
            .items
            .iter()
            .position(|item| {
                options
                    .focus
                    .get(item.value())
                    .is_some_and(|focus| focus.is_focused(window))
            })
            .map(SelectFocus::Option)
            .unwrap_or(SelectFocus::Outside)
    }

    fn on_source_key(&mut self, key: &Keystroke, window: &mut Window, cx: &mut Context<Self>) {
        let request = SelectKeyboard {
            presentation: self.presentation,
            open: self.open,
            disabled: self.is_disabled(cx),
            focus: self.current_focus(window, cx),
            items: &self.list.read(cx).delegate().items,
        }
        .request(&key.key, key.modifiers.shift);
        if request == SelectKeyRequest::Ignore {
            return;
        }
        // Interception precedes Kit's Select/List actions and its private Tab
        // action. All paths still enter this dropdown's controlled state owner.
        window.prevent_default();
        cx.stop_propagation();
        match request {
            SelectKeyRequest::Ignore => unreachable!(),
            SelectKeyRequest::Toggle => self.activate_source_trigger(window, cx),
            SelectKeyRequest::Focus(ix) => {
                let focus = self.list.update(cx, |list, cx| {
                    let item = &list.delegate().items[ix];
                    let focus = list.delegate().focus.get(item.value()).cloned();
                    list.set_selected_index(Some(IndexPath::new(ix)), window, cx);
                    list.scroll_to_selected_item(window, cx);
                    focus
                });
                if self.pointer_only {
                    self.pointer_scroll.scroll_to_item(ix);
                }
                if let Some(focus) = focus {
                    focus.focus(window, cx);
                }
                cx.notify();
            }
            SelectKeyRequest::Confirm(ix) => {
                let value = self.list.read(cx).delegate().items[ix].value().clone();
                self.confirm(value, self.generation, window, cx);
            }
            SelectKeyRequest::Tab { backwards } => {
                self.activate_source_trigger(window, cx);
                if !self.open {
                    self.pending_tab = Some(TabRequest {
                        generation: self.generation,
                        backwards,
                    });
                    cx.notify();
                }
            }
        }
    }

    fn activate_source_trigger(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.set_open(!self.open, window, cx);
        if let Some(handler) = self.on_trigger_click.clone() {
            // Source O synthesizes left down/up/click at the trigger's center,
            // with no modifiers, including for arrows and Tab dismissal.
            let position = self.pointer_bounds.get().center();
            let click = ClickEvent::Mouse(MouseClickEvent {
                down: MouseDownEvent {
                    button: MouseButton::Left,
                    position,
                    ..Default::default()
                },
                up: MouseUpEvent {
                    button: MouseButton::Left,
                    position,
                    ..Default::default()
                },
            });
            handler(&click, window, cx);
        }
    }

    fn did_layout(&mut self, bounds: Bounds<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        self.width = bounds.size.width;
        self.pointer_bounds.set(bounds);
        if let Some(request) = self.pending_tab.take() {
            let owner = cx.entity().downgrade();
            // Run after this closed frame has painted every remaining target.
            window.defer(cx, move |window, cx| {
                let _ = owner.update(cx, |owner, cx| {
                    if !owner.open
                        && owner.generation == request.generation
                        && !owner.is_disabled(cx)
                    {
                        keyboard_navigation::focus_adjacent_to(
                            &owner.state.focus_handle(cx),
                            request.backwards,
                            window,
                            cx,
                        );
                    }
                });
            });
        }
    }

    /// Glue the controlled Base Select and Popover to the framework List cursor.
    fn set_open(&mut self, open: bool, window: &mut Window, cx: &mut Context<Self>) {
        let open = open && !self.is_disabled(cx);
        if self.open == open {
            return;
        }
        self.open = open;
        self.generation = self
            .generation
            .checked_add(1)
            .expect("Select generation exhausted");
        self.pending_tab = None;
        self.list.update(cx, |list, _| {
            list.delegate_mut().generation = self.generation
        });
        self._popup_context = (self.presentation == Presentation::Alexa && open)
            .then(|| GlobalState::register_deferred_popover(cx));
        if open {
            self.pointer_position
                .set(self.pointer_position.get().begin_open());
            let selected = self.state.read(cx).selected_value().cloned();
            let pointer_scroll = self.pointer_only.then(|| self.pointer_scroll.clone());
            self.list.update(cx, |list, cx| {
                let index = list
                    .delegate()
                    .items
                    .iter()
                    .position(|item| Some(item.value()) == selected.as_ref())
                    .map(IndexPath::new);
                list.set_selected_index(index, window, cx);
                list.scroll_to_selected_item(window, cx);
                if let (Some(scroll), Some(index)) = (&pointer_scroll, index) {
                    // Source scrollTo uses the selected row's mounted offset.
                    scroll.scroll_to_top_of_item(index.row);
                }
            });
            if self.presentation == Presentation::Alexa {
                self.list.focus_handle(cx).focus(window, cx);
            }
        } else {
            let restore_focus = window.is_window_active()
                && (!self.pointer_only || self.current_focus(window, cx) != SelectFocus::Outside);
            self.state.update(cx, |state, cx| {
                if restore_focus {
                    state.focus(window, cx);
                }
                cx.emit(DismissEvent);
            });
        }
        cx.notify();
    }

    fn confirm(
        &mut self,
        value: String,
        generation: u64,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_disabled(cx)
            || !self.open
            || self.generation != generation
            || !self
                .list
                .read(cx)
                .delegate()
                .items
                .iter()
                .any(|item| item.value() == &value && !item.disabled())
        {
            return;
        }
        self.state.update(cx, |state, cx| {
            state.set_selected_value(&value, window, cx);
            cx.emit(SelectEvent::Confirm(state.selected_value().cloned()));
            cx.notify();
        });
        self.set_open(false, window, cx);
    }

    fn sync_items(&mut self, items: Vec<I>, window: &mut Window, cx: &mut Context<Self>) {
        let committed = self.state.read(cx).selected_value().cloned();
        let open = self.open;
        let presentation = self.presentation;
        let changed = self.list.update(cx, |list, cx| {
            let previous = list.delegate();
            let items_changed = previous.items.len() != items.len()
                || previous.items.iter().zip(&items).any(|(old, new)| {
                    old.value() != new.value()
                        || old.title() != new.title()
                        || old.disabled() != new.disabled()
                });
            if !items_changed
                && previous.committed == committed
                && previous.presentation == presentation
            {
                return false;
            }

            // A positional cursor must follow its value when rows before it are
            // inserted or removed. If that value disappeared, use the owner's
            // surviving committed value; no value is committed during this sync.
            let cursor_value = list
                .selected_index()
                .and_then(|index| previous.items.get(index.row))
                .map(|item| item.value().clone());
            let cursor = cursor_value
                .as_ref()
                .and_then(|value| items.iter().position(|item| item.value() == value))
                .or_else(|| {
                    committed
                        .as_ref()
                        .and_then(|value| items.iter().position(|item| item.value() == value))
                })
                .map(IndexPath::new);
            list.delegate_mut().items = items;
            let options = list.delegate_mut();
            options
                .focus
                .retain(|value, _| options.items.iter().any(|item| item.value() == value));
            for item in &options.items {
                options
                    .focus
                    .entry(item.value().clone())
                    .or_insert_with(|| cx.focus_handle());
            }
            list.delegate_mut().committed = committed;
            list.delegate_mut().presentation = presentation;
            if items_changed && list.selected_index() != cursor {
                list.set_selected_index(cursor, window, cx);
                if open && let Some(cursor) = cursor {
                    list.scroll_to_item(cursor, ScrollStrategy::Nearest, window, cx);
                }
            }
            // ListState::render remeasures rows and rebuilds its cache when the
            // row count changes. Notify only on changed presentation or selection,
            // so an ordinary parent redraw does not restart navigation or redraw.
            cx.notify();
            true
        });
        if changed {
            cx.notify();
        }
    }
}

impl<I: SelectItem<Value = String> + 'static> RenderOnce for SynapseSelect<I> {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let view = window.use_keyed_state(self.id.clone(), cx, |window, cx| {
            SynapseSelectView::new(self.state, self.id, window, cx)
        });
        view.update(cx, |view, cx| {
            view.style = self.style;
            view.label = self.label;
            view.placeholder = self.placeholder;
            view.selected_text = self.selected_text;
            view.disabled = self.disabled;
            view.presentation = self.presentation;
            view.on_trigger_click = self.on_trigger_click.clone();
            view.pointer_only = self.pointer_only;
            view.pointer_decorations = self.pointer_decorations;
            view.pointer_style = self.pointer_style;
            view.pointer_placement = self.pointer_placement;
            view.allow_one_item = self.allow_one_item;
            view.sync_items(self.items, window, cx);
            if view.is_disabled(cx) {
                view.set_open(false, window, cx);
            }
        });
        view
    }
}

impl<I: SelectItem<Value = String> + 'static> Render for SynapseSelectView<I> {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.pointer_only {
            return self.render_pointer(window, cx);
        }
        let focus = self.state.focus_handle(cx).tab_stop(true);
        let content_focus = self.list.focus_handle(cx);
        let options = self.list.read(cx).delegate();
        let selected = self.state.read(cx).selected_value();
        let title = self
            .selected_text
            .clone()
            .filter(|_| selected.is_some())
            .or_else(|| {
                options
                    .items
                    .iter()
                    .find(|item| Some(item.value()) == selected)
                    .map(SelectItem::title)
            })
            .unwrap_or_else(|| self.placeholder.clone().unwrap_or_default());
        let presentation = self.presentation;
        let alexa = presentation == Presentation::Alexa;
        let menu_height = (options.items.len() as f32 * presentation.row_height() + 2.)
            .min(presentation.max_height());
        let width = self.width;
        let list = self.list.clone();
        let open_change = cx.entity().downgrade();
        let popup_change = cx.entity().downgrade();
        let focused = focus.is_focused(window);
        let open = self.open;
        let colors = DropdownColors::new();
        let border = if !self.disabled && (open || focused || (alexa && self.hovered)) {
            cx.theme().primary
        } else if alexa {
            cx.theme().border
        } else {
            cx.theme().input
        };
        let border = if alexa {
            motion::transition(
                (self.id.clone(), "alexa-trigger-border"),
                border,
                Transition::new(Duration::from_millis(100)).easing(Easing::EaseInOut),
                window,
                cx,
            )
        } else {
            border
        };
        // `$p` first mounts at height 0, waits 100ms, then sets the final height.
        // Its close path sets height 0 immediately and unmounts after 100ms.
        // Sequence retains the previous timing when retargeted during the delay
        // or animation, so reversing does not jump to the other curve's value.
        let alexa_reveal = alexa.then(|| {
            Sequence::new((self.id.clone(), "alexa-options-height"), 0.)
                .with_step(
                    if open { 1. } else { 0. },
                    Transition::new(Duration::from_millis(100))
                        .delay(if open {
                            Duration::from_millis(100)
                        } else {
                            Duration::ZERO
                        })
                        .easing(Easing::EaseInOut),
                )
                .sample(window, cx)
        });
        let trigger = div()
            .id("input")
            .test_support()
            .when_some(self.on_trigger_click.clone(), |trigger, handler| {
                trigger.on_click(move |event, window, cx| handler(event, window, cx))
            })
            .w_full()
            .h(css(25.))
            .flex()
            .items_center()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .pl(css(if alexa { 6. } else { 4. }))
                    .truncate()
                    .child(title.clone()),
            )
            .child(
                div()
                    .w(css(if alexa { 30. } else { 29. }))
                    .h_full()
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        razer_assets::resource_svg()
                            .path("synapse/expand.svg")
                            .size(css(10.))
                            .when(alexa, |this| this.text_color(AlexaColors::dropdown_arrow()))
                            .flex_shrink_0()
                            .with_transformation(Transformation::rotate(radians(
                                motion::transition(
                                    (self.id.clone(), "arrow-angle"),
                                    if open { std::f32::consts::PI } else { 0. },
                                    if alexa {
                                        Transition::new(Duration::from_millis(100))
                                            .easing(Easing::EaseInOut)
                                    } else {
                                        Transition::new(Duration::from_millis(300))
                                            .easing(Easing::Ease)
                                    },
                                    window,
                                    cx,
                                ),
                            ))),
                    ),
            );

        div()
            .id("source-dropdown")
            .test_support()
            .h(css(27.))
            .when(alexa, |this| this.w(css(230.)).mb(css(10.)))
            .min_w_0()
            .border_1()
            .border_color(border)
            .text_size(css(14.))
            .line_height(css(if alexa { 26. } else { 17. }))
            .text_color(cx.theme().foreground)
            .when(!self.disabled && !alexa, |this| {
                this.hover(|style| style.border_color(cx.theme().primary))
            })
            .when(alexa, |this| {
                this.on_hover(cx.listener(|view, hovered, _, cx| {
                    view.hovered = *hovered;
                    cx.notify();
                }))
            })
            .when(self.disabled, |this| this.opacity(0.3))
            .refine_style(&self.style)
            .on_prepaint({
                let view = cx.entity().downgrade();
                move |bounds, window, cx| {
                    _ = view.update(cx, |view, cx| view.did_layout(bounds, window, cx));
                }
            })
            .child(focus_target(
                BaseSelect::new(self.id.clone())
                    .open(open)
                    .disabled(self.disabled)
                    .focus_handle(&focus)
                    .content_focus_handle(if alexa { &content_focus } else { &focus })
                    .accessibility_value(title)
                    .when_some(self.label.clone(), |this, label| {
                        this.accessibility_label(label)
                    })
                    .on_open_change(move |open, window, cx| {
                        _ = open_change.update(cx, |view, cx| view.set_open(open, window, cx));
                    })
                    .size_full()
                    .child(if self.disabled {
                        trigger.into_any_element()
                    } else if let Some(reveal) = alexa_reveal {
                        let progress = *reveal.value();
                        let visible = open || progress > 0.;
                        let toggle = cx.entity().downgrade();
                        let dismiss = cx.entity().downgrade();
                        // Base Select retains keyboard/accessible activation and focus.
                        // Popup retains placement and deferred painting while its body
                        // finishes closing, independently of the logical open state.
                        Popup::new(
                            "alexa-options-popup",
                            trigger.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                                cx.stop_propagation();
                                _ = toggle.update(cx, |view, cx| {
                                    // An outside-down handler may have closed it
                                    // earlier in this same trigger gesture.
                                    if view.open == open {
                                        view.set_open(!open, window, cx);
                                    }
                                });
                            }),
                        )
                        // One trigger border pixel plus the source's 2px outer gap.
                        .offset(px(1.) + css(2.).to_pixels(window.rem_size()))
                        .size_full()
                        .when(visible, |this| {
                            this.content(
                                // Resolve against the final size throughout the animation.
                                razer_assets::resource_scope(
                                    div().w(width).h(css(menu_height)).child(
                                        div()
                                            .id("source-options")
                                            .test_support()
                                            .w(width)
                                            .h(css(menu_height * progress))
                                            .max_h(css(110.))
                                            .ml(-px(1.))
                                            .border_x_1()
                                            .border_t(px(progress))
                                            .border_b(px(progress))
                                            .border_color(cx.theme().border)
                                            .rounded_none()
                                            .bg(colors.surface())
                                            .overflow_hidden()
                                            .on_mouse_down(MouseButton::Left, |_, _, cx| {
                                                cx.stop_propagation();
                                            })
                                            .when(open, |this| {
                                                this.on_mouse_down_out(move |_, window, cx| {
                                                    _ = dismiss.update(cx, |view, cx| {
                                                        view.set_open(false, window, cx);
                                                    });
                                                })
                                            })
                                            .when(!open, |this| {
                                                // Exit is presentation only: intercept before the
                                                // retained List can refocus or activate a row.
                                                this.capture_any_mouse_down(|_, _, cx| {
                                                    cx.stop_propagation()
                                                })
                                                .capture_any_mouse_up(|_, _, cx| {
                                                    cx.stop_propagation()
                                                })
                                            })
                                            .child(
                                                List::new(&list)
                                                    .p_0()
                                                    .h(css(menu_height - 2.))
                                                    .scrollbar_visible(
                                                        open && reveal.is_finished(),
                                                    ),
                                            ),
                                    ),
                                ),
                            )
                        })
                        .into_any_element()
                    } else {
                        Popover::new("options-popover")
                            .open(open)
                            .track_focus(&focus)
                            // The trigger sits inside the 1px border; 2px reaches the
                            // original dropdown bottom plus `.s3-options { margin-top:1px }`.
                            .offset(px(2.))
                            .size_full()
                            .trigger_with(move |_, _, _| trigger.into_any_element())
                            .on_open_change(move |open, window, cx| {
                                _ = popup_change
                                    .update(cx, |view, cx| view.set_open(*open, window, cx));
                            })
                            .content(move |_, window, cx| {
                                razer_assets::resource_scope({
                                    // height:auto; max-height:0 -> 180px, CSS ease.
                                    // Closing sets height:0 without a slide or fade.
                                    let reveal = Presence::new("options-reveal", true)
                                        .transition(
                                            Transition::new(Duration::from_millis(200))
                                                .easing(Easing::Ease),
                                        )
                                        .sample(window, cx)
                                        .progress;
                                    // Resolve placement against the final size, so
                                    // a popup near the bottom cannot flip mid-animation.
                                    div().w(width).h(css(menu_height)).child(
                                        div()
                                            .id("source-options")
                                            .test_support()
                                            .w(width)
                                            .h(css(menu_height.min(180. * reveal)))
                                            .max_h(css(180.))
                                            .ml(-px(1.))
                                            .border_1()
                                            .border_color(cx.theme().input)
                                            .rounded_none()
                                            .bg(colors.surface())
                                            .overflow_hidden()
                                            .child(List::new(&list).p_0().h(css(menu_height - 2.))),
                                    )
                                })
                            })
                            .into_any_element()
                    }),
                &focus,
                if alexa {
                    FocusBehavior::Spatial
                } else {
                    FocusBehavior::LocalVertical
                },
                !self.disabled,
            ))
            .into_any_element()
    }
}

/// Only the List's transient cursor lives here. Its value is never the committed selection.
struct Options<I: SelectItem<Value = String> + 'static> {
    items: Vec<I>,
    committed: Option<String>,
    cursor: Option<IndexPath>,
    presentation: Presentation,
    focus: BTreeMap<String, FocusHandle>,
    generation: u64,
    view: WeakEntity<SynapseSelectView<I>>,
}

impl<I: SelectItem<Value = String> + 'static> ListDelegate for Options<I> {
    type Item = OptionRow;

    fn items_count(&self, _: usize, _: &App) -> usize {
        self.items.len()
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) -> Option<Self::Item> {
        let item = self.items.get(ix.row)?;
        Some(OptionRow {
            id: SharedString::from(format!("option-{}", item.value())).into(),
            title: item.title(),
            checked: self.committed.as_ref() == Some(item.value()),
            highlighted: false,
            disabled: item.disabled(),
            presentation: self.presentation,
            focus: (self.presentation == Presentation::Synapse)
                .then(|| self.focus.get(item.value()).cloned())
                .flatten(),
        })
    }

    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) {
        self.cursor = ix;
    }

    fn confirm(&mut self, _: bool, window: &mut Window, cx: &mut Context<ListState<Self>>) {
        let Some(item) = self.cursor.and_then(|ix| self.items.get(ix.row)) else {
            return;
        };
        if item.disabled() {
            return;
        }
        let value = item.value().clone();
        let generation = self.generation;
        let view = self.view.clone();
        // Release the List borrow before an owner callback can refresh its options.
        window.defer(cx, move |window, cx| {
            _ = view.update(cx, |view, cx| view.confirm(value, generation, window, cx));
        });
    }

    fn render_empty(
        &mut self,
        _: &mut Window,
        _: &mut Context<ListState<Self>>,
    ) -> impl IntoElement {
        div()
    }
}

#[derive(IntoElement)]
struct OptionRow {
    id: ElementId,
    title: SharedString,
    checked: bool,
    highlighted: bool,
    disabled: bool,
    presentation: Presentation,
    focus: Option<FocusHandle>,
}

impl Selectable for OptionRow {
    fn selected(mut self, selected: bool) -> Self {
        self.highlighted = selected;
        self
    }

    fn is_selected(&self) -> bool {
        self.highlighted
    }
}

impl RenderOnce for OptionRow {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let colors = DropdownColors::new();
        let alexa = self.presentation == Presentation::Alexa;
        let hover = if alexa {
            AlexaColors::dropdown_hover()
        } else {
            colors.hover()
        };
        let hover_motion = alexa.then(|| {
            let state =
                window.use_keyed_state((self.id.clone(), "alexa-row-hover"), cx, |_, _| false);
            let target =
                if !self.disabled && (*state.read(cx) || (self.highlighted && !self.checked)) {
                    hover
                } else {
                    cx.theme().transparent
                };
            let color = motion::transition(
                (self.id.clone(), "alexa-row-background"),
                target,
                Transition::new(Duration::from_millis(100)).easing(Easing::EaseInOut),
                window,
                cx,
            );
            (state, color)
        });
        let element = div()
            .id(self.id)
            .test_support()
            .role(Role::ListBoxOption)
            .aria_label(self.title.clone())
            .aria_selected(self.checked)
            .when(self.highlighted, |this| this.aria_active_descendant())
            .h(css(self.presentation.row_height()))
            .min_h(css(self.presentation.row_height()))
            .w_full()
            .p(css(4.))
            .when(alexa, |this| this.px(css(6.)))
            .rounded_none()
            .text_size(css(14.))
            .line_height(css(if alexa { 14. * 1.36 } else { 17. }))
            .text_color(if self.checked {
                cx.theme().primary
            } else {
                cx.theme().foreground
            })
            .when_some(hover_motion, |this, (state, color)| {
                this.bg(color)
                    .on_hover(window.listener_for(&state, |state, hovered, _, cx| {
                        *state = *hovered;
                        cx.notify();
                    }))
            })
            .when(!self.disabled && !alexa, |this| {
                this.hover(|this| this.bg(hover))
                    .active(|this| this.text_color(cx.theme().primary))
            })
            .when(self.disabled, |this| {
                this.text_color(if alexa {
                    AlexaColors::dropdown_disabled()
                } else {
                    cx.theme().foreground.opacity(0.3)
                })
            })
            .overflow_hidden()
            .whitespace_nowrap()
            .text_ellipsis()
            .child(self.title);
        if let Some(focus) = self.focus {
            focus_target(
                element.track_focus(&focus),
                &focus,
                FocusBehavior::LocalVertical,
                !self.disabled,
            )
            .ring(FocusRing::Options)
            .into_any_element()
        } else {
            element.into_any_element()
        }
    }
}

#[cfg(test)]
mod keyboard_tests {
    use super::{Presentation, SelectFocus, SelectKeyRequest, SelectKeyboard};
    use gpui_kit::SharedString;
    use gpui_kit::component::select::SelectItem;

    #[derive(Clone)]
    struct Item {
        value: &'static str,
        disabled: bool,
    }
    impl SelectItem for Item {
        type Value = &'static str;
        fn title(&self) -> SharedString {
            self.value.into()
        }
        fn value(&self) -> &Self::Value {
            &self.value
        }
        fn disabled(&self) -> bool {
            self.disabled
        }
    }

    fn options() -> [Item; 4] {
        [
            Item {
                value: "alpha",
                disabled: false,
            },
            Item {
                value: "inactive",
                disabled: true,
            },
            Item {
                value: "beta",
                disabled: false,
            },
            Item {
                value: "gamma",
                disabled: false,
            },
        ]
    }

    #[test]
    fn source_arrows_open_first_then_focus_boundaries_and_wrap_over_inactive_options() {
        let items = options();
        let mut keyboard = SelectKeyboard {
            presentation: Presentation::Synapse,
            open: false,
            disabled: false,
            focus: SelectFocus::Trigger,
            items: &items,
        };
        assert_eq!(keyboard.request("down", false), SelectKeyRequest::Toggle);
        assert_eq!(keyboard.request("up", false), SelectKeyRequest::Toggle);
        keyboard.open = true;
        assert_eq!(keyboard.request("down", false), SelectKeyRequest::Focus(0));
        assert_eq!(keyboard.request("up", false), SelectKeyRequest::Focus(3));
        for (current, next, previous) in [(0, 2, 3), (2, 3, 0), (3, 0, 2)] {
            keyboard.focus = SelectFocus::Option(current);
            assert_eq!(
                keyboard.request("down", false),
                SelectKeyRequest::Focus(next)
            );
            assert_eq!(
                keyboard.request("up", false),
                SelectKeyRequest::Focus(previous)
            );
        }
        keyboard.focus = SelectFocus::Option(1);
        assert_eq!(keyboard.request("down", false), SelectKeyRequest::Focus(0));
        assert_eq!(keyboard.request("up", false), SelectKeyRequest::Focus(3));
        assert_eq!(keyboard.request("enter", false), SelectKeyRequest::Ignore);
    }

    #[test]
    fn source_tab_counts_inactive_rows_but_empty_menus_keep_normal_tab_behavior() {
        let items = [Item {
            value: "inactive",
            disabled: true,
        }];
        let mut keyboard = SelectKeyboard {
            presentation: Presentation::Synapse,
            open: true,
            disabled: false,
            focus: SelectFocus::Trigger,
            items: &items,
        };
        assert_eq!(
            keyboard.request("tab", false),
            SelectKeyRequest::Tab { backwards: false }
        );
        assert_eq!(
            keyboard.request("tab", true),
            SelectKeyRequest::Tab { backwards: true }
        );
        assert_eq!(keyboard.request("down", false), SelectKeyRequest::Toggle);
        keyboard.items = &[];
        assert_eq!(keyboard.request("tab", false), SelectKeyRequest::Ignore);
        keyboard.items = &items;
        keyboard.open = false;
        assert_eq!(keyboard.request("tab", false), SelectKeyRequest::Ignore);
    }

    #[test]
    fn source_activation_distinguishes_trigger_focus_and_committed_option() {
        let items = options();
        let mut keyboard = SelectKeyboard {
            presentation: Presentation::Synapse,
            open: true,
            disabled: false,
            focus: SelectFocus::Trigger,
            items: &items,
        };
        for key in ["enter", "space"] {
            assert_eq!(keyboard.request(key, false), SelectKeyRequest::Toggle);
        }
        keyboard.focus = SelectFocus::Option(2);
        for key in ["enter", "space"] {
            assert_eq!(keyboard.request(key, false), SelectKeyRequest::Confirm(2));
        }
        for key in ["left", "right", "escape", "home", "end"] {
            assert_eq!(keyboard.request(key, false), SelectKeyRequest::Ignore);
        }
    }

    #[test]
    fn source_special_navigation_does_not_apply_to_alexa_disabled_or_unfocused_controls() {
        let items = options();
        let mut keyboard = SelectKeyboard {
            presentation: Presentation::Alexa,
            open: true,
            disabled: false,
            focus: SelectFocus::Trigger,
            items: &items,
        };
        for key in ["up", "down", "enter", "space", "tab"] {
            assert_eq!(keyboard.request(key, false), SelectKeyRequest::Ignore);
        }
        keyboard.presentation = Presentation::Synapse;
        keyboard.disabled = true;
        assert_eq!(keyboard.request("down", false), SelectKeyRequest::Ignore);
        assert_eq!(keyboard.request("tab", false), SelectKeyRequest::Ignore);
        keyboard.disabled = false;
        keyboard.focus = SelectFocus::Outside;
        assert_eq!(keyboard.request("down", false), SelectKeyRequest::Ignore);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        PointerSelectPlacement, PointerSelectStyle, SelectOptionDecoration, css, select,
        select_alexa, select_pointer,
    };
    use gpui_kit::component::{
        Disableable, IndexPath, Root,
        select::{SelectEvent, SelectState},
    };
    use gpui_kit::test::TestWindowExt as _;
    use gpui_kit::{
        AppContext, Context, Entity, IntoElement, ParentElement, Render, ScrollDelta, Styled,
        Subscription, TestAppContext, VisualTestContext, Window, div, point, px, size,
    };

    struct SelectFixture {
        state: Entity<SelectState<Vec<String>>>,
        items: Vec<String>,
        disabled: bool,
        alexa: bool,
        pointer: bool,
        image_options: bool,
        confirmed: Vec<Option<String>>,
        _subscription: Subscription,
    }

    impl SelectFixture {
        fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
            let mut items = ["Alpha", "Beta", "Gamma"].map(String::from).to_vec();
            items.extend((4..=10).map(|number| format!("Choice {number}")));
            let state =
                cx.new(|cx| SelectState::new(items.clone(), Some(IndexPath::new(0)), window, cx));
            let subscription = cx.subscribe(&state, |this, _, event, cx| {
                let SelectEvent::Confirm(value) = event;
                this.confirmed.push(value.clone());
                cx.notify();
            });
            Self {
                state,
                items,
                disabled: false,
                alexa: false,
                pointer: false,
                image_options: false,
                confirmed: Vec::new(),
                _subscription: subscription,
            }
        }

        fn set_options(
            &mut self,
            items: &[&str],
            selected: &str,
            window: &mut Window,
            cx: &mut Context<Self>,
        ) {
            self.items = items.iter().map(|item| (*item).to_string()).collect();
            self.state.update(cx, |state, cx| {
                state.set_items(self.items.clone(), window, cx);
                state.set_selected_value(&selected.to_string(), window, cx);
            });
            cx.notify();
        }
    }

    impl Render for SelectFixture {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            let control = if self.pointer {
                select_pointer(&self.state).w(css(200.))
            } else if self.alexa {
                select_alexa(&self.state)
            } else {
                select(&self.state).w(css(200.))
            };
            let control = if self.image_options {
                control
                    .pointer_option_decorations(
                        self.items
                            .iter()
                            .map(|value| {
                                (
                                    value.clone(),
                                    SelectOptionDecoration::new().content_image(
                                        "synapse/expand.svg",
                                        32.,
                                        16.,
                                    ),
                                )
                            })
                            .collect(),
                    )
                    .pointer_style(
                        PointerSelectStyle::new()
                            .trigger_height(css(35.))
                            .row_height(css(32.)),
                    )
                    .pointer_placement(PointerSelectPlacement::ViewportBoundary)
                    .w(css(96.))
            } else {
                control
            };
            div().p_4().child(
                control
                    .id("choice")
                    .items(self.items.clone())
                    .accessibility_label("Choice")
                    .disabled(self.disabled),
            )
        }
    }

    fn assert_pointer_option_in_view(window: &mut Window, option: &str) {
        let viewport = window.find("source-pointer-options-clip").bounds();
        let row = window
            .find(format!("source-pointer-option-{option}"))
            .bounds();
        assert!(row.top() >= viewport.top());
        assert!(row.bottom() <= viewport.bottom());
    }

    #[gpui_kit::test]
    fn pointer_image_options_keep_domain_value_and_support_source_keyboard_focus(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.set_reduce_motion(true);
        });
        let mut fixture = None;
        let handle = cx.open_window(size(px(600.), px(480.)), |window, cx| {
            let view = cx.new(|cx| SelectFixture::new(window, cx));
            view.update(cx, |view, cx| {
                view.pointer = true;
                view.image_options = true;
                view.set_options(
                    &["lowpass", "highpass", "peaking", "lowshelf", "highshelf"],
                    "peaking",
                    window,
                    cx,
                );
            });
            fixture = Some(view.clone());
            Root::new(view, window, cx)
        });
        let fixture = fixture.unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("source-pointer-trigger", cx);
            let trigger = window.find("source-pointer-trigger").bounds();
            let menu = window.find("source-pointer-options-clip").bounds();
            assert_eq!(trigger.size.height, px(35.));
            assert_eq!(menu.size.height, px(162.));
            assert_eq!(menu.top(), trigger.bottom() + px(1.));
            assert_eq!(
                window
                    .find("source-pointer-option-highpass")
                    .bounds()
                    .size
                    .height,
                px(32.),
            );
            window.press("down", cx);
            window.press("enter", cx);
            assert!(window.try_find("source-pointer-options-clip").is_none());
            assert_eq!(
                fixture
                    .read(cx)
                    .state
                    .read(cx)
                    .selected_value()
                    .map(String::as_str),
                Some("lowpass"),
            );
            window.click("source-pointer-trigger", cx);
            window.click("source-pointer-option-highpass", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(
                fixture.read(cx).confirmed,
                vec![
                    Some(String::from("lowpass")),
                    Some(String::from("highpass"))
                ],
            );
            assert_eq!(
                fixture
                    .read(cx)
                    .state
                    .read(cx)
                    .selected_value()
                    .map(String::as_str),
                Some("highpass"),
            );
            assert!(window.try_find("source-pointer-options-clip").is_none());
            window.click("source-pointer-trigger", cx);
            assert!(window.try_find("source-pointer-option-highpass").is_some());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn pointer_menu_scrolls_to_the_last_option_and_confirms_once(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.set_reduce_motion(true);
        });
        let mut fixture = None;
        let handle = cx.open_window(size(px(420.), px(320.)), |window, cx| {
            let view = cx.new(|cx| SelectFixture::new(window, cx));
            view.update(cx, |view, _| view.pointer = true);
            fixture = Some(view.clone());
            Root::new(view, window, cx)
        });
        let fixture = fixture.unwrap();

        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("source-pointer-trigger", cx);
            let focused = window.focused(cx);
            assert_eq!(window.find("source-pointer-trigger").focused(), Some(true));
            let viewport = window.find("source-pointer-options-clip").bounds();
            assert_eq!(viewport.size.height, px(180.));
            assert_pointer_option_in_view(window, "Alpha");
            assert!(
                window
                    .find("source-pointer-option-Choice 10")
                    .bounds()
                    .bottom()
                    > viewport.bottom()
            );
            window.scroll(
                "source-pointer-options-clip",
                ScrollDelta::Pixels(point(px(0.), px(-900.))),
                cx,
            );
            window.render_frame(cx);
            assert_pointer_option_in_view(window, "Choice 10");
            assert!(window.find("source-pointer-option-Alpha").bounds().top() < viewport.top());
            window.click("source-pointer-option-Choice 10", cx);
            assert_eq!(window.focused(cx), focused);
        })
        .unwrap();
        cx.run_until_parked();

        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let owner = fixture.read(cx);
            assert_eq!(
                owner.state.read(cx).selected_value().map(String::as_str),
                Some("Choice 10")
            );
            assert_eq!(owner.confirmed, [Some("Choice 10".into())]);
            assert!(window.try_find("source-pointer-options-clip").is_none());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn pointer_menu_reveals_selected_rows_on_first_open_reopen_and_replacement(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.set_reduce_motion(true);
        });
        let mut fixture = None;
        let handle = cx.open_window(size(px(420.), px(320.)), |window, cx| {
            let view = cx.new(|cx| SelectFixture::new(window, cx));
            view.update(cx, |view, cx| {
                view.pointer = true;
                view.state.update(cx, |state, cx| {
                    state.set_selected_value(&"Choice 10".into(), window, cx);
                });
            });
            fixture = Some(view.clone());
            Root::new(view, window, cx)
        });
        let fixture = fixture.unwrap();

        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("source-pointer-trigger", cx);
            let focused = window.focused(cx);
            assert_pointer_option_in_view(window, "Choice 10");
            window.click("source-pointer-trigger", cx);
            assert!(window.try_find("source-pointer-options-clip").is_none());
            assert_eq!(window.focused(cx), focused);
            window.click("source-pointer-trigger", cx);
            assert_pointer_option_in_view(window, "Choice 10");
            window.click("source-pointer-trigger", cx);
            assert!(fixture.read(cx).confirmed.is_empty());

            fixture.update(cx, |view, cx| {
                let replacements = (1..=20)
                    .map(|index| format!("Replacement {index}"))
                    .collect::<Vec<_>>();
                let items = replacements.iter().map(String::as_str).collect::<Vec<_>>();
                view.set_options(&items, "Replacement 20", window, cx);
            });
            window.render_frame(cx);
            window.click("source-pointer-trigger", cx);
            assert_pointer_option_in_view(window, "Replacement 20");
            assert!(window.try_find("source-pointer-option-Choice 10").is_none());
            let viewport = window.find("source-pointer-options-clip").bounds();
            assert!(
                window
                    .find("source-pointer-option-Replacement 1")
                    .bounds()
                    .top()
                    < viewport.top()
            );
            assert_eq!(window.focused(cx), focused);
            window.click("source-pointer-option-Replacement 20", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(fixture.read(cx).confirmed, [Some("Replacement 20".into())]);
            assert!(window.try_find("source-pointer-options-clip").is_none());
            window.click("source-pointer-trigger", cx);
            assert_pointer_option_in_view(window, "Replacement 20");
        })
        .unwrap();

        VisualTestContext::from_window(handle.into(), cx).deactivate_window();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("source-pointer-options-clip").is_none());
            let owner = fixture.read(cx);
            assert_eq!(
                owner.state.read(cx).selected_value().map(String::as_str),
                Some("Replacement 20")
            );
            assert_eq!(owner.confirmed, [Some("Replacement 20".into())]);
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn alexa_waits_before_reveal_and_keeps_closing_content_without_keeping_focus(
        cx: &mut TestAppContext,
    ) {
        use std::time::Duration;

        cx.update(gpui_kit::init);
        let mut fixture = None;
        let handle = cx.open_window(size(px(420.), px(320.)), |window, cx| {
            let view = cx.new(|cx| SelectFixture::new(window, cx));
            view.update(cx, |view, _| view.alexa = true);
            fixture = Some(view.clone());
            Root::new(view, window, cx)
        });
        let fixture = fixture.unwrap();
        let top = cx
            .update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                let trigger = window.find("source-dropdown").bounds();
                assert_eq!(trigger.size.width, px(230.));
                assert_eq!(trigger.size.height, px(27.));
                window.within("choice").click("input", cx);
                let menu = window.find("source-options").bounds();
                assert_eq!(menu.size.height, px(0.));
                assert_eq!(menu.top(), trigger.bottom() + px(2.));
                menu.top()
            })
            .unwrap();
        cx.executor().advance_clock(Duration::from_millis(75));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("source-options").bounds().size.height, px(0.));
        })
        .unwrap();
        cx.executor().advance_clock(Duration::from_millis(75));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let menu = window.find("source-options").bounds();
            assert_eq!(menu.top(), top);
            assert!(menu.size.height > px(0.) && menu.size.height < px(110.));
        })
        .unwrap();
        cx.executor().advance_clock(Duration::from_millis(50));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("source-options").bounds().size.height, px(110.));
            window.press("escape", cx);
            assert_eq!(window.find("choice").expanded(), Some(false));
            assert_eq!(window.find("choice").focused(), Some(true));
            assert!(window.try_find("source-options").is_some());
        })
        .unwrap();
        cx.executor().advance_clock(Duration::from_millis(50));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let menu = window.find("source-options").bounds();
            assert_eq!(menu.top(), top);
            assert!(menu.size.height > px(0.) && menu.size.height < px(110.));
            assert_eq!(window.find("choice").focused(), Some(true));
            window.click("option-Alpha", cx);
            assert_eq!(window.find("choice").focused(), Some(true));
            assert!(fixture.read(cx).confirmed.is_empty());
        })
        .unwrap();
        cx.executor().advance_clock(Duration::from_millis(50));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("source-options").is_none());
            assert_eq!(window.find("choice").value(), Some("Alpha"));
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn options_reveal_without_sliding_and_escape_restores_focus(cx: &mut TestAppContext) {
        cx.update(gpui_kit::init);
        let handle = cx.open_window(size(px(420.), px(320.)), |window, cx| {
            let view = cx.new(|cx| SelectFixture::new(window, cx));
            Root::new(view, window, cx)
        });
        let top = cx
            .update_window(handle.into(), |_, window, cx| {
                window.render_frame(cx);
                window.within("choice").click("input", cx);
                let menu = window.find("source-options").bounds();
                assert!(menu.size.height < px(180.));
                menu.top()
            })
            .unwrap();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(100));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let menu = window.find("source-options").bounds();
            assert_eq!(menu.top(), top);
            assert!(menu.size.height > px(0.) && menu.size.height < px(180.));
        })
        .unwrap();
        cx.executor()
            .advance_clock(std::time::Duration::from_millis(100));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("source-options").bounds().size.height, px(180.));
            window.press("escape", cx);
            assert!(window.try_find("source-options").is_none());
            assert_eq!(window.find("choice").focused(), Some(true));
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn pointer_and_keyboard_confirm_update_the_existing_state(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.set_reduce_motion(true);
        });
        let mut fixture = None;
        let handle = cx.open_window(size(px(420.), px(320.)), |window, cx| {
            let view = cx.new(|cx| SelectFixture::new(window, cx));
            fixture = Some(view.clone());
            Root::new(view, window, cx)
        });
        let fixture = fixture.unwrap();

        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.within("choice").click("input", cx);
            assert_eq!(window.find("choice").expanded(), Some(true));
            let menu = window.find("source-options").bounds();
            assert_eq!(menu.size.height, px(180.));
            let first = window.find("option-Alpha").bounds();
            let second = window.find("option-Beta").bounds();
            assert_eq!(first.size.height, px(25.));
            assert_eq!(second.top(), first.bottom());
            assert_eq!(first.top(), menu.top() + px(1.));
            assert_eq!(
                menu.size.width,
                window.find("source-dropdown").bounds().size.width
            );
            assert_eq!(window.find("option-Alpha").selected(), Some(true));
            window.click("option-Beta", cx);
        })
        .unwrap();
        cx.run_until_parked();

        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(
                fixture
                    .read(cx)
                    .state
                    .read(cx)
                    .selected_value()
                    .map(String::as_str),
                Some("Beta")
            );
            assert_eq!(fixture.read(cx).confirmed, [Some("Beta".into())]);
            assert_eq!(window.find("choice").expanded(), Some(false));
            assert_eq!(window.find("choice").focused(), Some(true));
            window.press("enter", cx);
            window.press("down", cx);
            window.press("enter", cx);
        })
        .unwrap();
        cx.run_until_parked();

        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(
                fixture
                    .read(cx)
                    .state
                    .read(cx)
                    .selected_value()
                    .map(String::as_str),
                Some("Alpha")
            );
            assert_eq!(
                fixture.read(cx).confirmed,
                [Some("Beta".into()), Some("Alpha".into())]
            );
            window.within("choice").click("input", cx);
            window.press("down", cx);
            window.press("escape", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("choice").expanded(), Some(false));
            assert_eq!(window.find("choice").focused(), Some(true));
            assert_eq!(
                fixture
                    .read(cx)
                    .state
                    .read(cx)
                    .selected_value()
                    .map(String::as_str),
                Some("Alpha")
            );
            assert_eq!(fixture.read(cx).confirmed.len(), 2);
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn disabled_select_and_cancel_preserve_owner_supplied_value(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.set_reduce_motion(true);
        });
        let mut fixture = None;
        let handle = cx.open_window(size(px(420.), px(320.)), |window, cx| {
            let view = cx.new(|cx| SelectFixture::new(window, cx));
            view.update(cx, |view, _| view.disabled = true);
            fixture = Some(view.clone());
            Root::new(view, window, cx)
        });
        let fixture = fixture.unwrap();

        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.within("choice").click("input", cx);
            window.press("down", cx);
            window.press("enter", cx);
            assert_eq!(window.find("choice").expanded(), Some(false));
            assert!(window.try_find("source-options").is_none());
            assert_eq!(
                fixture
                    .read(cx)
                    .state
                    .read(cx)
                    .selected_value()
                    .map(String::as_str),
                Some("Alpha")
            );
            assert!(fixture.read(cx).confirmed.is_empty());

            fixture.update(cx, |view, cx| {
                view.disabled = false;
                view.state.update(cx, |state, cx| {
                    state.set_selected_value(&"Gamma".into(), window, cx);
                    state.focus(window, cx);
                });
                cx.notify();
            });
            window.render_frame(cx);
            assert_eq!(window.find("choice").value(), Some("Gamma"));
            window.press("enter", cx);
            assert_eq!(window.find("option-Gamma").selected(), Some(true));
            window.press("up", cx);
            window.press("escape", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(
                fixture
                    .read(cx)
                    .state
                    .read(cx)
                    .selected_value()
                    .map(String::as_str),
                Some("Gamma")
            );
            assert!(fixture.read(cx).confirmed.is_empty());
            assert_eq!(window.find("choice").value(), Some("Gamma"));
            assert_eq!(window.find("choice").focused(), Some(true));
            assert!(window.try_find("source-options").is_none());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn changing_options_refreshes_open_and_reopened_menu_without_losing_cursor(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.set_reduce_motion(true);
        });
        let mut fixture = None;
        let handle = cx.open_window(size(px(420.), px(320.)), |window, cx| {
            let view = cx.new(|cx| SelectFixture::new(window, cx));
            view.update(cx, |view, cx| {
                view.set_options(&["Alpha", "Beta", "Gamma"], "Alpha", window, cx);
            });
            fixture = Some(view.clone());
            Root::new(view, window, cx)
        });
        let fixture = fixture.unwrap();

        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.within("choice").click("input", cx);
            window.press("down", cx);
            window.press("down", cx); // Focus is Beta, committed value is Alpha.
            fixture.update(cx, |view, cx| {
                view.set_options(&["Inserted", "Alpha", "Beta", "Gamma"], "Alpha", window, cx);
            });
            window.render_frame(cx);
            assert_eq!(window.find("choice").expanded(), Some(true));
            assert!(window.find("option-Inserted").visible());
            assert_eq!(window.find("source-options").bounds().size.height, px(102.));
            assert!(fixture.read(cx).confirmed.is_empty());
            window.press("enter", cx);
        })
        .unwrap();
        cx.run_until_parked();

        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("choice").value(), Some("Beta"));
            assert_eq!(fixture.read(cx).confirmed, [Some("Beta".into())]);
            window.within("choice").click("input", cx);
            fixture.update(cx, |view, cx| {
                view.set_options(&["Alpha", "Replacement", "Tail"], "Alpha", window, cx);
            });
            window.render_frame(cx);
            assert_eq!(window.find("choice").expanded(), Some(true));
            assert!(window.try_find("option-Beta").is_none());
            assert!(window.try_find("option-Inserted").is_none());
            assert_eq!(window.find("source-options").bounds().size.height, px(77.));
            assert_eq!(window.find("option-Alpha").selected(), Some(true));
            assert_eq!(fixture.read(cx).confirmed.len(), 1);
            window.press("down", cx);
            window.press("down", cx);
            // An unrelated parent redraw must leave the pending cursor alone.
            fixture.update(cx, |_, cx| cx.notify());
            window.render_frame(cx);
            window.press("enter", cx);
        })
        .unwrap();
        cx.run_until_parked();

        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("choice").value(), Some("Replacement"));
            assert_eq!(fixture.read(cx).confirmed.len(), 2);
            fixture.update(cx, |view, cx| {
                view.set_options(
                    &["Alpha", "Replacement", "Tail", "Reopened"],
                    "Replacement",
                    window,
                    cx,
                );
            });
            window.render_frame(cx);
            window.within("choice").click("input", cx);
            assert!(window.find("option-Reopened").visible());
            window.click("option-Reopened", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_eq!(window.find("choice").value(), Some("Reopened"));
            assert_eq!(
                fixture.read(cx).confirmed,
                [
                    Some("Beta".into()),
                    Some("Replacement".into()),
                    Some("Reopened".into()),
                ]
            );
        })
        .unwrap();
    }
}
