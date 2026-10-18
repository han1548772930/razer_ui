//! Current PID 190 middleware 32807 movement arithmetic and command state.
//! Inputs are actual source callbacks; no synthetic movement or completion timer.
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const PRODUCT_ID: u32 = 190;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum State {
    #[default]
    Ready,
    RequestingLock,
    Calibrating,
    Completed,
    Error,
}
impl State {
    pub fn ui(self) -> &'static str {
        match self {
            Self::Ready | Self::RequestingLock => "ready",
            Self::Calibrating => "calibrating",
            Self::Completed => "completed",
            Self::Error => "error",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Movement {
    pub x: f64,
    #[serde(rename = "timeTick")]
    pub time_tick: f64,
}

#[derive(Clone, Debug)]
pub struct InputBatch {
    pub product_id: u32,
    pub container_id: String,
    pub movement: Vec<Movement>,
}
impl InputBatch {
    /// Host callback jsonEvent contains JSON rows whose `data` is another JSON
    /// string. Only mouseMove participates; callback identity remains separate.
    pub fn from_callback(value: &Value) -> anyhow::Result<Self> {
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Callback {
            product_id: u32,
            container_id: String,
            json_event: String,
        }
        let callback: Callback = serde_json::from_value(value.clone())?;
        Self::from_event(
            callback.product_id,
            &callback.container_id,
            &callback.json_event,
        )
    }

    pub fn from_event(
        product_id: u32,
        container_id: &str,
        json_event: &str,
    ) -> anyhow::Result<Self> {
        ensure!(
            product_id != 0 && !container_id.is_empty(),
            "missing input identity"
        );
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Tick {
            Number(f64),
            Text(String),
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Row {
            data: String,
            time_tick: Option<Tick>,
        }
        #[derive(Deserialize)]
        #[serde(tag = "type")]
        enum Data {
            #[serde(rename = "mouseMove")]
            MouseMove { x: f64 },
            #[serde(other)]
            Other,
        }
        let rows: Vec<Row> = serde_json::from_str(json_event)?;
        let mut movement = Vec::new();
        for row in rows {
            let data: Data = serde_json::from_str(&row.data)?;
            let Data::MouseMove { x } = data else {
                continue;
            };
            let time_tick = match row.time_tick.context("missing movement timeTick")? {
                Tick::Number(number) => number,
                Tick::Text(text) => text.parse::<f64>().context("invalid movement timeTick")?,
            };
            ensure!(
                time_tick.is_finite() && x.is_finite(),
                "nonfinite source sample"
            );
            movement.push(Movement { x, time_tick });
        }
        Ok(Self {
            product_id,
            container_id: container_id.to_owned(),
            movement,
        })
    }
}

#[derive(Default)]
pub struct Session {
    pub state: State,
    current_dpi: f64,
    new_dpi: f64,
    process: f64,
    traveled: f64,
    stop_tick: f64,
    main: Vec<Movement>,
    source: Vec<Movement>,
    buffered: Vec<Movement>,
    main_container: String,
    source_container: String,
    pub source_product_id: Option<u32>,
    main_last_received: Option<u64>,
    source_last_received: Option<u64>,
    main_timeout_received: Option<u64>,
    source_timeout_received: Option<u64>,
}
impl Session {
    pub fn begin(&mut self, current_dpi: f64) -> anyhow::Result<()> {
        ensure!(
            matches!(self.state, State::Ready | State::Error),
            "matcher already active"
        );
        ensure!(
            current_dpi.is_finite() && current_dpi > 0.,
            "invalid observed current DPI"
        );
        *self = Self {
            state: State::RequestingLock,
            current_dpi,
            ..Self::default()
        };
        Ok(())
    }
    /// Called only after the actual source shared/exclusive lock is acquired.
    pub fn acquired_lock(&mut self) -> anyhow::Result<()> {
        ensure!(
            self.state == State::RequestingLock,
            "matcher is not requesting lock"
        );
        self.state = State::Calibrating;
        Ok(())
    }
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn fail(&mut self) {
        self.state = State::Error;
    }
    pub fn new_dpi(&self) -> Option<f64> {
        (self.state == State::Completed).then_some(self.new_dpi)
    }
    pub fn source_container(&self) -> &str {
        &self.source_container
    }
    pub fn response(&self) -> Value {
        json!({"type":"MW_SET_DPI_MATCHER_STATE","payload":{"state":self.state.ui(),"process":self.process,"newDpi":self.new_dpi,"currentDpi":self.current_dpi}})
    }
    /// The caller supplies the independently observed product-family predicate
    /// from source R; once a container is known R uses its identity instead.
    pub fn ingest(
        &mut self,
        batch: InputBatch,
        initial_is_main: bool,
        received_ms: u64,
    ) -> anyhow::Result<bool> {
        ensure!(
            self.state == State::Calibrating,
            "matcher is not calibrating"
        );
        let main = if !self.main_container.is_empty() {
            batch.container_id == self.main_container
        } else if !self.source_container.is_empty() {
            batch.container_id != self.source_container
        } else {
            initial_is_main
        };
        ensure!(
            batch
                .movement
                .iter()
                .all(|m| m.x.is_finite() && m.time_tick.is_finite()),
            "nonfinite source movement"
        );
        if main {
            self.main_container = batch.container_id;
            self.main_last_received = Some(received_ms);
            if self.source_last_received.is_some() {
                self.main_timeout_received = Some(received_ms);
            }
            self.buffered.extend_from_slice(&batch.movement);
            self.main.extend(batch.movement);
        } else {
            self.source_container = batch.container_id;
            self.source_product_id = Some(batch.product_id);
            self.source_last_received = Some(received_ms);
            if self.main_last_received.is_some() {
                self.source_timeout_received = Some(received_ms);
            }
            self.source.extend(batch.movement);
        }
        self.advance()
    }
    fn advance(&mut self) -> anyhow::Result<bool> {
        let maximum = 5. * self.current_dpi;
        self.process = (self.traveled / maximum * 100.).round().min(99.);
        if self.main.is_empty() || self.source.is_empty() || self.buffered.is_empty() {
            return Ok(false);
        }
        // Source checks completion before consuming another max-100 batch.
        if maximum <= self.traveled && self.stop_tick <= self.source.last().unwrap().time_tick {
            let start = self.main[0].time_tick.max(self.source[0].time_tick);
            let end = self
                .main
                .last()
                .unwrap()
                .time_tick
                .min(self.source.last().unwrap().time_tick);
            let distance = |events: &[Movement]| {
                events
                    .iter()
                    .filter(|m| m.time_tick >= start && m.time_tick <= end && m.x > 0.)
                    .map(|m| m.x)
                    .sum::<f64>()
            };
            let main = distance(&self.main);
            let source = distance(&self.source);
            ensure!(main > 0. && source > 0., "no overlapping positive movement");
            self.new_dpi = (source / main * self.current_dpi).round();
            ensure!(
                self.new_dpi.is_finite() && self.new_dpi > 0.,
                "invalid measured DPI"
            );
            self.state = State::Completed;
            return Ok(true);
        }
        let entries: Vec<_> = self
            .buffered
            .drain(..self.buffered.len().min(100))
            .collect();
        let start = self.main[0].time_tick.max(self.source[0].time_tick);
        self.traveled += entries
            .iter()
            .filter(|m| m.x > 0. && m.time_tick >= start)
            .map(|m| m.x)
            .sum::<f64>();
        if self.stop_tick == 0. && self.traveled > maximum {
            self.stop_tick = entries.last().unwrap().time_tick;
        }
        if !self.buffered.is_empty() {
            return self.advance();
        }
        Ok(false)
    }
    /// P arms only the current stream's timer once both allEvents arrays contain
    /// a callback, even if a callback contains no mouseMove rows.
    pub fn check_timeout(&mut self, now_ms: u64) -> bool {
        if self.state != State::Calibrating {
            return false;
        }
        if [self.main_timeout_received, self.source_timeout_received]
            .into_iter()
            .flatten()
            .any(|last| now_ms.saturating_sub(last) >= 500)
        {
            // P sends the error transition, then K resets the movement record.
            *self = Self {
                state: State::Error,
                ..Self::default()
            };
            return true;
        }
        false
    }
}

#[cfg(test)]
#[path = "dpi_matcher_tests.rs"]
mod tests;
