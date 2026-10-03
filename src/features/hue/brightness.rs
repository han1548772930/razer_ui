use super::*;

impl HueWorkspace {
    pub(super) fn brightness_card(&self, cx: &Context<Self>) -> AnyElement {
        let bridge_enabled = self.bridge.controls_enabled();
        let enabled = self.editable_brightness();
        let mut content = v_flex()
            .when(!self.brightness.is_enabled, |v| v.opacity(0.3))
            .child(
                checkbox::Checkbox::new("hue-global-brightness")
                    .label(i18n::t("GLOBAL_BRIGHTNESS"))
                    .checked(self.brightness.global())
                    .disabled(!enabled)
                    .on_click(cx.listener(|this, _, _, cx| {
                        if this.editable_brightness() {
                            // CA renders undefined as global, but onCheck negates
                            // the raw optional field. Its first click materializes
                            // true; only the next click selects per-light controls.
                            this.brightness.is_global_brightness =
                                Some(!this.brightness.is_global_brightness.unwrap_or(false));
                            this.changed(cx);
                        }
                    })),
            );
        if self.brightness.global() {
            content = content.child(div().mt(surface::css(20.)).child(brightness_slider(
                &self.global_brightness,
                enabled,
                cx,
            )));
        } else {
            let (connected, _, offline) = self.bridge.counts();
            content = content
                .child(
                    div()
                        .mt(surface::css(20.))
                        .mb(surface::css(10.))
                        .child(format!(
                            "{connected}\u{a0}{};\u{a0}{offline}\u{a0}{}",
                            i18n::t("CONNECTED").to_lowercase(),
                            i18n::t("UNREACHABLE").to_lowercase()
                        )),
                )
                .child(
                    h_flex()
                        .items_start()
                        .flex_wrap()
                        .gap(surface::css(10.))
                        .children(self.bridge.devices.iter().filter_map(|light| {
                            let slider = self.light_brightness.get(&light.region_id)?;
                            Some(
                                v_flex()
                                    .id(SharedString::from(format!(
                                        "hue-light-brightness-{}",
                                        light.device_container_id
                                    )))
                                    .w(surface::css(255.))
                                    .p(surface::css(10.))
                                    .border_1()
                                    .border_color(cx.theme().transparent)
                                    .hover(|s| {
                                        s.bg(Colors::light_hover())
                                            .border_color(cx.theme().title_bar)
                                    })
                                    // The mounted NA omits the supplied handleSelectDevice.
                                    // No checkbox/selection handler is present in this source.
                                    .child(
                                        h_flex()
                                            .gap(surface::css(5.))
                                            .child(light_icon(&light.raw_data.physical_arche_type))
                                            .child(light.raw_data.physical_name.clone()),
                                    )
                                    .child(div().mt(surface::css(5.)).child(brightness_slider(
                                        slider,
                                        enabled && light.is_on,
                                        cx,
                                    ))),
                            )
                        })),
                );
        }
        widget(
            "BRIGHTNESS",
            Some("BRIGHTNESS_TOOLTIP"),
            surface::SynapseSwitch::new("hue-brightness-enable")
                .checked(self.brightness.is_enabled)
                .accessibility_label(i18n::t("BRIGHTNESS"))
                .disabled(!bridge_enabled)
                .on_change(cx.listener(|this, value, _, cx| {
                    if this.bridge.controls_enabled() {
                        this.brightness.is_enabled = *value;
                        this.changed(cx);
                    }
                })),
            cx,
        )
        .when(!bridge_enabled, |v| v.opacity(0.3))
        .child(content)
        .into_any_element()
    }
    pub(super) fn devices_card(&self, cx: &Context<Self>) -> AnyElement {
        let (connected, channels, unreachable) = self.bridge.counts();
        widget("DEVICE", Some("DEVICES_TIP"), div(), cx)
            .when(!self.bridge.controls_enabled(), |v| v.opacity(0.3))
            .child(div().mb(surface::css(24.)).child(format!(
                "{connected}\u{a0}{};\u{a0}{channels}\u{a0}{};\u{a0}{unreachable}\u{a0}{}",
                text("CONNECTED"),
                text("CHANNELS"),
                text("UNREACHABLE")
            )))
            .children(self.bridge.devices.iter().map(|light| {
                h_flex()
                    .id(SharedString::from(format!(
                        "hue-device-{}",
                        light.device_container_id
                    )))
                    .gap(surface::css(10.))
                    .mt(surface::css(8.))
                    .child(light_icon(&light.raw_data.physical_arche_type))
                    .child(
                        div()
                            .when(!light.is_on, |v| v.opacity(0.3))
                            .child(light.product_name.clone()),
                    )
                    .child(light.name.clone())
            }))
            .into_any_element()
    }
    pub(super) fn rebuild_lights(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.bridge.deduplicate_lights();
        self.light_subscriptions.clear();
        let regions = self
            .bridge
            .devices
            .iter()
            .map(|l| l.region_id)
            .collect::<BTreeSet<_>>();
        self.light_brightness
            .retain(|region, _| regions.contains(region));
        for region in regions {
            let initial = self.port_brightness(region);
            let slider = self
                .light_brightness
                .entry(region)
                .or_insert_with(|| {
                    cx.new(|_| {
                        SliderState::new()
                            .min(0.)
                            .max(100.)
                            .step(1.)
                            .default_value(initial)
                    })
                })
                .clone();
            self.light_subscriptions.push(cx.subscribe_in(
                &slider,
                window,
                move |this, _, event, _, cx| {
                    if !this.editable_brightness()
                        || this.brightness.global()
                        || !this
                            .bridge
                            .devices
                            .iter()
                            .any(|l| l.region_id == region && l.is_on)
                    {
                        return;
                    }
                    if let SliderEvent::Change(value) = event {
                        if !this.draft.get("ports").is_some_and(Value::is_array) {
                            this.draft["ports"] = json!([]);
                        }
                        let ports = this.draft["ports"].as_array_mut().unwrap();
                        if !ports
                            .iter()
                            .any(|p| p.get("id").and_then(Value::as_u64) == Some(region as u64))
                        {
                            ports.push(json!({"id":region}));
                        }
                        let port = ports
                            .iter_mut()
                            .find(|p| p.get("id").and_then(Value::as_u64) == Some(region as u64))
                            .unwrap();
                        port["brightness"] = json!({"value":value.start().round().clamp(0.,100.)});
                        this.changed(cx);
                    }
                },
            ));
        }
        self.sync_brightness(window, cx);
    }
}
fn brightness_slider(slider: &Entity<SliderState>, enabled: bool, cx: &App) -> AnyElement {
    v_flex()
        .gap(surface::css(5.))
        .child(div().text_center().child(format!(
            "{}",
            slider.read(cx).value().start().round() as i32
        )))
        .child(Slider::new(slider).disabled(!enabled))
        .child(
            h_flex()
                .justify_between()
                .text_size(surface::css(12.))
                .text_color(cx.theme().muted_foreground)
                .child(i18n::t("OFF"))
                .child(i18n::t("BRIGHT")),
        )
        .into_any_element()
}
