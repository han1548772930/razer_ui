//! Actual root renderSaveAlert. Close and Don't Save have different caller effects.
use super::*;
#[derive(IntoElement)]
pub(super) struct Dialog {
    pub(super) content: AnyElement,
}
impl RenderOnce for Dialog {
    fn render(self, window: &mut Window, _: &mut App) -> impl IntoElement {
        let size = window.viewport_size();
        let scale = window.rem_size() / 16.;
        let top = scale * 42.;
        div()
            .child(
                anchored()
                    .anchor(Anchor::TopLeft)
                    .position(point(px(0.), top))
                    .child(
                        div()
                            .id("keyboard-mapping-save-backdrop")
                            .w(size.width)
                            .h((size.height - top).max(px(0.)))
                            .occlude()
                            .bg(rgba(0x00000080))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                            .on_scroll_wheel(|_, _, cx| cx.stop_propagation()),
                    ),
            )
            .child(
                anchored()
                    .anchor(Anchor::LeftCenter)
                    .position(point(
                        size.width / 2. - scale * 200.,
                        top + (size.height - top) * 0.3,
                    ))
                    .child(self.content),
            )
    }
}
#[derive(Deserialize)]
struct Source {
    product_id: u32,
    title: String,
    message: String,
    message2: String,
    save: String,
    dont_save: String,
    hypershift_title: String,
    hypershift_message: String,
}
fn source(pid: u32) -> &'static Source {
    static DATA: OnceLock<Vec<Source>> = OnceLock::new();
    DATA.get_or_init(|| read_keyboard_mapping_confirmation_catalog())
        .iter()
        .find(|row| row.product_id == pid)
        .expect("audited current root unsaved mapping caller")
}
fn button(id: &str, label: String) -> gpui_kit::base::Button {
    let long = label.encode_utf16().count() > 9;
    BaseButton::new(SharedString::from(id.to_owned()))
        .min_w(surface::css(90.))
        .h(surface::css(27.))
        .px(surface::css(if long { 16. } else { 5. }))
        .bg(rgb(0x707070))
        .border_1()
        .border_color(rgba(0x0000004d))
        .rounded(surface::css(3.))
        .font_family("Roboto")
        .text_size(surface::css(12.))
        .line_height(surface::css(14.))
        .text_color(rgb(0x111111))
        .hover(|button| button.opacity(0.8))
        .flex()
        .items_center()
        .justify_center()
        .child(label)
}
impl KeyboardProductWorkspace {
    pub(super) fn request_mapping_save(&mut self, slot: usize, cx: &mut Context<Self>) {
        fn truthy(value: &Value) -> bool {
            match value {
                Value::Null => false,
                Value::Bool(value) => *value,
                Value::String(value) => !value.is_empty(),
                Value::Number(value) => value.as_f64().is_some_and(|value| value != 0.),
                _ => true,
            }
        }
        let Some(edit) = &self.analog_gamepad.mapping.edit else {
            return;
        };
        let button = &edit.button;
        let secondary_key = self.spec.config["DEFAULTMAPPINGS"]
            .as_array()
            .and_then(|rows| {
                rows.iter().find(|row| {
                    truthy(&row["keyInput"]["HID"]) && row["keyInput"]["HID"] == button["HID"]
                })
            });
        let reserved = truthy(&button["HID"])
            && (slot == 1 || secondary_key.is_some() || truthy(&button["MappingGroup"]));
        if reserved && edit.slots[slot].category == "RAZER_HYPERSHIFT" {
            self.analog_gamepad.mapping.hypershift_confirmation = true;
            cx.notify();
            return;
        }
        if self.request_mapping_sensitivity_warning(slot, cx) {
            return;
        }
        self.save_analog_mapping(cx);
    }
    pub(super) fn mapping_hypershift_confirmation_card(&self, cx: &Context<Self>) -> AnyElement {
        let source = source(self.spec.product_id);
        v_flex()
            .relative()
            .w(surface::css(400.))
            .px(surface::css(30.))
            .py(surface::css(20.))
            .bg(rgb(0x111111))
            .border_1()
            .border_color(rgb(0x44d62c))
            .rounded(surface::css(5.))
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .text_color(rgb(0xcccccc))
            .text_center()
            .child(
                BaseButton::new("keyboard-map-hypershift-confirmation-close")
                    .accessibility_label(t("CLOSE"))
                    .absolute()
                    .right(surface::css(8.))
                    .top(surface::css(8.))
                    .size(surface::css(20.))
                    .child(img("synapse/keyboard-mapping-recording/close.svg").size_full())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.analog_gamepad.mapping.hypershift_confirmation = false;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .mb(surface::css(20.))
                    .font_family("RazerF5")
                    .text_size(surface::css(16.))
                    .line_height(surface::css(19.))
                    .text_color(rgb(0x44d62c))
                    .child(t(&source.hypershift_title).to_uppercase()),
            )
            .child(t(&source.hypershift_message))
            .child(
                h_flex()
                    .mt(surface::css(20.))
                    .gap(surface::css(10.))
                    .justify_center()
                    .child(
                        button("keyboard-map-hypershift-discard", t(&source.dont_save)).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.analog_gamepad.mapping.hypershift_confirmation = false;
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        button("keyboard-map-hypershift-confirm", t(&source.save)).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.analog_gamepad.mapping.hypershift_confirmation = false;
                                this.save_analog_mapping(cx);
                                cx.notify();
                            }),
                        ),
                    ),
            )
            .into_any_element()
    }
    pub(super) fn mapping_confirmation_card(&self, cx: &Context<Self>) -> AnyElement {
        let source = source(self.spec.product_id);
        v_flex()
            .relative()
            .w(surface::css(400.))
            .px(surface::css(30.))
            .py(surface::css(20.))
            .bg(rgb(0x111111))
            .border_1()
            .border_color(rgb(0x44d62c))
            .rounded(surface::css(5.))
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .text_color(rgb(0xcccccc))
            .text_center()
            .child(
                BaseButton::new("keyboard-map-confirmation-close")
                    .accessibility_label(t("CLOSE"))
                    .absolute()
                    .right(surface::css(8.))
                    .top(surface::css(8.))
                    .size(surface::css(20.))
                    .child(img("synapse/keyboard-mapping-recording/close.svg").size_full())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.analog_gamepad.mapping.pending_save = None;
                        this.analog_gamepad.mapping.can_save_after_prompt = true;
                        // Root dismissSave(true) calls saveMapRef(false,true): local
                        // canSave becomes true while Redux isMappingChanged is retained.
                        cx.emit(KeyboardProductChanged);
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .mb(surface::css(20.))
                    .font_family("RazerF5")
                    .text_size(surface::css(16.))
                    .line_height(surface::css(19.))
                    .text_color(rgb(0x44d62c))
                    .child(t(&source.title).to_uppercase()),
            )
            .child(t(&source.message))
            .child(div().h(surface::css(17.)))
            .child(t(&source.message2))
            .child(
                h_flex()
                    .mt(surface::css(20.))
                    .gap(surface::css(10.))
                    .justify_center()
                    .child(
                        button("analog-confirm-discard", t(&source.dont_save)).on_click(
                            cx.listener(|this, _, _, cx| {
                                // Caller supplies nextActionRef only. Original dontSave()
                                // executes dontSaveActionRef, so it does not run nextAction.
                                this.analog_gamepad.mapping.pending_save = None;
                                this.analog_gamepad.mapping.can_save_after_prompt = true;
                                if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                                    edit.mark_clean();
                                }
                                cx.emit(KeyboardProductChanged);
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        button("analog-confirm-save", t(&source.save)).on_click(cx.listener(
                            |this, _, window, cx| {
                                let next = this.analog_gamepad.mapping.pending_save.clone();
                                if this.save_analog_mapping(cx) {
                                    if let Some(next) = next {
                                        this.analog_mapping_continue(next, window, cx);
                                    }
                                }
                            },
                        )),
                    ),
            )
            .into_any_element()
    }
}

// Generated catalog reader: razer-pages/keyboard_mapping_confirmation_data.
fn read_keyboard_mapping_confirmation_catalog() -> Vec<Source> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<Source>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Source> {
        let value = match r.tag()? {
            0 => Source {
                product_id: r.child(decode_2)?,
                title: r.child(decode_3)?,
                message: r.child(decode_3)?,
                message2: r.child(decode_3)?,
                save: r.child(decode_3)?,
                dont_save: r.child(decode_3)?,
                hypershift_title: r.child(decode_3)?,
                hypershift_message: r.child(decode_3)?,
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
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_MAPPING_CONFIRMATION_DATA,
        0,
        "06af1aab71ff369792dfadc2486508ecd710c2cc76328dfbcd52dff3a2bb37b2",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod keyboard_mapping_confirmation_data_catalog_tests {
    #[test]
    fn packaged_catalog_keyboard_mapping_confirmation_data_decodes() {
        let _ = super::read_keyboard_mapping_confirmation_catalog();
    }
}
// End generated catalog reader: razer-pages/keyboard_mapping_confirmation_data.
