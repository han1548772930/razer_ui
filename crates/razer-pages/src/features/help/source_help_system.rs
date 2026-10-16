//! Actual basic and image System Details Help mounts.
use super::*;
#[derive(Clone, Copy, Debug)]
pub enum HelpSystemInfoCommand {
    Observe,
    SaveLogs,
}
#[derive(Clone, Debug)]
pub struct HelpSystemInfoRequested {
    owner: EntityId,
    generation: u64,
    product_id: u32,
    page_offset: usize,
    edition_id: u32,
    layout_id: u32,
    command: HelpSystemInfoCommand,
}
impl HelpSystemInfoRequested {
    pub fn product_id(&self) -> u32 {
        self.product_id
    }
    pub fn page_offset(&self) -> usize {
        self.page_offset
    }
    pub fn owner(&self) -> EntityId {
        self.owner
    }
    pub fn command(&self) -> HelpSystemInfoCommand {
        self.command
    }
    pub fn edition_id(&self) -> u32 {
        self.edition_id
    }
    pub fn layout_id(&self) -> u32 {
        self.layout_id
    }
    pub fn middleware_message(&self, timer_tick: u64) -> Option<serde_json::Value> {
        matches!(self.command, HelpSystemInfoCommand::SaveLogs).then(|| serde_json::json!({"type":"ON_STORE_SYSTEM_INFO_DATA","timerTick":timer_tick,"payload":null}))
    }
}
pub(super) struct State {
    owner: EntityId,
    generation: u64,
    source: serde_json::Value,
    expanded: bool,
    toggle_generation: u64,
}
impl State {
    pub(super) fn new(owner: EntityId) -> Self {
        Self {
            owner,
            generation: 0,
            source: serde_json::json!({}),
            expanded: false,
            toggle_generation: 0,
        }
    }
    pub(super) fn unmount(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.source = serde_json::json!({});
        self.expanded = false;
        self.toggle_generation = 0;
    }
}
impl EventEmitter<HelpSystemInfoRequested> for SourceHelp {}
impl SourceHelp {
    pub(super) fn system_info_request(
        &self,
        command: HelpSystemInfoCommand,
    ) -> Option<HelpSystemInfoRequested> {
        let (_, page) = self.metadata()?;
        (page.system_info
            && (matches!(self.device.product_id, 694 | 695 | 696)
                || image_variant(self.device.product_id)))
        .then(|| HelpSystemInfoRequested {
            owner: self.system_info.owner,
            generation: self.system_info.generation,
            product_id: self.device.product_id,
            page_offset: page.offset,
            edition_id: self.device.edition_id,
            layout_id: self.device.layout_id,
            command,
        })
    }
    pub(in crate::features) fn accepts_system_info_request(
        &self,
        request: &HelpSystemInfoRequested,
    ) -> bool {
        self.system_info_request(HelpSystemInfoCommand::Observe)
            .is_some_and(|current| {
                current.owner == request.owner
                    && current.generation == request.generation
                    && current.product_id == request.product_id
                    && current.page_offset == request.page_offset
            })
    }
    pub(super) fn mount_system_info(&mut self, cx: &mut Context<Self>) {
        self.system_info.unmount();
        if let Some(request) = self.system_info_request(HelpSystemInfoCommand::Observe) {
            cx.emit(request);
        }
    }
    pub(in crate::features) fn observe_system_info(
        &mut self,
        request: &HelpSystemInfoRequested,
        source: serde_json::Value,
        cx: &mut Context<Self>,
    ) -> bool {
        if !source.is_object() || !self.accepts_system_info_request(request) {
            return false;
        }
        self.system_info.source = source;
        cx.notify();
        true
    }
    fn system_info_arrow(&self) -> AnyElement {
        let icon = svg()
            .path("synapse/help-system-expand.svg")
            .w(surface::css(10.))
            .h(surface::css(5.))
            .text_color(razer_widgets::theme::HelpSystemInfoColors::arrow());
        let expanded = self.system_info.expanded;
        let icon = if self.system_info.toggle_generation == 0 {
            icon.with_transformation(Transformation::rotate(radians(
                270. * std::f32::consts::PI / 180.,
            )))
            .into_any_element()
        } else {
            icon.with_animation(
                SharedString::from(format!(
                    "help-system-expand-{}",
                    self.system_info.toggle_generation
                )),
                Animation::new(Duration::from_millis(300)).with_easing(camo::source_ease),
                move |icon, phase| {
                    let angle = if expanded {
                        270. + 90. * phase
                    } else {
                        360. - 90. * phase
                    };
                    icon.with_transformation(Transformation::rotate(radians(
                        angle * std::f32::consts::PI / 180.,
                    )))
                },
            )
            .into_any_element()
        };
        div()
            .w(surface::css(10.))
            .h(surface::css(15.))
            .mr(surface::css(5.))
            .flex()
            .items_center()
            .child(icon)
            .into_any_element()
    }
    pub(super) fn system_info_panel(&self, exact: bool, cx: &Context<Self>) -> AnyElement {
        if image_variant(self.device.product_id) {
            return self.system_image_panel(exact, cx);
        }
        let mut info = v_flex().my(surface::css(10.)).child(
            BaseButton::new("source-help-system-expand")
                .p_0()
                .self_start()
                .child(
                    h_flex()
                        .child(self.system_info_arrow())
                        .child(div().underline().child(i18n::t("VIEW_SYSTEM_INFO"))),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.system_info.expanded = !this.system_info.expanded;
                    this.system_info.toggle_generation =
                        this.system_info.toggle_generation.wrapping_add(1);
                    cx.notify();
                })),
        );
        if self.system_info.expanded {
            let source = &self.system_info.source;
            let mut data = v_flex().mt(surface::css(8.));
            let fields = [
                ("DEVICE_NAME", "DeviceName", ""),
                ("PROCESSOR", "CPU", ""),
                ("INSTALLED_RAM", "RAM", ""),
                ("SERIAL_NUMBER", "SerialNumber", ""),
                ("MODEL_NUMBER", "ModelName", ""),
                ("OS_VERSION", "OSVersion", ""),
                ("DISPLAY_1", "", ""),
                ("DISPLAY_2", "", ""),
                ("NVIDIA_GEFORCE_EXPERIENCE", "NVIDIAGeForceExperience", ""),
                ("GPU_MODE_NVIDIA_CONTROL_PANEL", "NVIDIAControlPanel", ""),
                ("COMMAND_CENTER", "IntelGraphicCommandCenter", ""),
                ("AMD_ADRENALINE", "AMDAdrenalinEdition", "Not Installed"),
                ("CORTEX", "RazerCortex", "Not Installed"),
                ("AXON", "RazerAxon", "Not Installed"),
            ];
            for (index, (label, key, fallback)) in fields.into_iter().enumerate() {
                let value = if matches!(index, 6 | 7) {
                    source
                        .get("Display")
                        .and_then(|display| display.get(index - 6))
                        .and_then(|display| display.get("Name"))
                        .filter(|value| js_truthy(value))
                        .map(js_string)
                        .unwrap_or_default()
                } else {
                    source
                        .get(key)
                        .map(js_string)
                        .unwrap_or_else(|| fallback.into())
                };
                data = data.child(format!("{}: {}", i18n::t(label), value));
            }
            info = info.child(data);
        }
        help_panel("SYSTEM_DETAILS", exact, cx)
            .child(
                div()
                    .my(surface::css(10.))
                    .child(i18n::t("SYSTEM_DETAILS_MSG")),
            )
            .child(info)
            .child(
                BaseButton::new("source-help-system-download")
                    .p_0()
                    .self_start()
                    .my(surface::css(10.))
                    .child(
                        h_flex()
                            .child(
                                img("synapse/help-system-download.svg")
                                    .size(surface::css(20.))
                                    .mr(surface::css(5.))
                                    .mt(surface::css(7.)),
                            )
                            .child(div().underline().child(i18n::t("SAVE_FULL_SYSTEM_LOGS"))),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(request) =
                            this.system_info_request(HelpSystemInfoCommand::SaveLogs)
                        {
                            cx.emit(request);
                        }
                    })),
            )
            .into_any_element()
    }
    fn system_image_panel(&self, exact: bool, cx: &Context<Self>) -> AnyElement {
        let source = &self.system_info.source;
        let mut top = v_flex().gap(surface::css(15.));
        for (label, key, vertical) in [
            (None, "DeviceName", false),
            (Some("PROCESSOR"), "CPU", false),
            (Some("GRAPHICS"), "GPU", false),
            (Some("INSTALLED_RAM"), "RAM", true),
        ] {
            let content = source.get(key).map(js_string).unwrap_or_default();
            let mut line = v_flex();
            if vertical {
                line = line.child(format!("{}: {content}", i18n::t(label.unwrap())));
            } else {
                line = line
                    .child(
                        div().min_h(surface::css(17.)).child(
                            label
                                .map(|label| format!("{}:", i18n::t(label)))
                                .unwrap_or_default(),
                        ),
                    )
                    .child(content);
            }
            top = top.child(line);
        }
        let mut info = v_flex().my(surface::css(10.)).child(
            BaseButton::new("source-help-system-expand")
                .p_0()
                .self_start()
                .child(
                    h_flex()
                        .child(self.system_info_arrow())
                        .child(i18n::t("VIEW_SYSTEM_INFO").to_uppercase()),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.system_info.expanded = !this.system_info.expanded;
                    this.system_info.toggle_generation =
                        this.system_info.toggle_generation.wrapping_add(1);
                    cx.notify();
                })),
        );
        if self.system_info.expanded {
            let mut data = v_flex().mt(surface::css(8.)).child(format!(
                "OS Version: {}",
                source.get("OSVersion").map(js_string).unwrap_or_default()
            ));
            if let Some(displays) = source.get("Display").and_then(|value| value.as_array()) {
                for (index, display) in displays.iter().enumerate() {
                    data = data.child(format!(
                        "Display {}: {}",
                        index + 1,
                        display
                            .get("Name")
                            .filter(|value| js_truthy(value))
                            .map(js_string)
                            .unwrap_or_default()
                    ));
                }
            }
            let missing = i18n::t("NOT_INSTALLED");
            let ge = source
                .get("NVIDIAGeForceExperience")
                .map(js_string)
                .unwrap_or_else(|| missing.to_string());
            let app = source
                .get("NVIDIANvApp")
                .map(js_string)
                .unwrap_or_else(|| missing.to_string());
            let app = if ge != missing.as_str() && app == missing.as_str() {
                ge
            } else {
                app
            };
            data = data.child(format!("NVIDIA App: {app}"));
            for (label, key) in [("Razer Cortex", "RazerCortex"), ("Razer Axon", "RazerAxon")] {
                data = data.child(format!(
                    "{label}: {}",
                    source
                        .get(key)
                        .map(js_string)
                        .unwrap_or_else(|| missing.to_string())
                ));
            }
            info = info.child(data);
        }
        help_panel("MY_SYSTEM", exact, cx)
            .child(h_flex().items_center().gap(surface::css(15.)).child(top))
            .child(info)
            .child(
                BaseButton::new("source-help-system-download")
                    .p_0()
                    .self_start()
                    .my(surface::css(10.))
                    .child(
                        h_flex()
                            .child(
                                img("synapse/help-system-download.svg")
                                    .size(surface::css(20.))
                                    .mr(surface::css(5.)),
                            )
                            .child(i18n::t("SAVE_FULL_SYSTEM_LOGS").to_uppercase()),
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        if let Some(request) =
                            this.system_info_request(HelpSystemInfoCommand::SaveLogs)
                        {
                            cx.emit(request);
                        }
                    })),
            )
            .into_any_element()
    }
}
fn image_variant(product_id: u32) -> bool {
    matches!(product_id, 709 | 710 | 711 | 736 | 737)
}
fn js_truthy(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Null => false,
        serde_json::Value::Bool(value) => *value,
        serde_json::Value::Number(value) => value.as_f64().is_some_and(|number| number != 0.),
        serde_json::Value::String(value) => !value.is_empty(),
        _ => true,
    }
}
fn js_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value) => value.clone(),
        serde_json::Value::Object(_) => "[object Object]".into(),
        serde_json::Value::Array(values) => values
            .iter()
            .map(|value| {
                if value.is_null() {
                    String::new()
                } else {
                    js_string(value)
                }
            })
            .collect::<Vec<_>>()
            .join(","),
        _ => value.to_string(),
    }
}
