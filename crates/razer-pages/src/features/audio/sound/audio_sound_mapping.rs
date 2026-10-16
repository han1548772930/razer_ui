//! Six independently audited AUDIO roots: their single wheel and source drawer.
//! Local mapping lists and original requests do not acknowledge device writes.
use super::sound::{Product, js_truthy, message, message_for_locale, product};
use super::*;
use gpui_kit::component::select::SelectItem;
use razer_widgets::theme::AudioEnhancementColors as Colors;
use std::{cell::Cell, rc::Rc};

#[path = "audio_sound_mapping_popup.rs"]
mod popup;

#[derive(Default)]
pub(super) struct MappingState {
    pub(super) open: bool,
    pub(super) observed: bool,
    pub(super) anchor: Rc<Cell<Bounds<Pixels>>>,
    pub(super) root: Rc<Cell<Bounds<Pixels>>>,
    function: String,
    option: usize,
    selector: Option<Entity<SelectState<Vec<MappingChoice>>>>,
    selector_subscription: Option<Subscription>,
    functions_expanded: bool,
    outside_press: bool,
    can_save: bool,
    changed: bool,
    local_changed: bool,
    alert: bool,
}

#[derive(Clone)]
struct MappingChoice {
    choice: Choice,
    disabled: bool,
}

impl MappingChoice {
    fn title_for_locale(&self, locale: &str) -> SharedString {
        self.choice.title_for_locale(locale)
    }
}

impl SelectItem for MappingChoice {
    type Value = String;
    fn value(&self) -> &String {
        self.choice.value()
    }
    fn title(&self) -> SharedString {
        self.title_for_locale(&razer_i18n::locale())
    }
    fn disabled(&self) -> bool {
        self.disabled
    }
}

fn mapping_choices(row: &Product, firmware: &Value) -> Vec<MappingChoice> {
    row.configs["mapping_audio_options"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|option| {
            let id = option["id"].as_str()?;
            let key = option["content"].as_str()?;
            Some(MappingChoice {
                choice: Choice::localized_with_translations(
                    id,
                    key,
                    row.locales
                        .keys()
                        .map(|locale| (locale.clone(), message_for_locale(row, key, locale))),
                ),
                disabled: firmware_disabled(row, id, firmware),
            })
        })
        .collect()
}

fn footstep_animation(cx: &App) -> AnyElement {
    // Current MapAudio.8771f7ec mounts footsteps-scaling-animation.9ae7484f.
    // Its 15 literal SMIL tracks repeat every 5.5s. GPUI's SVG decoder cannot
    // execute SMIL, so the unified resource preparer samples those source
    // colors and motion paths at 60 frames/s without running vendor code.
    let image = div().w(surface::css(202.)).h(surface::css(105.));
    if cx.reduce_motion() {
        return image
            .child(img("synapse/audio-footstep-090.svg").size_full())
            .into_any_element();
    }
    image
        .with_animation(
            "sound-mapping-footstep-animation",
            Animation::new(std::time::Duration::from_millis(5500))
                .repeat()
                .with_max_fps(60.)
                .with_easing(linear),
            |image, phase| {
                let frame = ((phase * 330.).floor() as u32).min(329);
                image.child(
                    img(SharedString::from(format!(
                        "synapse/audio-footstep-{frame:03}.svg"
                    )))
                    .size_full(),
                )
            },
        )
        .into_any_element()
}

// Current shared ju/compareVersions: an absent firmware disables a gated
// function, segments compare numerically and missing segments are zero.
fn firmware_disabled(row: &Product, function: &str, current: &Value) -> bool {
    let Some(minimum) = row.configs["device_info"]["featureMinFw"][function]
        .as_str()
        .filter(|value| !value.is_empty())
    else {
        return false;
    };
    let Some(current) = current.as_str().filter(|value| !value.is_empty()) else {
        return true;
    };
    let parts = |value: &str| {
        value
            .split('.')
            .map(|value| value.parse::<f64>().unwrap_or(0.))
            .collect::<Vec<_>>()
    };
    let a = parts(current);
    let b = parts(minimum);
    for index in 0..a.len().max(b.len()) {
        let delta = a.get(index).copied().unwrap_or(0.) - b.get(index).copied().unwrap_or(0.);
        if delta != 0. {
            return delta < 0.;
        }
    }
    false
}

impl AudioProductWorkspace {
    pub(super) fn sound_mapping_overlays(&self, row: &Product, cx: &Context<Self>) -> AnyElement {
        div()
            .when(self.sound.mapping.open, |body| {
                body.child(self.current_sound_mapping_drawer(row, cx))
            })
            .when(self.sound.mapping.alert, |body| {
                body.child(self.current_sound_mapping_alert(row, cx))
            })
            .into_any_element()
    }
    pub(super) fn sound_mapping_button(&self, row: &Product) -> Value {
        let mut button = row.configs["customizer_groups"][0]["group"]["buttonList"][0].clone();
        if !button.is_object() {
            return Value::Null;
        }
        button["assignmentValue"] = button["defaultValue"].clone();
        button["isFocused"] = json!(self.sound.mapping.open);
        button["isMapped"] = json!(false);
        button["isEnabled"] = json!(true);
        button["isRequireSynapse"] = json!(false);
        let mapping = self.draft["sound"]["mappingList"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|mapping| {
                mapping["inputID"] == button["inputID"] && !js_truthy(&mapping["isHyperShift"])
            });
        match mapping.and_then(|mapping| mapping["outputType"].as_str().map(|kind| (mapping, kind)))
        {
            Some((_, "disableGroup")) => {
                button["isMapped"] = json!(true);
                button["assignment"] = json!("DISABLE");
                button["assignmentValue"] = json!("DISABLE");
            }
            Some((mapping, "audioGroup")) => {
                button["isMapped"] = json!(true);
                button["assignment"] = json!("AUDIO_FUNCTION");
                button["assignmentGroup"] = mapping["audioGroup"].clone();
                let assignment = &mapping["audioGroup"]["audioAssignment"];
                button["assignmentValue"] = row.configs["mapping_assignment_table"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|option| option["id"] == *assignment)
                    .map_or_else(|| assignment.clone(), |option| option["content"].clone());
            }
            // Current AUDIO's supported functions do not invent presentations
            // for unresolved foreign mappings.
            _ => {}
        }
        button
    }
    fn emit_sound_mapping_changed(&mut self, changed: bool, cx: &mut Context<Self>) {
        self.sound.mapping.changed = changed;
        let Some(row) = product(self.spec.product_id) else {
            return;
        };
        let types = row.configs["mapping_action_types"]["updateMappingChanged"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str);
        self.emit_sound(
            types
                .map(|kind| json!({"redux":{"type":kind,"payload":changed}}))
                .collect(),
            cx,
        );
    }
    pub(super) fn open_sound_mapping(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sound.mapping.open {
            return;
        }
        let Some(row) = product(self.spec.product_id) else {
            return;
        };
        let button = self.sound_mapping_button(row);
        if !js_truthy(&button["isEnabled"]) || js_truthy(&button["disabled"]) {
            return;
        }
        self.sound.mapping.open = true;
        let active = self.draft["sound"]["mappingList"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|mapping| mapping["inputID"] == button["inputID"])
            .cloned()
            .unwrap_or(json!({}));
        self.emit_sound_action(
            "setActiveButton",
            json!({"fromPanel":false,"buttonKey":button["buttonKey"]}),
            cx,
        );
        self.emit_sound_action("toggleMappingPanel", json!(true), cx);
        self.emit_sound_action("setActiveKeyMapping", active, cx);
        self.sound.mapping.function = if js_truthy(&button["isMapped"]) {
            button["assignment"]
                .as_str()
                .unwrap_or("DEFAULT")
                .to_string()
        } else {
            "DEFAULT".to_string()
        };
        self.initialize_sound_mapping_function(row, &button, window, cx);
        cx.notify();
    }
    fn initialize_sound_mapping_function(
        &mut self,
        row: &Product,
        button: &Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let function = self.sound.mapping.function.as_str();
        self.sound.mapping.can_save = (!js_truthy(&button["isMapped"]) && function != "DEFAULT")
            || (js_truthy(&button["isMapped"]) && button["assignment"].as_str() != Some(function));
        self.sound.mapping.local_changed = false;
        // A function switch unmounts MapAudio and its dropdown. Its retained
        // SelectState/subscription must have the same lifetime.
        self.sound.mapping.selector_subscription = None;
        self.sound.mapping.selector = None;
        self.emit_sound_mapping_changed(false, cx);
        if self.sound.mapping.function == "AUDIO_FUNCTION" {
            let options = row.configs["mapping_audio_options"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let found = options.iter().position(|option| {
                option["content"] == button["assignmentValue"]
                    || option["id"] == button["assignmentValue"]
            });
            self.sound.mapping.option = found.unwrap_or(0);
            if options
                .get(self.sound.mapping.option)
                .is_some_and(|option| {
                    firmware_disabled(
                        row,
                        option["id"].as_str().unwrap_or_default(),
                        &self.draft["sound"]["currentFWVersion"],
                    )
                })
            {
                self.sound.mapping.option = options
                    .iter()
                    .position(|option| {
                        !firmware_disabled(
                            row,
                            option["id"].as_str().unwrap_or_default(),
                            &self.draft["sound"]["currentFWVersion"],
                        )
                    })
                    .unwrap_or(0);
            }
            // Source te exits before enableSave when assignmentGroup is absent.
            if button.get("assignmentGroup").is_some() {
                self.sound.mapping.can_save = found.is_none();
            }
            let choices = mapping_choices(row, &self.draft["sound"]["currentFWVersion"]);
            let selected = choices
                .get(self.sound.mapping.option)
                .map(|choice| choice.value().clone());
            let selector = cx.new(|cx| SelectState::new(choices, None, window, cx));
            selector.update(cx, |state, cx| {
                if let Some(selected) = selected {
                    state.set_selected_value(&selected, window, cx);
                }
            });
            let subscription = cx.subscribe(&selector, |this, _, event, cx| {
                let SelectEvent::Confirm(Some(value)) = event else {
                    return;
                };
                let Some(row) = product(this.spec.product_id) else {
                    return;
                };
                if !this.sound.mapping.open
                    || this.sound.mapping.function != "AUDIO_FUNCTION"
                    || firmware_disabled(row, value, &this.draft["sound"]["currentFWVersion"])
                {
                    return;
                }
                let Some(index) =
                    row.configs["mapping_audio_options"]
                        .as_array()
                        .and_then(|options| {
                            options
                                .iter()
                                .position(|option| option["id"] == value.as_str())
                        })
                else {
                    return;
                };
                // MapAudio.selectOption marks a user confirmation dirty even
                // when it confirms the existing choice. State sync emits none.
                this.sound.mapping.option = index;
                this.sound.mapping.can_save = true;
                this.sound.mapping.local_changed = true;
                this.emit_sound_mapping_changed(true, cx);
                cx.notify();
            });
            self.sound.mapping.selector = Some(selector);
            self.sound.mapping.selector_subscription = Some(subscription);
        }
    }
    pub(super) fn refresh_sound_mapping(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.sound.mapping.open {
            return;
        }
        let Some(row) = product(self.spec.product_id) else {
            return;
        };
        let button = self.sound_mapping_button(row);
        self.initialize_sound_mapping_function(row, &button, window, cx);
    }
    fn select_sound_mapping_function(
        &mut self,
        function: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(row) = product(self.spec.product_id) else {
            return;
        };
        if firmware_disabled(row, &function, &self.draft["sound"]["currentFWVersion"]) {
            return;
        }
        let button = self.sound_mapping_button(row);
        self.sound.mapping.functions_expanded = false;
        self.sound.mapping.function = function;
        self.initialize_sound_mapping_function(row, &button, window, cx);
        cx.notify();
    }
    fn clear_sound_mapping(&mut self, cx: &mut Context<Self>) {
        self.emit_sound_action(
            "setActiveButton",
            json!({"fromPanel":false,"buttonKey":""}),
            cx,
        );
        self.emit_sound_action("toggleMappingPanel", json!(false), cx);
        self.sound.mapping.open = false;
        self.sound.mapping.functions_expanded = false;
        self.sound.mapping.outside_press = false;
        self.sound.mapping.can_save = false;
        self.sound.mapping.local_changed = false;
        self.sound.mapping.alert = false;
        self.sound.mapping.selector_subscription = None;
        self.sound.mapping.selector = None;
        self.emit_sound_mapping_changed(false, cx);
        cx.notify();
    }
    fn close_sound_mapping(&mut self, cancel: bool, cx: &mut Context<Self>) {
        if !cancel && self.sound.mapping.changed {
            self.sound.mapping.alert = true;
            cx.notify();
        } else {
            self.clear_sound_mapping(cx);
        }
    }
    fn save_sound_mapping(&mut self, cx: &mut Context<Self>) {
        if !self.sound.mapping.can_save {
            return;
        }
        let Some(row) = product(self.spec.product_id) else {
            return;
        };
        let button = self.sound_mapping_button(row);
        let mut mapping = json!({"outputType":"defaultGroup","isHyperShift":false,"inputType":button["inputType"],"inputID":button["inputID"]});
        match self.sound.mapping.function.as_str() {
            "DEFAULT" => mapping["isDefault"] = json!(true),
            "DISABLE" => mapping["outputType"] = json!("disableGroup"),
            "AUDIO_FUNCTION" => {
                let option = &row.configs["mapping_audio_options"][self.sound.mapping.option];
                if !option.is_object()
                    || firmware_disabled(
                        row,
                        option["id"].as_str().unwrap_or_default(),
                        &self.draft["sound"]["currentFWVersion"],
                    )
                {
                    return;
                }
                mapping["outputType"] = json!("audioGroup");
                mapping["audioGroup"] =
                    json!({"audioMode":option["id"],"audioAssignment":option["id"]});
            }
            _ => return,
        }
        let mut updates = vec![mapping];
        // MapDefault emits the second default record for a disabled primary.
        // Yt still filters using the active (normal) layer for both records.
        if self.sound.mapping.function == "DEFAULT"
            && matches!(
                button["assignment"].as_str(),
                Some("DISABLE" | "RAZER_HYPERSHIFT")
            )
        {
            let mut hyper = updates[0].clone();
            hyper["isHyperShift"] = json!(true);
            updates.push(hyper);
        }
        let mut mappings = self.draft["sound"]["mappingList"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        for mapping in updates {
            merge_sound_mapping(&mut mappings, &mapping);
            self.emit_sound_action("setActiveKeyMapping", mapping, cx);
        }
        let kind = &row.actions["setMappingList"];
        self.emit_sound(vec![json!({"redux":{"type":kind,"payload":mappings},"transport":{"type":"ON_SET_KEYMAPPING","payload":{"mappingList":mappings}},"sourceQueue":{"actionType":"ON_SET_KEYMAPPING","payload":{"mappingList":mappings}}})],cx);
        // This is the source reducer's optimistic local list, not a native ack.
        self.draft["sound"]["mappingList"] = json!(mappings);
        self.emit_sound_mapping_changed(false, cx);
        self.capture_sound_device();
        self.clear_sound_mapping(cx);
        cx.emit(AudioProductChanged);
    }
    pub(super) fn current_sound_mapping_drawer(
        &self,
        row: &Product,
        cx: &Context<Self>,
    ) -> AnyElement {
        let function = self.sound.mapping.function.clone();
        let button = self.sound_mapping_button(row);
        let options = row.configs["mapping_audio_options"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let selected = options.get(self.sound.mapping.option);
        let notes = selected
            .and_then(|option| option["id"].as_str())
            .and_then(|mode| row.configs["mapping_audio_notes"][mode].as_array())
            .cloned()
            .unwrap_or_default();
        let functions = button["functionList"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        // 5064 Yt + main.8657cac6.css: the 36px heading sits above the
        // 562px minimum body. This outer wrapper includes both hit regions.
        let content = v_flex().id("sound-mapping-popup").w(surface::css(292.))
            .on_mouse_down_out(cx.listener(|this, event: &MouseDownEvent, _, _| {
                this.sound.mapping.outside_press = event.button == MouseButton::Left
                    && !this.sound.mapping.anchor.get().contains(&event.position);
            }))
            .on_mouse_up_out(MouseButton::Left, cx.listener(|this, event: &MouseUpEvent, _, cx| {
                let outside = std::mem::take(&mut this.sound.mapping.outside_press);
                if outside && !this.sound.mapping.alert
                    && !this.sound.mapping.anchor.get().contains(&event.position) {
                    this.close_sound_mapping(false, cx);
                }
            }))
            .child(h_flex().relative().h(surface::css(36.)).flex_none()
                .justify_center().items_center().text_size(surface::css(14.))
                .text_color(Colors::secondary()).bg(Colors::mapping_panel())
                .border_1().border_color(Colors::border()).rounded_t(surface::css(5.))
                .child(div().max_w(surface::css(200.)).text_ellipsis()
                    .child(message(row,button["counter"].as_str().unwrap_or_default())))
                .child(gpui_kit::base::Button::new("sound-mapping-close")
                    .absolute().right_0().top_0().size(surface::css(36.))
                    .flex().justify_center().items_center()
                    .hover(|style| style.bg(Colors::mapping_control_hover()))
                    .active(|style| style.bg(Colors::mapping_control_pressed()))
                    .child(img("synapse/audio-sound-mapping-close.svg").size(surface::css(20.)))
                    .on_click(cx.listener(|this,_,_,cx|this.close_sound_mapping(false,cx)))))
            .child(div().relative().w_full().min_h(surface::css(562.))
                .bg(Colors::background()).border_1().border_t_0()
                .border_color(Colors::border()).rounded_b(surface::css(5.))
                .child(v_flex().ml(surface::css(40.)).w(surface::css(250.))
                    .p(surface::css(20.)).text_size(surface::css(14.))
                    .line_height(surface::css(17.)).text_color(Colors::text())
                    .child(div().mb(surface::css(20.)).font_family("RazerF5")
                        .text_size(surface::css(16.)).line_height(surface::css(19.))
                        .text_color(Colors::selected()).child(message(row,&function)))
                    .when(function=="DEFAULT",|body|body.child(div()
                        .w(surface::css(210.)).mb(surface::css(20.)).p(surface::css(5.))
                        .border_1().border_color(Colors::border()).text_center()
                        .child(message(row,button["defaultValue"].as_str().unwrap_or_default()))))
                    .when(function=="AUDIO_FUNCTION",|body|body.child(v_flex().mb(surface::css(30.))
                        // Current MapAudio extraStyle overrides the shared
                        // dropdown margin to 20px; the function body adds 30px.
                        .when_some(self.sound.mapping.selector.as_ref(), |body, selector| {
                            body.child(razer_widgets::synapse_select::select(selector)
                                .items(mapping_choices(row, &self.draft["sound"]["currentFWVersion"]))
                                .accessibility_label(message(row, "AUDIO_FUNCTION"))
                                .w(surface::css(210.)).mb(surface::css(20.)))
                        })
                        .children(notes.iter().filter_map(Value::as_str).map(|key|div().mb(surface::css(20.)).child(message(row,key))))
                        .when(selected.is_some_and(|option|option["id"]==json!("FootstepScaling")),|body|body.child(footstep_animation(cx)))))
                    .child(h_flex().gap(surface::css(10.)).text_size(surface::css(12.))
                        .child(mapping_action("sound-mapping-cancel", message(row,"CANCEL"), false)
                            .on_click(cx.listener(|this,_,_,cx|this.close_sound_mapping(true,cx))))
                        .child(mapping_action("sound-mapping-save", message(row,"SAVE"), true)
                            .disabled(!self.sound.mapping.can_save)
                            .when(!self.sound.mapping.can_save,|body|body.opacity(0.3))
                            // Original Save is mouseup, including release after a drag.
                            .on_mouse_up(MouseButton::Left,cx.listener(|this,_,_,cx|this.save_sound_mapping(cx)))
                            .on_click(cx.listener(|this,event,_,cx|{
                                if !matches!(event, ClickEvent::Mouse(_)) { this.save_sound_mapping(cx); }
                            }))))
                )
                .child(v_flex().id("sound-mapping-functions").absolute().left_0().top_0().h_full()
                    .w(surface::css(if self.sound.mapping.functions_expanded {250.} else {40.}))
                    .bg(Colors::mapping_panel()).rounded_bl(surface::css(5.)).overflow_hidden()
                    .on_hover(cx.listener(|this, hovered, _, cx|{
                        this.sound.mapping.functions_expanded=*hovered;
                        cx.notify();
                    }))
                    .children(functions.iter().filter_map(Value::as_str).map(|name|{
                        let name=name.to_string();
                        let current=function==name;
                        let disabled=firmware_disabled(row,&name,&self.draft["sound"]["currentFWVersion"]);
                        gpui_kit::base::Button::new(SharedString::from(format!("sound-mapping-function-{name}")))
                            .relative().w_full().h(surface::css(40.)).flex_none()
                            .text_size(surface::css(12.)).pl(surface::css(50.)).py(surface::css(13.))
                            .bg(if current {Colors::background()} else {Colors::mapping_panel()})
                            .text_color(if current {Colors::selected()} else {Colors::text()})
                            .disabled(disabled).when(disabled,|button|button.opacity(0.3))
                            .when(!disabled && !current,|button|button.hover(|style|style.bg(Colors::mapping_function_hover())))
                            .child(img(mapping_icon(&name,current)).absolute().left(surface::css(10.))
                                .top(surface::css(10.)).size(surface::css(20.)))
                            .when(self.sound.mapping.functions_expanded,|button|button.child(message(row,&name)))
                            .on_click(cx.listener(move|this,_,window,cx|this.select_sound_mapping_function(name.clone(),window,cx)))
                    })))
            );
        popup::place(
            content.into_any_element(),
            self.sound.mapping.root.clone(),
            self.sound.mapping.anchor.clone(),
        )
    }
    pub(super) fn current_sound_mapping_alert(
        &self,
        row: &Product,
        cx: &Context<Self>,
    ) -> AnyElement {
        deferred(
            div()
                .absolute()
                .inset_0()
                .bg(Colors::mapping_backdrop())
                .flex()
                .items_center()
                .justify_center()
                .child(
                    v_flex()
                        .relative()
                        .w(surface::css(400.))
                        .px(surface::css(30.))
                        .py(surface::css(20.))
                        .text_center()
                        .text_size(surface::css(14.))
                        .line_height(surface::css(17.))
                        .bg(Colors::background())
                        .border_1()
                        .border_color(Colors::selected())
                        .rounded(surface::css(5.))
                        .child(
                            gpui_kit::base::Button::new("sound-mapping-alert-close")
                                .absolute()
                                .top(surface::css(10.))
                                .right(surface::css(10.))
                                .child("×")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.sound.mapping.alert = false;
                                    this.sound.mapping.can_save = true;
                                    this.sound.mapping.local_changed = false;
                                    cx.notify();
                                })),
                        )
                        .child(
                            div()
                                .mb(surface::css(20.))
                                .font_family("RazerF5")
                                .text_size(surface::css(16.))
                                .line_height(surface::css(19.))
                                .text_color(Colors::selected())
                                .child(message(row, "SAVE_REMAPPED_BUTTON_HEADER")),
                        )
                        .child(
                            div()
                                .mb(surface::css(17.))
                                .child(message(row, "SAVE_REMAPPED_BUTTON_MSG1")),
                        )
                        .child(message(row, "SAVE_REMAPPED_BUTTON_MSG2"))
                        .child(
                            h_flex()
                                .mt(surface::css(20.))
                                .justify_center()
                                .gap(surface::css(10.))
                                .child(
                                    mapping_action(
                                        "sound-mapping-dont-save",
                                        message(row, "DONT_SAVE"),
                                        false,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.sound.mapping.alert = false;
                                            this.sound.mapping.can_save = true;
                                            this.sound.mapping.local_changed = false;
                                            this.emit_sound_mapping_changed(false, cx);
                                            cx.notify();
                                        },
                                    )),
                                )
                                .child(
                                    mapping_action(
                                        "sound-mapping-alert-save",
                                        message(row, "SAVE"),
                                        true,
                                    )
                                    .on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.sound.mapping.alert = false;
                                            this.save_sound_mapping(cx);
                                        },
                                    )),
                                ),
                        ),
                ),
        )
        .with_priority(1100)
        .into_any_element()
    }
}

fn mapping_icon(function: &str, active: bool) -> &'static str {
    match (function, active) {
        ("AUDIO_FUNCTION", false) => "synapse/audio-sound-mapping-audio.svg",
        ("AUDIO_FUNCTION", true) => "synapse/audio-sound-mapping-audio-active.svg",
        ("DISABLE", false) => "synapse/audio-sound-mapping-disable.svg",
        ("DISABLE", true) => "synapse/audio-sound-mapping-disable-active.svg",
        (_, false) => "synapse/audio-sound-mapping-default.svg",
        (_, true) => "synapse/audio-sound-mapping-default-active.svg",
    }
}

fn mapping_action(id: &'static str, label: String, primary: bool) -> gpui_kit::base::Button {
    gpui_kit::base::Button::new(id)
        .min_w(surface::css(100.))
        .h(surface::css(27.))
        .px(surface::css(10.))
        .flex()
        .items_center()
        .justify_center()
        .text_size(surface::css(12.))
        .border_1()
        .border_color(Colors::mapping_black())
        .rounded(surface::css(3.))
        .bg(if primary {
            Colors::selected()
        } else {
            Colors::mapping_cancel()
        })
        .text_color(if primary {
            Colors::mapping_black()
        } else {
            Colors::mapping_white()
        })
        .hover(|style| style.opacity(0.8))
        .active(|style| style.opacity(0.6))
        .child(label)
}

// Current 5064 Gt + Yt.saveChanges, normal layer used by all six Audio roots.
fn merge_sound_mapping(mappings: &mut Vec<Value>, mapping: &Value) {
    if js_truthy(&mapping["isDefault"]) {
        mappings.retain(|old| {
            old["inputID"] != mapping["inputID"]
                || (js_truthy(&old["isHyperShift"]) && old["outputType"] != "hyperShiftGroup")
        });
        return;
    }
    if let Some(index) = mappings.iter().position(|old| {
        old["inputID"] == mapping["inputID"] && old["isHyperShift"] == mapping["isHyperShift"]
    }) {
        let was_hyper = mappings[index]["outputType"] == "hyperShiftGroup";
        let paired = was_hyper
            .then(|| {
                mappings.iter().position(|old| {
                    old["inputID"] == mapping["inputID"] && js_truthy(&old["isHyperShift"])
                })
            })
            .flatten();
        mappings[index] = mapping.clone();
        if let Some(index) = paired {
            mappings.remove(index);
        }
    } else {
        mappings.push(mapping.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::{firmware_disabled, mapping_choices, merge_sound_mapping, product};
    use gpui_kit::component::select::SelectItem;
    use serde_json::{Value, json};

    // The glob imports above also carry GPUI's `test` macro; name the
    // built-in attribute so this remains an ordinary Rust test.
    use std::prelude::v1::test;
    #[test]
    fn retained_mapping_titles_follow_each_products_source_locale_without_changing_values() {
        for pid in [1398, 1401, 2638, 2641, 4124, 4126] {
            let row = product(pid).unwrap();
            let choices = mapping_choices(row, &Value::Null);
            assert!(!choices.is_empty());
            for choice in &choices {
                let id = choice.value().clone();
                let option = row.configs["mapping_audio_options"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|option| option["id"] == id)
                    .unwrap();
                let content_key = option["content"].as_str().unwrap();
                let disabled = choice.disabled;
                for locale in ["en", "zh-CN", "ja", "de"] {
                    assert_eq!(
                        choice.title_for_locale(locale).as_ref(),
                        row.locales[locale][content_key]
                    );
                }
                assert_eq!(
                    choice.title_for_locale("zh-cn"),
                    choice.title_for_locale("zh-CN")
                );
                assert_eq!(
                    choice.title_for_locale("unavailable-locale"),
                    choice.title_for_locale("en")
                );
                assert_eq!(choice.value(), &id);
                assert_eq!(choice.disabled, disabled);
            }
        }
    }

    #[test]
    fn firmware_gates_require_observed_versions_and_compare_numeric_segments() {
        for pid in [1398, 1401, 2638, 2641, 4124, 4126] {
            let row = product(pid).unwrap();
            assert!(firmware_disabled(row, "BluetoothVolume", &Value::Null));
            assert!(firmware_disabled(row, "DISABLE", &json!("1.0.0")));
            assert!(!firmware_disabled(
                row,
                "DISABLE",
                &row.configs["device_info"]["featureMinFw"]["DISABLE"]
            ));
            assert!(!firmware_disabled(row, "BluetoothVolume", &json!("1.10.0")));
            assert!(!firmware_disabled(row, "SidetoneLevel", &Value::Null));
        }
    }

    #[test]
    fn mapping_updates_do_not_overwrite_an_existing_other_layer() {
        let hyper =
            json!({"inputID":"KEY_SWITCH_AUDIO_EQ","isHyperShift":true,"outputType":"audioGroup"});
        let primary = json!({"inputID":"KEY_SWITCH_AUDIO_EQ","isHyperShift":false,"outputType":"disableGroup"});
        let mut mappings = vec![hyper.clone()];
        merge_sound_mapping(&mut mappings, &primary);
        assert_eq!(mappings, vec![hyper.clone(), primary]);
        merge_sound_mapping(
            &mut mappings,
            &json!({"inputID":"KEY_SWITCH_AUDIO_EQ","isHyperShift":false,"isDefault":true}),
        );
        assert_eq!(mappings, vec![hyper]);
    }

    #[test]
    fn leaving_hypershift_assignment_removes_its_paired_mapping_only() {
        let unrelated = json!({"inputID":"OTHER","isHyperShift":true,"outputType":"audioGroup"});
        let mut mappings = vec![
            json!({"inputID":"KEY_SWITCH_AUDIO_EQ","isHyperShift":true,"outputType":"audioGroup"}),
            json!({"inputID":"KEY_SWITCH_AUDIO_EQ","isHyperShift":false,"outputType":"hyperShiftGroup"}),
            unrelated.clone(),
        ];
        let replacement = json!({"inputID":"KEY_SWITCH_AUDIO_EQ","isHyperShift":false,"outputType":"disableGroup"});
        merge_sound_mapping(&mut mappings, &replacement);
        assert_eq!(mappings, vec![replacement, unrelated]);
    }
}
