//! Typed input batches from the current recorder's Windows hook implementation.
//! Native owners: d9d60, da110, da3e0, da640 and da9c0 in mapping_engine.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum Mode {
    #[serde(rename = "kSoftware")]
    Software,
    #[serde(rename = "kOTFM")]
    Otfm,
}
impl Mode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Software => "kSoftware",
            Self::Otfm => "kOTFM",
        }
    }
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub(crate) enum Action {
    Name(&'static str),
    Message(i32),
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(untagged)]
pub(crate) enum Input {
    Keyboard {
        action: Action,
        time: i32,
        #[serde(rename = "vkCode")]
        virtual_key: i32,
        #[serde(rename = "scanCode")]
        scan_code: i32,
        flags: i32,
    },
    Mouse {
        action: Action,
        time: i32,
        x: i32,
        y: i32,
        data: i32,
    },
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub(crate) enum EventKind {
    Started { mode: Mode },
    Stopped { mode: Mode },
    Item { event: Vec<Input> },
}

#[derive(Debug, Serialize)]
pub struct Event {
    #[serde(flatten)]
    kind: EventKind,
    event_type: i32,
    time_tick: u64,
    received_at_ms: u64,
}
impl Event {
    pub(crate) fn new(kind: EventKind, time_tick: u64, received_at_ms: u64) -> Self {
        let event_type = match kind {
            EventKind::Started { .. } => 0,
            EventKind::Stopped { .. } => 1,
            EventKind::Item { .. } => 2,
        };
        Self {
            kind,
            event_type,
            time_tick,
            received_at_ms,
        }
    }
}

/// The source uses one previous wall-clock microsecond value for both hooks.
/// JSON integers are signed 32-bit, including delta truncation and DWORD bits.
#[derive(Default)]
pub(crate) struct Batch {
    previous: i64,
    items: Vec<Input>,
}
impl Batch {
    fn delta(&mut self, wall_tick: i64) -> i32 {
        let delta = if self.previous == 0 {
            0
        } else {
            wall_tick.wrapping_sub(self.previous) as i32
        };
        self.previous = wall_tick;
        delta
    }
    pub(crate) fn keyboard(
        &mut self,
        message: usize,
        key: u32,
        scan: u32,
        flags: u32,
        wall_tick: i64,
    ) -> Vec<Input> {
        let action = match message {
            0x100 | 0x104 => Action::Name("keydown"),
            0x101 | 0x105 => Action::Name("keyup"),
            _ => Action::Message(message as i32),
        };
        let time = self.delta(wall_tick);
        self.items.push(Input::Keyboard {
            action,
            time,
            virtual_key: key as i32,
            scan_code: scan as i32,
            flags: flags as i32,
        });
        self.flush()
    }
    pub(crate) fn mouse(
        &mut self,
        message: usize,
        x: i32,
        y: i32,
        data: u32,
        wall_tick: i64,
    ) -> Option<Vec<Input>> {
        let action = match message {
            0x200 => Action::Name("mousemove"),
            0x201 => Action::Name("lbuttondown"),
            0x202 => Action::Name("lbuttonup"),
            0x204 => Action::Name("rbuttondown"),
            0x205 => Action::Name("rbuttonup"),
            0x207 => Action::Name("mbuttondown"),
            0x208 => Action::Name("mbuttonup"),
            0x20a => Action::Name("mousewheel"),
            0x20e => Action::Name("mousehwheel"),
            _ => Action::Message(message as i32),
        };
        let time = self.delta(wall_tick);
        self.items.push(Input::Mouse {
            action,
            time,
            x,
            y,
            data: data as i32,
        });
        (message != 0x200 || self.items.len() > 1000).then(|| self.flush())
    }
    pub(crate) fn flush(&mut self) -> Vec<Input> {
        std::mem::take(&mut self.items)
    }
    pub(crate) fn finish(&mut self, mode: Mode) -> Vec<EventKind> {
        let mut events = Vec::with_capacity(2);
        let event = self.flush();
        if !event.is_empty() {
            events.push(EventKind::Item { event });
        }
        events.push(EventKind::Stopped { mode });
        events
    }
}

#[derive(Debug, Serialize)]
pub struct Events {
    pub events: Vec<Event>,
    pub overflow: bool,
}

#[derive(Default)]
pub struct MacroRecorder {
    #[cfg(windows)]
    owner: Option<crate::platform::windows::macro_recorder::Owner>,
}

/// Only successful hook acquisition/release commits these transitions.
#[derive(Default)]
pub(crate) struct Session {
    mode: Option<Mode>,
    pub(crate) batch: Batch,
}
impl Session {
    pub(crate) fn mode(&self) -> Option<Mode> {
        self.mode
    }
    pub(crate) fn start(&mut self, mode: Mode) -> anyhow::Result<EventKind> {
        anyhow::ensure!(self.mode.is_none(), "macro recorder has already started");
        self.batch = Batch::default();
        self.mode = Some(mode);
        Ok(EventKind::Started { mode })
    }
    pub(crate) fn stop(&mut self) -> anyhow::Result<Vec<EventKind>> {
        let mode = self
            .mode
            .take()
            .ok_or_else(|| anyhow::anyhow!("macro recorder has not started"))?;
        Ok(self.batch.finish(mode))
    }
}
impl MacroRecorder {
    pub fn prepare(&mut self) -> anyhow::Result<()> {
        #[cfg(windows)]
        {
            if self.owner.is_none() {
                self.owner = Some(crate::platform::windows::macro_recorder::Owner::spawn()?);
            }
            Ok(())
        }
        #[cfg(not(windows))]
        anyhow::bail!("Macro input recording requires the Windows hook adapter")
    }
    pub fn start(&mut self, mode: Mode) -> anyhow::Result<()> {
        self.prepare()?;
        #[cfg(windows)]
        return self.owner.as_ref().unwrap().start(mode);
        #[cfg(not(windows))]
        {
            let _ = mode;
            unreachable!()
        }
    }
    pub fn mode(&self) -> anyhow::Result<Option<Mode>> {
        #[cfg(windows)]
        return self.owner.as_ref().map_or(Ok(None), |owner| owner.mode());
        #[cfg(not(windows))]
        anyhow::bail!("Macro input recording requires the Windows hook adapter")
    }
    pub fn stop(&mut self) -> anyhow::Result<()> {
        #[cfg(windows)]
        return self
            .owner
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("macro recorder has not started"))?
            .stop();
        #[cfg(not(windows))]
        anyhow::bail!("Macro input recording requires the Windows hook adapter")
    }
    pub fn events(&self) -> anyhow::Result<Events> {
        #[cfg(windows)]
        return self.owner.as_ref().map_or_else(
            || {
                Ok(Events {
                    events: Vec::new(),
                    overflow: false,
                })
            },
            |owner| owner.events(),
        );
        #[cfg(not(windows))]
        anyhow::bail!("Macro input recording requires the Windows hook adapter")
    }
    pub fn shutdown(&mut self) -> anyhow::Result<()> {
        #[cfg(windows)]
        if let Some(mut owner) = self.owner.take() {
            owner.shutdown()?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "macro_recorder_tests.rs"]
mod tests;
