//! Current Carol/virtual-route Surround middleware semantics. Native I/O belongs
//! to the service adapter; stored preferences and acknowledged values are distinct.
use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};

#[path = "surround/observed.rs"]
pub mod observed;
#[path = "surround/presentation.rs"]
pub mod presentation;
#[path = "surround/protocol.rs"]
pub mod protocol;
#[path = "surround/routing.rs"]
pub mod routing;
#[path = "surround/wire.rs"]
pub mod wire;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u32", into = "u32")]
pub enum Product {
    Clio,
    Phantom,
    VirtualRoute,
}
impl TryFrom<u32> for Product {
    type Error = anyhow::Error;
    fn try_from(value: u32) -> Result<Self> {
        match value {
            1392 => Ok(Self::Clio),
            1442 => Ok(Self::Phantom),
            3942 => Ok(Self::VirtualRoute),
            _ => bail!("No current Surround source for product {value}"),
        }
    }
}
impl From<Product> for u32 {
    fn from(value: Product) -> Self {
        match value {
            Product::Clio => 1392,
            Product::Phantom => 1442,
            Product::VirtualRoute => 3942,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "IdentityFields")]
pub struct Identity {
    product: Product,
    container_id: String,
    owner: String,
    generation: u64,
}
#[derive(Deserialize)]
struct IdentityFields {
    product: Product,
    container_id: String,
    owner: String,
    generation: u64,
}
impl TryFrom<IdentityFields> for Identity {
    type Error = anyhow::Error;
    fn try_from(value: IdentityFields) -> Result<Self> {
        Self::new(
            value.product,
            value.container_id,
            value.owner,
            value.generation,
        )
    }
}
impl Identity {
    pub fn new(
        product: Product,
        container_id: String,
        owner: String,
        generation: u64,
    ) -> Result<Self> {
        ensure!(
            !container_id.is_empty() && !container_id.contains('\0'),
            "Surround container identity is absent or invalid"
        );
        ensure!(
            !owner.is_empty() && owner.len() <= 256 && !owner.contains('\0'),
            "Surround session owner is absent or invalid"
        );
        Ok(Self {
            product,
            container_id,
            owner,
            generation,
        })
    }
    pub fn product(&self) -> Product {
        self.product
    }
    pub fn container_id(&self) -> &str {
        &self.container_id
    }
    pub fn owner(&self) -> &str {
        &self.owner
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct Volume(u8);
impl Volume {
    pub fn new(value: u8) -> Result<Self> {
        ensure!(value <= 100, "Surround volume exceeds 100");
        Ok(Self(value))
    }
    pub fn percent(self) -> u8 {
        self.0
    }
    pub fn native_value(self) -> f32 {
        f32::from(self.0) / 100.0
    }
    pub fn from_native(value: f32) -> Result<Self> {
        ensure!(
            value.is_finite() && (0.0..=1.0).contains(&value),
            "Invalid native Surround volume"
        );
        Self::new((value * 100.0).round() as u8)
    }
}
impl TryFrom<u8> for Volume {
    type Error = anyhow::Error;
    fn try_from(value: u8) -> Result<Self> {
        Self::new(value)
    }
}
impl From<Volume> for u8 {
    fn from(value: Volume) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "i16", into = "i16")]
pub struct Delay(i16);
impl Delay {
    pub const ZERO: Self = Self(0);
    pub fn new(value: i16) -> Result<Self> {
        ensure!(
            (-100..=100).contains(&value),
            "Surround delay is outside -100..100"
        );
        Ok(Self(value))
    }
    pub fn milliseconds(self) -> i16 {
        self.0
    }
    /// The current wrapper divides its millisecond slider value by 100.
    pub fn native_value(self) -> f32 {
        f32::from(self.0) / 100.0
    }
}
impl TryFrom<i16> for Delay {
    type Error = anyhow::Error;
    fn try_from(value: i16) -> Result<Self> {
        Self::new(value)
    }
}
impl From<Delay> for i16 {
    fn from(value: Delay) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "[i8; 3]", into = "[i8; 3]")]
pub struct Equalizer([i8; 3]);
impl Equalizer {
    pub const FLAT: Self = Self([0; 3]);
    pub fn new(bands: [i8; 3]) -> Result<Self> {
        ensure!(
            bands.iter().all(|band| (-6..=6).contains(band)),
            "Surround EQ exceeds -6..6 dB"
        );
        Ok(Self(bands))
    }
    pub fn bands(self) -> [i8; 3] {
        self.0
    }
}
impl TryFrom<[i8; 3]> for Equalizer {
    type Error = anyhow::Error;
    fn try_from(value: [i8; 3]) -> Result<Self> {
        Self::new(value)
    }
}
impl From<Equalizer> for [i8; 3] {
    fn from(value: Equalizer) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "EndpointFields")]
pub struct Endpoint {
    id: String,
    name: String,
}
#[derive(Deserialize)]
struct EndpointFields {
    id: String,
    name: String,
}
impl TryFrom<EndpointFields> for Endpoint {
    type Error = anyhow::Error;
    fn try_from(value: EndpointFields) -> Result<Self> {
        Self::new(value.id, value.name)
    }
}
impl Endpoint {
    pub fn new(id: String, name: String) -> Result<Self> {
        ensure!(
            !id.is_empty() && !name.is_empty(),
            "Front speaker identity is absent"
        );
        ensure!(
            !id.contains('\0') && !name.contains('\0'),
            "Front speaker contains a NUL"
        );
        Ok(Self { id, name })
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Channel {
    Front,
    Rear,
}

/// Middleware's persisted defaults differ from the pre-initialization UI state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    enabled: bool,
    delay: Delay,
    front_speaker: Option<Endpoint>,
    front_equalizer: Equalizer,
    rear_equalizer: Equalizer,
    replicate_front_stereo: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            delay: Delay::ZERO,
            front_speaker: None,
            front_equalizer: Equalizer::FLAT,
            rear_equalizer: Equalizer::FLAT,
            replicate_front_stereo: false,
        }
    }
}
impl Settings {
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
    pub fn delay(&self) -> Delay {
        self.delay
    }
    pub fn front_speaker(&self) -> Option<&Endpoint> {
        self.front_speaker.as_ref()
    }
    pub fn equalizer(&self, channel: Channel) -> Equalizer {
        match channel {
            Channel::Front => self.front_equalizer,
            Channel::Rear => self.rear_equalizer,
        }
    }
    pub fn is_front_stereo_replicated(&self) -> bool {
        self.replicate_front_stereo
    }
    /// Called by the persistence owner before the current task runner. Volume
    /// and the audition signal are deliberately absent from stored preferences.
    pub fn apply(&mut self, change: &Change) {
        match change {
            Change::Enabled(enabled) => self.enabled = *enabled,
            Change::FrontSpeaker(endpoint) => self.front_speaker = Some(endpoint.clone()),
            Change::Delay(delay) => self.delay = *delay,
            Change::Equalizer {
                channel: Channel::Front,
                equalizer,
            } => self.front_equalizer = *equalizer,
            Change::Equalizer {
                channel: Channel::Rear,
                equalizer,
            } => self.rear_equalizer = *equalizer,
            Change::Replicate(enabled) => self.replicate_front_stereo = *enabled,
            Change::Volume { .. } | Change::TestSignal(_) => {}
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "setting", content = "value", rename_all = "snake_case")]
pub enum Change {
    Enabled(bool),
    FrontSpeaker(Endpoint),
    Volume {
        channel: Channel,
        volume: Volume,
    },
    Equalizer {
        channel: Channel,
        equalizer: Equalizer,
    },
    Delay(Delay),
    Replicate(bool),
    TestSignal(bool),
}
impl Change {
    pub fn is_persisted(&self) -> bool {
        !matches!(self, Self::Volume { .. } | Self::TestSignal(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "call", content = "value", rename_all = "snake_case")]
pub enum NativeCall {
    SetEnabled(bool),
    SetFrontEndpoint(String),
    SetVolume {
        channel: Channel,
        volume: Volume,
    },
    SetRearMute(bool),
    SetEqualizer {
        channel: Channel,
        equalizer: Equalizer,
    },
    SetDelay(Delay),
    SetReplicate(bool),
    SetTestSignal(bool),
}

pub trait Transport {
    /// True confirms the current protocol acknowledgment. Audition delivery does not confirm playback.
    fn call(&mut self, call: &NativeCall) -> Result<bool>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallReceipt {
    call: NativeCall,
    accepted: bool,
}
impl CallReceipt {
    pub fn call(&self) -> &NativeCall {
        &self.call
    }
    pub fn is_accepted(&self) -> bool {
        self.accepted
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    calls: Vec<CallReceipt>,
    follow_ups: Vec<Change>,
    completed: bool,
    failure: Option<String>,
}
impl Outcome {
    pub fn calls(&self) -> &[CallReceipt] {
        &self.calls
    }
    pub fn is_completed(&self) -> bool {
        self.completed
    }
    pub fn failure(&self) -> Option<&str> {
        self.failure.as_deref()
    }
    pub fn follow_ups(&self) -> &[Change] {
        &self.follow_ups
    }
    fn call(&mut self, transport: &mut impl Transport, call: NativeCall) -> Result<bool> {
        let accepted = transport.call(&call)?;
        self.calls.push(CallReceipt { call, accepted });
        Ok(accepted)
    }
    fn required(&mut self, transport: &mut impl Transport, call: NativeCall) -> Result<()> {
        ensure!(
            self.call(transport, call)?,
            "Surround native call returned false"
        );
        Ok(())
    }
}

/// The middleware's acknowledged runtime cache, separate from local settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Runtime {
    settings: Settings,
    front_volume: Option<Volume>,
    rear_volume: Option<Volume>,
    test_signal: Option<bool>,
    test_signal_requested: Option<bool>,
}
impl Runtime {
    /// Initial runtime from the stored comboMode preferences. Volumes and the
    /// audition signal stay unobserved until a native receipt exists.
    pub fn with_settings(settings: Settings) -> Self {
        Self {
            settings,
            ..Self::default()
        }
    }
    pub fn settings(&self) -> &Settings {
        &self.settings
    }
    pub fn volume(&self, channel: Channel) -> Option<Volume> {
        match channel {
            Channel::Front => self.front_volume,
            Channel::Rear => self.rear_volume,
        }
    }
    pub fn test_signal(&self) -> Option<bool> {
        self.test_signal
    }
    pub fn replace_observed_volumes(&mut self, front: Option<Volume>, rear: Option<Volume>) {
        self.front_volume = front;
        self.rear_volume = rear;
    }
    pub fn observe_front_endpoint(&mut self, endpoint: Option<Endpoint>) {
        self.settings.front_speaker = endpoint;
    }
    pub fn test_signal_requested(&self) -> Option<bool> {
        self.test_signal_requested
    }
    pub fn observe_volume(&mut self, channel: Channel, volume: Volume) {
        match channel {
            Channel::Front => self.front_volume = Some(volume),
            Channel::Rear => self.rear_volume = Some(volume),
        }
    }
    pub fn execute(
        &mut self,
        change: &Change,
        transport: &mut impl Transport,
        mut validate: impl FnMut() -> Result<()>,
    ) -> Outcome {
        let mut outcome = Outcome {
            calls: Vec::new(),
            follow_ups: Vec::new(),
            completed: false,
            failure: None,
        };
        let result = (|| {
            validate()?;
            match change {
                Change::Enabled(enabled) => self.set_enabled(*enabled, transport, &mut outcome)?,
                Change::FrontSpeaker(endpoint) => {
                    outcome
                        .required(transport, NativeCall::SetFrontEndpoint(endpoint.id.clone()))?;
                    self.settings.front_speaker = Some(endpoint.clone());
                    if self.settings.enabled {
                        // eo starts Xn without awaiting it. The endpoint task
                        // completes independently; the service queues this work.
                        outcome.follow_ups.push(Change::Enabled(true));
                    }
                }
                Change::Volume { channel, volume } => {
                    if *channel == Channel::Rear && volume.percent() > 0 {
                        outcome.required(transport, NativeCall::SetRearMute(false))?;
                    }
                    outcome.required(
                        transport,
                        NativeCall::SetVolume {
                            channel: *channel,
                            volume: *volume,
                        },
                    )?;
                    self.observe_volume(*channel, *volume);
                }
                Change::Equalizer { channel, equalizer } => {
                    outcome.required(
                        transport,
                        NativeCall::SetEqualizer {
                            channel: *channel,
                            equalizer: *equalizer,
                        },
                    )?;
                    self.settings.apply(change);
                }
                Change::Delay(delay) => {
                    outcome.required(transport, NativeCall::SetDelay(*delay))?;
                    self.settings.apply(change);
                }
                Change::Replicate(enabled) => {
                    outcome.required(transport, NativeCall::SetReplicate(*enabled))?;
                    self.settings.apply(change);
                }
                Change::TestSignal(enabled) => {
                    outcome.required(transport, NativeCall::SetTestSignal(*enabled))?;
                    self.test_signal_requested = Some(*enabled);
                }
            }
            validate()?;
            Ok::<_, anyhow::Error>(())
        })();
        match result {
            Ok(()) => outcome.completed = true,
            Err(error) => outcome.failure = Some(format!("{error:#}")),
        }
        outcome
    }
    fn set_enabled(
        &mut self,
        enabled: bool,
        transport: &mut impl Transport,
        outcome: &mut Outcome,
    ) -> Result<()> {
        // Xn publishes this runtime field before checking the endpoint or ACK.
        self.settings.enabled = enabled;
        ensure!(
            !enabled || self.settings.front_speaker.is_some(),
            "Front speaker is not set"
        );
        outcome.required(transport, NativeCall::SetEnabled(enabled))?;
        for channel in [Channel::Rear, Channel::Front] {
            let equalizer = if enabled {
                self.settings.equalizer(channel)
            } else {
                Equalizer::FLAT
            };
            // Current Xn awaits both EQ calls but does not inspect their bools.
            // Keep each response for diagnostics without inventing rollback.
            outcome.call(transport, NativeCall::SetEqualizer { channel, equalizer })?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "surround_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "surround/protocol_tests.rs"]
mod protocol_tests;
