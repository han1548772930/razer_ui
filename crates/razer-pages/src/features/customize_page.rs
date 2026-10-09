use super::workspace::{Continue, DeviceWorkspace};
use gpui_kit::base::TestSupportExt as _;
use gpui_kit::component::{
    button::{Button, ButtonCustomVariant, ButtonVariants},
    checkbox::Checkbox,
    *,
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use razer_widgets::surface;
use razer_widgets::surface::SynapseSwitch as Switch;

// The 182 KP order and anchor coordinates come from drawLines, not DKM array order.
// 标签是源码自己的语言键（与 `customize_drawer.rs` 的 182 module 1368 表一致）：
// `LEFT_CLICK`/`RIGHT_CLICK`/`SCROLL_CLICK`/`SCROLL_UP`/`SCROLL_DOWN`/
// `MOUSE_BUTTON_4`/`MOUSE_BUTTON_5`/`CYCLE_UP_SENSITIVITY`。
const MOUSE_INPUTS: [(&str, &str); 8] = [
    ("LeftButton", "LEFT_CLICK"),
    ("RightButton", "RIGHT_CLICK"),
    ("MiddleButton", "SCROLL_CLICK"),
    ("ScrollUp", "SCROLL_UP"),
    ("ScrollDown", "SCROLL_DOWN"),
    ("Button5", "MOUSE_BUTTON_5"),
    ("Button4", "MOUSE_BUTTON_4"),
    ("CycleUpSensitivityStages", "CYCLE_UP_SENSITIVITY"),
];
impl DeviceWorkspace {
    pub(super) fn customize_page(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        v_flex()
            .child(if self.pid() == 653 {
                self.keyboard_image(cx)
            } else {
                self.mouse_image(cx)
            })
            .child(
                h_flex()
                    .justify_center()
                    .items_center()
                    .gap(surface::css(10.))
                    .mt(surface::css(20.))
                    .mb(surface::css(10.))
                    .child(self.drawer_toggle(cx))
                    .child(self.layer_switch(window, cx))
                    .child(surface::help_control(
                        "mapping-hypershift-help",
                        razer_i18n::t("HYPERSHIFT_TOOLTIP"),
                    )),
            )
            .when(self.pid() == 653, |this| {
                this.child(self.keyboard_panels(cx))
            })
            .into_any_element()
    }
    /// `.hyper-wrapper`：`#111` 底、`1px solid #5d5d5d`、圆角 18px、高 36px、内边距 5px，
    /// 悬停边框 `#44d62c`、按下底色 `#292929`，边框与底色都有 200ms 过渡；
    /// 标记与源码一致：`role="switch"` + `aria-checked` + 两个 `.text` 标签。
    fn layer_switch(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let pointer = surface::pointer_state("mapping-layer-switch", window, cx);
        let (hovered, pressed) = pointer.read(cx).sample();
        let border = surface::fade_color(
            "mapping-layer-switch-border",
            if hovered {
                rgb(0x44d62c).into()
            } else {
                rgb(0x5d5d5d).into()
            },
            200,
            window,
            cx,
        );
        let background = surface::fade_color(
            "mapping-layer-switch-background",
            if pressed {
                rgb(0x292929).into()
            } else {
                rgb(0x111111).into()
            },
            200,
            window,
            cx,
        );
        let mut wrapper = div()
            .flex()
            .items_center()
            .h(surface::css(36.))
            .p(surface::css(5.))
            .rounded(surface::css(18.))
            .border_1()
            .border_color(border)
            .bg(background)
            .id("mapping-layer-switch")
            // 源码的 `role="switch"` + `aria-checked`：本地用 kit 的 `Role` 与
            // `aria_label`（GPUI 没有 `aria-checked`，因此把状态并入标签）。
            .role(Role::Switch)
            .aria_label(format!(
                "{} {}",
                razer_i18n::t("HYPERSHIFT"),
                if self.hypershift { "on" } else { "off" }
            ));
        wrapper = surface::track_pointer(wrapper, &pointer, window);
        wrapper
            .on_click(cx.listener(|this, _, window, cx| {
                this.continue_with(Continue::Layer(!this.hypershift), window, cx)
            }))
            .child(self.layer_label(false, window, cx))
            .child(self.layer_label(true, window, cx))
            .into_any_element()
    }

    /// `.text`：14px、高 24px、`padding:6px 10px 5px`、圆角 12px、首字母大写，
    /// 颜色有 200ms 过渡（`.text{transition:color .2s}`）；选中的一侧拿到
    /// `#44d62c`（标准层）或 `#fd8611`（Hypershift 层）底色与 `#212121` 文字。
    fn layer_label(&self, hyper: bool, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let selected = self.hypershift == hyper;
        let id = if hyper {
            "mapping-hypershift"
        } else {
            "mapping-standard"
        };
        let accent = if hyper { rgb(0xfd8611) } else { rgb(0x44d62c) };
        let color = surface::fade_color(
            (ElementId::from(id), "label-color"),
            if selected {
                rgb(0x212121).into()
            } else {
                rgb(0xcccccc).into()
            },
            200,
            window,
            cx,
        );
        div()
            .id(id)
            .relative()
            .h(surface::css(24.))
            .pt(surface::css(6.))
            .pb(surface::css(5.))
            .px(surface::css(10.))
            .rounded(surface::css(12.))
            .text_size(surface::css(14.))
            .line_height(surface::css(14.))
            .text_color(color)
            .when(selected, |label| label.bg(accent))
            .when(!hyper, |label| label.mr(surface::css(5.)))
            .child(razer_i18n::t(if hyper { "HYPERSHIFT" } else { "STANDARD" }))
            .into_any_element()
    }

    fn layer_button(&self, hyper: bool, cx: &mut Context<Self>) -> Button {
        let selected = self.hypershift == hyper;
        let color = if hyper {
            razer_widgets::theme::DrawerColors::new().hypershift()
        } else {
            cx.theme().primary
        };
        Button::new(if hyper {
            "mapping-hypershift"
        } else {
            "mapping-standard"
        })
        .label(razer_i18n::t(if hyper { "HYPERSHIFT" } else { "STANDARD" }))
        .selected(selected)
        .h(surface::css(24.))
        .px(surface::css(10.))
        .py_0()
        .text_size(surface::css(14.))
        .border_0()
        .rounded(cx.theme().font_size * (12. / 16.))
        .custom(
            ButtonCustomVariant::new(cx)
                .color(if selected {
                    color
                } else {
                    cx.theme().transparent
                })
                .foreground(if selected {
                    cx.theme().primary_foreground
                } else {
                    cx.theme().foreground
                })
                .hover(if selected {
                    color
                } else {
                    cx.theme().secondary_hover
                })
                .active(color),
        )
        .on_click(
            cx.listener(move |this, _, w, cx| this.continue_with(Continue::Layer(hyper), w, cx)),
        )
    }
    fn input_button(
        &self,
        id: &'static str,
        label: impl Into<String>,
        cx: &mut Context<Self>,
    ) -> Button {
        let label = label.into();
        let selected = self.mapping.as_ref().is_some_and(|m| m.input == id);
        let bindings = if self.hypershift {
            &self.settings().hypershift_bindings
        } else {
            &self.settings().bindings
        };
        let assignment = bindings.get(id).filter(|value| value.as_str() != "default");
        let text = assignment
            .map(|value| self.mapping_summary(value).1)
            .unwrap_or_else(|| label.clone());
        let enabled = self.mapping_input_enabled(id);
        let foreground = if !enabled {
            cx.theme().muted_foreground
        } else if assignment.is_some() {
            if self.hypershift {
                razer_widgets::theme::DrawerColors::new().hypershift()
            } else {
                cx.theme().primary
            }
        } else {
            cx.theme().foreground
        };
        Button::new(SharedString::from(format!("mouse-input-{id}")))
            .label(text)
            .accessibility_label(label)
            .disabled(!enabled)
            .custom(
                ButtonCustomVariant::new(cx)
                    .color(if selected {
                        cx.theme().group_box
                    } else {
                        cx.theme().transparent
                    })
                    .foreground(foreground)
                    .hover(cx.theme().list_hover)
                    .active(cx.theme().group_box),
            )
            .border_0()
            .map(|button| Styled::rounded(button, surface::css(3.)))
            .text_size(surface::css(14.))
            .max_w(surface::css(220.))
            .px(surface::css(10.))
            .py_0()
            .h(surface::css(30.))
            .selected(selected)
            .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                let active = this.hovered_input.as_deref() == Some(id);
                if *hovered && enabled && !active {
                    this.hovered_input = Some(id.to_string());
                    cx.notify();
                } else if !*hovered && active {
                    this.hovered_input = None;
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |this, _, w, cx| {
                this.continue_with(Continue::Input(id.into()), w, cx)
            }))
    }
    fn mouse_image(&self, cx: &mut Context<Self>) -> AnyElement {
        let device = self.device();
        let front_image = razer_assets::device_image(
            razer_model::demo::artwork_product_id(self.pid()),
            device.edition_id,
            device.layout_id,
            razer_assets::DeviceImage::Product,
        );
        let bottom_image = razer_assets::device_image(
            razer_model::demo::artwork_product_id(self.pid()),
            device.edition_id,
            device.layout_id,
            razer_assets::DeviceImage::MouseBottom,
        );
        let bottom = self
            .mapping
            .as_ref()
            .is_some_and(|m| m.input == "CycleUpSensitivityStages")
            || self.hovered_input.as_deref() == Some("CycleUpSensitivityStages");
        // Geometry corresponds to the source's 770 x 340 logical canvas.
        surface::config_wrapper()
            .h(surface::css(340.))
            .child(surface::dot_background(cx))
            .child(
                div()
                    .relative()
                    .mx_auto()
                    .w(rems(48.125))
                    .h(rems(21.25))
                    .when_some(front_image, |this, path| {
                        this.child(
                            img(path)
                                .id("mouse-front-image")
                                .absolute()
                                .left(rems(235. / 16.))
                                .h_full()
                                .w(rems(18.75))
                                .object_fit(ObjectFit::Contain)
                                .test_support(),
                        )
                    })
                    .when_some(bottom.then_some(bottom_image).flatten(), |this, path| {
                        this.child(
                            // KP.getCycleImage is an extra 300x340 layer; it never replaces zp.
                            img(path)
                                .id("mouse-cycle-overlay")
                                .test_support()
                                .absolute()
                                .left(surface::css(272.))
                                .top(surface::css(-77.))
                                .w(surface::css(300.))
                                .h(surface::css(340.)),
                        )
                    })
                    .child(self.mouse_connections(bottom, cx))
                    .children([0, 2, 6, 5, 7].into_iter().enumerate().map(|(row, i)| {
                        div()
                            .absolute()
                            .left_0()
                            .top(rems(row as f32 * 50. / 16.))
                            // KP passes side="right" only for the last left label.
                            .w(surface::css(if i == 7 { 220. } else { 210. }))
                            .flex()
                            .justify_end()
                            .child(self.input_button(
                                MOUSE_INPUTS[i].0,
                                razer_i18n::t(MOUSE_INPUTS[i].1),
                                cx,
                            ))
                    }))
                    .children([1, 3, 4].into_iter().enumerate().map(|(row, i)| {
                        div()
                            .absolute()
                            .right_0()
                            .top(rems(row as f32 * 50. / 16.))
                            .w(surface::css(210.))
                            .flex()
                            .justify_start()
                            .child(self.input_button(
                                MOUSE_INPUTS[i].0,
                                razer_i18n::t(MOUSE_INPUTS[i].1),
                                cx,
                            ))
                    })),
            )
            .into_any_element()
    }
    fn mouse_connections(&self, bottom: bool, cx: &App) -> AnyElement {
        let selected = self.mapping.as_ref().map(|m| m.input.clone());
        let hovered = self.hovered_input.clone();
        let muted = cx.theme().border;
        let active = if self.hypershift {
            razer_widgets::theme::DrawerColors::new().hypershift()
        } else {
            cx.theme().primary
        };
        canvas(
            |_, _, _| (),
            move |bounds, _, window, _| {
                let sx = bounds.size.width / 310.;
                let sy = bounds.size.height / 340.;
                let anchors = [
                    (0, true, 0, 114., 53.),
                    (2, true, 1, 148., 84.),
                    (6, true, 2, 86., 140.),
                    (5, true, 3, 90., 178.),
                    (
                        7,
                        true,
                        4,
                        if bottom { 185. } else { 90. },
                        if bottom { 155. } else { 195. },
                    ),
                    (1, false, 0, 181., 53.),
                    (3, false, 1, 148., 67.),
                    (4, false, 2, 148., 102.),
                ];
                for (index, left, row, x, y) in anchors {
                    let id = MOUSE_INPUTS[index].0;
                    let focused = selected.as_deref() == Some(id) || hovered.as_deref() == Some(id);
                    let color = if focused { active } else { muted };
                    let start = if left { 3. } else { 307. };
                    let mid = if left { 56. } else { 254. };
                    let line_y = 16. + 50. * row as f32;
                    let pt = |x: f32, y: f32| point(bounds.left() + sx * x, bounds.top() + sy * y);
                    let mut path = PathBuilder::stroke(px(1.));
                    path.move_to(pt(start, line_y));
                    path.line_to(pt(mid, line_y));
                    path.line_to(pt(x + 7.5, y + 2.5));
                    if let Ok(path) = path.build() {
                        window.paint_path(path, color);
                    }
                    let dot = pt(x + 7.5, y + 2.5);
                    let radius = sx * 2.5;
                    for center in [pt(start, line_y), dot] {
                        // Original endpoints are filled circles, not square quads.
                        window.paint_quad(quad(
                            Bounds::new(
                                center - point(radius, radius),
                                size(radius * 2., radius * 2.),
                            ),
                            radius,
                            color,
                            px(0.),
                            color,
                            BorderStyle::Solid,
                        ));
                    }
                    if focused {
                        let radius = sx * 5.5;
                        window.paint_quad(quad(
                            Bounds::new(
                                dot - point(radius, radius),
                                size(radius * 2., radius * 2.),
                            ),
                            radius,
                            Hsla::transparent_black(),
                            px(1.),
                            color,
                            BorderStyle::Solid,
                        ));
                    }
                }
            },
        )
        .absolute()
        .left(rems(230. / 16.))
        .top_0()
        .w(rems(310. / 16.))
        .h_full()
        .into_any_element()
    }
    fn keyboard_image(&self, cx: &mut Context<Self>) -> AnyElement {
        let device = self.device();
        let resolved = razer_assets::resolve_device_image(
            razer_model::demo::artwork_product_id(self.pid()),
            device.edition_id,
            device.layout_id,
            razer_assets::DeviceImage::Product,
        );
        let image_path = resolved.map(|image| image.asset);
        let keys = resolved
            .filter(|image| !image.is_fallback_preview)
            .map(|image| razer_assets::keyboard_keys_for_layout(image.layout_id))
            .unwrap_or_default();
        let has_geometry = !keys.is_empty();
        // Source IM viewBox=730 x 340; raster is 920 x 340, displayed at width 830.
        // Both are centered in a 830 x (340*830/730) region. Chroma SVG has a different geometry.
        let source_width = if resolved.is_some_and(|image| image.edition_id == 130) {
            830.8
        } else {
            830.
        };
        let width = source_width / 16.;
        let height = 340.0 * source_width / 730.0 / 16.;
        surface::config_wrapper()
            .child(surface::dot_background(cx))
            .child(
                div()
                    .relative()
                    .w(rems(width))
                    .h(rems(height))
                    .mx_auto()
                    .flex_shrink_0()
                    .when_some(image_path, |this, path| {
                        this.child(
                            img(path)
                                .absolute()
                                .left_0()
                                .top(rems((height - 340.0 * source_width / 920.0 / 16.) / 2.))
                                .w(rems(width))
                                .h(rems(340.0 * source_width / 920.0 / 16.)),
                        )
                    })
                    .children(keys.iter().map(|key| {
                        let id = key.id.clone();
                        let bounds = key.bounds;
                        let hover_id = id.clone();
                        let selected = self.mapping.as_ref().is_some_and(|m| m.input == id);
                        let hovered = self.hovered_input.as_deref() == Some(id.as_str());
                        let enabled = self.mapping_input_enabled(&id);
                        let (bindings, color, opacity) = if self.hypershift {
                            (
                                &self.settings().hypershift_bindings,
                                razer_widgets::theme::DrawerColors::new().hypershift(),
                                0.7,
                            )
                        } else {
                            (&self.settings().bindings, cx.theme().primary, 0.4)
                        };
                        let assignment = bindings.get(&id);
                        let fill =
                            if assignment.is_some_and(|value| self.mapping_is_disabled(value)) {
                                razer_widgets::theme::DrawerColors::new().disabled_mapping()
                            } else if assignment.is_some_and(|value| value != "default") {
                                color.opacity(opacity)
                            } else {
                                cx.theme().transparent
                            };
                        div()
                            .absolute()
                            .left(rems(bounds[0] / 730. * width))
                            .top(rems(bounds[1] / 340. * height))
                            .w(rems(bounds[2] / 730. * width))
                            .h(rems(bounds[3] / 340. * height))
                            .child(
                                crate::keyboard_geometry::KeyRegion::new(
                                    key,
                                    fill,
                                    if enabled {
                                        color
                                    } else {
                                        cx.theme().muted_foreground
                                    },
                                    selected,
                                    hovered,
                                    cx.listener(move |this, _, w, cx| {
                                        this.continue_with(Continue::Input(id.clone()), w, cx)
                                    }),
                                    cx.listener(move |this, hovered: &bool, _, cx| {
                                        let was_hovered = this.hovered_input.as_deref()
                                            == Some(hover_id.as_str());
                                        if *hovered && !was_hovered {
                                            this.hovered_input = Some(hover_id.clone());
                                            cx.notify();
                                        } else if !*hovered && was_hovered {
                                            this.hovered_input = None;
                                            cx.notify();
                                        }
                                    }),
                                )
                                .disabled(!enabled),
                            )
                    })),
            )
            .when(!has_geometry, |this| {
                this.child(
                    surface::note(
                        if resolved.is_some_and(|image| image.is_fallback_preview) {
                            "暂时无法识别键盘布局，当前显示标准布局预览。"
                        } else if image_path.is_some() {
                            "此键盘布局暂不支持按键编辑。"
                        } else {
                            "当前设备版本或布局的图片尚不可用。"
                        },
                        cx,
                    )
                    .text_center(),
                )
            })
            .into_any_element()
    }
    pub(super) fn mapping_panel(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        self.render_mapping_editor(window, cx)
    }
    fn keyboard_panels(&self, cx: &mut Context<Self>) -> AnyElement {
        surface::page_columns()
            .child(surface::page_column(
                v_flex()
                    .flex_1()
                    .gap_5()
                    .child(self.keyboard_gaming_panel(cx))
                    .child(self.snap_tap_panel(cx))
                    .child(self.polling_panel(cx))
                    .child(surface::panel("键盘属性", cx).child(
                        super::device_pages::system_button(
                            "keyboard-properties",
                            "打开 Windows 键盘属性",
                            razer_platform::system::Properties::Keyboard,
                        ),
                    )),
            ))
            .child(surface::page_column(self.command_dial_panel(cx)))
            .into_any_element()
    }

    fn keyboard_gaming_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let keyboard = &self.settings().keyboard;
        let has_key = |id| {
            super::customize_drawer::keyboard_mapping_input(self.device().layout_id, id).is_some()
        };
        // QL derives these fixed exclusions from the active layout's buttonList.
        // Its changeGameMode sets isWindowsKeyDisabled to the enabled state;
        // Menu and Copilot share that value instead of having independent toggles.
        let has_menu = has_key("KEY_APPLICATION");
        let has_copilot = has_key("DKM_D2") || has_key("DKM_F6");
        surface::panel_with_control(
            razer_i18n::t("GAMING_MODE_HEADER"),
            h_flex()
                .items_center()
                .gap(surface::css(10.))
                .child(
                    Switch::new("gaming-enabled")
                        .accessibility_label(razer_i18n::t("GAMING_MODE_HEADER"))
                        .checked(keyboard.gaming)
                        .on_change(cx.listener(|this, value, window, cx| {
                            this.edit(window, cx, |settings| settings.keyboard.gaming = *value)
                        })),
                )
                .child(
                    surface::asset_button(
                        "gaming-help",
                        "synapse/help-default.svg",
                        "游戏模式说明",
                        cx,
                    )
                    .tooltip(razer_i18n::t("GAMING_MODE_TOOLTIP")),
                ),
            cx,
        )
        .child(
            Checkbox::new("gaming-in-game")
                .label(razer_i18n::t("APPLY_IN_GAME_ONLY"))
                .checked(keyboard.in_game)
                .disabled(!keyboard.gaming)
                .on_change(cx.listener(|this, value, window, cx| {
                    this.edit(window, cx, |settings| settings.keyboard.in_game = *value)
                })),
        )
        .child(
            div()
                .when(!keyboard.gaming, |description| description.opacity(0.3))
                .child(razer_i18n::t("GAMING_MODE_DESC")),
        )
        .child(
            Checkbox::new("gaming-windows")
                .label(razer_i18n::t("DISABLE_WINDOWS_KEY"))
                .checked(keyboard.gaming)
                .disabled(true),
        )
        .when(has_menu, |panel| {
            panel.child(
                Checkbox::new("gaming-menu")
                    .label(razer_i18n::t("DISABLE_MENU_KEY"))
                    .checked(keyboard.gaming)
                    .disabled(true),
            )
        })
        .when(has_copilot, |panel| {
            panel.child(
                Checkbox::new("gaming-copilot")
                    .label(razer_i18n::t("DISABLE_COPILOT_KEY"))
                    .checked(keyboard.gaming)
                    .disabled(true),
            )
        })
        .child(
            Checkbox::new("gaming-alt-tab")
                .label(razer_i18n::t("DISABLE_ALT_TAB"))
                .checked(keyboard.disable_alt_tab)
                .disabled(!keyboard.gaming)
                .on_change(cx.listener(|this, value, window, cx| {
                    this.edit(window, cx, |settings| {
                        settings.keyboard.set_disable_alt_tab(*value)
                    })
                })),
        )
        .child(
            Checkbox::new("gaming-alt-f4")
                .label(razer_i18n::t("DISABLE_ALT_F4"))
                .checked(keyboard.disable_alt_f4)
                .disabled(!keyboard.gaming)
                .on_change(cx.listener(|this, value, window, cx| {
                    this.edit(window, cx, |settings| {
                        settings.keyboard.disable_alt_f4 = *value
                    })
                })),
        )
        .into_any_element()
    }
}
