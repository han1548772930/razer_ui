use super::*;
use crate::ui::source_slider::SourceSlider;

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
/// 源 `OT`（`.slider-container`）的亮度滑条。当前 769 的两个亮度滑条都是
/// `<OT min={0} max={100} step={1} value={…} active={…} minTag={w.KFn} maxTag={w.zrT}/>`
/// （`KFn`/`zrT` 按 769 的导出表就是 `OFF`/`BRIGHT`），没有 `noTip`，所以值显示在
/// `.slider-tip` 里；`active` 对应 `.slider-container.on`（否则 `.3` 透明度 +
/// `pointer-events:none`）。绘制交给共享 `SourceSlider`（同一 `.slider-container`、
/// `.track`、`.left`、`.slider-tip` 与滑柄 hover/active 配色），这里只补源容器下沿的
/// `.foot`：`.slider-container .foot{bottom:-2px;opacity:1;position:absolute;
/// text-transform:uppercase;…}` + `.foot.min{left:0}` + `.foot.max{right:0}`。
/// 原版 `.foot` 不声明颜色与字号，由父级继承，所以这里用主题前景色。
fn brightness_slider(slider: &Entity<SliderState>, enabled: bool, cx: &App) -> AnyElement {
    let value = slider.read(cx).value().start().round().clamp(0., 100.);
    let progress = (value / 100.).clamp(0., 1.);
    div()
        .relative()
        .w_full()
        .child(
            SourceSlider::new(slider, progress)
                .tip(Some(format!("{value:.0}")))
                .enabled(enabled),
        )
        .child(
            div()
                .absolute()
                .bottom(surface::css(-2.))
                .w_full()
                .flex()
                .justify_between()
                .text_size(surface::css(14.))
                .text_color(cx.theme().foreground)
                // `.slider-container .foot{text-transform:uppercase}`；容器未 `.on`
                // 时整块是 `.3`，而 `.foot` 在这里是同级节点，所以单独降透明度。
                .when(!enabled, |row| row.opacity(0.3))
                .child(i18n::t("OFF").to_uppercase())
                .child(i18n::t("BRIGHT").to_uppercase()),
        )
        .into_any_element()
}
