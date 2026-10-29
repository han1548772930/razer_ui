//! Current matcher middleware movement arithmetic and command state.
//! Inputs are actual source callbacks; no synthetic movement or completion timer.
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[cfg(test)]
use serde_json::json;

pub const PRODUCT_ID: u32 = 190;

// ECMAScript extrema propagate NaN and choose the specified sign for equal zeros.
fn source_max(left: f64, right: f64) -> f64 {
    if left.is_nan() || right.is_nan() {
        return f64::NAN;
    }
    if left == right {
        if left == 0. {
            return if left.is_sign_positive() || right.is_sign_positive() {
                0.
            } else {
                -0.
            };
        }
        return left;
    }
    if left > right { left } else { right }
}

fn source_min(left: f64, right: f64) -> f64 {
    if left.is_nan() || right.is_nan() {
        return f64::NAN;
    }
    if left == right {
        if left == 0. {
            return if left.is_sign_negative() || right.is_sign_negative() {
                -0.
            } else {
                0.
            };
        }
        return left;
    }
    if left < right { left } else { right }
}

fn source_round(value: f64) -> f64 {
    if value.is_nan() || value.is_infinite() || value == 0. {
        return value;
    }
    // Math.round selects the integer toward +infinity at an exact half and
    // preserves negative zero. Adding 0.5 first would round some inputs twice.
    let lower = value.floor();
    if value - lower < 0.5 { lower } else { value.ceil() }
}

/// Logical products with a mounted matcher and a reviewed sampling factory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Product(u32);

impl Product {
    pub fn new(product_id: u32) -> anyhow::Result<Self> {
        ensure!(
            matches!(product_id, 190 | 192 | 222 | 226 | 229 | 239),
            "product has no audited mounted DPI matcher"
        );
        Ok(Self(product_id))
    }

    pub fn product_id(self) -> u32 {
        self.0
    }

    /// The initial main stream is selected from the logical product's reviewed
    /// wired/dongle identities. PID226 also receives the BLE 228 callback;
    /// paired input-monitoring records remain a separate fallback in the host.
    pub fn accepts_initial_main_product(self, product_id: u32) -> bool {
        let Some(capability) = crate::mouse_current_dpi::capability(self.0) else {
            return false;
        };
        product_id == capability.wired_pid()
            || capability.dongle_pid() == Some(product_id)
            || (self.0 == 226 && product_id == 228)
    }

    pub fn is_dual_link_reference(self, product_id: u32) -> bool {
        product_id == 241
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum State {
    #[default]
    Ready,
    RequestingLock,
    Calibrating,
    Stopping,
    Completed,
    Error,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    /// The source reducer's initial value, before middleware GET_STATE.
    #[default]
    Idle,
    Ready,
    Calibrating,
    Completed,
    Error,
}

/// One immutable middleware observation. BroadcastChannel carries numbers
/// directly, so this in-process snapshot must not pass through JSON.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Calibration {
    phase: Phase,
    process: f64,
    current_dpi: Option<f64>,
    new_dpi: f64,
}

impl Calibration {
    pub fn new(phase: Phase, process: f64, current_dpi: f64, new_dpi: f64) -> Self {
        Self {
            phase,
            process,
            current_dpi: Some(current_dpi),
            new_dpi,
        }
    }

    pub fn phase(self) -> Phase {
        self.phase
    }

    pub fn process(self) -> f64 {
        self.process
    }

    pub fn current_dpi(self) -> Option<f64> {
        self.current_dpi
    }

    pub fn new_dpi(self) -> f64 {
        self.new_dpi
    }
}

impl State {
    pub fn phase(self) -> Phase {
        match self {
            Self::Ready | Self::RequestingLock => Phase::Ready,
            Self::Calibrating | Self::Stopping => Phase::Calibrating,
            Self::Completed => Phase::Completed,
            Self::Error => Phase::Error,
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
            movement.push(Movement { x, time_tick });
        }
        Ok(Self {
            product_id,
            container_id: container_id.to_owned(),
            movement,
        })
    }
}

pub struct Session {
    product: Product,
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

impl Default for Session {
    fn default() -> Self {
        Self::new(Product(PRODUCT_ID))
    }
}

impl Session {
    pub fn new(product: Product) -> Self {
        Self {
            product,
            state: State::Ready,
            current_dpi: 0.,
            new_dpi: 0.,
            process: 0.,
            traveled: 0.,
            stop_tick: 0.,
            main: Vec::new(),
            source: Vec::new(),
            buffered: Vec::new(),
            main_container: String::new(),
            source_container: String::new(),
            source_product_id: None,
            main_last_received: None,
            source_last_received: None,
            main_timeout_received: None,
            source_timeout_received: None,
        }
    }

    pub fn begin(&mut self, current_dpi: f64) -> anyhow::Result<()> {
        ensure!(
            matches!(self.state, State::Ready | State::Error),
            "matcher already active"
        );
        self.reset();
        self.state = State::RequestingLock;
        self.current_dpi = current_dpi;
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
        *self = Self::new(self.product);
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
    pub fn response(&self) -> Calibration {
        Calibration::new(
            self.state.phase(),
            self.process,
            self.current_dpi,
            self.new_dpi,
        )
    }
    /// The caller supplies the current source product/dual-link predicate.
    /// Every current factory prefers an already retained stream container.
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
        let main = if !batch.container_id.is_empty() && !self.main_container.is_empty() {
            batch.container_id == self.main_container
        } else if !batch.container_id.is_empty() && !self.source_container.is_empty() {
            batch.container_id != self.source_container
        } else {
            initial_is_main
        };
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
        loop {
            let process = source_round(self.traveled / maximum * 100.);
            self.process = source_min(process, 99.);
            if self.main.is_empty() || self.source.is_empty() || self.buffered.is_empty() {
                return Ok(false);
            }
            // Source checks completion before consuming another max-100 batch.
            if maximum <= self.traveled && self.stop_tick <= self.source.last().unwrap().time_tick {
                // k/w removes the listener and awaits STOP before calling U. Keep
                // the result unavailable while that transport obligation is pending.
                self.state = State::Stopping;
                self.main_timeout_received = None;
                self.source_timeout_received = None;
                return Ok(true);
            }
            let entries: Vec<_> = self
                .buffered
                .drain(..self.buffered.len().min(100))
                .collect();
            let start = source_max(self.main[0].time_tick, self.source[0].time_tick);
            self.traveled += entries
                .iter()
                .filter(|m| !(m.x <= 0. || m.time_tick < start))
                .fold(0., |distance, movement| distance + movement.x);
            if self.stop_tick == 0. && self.traveled > maximum {
                self.stop_tick = entries.last().unwrap().time_tick;
            }
            // k/w schedules its next pass after every consumed chunk. That
            // pass publishes progress before checking whether the buffer is empty.
        }
    }

    /// Invoke the source U continuation only after the caller's real STOP ACK.
    pub fn complete_after_stop(&mut self) -> anyhow::Result<()> {
        ensure!(
            self.state == State::Stopping,
            "matcher is not waiting for STOP"
        );
        let start = source_max(self.main[0].time_tick, self.source[0].time_tick);
        let end = source_min(
            self.main.last().unwrap().time_tick,
            self.source.last().unwrap().time_tick,
        );
        let distance = |events: &[Movement]| {
            events
                .iter()
                .filter(|m| !(m.time_tick > end || m.time_tick < start || m.x <= 0.))
                .fold(0., |distance, movement| distance + movement.x)
        };
        let main = distance(&self.main);
        let source = distance(&self.source);
        self.new_dpi = source_round(source / main * self.current_dpi);
        self.state = State::Completed;
        Ok(())
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
            self.reset();
            self.state = State::Error;
            return true;
        }
        false
    }
}

#[cfg(test)]
#[path = "dpi_matcher_tests.rs"]
mod tests;
