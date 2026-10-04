use super::*;

const PALETTE: [(&str, &str); 8] = [
    ("delay", "TEXT_ADD_MENU_DELAY"),
    ("keyboard", "TEXT_ADD_MENU_KEYBOARD"),
    ("mouse", "TEXT_ADD_MENU_MOUSE_FUNCTION"),
    ("macro", "TEXT_ADD_MENU_MACRO"),
    ("launch", "TEXT_ADD_MENU_LAUNCH"),
    ("command", "TEXT_ADD_MENU_RUN_COMMAND"),
    ("text", "TEXT_ADD_MENU_TEXT_FUNCTION"),
    ("loop", "TEXT_ADD_MENU_LOOP"),
];

impl MacroPage {
    pub(super) fn editor(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let viewport = window.viewport_size();
        let width = f32::from(viewport.width / window.rem_size()) * 16.;
        let height = f32::from(viewport.height / window.rem_size()) * 16.;
        // main .wrapper:1280px minimum, changed to 900 only below 900px;
        // editor's 270px margin override belongs to the <=1120px media query.
        let minimum = if width <= 900. { 900. } else { 1280. };
        let left = if width <= 1120. {
            290.
        } else {
            width.max(minimum) * 0.5 - 290.
        };
        let list_height = (height - 208.).max(450.);
        let empty = self.macro_count() == 0;
        let palette_disabled =
            empty || matches!(self.tutorial, Tutorial::Initial | Tutorial::Record);
        div()
            .id("macro-source-wrapper")
            .relative()
            .w_full()
            .min_w(css(minimum))
            .h(css(list_height + 74.))
            .child(
                div()
                    .absolute()
                    .left(css(left))
                    .top(css(20.))
                    .w(css(600.))
                    .child(
                        v_flex()
                            .id("macro-editor")
                            .w_full()
                            .when(empty, |v| v.opacity(0.7))
                            .child(self.action_bar())
                            .child(
                                div()
                                    .id("macro-item-list")
                                    .w_full()
                                    .h(css(list_height))
                                    .min_h(css(420.))
                                    .scrollable_y(),
                            ),
                    )
                    .child(
                        v_flex()
                            .id("macro-palette")
                            .absolute()
                            .left(css(-260.))
                            .top_0()
                            .w(css(250.))
                            .py(css(10.))
                            .bg(rgb(0x111111))
                            .rounded(css(5.))
                            .when(palette_disabled, |v| v.opacity(0.7))
                            // AI group is absent until an explicit source capability is supplied.
                            .child(
                                div()
                                    .h(css(30.))
                                    .px(css(10.))
                                    .font_family("RazerF5")
                                    .text_size(css(16.))
                                    .child(tr("TEXT_ADD_MENU").to_uppercase()),
                            )
                            .children(PALETTE.into_iter().map(|(kind, key)| {
                                let id = SharedString::from(format!("macro-palette-{kind}"));
                                // Source-rendered rows are retained while the event editor is ported.
                                // No recorded events or executable actions are synthesized locally.
                                BaseButton::new(id.clone())
                                    .group(id)
                                    .w_full()
                                    .h(css(40.))
                                    .px(css(10.))
                                    .py_0()
                                    .justify_start()
                                    .disabled(true)
                                    .text_size(css(12.))
                                    .line_height(css(14.))
                                    .child(
                                        img(SharedString::from(format!(
                                            "synapse/macro/{kind}.svg"
                                        )))
                                        .size(css(20.))
                                        .mr(css(12.)),
                                    )
                                    .child(tr(key))
                            })),
                    )
                    .when(self.tutorial != Tutorial::Complete, |v| {
                        v.child(self.onboarding(cx))
                    }),
            )
            .into_any_element()
    }

    fn action_bar(&self) -> AnyElement {
        h_flex()
            .id("macro-action-bar")
            .w_full()
            .h(css(54.))
            .pl(css(12.))
            .pr(css(10.))
            .py(css(12.))
            .rounded_t(css(5.))
            .bg(rgb(0x111111))
            .border_b_1()
            .border_color(rgb(0x222222))
            .child(
                h_flex()
                    .flex_1()
                    .mt(css(1.))
                    .child(
                        div()
                            .size(css(16.))
                            .mr(css(10.))
                            .border_1()
                            .border_color(rgb(0x515151)),
                    )
                    .child(img("synapse/macro/delay.svg").size(css(20.)).mr(css(10.)))
                    .when(self.current.is_some(), |v| {
                        v.child(div().text_size(css(14.)).child("0.000s"))
                    }),
            )
            .child(
                h_flex().flex_1().justify_center().child(
                    h_flex()
                        .h(css(27.))
                        .bg(rgb(0x707070))
                        .rounded(css(3.))
                        .border_1()
                        .border_color(rgba(0x0000004d))
                        .child(
                            BaseButton::new("macro-record")
                                .disabled(true)
                                .h_full()
                                .min_w(css(94.))
                                .px(css(10.))
                                .py_0()
                                .text_size(css(12.))
                                .line_height(css(14.))
                                .text_color(rgb(0xffffff))
                                .border_r_1()
                                .border_color(rgba(0x0000004d))
                                .child(img("synapse/macro/record.svg").size(css(12.)).mr(css(4.)))
                                .child(tr("TEXT_ACTION_BAR_RECORD").to_uppercase()),
                        )
                        .child(
                            BaseButton::new("macro-record-options")
                                .disabled(true)
                                .w(css(27.))
                                .h_full()
                                .p_0()
                                .child(img("synapse/macro/record-expand.svg").size(css(10.))),
                        ),
                ),
            )
            .child(
                h_flex()
                    .flex_1()
                    .justify_end()
                    .child(
                        BaseButton::new("macro-undo")
                            .disabled(true)
                            .size(css(20.))
                            .p_0()
                            .mr(css(16.))
                            .child(img("synapse/macro/undo.svg").size_full()),
                    )
                    .child(
                        BaseButton::new("macro-redo")
                            .disabled(true)
                            .size(css(20.))
                            .p_0()
                            .flex_1()
                            .justify_start()
                            .child(img("synapse/macro/redo.svg").size(css(20.))),
                    )
                    .child(
                        BaseButton::new("macro-save")
                            .disabled(true)
                            .opacity(0.3)
                            .min_w(css(100.))
                            .h(css(27.))
                            .mr(css(10.))
                            .px(css(10.))
                            .py_0()
                            .rounded(css(3.))
                            .border_1()
                            .border_color(rgba(0x0000004d))
                            .bg(rgb(0x44d62c))
                            .text_color(rgb(0))
                            .text_size(css(12.))
                            .line_height(css(14.))
                            .child(tr("TEXT_LAUNCH_SAVE")),
                    ),
            )
            .into_any_element()
    }

    fn onboarding(&self, cx: &mut Context<Self>) -> AnyElement {
        let initial = self.tutorial == Tutorial::Initial;
        let adding = self.tutorial == Tutorial::Add;
        let step = if initial {
            0
        } else if adding {
            2
        } else {
            1
        };
        v_flex()
            .id("macro-onboarding")
            .absolute()
            .top(css(63.))
            .left(css(if adding { 0. } else { 150. }))
            .w(css(300.))
            .p(css(20.))
            .bg(rgb(0x111111))
            .border_1()
            .border_color(rgb(0xfd8611))
            .rounded(css(3.))
            .text_center()
            .text_size(css(14.))
            .line_height(css(17.))
            .items_center()
            .shadow(vec![BoxShadow {
                inset: false,
                color: rgba(0x00000033).into(),
                offset: point(px(0.), px(6.)),
                blur_radius: px(10.),
                spread_radius: px(0.),
            }])
            .when(initial, |v| v.h(css(97.)))
            .when(adding, |v| v.h(css(239.)))
            .when(!initial, |v| {
                v.child(
                    BaseButton::new("macro-onboarding-skip")
                        .absolute()
                        .right(css(10.))
                        .top(css(10.))
                        .p_0()
                        .text_size(css(12.))
                        .underline()
                        .hover(|s| s.text_color(rgb(0x44d62c)))
                        .active(|s| s.opacity(0.7))
                        .child(tr("TEXT_MACRO_CONTENT_SKIP").to_uppercase())
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.tutorial = Tutorial::Complete;
                            cx.notify();
                        })),
                )
                .child(
                    img(if adding {
                        "synapse/macro/onboarding-add.svg"
                    } else {
                        "synapse/macro/onboarding-record.svg"
                    })
                    .w(css(260.))
                    .h(css(if adding { 48.5 } else { 68. }))
                    .mb(css(10.)),
                )
                .child(
                    TutorialIndicator::new("macro-onboarding-indicator")
                        .absolute()
                        .left(css(if adding { -45. } else { 94. }))
                        .top(css(if adding { 19.12 } else { -55. })),
                )
            })
            .child(div().w_full().mb(css(10.)).child(tr(if initial {
                "TEXT_MACRO_CONTENT_FIRST"
            } else if adding {
                "TEXT_MACRO_CONTENT_THIRD"
            } else {
                "TEXT_MACRO_CONTENT_SECOND"
            })))
            .when(!initial, |v| {
                v.child(
                    BaseButton::new("macro-onboarding-next")
                        .w(css(90.))
                        .h(css(27.))
                        .p_0()
                        .mb(css(10.))
                        .bg(rgb(0xfd8611))
                        .border_1()
                        .border_color(rgba(0x0000004d))
                        .rounded(css(3.))
                        .text_color(rgb(0x212121))
                        .text_size(css(12.))
                        .hover(|s| s.bg(rgb(0xfcae61)))
                        .active(|s| s.bg(rgba(0xfd8611b3)))
                        .child(
                            tr(if adding {
                                "TEXT_MACRO_CONTENT_DONE"
                            } else {
                                "TEXT_MACRO_CONTENT_NEXT"
                            })
                            .to_uppercase(),
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.tutorial = if this.tutorial == Tutorial::Record {
                                Tutorial::Add
                            } else {
                                Tutorial::Complete
                            };
                            cx.notify();
                        })),
                )
            })
            .child(h_flex().justify_center().children((0..3).map(|i| {
                div()
                    .size(css(6.))
                    .mr(css(6.))
                    .rounded_full()
                    .bg(if i == step {
                        rgb(0xfd8611)
                    } else {
                        rgb(0xcccccc)
                    })
            })))
            .into_any_element()
    }

    pub(super) fn key_binds(&self, _cx: &mut Context<Self>) -> AnyElement {
        // 21700: currentProfile == "" still mounts the warning and inert card;
        // only null returns no page. No validDevices are invented for assignment.
        v_flex()
            .id("macro-key-binds-page")
            .w_full()
            .min_w(css(900.))
            .child(
                h_flex()
                    .justify_center()
                    .mt(css(22.))
                    .text_size(css(14.))
                    .child(
                        img("synapse/macro/warning.svg")
                            .w(css(20.))
                            .h(css(17.))
                            .mr(css(10.)),
                    )
                    .child(tr("TEXT_MARCRO_WARING")),
            )
            .child(
                h_flex()
                    .w_full()
                    .justify_center()
                    .mt(css(20.))
                    .opacity(0.3)
                    .child(
                        v_flex()
                            .w(css(300.))
                            .h(css(230.))
                            .px(css(20.))
                            .py(css(8.))
                            .bg(rgb(0x222222))
                            .rounded(css(5.))
                            .border_2()
                            .border_dashed()
                            .border_color(rgb(0x5d5d5d))
                            .items_center()
                            .child(
                                div().relative().w(css(250.)).h(css(140.)).child(
                                    img("synapse/macro/add.svg")
                                        .absolute()
                                        .top(css(50.))
                                        .left(css(105.))
                                        .size(css(40.)),
                                ),
                            )
                            .child(
                                div()
                                    .w_full()
                                    .text_center()
                                    .text_size(css(14.))
                                    .line_height(css(16.))
                                    .child(tr("TEXT_ASSIGN_MACRO_TO_DEVICES")),
                            ),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn help(&self, _cx: &mut Context<Self>) -> AnyElement {
        // 1519 C: body-widgets > widget-col.left > Support widget > anchor.
        div()
            .w_full()
            .min_w(css(900.))
            .pt(css(20.))
            .flex()
            .justify_center()
            .child(
                v_flex()
                    .w(css(600.))
                    .px(css(40.))
                    .py(css(30.))
                    .bg(rgb(0x111111))
                    .rounded(css(5.))
                    .child(
                        div()
                            .font_family("RazerF5")
                            .text_size(css(16.))
                            .text_color(rgb(0x44d62c))
                            .mb(css(20.))
                            .child(tr("SUPPORT").to_uppercase()),
                    )
                    .child(
                        BaseButton::new("macro-support-link")
                            .p_0()
                            .justify_start()
                            .text_color(rgb(0xcccccc))
                            .text_size(css(14.))
                            .child(tr("VISIT_MACRO_SUPPORT_PAGE"))
                            .on_click(|_, _, cx| {
                                cx.open_url("https://www.razer.com/search/macros?sel=support")
                            }),
                    ),
            )
            .into_any_element()
    }

    pub(super) fn delete_confirmation(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(id) = self.deletion else {
            return div().into_any_element();
        };
        let folder = self
            .entries
            .iter()
            .find(|e| e.id == id)
            .is_some_and(|e| e.kind == EntryKind::Folder);
        let opacity = Presence::new("macro-delete-opacity", true)
            .transition(Transition::new(Duration::from_millis(300)).easing(Easing::Linear))
            .sample(window, cx)
            .progress;
        // Ut -> xt.profile-del: anchored after the profile menu, no modal scrim.
        v_flex()
            .id("macro-delete-confirmation")
            .absolute()
            .left(css(280.))
            .top(css(53.))
            .occlude()
            .w(css(300.))
            .p(css(20.))
            .items_center()
            .bg(rgb(0x111111))
            .rounded(css(3.))
            .border_1()
            .border_color(rgb(0xfd4949))
            .opacity(opacity)
            .text_size(css(14.))
            .line_height(css(17.))
            .child(
                div()
                    .text_color(rgb(0xc83200))
                    .font_weight(FontWeight::BOLD)
                    .mb(css(10.))
                    .child(
                        tr(if folder {
                            "TEXT_DELETE_FOLDER_LABEL"
                        } else {
                            "TEXT_DELETE_MACRO_LABEL"
                        })
                        .to_uppercase(),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .text_center()
                    .mb(css(10.))
                    .child(tr(if folder {
                        "TEXT_DELETE_FOLDER_DESC"
                    } else {
                        "TEXT_DELETE_MACRO_DESC"
                    })),
            )
            .child(
                BaseButton::new("macro-delete-confirm")
                    .min_w(css(90.))
                    .h(css(27.))
                    .px(css(10.))
                    .py(css(4.))
                    .bg(rgb(0xfd4949))
                    .border_1()
                    .border_color(rgba(0x0000004d))
                    .text_color(rgb(0x111111))
                    .text_size(css(12.))
                    .child(tr("TEXT_TOOLTIP_DELETE"))
                    .on_click(cx.listener(move |this, _, _, cx| this.delete_entry(id, cx))),
            )
            .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                this.deletion = None;
                cx.notify();
            }))
            .into_any_element()
    }
}
