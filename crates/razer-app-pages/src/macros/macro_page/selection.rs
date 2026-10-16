//! 58190.br/va/be and the current main reducer's selectAll/deselectAll.
use super::*;

impl MacroPage {
    pub fn hovered_pair(&self) -> Option<(usize, usize)> {
        if self.recording_busy() {
            return None;
        }
        let index = self.hovered_action?;
        let other = row_actions::counterpart(self.actions(), index)?;
        Some((index.min(other), index.max(other)))
    }

    /// ja -> Le: hover uses 15030.cf, independent from selection highlights.
    pub fn pairing_line(&self, (start, end): (usize, usize)) -> AnyElement {
        let height = ((end - start) * 42) as f32;
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let unit = f32::from(window.rem_size()) / 16.;
                // Only emit visible dashes even for a long imported macro.
                let first = ((-f32::from(bounds.top()) / unit - 5.) / 7.)
                    .floor()
                    .max(0.) as usize;
                let last = ((f32::from(window.viewport_size().height - bounds.top()) / unit) / 7.)
                    .ceil()
                    .max(0.) as usize;
                for n in first..=last.min((height / 7.).ceil() as usize) {
                    let y = n as f32 * 7.;
                    let h = (height - y).min(5.);
                    if h > 0. {
                        window.paint_quad(fill(
                            Bounds::new(
                                bounds.origin + point(px(0.), px(y * unit)),
                                size(px(2. * unit), px(h * unit)),
                            ),
                            rgb(0x44d62c),
                        ));
                    }
                }
                for y in [0., height] {
                    let mut dot = PathBuilder::fill();
                    let center = bounds.origin + point(px(unit), px(y * unit));
                    for n in 0..=16 {
                        let angle = n as f32 / 16. * std::f32::consts::TAU;
                        let p = center
                            + point(px(angle.cos() * 2.5 * unit), px(angle.sin() * 2.5 * unit));
                        if n == 0 {
                            dot.move_to(p);
                        } else {
                            dot.line_to(p);
                        }
                    }
                    dot.close();
                    if let Ok(dot) = dot.build() {
                        window.paint_path(dot, rgb(0x44d62c));
                    }
                }
            },
        )
        .absolute()
        .left(css(39.))
        .top(css((start * 42 + 21) as f32))
        .w(css(2.))
        .h(css(height))
        .into_any_element()
    }

    pub fn all_actions_selected(&self) -> bool {
        !self.actions().is_empty()
            && (0..self.actions().len()).all(|index| self.selected_actions.contains(&index))
    }

    pub fn toggle_all_actions(&mut self, cx: &mut Context<Self>) {
        if self.recording_busy() || self.record_ui.open || self.tutorial != Tutorial::Complete {
            return;
        }
        self.selected_actions = if self.all_actions_selected() {
            Vec::new()
        } else {
            (0..self.actions().len()).collect()
        };
        cx.notify();
    }

    // 25572.M highlights the unselected counterpart without selecting it.
    pub fn action_pair_highlighted(&self, index: usize) -> bool {
        let Some(item) = self.actions().get(index) else {
            return false;
        };
        let (Some(pair), Some(state)) =
            (row_actions::pair_id(item), row_actions::event_state(item))
        else {
            return false;
        };
        let opposite = if state == 1 { 0 } else { 1 };
        self.actions()
            .iter()
            .enumerate()
            .any(|(other_index, item)| {
                item.kind == self.actions()[index].kind
                    && row_actions::pair_id(item) == Some(pair)
                    && row_actions::event_state(item) == Some(opposite)
                    && (self.selected_actions.contains(&index)
                        || self.selected_actions.contains(&other_index))
            })
    }
}

/// Macro mounts the shared input+label checkbox, not `.check-box.checked`.
/// Its label's 10px margin and the wrapper's `mr10` give a 40px total slot.
pub fn checkbox(
    id: impl Into<ElementId>,
    selected: bool,
    disabled: bool,
    window: &mut Window,
    cx: &mut App,
) -> BaseButton {
    let id = id.into();
    let top = Presence::new((id.clone(), "tick-top"), selected)
        // ticktop is 200ms with a zero-height keyframe at 50%.
        .transition(
            Transition::new(Duration::from_millis(100))
                .delay(Duration::from_millis(if selected { 100 } else { 0 }))
                .easing(Easing::Ease),
        )
        .sample(window, cx)
        .progress;
    let bottom = Presence::new((id.clone(), "tick-bottom"), selected)
        .transition(Transition::new(Duration::from_millis(100)).easing(Easing::Ease))
        .sample(window, cx)
        .progress;
    BaseButton::new(id)
        .role(Role::CheckBox)
        .selected(selected)
        .disabled(disabled)
        .when(disabled, |v| v.opacity(0.3))
        .size(css(20.))
        .flex_shrink_0()
        .mr(css(20.))
        .p_0()
        .rounded(css(2.4))
        .border_1()
        .border_color(rgb(if selected { 0x44d62c } else { 0x737373 }))
        .bg(if selected { rgb(0x44d62c) } else { rgba(0) })
        .when(!disabled, |v| v.hover(|s| s.border_color(rgb(0x44d62c))))
        .when(selected, |v| {
            v.child(
                canvas(
                    |_, _, _| (),
                    move |bounds, _, window, _| {
                        // The canvas occupies the 18px padding box; the CSS
                        // pseudo-elements are measured from its inner edge.
                        let scale = f32::from(window.rem_size()) / 16.;
                        for (x, y, angle, length) in [
                            (8.6_f32, 16.4_f32, -145_f32, 15.4 * top),
                            (0.8, 10.2, -50., 9.6 * bottom),
                        ] {
                            let angle = angle.to_radians();
                            if length <= 0. {
                                continue;
                            }
                            // CSS rotates a filled 3px rectangle about its
                            // top-left, not a centred stroked line.
                            let p = |dx: f32, dy: f32| {
                                bounds.origin
                                    + point(
                                        px((x + dx * angle.cos() - dy * angle.sin()) * scale),
                                        px((y + dx * angle.sin() + dy * angle.cos()) * scale),
                                    )
                            };
                            let r = 1.5_f32.min(length / 2.);
                            let k = r * 0.552_284_8;
                            let mut path = PathBuilder::fill();
                            path.move_to(p(r, 0.));
                            path.line_to(p(3. - r, 0.));
                            path.cubic_bezier_to(p(3., r), p(3. - r + k, 0.), p(3., r - k));
                            path.line_to(p(3., length - r));
                            path.cubic_bezier_to(
                                p(3. - r, length),
                                p(3., length - r + k),
                                p(3. - r + k, length),
                            );
                            path.line_to(p(r, length));
                            path.cubic_bezier_to(
                                p(0., length - r),
                                p(r - k, length),
                                p(0., length - r + k),
                            );
                            path.line_to(p(0., r));
                            path.cubic_bezier_to(p(r, 0.), p(0., r - k), p(r - k, 0.));
                            path.close();
                            if let Ok(path) = path.build() {
                                window.paint_path(path, rgb(0x111111));
                            }
                        }
                    },
                )
                .size_full(),
            )
        })
}

pub fn history_icon(kind: &'static str, enabled: bool) -> AnyElement {
    let asset = |suffix: &str| SharedString::from(format!("synapse/macro/{kind}{suffix}.svg"));
    div()
        .relative()
        .size(css(20.))
        .child(img(asset(if enabled { "-enable" } else { "" })).size_full())
        .when(enabled, |v| {
            v.child(
                img(asset("-hover"))
                    .absolute()
                    .inset_0()
                    .size_full()
                    .opacity(0.)
                    .group_hover(kind, |s| s.opacity(1.)),
            )
        })
        .into_any_element()
}
