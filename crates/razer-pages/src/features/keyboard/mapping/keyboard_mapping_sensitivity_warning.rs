//! Original mapped sensitivity-clutch warning and existing local storage object.
use super::*;

#[derive(Clone)]
pub struct KeyboardMappingSensitivityWarningReadRequested {
    pub product_id: u32,
}
#[derive(Clone)]
pub struct KeyboardMappingSensitivityWarningHidden {
    pub product_id: u32,
}
impl EventEmitter<KeyboardMappingSensitivityWarningReadRequested> for KeyboardProductWorkspace {}
impl EventEmitter<KeyboardMappingSensitivityWarningHidden> for KeyboardProductWorkspace {}

#[derive(Default)]
pub(super) struct State {
    preference: Option<Value>,
    awaiting_preference: bool,
    shown: bool,
    checked: bool,
}
impl State {
    pub(super) fn dismiss(&mut self) {
        self.awaiting_preference = false;
        self.shown = false;
        self.checked = false;
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Source {
    #[serde(rename = "product_id")]
    product_id: u32,
    title: String,
    description: String,
    second_desc: String,
    cancel_text: String,
    confirm_text: String,
    checkbox_label: Option<String>,
}
fn source(pid: u32) -> &'static Source {
    static DATA: OnceLock<Vec<Source>> = OnceLock::new();
    DATA.get_or_init(|| read_keyboard_mapping_sensitivity_warning_catalog())
        .iter()
        .find(|row| row.product_id == pid)
        .expect("audited mapped clutch warning")
}
impl KeyboardProductWorkspace {
    pub(super) fn request_mapping_sensitivity_warning(
        &mut self,
        slot: usize,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(edit) = &self.analog_gamepad.mapping.edit else {
            return false;
        };
        // Original secondary caller overwrites assignment and assignmentValue with
        // selectedSecondaryFunction before testing clutch values.
        if slot != 0
            || edit.button["assignment"] == "CONTROLLER_V2"
            || !matches!(
                edit.button["assignmentValue"].as_str(),
                Some(
                    "LEFT_SENSITIVITY_CLUTCH"
                        | "RIGHT_SENSITIVITY_CLUTCH"
                        | "GLOBAL_SENSITIVITY_CLUTCH"
                )
            )
        {
            return false;
        }
        self.analog_gamepad
            .mapping
            .sensitivity_warning
            .awaiting_preference = true;
        cx.emit(KeyboardMappingSensitivityWarningReadRequested {
            product_id: self.spec.product_id,
        });
        cx.notify();
        true
    }
    /// The original synapse_<pid> localStorage object, preserving absence.
    pub fn observe_mapping_sensitivity_warning(
        &mut self,
        preference: Option<Value>,
        cx: &mut Context<Self>,
    ) {
        let state = &mut self.analog_gamepad.mapping.sensitivity_warning;
        state.preference = preference;
        if !state.awaiting_preference {
            return;
        }
        state.awaiting_preference = false;
        let show = state
            .preference
            .as_ref()
            .map(|pref| {
                pref.get("showSensitivityWarning")
                    .is_none_or(|value| value.is_null() || js_truthy(value))
            })
            .unwrap_or(true);
        if show {
            state.shown = true
        } else {
            self.save_analog_mapping(cx);
        }
        cx.notify();
    }
    pub(super) fn mapping_sensitivity_warning_element(
        &self,
        cx: &Context<Self>,
    ) -> Option<AnyElement> {
        self.analog_gamepad
            .mapping
            .sensitivity_warning
            .shown
            .then(|| {
                deferred(Warning { owner: cx.entity() })
                    .with_priority(107)
                    .into_any_element()
            })
    }
    fn mapping_sensitivity_warning_card(&self, cx: &Context<Self>) -> AnyElement {
        let metadata = source(self.spec.product_id);
        let checked = self.analog_gamepad.mapping.sensitivity_warning.checked;
        v_flex()
            .w(surface::css(402.))
            .p(surface::css(20.))
            .gap(surface::css(20.))
            .relative()
            .bg(rgb(0x111111))
            .border_1()
            .border_color(rgb(0xfd8611))
            .rounded(surface::css(3.))
            .font_family("Roboto")
            .text_size(surface::css(14.))
            .line_height(surface::css(17.))
            .text_color(rgb(0xcccccc))
            .child(
                h_flex()
                    .items_center()
                    .justify_center()
                    .gap(surface::css(10.))
                    .h(surface::css(20.))
                    .text_size(surface::css(16.))
                    .text_color(rgb(0xfd8611))
                    .child(
                        img("synapse/keyboard-mapping-sensitivity-warning/warning.svg")
                            .w(surface::css(20.))
                            .h(surface::css(27.)),
                    )
                    .child(t(&metadata.title).to_uppercase()),
            )
            .child(
                v_flex()
                    .child(t(&metadata.description))
                    .child(div().h(surface::css(17.)))
                    .child(t(&metadata.second_desc)),
            )
            .children(metadata.checkbox_label.as_ref().map(|label| {
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_center()
                    .child(
                        Checkbox::new("keyboard-map-clutch-warning-checkbox")
                            .checked(checked)
                            .on_click(cx.listener(|this, checked: &bool, _, cx| {
                                this.analog_gamepad.mapping.sensitivity_warning.checked = *checked;
                                cx.notify();
                            })),
                    )
                    .child(div().mt(surface::css(-13.)).child(t(label)))
            }))
            .child(
                h_flex()
                    .items_center()
                    .justify_center()
                    .gap(surface::css(10.))
                    .child(
                        warning_button(
                            "keyboard-map-clutch-warning-cancel",
                            t(&metadata.cancel_text),
                            false,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.analog_gamepad.mapping.sensitivity_warning.dismiss();
                            this.analog_gamepad.mapping.can_save_after_prompt = true;
                            if let Some(edit) = &mut this.analog_gamepad.mapping.edit {
                                edit.mark_clean()
                            }
                            cx.emit(KeyboardProductChanged);
                            cx.notify();
                        })),
                    )
                    .child(
                        warning_button(
                            "keyboard-map-clutch-warning-confirm",
                            t(&metadata.confirm_text),
                            true,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            let state = &mut this.analog_gamepad.mapping.sensitivity_warning;
                            let hide = state.checked
                                && state.preference.as_ref().is_some_and(Value::is_object);
                            if hide {
                                if let Some(pref) = &mut state.preference {
                                    pref["showSensitivityWarning"] = json!(false)
                                }
                            }
                            state.dismiss();
                            if hide {
                                cx.emit(KeyboardMappingSensitivityWarningHidden {
                                    product_id: this.spec.product_id,
                                })
                            }
                            this.save_analog_mapping(cx);
                            cx.notify();
                        })),
                    ),
            )
            .into_any_element()
    }
}
fn js_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::Number(v) => v.as_f64().is_some_and(|v| v != 0.),
        Value::String(v) => !v.is_empty(),
        _ => true,
    }
}
fn warning_button(id: &'static str, label: String, confirm: bool) -> gpui_kit::base::Button {
    BaseButton::new(id)
        .min_w(surface::css(90.))
        .h(surface::css(27.))
        .px(surface::css(16.))
        .rounded(surface::css(3.))
        .font_family("Roboto")
        .text_size(surface::css(12.))
        .line_height(surface::css(14.))
        .bg(rgb(if confirm { 0x44d62c } else { 0x707070 }))
        .text_color(rgb(if confirm { 0x222222 } else { 0xffffff }))
        .when(confirm, |button| {
            button.border_1().border_color(rgba(0x0000004d))
        })
        .active(|button| button.opacity(0.3))
        .child(label.to_uppercase())
}
#[derive(IntoElement)]
struct Warning {
    owner: Entity<KeyboardProductWorkspace>,
}
impl RenderOnce for Warning {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
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
                            .id("keyboard-map-clutch-warning-backdrop")
                            .w(size.width)
                            .h(size.height)
                            .occlude()
                            .bg(rgba(0x111111b3))
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
                            .on_scroll_wheel(|_, _, cx| cx.stop_propagation()),
                    ),
            )
            .child(
                anchored()
                    .anchor(Anchor::TopLeft)
                    .position(point(size.width / 2. - scale * 201., top + scale * 110.))
                    .child(
                        self.owner
                            .update(cx, |this, cx| this.mapping_sensitivity_warning_card(cx)),
                    ),
            )
    }
}

// Generated catalog reader: razer-pages/keyboard_mapping_sensitivity_warning_data.
fn read_keyboard_mapping_sensitivity_warning_catalog() -> Vec<Source> {
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
                description: r.child(decode_3)?,
                second_desc: r.child(decode_3)?,
                cancel_text: r.child(decode_3)?,
                confirm_text: r.child(decode_3)?,
                checkbox_label: r.child(decode_4)?,
            },
            1 => Source {
                product_id: r.child(decode_2)?,
                title: r.child(decode_3)?,
                description: r.child(decode_3)?,
                second_desc: r.child(decode_3)?,
                cancel_text: r.child(decode_3)?,
                confirm_text: r.child(decode_3)?,
                checkbox_label: None,
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
    fn decode_4(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<String>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_3)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_KEYBOARD_MAPPING_SENSITIVITY_WARNING_DATA,
        0,
        "55318f210a5c588087858dec4a78010cbf5be9de93f00c1e22ce317820d2a380",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
#[test]
fn packaged_catalog_keyboard_mapping_sensitivity_warning_data_decodes() {
    let _ = read_keyboard_mapping_sensitivity_warning_catalog();
}
// End generated catalog reader: razer-pages/keyboard_mapping_sensitivity_warning_data.
