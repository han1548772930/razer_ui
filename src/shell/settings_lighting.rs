//! Settings 720 `ia`: Chroma/WDL descriptions, warning and switching state.
//! Unknown control ownership is distinct from either actual selected mode.
use crate::{
    i18n,
    ui::{surface, theme},
};
use gpui_kit::component::{
    button::{Button, ButtonCustomVariant, ButtonVariants},
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use std::rc::Rc;

pub(super) fn content(
    dynamic: Option<bool>,
    switching: bool,
    change: impl Fn(bool, &mut Window, &mut App) + 'static,
    cx: &App,
) -> AnyElement {
    let change = Rc::new(change);
    v_flex()
        .text_size(surface::css(14.))
        .line_height(surface::css(17.))
        .child(
            h_flex()
                .gap(surface::css(14.))
                .child(
                    h_flex()
                        .h(surface::css(36.))
                        .p(surface::css(5.))
                        .border_1()
                        .border_color(theme::stepper_border())
                        .rounded(surface::css(18.))
                        .bg(cx.theme().group_box)
                        .children(
                            [
                                (false, "settings-chroma", "CHROMA_RGB"),
                                (true, "settings-wdl", "DYNAMIC_LIGHTING"),
                            ]
                            .map(|(wdl, id, label)| {
                                let selected = dynamic == Some(wdl);
                                let change = change.clone();
                                Button::new(id)
                                    .xsmall()
                                    .label(i18n::t(label))
                                    .h(surface::css(26.))
                                    .px(surface::css(10.))
                                    .py_0()
                                    .rounded(cx.theme().font_size * (13. / 16.))
                                    .border_0()
                                    .selected(selected)
                                    .disabled(dynamic.is_none() || switching)
                                    .custom(
                                        ButtonCustomVariant::new(cx)
                                            .color(if selected {
                                                cx.theme().primary
                                            } else {
                                                cx.theme().transparent
                                            })
                                            .foreground(if selected {
                                                cx.theme().primary_foreground
                                            } else {
                                                cx.theme().foreground
                                            }),
                                    )
                                    .on_click(move |_, window, cx| change(wdl, window, cx))
                            }),
                        ),
                )
                .when(switching, |row| row.child(spinner::Spinner::new().small())),
        )
        .child(
            v_flex()
                .pt(surface::css(20.))
                .when_some(dynamic, |panel, wdl| {
                    panel
                        .child(i18n::t(if wdl {
                            "DYNAMIC_LIGHTING_TITLE"
                        } else {
                            "CHROMA_RGB_TITLE"
                        }))
                        .child(
                            h_flex()
                                .items_start()
                                .mt(surface::css(10.))
                                .child(
                                    img("synapse/settings-warning.svg")
                                        .size(surface::css(16.))
                                        .mt(surface::css(3.))
                                        .mr(surface::css(10.))
                                        .flex_shrink_0(),
                                )
                                .child(div().flex_1().child(i18n::t(if wdl {
                                    "DYNAMIC_LIGHTING_MSG"
                                } else {
                                    "CHROMA_RGB_MSG"
                                }))),
                        )
                })
                .when(dynamic.is_none(), |panel| {
                    panel
                        .child(i18n::t("DEVICE_LIGHTING_TIPS"))
                        .child(surface::note(
                            "尚未读取灯光控制权，连接服务后才能切换。",
                            cx,
                        ))
                })
                .child(
                    h_flex()
                        .mt(surface::css(20.))
                        .child(
                            img("synapse/settings-wdl.svg")
                                .size(surface::css(44.))
                                .mr(surface::css(10.))
                                .flex_shrink_0(),
                        )
                        .child(
                            gpui_kit::base::Link::new("settings-open-wdl")
                                .href("ms-settings:personalization-lighting")
                                .open_with(|url, _, _, cx| cx.open_url(url))
                                .ml(surface::css(10.))
                                .text_color(cx.theme().foreground)
                                .cursor_pointer()
                                .text_decoration_1()
                                .hover(|link| link.text_color(cx.theme().primary))
                                .focus_visible(|link| link.text_color(cx.theme().primary))
                                .child(i18n::t("OPEN_WINDOWS_DYNAMIC_LIGHTING")),
                        ),
                ),
        )
        .into_any_element()
}

struct LightingPreview {
    dynamic: bool,
    switching: bool,
}
pub(super) fn open_preview(window: &mut Window, cx: &mut App) {
    let view = cx.new(|_| LightingPreview {
        dynamic: false,
        switching: false,
    });
    window.open_dialog(cx, move |dialog, window, _| {
        dialog
            .title("设备灯光 · 界面预览")
            .w(window.rem_size() * (680. / 16.))
            .child(view.clone())
    });
}
impl Render for LightingPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let owner = cx.entity().downgrade();
        v_flex()
            .gap_4()
            .child(surface::note(
                "以下为原版界面状态示例，不读取或更改设备灯光控制权。",
                cx,
            ))
            .child(
                checkbox::Checkbox::new("preview-lighting-switching")
                    .label("预览切换中的状态")
                    .checked(self.switching)
                    .on_click(cx.listener(|this, checked, _, cx| {
                        this.switching = *checked;
                        cx.notify();
                    })),
            )
            .child(
                surface::panel(i18n::t("DEVICE_LIGHTING"), cx).child(content(
                    Some(self.dynamic),
                    self.switching,
                    move |wdl, _, cx| {
                        let _ = owner.update(cx, |this, cx| {
                            this.dynamic = wdl;
                            cx.notify();
                        });
                    },
                    cx,
                )),
            )
    }
}
