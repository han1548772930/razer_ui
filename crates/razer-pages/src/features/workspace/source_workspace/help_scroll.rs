//! Eo requests scrollBy on the actual product body, after its popup is laid out.
use super::*;
use gpui_kit::base::spring;

pub(super) struct Target {
    to: Pixels,
    last: Option<Pixels>,
}

impl SourceProductWorkspace {
    pub(super) fn request_help_scroll(&mut self, distance: Pixels, cx: &mut Context<Self>) {
        if !self.active
            || !self
                .current_page()
                .is_some_and(|page| page.role() == ProductPageRole::Help)
            || distance <= px(0.)
        {
            return;
        }
        let to = (self.body_scroll.offset().y - distance)
            .min(px(0.))
            .max(-self.body_scroll.max_offset().y);
        self.help_scroll = Some(Target { to, last: None });
        cx.notify();
    }

    pub(super) fn advance_help_scroll(
        &mut self,
        is_help: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !is_help || !self.active {
            self.help_scroll = None;
            return;
        }
        let Some(target) = &mut self.help_scroll else {
            return;
        };
        let offset = self.body_scroll.offset();
        // Manual scrolling takes ownership back, including a scrollbar drag.
        if target.last.is_some_and(|last| last != offset.y) {
            self.help_scroll = None;
            return;
        }
        let id = SharedString::from(format!("source-help-scroll-{:?}", cx.entity().entity_id()));
        let policy = cx.theme().motion_tokens().spring_move;
        if target.last.is_none() {
            spring(id.clone(), offset.y, policy.with_travel(false), window, cx);
        }
        let y = spring(id, target.to, policy, window, cx);
        self.body_scroll.set_offset(point(offset.x, y));
        if y == target.to {
            self.help_scroll = None;
        } else {
            target.last = Some(y);
        }
    }
}
