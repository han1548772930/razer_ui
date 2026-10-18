//! Actual KeypadButtonPanel (30910), mounted only for the three KEYPAD products.
//! Current independent source receipts: keyboard-keypad-panel-current.json.gz.
use super::*;

#[derive(Deserialize)]
struct ButtonGroup {
    button_key: String,
    group: Value,
}

/// CSS fixed positioning belongs to the product iframe, which includes the
/// shell toolbar. Build the definite layout after its current bounds have been
/// measured, so neither initial paint nor resizing uses yesterday's height.
struct FixedDrawer {
    header: Option<AnyElement>,
    list: Option<AnyElement>,
    child: Option<AnyElement>,
    frame: Rc<Cell<Bounds<Pixels>>>,
    list_subtract: f32,
}
impl IntoElement for FixedDrawer {
    type Element = Self;
    fn into_element(self) -> Self {
        self
    }
}
impl Element for FixedDrawer {
    type RequestLayoutState = ();
    type PrepaintState = ();
    fn id(&self) -> Option<ElementId> {
        None
    }
    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }
    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        (
            window.request_layout(
                Style {
                    position: Position::Absolute,
                    ..Style::default()
                },
                [],
                cx,
            ),
            (),
        )
    }
    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        let frame = self.frame.get();
        let css = |value| surface::css(value).to_pixels(window.rem_size());
        let height = (frame.size.height - css(90.)).max(px(0.));
        let list_height = (height - css(self.list_subtract)).max(px(0.));
        let mut child = v_flex()
            .w(surface::css(430.))
            .h(height)
            .overflow_hidden()
            .bg(rgb(0x2b2b2b))
            .children(self.header.take())
            .child(
                div()
                    .h(list_height)
                    .flex_shrink_0()
                    .children(self.list.take()),
            )
            .into_any_element();
        child.layout_as_root(
            size(
                AvailableSpace::Definite(css(430.)),
                AvailableSpace::Definite(height),
            ),
            window,
            cx,
        );
        child.prepaint_at(
            point(
                frame.left(),
                frame.top() + surface::css(88.).to_pixels(window.rem_size()),
            ),
            window,
            cx,
        );
        self.child = Some(child);
    }
    fn paint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        _: &mut (),
        _: &mut (),
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(child) = &mut self.child {
            child.paint(window, cx);
        }
    }
}
#[derive(Deserialize)]
struct Product {
    product_id: u32,
    constants: Value,
    buttons: Vec<ButtonGroup>,
    secondary: panel_secondary::Metadata,
}
fn source(pid: u32) -> Option<&'static Product> {
    static DATA: OnceLock<Vec<Product>> = OnceLock::new();
    DATA.get_or_init(|| read_keyboard_mapping_keypad_panel_catalog())
        .iter()
        .find(|row| row.product_id == pid)
}
pub(super) fn supported(pid: u32) -> bool {
    source(pid).is_some()
}
pub(super) fn width(pid: u32) -> f32 {
    if supported(pid) { 430. } else { 230. }
}
fn visible(button: &Value, side: &Value, mode: &Value, constants: &Value) -> bool {
    let group = &button["keyPadGroup"];
    let direction = group.get("directionMode");
    let shown = direction.is_none_or(|direction| {
        if *mode == constants["mZI"]["DIRECTIONAL"] {
            *direction != constants["mZI"]["BUTTONS"]
        } else if *mode == constants["mZI"]["BUTTONS"] {
            *direction != constants["mZI"]["DIRECTIONAL"]
        } else if *mode == constants["mZI"]["ALL"] {
            *direction != constants["mZI"]["BUTTONS"]
                && *direction != constants["mZI"]["DIRECTIONAL"]
                && *direction != constants["bFI"]["Other"]
        } else {
            true
        }
    });
    shown
        && group["columnSide"] == *side
        && button.get("isSidePanelList").is_none_or(panel_truthy)
        && text(&button["counter"]) != "mediaVolume"
}
const ARROWS: [[&str; 4]; 2] = [
    [
        "synapse/keyboard-keypad-arrow-up.svg",
        "synapse/keyboard-keypad-arrow-down.svg",
        "synapse/keyboard-keypad-arrow-up-left.svg",
        "synapse/keyboard-keypad-arrow-down-left.svg",
    ],
    [
        "synapse/keyboard-keypad-arrow-left.svg",
        "synapse/keyboard-keypad-arrow-right.svg",
        "synapse/keyboard-keypad-arrow-up-right.svg",
        "synapse/keyboard-keypad-arrow-down-right.svg",
    ],
];
impl KeyboardProductWorkspace {
    pub(super) fn keypad_mapping_drawer(&self, cx: &Context<Self>) -> AnyElement {
        let metadata = source(self.spec.product_id).expect("mounted actual keypad panel");
        let state = &self.analog_gamepad.mapping;
        let mut rows = self
            .mapping_buttons()
            .iter()
            .map(|button| {
                let mut button = self.mapping_active_button(button);
                if let Some(group) = metadata
                    .buttons
                    .iter()
                    .find(|group| button["buttonKey"] == group.button_key)
                {
                    button["keyPadGroup"] = group.group.clone();
                }
                button
            })
            .filter(|button| {
                (!self.hypershift
                    || !panel_truthy(&button["HID"])
                    || !panel_truthy(&button["disableHypershiftMapping"]))
                    && (!state.filter_customized || panel_truthy(&button["isMapped"]))
            })
            .collect::<Vec<_>>();
        if rows.iter().all(|button| {
            button["counter"]
                .as_f64()
                .is_some_and(|value| value.fract() == 0.)
        }) {
            rows.sort_by(|a, b| {
                a["counter"]
                    .as_f64()
                    .unwrap()
                    .partial_cmp(&b["counter"].as_f64().unwrap())
                    .expect("finite source counters")
            });
        }
        // Map preserves the first occurrence of each group, including empty
        // rendered columns after direction and side-panel filtering.
        let mut groups: Vec<(String, Vec<Value>)> = vec![];
        for button in rows {
            let kind = text(&button["keyPadGroup"]["type"]);
            if let Some((_, rows)) = groups.iter_mut().find(|(key, _)| *key == kind) {
                rows.push(button);
            } else {
                groups.push((kind, vec![button]));
            }
        }
        // Original keymapReducer starts with BUTTONS. An observed/local profile
        // directionMode replaces that initial value; no device read is fabricated.
        let mode = self
            .draft
            .get("directionMode")
            .unwrap_or(&metadata.constants["mZI"]["BUTTONS"]);
        let header = h_flex()
            .relative()
            .h(surface::css(67.))
            .flex_shrink_0()
            .border_b_1()
            .border_color(rgb(0x5d5d5d))
            .items_center()
            .child(
                div()
                    .ml(surface::css(20.))
                    .child(panel_dropdown::control(self, cx)),
            )
            .child(
                BaseButton::new("keypad-drawer-close")
                    .absolute()
                    .right(surface::css(10.))
                    .size(surface::css(26.))
                    .p_0()
                    .border_1()
                    .border_color(rgba(0x00000000))
                    .flex()
                    .items_center()
                    .justify_center()
                    .hover(|close| close.border_color(rgb(0x5d5d5d)))
                    .active(|close| close.border_color(rgb(0x44d62c)))
                    .child(panel_dropdown::close_icon(self.spec.product_id))
                    .on_click(cx.listener(|this, _, window, cx| {
                        // Actual close calls the root updateActiveButton
                        // for the same key without the ordinary dirty
                        // guard, reselecting the saved active mapping.
                        this.button_drawer_open = !this.button_drawer_open;
                        let active = this
                            .analog_gamepad
                            .mapping
                            .edit
                            .as_ref()
                            .map(|edit| text(&edit.button["buttonKey"]));
                        if let Some(active) = active {
                            this.clear_analog_mapping(cx);
                            this.analog_mapping_continue(Next::Key(active, false), window, cx);
                        }
                        cx.notify();
                    })),
            )
            .into_any_element();
        let list =
            v_flex()
                .id("keypad-drawer-list")
                .h_full()
                .min_h_0()
                .overflow_y_scroll()
                .track_scroll(&state.scroll)
                .children(
                    groups
                        .into_iter()
                        .enumerate()
                        .map(|(group_index, (kind, buttons))| {
                            let dpad = kind == text(&metadata.constants["bFI"]["DPad"]);
                            v_flex()
                                .child(
                                    div()
                                        .px(surface::css(20.))
                                        .py(surface::css(15.))
                                        .font_family("Roboto")
                                        .text_size(surface::css(14.))
                                        .text_color(rgb(0x707070))
                                        .text_left()
                                        .child(kind),
                                )
                                .child(h_flex().items_start().children(
                                    ["LEFT", "RIGHT"].into_iter().enumerate().map(
                                        |(column, side)| {
                                            v_flex().children(
                                                buttons
                                                    .iter()
                                                    .enumerate()
                                                    .filter(|(_, button)| {
                                                        visible(
                                                            button,
                                                            &metadata.constants["fkE"][side],
                                                            mode,
                                                            &metadata.constants,
                                                        )
                                                    })
                                                    .enumerate()
                                                    .map(|(visible_index, (index, button))| {
                                                        let identity = group_index * 2048
                                                            + column * 1024
                                                            + index;
                                                        self.keypad_mapping_row(
                                                            identity,
                                                            button,
                                                            dpad.then(|| {
                                                                ARROWS[column]
                                                                    .get(visible_index)
                                                                    .copied()
                                                            })
                                                            .flatten(),
                                                            dpad,
                                                            cx,
                                                        )
                                                    }),
                                            )
                                        },
                                    ),
                                ))
                        }),
                )
                .into_any_element();
        FixedDrawer {
            header: Some(header),
            list: Some(list),
            child: None,
            frame: state.app_frame_bounds.clone(),
            // 555/580 mount config-drawer-content, retaining max-height -145;
            // 747's actual parent omits that class and uses show-btn -70.
            list_subtract: if self.spec.product_id == 747 { 70. } else { 145. },
        }
        .into_any_element()
    }

    fn keypad_mapping_row(
        &self,
        index: usize,
        button: &Value,
        arrow: Option<&'static str>,
        dpad: bool,
        cx: &Context<Self>,
    ) -> AnyElement {
        let key = text(&button["buttonKey"]);
        let mapped = panel_truthy(&button["isMapped"]);
        let disabled = !panel_truthy(&button["isEnabled"]);
        let label = t(&text(
            &button[if mapped { "assignmentValue" } else { "defaultValue" }],
        ));
        let assignment = text(&button["assignment"]);
        let heading = if assignment.contains(' ') {
            assignment.split_whitespace().collect::<Vec<_>>().join("_")
        } else {
            assignment
        };
        BaseButton::new(SharedString::from(format!("keypad-drawer-row-{key}")))
            .relative()
            .w(surface::css(205.))
            .min_h(surface::css(50.))
            .p(surface::css(if self.spec.product_id == 747 {
                10.
            } else {
                9.
            }))
            .when(self.spec.product_id == 747, |row| row.h(surface::css(50.)))
            .flex()
            .items_center()
            .disabled(disabled)
            .bg(if self.mapping_selection_matches(&key) {
                rgb(0x111111)
            } else {
                rgba(0x00000000)
            })
            .hover(|row| row.bg(rgb(0x383838)))
            .active(|row| row.bg(rgb(0x111111)))
            .child(
                div()
                    .w(surface::css(56.))
                    .ml(surface::css(-10.))
                    .text_center()
                    .text_color(rgb(if disabled { 0x707070 } else { 0xcccccc }))
                    .when(dpad, |index| {
                        index
                            .h(surface::css(20.))
                            .flex()
                            .items_center()
                            .justify_center()
                    })
                    .children(arrow.map(|asset| img(asset).size(surface::css(10.))))
                    .when(!dpad, |title| {
                        title.child(panel_index::Index::new(
                            self,
                            index,
                            text(&button["counter"]),
                            disabled,
                            cx,
                        ))
                    }),
            )
            .child(
                v_flex()
                    .w(surface::css(134.))
                    .whitespace_nowrap()
                    .border_l_1()
                    .border_color(rgb(0x707070))
                    .pl(surface::css(5.))
                    .line_height(surface::css(16.))
                    .text_left()
                    .text_color(if mapped {
                        rgb(if self.hypershift { 0xfd8611 } else { 0x44d62c })
                    } else {
                        rgb(if disabled { 0x707070 } else { 0xcccccc })
                    })
                    .child(
                        div()
                            .text_size(surface::css(10.))
                            .text_color(rgb(0x707070))
                            .child(t(&heading).to_uppercase()),
                    )
                    // The source's onHoverData has no mounted optDom for
                    // KeypadButtonPanel. Render only this binding text.
                    .child(
                        div()
                            .font_family("Roboto")
                            .text_size(surface::css(14.))
                            .line_height(surface::css(16.))
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(label),
                    ),
            )
            .children(panel_secondary::marker(
                self,
                &source(self.spec.product_id)
                    .expect("actual keypad row")
                    .secondary,
                button,
                index,
                disabled,
                cx,
            ))
            .on_click(cx.listener(move |this, _, window, cx| {
                this.analog_mapping_next(Next::Key(key.clone(), true), window, cx);
            }))
            .into_any_element()
    }
}

// Generated catalog reader: razer-pages/keyboard_mapping_keypad_panel_data.
fn read_keyboard_mapping_keypad_panel_catalog() -> Vec<Product> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<Product>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Product> {
        let value = match r.tag()? {
            0 => Product {
                product_id: r.child(decode_2)?,
                constants: r.child(decode_3)?,
                buttons: r.child(decode_4)?,
                secondary: r.child(decode_7)?,
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
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<serde_json::Value> {
        let value = match r.tag()? {
            0 => r.external_value()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<ButtonGroup>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_5)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<ButtonGroup> {
        let value = match r.tag()? {
            0 => ButtonGroup {
                button_key: r.child(decode_6)?,
                group: r.child(decode_3)?,
            },
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
    ) -> anyhow::Result<panel_secondary::Metadata> {
        let value = match r.tag()? {
            0 => panel_secondary::Metadata {
                group_field: r.child(decode_8)?,
                assignment_field: r.child(decode_8)?,
                default_assignment: r.child(decode_8)?,
                label: r.child(decode_8)?,
                asset: r.child(decode_8)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_8(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_MAPPING_KEYPAD_PANEL_DATA,
        0,
        "085ebaa83c281334c46d147af389262bdc3df7c90cbd27afe7d3c539dba6da6a",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod keyboard_mapping_keypad_panel_data_catalog_tests {
    #[test]
    fn packaged_catalog_keyboard_mapping_keypad_panel_data_decodes() {
        let _ = super::read_keyboard_mapping_keypad_panel_catalog();
    }
}
// End generated catalog reader: razer-pages/keyboard_mapping_keypad_panel_data.
