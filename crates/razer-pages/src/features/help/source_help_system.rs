//! Actual basic and image System Details Help mounts.
use super::*;
use razer_device::system_info::{self, SystemDetails};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
    device_name: Option<String>,
    cancelled: Arc<AtomicBool>,
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
    pub fn same_mount(&self, other: &Self) -> bool {
        self.owner == other.owner
            && self.generation == other.generation
            && self.product_id == other.product_id
            && self.page_offset == other.page_offset
            && self.edition_id == other.edition_id
            && self.layout_id == other.layout_id
    }
    pub fn file_name(&self, timer_tick: u64) -> String {
        format!(
            "{} - {timer_tick}.synapse4",
            self.device_name.as_deref().unwrap_or("undefined")
        )
    }
    pub fn cancellation(&self) -> Arc<AtomicBool> {
        self.cancelled.clone()
    }
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
    pub fn middleware_message(&self, timer_tick: u64) -> Option<serde_json::Value> {
        matches!(self.command, HelpSystemInfoCommand::SaveLogs).then(|| serde_json::json!({"type":"ON_STORE_SYSTEM_INFO_DATA","timerTick":timer_tick,"payload":null}))
    }
}
pub(super) struct State {
    owner: EntityId,
    generation: u64,
    source: Option<SystemDetails>,
    error: Option<String>,
    cancelled: Arc<AtomicBool>,
    expanded: bool,
    toggle_generation: u64,
}
impl State {
    pub(super) fn new(owner: EntityId) -> Self {
        Self {
            owner,
            generation: 0,
            source: None,
            error: None,
            cancelled: Arc::new(AtomicBool::new(false)),
            expanded: false,
            toggle_generation: 0,
        }
    }
    pub(super) fn unmount(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        self.cancelled = Arc::new(AtomicBool::new(false));
        self.generation = self.generation.wrapping_add(1);
        self.source = None;
        self.error = None;
        self.expanded = false;
        self.toggle_generation = 0;
    }
}
impl Drop for State {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
    }
}
impl EventEmitter<HelpSystemInfoRequested> for SourceHelp {}
impl SourceHelp {
    pub(super) fn system_info_request(
        &self,
        command: HelpSystemInfoCommand,
    ) -> Option<HelpSystemInfoRequested> {
        let (_, page) = self.metadata()?;
        (page.system_info && system_info::layout(self.device.product_id).is_some()).then(|| {
            HelpSystemInfoRequested {
                owner: self.system_info.owner,
                generation: self.system_info.generation,
                product_id: self.device.product_id,
                page_offset: page.offset,
                edition_id: self.device.edition_id,
                layout_id: self.device.layout_id,
                command,
                device_name: self
                    .system_info
                    .source
                    .as_ref()
                    .and_then(|source| source.device_name.clone()),
                cancelled: self.system_info.cancelled.clone(),
            }
        })
    }
    pub(in crate::features) fn accepts_system_info_request(
        &self,
        request: &HelpSystemInfoRequested,
    ) -> bool {
        !request.is_cancelled()
            && self
                .system_info_request(HelpSystemInfoCommand::Observe)
                .is_some_and(|current| current.same_mount(request))
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
        source: Result<SystemDetails, String>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.accepts_system_info_request(request) {
            return false;
        }
        match source {
            Ok(source) => {
                self.system_info.source = Some(source);
                self.system_info.error = None;
            }
            Err(error) => {
                self.system_info.source = None;
                self.system_info.error = Some(error);
            }
        }
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
            let source = self.system_info.source.as_ref();
            let mut data = v_flex().mt(surface::css(8.));
            let fields = [
                (
                    "DEVICE_NAME",
                    source.and_then(|s| s.device_name.as_deref()),
                    "",
                ),
                ("PROCESSOR", source.and_then(|s| s.cpu.as_deref()), ""),
                ("INSTALLED_RAM", source.and_then(|s| s.ram.as_deref()), ""),
                (
                    "SERIAL_NUMBER",
                    source.and_then(|s| s.serial_number.as_deref()),
                    "",
                ),
                (
                    "MODEL_NUMBER",
                    source.and_then(|s| s.model_name.as_deref()),
                    "",
                ),
                (
                    "OS_VERSION",
                    source.and_then(|s| s.os_version.as_deref()),
                    "",
                ),
                (
                    "DISPLAY_1",
                    source
                        .and_then(|s| s.monitors().first())
                        .map(|display| display.name.as_str()),
                    "",
                ),
                (
                    "DISPLAY_2",
                    source
                        .and_then(|s| s.monitors().get(1))
                        .map(|display| display.name.as_str()),
                    "",
                ),
                (
                    "NVIDIA_GEFORCE_EXPERIENCE",
                    source.and_then(|s| s.nvidia_geforce_experience.as_deref()),
                    "",
                ),
                (
                    "GPU_MODE_NVIDIA_CONTROL_PANEL",
                    source.and_then(|s| s.nvidia_control_panel.as_deref()),
                    "",
                ),
                (
                    "COMMAND_CENTER",
                    source.and_then(|s| s.intel_graphic_command_center.as_deref()),
                    "",
                ),
                (
                    "AMD_ADRENALINE",
                    source.and_then(|s| s.amd_adrenalin_edition.as_deref()),
                    "Not Installed",
                ),
                (
                    "CORTEX",
                    source.and_then(|s| s.razer_cortex.as_deref()),
                    "Not Installed",
                ),
                (
                    "AXON",
                    source.and_then(|s| s.razer_axon.as_deref()),
                    "Not Installed",
                ),
            ];
            for (label, value, fallback) in fields {
                let value = value.unwrap_or(if source.is_some() { fallback } else { "" });
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
        let source = self.system_info.source.as_ref();
        let mut top = v_flex().gap(surface::css(15.));
        for (label, content, vertical) in [
            (None, source.and_then(|s| s.device_name.as_deref()), false),
            (
                Some("PROCESSOR"),
                source.and_then(|s| s.cpu.as_deref()),
                false,
            ),
            (
                Some("GRAPHICS"),
                source.and_then(|s| s.gpu.as_deref()),
                false,
            ),
            (
                Some("INSTALLED_RAM"),
                source.and_then(|s| s.ram.as_deref()),
                true,
            ),
        ] {
            let content = content.unwrap_or("");
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
                    .child(content.to_owned());
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
                source.and_then(|s| s.os_version.as_deref()).unwrap_or("")
            ));
            if let Some(displays) = source.map(SystemDetails::monitors) {
                for (index, display) in displays.iter().enumerate() {
                    data = data.child(format!("Display {}: {}", index + 1, display.name));
                }
            }
            let missing = if source.is_some() {
                i18n::t("NOT_INSTALLED")
            } else {
                "".into()
            };
            let ge = source
                .and_then(|s| s.nvidia_geforce_experience.as_deref())
                .unwrap_or(&missing);
            let app = source
                .and_then(|s| s.nvidia_app.as_deref())
                .unwrap_or(&missing);
            let app = if ge != missing.as_str() && app == missing.as_str() {
                ge
            } else {
                app
            };
            data = data.child(format!("NVIDIA App: {app}"));
            for (label, value) in [
                (
                    "Razer Cortex",
                    source.and_then(|s| s.razer_cortex.as_deref()),
                ),
                ("Razer Axon", source.and_then(|s| s.razer_axon.as_deref())),
            ] {
                data = data.child(format!("{label}: {}", value.unwrap_or(&missing)));
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
    matches!(
        system_info::layout(product_id),
        Some(system_info::Layout::Image)
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> HelpSystemInfoRequested {
        HelpSystemInfoRequested {
            owner: EntityId::from(12),
            generation: 1,
            product_id: 737,
            page_offset: 123,
            edition_id: 2,
            layout_id: 3,
            command: HelpSystemInfoCommand::Observe,
            device_name: Some("Observed laptop".into()),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
    #[::core::prelude::v1::test]
    fn help_system_info_mount_identity_includes_owner_product_edition_layout_and_generation() {
        let original = request();
        for branch in 0..6 {
            let mut stale = original.clone();
            match branch {
                0 => stale.owner = EntityId::from(13),
                1 => stale.generation += 1,
                2 => stale.product_id = 711,
                3 => stale.page_offset += 1,
                4 => stale.edition_id += 1,
                _ => stale.layout_id += 1,
            }
            assert!(!original.same_mount(&stale));
        }
        let mut save = original.clone();
        save.command = HelpSystemInfoCommand::SaveLogs;
        save.device_name = None;
        assert!(original.same_mount(&save));
        assert_eq!(original.file_name(456), "Observed laptop - 456.synapse4");
    }
    #[::core::prelude::v1::test]
    fn help_system_info_unmount_cancels_the_old_token_and_preserves_new_mount_admission() {
        let mut state = State::new(EntityId::from(12));
        let previous = state.cancelled.clone();
        state.unmount();
        assert!(previous.load(Ordering::Acquire));
        assert!(!state.cancelled.load(Ordering::Acquire));
        assert!(state.source.is_none());
        let current = state.cancelled.clone();
        drop(state);
        assert!(current.load(Ordering::Acquire));
    }
}
