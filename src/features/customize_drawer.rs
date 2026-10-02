//! The shared ButtonPanelComponent (182 module 5035 / 653 module 95035).
//! It has an All/Customized filter, not a text search. Input activation goes
//! through the same continuation as the product image's hit regions.
use super::{
    controls::{Choice, Choices},
    workspace::{Continue, DeviceWorkspace},
};
use crate::{
    model::Device,
    ui::{surface, theme::DrawerColors},
};
use gpui_kit::base::Button as BaseButton;
use gpui_kit::component::{
    ActiveTheme, IndexPath,
    button::{Button, ButtonCustomVariant, ButtonVariants},
    scroll::{ScrollableElement, ScrollbarAxis},
    select::{SelectEvent, SelectState},
};
use gpui_kit::{prelude::FluentBuilder as _, *};
use serde::Deserialize;
use std::collections::BTreeMap;

pub(super) struct CustomizeDrawer {
    pub(super) open: bool,
    pub(super) toggle_focus: FocusHandle,
    filter: Entity<Choices>,
    scroll: ScrollHandle,
    body_scroll: ScrollHandle,
    toggle_anchor: ScrollAnchor,
    input_focus: BTreeMap<String, FocusHandle>,
    pub(super) source_input: Option<String>,
    _filter_subscription: Subscription,
    _focus_subscriptions: Vec<Subscription>,
}

impl CustomizeDrawer {
    pub(super) fn filter_focus(&self, cx: &App) -> FocusHandle {
        self.filter.focus_handle(cx)
    }

    pub(super) fn focus_toggle(&self, window: &mut Window, cx: &mut App) {
        self.toggle_anchor.scroll_to(window, cx);
        window.focus(&self.toggle_focus, cx);
    }

    pub(super) fn reset_scroll(&self) {
        self.scroll.set_offset(point(px(0.), px(0.)));
    }

    pub(super) fn new(
        device: &Device,
        window: &mut Window,
        cx: &mut Context<DeviceWorkspace>,
    ) -> Self {
        let filter =
            cx.new(|cx| SelectState::new(filter_choices(), Some(IndexPath::new(0)), window, cx));
        let subscription = cx.subscribe(&filter, |view, _, _: &SelectEvent<Vec<Choice>>, cx| {
            view.customize_drawer
                .scroll
                .set_offset(point(px(0.), px(0.)));
            cx.notify();
        });
        let input_ids = if device.product_id == 653 {
            keyboard_inputs(device.layout_id)
                .iter()
                .map(|input| input.id.clone())
                .collect::<Vec<_>>()
        } else {
            MOUSE_INPUTS
                .iter()
                .map(|(id, _, _)| (*id).to_owned())
                .collect()
        };
        let mut input_focus = BTreeMap::new();
        let mut focus_subscriptions = Vec::new();
        for id in input_ids {
            let focus = cx.focus_handle();
            let focused_id = id.clone();
            focus_subscriptions.push(cx.on_focus_in(&focus, window, move |view, _, cx| {
                if !view.customize_drawer.open {
                    return;
                }
                // Filtering changes row positions. Resolve the stable input ID
                // against the visible list when focus enters, then let the
                // ScrollHandle reveal only the part outside its viewport.
                if let Some(ix) = view
                    .drawer_rows(cx)
                    .iter()
                    .position(|row| row.id == focused_id)
                {
                    view.customize_drawer.scroll.scroll_to_item(ix);
                    cx.notify();
                }
            }));
            input_focus.insert(id, focus);
        }
        let body_scroll = ScrollHandle::default();
        let toggle_anchor = ScrollAnchor::for_handle(body_scroll.clone());
        Self {
            open: false,
            toggle_focus: cx.focus_handle(),
            filter,
            scroll: ScrollHandle::default(),
            body_scroll,
            toggle_anchor,
            input_focus,
            source_input: None,
            _filter_subscription: subscription,
            _focus_subscriptions: focus_subscriptions,
        }
    }
}

fn filter_choices() -> Vec<Choice> {
    vec![
        Choice::new("all", crate::i18n::t("ALL_BUTTONS")),
        Choice::new("customized", crate::i18n::t("CUSTOMIZED")),
    ]
}

// 182 module 1368, numeric counter order. The product image's visual order differs.
const MOUSE_INPUTS: [(&str, &str, &str); 8] = [
    ("LeftButton", "LEFT_CLICK", "MOUSE"),
    ("RightButton", "RIGHT_CLICK", "MOUSE"),
    ("MiddleButton", "SCROLL_CLICK", "MOUSE"),
    ("ScrollUp", "SCROLL_UP", "MOUSE"),
    ("ScrollDown", "SCROLL_DOWN", "MOUSE"),
    ("Button4", "MOUSE_BUTTON_4", "MOUSE"),
    ("Button5", "MOUSE_BUTTON_5", "MOUSE"),
    (
        "CycleUpSensitivityStages",
        "CYCLE_UP_SENSITIVITY",
        "SENSITIVITY",
    ),
];

#[derive(Deserialize)]
struct SourceGroup {
    group: SourceButtons,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceButtons {
    button_list: Vec<SourceInput>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceInput {
    #[serde(rename = "inputID")]
    pub(super) id: String,
    pub(super) button_key: String,
    counter: serde_json::Value,
    pub(super) default_value: String,
    assignment: String,
    pub(super) is_enabled: bool,
    #[serde(default)]
    pub(super) disabled: bool,
    is_side_panel_list: Option<bool>,
    is_key_toggle: Option<bool>,
    #[serde(default)]
    pub(super) disable_hypershift_mapping: bool,
    #[serde(default)]
    pub(super) function_list: Vec<String>,
    #[serde(default)]
    pub(super) disable_turbo_in_assignment: Vec<String>,
    #[serde(rename = "HID")]
    hid: Option<String>,
}

fn keyboard_inputs(layout: u32) -> &'static [SourceInput] {
    static INPUTS: std::sync::OnceLock<BTreeMap<u32, Vec<SourceInput>>> =
        std::sync::OnceLock::new();
    INPUTS
        .get_or_init(|| {
            (1..=18)
                .filter_map(|layout| {
                    crate::resources::keyboard_source_for_layout(layout).map(|source| {
                        let inputs = serde_json::from_str::<Vec<SourceGroup>>(source)
                            .expect("audited 653 ButtonPanel data")
                            .into_iter()
                            .flat_map(|group| group.group.button_list)
                            .collect();
                        (layout, inputs)
                    })
                })
                .collect()
        })
        .get(&if layout == 0 { 1 } else { layout })
        .map(Vec::as_slice)
        .unwrap_or_default()
}

pub(super) fn keyboard_mapping_input(layout: u32, id: &str) -> Option<&'static SourceInput> {
    keyboard_inputs(layout).iter().find(|input| input.id == id)
}

struct DrawerRow {
    id: String,
    counter: String,
    category: String,
    value: String,
    remapped: bool,
    enabled: bool,
}

impl DeviceWorkspace {
    pub(super) fn drawer_input_visible(&self, id: &str, cx: &App) -> bool {
        self.customize_drawer.open && self.drawer_rows(cx).iter().any(|row| row.id == id)
    }

    pub(super) fn apply_drawer_toggle(&mut self, open: bool) {
        if !open
            && self.mapping.as_ref().is_some_and(|mapping| {
                self.customize_drawer.source_input.as_ref() == Some(&mapping.input)
            })
        {
            self.mapping = None;
            self.customize_drawer.source_input = None;
        }
        self.customize_drawer.open = open;
    }

    pub(super) fn drawer_toggle(&self, cx: &mut Context<Self>) -> AnyElement {
        let open = self.customize_drawer.open;
        let button = Button::new("customize-drawer-toggle")
            .track_focus(&self.customize_drawer.toggle_focus)
            .ghost()
            .accessibility_label("输入列表")
            .toggled(open)
            .w(surface::css(38.))
            .h(surface::css(27.))
            .p_0()
            .border_1()
            .border_color(if open {
                cx.theme().primary
            } else {
                cx.theme().border
            })
            .rounded(cx.theme().font_size * (14. / 16.))
            .custom(
                ButtonCustomVariant::new(cx)
                    .color(cx.theme().group_box)
                    .hover(cx.theme().group_box),
            )
            .child(
                img(if open {
                    "synapse/drawer-active.svg"
                } else {
                    "synapse/drawer.svg"
                })
                .size(surface::css(18.)),
            )
            .on_click(cx.listener(move |view, _, window, cx| {
                view.continue_with(Continue::Drawer(!open), window, cx);
            }));
        div()
            .id("customize-drawer-trigger")
            .w(surface::css(38.))
            .h(surface::css(27.))
            .flex_shrink_0()
            .anchor_scroll(Some(self.customize_drawer.toggle_anchor.clone()))
            .child(button)
            .into_any_element()
    }

    /// Source left:230px/right width:calc(100% - 230px), including the
    /// <=1250px rule allowing the right body to shrink and scroll horizontally.
    pub(super) fn customize_surface(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let open = self.customize_drawer.open;
        // Retain the 230px drawer while its left edge and the remaining body
        // width transition over .2s (the source CSS's default ease curve).
        let drawer_width = gpui_kit::base::motion::transition(
            ElementId::from(("customize-drawer-width", cx.entity_id())),
            if open { 230_f32 } else { 0. },
            gpui_kit::base::motion::Transition::new(std::time::Duration::from_millis(200))
                .easing(gpui_kit::base::motion::Easing::Ease),
            window,
            cx,
        );
        let min_width = if self.pid() == 653 { 830.8 } else { 770. };
        let content = div()
            .id(SharedString::from(format!(
                "device-body-{}",
                self.identity()
            )))
            .test_support()
            .relative()
            .flex_1()
            .h_full()
            .min_w_0()
            .min_h_0()
            .overflow_scroll()
            .track_scroll(&self.customize_drawer.body_scroll)
            .child(
                div()
                    .w_full()
                    .min_w(surface::css(min_width + 40.))
                    .max_w(surface::css(surface::BODY_MAX_WIDTH + 40.))
                    .mx_auto()
                    // Both source stylesheets fix the widget body at 1000px;
                    // the scroll content also includes its two 20px insets.
                    .when(open, |this| {
                        this.w(surface::css(1040.))
                            .min_w(surface::css(1040.))
                            .max_w(surface::css(1040.))
                    })
                    .pt(surface::css(10.))
                    .px(surface::css(20.))
                    .pb(surface::css(20.))
                    .child(self.customize_page(cx)),
            )
            .scrollbar(&self.customize_drawer.body_scroll, ScrollbarAxis::Both);
        let from_drawer = open
            && self.mapping.as_ref().is_some_and(|mapping| {
                self.customize_drawer.source_input.as_ref() == Some(&mapping.input)
            });
        // The source gives drawer-origin editors left=drawer.width-20. Image
        // editors use the original left/right positions and stay inside the window.
        let scale = f32::from(window.rem_size()) / 16.;
        let viewport_width = f32::from(window.viewport_size().width) / scale;
        let right_side = self.mapping.as_ref().is_some_and(|mapping| {
            matches!(
                mapping.input.as_str(),
                "RightButton" | "ScrollUp" | "ScrollDown"
            )
        });
        let max_left = (viewport_width - 292.).max(0.);
        let min_left = if open && !from_drawer {
            230_f32.min(max_left)
        } else {
            0.
        };
        let left = if from_drawer {
            210.
        } else if right_side {
            viewport_width / 2. + if open { 375. } else { 320. }
        } else {
            viewport_width / 2. - if open { 395. } else { 622. }
        }
        .clamp(min_left, max_left);
        gpui_kit::component::h_flex()
            .id("customize-surface")
            .test_support()
            .relative()
            .items_stretch()
            .flex_1()
            .min_h_0()
            .min_w_0()
            .overflow_hidden()
            .when(drawer_width > 0., |this| {
                this.child(
                    div()
                        .relative()
                        .w(surface::css(drawer_width))
                        .h_full()
                        .flex_shrink_0()
                        .overflow_hidden()
                        .child(
                            div()
                                .absolute()
                                .top_0()
                                .left(surface::css(drawer_width - 230.))
                                .w(surface::css(230.))
                                .h_full()
                                .child(self.drawer_panel(cx)),
                        ),
                )
            })
            .child(content)
            .when(self.mapping.is_some(), |this| {
                this.child(
                    div()
                        .id("mapping-overlay")
                        .test_support()
                        .absolute()
                        .top(surface::css(4.))
                        .left(surface::css(left))
                        .w(surface::css(292.))
                        .h_full()
                        .max_h_full()
                        .pb(surface::css(8.))
                        .child(self.mapping_panel(window, cx)),
                )
            })
            .into_any_element()
    }

    fn drawer_rows(&self, cx: &App) -> Vec<DrawerRow> {
        let bindings = if self.hypershift {
            &self.settings().hypershift_bindings
        } else {
            &self.settings().bindings
        };
        let customized = self
            .customize_drawer
            .filter
            .read(cx)
            .selected_value()
            .is_some_and(|value| value == "customized");
        let mut rows = if self.pid() == 653 {
            keyboard_inputs(self.device().layout_id)
                .iter()
                .filter(|key| !key.disabled && key.is_side_panel_list != Some(false))
                .filter(|key| {
                    key.is_key_toggle
                        .is_none_or(|hyper| hyper == self.hypershift)
                })
                .filter(|key| {
                    !self.hypershift || key.hid.is_none() || !key.disable_hypershift_mapping
                })
                .filter(|key| key.counter.as_str() != Some("mediaVolume"))
                .map(|key| {
                    let counter = key
                        .counter
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| key.counter.to_string());
                    self.drawer_row_data(
                        &key.id,
                        counter,
                        &key.assignment,
                        &key.default_value,
                        key.is_enabled && self.mapping_input_enabled(&key.id),
                        bindings.get(&key.id),
                    )
                })
                .collect::<Vec<_>>()
        } else {
            MOUSE_INPUTS
                .into_iter()
                .enumerate()
                .map(|(index, (id, default, category))| {
                    self.drawer_row_data(
                        id,
                        (index + 1).to_string(),
                        category,
                        default,
                        self.mapping_input_enabled(id),
                        bindings.get(id),
                    )
                })
                .collect()
        };
        if customized {
            rows.retain(|row| row.remapped);
        }
        rows
    }

    fn drawer_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let colors = DrawerColors::new();
        let rows = self.drawer_rows(cx);
        let background = if self.pid() == 653 {
            colors.keyboard_surface()
        } else {
            colors.surface()
        };
        gpui_kit::component::v_flex()
            .id("customize-drawer")
            .test_support()
            .role(Role::Complementary)
            .aria_label("输入列表")
            .w(surface::css(230.))
            .h_full()
            .flex_shrink_0()
            .pb(surface::css(12.))
            .bg(background)
            .child(
                gpui_kit::component::h_flex()
                    .h(surface::css(67.))
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        surface::select(&self.customize_drawer.filter)
                            .id("drawer-filter")
                            .items(filter_choices())
                            .accessibility_label("输入列表筛选")
                            .w(surface::css(180.)),
                    )
                    .child(
                        Button::new("customize-drawer-close")
                            .ghost()
                            .accessibility_label("关闭输入列表")
                            .size(surface::css(26.))
                            .p_0()
                            .border_1()
                            .rounded_none()
                            .border_color(cx.theme().transparent)
                            .custom(
                                ButtonCustomVariant::new(cx)
                                    .color(cx.theme().transparent)
                                    .hover(background),
                            )
                            .child(img("synapse/drawer-close.svg").size(surface::css(20.)))
                            .on_click(cx.listener(|view, _, window, cx| {
                                view.continue_with(Continue::Drawer(false), window, cx)
                            })),
                    ),
            )
            .child(
                gpui_kit::component::v_flex()
                    .id("drawer-list")
                    .test_support()
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.customize_drawer.scroll)
                    .children(rows.into_iter().map(|row| self.drawer_row(row, cx)))
                    .scrollbar(&self.customize_drawer.scroll, ScrollbarAxis::Vertical),
            )
            .into_any_element()
    }

    fn drawer_row(&self, row: DrawerRow, cx: &mut Context<Self>) -> AnyElement {
        let colors = DrawerColors::new();
        let active = self
            .mapping
            .as_ref()
            .is_some_and(|mapping| mapping.input == row.id);
        let foreground = if !row.enabled {
            colors.muted()
        } else if row.remapped {
            if self.hypershift {
                colors.hypershift()
            } else {
                cx.theme().primary
            }
        } else {
            cx.theme().foreground
        };
        let id = row.id.clone();
        BaseButton::new(SharedString::from(format!("drawer-input-{}", row.id)))
            .track_focus(
                self.customize_drawer
                    .input_focus
                    .get(&row.id)
                    .expect("drawer inputs retain their source identity"),
            )
            .accessibility_label(format!(
                "{} · {} · {}",
                row.counter, row.category, row.value
            ))
            .disabled(!row.enabled)
            .selected(active)
            .w_full()
            .min_h(surface::css(50.))
            .flex_shrink_0()
            .p(surface::css(9.))
            .flex()
            .items_center()
            .bg(if active {
                cx.theme().group_box
            } else {
                cx.theme().transparent
            })
            .when(row.enabled, |this| {
                this.hover(|s| s.bg(colors.hover()))
                    .active(|s| s.bg(cx.theme().group_box))
                    .focus_visible(|s| s.bg(cx.theme().group_box))
            })
            .child(
                div()
                    .w(surface::css(56.))
                    .ml(-surface::css(10.))
                    .flex_shrink_0()
                    .text_center()
                    .text_size(surface::css(14.))
                    .line_height(surface::css(14.))
                    .text_color(if row.enabled {
                        cx.theme().foreground
                    } else {
                        colors.muted()
                    })
                    .child(crate::i18n::t_or(&row.counter, &row.counter)),
            )
            .child(
                div()
                    .w(surface::css(164.))
                    .border_l_1()
                    .border_color(colors.muted())
                    .pl(surface::css(5.))
                    .text_size(surface::css(14.))
                    .line_height(surface::css(16.))
                    .text_color(foreground)
                    .child(
                        div()
                            .text_size(surface::css(10.))
                            .text_color(colors.muted())
                            .child(row.category),
                    )
                    .child(
                        div()
                            .id("assignment")
                            .test_support()
                            .aria_label(row.value.clone())
                            .text_ellipsis()
                            .child(row.value),
                    ),
            )
            .on_click(cx.listener(move |view, _, window, cx| {
                if view
                    .mapping
                    .as_ref()
                    .is_none_or(|mapping| mapping.input != id)
                {
                    view.continue_with(Continue::DrawerInput(id.clone()), window, cx);
                }
            }))
            .into_any_element()
    }
    fn drawer_row_data(
        &self,
        id: &str,
        counter: String,
        category: &str,
        default: &str,
        enabled: bool,
        binding: Option<&String>,
    ) -> DrawerRow {
        let remapped = binding.is_some_and(|value| value != "default");
        let (category, value) = if let Some(value) = binding.filter(|_| remapped) {
            self.mapping_summary(value)
        } else {
            (category.to_string(), crate::i18n::t_or(default, default))
        };
        DrawerRow {
            id: id.to_string(),
            counter,
            category: crate::i18n::t_or(&category, &category),
            value,
            remapped,
            enabled,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::features::{settings::ProfileSettings, workspace::DeviceWorkspace};
    use gpui_kit::component::{Root, Theme};
    use gpui_kit::test::TestWindowExt;
    use gpui_kit::{AppContext, ScrollDelta, SharedString, TestAppContext, point, px, size};

    #[gpui_kit::test]
    fn drawer_activation_preserves_dirty_continuations_and_primary_click(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.set_reduce_motion(true);
        });
        cx.update(|cx| Theme::update(cx, |theme| theme.font_size = px(16.)));
        let mut workspace = None;
        let handle = cx.open_window(size(px(1080.), px(800.)), |window, cx| {
            let mut device = crate::model::measured_devices().remove(0);
            device.dkm_keys.clear();
            let view = cx.new(|cx| DeviceWorkspace::new(device, true, window, cx));
            workspace = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = workspace.unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("customize-drawer-toggle", cx);
            assert_eq!(
                window.find("customize-drawer").bounds().size.width,
                px(230.)
            );
            assert_eq!(
                window.find("drawer-input-LeftButton").disabled(),
                Some(true)
            );
            window.click("drawer-input-LeftButton", cx);
        })
        .unwrap();
        cx.update(|cx| assert!(view.read(cx).mapping.is_none()));
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("drawer-input-RightButton", cx);
            assert_eq!(window.find("mapping-overlay").bounds().left(), px(210.));
            window.click("mapping-category-mouse", cx);
            window.click("drawer-input-MiddleButton", cx);
            assert!(window.find("mapping-keep-editing").visible());
            window.click("mapping-keep-editing", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let state = view.read(cx);
            assert!(state.mapping_dirty());
            assert_eq!(state.mapping.as_ref().unwrap().input, "RightButton");
            assert_eq!(
                state.customize_drawer.source_input.as_deref(),
                Some("RightButton")
            );
        });
        cx.update_window(handle.into(), |_, window, cx| {
            assert_eq!(window.find("mapping-overlay").bounds().left(), px(210.));
            window.click("drawer-input-MiddleButton", cx);
            window.click("mapping-discard", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let state = view.read(cx);
            assert_eq!(state.mapping.as_ref().unwrap().input, "MiddleButton");
            assert!(!state.mapping_dirty());
            assert!(!state.settings().bindings.contains_key("RightButton"));
        });
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("customize-drawer-close", cx);
            assert!(window.try_find("customize-drawer").is_none());
            assert!(window.try_find("mapping-overlay").is_none());
            window.click("customize-drawer-toggle", cx);
            window.click("drawer-input-RightButton", cx);
            window.click("mapping-category-mouse", cx);
            window.click("mapping-apply", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert_ne!(
                window.find("drawer-input-LeftButton").disabled(),
                Some(true)
            );
            let summary = window.within("drawer-input-RightButton").find("assignment");
            assert!(!summary.label().unwrap().contains("local-mapping:"));
            window.click("drawer-input-LeftButton", cx);
        })
        .unwrap();
        cx.update(|cx| assert_eq!(view.read(cx).mapping.as_ref().unwrap().input, "LeftButton"));
    }

    #[gpui_kit::test]
    fn keyboard_drawer_filters_each_layer_and_scrolls_independently_after_resize(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.set_reduce_motion(true);
        });
        cx.update(|cx| Theme::update(cx, |theme| theme.font_size = px(16.)));
        let mut workspace = None;
        let handle = cx.open_window(size(px(1100.), px(900.)), |window, cx| {
            let mut device = crate::demo::demo_keyboard();
            let index = device
                .profiles
                .iter()
                .position(|profile| profile.id == device.active_profile)
                .unwrap();
            let mut settings = ProfileSettings::from_legacy(&device, &device.profiles[index]);
            settings.bindings.clear();
            settings.hypershift_bindings.clear();
            settings
                .bindings
                .insert("KEY_A".into(), "keyboard:Ctrl+C".into());
            settings
                .hypershift_bindings
                .insert("KEY_B".into(), "keyboard:Ctrl+V".into());
            device.profiles[index].settings = Some(settings);
            let view = cx.new(|cx| DeviceWorkspace::new(device, true, window, cx));
            workspace = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = workspace.unwrap();
        let (body_id, profile_entity, filter_entity, settings) = cx.update(|cx| {
            let state = view.read(cx);
            (
                SharedString::from(format!("device-body-{}", state.identity())),
                state.controls.profile.entity_id(),
                state.customize_drawer.filter.entity_id(),
                state.settings().clone(),
            )
        });
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("customize-drawer-toggle", cx);
            assert!(window.try_find("drawer-input-KEY_FN").is_none());
            assert!(window.try_find("drawer-input-KEY_WINDOWS").is_none());
            assert!(window.try_find("drawer-input-KEY_F9").is_some());
            window.within("drawer-filter").click("input", cx);
            window.press("down", cx);
            window.press("enter", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.find("drawer-input-KEY_A").visible());
            assert!(window.try_find("drawer-input-KEY_B").is_none());
            assert!(
                window
                    .within("drawer-input-KEY_A")
                    .find("assignment")
                    .label()
                    .unwrap()
                    .contains("C")
            );
            window.click("mapping-hypershift", cx);
            assert!(window.find("drawer-input-KEY_B").visible());
            assert!(window.try_find("drawer-input-KEY_A").is_none());
        })
        .unwrap();
        cx.simulate_window_resize(handle.into(), size(px(700.), px(430.)));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let drawer = window.find("customize-drawer").bounds();
            let body = window.find(body_id.clone()).bounds();
            assert_eq!(drawer.size.width, px(230.));
            assert_eq!(body.left(), drawer.right());
            assert_eq!(body.size.width, px(470.));
            assert_eq!(
                window.find("drawer-filter").value(),
                Some(crate::i18n::t("CUSTOMIZED").as_str())
            );
            window.within("drawer-filter").click("input", cx);
            window.press("up", cx);
            window.press("enter", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("drawer-input-KEY_F9").is_none());
            assert!(!window.find("drawer-input-KEY_NUMPAD_PERIOD").visible());
            let nav = window.find("device-navigation").bounds();
            let product_key = window.find("keyboard-input-KEY_A").bounds();
            let body = window.find(body_id.clone()).bounds();
            window.scroll(
                "drawer-list",
                ScrollDelta::Pixels(point(px(0.), px(-10_000.))),
                cx,
            );
            let last = window.find("drawer-input-KEY_NUMPAD_PERIOD");
            assert!(last.visible());
            assert!(last.bounds().bottom() <= window.find("customize-drawer").bounds().bottom());
            assert_eq!(window.find("keyboard-input-KEY_A").bounds(), product_key);
            assert_eq!(window.find(body_id.clone()).bounds(), body);
            window.scroll(
                body_id.clone(),
                ScrollDelta::Pixels(point(px(-10_000.), px(0.))),
                cx,
            );
            let shifted_key = window.find("keyboard-input-KEY_A").bounds();
            assert!(shifted_key.left() < product_key.left());
            assert_eq!(shifted_key.size, product_key.size);
            assert_eq!(
                window.find("drawer-input-KEY_NUMPAD_PERIOD").bounds(),
                last.bounds()
            );
            assert_eq!(window.find("device-navigation").bounds(), nav);
            window.click("customize-drawer-close", cx);
            assert_eq!(window.find(body_id.clone()).bounds().size.width, px(700.));
        })
        .unwrap();
        cx.update(|cx| {
            let state = view.read(cx);
            assert!(state.hypershift);
            assert_eq!(state.settings(), &settings);
            assert_eq!(state.controls.profile.entity_id(), profile_entity);
            assert_eq!(state.customize_drawer.filter.entity_id(), filter_entity);
        });
    }

    #[gpui_kit::test]
    fn mouse_drawer_close_and_layer_change_preserve_pending_mapping(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.set_reduce_motion(true);
        });
        let mut workspace = None;
        let handle = cx.open_window(size(px(1080.), px(800.)), |window, cx| {
            let mut device = crate::model::measured_devices().remove(0);
            device.dkm_keys.clear();
            let view = cx.new(|cx| DeviceWorkspace::new(device, true, window, cx));
            workspace = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = workspace.unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("customize-drawer-toggle", cx);
            window.click("drawer-input-RightButton", cx);
            window.click("mapping-category-mouse", cx);
            // The current key and layer are not navigation requests.
            window.click("drawer-input-RightButton", cx);
            window.click("mapping-standard", cx);
            assert!(window.try_find("mapping-keep-editing").is_none());
            window.click("customize-drawer-close", cx);
            assert!(window.find("mapping-keep-editing").visible());
            window.click("mapping-keep-editing", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            let state = view.read(cx);
            assert!(state.customize_drawer.open);
            assert!(state.mapping_dirty());
            assert!(!state.hypershift);
        });
        cx.update_window(handle.into(), |_, window, cx| {
            window.click("mapping-hypershift", cx);
            assert!(window.find("mapping-save").visible());
            window.click("mapping-save", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("customize-drawer").is_none());
            assert!(window.try_find("mapping-overlay").is_none());
        })
        .unwrap();
        cx.update(|cx| {
            let state = view.read(cx);
            assert!(state.hypershift);
            assert!(state.settings().bindings.contains_key("RightButton"));
            assert!(
                !state
                    .settings()
                    .hypershift_bindings
                    .contains_key("RightButton")
            );
            assert!(!state.mapping_dirty());
        });
    }

    #[gpui_kit::test]
    fn japanese_keyboard_drawer_and_product_use_the_same_input_set(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.set_reduce_motion(true);
        });
        let handle = cx.open_window(size(px(1200.), px(850.)), |window, cx| {
            let mut device = crate::demo::demo_keyboard();
            device.layout_id = 12;
            let view = cx.new(|cx| DeviceWorkspace::new(device, true, window, cx));
            Root::new(view, window, cx)
        });
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("customize-drawer-toggle", cx);
            for input in [
                "KEY_YEN",
                "KEY_RO",
                "KEY_MUHENKAN",
                "KEY_HENKAN",
                "KEY_KATAKANA_HIRAGANA",
            ] {
                assert!(
                    window
                        .try_find(SharedString::from(format!("drawer-input-{input}")))
                        .is_some()
                );
                assert!(
                    window
                        .try_find(SharedString::from(format!("keyboard-input-{input}")))
                        .is_some()
                );
            }
            // Invisible dial children still belong to the list and editor.
            assert!(window.try_find("drawer-input-ScrollUp").is_some());
            assert!(window.try_find("drawer-input-ScrollDown").is_some());
        })
        .unwrap();
    }

    #[gpui_kit::test]
    fn tab_reveals_drawer_rows_and_filtering_preserves_a_visible_focus_target(
        cx: &mut TestAppContext,
    ) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            cx.set_reduce_motion(true);
        });
        cx.update(|cx| Theme::update(cx, |theme| theme.font_size = px(16.)));
        let mut workspace = None;
        let handle = cx.open_window(size(px(1100.), px(850.)), |window, cx| {
            let mut device = crate::demo::demo_keyboard();
            device.layout_id = 16;
            let index = device
                .profiles
                .iter()
                .position(|profile| profile.id == device.active_profile)
                .unwrap();
            let mut settings = ProfileSettings::from_legacy(&device, &device.profiles[index]);
            settings.bindings.clear();
            settings
                .bindings
                .insert("KEY_NUMPAD_PERIOD".into(), "keyboard:Ctrl+C".into());
            device.profiles[index].settings = Some(settings);
            let view = cx.new(|cx| DeviceWorkspace::new(device, true, window, cx));
            workspace = Some(view.clone());
            Root::new(view, window, cx)
        });
        let view = workspace.unwrap();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("customize-drawer-toggle", cx);
            window.click("drawer-input-DKM_KB_SIDE1", cx);
            window.click("mapping-close", cx);
            assert_eq!(
                window.find("drawer-input-DKM_KB_SIDE1").focused(),
                Some(true)
            );
        })
        .unwrap();
        cx.simulate_window_resize(handle.into(), size(px(900.), px(430.)));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            let product_key = window.find("keyboard-input-KEY_A").bounds();
            assert!(!window.find("drawer-input-DKM_M_01").visible());
            for _ in 0..7 {
                window.press("tab", cx);
            }
            window.render_frame(cx);
            let row = window.find("drawer-input-DKM_M_01");
            let list = window.find("drawer-list").bounds();
            assert_eq!(row.focused(), Some(true));
            assert!(row.visible());
            assert!(row.bounds().top() >= list.top());
            assert!(row.bounds().bottom() <= list.bottom());
            assert_eq!(window.find("keyboard-input-KEY_A").bounds(), product_key);
            window.press("enter", cx);
            assert_eq!(window.find("drawer-input-DKM_M_01").selected(), Some(true));
            window.click("customize-drawer-close", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("customize-drawer").is_none());
            let toggle = window.find("customize-drawer-toggle");
            assert_eq!(toggle.focused(), Some(true));
            assert!(toggle.visible());
        })
        .unwrap();
        cx.simulate_window_resize(handle.into(), size(px(1100.), px(850.)));
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            window.click("customize-drawer-toggle", cx);
            window.scroll(
                "drawer-list",
                ScrollDelta::Pixels(point(px(0.), px(-10_000.))),
                cx,
            );
            window.within("drawer-filter").click("input", cx);
            window.press("down", cx);
            window.press("enter", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.find("drawer-input-KEY_NUMPAD_PERIOD").visible());
            assert!(window.try_find("drawer-input-DKM_M_01").is_none());
            window.click("drawer-input-KEY_NUMPAD_PERIOD", cx);
            window.click("mapping-category-default", cx);
            window.click("mapping-apply", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.try_find("drawer-input-KEY_NUMPAD_PERIOD").is_none());
            assert!(window.try_find("mapping-overlay").is_none());
            assert_eq!(
                window.within("drawer-filter").find("input").focused(),
                Some(true)
            );
            window.press("enter", cx);
            window.press("up", cx);
            window.press("enter", cx);
        })
        .unwrap();
        cx.run_until_parked();
        cx.update_window(handle.into(), |_, window, cx| {
            window.render_frame(cx);
            assert!(window.find("drawer-input-DKM_KB_SIDE1").visible());
        })
        .unwrap();
        cx.update(|cx| {
            assert_eq!(
                view.read(cx)
                    .settings()
                    .bindings
                    .get("KEY_NUMPAD_PERIOD")
                    .map(String::as_str),
                Some("default"),
            );
        });
    }
}
