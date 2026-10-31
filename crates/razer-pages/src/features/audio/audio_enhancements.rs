//! Actual mounted current Enhancements page. Draft mutations are local Redux
//! equivalents; source transport requests remain distinct from device receipts.
use super::*;
use razer_widgets::theme::AudioEnhancementColors as Colors;

#[derive(Deserialize)]
struct Product {
    product_id: u32,
    enhancement: Enhancement,
    left: Left,
    locales: BTreeMap<String, BTreeMap<String, String>>,
}
#[derive(Deserialize)]
struct Enhancement {
    title: String,
    tip: String,
    options: Vec<EnhancementOption>,
    min_tag: String,
    max_tag: String,
    queue_type: String,
}
#[derive(Deserialize)]
struct EnhancementOption {
    key: String,
    name: String,
    description: String,
    sub_description: Option<String>,
    action: String,
    transport: Transport,
}
#[derive(Deserialize)]
struct Transport {
    #[serde(rename = "type")]
    kind: String,
    timer_tick: bool,
}
#[derive(Deserialize)]
struct Left {
    ultra_low_latency: LowLatency,
    noise_cancellation: Option<NoiseCancellation>,
}
#[derive(Deserialize)]
struct LowLatency {
    title: String,
    tip: String,
    header: String,
    description: String,
    descriptions: BTreeMap<String, String>,
    icon: String,
    options: Vec<Mode>,
    initial: Value,
    action: String,
    transport: Transport,
}
#[derive(Deserialize)]
struct NoiseCancellation {
    title: String,
    tip: String,
    options: Vec<Mode>,
    levels: BTreeMap<String, Vec<u8>>,
    initial: Value,
    action: String,
    transport: Transport,
    queue_type: String,
}
#[derive(Deserialize)]
struct Mode {
    value: Value,
    name: String,
}
fn product(pid: u32) -> Option<&'static Product> {
    static DATA: OnceLock<Vec<Product>> = OnceLock::new();
    DATA.get_or_init(|| read_audio_enhancements_catalog())
        .iter()
        .find(|p| p.product_id == pid)
}
fn message(row: &Product, key: &str) -> String {
    row.locales
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(&razer_i18n::locale()))
        .and_then(|(_, values)| values.get(key))
        .or_else(|| row.locales.get("en").and_then(|values| values.get(key)))
        .cloned()
        .unwrap_or_default()
}
#[derive(Default)]
pub(super) struct State {
    current: String,
    observed: BTreeMap<String, Value>,
    preview_source: BTreeMap<String, Value>,
    retained_device: Option<Value>,
}
impl State {
    fn mount(&mut self, draft: &Value) {
        self.current = "soundNormalization".into();
        self.observed.clear();
        self.preview_source.clear();
        // Original componentDidMount performs three independent checks in
        // this order, so Voice Clarity wins if several source props are enabled.
        self.refresh(draft);
    }
    fn refresh(&mut self, draft: &Value) {
        for key in ["bassBoost", "soundNormalization", "voiceClarity"] {
            let Some(value) = draft.pointer(&format!("/profile/{key}")) else {
                continue;
            };
            if self.observed.get(key) != Some(value) {
                if value["isEnabled"].as_bool() == Some(true) {
                    self.current = key.into();
                }
                self.observed.insert(key.into(), value.clone());
            }
        }
    }
}
fn transport(
    action: &str,
    descriptor: &Transport,
    redux_payload: Value,
    native_payload: Value,
) -> Value {
    let mut native = json!({"type":descriptor.kind,"payload":native_payload});
    if descriptor.timer_tick {
        let tick = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| duration.as_millis() as u64);
        native["timerTick"] = json!(tick);
    }
    json!({"redux":{"type":action,"payload":redux_payload},"transport":native})
}
impl AudioProductWorkspace {
    pub(super) fn initialize_enhancements(&mut self) {
        let Some(row) = product(self.spec.product_id) else {
            return;
        };
        self.draft["device"]["ultraLowLatency"] = row.left.ultra_low_latency.initial.clone();
        if let Some(anc) = &row.left.noise_cancellation {
            self.draft["device"]["noiseCancellation"] = anc.initial.clone();
        }
        self.enhancements.mount(&self.draft);
    }
    pub(super) fn refresh_enhancements(&mut self) {
        if product(self.spec.product_id).is_some() {
            self.enhancements.refresh(&self.draft);
        }
    }
    pub(super) fn remount_enhancements(&mut self) {
        if product(self.spec.product_id).is_some() {
            self.enhancements.mount(&self.draft);
        }
    }
    fn capture_enhancement_device(&mut self) {
        if product(self.spec.product_id).is_some() {
            self.enhancements.retained_device = Some(json!({
                "ultraLowLatency":self.draft["device"]["ultraLowLatency"],
                "noiseCancellation":self.draft["device"].get("noiseCancellation"),
            }));
        }
    }
    pub(super) fn restore_enhancement_device(&mut self) {
        let Some(row) = product(self.spec.product_id) else {
            return;
        };
        // These source reducers are device-wide and survive profile changes.
        // The first restoration reads local state; later restorations retain
        // this physical owner's state rather than another profile's snapshot.
        if let Some(state) = &self.enhancements.retained_device {
            self.draft["device"]["ultraLowLatency"] = state["ultraLowLatency"].clone();
            if row.left.noise_cancellation.is_some() {
                self.draft["device"]["noiseCancellation"] = state["noiseCancellation"].clone();
            }
        }
        self.capture_enhancement_device();
        self.remount_enhancements();
    }
    pub(super) fn preserves_enhancement_preview(&self, path: &str) -> bool {
        self.enhancements
            .preview_source
            .get(path)
            .is_some_and(|old| self.draft.pointer(path) == Some(old))
    }
    fn emit_enhancements(&self, messages: Vec<Value>, cx: &mut Context<Self>) {
        if let Some(owner) = &self.source_owner {
            owner.update(cx, |owner, cx| {
                owner.request_audio_enhancements(messages, cx)
            });
        }
    }
    fn submit_enhancement(
        &mut self,
        key: &str,
        payload: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(option) = product(self.spec.product_id)
            .and_then(|p| p.enhancement.options.iter().find(|o| o.key == key))
        else {
            return;
        };
        self.draft["profile"][key] = payload.clone();
        self.emit_enhancements(
            vec![transport(
                &option.action,
                &option.transport,
                payload.clone(),
                json!({key:payload}),
            )],
            cx,
        );
        self.sync(window, cx);
        cx.emit(AudioProductChanged);
        cx.notify();
    }
    fn select_enhancement(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        let old = self.enhancements.current.clone();
        if self.draft["profile"][&old]["isEnabled"] != json!(true) {
            return;
        }
        self.enhancements.current = key.into();
        let mut old_value = self.draft["profile"][&old].clone();
        old_value["isEnabled"] = json!(false);
        // Preserve original handleChangeSelect: old disable before new enable.
        self.submit_enhancement(&old, old_value, window, cx);
        let mut new_value = self.draft["profile"][key].clone();
        new_value["isEnabled"] = json!(true);
        self.submit_enhancement(key, new_value, window, cx);
    }
    pub(super) fn enhancement_slider_event(
        &mut self,
        path: &str,
        event: &SliderEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(row) = product(self.spec.product_id) else {
            return false;
        };
        let Some(option) = row
            .enhancement
            .options
            .iter()
            .find(|o| path == format!("/profile/{}/value", o.key))
        else {
            return false;
        };
        let SliderEvent::Release(value) = event else {
            if let Some(value) = self.draft.pointer(path) {
                self.enhancements
                    .preview_source
                    .insert(path.into(), value.clone());
            }
            cx.notify();
            return true;
        };
        self.enhancements.preview_source.remove(path);
        let value = value.start();
        let key = option.key.clone();
        let mut payload = self.draft["profile"][&key].clone();
        payload["value"] = normalized(value, 10., 100., 1.);
        payload["isEnabled"] = json!(true);
        let mut queued = payload.clone();
        queued["label"] = json!(key);
        self.emit_enhancements(
            vec![json!({"sourceQueue":{"type":row.enhancement.queue_type,"payload":queued}})],
            cx,
        );
        self.submit_enhancement(&key, payload, window, cx);
        true
    }
    pub(crate) fn observe_enhancements(
        &mut self,
        key: &str,
        payload: Value,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(row) = product(self.spec.product_id) else {
            return false;
        };
        if row
            .enhancement
            .options
            .iter()
            .any(|option| option.key == key)
        {
            // Original updateBassBoost/updateSoundNormalization/updateVoiceClarity
            // replaces the corresponding reducer object; no write is emitted.
            self.draft["profile"][key] = payload;
        } else if key == "noiseCancellation" && row.left.noise_cancellation.is_some() {
            let Some(fields) = payload.as_object() else {
                return false;
            };
            for (key, value) in fields {
                self.draft["device"]["noiseCancellation"][key] = value.clone();
            }
        } else {
            return false;
        }
        self.capture_enhancement_device();
        self.sync(window, cx);
        cx.emit(AudioProductChanged);
        cx.notify();
        true
    }
    fn set_low_latency(&mut self, value: Value, cx: &mut Context<Self>) {
        let Some(row) = product(self.spec.product_id) else {
            return;
        };
        let setting = &row.left.ultra_low_latency;
        if !setting.options.iter().any(|o| o.value == value) {
            return;
        }
        let state = &mut self.draft["device"]["ultraLowLatency"];
        state["mode"] = value.clone();
        state["isEnabled"] = json!(value != json!("alwaysOff"));
        let native = state.clone();
        self.emit_enhancements(
            vec![transport(
                &setting.action,
                &setting.transport,
                json!({"mode":value}),
                native,
            )],
            cx,
        );
        self.capture_enhancement_device();
        cx.emit(AudioProductChanged);
        cx.notify();
    }
    fn set_noise_cancellation(&mut self, payload: Value, cx: &mut Context<Self>) {
        let Some(setting) =
            product(self.spec.product_id).and_then(|p| p.left.noise_cancellation.as_ref())
        else {
            return;
        };
        let state = &mut self.draft["device"]["noiseCancellation"];
        for (key, value) in payload.as_object().into_iter().flatten() {
            state[key] = value.clone();
        }
        // Original reducer includes level only in the ANC branch, and preserves
        // the underlying level when changing to Ambient.
        let native = if state["status"] == setting.options[0].value {
            state.clone()
        } else {
            json!({"isEnabled":state["isEnabled"],"status":state["status"]})
        };
        self.emit_enhancements(
            vec![json!({"sourceQueue":{"type":setting.queue_type,"payload":payload}})],
            cx,
        );
        self.emit_enhancements(
            vec![transport(
                &setting.action,
                &setting.transport,
                payload,
                native,
            )],
            cx,
        );
        self.capture_enhancement_device();
        cx.emit(AudioProductChanged);
        cx.notify();
    }
    fn mode_tabs(
        &self,
        modes: &[Mode],
        current: &Value,
        enabled: bool,
        low_latency: bool,
        row: &Product,
        cx: &Context<Self>,
    ) -> AnyElement {
        h_flex()
            .self_start()
            .h(surface::css(36.))
            .p(surface::css(4.))
            .rounded(surface::css(18.))
            .border_1()
            .border_color(Colors::border())
            .bg(Colors::background())
            .when(enabled, |tabs| {
                tabs.hover(|style| style.border_color(Colors::selected()))
            })
            .children(modes.iter().map(|mode| {
                let value = mode.value.clone();
                let selected = current == &value;
                razer_widgets::keyboard_navigation::Button::new(SharedString::from(format!(
                    "audio-enhancements-mode-{low_latency}-{value}"
                )))
                .accessibility_label(message(row, &mode.name))
                .disabled(!enabled)
                .h_full()
                .min_w(surface::css(36.))
                .px(surface::css(10.))
                .py(surface::css(5.))
                .rounded(surface::css(13.))
                .mr(surface::css(
                    if modes.last().is_some_and(|last| last.value == value) {
                        0.
                    } else {
                        5.
                    },
                ))
                .flex()
                .items_center()
                .justify_center()
                .text_size(surface::css(14.))
                .bg(if selected {
                    Colors::selected()
                } else {
                    Colors::background()
                })
                .text_color(if selected {
                    Colors::background()
                } else {
                    Colors::text()
                })
                .child(message(row, &mode.name))
                .on_click(cx.listener(move |this, _, _, cx| {
                    if low_latency {
                        this.set_low_latency(value.clone(), cx)
                    } else {
                        this.set_noise_cancellation(json!({"status":value}), cx)
                    }
                }))
            }))
            .into_any_element()
    }
    fn low_latency_card(&self, row: &Product, cx: &Context<Self>) -> AnyElement {
        let setting = &row.left.ultra_low_latency;
        let mode = &self.draft["device"]["ultraLowLatency"]["mode"];
        // Original hidden grid sizers reserve the longest of the three mode
        // descriptions; visible content shares exactly the same grid area.
        let descriptions = div()
            .grid()
            .grid_cols(1)
            .text_color(Colors::secondary())
            .children(setting.options.iter().map(|option| {
                v_flex()
                    .row_start(1)
                    .col_start(1)
                    .gap(surface::css(10.))
                    .invisible()
                    .child(message(row, &setting.description))
                    .when_some(
                        option
                            .value
                            .as_str()
                            .and_then(|k| setting.descriptions.get(k)),
                        |body, text| body.child(message(row, text)),
                    )
            }))
            .child(
                v_flex()
                    .row_start(1)
                    .col_start(1)
                    .gap(surface::css(10.))
                    .child(message(row, &setting.description))
                    .when_some(
                        mode.as_str().and_then(|k| setting.descriptions.get(k)),
                        |body, text| body.child(message(row, text)),
                    ),
            );
        surface::panel_with_help(
            message(row, &setting.title),
            surface::help_control("audio-low-latency-tip", message(row, &setting.tip)),
            cx,
        )
        .child(
            h_flex()
                .gap(surface::css(10.))
                .mb(surface::css(20.))
                .child(
                    img(setting.icon.clone())
                        .size(surface::css(40.))
                        .flex_shrink_0()
                        .object_fit(ObjectFit::Contain),
                )
                .child(message(row, &setting.header)),
        )
        .child(div().mb(surface::css(20.)).child(self.mode_tabs(
            &setting.options,
            mode,
            true,
            true,
            row,
            cx,
        )))
        .child(descriptions)
        .into_any_element()
    }
    fn noise_cancellation_card(&self, row: &Product, cx: &Context<Self>) -> Option<AnyElement> {
        let setting = row.left.noise_cancellation.as_ref()?;
        let state = &self.draft["device"]["noiseCancellation"];
        let enabled = state["isEnabled"] == json!(true);
        let status = &state["status"];
        let level = state
            .get("level")
            .filter(|v| !v.is_null())
            .and_then(Value::as_u64)
            .or_else(|| {
                setting
                    .levels
                    .get(&setting.options[0].value.to_string())
                    .and_then(|l| l.first())
                    .copied()
                    .map(u64::from)
            });
        let owner = cx.entity();
        let switch = surface::SynapseSwitch::new("audio-anc-switch")
            .checked(enabled)
            .accessibility_label(message(row, &setting.title))
            .on_change(move |value, _, cx| {
                owner.update(cx, |this, cx| {
                    this.set_noise_cancellation(json!({"isEnabled":value}), cx)
                });
            });
        Some(
            surface::panel_with_title_switch(
                message(row, &setting.title),
                switch,
                surface::help_control("audio-anc-tip", message(row, &setting.tip)),
                cx,
            )
            .child(
                v_flex()
                    .when(!enabled, |body| body.opacity(0.3))
                    .child(self.mode_tabs(&setting.options, status, enabled, false, row, cx))
                    .child(
                        h_flex().gap(surface::css(10.)).children(
                            setting
                                .levels
                                .get(&status.to_string())
                                .into_iter()
                                .flatten()
                                .map(|value| {
                                    let value = *value;
                                    let selected = level == Some(u64::from(value));
                                    razer_widgets::keyboard_navigation::Button::new(SharedString::from(format!(
                                        "audio-anc-level-{value}"
                                    )))
                                    .accessibility_label(value.to_string())
                                    .disabled(!enabled || selected)
                                    .mt(surface::css(20.))
                                    .h(surface::css(27.))
                                    .min_w(surface::css(90.))
                                    .rounded(surface::css(3.))
                                    .border_1()
                                    .border_color(if selected {
                                        Colors::selected()
                                    } else {
                                        Colors::border()
                                    })
                                    .bg(if selected {
                                        Colors::tab_active()
                                    } else {
                                        Colors::background()
                                    })
                                    .text_size(surface::css(12.))
                                    .line_height(surface::css(14.))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .when(enabled && !selected, |button| {
                                        button.hover(|style| style.border_color(Colors::text()))
                                    })
                                    .child(value.to_string())
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.set_noise_cancellation(json!({"level":value}), cx)
                                    }))
                                }),
                        ),
                    ),
            )
            .into_any_element(),
        )
    }
    fn enhancement_card(&self, row: &Product, cx: &Context<Self>) -> AnyElement {
        let setting = &row.enhancement;
        let Some(option) = setting
            .options
            .iter()
            .find(|o| o.key == self.enhancements.current)
        else {
            return div().into_any_element();
        };
        let key = option.key.clone();
        let enabled = self.draft["profile"][&key]["isEnabled"] == json!(true);
        let owner = cx.entity();
        let switch = surface::SynapseSwitch::new("audio-enhancement-switch")
            .checked(enabled)
            .accessibility_label(message(row, &setting.title))
            .on_change(move |value, window, cx| {
                owner.update(cx, |this, cx| {
                    let mut payload = this.draft["profile"][&key].clone();
                    payload["isEnabled"] = json!(value);
                    this.submit_enhancement(&key, payload, window, cx);
                });
            });
        surface::panel_with_title_switch(
            message(row, &setting.title),
            switch,
            surface::help_control("audio-enhancement-tip", message(row, &setting.tip)),
            cx,
        )
        .child(
            v_flex()
                .child(
                    h_flex()
                        .when(!enabled, |body| body.opacity(0.3))
                        .mt(surface::css(20.))
                        .gap(surface::css(10.))
                        .children(setting.options.iter().map(|tab| {
                            let key = tab.key.clone();
                            let selected = self.enhancements.current == key;
                            razer_widgets::keyboard_navigation::Button::new(SharedString::from(format!(
                                "audio-enhancement-tab-{key}"
                            )))
                            .accessibility_label(message(row, &tab.name))
                            .disabled(!enabled)
                            .flex_1()
                            .flex()
                            .items_center()
                            .justify_center()
                            .py(surface::css(13.))
                            .px(surface::css(8.))
                            .border_1()
                            .border_color(if selected {
                                Colors::selected()
                            } else {
                                Colors::border()
                            })
                            .rounded(surface::css(3.))
                            .bg(if selected {
                                Colors::tab_active()
                            } else {
                                Colors::background()
                            })
                            .text_size(surface::css(12.))
                            .text_center()
                            .child(message(row, &tab.name).to_uppercase())
                            .on_click(cx.listener(
                                move |this, _, window, cx| {
                                    this.select_enhancement(&key, window, cx)
                                },
                            ))
                        })),
                )
                .child(
                    v_flex()
                        .when(!enabled, |body| body.opacity(0.3))
                        .mt(surface::css(20.))
                        .mb(surface::css(30.))
                        .child(message(row, &option.description))
                        .when_some(option.sub_description.as_ref(), |desc, key| {
                            desc.child(
                                div()
                                    .text_color(Colors::secondary())
                                    .child(message(row, key)),
                            )
                        }),
                )
                .when_some(
                    self.sliders.get(&format!("/profile/{}/value", option.key)),
                    |body, state| {
                        let value = state.read(cx).value().start();
                        body.child(
                            div()
                                .relative()
                                .capture_key_down(|event, _, cx| {
                                    if matches!(
                                        event.keystroke.key.as_str(),
                                        "home" | "end" | "left" | "right" | "up" | "down"
                                    ) {
                                        cx.stop_propagation();
                                    }
                                })
                                .child(
                                    razer_widgets::source_slider::SourceSlider::new(
                                        state,
                                        (value - 10.) / 90.,
                                    )
                                    .tip(Some(format!("{value:.0}")))
                                    .enabled(enabled),
                                )
                                .child(
                                    div()
                                        .absolute()
                                        .bottom_0()
                                        .w_full()
                                        .when(!enabled, |tags| tags.opacity(0.3))
                                        .child(surface::slider_tags(
                                            &message(row, &setting.min_tag),
                                            None,
                                            &message(row, &setting.max_tag),
                                            None,
                                        )),
                                ),
                        )
                    },
                ),
        )
        .into_any_element()
    }
    pub(super) fn enhancements_page(&self, key: &str, cx: &Context<Self>) -> Option<AnyElement> {
        let row = product(self.spec.product_id)?;
        let owner = self
            .source_owner
            .as_ref()
            .filter(|owner| owner.read(cx).smart_switch_page() == Some(key))?;
        let simultaneous =
            super::super::source_help_owner::SourceHelpOwner::smart_switch_card(owner, cx)?;
        Some(
            surface::page_columns()
                .gap_0()
                .child(surface::page_column(
                    v_flex()
                        .child(self.low_latency_card(row, cx))
                        .children(self.noise_cancellation_card(row, cx)),
                ))
                .child(surface::page_column(
                    v_flex()
                        .child(simultaneous)
                        .child(self.enhancement_card(row, cx)),
                ))
                .into_any_element(),
        )
    }
}

// Generated catalog reader: razer-pages/audio_enhancements_data.
fn read_audio_enhancements_catalog() -> Vec<Product> {
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
                enhancement: r.child(decode_3)?,
                left: r.child(decode_10)?,
                locales: r.child(decode_21)?,
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
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Enhancement> {
        let value = match r.tag()? {
            0 => Enhancement {
                title: r.child(decode_4)?,
                tip: r.child(decode_4)?,
                options: r.child(decode_5)?,
                min_tag: r.child(decode_4)?,
                max_tag: r.child(decode_4)?,
                queue_type: r.child(decode_4)?,
            },
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
    fn decode_5(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<EnhancementOption>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_6)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<EnhancementOption> {
        let value = match r.tag()? {
            0 => EnhancementOption {
                key: r.child(decode_4)?,
                name: r.child(decode_4)?,
                description: r.child(decode_4)?,
                sub_description: None,
                action: r.child(decode_4)?,
                transport: r.child(decode_7)?,
            },
            1 => EnhancementOption {
                key: r.child(decode_4)?,
                name: r.child(decode_4)?,
                description: r.child(decode_4)?,
                sub_description: r.child(decode_9)?,
                action: r.child(decode_4)?,
                transport: r.child(decode_7)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Transport> {
        let value = match r.tag()? {
            0 => Transport {
                kind: r.child(decode_4)?,
                timer_tick: r.child(decode_8)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_8(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<bool> {
        let value = match r.tag()? {
            0 => r.boolean()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_9(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<String>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_4)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_10(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Left> {
        let value = match r.tag()? {
            0 => Left {
                ultra_low_latency: r.child(decode_11)?,
                noise_cancellation: r.child(decode_16)?,
            },
            1 => Left {
                ultra_low_latency: r.child(decode_11)?,
                noise_cancellation: None,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_11(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<LowLatency> {
        let value = match r.tag()? {
            0 => LowLatency {
                title: r.child(decode_4)?,
                tip: r.child(decode_4)?,
                header: r.child(decode_4)?,
                description: r.child(decode_4)?,
                descriptions: r.child(decode_12)?,
                icon: r.child(decode_4)?,
                options: r.child(decode_13)?,
                initial: r.child(decode_15)?,
                action: r.child(decode_4)?,
                transport: r.child(decode_7)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_12(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, String>> {
        let value = match r.tag()? {
            0 => r.map(decode_4, decode_4)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_13(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<std::vec::Vec<Mode>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_14)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_14(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<Mode> {
        let value = match r.tag()? {
            0 => Mode {
                value: r.child(decode_15)?,
                name: r.child(decode_4)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_15(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<serde_json::Value> {
        let value = match r.tag()? {
            0 => r.external_value()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_16(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::option::Option<NoiseCancellation>> {
        let value = match r.tag()? {
            0 => Some(r.child(decode_17)?),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_17(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<NoiseCancellation> {
        let value = match r.tag()? {
            0 => NoiseCancellation {
                title: r.child(decode_4)?,
                tip: r.child(decode_4)?,
                options: r.child(decode_13)?,
                levels: r.child(decode_18)?,
                initial: r.child(decode_15)?,
                action: r.child(decode_4)?,
                transport: r.child(decode_7)?,
                queue_type: r.child(decode_4)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_18(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::collections::BTreeMap<String, std::vec::Vec<u8>>> {
        let value = match r.tag()? {
            0 => r.map(decode_4, decode_19)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_19(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<std::vec::Vec<u8>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_20)?,
            1 => Default::default(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_20(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u8> {
        let value = match r.tag()? {
            0 => r.u8()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_21(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<
        std::collections::BTreeMap<String, std::collections::BTreeMap<String, String>>,
    > {
        let value = match r.tag()? {
            0 => r.map(decode_4, decode_12)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_PAGES_AUDIO_ENHANCEMENTS_DATA,
        0,
        "b5a486b9323b32c15fad37fdedb2984835d9cb308c697d1eeb5efac617733d3a",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod audio_enhancements_data_catalog_tests {
    #[test]
    fn packaged_catalog_audio_enhancements_data_decodes() {
        let _ = super::read_audio_enhancements_catalog();
    }
}
// End generated catalog reader: razer-pages/audio_enhancements_data.
