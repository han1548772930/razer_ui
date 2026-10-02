//! Keep viewport constraints on the native scroll owner, never on its content.
//!
//! Kit 0.7's overflow_*_scrollbar wrapper copies max_size to its outer node but
//! also retains it on an auto-sized content node. In height-limited dialogs that
//! can cap the measured content at the viewport height. Use GPUI's native scroll
//! container and Kit's scrollbar on the same node instead. Both still own all
//! wheel, gesture, hit testing and thumb-drag behavior.
use gpui_kit::base::InteractiveElementExt as _;
use gpui_kit::component::scroll::{ScrollableElement as _, ScrollbarAxis};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::panic::Location;

pub(crate) trait SourceScrollable:
    InteractiveElement + Styled + ParentElement + Element + Sized
{
    #[track_caller]
    fn scrollable_both(self) -> SourceScroll<Self> {
        SourceScroll::new(self, ScrollbarAxis::Both)
    }

    #[track_caller]
    fn scrollable_y(self) -> SourceScroll<Self> {
        SourceScroll::new(self, ScrollbarAxis::Vertical)
    }
}

impl<E: InteractiveElement + Styled + ParentElement + Element> SourceScrollable for E {}

#[derive(IntoElement)]
pub(crate) struct SourceScroll<E: InteractiveElement + Styled + ParentElement + Element> {
    element: E,
    id: ElementId,
    axis: ScrollbarAxis,
    scroll: Option<ScrollHandle>,
}

impl<E: InteractiveElement + Styled + ParentElement + Element> SourceScroll<E> {
    #[track_caller]
    fn new(mut element: E, axis: ScrollbarAxis) -> Self {
        let id = element
            .interactivity()
            .element_id
            .clone()
            .unwrap_or_else(|| ElementId::CodeLocation(*Location::caller()));
        Self {
            element,
            id,
            axis,
            scroll: None,
        }
    }

    pub(crate) fn track_scroll(mut self, scroll: &ScrollHandle) -> Self {
        self.scroll = Some(scroll.clone());
        self
    }
}

impl<E: InteractiveElement + Styled + ParentElement + Element> Styled for SourceScroll<E> {
    fn style(&mut self) -> &mut StyleRefinement {
        self.element.style()
    }
}

impl<E: InteractiveElement + Styled + ParentElement + Element> ParentElement for SourceScroll<E> {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.element.extend(elements)
    }
}

impl<E: InteractiveElement + Styled + ParentElement + Element + 'static> RenderOnce
    for SourceScroll<E>
{
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let scroll = self.scroll.unwrap_or_else(|| {
            window
                .use_keyed_state((self.id.clone(), "source-scroll"), cx, |_, _| {
                    ScrollHandle::new()
                })
                .read(cx)
                .clone()
        });
        self.element
            .id(self.id)
            .relative()
            .map(|element| match self.axis {
                ScrollbarAxis::Vertical => element.overflow_y_scroll().lock_scroll_axis(),
                _ => element.overflow_scroll(),
            })
            .track_scroll(&scroll)
            .scrollbar(&scroll, self.axis)
    }
}

#[cfg(test)]
#[path = "scroll_tests.rs"]
mod tests;
