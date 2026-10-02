use super::*;
use controls::{close_button, spinner};
use installer::progress_bar;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ModalKind {
    Logout,
    PatchNotes { loading: bool },
}
pub(super) struct Modal {
    kind: ModalKind,
    shown: bool,
    closing: bool,
    install: InstallState,
    empty_notes: bool,
}

impl AlexaPage {
    pub(super) fn open_patch_scene(
        &mut self,
        scene: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.open_modal(
            ModalKind::PatchNotes {
                loading: scene == "notes-loading",
            },
            window,
            cx,
        );
        if let Some(modal) = &mut self.modal {
            modal.empty_notes = scene == "notes-empty";
            modal.install = match scene {
                "notes-waiting" => InstallState::Waiting,
                "notes-downloading" => InstallState::Downloading,
                "notes-saving" => InstallState::Saving,
                "notes-installing" => InstallState::Installing,
                _ => InstallState::Available,
            };
        }
    }
    pub(super) fn open_modal(
        &mut self,
        kind: ModalKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.modal.is_some() || self.account != Account::Ready {
            return;
        }
        self.return_focus = window.focused(cx);
        self.modal_focus.focus(window, cx);
        self.modal = Some(Modal {
            kind,
            shown: cx.reduce_motion(),
            closing: false,
            install: InstallState::Available,
            empty_notes: false,
        });
        self.lifecycle_task = (!cx.reduce_motion()).then(|| {
            cx.spawn_in(window, async move |this, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                _ = this.update_in(cx, |this, _, cx| {
                    if let Some(modal) = &mut this.modal {
                        modal.shown = true;
                    }
                    this.lifecycle_task = None;
                    cx.notify();
                });
            })
        });
        cx.notify();
    }
    pub(super) fn close_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(modal) = &mut self.modal else {
            return;
        };
        if modal.closing {
            return;
        }
        modal.closing = true;
        modal.shown = false;
        self.lifecycle_task = None;
        if cx.reduce_motion() {
            self.finish_modal(window, cx);
        } else {
            self.lifecycle_task = Some(cx.spawn_in(window, async move |this, cx| {
                cx.background_executor()
                    .timer(Duration::from_millis(300))
                    .await;
                _ = this.update_in(cx, |this, window, cx| this.finish_modal(window, cx));
            }));
            cx.notify();
        }
    }
    fn finish_modal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.lifecycle_task = None;
        self.modal = None;
        if let Some(focus) = self.return_focus.take() {
            focus.focus(window, cx);
        }
        cx.notify();
    }
    fn confirm_logout(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !matches!(
            self.modal.as_ref().map(|modal| (modal.kind, modal.closing)),
            Some((ModalKind::Logout, false))
        ) {
            return;
        }
        // Source resets Home and history, but this only clears the sample Amazon session.
        self.account = Account::Razer;
        self.page = Page::Home;
        self.history = vec![Page::Home];
        self.history_index = 0;
        self.scene = "razer".into();
        self.expanded = None;
        self.recording = false;
        self.scenes.update(cx, |state, cx| {
            state.set_selected_value(&"razer".to_owned(), window, cx)
        });
        self.return_focus = Some(self.focus.clone());
        self.close_modal(window, cx);
    }
    pub(super) fn modal_view(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some(modal) = &self.modal else {
            return div().into_any_element();
        };
        let opacity = Presence::new("alexa-modal-opacity", modal.shown)
            .transition(Transition::new(Duration::from_millis(150)).easing(Easing::Linear))
            .sample(window, cx)
            .progress;
        let patch = matches!(modal.kind, ModalKind::PatchNotes { .. });
        let viewport = window.viewport_size();
        let panel = if patch {
            self.patch_notes(window, cx)
        } else {
            self.logout(window, cx)
        };
        let popup = if patch {
            let reveal = Presence::new("alexa-patch-slide", modal.shown)
                .transition(Transition::new(Duration::from_millis(300)).easing(Easing::EaseOut))
                .sample(window, cx)
                .progress;
            // CSS translateY(100%) is relative to the measured panel height.
            // First mount starts below the viewport until prepaint reports it.
            let height = if self.patch_height > px(0.) {
                self.patch_height
            } else {
                viewport.height
            };
            let owner = cx.entity().downgrade();
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_end()
                .justify_center()
                .child(
                    div()
                        .relative()
                        .w_full()
                        .max_w(css(800.))
                        .top(height * (1. - reveal))
                        .on_prepaint(move |bounds, _, cx| {
                            _ = owner.update(cx, |this, cx| {
                                if this.patch_height != bounds.size.height {
                                    this.patch_height = bounds.size.height;
                                    cx.notify();
                                }
                            });
                        })
                        .child(panel),
                )
                .into_any_element()
        } else {
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .opacity(opacity)
                .child(
                    div()
                        .w_full()
                        .max_w(css(400.))
                        .max_h(viewport.height)
                        .child(panel),
                )
                .into_any_element()
        };
        gpui_kit::base::Dialog::new(cx)
            // At Popup priority, later popup children paint above their owner;
            // ordinary page popups remain beneath this modal and its backdrop.
            .layer(90, true)
            .focus_handle(self.modal_focus.clone())
            .close_on_backdrop_press(false)
            .on_ok(|_, _, _| false)
            .on_close(cx.listener(|this, _, window, cx| this.close_modal(window, cx)))
            .backdrop(
                div()
                    .absolute()
                    .inset_0()
                    .occlude()
                    .bg(cx.theme().title_bar.opacity(0.7 * opacity))
                    .on_scroll_wheel(|_, _, cx| cx.stop_propagation()),
            )
            .popup(popup)
            .into_any_element()
    }
    fn logout(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .id("alexa-logout-dialog")
            .test_support()
            .relative()
            .occlude()
            .p(css(20.))
            .px(css(30.))
            .bg(cx.theme().group_box)
            .rounded(css(5.))
            .border_1()
            .border_color(cx.theme().primary)
            .text_size(css(14.))
            .line_height(relative(1.22))
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .mb(css(20.))
                    .font_family("RazerF5")
                    .text_size(css(16.))
                    .text_color(cx.theme().primary)
                    .child(text("ALEXA_LOGOUT").to_uppercase()),
            )
            .child(div().child(text("ALEXA_LOGOUT_CONFIRM")))
            .child(div().mt(css(17.)).child(text("ALEXA_LOGOUT_DESC")))
            .child(
                h_flex()
                    .justify_end()
                    .gap(css(10.))
                    .mt(css(20.))
                    .child(
                        source_button(
                            "alexa-logout-cancel",
                            text("TEXT_CANCEL"),
                            SourceButtonKind::Gray,
                            false,
                            window,
                            cx,
                        )
                        .on_click(cx.listener(|this, _, window, cx| this.close_modal(window, cx))),
                    )
                    .child(
                        source_button(
                            "alexa-logout-confirm",
                            text("TEXT_LOG_OUT"),
                            SourceButtonKind::Green,
                            false,
                            window,
                            cx,
                        )
                        .on_click(
                            cx.listener(|this, _, window, cx| this.confirm_logout(window, cx)),
                        ),
                    ),
            )
            .child(
                close_button(false, window, cx)
                    .on_click(cx.listener(|this, _, window, cx| this.close_modal(window, cx))),
            )
            .into_any_element()
    }
    fn patch_notes(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let modal = self.modal.as_ref().expect("mounted patch notes");
        let loading = matches!(modal.kind, ModalKind::PatchNotes { loading: true });
        let state = modal.install;
        let max_body = (window.viewport_size().height - window.rem_size() * (90. / 16.))
            .max(px(0.))
            .min(window.rem_size() * (534. / 16.));
        v_flex()
            .id("alexa-patch-dialog")
            .test_support()
            .relative()
            .occlude()
            .w_full()
            .max_w(css(800.))
            .bg(cx.theme().sidebar)
            .rounded_t(css(5.))
            .text_color(cx.theme().muted_foreground)
            .text_size(css(14.))
            .line_height(relative(1.22))
            .child(
                div()
                    .px(css(10.))
                    .pt(css(9.))
                    .pb(css(8.))
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .font_family("RazerF5")
                    .text_size(css(16.))
                    .child(text("TEXT_PATCH_NOTES").to_uppercase()),
            )
            .child(
                v_flex()
                    .id("alexa-patch-body")
                    .w_full()
                    .max_h(max_body)
                    .scrollable_y()
                    .child(
                        v_flex()
                            .py(css(17.))
                            .px(css(30.))
                            .child(
                                h_flex()
                                    .flex_wrap()
                                    .gap(css(10.))
                                    .mb(css(18.))
                                    .child(
                                        div()
                                            .text_size(css(24.))
                                            .text_color(cx.theme().primary)
                                            .child("示例发布日期"),
                                    )
                                    .child(text("TEXT_VERSION_N").replace("{{number}}", "SAMPLE"))
                                    .child(
                                        text("TEXT_SIZE_B").replace("{{bytes}}", "10 MB（示例）"),
                                    ),
                            )
                            .child(div().text_size(css(16.)).mb(css(10.)).child("Alexa"))
                            .child(
                                div()
                                    .mb(css(10.))
                                    .child("以下为本地排版示例，未读取真实版本或更新说明。"),
                            )
                            .when(loading, |view| {
                                view.child(
                                    div()
                                        .mx_auto()
                                        .mt(css(67.))
                                        .mb(css(70.))
                                        .child(spinner(74.)),
                                )
                            })
                            .when(!loading && !modal.empty_notes, |view| {
                                view.children(
                                    [
                                        ("TEXT_NEW", cx.theme().primary, "新增项目示例"),
                                        (
                                            "TEXT_IMPROVEMENTS",
                                            AlexaColors::improvement(),
                                            "改进项目示例",
                                        ),
                                        ("TEXT_FIXED", AlexaColors::fixed(), "修复项目示例"),
                                    ]
                                    .into_iter()
                                    .map(
                                        |(key, color, label)| {
                                            v_flex()
                                                .child(
                                                    div().flex().child(
                                                        div()
                                                            .min_w(css(90.))
                                                            .px(css(16.))
                                                            .py(css(5.))
                                                            .rounded(css(3.))
                                                            .bg(color)
                                                            .text_size(css(12.))
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(cx.theme().sidebar)
                                                            .child(text(key)),
                                                    ),
                                                )
                                                .child(
                                                    h_flex()
                                                        .items_start()
                                                        .gap(css(10.))
                                                        .my(css(10.))
                                                        .pl(css(16.))
                                                        .child(
                                                            div()
                                                                .size(css(6.))
                                                                .mt(css(5.))
                                                                .rounded_full()
                                                                .bg(cx.theme().muted_foreground),
                                                        )
                                                        .child(label),
                                                )
                                        },
                                    ),
                                )
                            }),
                    ),
            )
            .child(
                h_flex()
                    .px(css(30.))
                    .py(css(10.))
                    .border_t_1()
                    .border_color(crate::ui::theme::MainPageColors.module_action_gray())
                    .child(v_flex().flex_1().when(state.busy(), |view| {
                        view.child(div().mb(css(5.)).text_size(css(10.)).child(state.status()))
                            .child(progress_bar(
                                "alexa-patch-progress",
                                state.progress(),
                                5.,
                                window,
                                cx,
                            ))
                    }))
                    .child(
                        source_button(
                            "alexa-patch-update",
                            text(if state.busy() {
                                "TEXT_CANCEL"
                            } else {
                                "TEXT_UPDATE"
                            }),
                            if state.busy() {
                                SourceButtonKind::Gray
                            } else {
                                SourceButtonKind::Green
                            },
                            state.disabled(),
                            window,
                            cx,
                        )
                        .ml(css(30.))
                        .on_click(cx.listener(|this, _, _, cx| {
                            if let Some(modal) = &mut this.modal {
                                modal.install = modal.install.after_action();
                                cx.notify();
                            }
                        })),
                    ),
            )
            .child(
                close_button(true, window, cx)
                    .on_click(cx.listener(|this, _, window, cx| this.close_modal(window, cx))),
            )
            .into_any_element()
    }
}
