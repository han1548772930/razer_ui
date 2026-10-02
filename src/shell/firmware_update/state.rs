//! Native state names and transitions from update-fw main.69cc5fbd.js / ao.
//! This model contains local preview data only. No field is a hardware result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Stage {
    Unknown,
    Launch,
    Prepare,
    ReadyToUpgrade,
    Upgrade,
    PartialCompleteUpgrade,
    SwitchDeviceMode,
    CompleteDone,
    CompleteSkipOrFail,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Component {
    Device,
    Dongle,
}
impl Component {
    pub(super) fn other(self) -> Self {
        match self {
            Self::Device => Self::Dongle,
            Self::Dongle => Self::Device,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Warning {
    WaitForOtherDevices,
    DeviceIsUpgrading,
    Disconnected,
    IsolateFailed,
    UpdateFailed,
}
impl Warning {
    pub(super) fn title(self) -> Option<&'static str> {
        match self {
            Self::WaitForOtherDevices => None,
            Self::DeviceIsUpgrading => Some("UPDATE_IN_PROGRESS"),
            Self::Disconnected => Some("DEVICE_IS_DISCONNECTED"),
            Self::IsolateFailed | Self::UpdateFailed => Some("UPDATE_FAILED"),
        }
    }
    pub(super) fn body(self) -> &'static str {
        match self {
            Self::WaitForOtherDevices => "WAIT_FOR_OTHER_DEVICES",
            Self::DeviceIsUpgrading => "DEVICE_IS_UPGRADING",
            Self::Disconnected => "DEVICE_IS_DISCONNECTED_DETAIL",
            Self::IsolateFailed | Self::UpdateFailed => "UPDATE_FAILED_DETAIL",
        }
    }
    pub(super) fn cancellable(self) -> bool {
        matches!(
            self,
            Self::Disconnected | Self::IsolateFailed | Self::UpdateFailed
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Preset {
    pub(super) keyboard: bool,
    pub(super) initial: Component,
    pub(super) dual: bool,
    pub(super) large_dongle: bool,
}
impl Default for Preset {
    fn default() -> Self {
        Self {
            keyboard: true,
            initial: Component::Device,
            dual: true,
            large_dongle: false,
        }
    }
}
impl Preset {
    pub(super) fn from_key(key: &str) -> Self {
        Self {
            keyboard: !key.starts_with("mouse"),
            initial: if key.ends_with("dongle") {
                Component::Dongle
            } else {
                Component::Device
            },
            dual: key != "keyboard-single",
            large_dongle: key == "mouse-dongle",
        }
    }
}

pub(super) struct Flow {
    pub(super) stage: Stage,
    pub(super) preset: Preset,
    pub(super) current: Component,
    pub(super) completed: Vec<Component>,
    pub(super) warning: Option<Warning>,
    pub(super) progress: u8,
    pub(super) connection_changed: bool,
    pub(super) matching_devices: usize,
    pub(super) already_current: bool,
    generation: u64,
}
impl Default for Flow {
    fn default() -> Self {
        Self {
            stage: Stage::Unknown,
            preset: Preset::default(),
            current: Component::Device,
            completed: vec![],
            warning: None,
            progress: 0,
            connection_changed: false,
            matching_devices: 0,
            already_current: false,
            generation: 0,
        }
    }
}
impl Flow {
    pub(super) fn preview(&self) -> bool {
        self.stage != Stage::Unknown
    }
    pub(super) fn load(&mut self, key: &str) {
        self.generation = self.generation.wrapping_add(1);
        self.current = self.preset.initial;
        self.completed.clear();
        self.warning = None;
        self.progress = 0;
        self.connection_changed = false;
        self.matching_devices = 0;
        self.already_current = key == "latest";
        self.stage = match key {
            "unknown" => Stage::Unknown,
            "prepare" => Stage::Prepare,
            "ready" | "waiting" => Stage::ReadyToUpgrade,
            "upgrade" | "exit-warning" => Stage::Upgrade,
            "partial" | "latest" => Stage::PartialCompleteUpgrade,
            "switch" | "multiple" => Stage::SwitchDeviceMode,
            "success" => Stage::CompleteDone,
            "incomplete" | "disconnected" | "isolate-failed" | "failed" => {
                Stage::CompleteSkipOrFail
            }
            _ => Stage::Launch,
        };
        if matches!(
            key,
            "partial" | "latest" | "switch" | "multiple" | "success"
        ) || key == "incomplete" && self.preset.dual
        {
            self.completed.push(self.current);
        }
        if matches!(key, "switch" | "multiple") {
            self.current = self.current.other();
        }
        if key == "multiple" {
            self.connection_changed = true;
            self.matching_devices = 2;
        }
        if key == "success" && self.preset.dual {
            self.completed.push(self.current.other());
        }
        if !self.preset.dual && matches!(key, "partial" | "latest" | "switch" | "multiple") {
            self.stage = Stage::CompleteDone;
            self.current = self.preset.initial;
        }
        self.warning = match key {
            "waiting" => Some(Warning::WaitForOtherDevices),
            "exit-warning" => Some(Warning::DeviceIsUpgrading),
            "disconnected" => Some(Warning::Disconnected),
            "isolate-failed" => Some(Warning::IsolateFailed),
            "failed" => Some(Warning::UpdateFailed),
            _ => None,
        };
    }
    pub(super) fn ticket(&self) -> (u64, Stage, Component) {
        (self.generation, self.stage, self.current)
    }
    pub(super) fn accepts(&self, ticket: (u64, Stage, Component)) -> bool {
        self.ticket() == ticket
    }
    pub(super) fn can_switch(&self) -> bool {
        self.stage == Stage::SwitchDeviceMode
            && self.connection_changed
            && self.matching_devices == 1
    }
    pub(super) fn next(&mut self) -> bool {
        if self.warning.is_some() {
            return false;
        }
        self.stage = match self.stage {
            Stage::Launch => Stage::Prepare,
            Stage::ReadyToUpgrade => Stage::Upgrade,
            Stage::PartialCompleteUpgrade if self.preset.dual => {
                self.current = self.current.other();
                self.connection_changed = false;
                self.matching_devices = 0;
                Stage::SwitchDeviceMode
            }
            Stage::SwitchDeviceMode if self.can_switch() => Stage::Prepare,
            _ => return false,
        };
        self.progress = 0;
        self.already_current = false;
        true
    }
    pub(super) fn tick(&mut self, ticket: (u64, Stage, Component)) -> bool {
        if !self.accepts(ticket) || !matches!(self.stage, Stage::Prepare | Stage::Upgrade) {
            return false;
        }
        self.progress = self.progress.saturating_add(10).min(100);
        if self.progress == 100 {
            if self.stage == Stage::Prepare {
                if self.completed.contains(&self.current) {
                    self.already_current = true;
                    self.mark_complete();
                } else {
                    self.stage = Stage::ReadyToUpgrade;
                }
            } else {
                self.mark_complete();
            }
            return false;
        }
        true
    }
    fn mark_complete(&mut self) {
        if !self.completed.contains(&self.current) {
            self.completed.push(self.current);
        }
        self.stage = if self.preset.dual && self.completed.len() < 2 {
            Stage::PartialCompleteUpgrade
        } else {
            Stage::CompleteDone
        };
        // A close warning must never outlive the operation it describes.
        if self.warning == Some(Warning::DeviceIsUpgrading) {
            self.warning = None;
        }
    }
    pub(super) fn skip(&mut self) {
        if matches!(
            self.stage,
            Stage::Prepare
                | Stage::ReadyToUpgrade
                | Stage::PartialCompleteUpgrade
                | Stage::SwitchDeviceMode
        ) {
            self.stage = Stage::CompleteSkipOrFail;
            self.generation = self.generation.wrapping_add(1);
        }
    }
    pub(super) fn fail(&mut self, warning: Warning) {
        if self.preview() {
            self.stage = Stage::CompleteSkipOrFail;
            self.warning = Some(warning);
            self.generation = self.generation.wrapping_add(1);
        }
    }
    pub(super) fn restart(&mut self) {
        // ao.resetIndex retains previousUpgradeRes; successful components are skipped.
        if self.stage != Stage::CompleteSkipOrFail || self.warning.is_some() {
            return;
        }
        self.generation = self.generation.wrapping_add(1);
        self.current = self.preset.initial;
        self.progress = 0;
        self.already_current = false;
        self.stage = Stage::Launch;
    }
    pub(super) fn remaining(&self) -> Vec<Component> {
        [Component::Device, Component::Dongle]
            .into_iter()
            .filter(|part| {
                (self.preset.dual || *part == self.preset.initial) && !self.completed.contains(part)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_cannot_begin_or_accept_a_callback() {
        let mut flow = Flow::default();
        assert!(!flow.next());
        assert!(!flow.tick(flow.ticket()));
        assert_eq!(flow.stage, Stage::Unknown);
    }
    #[test]
    fn reset_rejects_previous_preview_callback() {
        let mut flow = Flow::default();
        flow.load("prepare");
        let old = flow.ticket();
        flow.load("prepare");
        assert!(!flow.tick(old));
        assert_eq!(flow.progress, 0);
    }
    #[test]
    fn switch_requires_changed_connection_and_exactly_one_matching_device() {
        let mut flow = Flow::default();
        flow.load("switch");
        flow.matching_devices = 1;
        assert!(!flow.next());
        flow.connection_changed = true;
        flow.matching_devices = 2;
        assert!(!flow.next());
        flow.matching_devices = 1;
        assert!(flow.next());
        assert_eq!(flow.stage, Stage::Prepare);
    }
    #[test]
    fn retry_keeps_completed_component_and_skip_does_not_mark_the_other_done() {
        let mut flow = Flow::default();
        flow.load("partial");
        flow.skip();
        assert_eq!(flow.remaining(), vec![Component::Dongle]);
        flow.restart();
        assert_eq!(flow.stage, Stage::Launch);
        assert_eq!(flow.completed, vec![Component::Device]);
        assert!(flow.next());
        let ticket = flow.ticket();
        for _ in 0..10 {
            flow.tick(ticket);
        }
        assert_eq!(flow.stage, Stage::PartialCompleteUpgrade);
        assert!(flow.already_current);
    }
}
