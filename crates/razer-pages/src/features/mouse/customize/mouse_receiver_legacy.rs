//! The mounted PID158/192/194 VD widget, its native radios and literal SMIL.
//! Current original owners/CSS: unified mouse-receiver-legacy query.
use super::*;
use gpui_kit::base::{
    Radio,
    motion::{self, Easing, Transition},
};

#[derive(Deserialize)]
pub(super) struct LegacySource {
    product_id: u32,
    title: String,
    description: String,
    disabled_tooltip: String,
    options: Vec<OptionSource>,
    modes: Vec<ImageSource>,
}
#[derive(Deserialize)]
struct OptionSource {
    mode: u8,
    id: String,
    label: String,
    description: String,
}
#[derive(Deserialize)]
struct ImageSource {
    mode: u8,
    base: String,
    layers: Vec<LayerSource>,
}
#[derive(Deserialize)]
struct LayerSource {
    image: String,
    initial_opacity: f32,
    animation: AnimationSource,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AnimationSource {
    id: Option<String>,
    values: String,
    begin: String,
    dur: String,
    repeat_count: Option<String>,
}
pub(super) fn source(product_id: u32) -> Option<&'static LegacySource> {
    static DATA: OnceLock<Vec<LegacySource>> = OnceLock::new();
    DATA.get_or_init(|| read_mouse_receiver_legacy_catalog())
        .iter()
        .find(|source| source.product_id == product_id)
}
fn image_mode(status: &Value) -> u8 {
    match status.as_f64() {
        Some(2.) => 2,
        Some(3.) => 3,
        _ => 1,
    }
}
fn visible(product: u32, connection: Option<PollingConnection>, display: &Value) -> bool {
    match product {
        158 => connection == Some(PollingConnection::Dongle),
        192 => connection == Some(PollingConnection::Dongle) && display != "macro",
        194 => true,
        _ => false,
    }
}
impl MouseProductWorkspace {
    pub fn observe_legacy_receiver_pairing(
        &mut self,
        kind: &str,
        payload: &Value,
        cx: &mut Context<Self>,
    ) {
        if self.spec.product_id != 192 {
            return;
        }
        // VA.componentDidMount handles the original response directly. In
        // particular, Unbind filters the existing raw dongle IDs; it does not
        // issue an additional getter or replace state from a diagnostic query.
        match kind {
            "DUALLINK_BIND_INFO" => {
                self.mouse_customize_observation["pairedInfo"] = payload.clone();
            }
            "DUALLINK_UNBIND_DEVICE" => {
                if let Some(paired) = self.mouse_customize_observation["pairedInfo"].as_array_mut()
                {
                    paired.retain(|row| row.get("dongleId") != payload.get("productId"));
                }
            }
            _ => return,
        }
        cx.notify();
    }
    pub(super) fn legacy_receiver_visible(&self) -> bool {
        visible(
            self.spec.product_id,
            self.polling_state.connection(),
            &self.mouse_customize_observation["displayMode"],
        )
    }
    fn legacy_receiver_paired(&self) -> bool {
        self.spec.product_id == 192
            && self.mouse_customize_observation["pairedInfo"]
                .as_array()
                .is_some_and(|paired| !paired.is_empty())
    }
    fn legacy_receiver_disabled(&self) -> bool {
        self.legacy_receiver_paired()
            || self.spec.product_id == 194
                && self.polling_state.connection() != Some(PollingConnection::Dongle)
    }
    pub(super) fn sync_legacy_receiver(&mut self, cx: &mut Context<Self>) {
        let scope = self.mouse_polling_scope(cx).filter(|_| {
            self.polling_state.connection() == Some(PollingConnection::Dongle)
                || self.spec.product_id == 192
                    && self.polling_state.connection() == Some(PollingConnection::Wired)
        });
        if self
            .receiver_indicator
            .monitor
            .as_ref()
            .is_some_and(|monitor| scope.is_none_or(|scope| !monitor.matches_scope(scope)))
        {
            self.receiver_indicator.cancel();
        }
        let Some(scope) = scope else {
            return;
        };
        if self.receiver_indicator.monitor.is_none() {
            let monitor = MouseReceiverMonitorRequested {
                scope,
                valid: Arc::new(AtomicBool::new(true)),
            };
            self.receiver_indicator.monitor = Some(monitor.clone());
            cx.emit(monitor);
            self.receiver_indicator.legacy_binding_requested = false;
        }
        self.request_legacy_binding_info(cx);
        if self.polling_state.connection() != Some(PollingConnection::Dongle) {
            return;
        }
        if self.receiver_indicator.initialized == Some(scope) {
            return;
        }
        self.receiver_indicator.initialized = Some(scope);
        self.enqueue_receiver_indicator(MouseReceiverAction::LegacyInitialize, scope, cx);
    }
    pub(super) fn request_legacy_binding_info(&mut self, cx: &mut Context<Self>) {
        // PID192's mounted TopView asks DUALLINK_BIND_INFO independently of
        // the indicator visibility, profile initialization and macro mode.
        if self.spec.product_id != 192
            || self.page != "TAB_CUSTOMIZE"
            || self.receiver_indicator.legacy_binding_requested
        {
            return;
        }
        let Some(scope) = self.mouse_polling_scope(cx).filter(|scope| {
            self.receiver_indicator
                .monitor
                .as_ref()
                .is_some_and(|monitor| monitor.matches_scope(*scope))
        }) else {
            return;
        };
        self.receiver_indicator.legacy_binding_requested = true;
        self.enqueue_receiver_indicator(MouseReceiverAction::LegacyBindingInfo, scope, cx);
    }
    pub(super) fn restore_legacy_receiver(&mut self, cx: &mut Context<Self>) {
        // Profile reload's actual switch publishes only fields present in G.
        // The hardware getter belongs to setupDevice, not each profile reload.
        let live = self
            .receiver_indicator
            .monitor
            .as_ref()
            .is_some_and(|monitor| {
                self.mouse_polling_scope(cx)
                    .is_some_and(|scope| monitor.matches_scope(scope))
            });
        if !live {
            self.sync_legacy_receiver(cx);
            return;
        }
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        if self.polling_state.connection() != Some(PollingConnection::Dongle) {
            return;
        }
        self.receiver_indicator.initialized = Some(scope);
        if let Some(status) = self.draft.get("indicatorLedStatus").cloned() {
            self.publish_legacy_receiver_status(status.clone(), cx);
            self.enqueue_receiver_indicator(
                MouseReceiverAction::LegacySet {
                    status,
                    persist: false,
                },
                scope,
                cx,
            );
        }
    }
    pub(super) fn publish_legacy_receiver_status(&mut self, status: Value, cx: &mut Context<Self>) {
        if image_mode(&status) != image_mode(&self.receiver_indicator.legacy_status) {
            self.receiver_indicator.legacy_image_epoch =
                self.receiver_indicator.legacy_image_epoch.wrapping_add(1);
        }
        self.receiver_indicator.legacy_status = status.clone();
        self.draft["indicatorLedStatus"] = status;
        cx.notify();
    }
    fn choose_legacy_receiver_status(&mut self, mode: u8, cx: &mut Context<Self>) {
        if !self.active
            || self.page != "TAB_CUSTOMIZE"
            || !self.legacy_receiver_visible()
            || self.legacy_receiver_disabled()
        {
            return;
        }
        let Some(scope) = self.mouse_polling_scope(cx) else {
            return;
        };
        let status = json!(mode);
        self.publish_legacy_receiver_status(status.clone(), cx);
        cx.emit(MouseProductChanged);
        self.enqueue_receiver_indicator(
            MouseReceiverAction::LegacySet {
                status,
                persist: true,
            },
            scope,
            cx,
        );
    }
    pub(super) fn legacy_receiver_panel(&self, cx: &Context<Self>) -> AnyElement {
        let Some(spec) = source(self.spec.product_id).filter(|_| self.legacy_receiver_visible())
        else {
            return div().into_any_element();
        };
        let disabled = self.legacy_receiver_disabled();
        let paired = self.legacy_receiver_paired();
        let whole_disabled = disabled && self.spec.product_id == 194;
        let status = &self.receiver_indicator.legacy_status;
        let selected_image = spec
            .modes
            .iter()
            .find(|mode| mode.mode == image_mode(status))
            .expect("source connection-status SVG fallback");
        let options = v_flex()
            .gap(surface::css(10.))
            .w(surface::css(310.))
            .opacity(if paired { 0.3 } else { 1. })
            .children(spec.options.iter().map(|option| {
                let mode = option.mode;
                let selected = status.as_f64() == Some(f64::from(mode));
                let listener = cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.choose_legacy_receiver_status(mode, cx)
                });
                v_flex()
                    .w(surface::css(310.))
                    .h(surface::css(if mode == 2 { 64. } else { 50. }))
                    .child(LegacyRadio {
                        id: SharedString::from(format!(
                            "receiver-{}-{}",
                            spec.product_id, option.id
                        ))
                        .into(),
                        checked: selected,
                        base: Radio::new(SharedString::from(format!(
                            "receiver-{}-{}",
                            spec.product_id, option.id
                        )))
                        .checked(selected)
                        .disabled(disabled)
                        .accessibility_label(t(&option.label))
                        .flex()
                        .items_center()
                        .gap(surface::css(10.))
                        .text_size(surface::css(14.))
                        .line_height(surface::css(20.))
                        .text_color(Colors::text())
                        .on_change(move |_, event, window, cx| listener(event, window, cx)),
                        label: t(&option.label).to_uppercase(),
                    })
                    .child(
                        div()
                            .ml(surface::css(30.))
                            .w(surface::css(280.))
                            .text_size(surface::css(12.))
                            .text_color(Colors::legacy_message())
                            .opacity(0.7)
                            .child(t(&option.description)),
                    )
            }));
        surface::panel(t(&spec.title), cx)
            .id("mouse-receiver-legacy-panel")
            .relative()
            .w(surface::css(600.))
            .mx_auto()
            .font_family("Roboto")
            .text_color(Colors::text())
            .opacity(if whole_disabled { 0.3 } else { 1. })
            .child(
                v_flex()
                    .gap(surface::css(20.))
                    .w(surface::css(520.))
                    .h(surface::css(227.))
                    .child(
                        div()
                            .text_size(surface::css(14.))
                            .child(t(&spec.description)),
                    )
                    .child(
                        v_flex().gap(surface::css(10.)).child(
                            h_flex()
                                .id(("mouse-receiver-legacy-options", spec.product_id))
                                .my(surface::css(5.))
                                .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
                                    this.receiver_indicator.legacy_tip_hovered = paired && *hovered;
                                    cx.notify();
                                }))
                                .child(options)
                                .child(LegacyImage {
                                    source: selected_image,
                                    id: SharedString::from(format!(
                                        "receiver-image-{}-{}-{}",
                                        spec.product_id,
                                        selected_image.mode,
                                        self.receiver_indicator.legacy_image_epoch
                                    ))
                                    .into(),
                                }),
                        ),
                    ),
            )
            .child(LegacyTooltip {
                open: paired && self.receiver_indicator.legacy_tip_hovered,
                text: t(&spec.disabled_tooltip),
            })
            .into_any_element()
    }
}
#[derive(IntoElement)]
struct LegacyRadio {
    id: ElementId,
    checked: bool,
    base: Radio,
    label: String,
}
impl RenderOnce for LegacyRadio {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        // Current global [type=radio]+label: 20px ring, 10px dot, .2s ease.
        let value = motion::transition(
            (self.id, "checked"),
            if self.checked { 1_f32 } else { 0. },
            Transition::new(Duration::from_millis(200)).easing(Easing::Ease),
            window,
            cx,
        );
        self.base
            .child(
                div()
                    .relative()
                    .size(surface::css(20.))
                    .flex_shrink_0()
                    .rounded_full()
                    .border_1()
                    .border_color(Colors::legacy_radio_border())
                    .child(
                        div()
                            .absolute()
                            .left(surface::css(10. - 5. * value))
                            .top(surface::css(10. - 5. * value))
                            .size(surface::css(10. * value))
                            .rounded_full()
                            .bg(Colors::accent())
                            .opacity(value),
                    ),
            )
            .child(self.label)
    }
}
#[derive(IntoElement)]
struct LegacyTooltip {
    open: bool,
    text: String,
}
impl RenderOnce for LegacyTooltip {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let opacity = motion::transition(
            "receiver-legacy-tooltip-opacity",
            if self.open { 1_f32 } else { 0. },
            Transition::new(Duration::from_millis(300)).easing(Easing::Linear),
            window,
            cx,
        );
        div().child(
            razer_assets::deferred_resource(
                div()
                    .absolute()
                    .left(surface::css(100.))
                    .top(surface::css(100.))
                    .w(surface::css(290.))
                    .px(surface::css(10.))
                    .py(surface::css(8.))
                    .border_1()
                    .border_color(Colors::tip_border())
                    .bg(Colors::tip_surface())
                    .text_color(Colors::text())
                    .text_size(surface::css(14.))
                    .line_height(surface::css(16.))
                    // Current [indicator-tooltip]:before changes visibility with
                    // zero delay; opacity alone leaves a hidden mouse hitbox.
                    .when(!self.open, |tooltip| tooltip.invisible())
                    .opacity(opacity)
                    .child(self.text),
            )
            .with_priority(106),
        )
    }
}
#[derive(IntoElement)]
struct LegacyImage {
    source: &'static ImageSource,
    id: ElementId,
}
fn seconds(value: &str) -> f64 {
    value
        .strip_suffix('s')
        .expect("literal SMIL seconds")
        .parse()
        .expect("SMIL seconds number")
}
fn interpolate(values: &str, progress: f64) -> f32 {
    let values = values
        .split(';')
        .map(|value| value.parse::<f32>().expect("literal SMIL opacity"))
        .collect::<Vec<_>>();
    let position = progress.clamp(0., 1.) * (values.len() - 1) as f64;
    let index = (position.floor() as usize).min(values.len() - 2);
    values[index] + (values[index + 1] - values[index]) * (position - index as f64) as f32
}
impl ImageSource {
    fn opacity(&self, index: usize, elapsed: f64) -> f32 {
        let layer = &self.layers[index];
        let animation = &layer.animation;
        let duration = seconds(&animation.dur);
        if animation.repeat_count.as_deref() == Some("indefinite") {
            let begin = seconds(&animation.begin);
            return if elapsed < begin {
                layer.initial_opacity
            } else {
                interpolate(
                    &animation.values,
                    (elapsed - begin).rem_euclid(duration) / duration,
                )
            };
        }
        // The source warning is one cyclic event chain, not three independent
        // repeatCount loops. Before a track begins its underlying opacity wins.
        let mut offset = 0.;
        let mut period = 0.;
        for (position, track) in self.layers.iter().enumerate() {
            let predecessor =
                &self.layers[(position + self.layers.len() - 1) % self.layers.len()].animation;
            let expected = format!(
                "{}.end",
                predecessor.id.as_deref().expect("named SMIL event")
            );
            assert_eq!(
                track.animation.begin,
                if position == 0 {
                    format!("0s;{expected}")
                } else {
                    expected
                }
            );
            if position < index {
                offset += seconds(&track.animation.dur);
            }
            period += seconds(&track.animation.dur);
        }
        let time = elapsed.rem_euclid(period) - offset;
        if time < 0. || time >= duration {
            layer.initial_opacity
        } else {
            interpolate(&animation.values, time / duration)
        }
    }
}
impl RenderOnce for LegacyImage {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let now = cx.background_executor().now();
        let start = window.use_keyed_state((self.id, "smil-start"), cx, |_, _| now);
        let elapsed = now.saturating_duration_since(*start.read(cx)).as_secs_f64();
        let animated = !cx.reduce_motion();
        if animated && !self.source.layers.is_empty() {
            window.request_animation_frame();
        }
        div()
            .relative()
            .w(surface::css(200.))
            .h(surface::css(192.))
            .flex_shrink_0()
            .child(
                img(self.source.base.clone())
                    .w(surface::css(243.))
                    .h(surface::css(187.)),
            )
            .children(self.source.layers.iter().enumerate().map(|(index, layer)| {
                img(layer.image.clone())
                    .absolute()
                    .left_0()
                    .top_0()
                    .w(surface::css(243.))
                    .h(surface::css(187.))
                    .opacity(if animated {
                        self.source.opacity(index, elapsed)
                    } else {
                        layer.initial_opacity
                    })
            }))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[::core::prelude::v1::test]
    fn receiver_mounts_follow_each_actual_product_branch() {
        assert!(!visible(158, None, &Value::Null));
        assert!(visible(
            158,
            Some(PollingConnection::Dongle),
            &json!("macro")
        ));
        assert!(!visible(
            192,
            Some(PollingConnection::Dongle),
            &json!("macro")
        ));
        assert!(visible(194, Some(PollingConnection::Wired), &Value::Null));
        assert!(!visible(211, Some(PollingConnection::Dongle), &Value::Null));
        assert!(!visible(222, Some(PollingConnection::Dongle), &Value::Null));
        assert_eq!(image_mode(&json!("2")), 1);
        assert_eq!(image_mode(&json!(2.0)), 2);
        assert_eq!(image_mode(&json!(3.0)), 3);
        assert_eq!(image_mode(&json!(255)), 1);
    }
    #[::core::prelude::v1::test]
    fn receiver_smil_preserves_initial_delays_and_event_chain() {
        for product in [158, 192, 194] {
            let source = source(product).unwrap();
            let battery = source.modes.iter().find(|mode| mode.mode == 2).unwrap();
            assert_eq!(battery.opacity(1, 2.49), 0.);
            assert!((battery.opacity(1, 2.575) - 0.5).abs() < 0.001);
            assert_eq!(battery.opacity(2, 4.), 0.);
            let warning = source.modes.iter().find(|mode| mode.mode == 3).unwrap();
            assert!((warning.opacity(0, 0.15) - 1.).abs() < 0.001);
            assert_eq!(warning.opacity(1, 0.15), 0.);
            assert!((warning.opacity(1, 0.45) - 1.).abs() < 0.001);
            assert_eq!(warning.opacity(0, 0.45), 0.);
            assert!((warning.opacity(2, 1.6) - 1.).abs() < 0.001);
            assert!((warning.opacity(0, 2.75) - 1.).abs() < 0.001);
        }
    }
}

// Generated catalog reader: razer-pages/mouse_receiver_legacy_data.
fn read_mouse_receiver_legacy_catalog() -> Vec<LegacySource> {
    fn decode_0(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<LegacySource>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_1)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<LegacySource> {
        let value = match r.tag()? {
            0 => LegacySource {
                product_id: r.child(decode_2)?,
                title: r.child(decode_3)?,
                description: r.child(decode_3)?,
                disabled_tooltip: r.child(decode_3)?,
                options: r.child(decode_4)?,
                modes: r.child(decode_7)?,
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
    ) -> anyhow::Result<std::vec::Vec<OptionSource>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_5)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_5(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<OptionSource> {
        let value = match r.tag()? {
            0 => OptionSource {
                mode: r.child(decode_6)?,
                id: r.child(decode_3)?,
                label: r.child(decode_3)?,
                description: r.child(decode_3)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_6(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<u8> {
        let value = match r.tag()? {
            0 => r.u8()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_7(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<ImageSource>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_8)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_8(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<ImageSource> {
        let value = match r.tag()? {
            0 => ImageSource {
                mode: r.child(decode_6)?,
                base: r.child(decode_3)?,
                layers: r.child(decode_9)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_9(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<LayerSource>> {
        let value = match r.tag()? {
            0 => Default::default(),
            1 => r.sequence(decode_10)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_10(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<LayerSource> {
        let value = match r.tag()? {
            0 => LayerSource {
                image: r.child(decode_3)?,
                initial_opacity: r.child(decode_11)?,
                animation: r.child(decode_12)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_11(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<f32> {
        let value = match r.tag()? {
            0 => r.f32()?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_12(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<AnimationSource> {
        let value = match r.tag()? {
            0 => AnimationSource {
                id: None,
                values: r.child(decode_3)?,
                begin: r.child(decode_3)?,
                dur: r.child(decode_3)?,
                repeat_count: r.child(decode_13)?,
            },
            1 => AnimationSource {
                id: r.child(decode_13)?,
                values: r.child(decode_3)?,
                begin: r.child(decode_3)?,
                dur: r.child(decode_3)?,
                repeat_count: None,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_13(
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
        razer_resource_db::catalogs::RAZER_PAGES_MOUSE_RECEIVER_LEGACY_DATA,
        0,
        "0b121d8512a0b8a3ec34ac5c357a97ad393ee8f68c34a187b98eb3b08de395ea",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod mouse_receiver_legacy_data_catalog_tests {
    #[test]
    fn packaged_catalog_mouse_receiver_legacy_data_decodes() {
        let _ = super::read_mouse_receiver_legacy_catalog();
    }
}
// End generated catalog reader: razer-pages/mouse_receiver_legacy_data.
