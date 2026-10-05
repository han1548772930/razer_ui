//! 58190.dn/on/en positioning in current-frame web-content coordinates.
use super::*;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Instant,
};

#[derive(Default)]
pub(super) struct TextPlacement {
    previous_trigger: Option<Bounds<Pixels>>,
    changed_at: Option<Instant>,
    origin: Option<Point<Pixels>>,
    hidden: bool,
}

pub(super) enum TextAnchor {
    Modal {
        row: Rc<RefCell<text::TextRowState>>,
        editor: Rc<Cell<Bounds<Pixels>>>,
        viewport: Rc<Cell<Bounds<Pixels>>>,
    },
    Emoji {
        modal: Rc<Cell<Bounds<Pixels>>>,
    },
    Launch {
        trigger: Rc<Cell<Bounds<Pixels>>>,
        editor: Rc<Cell<Bounds<Pixels>>>,
        row_offset: f32,
        upward: Rc<Cell<Option<bool>>>,
    },
    Variant {
        cell: Rc<Cell<Bounds<Pixels>>>,
        panel: Rc<Cell<Bounds<Pixels>>>,
        main: Rc<Cell<Bounds<Pixels>>>,
        placement: Rc<Cell<Option<(Pixels, bool)>>>,
    },
}

pub(super) struct TextOverlay {
    pub(super) content: AnyElement,
    pub(super) anchor: TextAnchor,
}

impl IntoElement for TextOverlay {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}

impl Element for TextOverlay {
    type RequestLayoutState = ();
    type PrepaintState = bool;
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        self.content.layout_as_root(
            size(AvailableSpace::MaxContent, AvailableSpace::MinContent),
            window,
            cx,
        );
        (
            window.request_layout(
                Style {
                    position: Position::Absolute,
                    ..Default::default()
                },
                [],
                cx,
            ),
            (),
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) -> bool {
        let unit = window.rem_size() / 16.;
        let (origin, clip) = match &self.anchor {
            TextAnchor::Modal {
                row,
                editor,
                viewport,
            } => {
                let mut row = row.borrow_mut();
                let trigger = row.trigger.get();
                let now = cx.background_executor().now();
                let state = &mut row.placement;
                if state.previous_trigger.is_some_and(|old| old != trigger) {
                    state.changed_at = Some(now);
                }
                state.previous_trigger = Some(trigger);
                let pending = state
                    .changed_at
                    .is_some_and(|time| now.duration_since(time) < Duration::from_millis(150));
                if state.origin.is_none() || !pending && state.changed_at.is_some() {
                    let down = trigger.bottom() + unit * 10.;
                    state.origin = Some(point(
                        trigger.left(),
                        if down + unit * 250. > viewport.get().bottom() {
                            trigger.top() - unit * 250.
                        } else {
                            down
                        },
                    ));
                    // Source checks this only in the debounced editor scroll path.
                    if state.changed_at.is_some() {
                        let position = trigger.top() + unit * 8.;
                        state.hidden =
                            position < editor.get().top() || position > editor.get().bottom();
                    }
                    state.changed_at = None;
                }
                if pending {
                    window.request_animation_frame();
                }
                let origin = state.origin.unwrap_or_default();
                if state.hidden {
                    row.modal.set(Bounds::default());
                    row.emoji.set(Bounds::default());
                    return false;
                }
                (origin, None)
            }
            TextAnchor::Emoji { modal } => {
                // The unpositioned left uses the modal content edge (border + padding).
                (
                    point(
                        modal.get().left() + unit * 21.,
                        modal.get().top() + unit * 185.,
                    ),
                    None,
                )
            }
            TextAnchor::Launch {
                trigger,
                editor,
                row_offset,
                upward,
            } => {
                // rt.W: fourth ancestor is ja's relative #item_editor and
                // third ancestor is the 42px action row. offsetTop deliberately
                // excludes scrollTop. Direction is chosen only on opening.
                let upward = upward.get().unwrap_or_else(|| {
                    let value = editor.get().size.height - unit * *row_offset <= unit * 100.;
                    upward.set(Some(value));
                    value
                });
                let trigger = trigger.get();
                (
                    point(
                        trigger.left(),
                        if upward {
                            trigger.top() - unit * 300.
                        } else {
                            trigger.bottom()
                        },
                    ),
                    Some(ContentMask {
                        bounds: editor.get(),
                    }),
                )
            }
            TextAnchor::Variant {
                cell,
                panel,
                main,
                placement,
            } => {
                let cell = cell.get();
                // en computes its relative offsets on mouseenter only. Scrolling
                // moves the cell and tip together without flipping the tip again.
                let (left, down) = placement.get().unwrap_or_else(|| {
                    let left = -(cell.left() - panel.get().left() - unit * 11.);
                    let left = if left < unit * -280. {
                        unit * -10. + left + unit * 280.
                    } else if left < unit * -80. {
                        unit * -80.
                    } else {
                        left
                    };
                    let value = (left, cell.top() - panel.get().top() <= unit * 119.);
                    placement.set(Some(value));
                    value
                });
                (
                    point(
                        cell.left() + left,
                        cell.top() + cell.size.height * if down { 1. } else { -1. },
                    ),
                    Some(ContentMask { bounds: main.get() }),
                )
            }
        };
        window.with_content_mask(clip, |window| self.content.prepaint_at(origin, window, cx));
        true
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        visible: &mut bool,
        window: &mut Window,
        cx: &mut App,
    ) {
        if !*visible {
            return;
        }
        let clip = match &self.anchor {
            TextAnchor::Variant { main, .. } => Some(ContentMask { bounds: main.get() }),
            TextAnchor::Launch { editor, .. } => Some(ContentMask {
                bounds: editor.get(),
            }),
            _ => None,
        };
        window.with_content_mask(clip, |window| self.content.paint(window, cx));
    }
}
