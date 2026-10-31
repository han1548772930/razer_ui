//! Actual main mapping menu consumers and popup shell. Native observations are
//! distinguished from local drafts; missing service state never grants AI access.
use super::*;
use razer_widgets::keyboard_navigation::Button as BaseButton;
use gpui_kit::base::{ElementExt as _};
use razer_widgets::theme::{MouseCustomizeColors as Colors, MouseMappingMenuColors as MenuColors};
use std::time::Duration;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};
#[path = "mouse_mapping_popup_current.rs"]
mod popup;
#[path = "mouse_remap_search_current.rs"]
mod remap;
#[path = "mouse_mapping_menu_tooltip_current.rs"]
mod tooltip;

#[derive(Deserialize)]
struct Icon {
    normal: Option<String>,
    selected: Option<String>,
}
#[derive(Deserialize)]
struct Policy {
    product_id: u32,
    babel_162: bool,
    device_info: Value,
    profile_names: Vec<String>,
    show_all: String,
    show_less: String,
    close_asset: String,
    icons: BTreeMap<String, Icon>,
    tooltip_offset: f32,
    tooltip_clamp: bool,
}
fn policy(pid: u32) -> Option<&'static Policy> {
    static DATA: OnceLock<Vec<Policy>> = OnceLock::new();
    DATA.get_or_init(|| read_mouse_mapping_menu_current_catalog())
        .iter()
        .find(|p| p.product_id == pid)
}
pub(super) struct State {
    remap: remap::State,
    collapsed: bool,
    hide: bool,
    scroll: ScrollHandle,
    pub(super) root: Rc<Cell<Bounds<Pixels>>>,
    pub(super) drawer: Rc<Cell<Bounds<Pixels>>>,
    pub(super) anchors: Rc<RefCell<BTreeMap<String, Bounds<Pixels>>>>,
    tooltip: Option<tooltip::Tip>,
    header_hover: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            remap: remap::State::default(),
            collapsed: true,
            hide: true,
            scroll: ScrollHandle::default(),
            root: Rc::new(Cell::new(Bounds::default())),
            drawer: Rc::new(Cell::new(Bounds::default())),
            anchors: Rc::new(RefCell::new(BTreeMap::new())),
            tooltip: None,
            header_hover: false,
        }
    }
}
fn ai_enabled(observation: &Value, device_info: &Value) -> bool {
    let flags = observation["provisionFlags"].as_u64().unwrap_or(0);
    let whitelist = observation["whiteList"].as_array();
    flags & 1 != 0
        && whitelist
            .is_none_or(|values| values.is_empty() || values.contains(&device_info["productId"]))
}
fn functions(
    button: &Value,
    hyper: bool,
    profile: Option<&str>,
    _ai: bool,
    policy: &Policy,
) -> Vec<String> {
    let Some(values) = button["functionList"].as_array() else {
        return Vec::new();
    };
    values
        .iter()
        .filter_map(Value::as_str)
        .filter(|name| {
            !(*name == "RAZER_HYPERSHIFT" && hyper)
                && (policy.babel_162
                    || *name != "APP_SPECIFIC"
                    || profile.is_some_and(|name| policy.profile_names.iter().any(|p| p == name)))
                && *name != "AI_LAUNCHER"
        })
        .map(str::to_owned)
        .collect()
}
impl MouseProductWorkspace {
    pub(super) fn collapse_current_mouse_mapping_menu(&mut self) {
        self.mapping_menu_current_state.collapsed = true;
        self.mapping_menu_current_state.tooltip = None;
    }
    pub(super) fn current_mouse_menu_functions(&self, button: &Value) -> Vec<String> {
        let Some(policy) = policy(self.spec.product_id) else {
            return Vec::new();
        };
        functions(
            button,
            self.hypershift,
            self.current_mouse_menu_profile(),
            ai_enabled(
                &self.mouse_customize_observation["global.aiLauncher"],
                &policy.device_info,
            ),
            policy,
        )
    }
    pub(super) fn current_mouse_menu_icon(
        &self,
        function: &str,
        selected: bool,
    ) -> Option<SharedString> {
        let icon = policy(self.spec.product_id)?.icons.get(function)?;
        if selected {
            icon.selected.as_ref().or(icon.normal.as_ref())
        } else {
            icon.normal.as_ref()
        }
        .map(|asset| SharedString::from(asset.clone()))
    }
    fn current_mouse_menu_profile(&self) -> Option<&str> {
        let reducer = &self.mouse_customize_observation["profileReducer"];
        let guid = &reducer["selectedProfileGuid"];
        // Undefined selectedProfile cannot match a real guid.
        if guid.is_null() {
            return None;
        }
        reducer["profiles"]
            .as_array()?
            .iter()
            .find(|p| p["guid"] == *guid)?["name"]
            .as_str()
    }
    pub(super) fn current_mouse_mapping_popup_contents(
        &self,
        button: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<Div> {
        let policy = policy(self.spec.product_id)?;
        let body = self.current_mouse_mapping_body(button, window, cx)?;
        let state = &self.mapping_menu_current_state;
        let macro_mode =
            self.mouse_customize_observation["customizeReducer"]["displayMode"] == "macro";
        let observed_ai = &self.mouse_customize_observation["global.aiLauncher"];
        let all = functions(
            button,
            self.hypershift,
            self.current_mouse_menu_profile(),
            ai_enabled(observed_ai, &policy.device_info),
            policy,
        );
        let count = policy.device_info["defaultVisibleFunctionsCount"]
            .as_u64()
            .unwrap_or(5) as usize;
        let hiding =
            policy.device_info["isHidingKeyMapping"] == true && button["buttonKey"] != "RazerAI";
        let display = if hiding && state.hide {
            all.iter()
                .take(count)
                .chain(all.last().into_iter())
                .cloned()
                .collect::<Vec<_>>()
        } else {
            all.clone()
        };
        let width = gpui_kit::base::motion::transition(
            ElementId::from(("mouse-current-menu-width", cx.entity_id())),
            if state.collapsed { 40_f32 } else { 250_f32 },
            gpui_kit::base::motion::Transition::new(Duration::from_millis(200))
                .easing(gpui_kit::base::motion::Easing::Ease),
            window,
            cx,
        );
        let body_height = (window.viewport_size().height - window.rem_size() * (140. / 16.))
            .max(window.rem_size() * (310. / 16.))
            .min(window.rem_size() * (570. / 16.));
        let mut menu = v_flex()
            .id("mouse-current-mapping-menu")
            .absolute()
            .top_0()
            .left_0()
            .w(surface::css(width))
            .h_full()
            .overflow_x_hidden()
            .bg(MenuColors::background())
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.mapping_menu_current_state.collapsed = !*hovered;
                if !*hovered {
                    this.mapping_menu_current_state.tooltip = None;
                }
                cx.notify();
            }));
        let mut list = v_flex()
            .id("mouse-current-mapping-menu-list")
            .w_full()
            .min_h_0()
            .max_h(body_height)
            .overflow_y_scroll()
            .track_scroll(&state.scroll);
        let mut occurrences = BTreeMap::<String, usize>::new();
        for function in display {
            let occurrence = occurrences.entry(function.clone()).or_default();
            let row_id = format!("{function}-{}", *occurrence);
            *occurrence += 1;
            let selected = self.mapping_current_state.function == function;
            let action = function.clone();
            let icon = policy.icons.get(&function).and_then(|icon| {
                if selected {
                    icon.selected.as_ref().or(icon.normal.as_ref())
                } else {
                    icon.normal.as_ref()
                }
            });
            let mut row =
                BaseButton::new(SharedString::from(format!("mouse-current-menu-{row_id}")))
                    .w_full()
                    .h(surface::css(40.))
                    .p_0()
                    .justify_start()
                    .relative()
                    .bg(if selected {
                        Colors::selected_surface()
                    } else {
                        MenuColors::background()
                    })
                    .text_color(if selected {
                        Colors::selected()
                    } else {
                        Colors::text()
                    })
                    .hover(|style| style.bg(MenuColors::hover()))
                    .active(|style| style.bg(Colors::selected_surface()))
                    .accessibility_label(t(&function))
                    .on_click(cx.listener(move |this, _, window, cx| {
                        if policy.babel_162 || this.mapping_current_state.function != action {
                            this.change_current_mouse_mapping_function(&action, window, cx);
                            this.mapping_menu_current_state.collapsed = true;
                        }
                    }));
            // Keep the icon in the same fixed 40px slot used by the other
            // mapping menus.  An absolutely positioned image attached
            // directly to a button can be laid out against the deferred
            // popup's transient bounds, leaving the decoded image outside
            // the visible row while the row background still paints.
            let mut row_content = h_flex()
                .w(surface::css(250.))
                .h(surface::css(40.))
                .flex_shrink_0()
                .child(
                    div()
                        .size(surface::css(40.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .justify_center()
                        .children(icon.map(|asset| {
                            img(SharedString::from(asset.clone())).size(surface::css(20.))
                        })),
                );
            if !state.collapsed {
                let label = t(&function);
                let upper = label.to_uppercase();
                let measured_label = upper.clone();
                let hovered_label = label.clone();
                let overflow = Rc::new(Cell::new(false));
                let measured_overflow = overflow.clone();
                let anchor = Rc::new(Cell::new(Bounds::default()));
                let measured_anchor = anchor.clone();
                row_content = row_content.child(
                    div()
                        .id(SharedString::from(format!(
                            "mouse-current-menu-label-{row_id}"
                        )))
                        .ml(surface::css(10.))
                        .w(surface::css(200.))
                        .text_size(surface::css(12.))
                        .line_height(surface::css(40.))
                        .truncate()
                        .on_prepaint(move |bounds, window, _| {
                            measured_anchor.set(bounds);
                            measured_overflow.set(
                                surface::label_width(&measured_label, 12., window)
                                    * f32::from(window.rem_size())
                                    / 16.
                                    > f32::from(bounds.size.width),
                            );
                        })
                        .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                            this.mapping_menu_current_state.tooltip = (*hovered && overflow.get())
                                .then(|| tooltip::Tip {
                                    text: hovered_label.clone(),
                                    anchor: anchor.clone(),
                                });
                            cx.notify();
                        }))
                        .child(upper),
                );
            }
            row = row.child(row_content);
            list = list.child(row);
        }
        if hiding && all.len() > count + 1 && !state.collapsed {
            list = list.child(
                BaseButton::new("mouse-current-menu-show-all")
                    .w_full()
                    .py(surface::css(15.))
                    .text_size(surface::css(14.))
                    .text_color(Colors::text())
                    .hover(|style| style.text_color(Colors::selected()))
                    .child(div().underline().child(t(if state.hide {
                        &policy.show_all
                    } else {
                        &policy.show_less
                    })))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.mapping_menu_current_state.hide =
                            !this.mapping_menu_current_state.hide;
                        cx.notify();
                    })),
            );
        }
        menu = menu.child(list);
        let title = button["defaultValue"].as_str().unwrap_or("");
        let title_text = t(title);
        let title_overflow = surface::label_width(&title_text, 14., window) > 200.;
        let header_tip = !policy.babel_162 && state.header_hover && title_overflow;
        let head = div()
            .id("mouse-current-mapping-menu-header")
            .absolute()
            .top(-surface::css(36.))
            .left(-surface::css(1.))
            .w(surface::css(292.))
            .h(surface::css(36.))
            .bg(MenuColors::background())
            .border_1()
            .border_color(Colors::line())
            .rounded_t(surface::css(5.))
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .text_color(MenuColors::header())
            .flex()
            .items_center()
            .justify_center()
            .on_hover(cx.listener(|this, hovered: &bool, _, cx| {
                this.mapping_menu_current_state.header_hover = *hovered;
                cx.notify();
            }))
            .children(self.current_mouse_remap_search(cx))
            .child(
                div()
                    .max_w(surface::css(200.))
                    .truncate()
                    .child(title_text.clone()),
            )
            .when(header_tip, |head| {
                head.child(
                    div()
                        .absolute()
                        .left(surface::css(41.))
                        .top(surface::css(35.))
                        .p(surface::css(10.))
                        .bg(razer_widgets::theme::TooltipColors::background())
                        .border_1()
                        .border_color(Colors::line())
                        .whitespace_nowrap()
                        .child(title_text),
                )
            })
            .child(
                BaseButton::new("mouse-current-popup-close")
                    .absolute()
                    .right_0()
                    .top_0()
                    .size(surface::css(36.))
                    .p_0()
                    .hover(|style| style.bg(MenuColors::close_hover()))
                    .active(|style| style.bg(MenuColors::close_pressed()))
                    .accessibility_label(t("CLOSE"))
                    .child(
                        img(SharedString::from(policy.close_asset.clone())).size(surface::css(20.)),
                    )
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.request_current_mapping_close(window, cx)
                    })),
            );
        let tip_opacity = surface::fade_opacity(
            ("mouse-current-menu-tooltip-opacity", cx.entity_id()),
            if state.tooltip.is_some() { 1. } else { 0. },
            100,
            window,
            cx,
        );
        Some(
            div()
                .relative()
                .w(surface::css(292.))
                .h(body_height)
                .bg(Colors::selected_surface())
                .border_1()
                .border_color(Colors::line())
                .rounded_b(surface::css(5.))
                .child(head)
                .child(
                    div()
                        .id("mouse-current-mapping-popup-body")
                        .ml(surface::css(40.))
                        .w(surface::css(250.))
                        .h_full()
                        .p(surface::css(20.))
                        .overflow_y_scroll()
                        .opacity(if state.remap.search_key.is_empty() { 1. } else { 0.3 })
                        .child(body),
                )
                .when(!macro_mode, |surface| surface.child(menu))
                .children(
                    state
                        .tooltip
                        .as_ref()
                        .filter(|_| !macro_mode)
                        .map(|tip| tooltip::render(tip, policy, tip_opacity)),
                ),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn source_product_id_and_provision_bits_control_whitelist() {
        assert!(ai_enabled(
            &json!({"provisionFlags":1,"whiteList":[163]}),
            &json!({"productId":163})
        ));
        assert!(!ai_enabled(
            &json!({"provisionFlags":2,"whiteList":[163]}),
            &json!({"productId":163})
        ));
        assert!(!ai_enabled(
            &json!({"provisionFlags":1,"whiteList":[162]}),
            &json!({"productId":163})
        ));
        assert!(!ai_enabled(&Value::Null, &json!({"productId":163})));
    }
    #[::core::prelude::v1::test]
    fn menu_version_filters_preserve_original_order() {
        let button = json!({"functionList":["DEFAULT","APP_SPECIFIC","AI_LAUNCHER","RAZER_HYPERSHIFT","DISABLE"]});
        assert_eq!(
            functions(&button, true, None, false, policy(70).unwrap()),
            vec!["DEFAULT", "DISABLE"]
        );
        assert_eq!(
            functions(&button, true, None, false, policy(231).unwrap()),
            vec!["DEFAULT", "DISABLE"]
        );
        assert_eq!(
            functions(&button, true, None, false, policy(162).unwrap()),
            vec!["DEFAULT", "APP_SPECIFIC", "DISABLE"]
        );
        assert_eq!(
            functions(
                &button,
                false,
                Some("Adobe Photoshop"),
                true,
                policy(70).unwrap()
            )
            .len(),
            4
        );
    }

    #[::core::prelude::v1::test]
    fn representative_product_branches_keep_registered_menu_icon_aliases() {
        let catalog = super::read_mouse_mapping_menu_current_catalog();
        for product_id in [70, 162, 190, 229, 231, 2594, 45066] {
            let policy = catalog
                .iter()
                .find(|policy| policy.product_id == product_id)
                .unwrap_or_else(|| panic!("missing mapping menu policy for PID{product_id}"));
            assert!(
                policy
                    .close_asset
                    .starts_with("synapse/mouse-mapping-menu-current/"),
                "PID{product_id} close asset must use the packaged menu namespace"
            );
            assert_eq!(
                policy.icons.len(),
                if product_id == 162 { 22 } else { 29 },
                "PID{product_id} icon coverage changed"
            );
            for (function, icon) in &policy.icons {
                let normal = icon
                    .normal
                    .as_deref()
                    .unwrap_or_else(|| panic!("PID{product_id} {function} has no normal icon"));
                assert!(
                    normal.starts_with("synapse/mouse-mapping-menu-current/"),
                    "PID{product_id} {function} normal icon escaped the packaged menu namespace"
                );
                if let Some(selected) = icon.selected.as_deref() {
                    assert!(
                        selected.starts_with("synapse/mouse-mapping-menu-current/"),
                        "PID{product_id} {function} selected icon escaped the packaged menu namespace"
                    );
                }
            }
        }
    }
}

// Generated catalog reader: razer-pages/mouse_mapping_menu_current_data.
fn read_mouse_mapping_menu_current_catalog() -> Vec<Policy> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<Policy>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Policy> {
        let value = match r.tag()? {
            0 => Policy {
                product_id: r.child(decode_2)?,
                babel_162: r.child(decode_3)?,
                device_info: r.child(decode_4)?,
                profile_names: r.child(decode_5)?,
                show_all: r.child(decode_6)?,
                show_less: r.child(decode_6)?,
                close_asset: r.child(decode_6)?,
                icons: r.child(decode_7)?,
                tooltip_offset: r.child(decode_10)?,
                tooltip_clamp: r.child(decode_3)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u32> {
        let value = match r.tag()? {
            0 => r.u32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<serde_json::Value> {
        let value = match r.tag()? {
            0 => r.external_value()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<String>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_6)?,
            1 => Default::default(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, Icon>> {
        let value = match r.tag()? {
            0 => r.map(decode_6, decode_8)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_8(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Icon> {
        let value = match r.tag()? {
            0 => Icon {
                normal: r.child(decode_9)?,
                selected: r.child(decode_9)?,
            },
            1 => Icon {
                normal: r.child(decode_9)?,
                selected: None,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_9(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<String>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_6)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_10(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<f32> {
        let value = match r.tag()? {
            0 => r.f32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_MOUSE_MAPPING_MENU_CURRENT_DATA,
        0,
        "f49eec40c89f14fb6070dab60d4cfbd589b61066e5b7a96fc654074fd6edcca1",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod mouse_mapping_menu_current_data_catalog_tests {
    #[test]
    fn packaged_catalog_mouse_mapping_menu_current_data_decodes() {
        let _ = super::read_mouse_mapping_menu_current_catalog();
    }
}
// End generated catalog reader: razer-pages/mouse_mapping_menu_current_data.
