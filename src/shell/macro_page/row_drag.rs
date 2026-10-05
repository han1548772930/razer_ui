//! 58190 H/F/va: group dragging and source after-row insertion boundaries.
use super::*;
use std::{cell::Cell, rc::Rc};

#[derive(Clone)]
pub(super) struct ActionDrag {
    pub(super) page: EntityId,
    pub(super) document: Option<u64>,
    pub(super) baseline: Rc<Vec<ActionItem>>,
    pub(super) indices: Vec<usize>,
    pub(super) palette_kind: Option<&'static str>,
    pub(super) kind: ActionKind,
    pub(super) allowed: Rc<Cell<bool>>,
    top: usize,
    bottom: usize,
}

impl ActionDrag {
    pub(super) fn allows(&self, insertion: usize) -> bool {
        insertion <= self.baseline.len() && insertion >= self.top && insertion <= self.bottom
    }
}

impl MacroPage {
    pub(super) fn action_drag(
        &self,
        index: usize,
        palette_kind: Option<&'static str>,
        page: EntityId,
        baseline: Rc<Vec<ActionItem>>,
    ) -> ActionDrag {
        let indices: Vec<_> = if palette_kind.is_some() {
            Vec::new()
        } else if self.selected_actions.contains(&index) {
            (0..baseline.len())
                .filter(|i| self.selected_actions.contains(i))
                .collect()
        } else {
            vec![index]
        };
        let (mut top, mut bottom) = (0, baseline.len());
        for &i in &indices {
            if let Some(pair) = row_actions::counterpart(&baseline, i)
                && !indices.contains(&pair)
            {
                if pair > i {
                    bottom = bottom.min(pair);
                } else {
                    top = top.max(pair + 1);
                }
            }
        }
        let kind = palette_kind
            .and_then(ActionKind::from_palette)
            .unwrap_or_else(|| baseline[index].kind);
        ActionDrag {
            page,
            document: self.current,
            baseline,
            indices,
            palette_kind,
            kind,
            allowed: Rc::new(Cell::new(true)),
            top,
            bottom,
        }
    }
}

pub(super) struct ActionDragPreview {
    pub(super) drag: ActionDrag,
    pub(super) offset: Point<Pixels>,
}

impl Render for ActionDragPreview {
    fn render(&mut self, window: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let delta = window.rem_size() * (10. / 16.);
        let icon = if self.drag.palette_kind.is_none() && self.drag.indices.len() != 1 {
            "layers"
        } else {
            self.drag.kind.icon()
        };
        let label = if self.drag.palette_kind.is_some() {
            tr(match self.drag.kind {
                ActionKind::Delay => "ADD_DELAY",
                ActionKind::Keyboard => "ADD_KEYBOARD_FUNCTION",
                ActionKind::Mouse => "ADD_MOUSE_FUNCTION",
                ActionKind::Macro => "ADD_MACRO",
                ActionKind::Launch => "ADD_LAUNCH",
                ActionKind::Command => "ADD_RUN_COMMAND",
                ActionKind::Text => "ADD_TEXT_FUNCTION",
                ActionKind::Loop => "ADD_LOOP",
            })
        } else {
            tr("MOVE_ACTIONS").replace("{{count}}", &self.drag.indices.len().to_string())
        };
        h_flex()
            .w(css(280.))
            .h(css(40.))
            .px(css(20.))
            .py(css(10.))
            .ml(self.offset.x - delta)
            .mt(self.offset.y - delta)
            .bg(rgb(if self.drag.allowed.get() {
                0x44d62c
            } else {
                0xc8323c
            }))
            .rounded(css(5.))
            .font_family("Roboto")
            .text_color(rgb(0x212121))
            .text_size(css(14.))
            .font_weight(FontWeight::BOLD)
            .child(
                img(SharedString::from(format!("synapse/macro/drag-{icon}.svg")))
                    .size(css(20.))
                    .mr(css(10.)),
            )
            .child(label)
    }
}

/// A 1px solid lower border: both va and br use dragging/no_drop classes.
pub(super) fn target_style(
    style: StyleRefinement,
    drag: &ActionDrag,
    page: EntityId,
    insertion: usize,
) -> StyleRefinement {
    if drag.page != page {
        return style;
    }
    let allowed = drag.allows(insertion);
    drag.allowed.set(allowed);
    style
        .border_b_1()
        .border_color(rgb(if allowed { 0x44d62c } else { 0xc8323c }))
}
