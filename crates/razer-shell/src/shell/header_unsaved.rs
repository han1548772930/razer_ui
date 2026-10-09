//! Current App/96776/R and w: profile list, conditional trigger and dropdown lifecycle.
use gpui_kit::base::motion::{Easing, Presence, Transition};
use gpui_kit::base::{Button as BaseButton, PopoverState, Positioner};
use gpui_kit::component::{
    button::{Button, ButtonRounded, ButtonVariants},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_i18n as i18n;
use razer_widgets::surface;
use razer_widgets::theme::AccountMenuColors;
use razer_widgets::theme::HeaderStatusColors;
use std::{rc::Rc, time::Duration};

type Command = Rc<dyn Fn(&mut Window, &mut App)>;

pub(in crate::shell) fn unsaved_profiles(
    items: Vec<(String, String)>,
    saving: bool,
    on_save: impl Fn(&mut Window, &mut App) + 'static,
    on_discard: impl Fn(&mut Window, &mut App) + 'static,
    _: &App,
) -> AnyElement {
    UnsavedProfilesElement {
        items,
        saving,
        on_save: Rc::new(on_save),
        on_discard: Rc::new(on_discard),
    }
    .into_any_element()
}

#[derive(IntoElement)]
struct UnsavedProfilesElement {
    items: Vec<(String, String)>,
    saving: bool,
    on_save: Command,
    on_discard: Command,
}

impl RenderOnce for UnsavedProfilesElement {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let state = window.use_keyed_state("header-unsaved-state", cx, UnsavedProfiles::new);
        state.update(cx, |state, cx| {
            let changed = state.items != self.items || state.saving != self.saving;
            state.items = self.items;
            state.saving = self.saving;
            state.on_save = self.on_save;
            state.on_discard = self.on_discard;
            if state.items.is_empty() {
                state.lifecycle_task = None;
                state.rendered = false;
                state.shown = false;
                state
                    .popup
                    .update(cx, |popup, cx| popup.dismiss(window, cx));
            }
            if changed {
                cx.notify();
            }
        });
        state
    }
}

struct UnsavedProfiles {
    items: Vec<(String, String)>,
    saving: bool,
    on_save: Command,
    on_discard: Command,
    popup: Entity<PopoverState>,
    rendered: bool,
    shown: bool,
    was_open: bool,
    lifecycle_task: Option<Task<()>>,
    trigger_focus: FocusHandle,
    trigger_bounds: Bounds<Pixels>,
    _popup_observer: Subscription,
    _activation_observer: Subscription,
}

impl UnsavedProfiles {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let popup = cx.new(|cx| PopoverState::new(false, cx));
        let observer = cx.observe_in(&popup, window, |this, popup, window, cx| {
            let open = popup.read(cx).is_open();
            if open == this.was_open {
                return;
            }
            this.was_open = open;
            this.lifecycle_task = None;
            if this.items.is_empty() || cx.reduce_motion() {
                this.shown = open && !this.items.is_empty();
                this.rendered = this.shown;
            } else {
                if open {
                    this.rendered = true;
                } else {
                    this.shown = false;
                }
                // w delays .show by 100 ms, and unmounts 100 ms after hiding.
                // CSS still declares 200 ms transitions; do not lengthen removal.
                this.lifecycle_task = Some(cx.spawn_in(window, async move |this, cx| {
                    cx.background_executor()
                        .timer(Duration::from_millis(100))
                        .await;
                    let _ = this.update_in(cx, |this, _, cx| {
                        if open {
                            this.shown = true;
                        } else {
                            this.rendered = false;
                        }
                        this.lifecycle_task = None;
                        cx.notify();
                    });
                }));
            }
            cx.notify();
        });
        let activation = cx.observe_window_activation(window, |this, window, cx| {
            if !window.is_window_active() {
                this.popup.update(cx, |popup, cx| popup.dismiss(window, cx));
            }
        });
        Self {
            items: vec![],
            saving: false,
            on_save: Rc::new(|_, _| {}),
            on_discard: Rc::new(|_, _| {}),
            popup,
            rendered: false,
            shown: false,
            was_open: false,
            lifecycle_task: None,
            trigger_focus: cx.focus_handle().tab_stop(true),
            trigger_bounds: Bounds::default(),
            _popup_observer: observer,
            _activation_observer: activation,
        }
    }

    fn content(&self, interactive: bool, cx: &App) -> AnyElement {
        let divider = || {
            div()
                .h(surface::css(1.))
                .my(surface::css(10.))
                .bg(AccountMenuColors::border())
        };
        let count = self.items.len();
        let action = |save: bool| {
            let label = i18n::t(if save {
                "BTN_SAVE_ALL"
            } else {
                "BTN_DISCARD_ALL"
            });
            if interactive {
                let popup = self.popup.clone();
                let command = if save {
                    self.on_save.clone()
                } else {
                    self.on_discard.clone()
                };
                Button::new(if save {
                    "header-unsaved-save"
                } else {
                    "header-unsaved-discard"
                })
                .label(label)
                .h(surface::css(25.))
                .min_w(surface::css(105.))
                .px(surface::css(13.))
                .py_0()
                .text_size(surface::css(12.))
                .line_height(surface::css(12.))
                .rounded(ButtonRounded::Size(cx.theme().font_size * (2. / 16.)))
                .when(save, |button| button.primary())
                .when(!save, |button| button.outline())
                .disabled(self.saving)
                .on_click(move |_, window, cx| {
                    popup.update(cx, |popup, cx| popup.dismiss(window, cx));
                    command(window, cx);
                })
                .into_any_element()
            } else {
                div()
                    .h(surface::css(25.))
                    .min_w(surface::css(105.))
                    .px(surface::css(13.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(surface::css(2.))
                    .text_size(surface::css(12.))
                    .line_height(surface::css(12.))
                    .bg(if save {
                        cx.theme().primary
                    } else {
                        cx.theme().transparent
                    })
                    .text_color(if save {
                        cx.theme().primary_foreground
                    } else {
                        cx.theme().foreground
                    })
                    .when(!save, |button| {
                        button.border_1().border_color(cx.theme().border)
                    })
                    .opacity(if self.saving { 0.3 } else { 1. })
                    .child(label)
                    .into_any_element()
            }
        };
        v_flex()
            .min_w(surface::css(140.))
            .child(
                v_flex()
                    .max_w(surface::css(220.))
                    .mx_auto()
                    .px(surface::css(5.))
                    .py(surface::css(1.))
                    .text_size(surface::css(14.))
                    .line_height(surface::css(17.))
                    .text_center()
                    .text_color(cx.theme().muted_foreground)
                    .child(i18n::t("TEXT_UNSAVED_1"))
                    .child(i18n::t("TEXT_UNSAVED_2")),
            )
            .child(divider())
            .children(
                self.items
                    .iter()
                    .enumerate()
                    .map(|(index, (identity, title))| {
                        if interactive {
                            let popup = self.popup.clone();
                            Button::new(SharedString::from(format!(
                                "header-unsaved-item-{identity}"
                            )))
                            .ghost()
                            .label(title.clone())
                            .justify_start()
                            .w_full()
                            .h(surface::css(26.))
                            .px(surface::css(18.))
                            .when(index + 1 < count, |row| row.mb(surface::css(4.)))
                            .text_size(surface::css(14.))
                            .line_height(surface::css(17.))
                            .rounded(ButtonRounded::Size(cx.theme().font_size * (13. / 16.)))
                            .on_click(move |_, window, cx| {
                                popup.update(cx, |popup, cx| popup.dismiss(window, cx))
                            })
                            .into_any_element()
                        } else {
                            div()
                                .h(surface::css(26.))
                                .px(surface::css(18.))
                                .pt(surface::css(5.))
                                .when(index + 1 < count, |row| row.mb(surface::css(4.)))
                                .text_size(surface::css(14.))
                                .line_height(surface::css(17.))
                                .child(title.clone())
                                .into_any_element()
                        }
                    }),
            )
            .child(divider())
            .child(
                h_flex()
                    .justify_end()
                    .gap(surface::css(8.))
                    .px(surface::css(4.))
                    .child(action(false))
                    .child(action(true)),
            )
            .into_any_element()
    }
}

impl Render for UnsavedProfiles {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.items.is_empty() {
            return div().into_any_element();
        }
        let open = self.popup.read(cx).is_open();
        let interactive = open && self.shown;
        let transition = |easing| Transition::new(Duration::from_millis(200)).easing(easing);
        let (opacity, position) = if self.rendered {
            (
                Presence::new("unsaved-opacity", self.shown)
                    .transition(transition(Easing::Linear))
                    .sample(window, cx)
                    .progress,
                Presence::new("unsaved-position", self.shown)
                    .transition(transition(Easing::EaseOut))
                    .sample(window, cx)
                    .progress,
            )
        } else {
            (0., 0.)
        };
        let popup = self.popup.clone();
        let focus = self.popup.focus_handle(cx);
        let trigger_bounds = self.trigger_bounds;
        let anchor = trigger_bounds.bottom_right() + point(px(0.), window.rem_size() * (2. / 16.));
        let count = self.items.len();
        div()
            .id("header-unsaved-root")
            .flex_shrink_0()
            .when(open && !self.shown, |root| {
                root.on_mouse_down_out(cx.listener(|this, _, window, cx| {
                    this.popup.update(cx, |popup, cx| popup.dismiss(window, cx));
                }))
            })
            .child(
                BaseButton::new("header-unsaved-trigger")
                    .accessibility_label(i18n::t("TEXT_UNSAVED_2"))
                    .aria_description(format!("{} ({count})", i18n::t("TEXT_UNSAVED_1")))
                    .track_focus(&self.trigger_focus)
                    .relative()
                    .w(surface::css(46.))
                    .h(surface::css(38.))
                    .bg(if self.rendered {
                        HeaderStatusColors::offline_hover()
                    } else {
                        cx.theme().transparent
                    })
                    .hover(|button| button.bg(cx.theme().secondary_hover))
                    .focus_visible(|button| button.border_1().border_color(cx.theme().primary))
                    .child(
                        div()
                            .size(surface::css(32.))
                            .rounded_full()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(HeaderStatusColors::status_surface())
                            .child(img("synapse/header-unsaved.svg").size(surface::css(20.))),
                    )
                    .child(
                        div()
                            .id("header-unsaved-count")
                            .test_support()
                            .absolute()
                            .right(surface::css(3.))
                            .bottom(surface::css(3.))
                            .min_w(surface::css(18.))
                            .h(surface::css(18.))
                            .px(surface::css(6.))
                            .rounded(surface::css(9.))
                            .bg(cx.theme().danger)
                            .text_color(cx.theme().foreground)
                            .text_size(surface::css(10.))
                            .font_weight(FontWeight::BOLD)
                            .line_height(surface::css(19.))
                            .text_center()
                            .child(count.to_string()),
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        let open = this.popup.read(cx).is_open();
                        // R keeps its active flag until w's onHidden callback.
                        if !open && this.rendered {
                            return;
                        }
                        if !open {
                            this.trigger_focus.focus(window, cx);
                        }
                        this.popup.update(cx, |popup, cx| {
                            if open {
                                popup.dismiss(window, cx);
                            } else {
                                popup.show(window, cx);
                            }
                        });
                    }))
                    .on_prepaint({
                        let owner = cx.entity().downgrade();
                        move |bounds, _, cx| {
                            let _ = owner.update(cx, |this, cx| {
                                if this.trigger_bounds != bounds {
                                    this.trigger_bounds = bounds;
                                    cx.notify();
                                }
                            });
                        }
                    }),
            )
            .when(open || self.rendered, |root| {
                root.child(
                    deferred(
                        Positioner::corner(Anchor::TopRight, anchor)
                            .margin(px(8.))
                            .when(interactive, |positioner| positioner.occlude())
                            .child(
                                div()
                                    .id("header-unsaved-menu")
                                    .test_support()
                                    .role(Role::Dialog)
                                    .aria_label(i18n::t("TEXT_UNSAVED_2"))
                                    .track_focus(&focus)
                                    .key_context("Popover")
                                    .tab_group()
                                    .on_action(cx.listener(
                                        |this, _: &gpui_kit::base::actions::Cancel, window, cx| {
                                            this.popup
                                                .update(cx, |popup, cx| popup.dismiss(window, cx));
                                        },
                                    ))
                                    .relative()
                                    .top(surface::css(-7. * (1. - position)))
                                    .opacity(opacity)
                                    .pt(surface::css(7.))
                                    .px(surface::css(14.))
                                    .pb(surface::css(12.))
                                    .border_1()
                                    .border_color(AccountMenuColors::border())
                                    .rounded(surface::css(5.))
                                    .bg(AccountMenuColors::surface())
                                    .when(open, |menu| {
                                        menu.on_mouse_down_out(move |event, window, cx| {
                                            if !trigger_bounds.contains(&event.position) {
                                                popup.update(cx, |popup, cx| {
                                                    popup.dismiss(window, cx)
                                                });
                                            }
                                        })
                                    })
                                    .child(self.content(interactive, cx)),
                            ),
                    )
                    .with_priority(gpui_kit::base::POPUP_PRIORITY),
                )
            })
            .into_any_element()
    }
}
