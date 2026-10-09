//! Host-specific reorder thresholds; GPUI owns the drag gesture and its lifetime.
use super::{AppShell, HostTab, MoveTabLeft, MoveTabRight};
use gpui_kit::*;

#[derive(Clone)]
pub(super) struct DragTab(pub(super) HostTab);

pub(super) struct TabDragPreview;
impl Render for TabDragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        // TabUI keeps the dragged tab inside its strip, with no detached ghost.
        Empty
    }
}

pub(super) struct TabDrag {
    pub(super) tab: HostTab,
    pub(super) left: f32,
    session: EntityId,
    grab: f32,
    last_x: f32,
    widths: Vec<(HostTab, f32)>,
    suspended: bool,
}
impl TabDrag {
    fn width(&self, tab: &HostTab) -> f32 {
        self.widths
            .iter()
            .find(|(key, _)| key == tab)
            .map(|(_, width)| *width)
            .unwrap_or(90.)
    }
}

impl AppShell {
    pub fn move_host_tab_left(
        &mut self,
        _: &MoveTabLeft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_host_tab(-1, window, cx);
    }
    pub fn move_host_tab_right(
        &mut self,
        _: &MoveTabRight,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_host_tab(1, window, cx);
    }
    fn move_host_tab(&mut self, delta: isize, window: &mut Window, cx: &mut Context<Self>) {
        if self.host_tabs.drag.is_some()
            || window
                .context_stack()
                .iter()
                .any(|context| context.contains("Dialog"))
        {
            return;
        }
        let Some(ix) = self
            .host_tabs
            .open
            .iter()
            .position(|entry| entry.tab.location() == self.location)
        else {
            return;
        };
        let target = ix as isize + delta;
        if target >= 0 && (target as usize) < self.host_tabs.open.len() {
            self.host_tabs.open.swap(ix, target as usize);
            self.host_tabs.order_changed = true;
            self.host_tabs.scroll.scroll_to_item(target as usize);
            self.save_auxiliary_preferences(cx);
            cx.notify();
        }
    }

    pub(super) fn start_host_tab_drag(
        &mut self,
        tab: HostTab,
        offset: Point<Pixels>,
        preview: &Entity<TabDragPreview>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.navigate(tab.location(), window, cx);
        // A draft guard may keep the previous page open; do not start a drag
        // underneath that modal or steal its focus.
        if self.location != tab.location() {
            return;
        }
        let scale = f32::from(window.rem_size()) / 16.;
        let scroll = &self.host_tabs.scroll;
        let Some(ix) = self
            .host_tabs
            .open
            .iter()
            .position(|entry| entry.tab == tab)
        else {
            return;
        };
        let Some(bounds) = scroll.bounds_for_item(ix) else {
            return;
        };
        let widths = self
            .host_tabs
            .open
            .iter()
            .enumerate()
            .filter_map(|(ix, entry)| {
                scroll
                    .bounds_for_item(ix)
                    .map(|bounds| (entry.tab.clone(), f32::from(bounds.size.width) / scale))
            })
            .collect();
        self.host_tabs.drag = Some(TabDrag {
            tab,
            left: f32::from(bounds.left() - scroll.bounds().left()) / scale,
            session: preview.entity_id(),
            grab: (f32::from(offset.x) / scale + 1.).max(0.),
            last_x: f32::from(window.mouse_position().x) / scale,
            widths,
            suspended: false,
        });
        let session = preview.entity_id();
        self.host_tabs.drag_subscription =
            Some(
                cx.observe_release_in(preview, window, move |shell, _, _, cx| {
                    if shell
                        .host_tabs
                        .drag
                        .as_ref()
                        .is_some_and(|drag| drag.session == session)
                    {
                        shell.host_tabs.drag = None;
                        if let Some(ix) = shell
                            .host_tabs
                            .open
                            .iter()
                            .position(|entry| entry.tab.location() == shell.location)
                        {
                            shell.host_tabs.scroll.scroll_to_item(ix);
                        }
                        if shell.host_tabs.order_pending() {
                            shell.save_auxiliary_preferences(cx);
                        }
                        cx.notify();
                    }
                }),
            );
        cx.notify();
    }

    pub(super) fn move_host_tab_drag(
        &mut self,
        event: &DragMoveEvent<DragTab>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(drag) = self.host_tabs.drag.as_mut() else {
            return;
        };
        if event.drag(cx).0 != drag.tab {
            return;
        }
        // TabUI.onMouseLeave stops tracking for the rest of this gesture once
        // the pointer exits the window; mouse-up still settles the current order.
        let position = event.event.position;
        let viewport_size = window.viewport_size();
        if position.x <= px(0.)
            || position.y <= px(0.)
            || position.x >= viewport_size.width
            || position.y >= viewport_size.height
        {
            drag.suspended = true;
        }
        if drag.suspended {
            return;
        }
        let scale = f32::from(window.rem_size()) / 16.;
        let pointer = f32::from(event.event.position.x) / scale;
        let scroll = &self.host_tabs.scroll;
        let origin = f32::from(scroll.bounds().left()) / scale;
        let offset = f32::from(scroll.offset().x) / scale;
        let viewport = f32::from(scroll.bounds().size.width) / scale;
        let width = drag.width(&drag.tab);
        let total = (self
            .host_tabs
            .open
            .iter()
            .map(|entry| drag.width(&entry.tab) + 4.)
            .sum::<f32>()
            - 4.)
            .max(width);
        let left = (pointer - origin - offset - drag.grab).clamp(0., total - width);
        let Some(ix) = self
            .host_tabs
            .open
            .iter()
            .position(|entry| entry.tab == drag.tab)
        else {
            return;
        };
        let moving_right = pointer > drag.last_x;
        if pointer != drag.last_x {
            let neighbor = if moving_right {
                ix.checked_add(1)
                    .filter(|ix| *ix < self.host_tabs.open.len())
            } else {
                ix.checked_sub(1)
            };
            if let Some(neighbor) = neighbor {
                let neighbor_left = self
                    .host_tabs
                    .open
                    .iter()
                    .take(neighbor)
                    .map(|entry| drag.width(&entry.tab) + 4.)
                    .sum();
                let neighbor_width = drag.width(&self.host_tabs.open[neighbor].tab);
                if crosses_neighbor(left, width, neighbor_left, neighbor_width, moving_right) {
                    self.host_tabs.open.swap(ix, neighbor);
                    self.host_tabs.order_changed = true;
                }
            }
        }
        drag.left = left;
        drag.last_x = pointer;
        // Keep the grabbed point at the edge while traversing an overflowing strip.
        let target = if left < -offset {
            -left
        } else if left + width > -offset + viewport {
            viewport - left - width
        } else {
            offset
        };
        scroll.set_offset(point(
            px(target.clamp(-(total - viewport).max(0.), 0.) * scale),
            px(0.),
        ));
        cx.notify();
    }
}

fn crosses_neighbor(
    left: f32,
    width: f32,
    neighbor_left: f32,
    neighbor_width: f32,
    right: bool,
) -> bool {
    if right {
        left + width >= neighbor_left + neighbor_width / 3.
    } else {
        left <= neighbor_left + neighbor_width * 2. / 3.
    }
}

#[cfg(test)]
mod tests {
    use super::crosses_neighbor;

    #[test]
    fn unequal_tabs_switch_at_the_neighbors_third_and_reverse_symmetrically() {
        assert!(!crosses_neighbor(79., 90., 94., 228., true));
        assert!(crosses_neighbor(80., 90., 94., 228., true));
        assert!(!crosses_neighbor(61., 228., 0., 90., false));
        assert!(crosses_neighbor(60., 228., 0., 90., false));
    }
}
