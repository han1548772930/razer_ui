//! Actual row counter helper; 162 has its independently retained space-only branch.
use super::*;

#[derive(IntoElement)]
pub(super) struct Index {
    key: String,
    label: String,
    policy: &'static Policy,
    disabled: bool,
    hovered: bool,
    owner: WeakEntity<MouseProductWorkspace>,
    sticky: Rc<RefCell<BTreeSet<String>>>,
    bounds: Rc<RefCell<BTreeMap<String, Bounds<Pixels>>>>,
    tip_bounds: Rc<RefCell<BTreeMap<String, Bounds<Pixels>>>>,
}
impl Index {
    pub(super) fn new(
        view: &MouseProductWorkspace,
        key: String,
        label: String,
        disabled: bool,
        policy: &'static Policy,
        cx: &Context<MouseProductWorkspace>,
    ) -> Self {
        let state = &view.sidebar_current_state;
        Self {
            hovered: state.index_hover.borrow().as_ref() == Some(&key),
            key,
            label,
            policy,
            disabled,
            owner: cx.entity().downgrade(),
            sticky: state.index_sticky.clone(),
            bounds: state.index_bounds.clone(),
            tip_bounds: state.index_tip_bounds.clone(),
        }
    }
}
fn hover(
    owner: &WeakEntity<MouseProductWorkspace>,
    key: &str,
    entered: bool,
    is_tip: bool,
    window: &Window,
    cx: &mut App,
) {
    let _ = owner.update(cx, |view, cx| {
        let state = &mut view.sidebar_current_state;
        let other = if is_tip {
            &state.index_bounds
        } else {
            &state.index_tip_bounds
        };
        let inside = entered
            || state.index_hover.borrow().as_deref() == Some(key)
                && other
                    .borrow()
                    .get(key)
                    .is_some_and(|b| b.contains(&window.mouse_position()));
        let next = if inside {
            Some(key.to_owned())
        } else if state.index_hover.borrow().as_deref() == Some(key) {
            None
        } else {
            state.index_hover.borrow().clone()
        };
        if next != *state.index_hover.borrow() {
            *state.index_hover.borrow_mut() = next;
            cx.notify();
        }
    });
}
impl RenderOnce for Index {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let slash = self.policy.split_slash && self.label.contains('/');
        let split = self
            .label
            .split(if slash { '/' } else { ' ' })
            .map(str::to_owned)
            .collect::<Vec<_>>();
        let parts = if split
            .iter()
            .map(|s| s.encode_utf16().count())
            .sum::<usize>()
            >= self.policy.split_min_utf16
        {
            split
        } else {
            vec![self.label.clone()]
        };
        // Same verified measure algorithm as keyboard's actual imported helper,
        // with product-specific splitting and sticky threshold from the mouse.
        let mut preferred = tip().child(self.label.clone()).into_any_element();
        let preferred = preferred.layout_as_root(
            size(AvailableSpace::MaxContent, AvailableSpace::MinContent),
            window,
            cx,
        );
        let mut minimum = tip().child(self.label.clone()).into_any_element();
        let minimum = minimum.layout_as_root(
            size(AvailableSpace::MinContent, AvailableSpace::MinContent),
            window,
            cx,
        );
        let tip_width = preferred
            .width
            .min((window.rem_size() * (200. / 16.)).max(minimum.width));
        let ratio = f32::from((tip_width - px(2.)).max(px(0.)))
            / f32::from(window.rem_size() * (56. / 16.));
        let font = if ratio <= 1. {
            14.
        } else if ratio <= 1.125 {
            13.
        } else {
            10.
        };
        let newly_sticky = ratio > 1.125
            && parts.len() > 1
            && parts
                .iter()
                .any(|s| s.encode_utf16().count() > self.policy.sticky_min_utf16);
        let sticky = self.sticky.borrow().contains(&self.key) || newly_sticky;
        let allow = (sticky || slash) && !self.disabled;
        let opacity = gpui_kit::base::motion::transition(
            ElementId::from((
                ElementId::from(SharedString::from(format!(
                    "mouse-sidebar-index-opacity-{}",
                    self.key
                ))),
                SharedString::from(self.owner.entity_id().to_string()),
            )),
            if self.hovered && allow { 1. } else { 0. },
            gpui_kit::base::motion::Transition::new(Duration::from_millis(300))
                .easing(gpui_kit::base::motion::Easing::Linear),
            window,
            cx,
        );
        let trigger = Rc::new(Cell::new(Bounds::default()));
        let observed = trigger.clone();
        let key = self.key.clone();
        let tip_key = self.key.clone();
        let hover_key = self.key.clone();
        let bounds = self.bounds;
        let tip_bounds = self.tip_bounds;
        let sticky_cache = self.sticky;
        let owner = self.owner.clone();
        let tip_owner = self.owner;
        let tooltip = div()
            .w(surface::css(200.))
            .opacity(opacity)
            .when(!self.hovered || !allow, |tip| tip.invisible())
            .child(
                tip()
                    .id(SharedString::from(format!(
                        "mouse-sidebar-index-tip-{}",
                        self.key
                    )))
                    .w(tip_width)
                    .on_prepaint(move |bounds, _, _| {
                        tip_bounds.borrow_mut().insert(tip_key, bounds);
                    })
                    .on_hover(move |entered, window, cx| {
                        hover(&tip_owner, &hover_key, *entered, true, window, cx)
                    })
                    .child(self.label),
            );
        let enter_key = self.key.clone();
        div()
            .id(SharedString::from(format!(
                "mouse-sidebar-index-{}",
                self.key
            )))
            .w_full()
            .on_prepaint(move |rect, _, _| {
                observed.set(rect);
                bounds.borrow_mut().insert(key.clone(), rect);
                if newly_sticky {
                    sticky_cache.borrow_mut().insert(key);
                }
            })
            .when(allow, |view| {
                view.on_hover(move |entered, window, cx| {
                    hover(&owner, &enter_key, *entered, false, window, cx)
                })
            })
            .children(parts.into_iter().map(|part| {
                div()
                    .text_size(surface::css(font))
                    .when(sticky, |part| {
                        part.overflow_hidden().whitespace_nowrap().text_ellipsis()
                    })
                    .child(part)
            }))
            .child(
                deferred(Layer::new(
                    LayerKind::Index(trigger),
                    tooltip.into_any_element(),
                ))
                .with_priority(151),
            )
    }
}
