//! Current ordinary mapping's hover-expanded function blade.
use super::*;
#[derive(Deserialize)]
struct Artwork {
    product_id: u32,
    assets: BTreeMap<String, Icon>,
}
#[derive(Deserialize)]
struct Icon {
    normal: Option<String>,
    active: Option<String>,
}
fn artwork(pid: u32) -> &'static Artwork {
    static DATA: OnceLock<Vec<Artwork>> = OnceLock::new();
    DATA.get_or_init(|| read_keyboard_mapping_blade_artwork_catalog())
        .iter()
        .find(|row| row.product_id == pid)
        .expect("audited keyboard blade")
}
impl KeyboardProductWorkspace {
    pub(super) fn mapping_function_blade(&self, slot: usize, cx: &Context<Self>) -> AnyElement {
        let edit = self
            .analog_gamepad
            .mapping
            .edit
            .as_ref()
            .expect("mounted mapping");
        // Source secondary forces the collapsed class even after mouse enter.
        let expanded = slot == 0 && self.analog_gamepad.mapping.category_menu[slot];
        let content = v_flex()
            .id(SharedString::from(format!(
                "keyboard-function-blade-{slot}"
            )))
            .w_full()
            .h_full()
            .overflow_hidden()
            .when(expanded, |blade| blade.overflow_y_scroll())
            .bg(rgb(0x222222))
            .occlude()
            .on_hover(cx.listener(move |this, on: &bool, _, cx| {
                this.analog_gamepad.mapping.category_menu[slot] = *on;
                cx.notify();
            }))
            .children(
                edit.button["functionList"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|row| {
                        let name = row.as_str()?;
                        if !two_tap::category_visible(self, slot, name) {
                            return None;
                        }
                        let active = edit.slots[slot].category == name;
                        let icon =
                            artwork(self.spec.product_id)
                                .assets
                                .get(name)
                                .and_then(|icon| {
                                    if active {
                                        icon.active.as_ref().or(icon.normal.as_ref())
                                    } else {
                                        icon.normal.as_ref()
                                    }
                                });
                        let name = name.to_owned();
                        Some(
                            BaseButton::new(SharedString::from(format!(
                                "keyboard-function-{slot}-{name}"
                            )))
                            .w(surface::css(250.))
                            .h(surface::css(40.))
                            .flex_shrink_0()
                            .p_0()
                            .bg(rgb(if active { 0x111111 } else { 0x222222 }))
                            .hover(|row| row.bg(rgb(0x393939)))
                            .text_color(rgb(if active { 0x44d62c } else { 0xcccccc }))
                            .text_size(surface::css(12.))
                            .line_height(surface::css(14.))
                            .child(
                                h_flex()
                                    .items_center()
                                    .w_full()
                                    .h_full()
                                    .child(
                                        div()
                                            .size(surface::css(40.))
                                            .flex_shrink_0()
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .children(icon.map(|icon| {
                                                img(SharedString::from(icon.clone()))
                                                    .size(surface::css(20.))
                                            })),
                                    )
                                    .child(
                                        div().ml(surface::css(10.)).child(t(&name).to_uppercase()),
                                    ),
                            )
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    if this
                                        .analog_gamepad
                                        .mapping
                                        .edit
                                        .as_ref()
                                        .is_some_and(|edit| edit.slots[slot].category != name)
                                    {
                                        this.stage_analog_category(&name, slot, window, cx);
                                    }
                                },
                            )),
                        )
                    }),
            )
            .into_any_element();
        BladeWidth {
            content,
            expanded,
            identity: (
                ElementId::from(("keyboard-function-blade-width", cx.entity_id())),
                format!("slot-{slot}"),
            )
                .into(),
        }
        .into_any_element()
    }
}
#[derive(IntoElement)]
struct BladeWidth {
    content: AnyElement,
    expanded: bool,
    identity: ElementId,
}
impl RenderOnce for BladeWidth {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let max_width = gpui_kit::base::motion::transition(
            self.identity,
            if self.expanded { 250. } else { 40. },
            gpui_kit::base::motion::Transition::new(std::time::Duration::from_millis(200))
                .easing(gpui_kit::base::motion::Easing::Ease),
            window,
            cx,
        );
        div()
            .absolute()
            .left_0()
            .top_0()
            .w(surface::css(250.))
            .max_w(surface::css(max_width))
            .h_full()
            .overflow_hidden()
            .child(self.content)
    }
}
#[derive(IntoElement)]
pub(super) struct PopupSize {
    pub(super) content: AnyElement,
    pub(super) width: f32,
}
impl RenderOnce for PopupSize {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        // Source root height is viewport - 140, min310/max570, plus its head36.
        let height =
            (f32::from(window.viewport_size().height) * 16. / f32::from(window.rem_size()) - 140.)
                .clamp(310., 570.);
        div()
            .w(surface::css(self.width))
            .h(surface::css(height + 36.))
            .child(self.content)
    }
}

// Generated catalog reader: razer-pages/keyboard_mapping_blade_artwork_data.
fn read_keyboard_mapping_blade_artwork_catalog() -> Vec<Artwork> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<Artwork>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Artwork> {
        let value = match r.tag()? {
            0 => Artwork {
                product_id: r.child(decode_2)?,
                assets: r.child(decode_3)?,
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
    fn decode_3(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, Icon>> {
        let value = match r.tag()? {
            0 => r.map(decode_4, decode_5)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_4(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Icon> {
        let value = match r.tag()? {
            0 => Icon {
                normal: r.child(decode_6)?,
                active: r.child(decode_6)?,
            },
            1 => Icon {
                normal: r.child(decode_6)?,
                active: None,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<String>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_4)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_MAPPING_BLADE_ARTWORK_DATA,
        0,
        "aa574c451c071f8d027a50350322806e2b8fb76d6f7375a6f4638ee4663eb75d",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod keyboard_mapping_blade_artwork_data_catalog_tests {
    #[test]
    fn packaged_catalog_keyboard_mapping_blade_artwork_data_decodes() {
        let _ = super::read_keyboard_mapping_blade_artwork_catalog();
    }
}
// End generated catalog reader: razer-pages/keyboard_mapping_blade_artwork_data.
