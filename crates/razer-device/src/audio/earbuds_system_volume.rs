//! Earbud Sound volume is an observed Windows endpoint, never a saved profile.
use crate::simple_audio::{EndpointType, SimpleAudioEndpoint};
use crate::simple_audio_volume::{AudioVolumeReadResult, AudioVolumeWriteResult};
use anyhow::{Result, ensure};
use uuid::Uuid;

pub fn supports(product_id: u32) -> bool {
    matches!(product_id, 2657 | 2660 | 2664 | 2668 | 2689)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scope {
    id: Uuid,
    product_id: u32,
    container_id: String,
}
impl Scope {
    pub fn new(product_id: u32, container_id: String) -> Result<Self> {
        ensure!(
            supports(product_id),
            "Product has no reviewed earbud volume chain"
        );
        ensure!(
            !container_id.is_empty()
                && !container_id.contains('\0')
                && !container_id.starts_with("hid-collection:"),
            "Earbud volume requires an observed device container"
        );
        Ok(Self {
            id: Uuid::new_v4(),
            product_id,
            container_id,
        })
    }
    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn product_id(&self) -> u32 {
        self.product_id
    }
    pub fn container_id(&self) -> &str {
        &self.container_id
    }
}

/// getRenderDevice filters speakers before the first container substring match.
/// Input retains simple_service's source hash-table order. There is no name fallback.
pub fn select(scope: &Scope, endpoints: Vec<SimpleAudioEndpoint>) -> Result<SimpleAudioEndpoint> {
    endpoints
        .into_iter()
        .find(|endpoint| {
            endpoint.endpoint_type == EndpointType::Speaker
                && endpoint.container_id.contains(scope.container_id())
                && !endpoint.id.is_empty()
        })
        .ok_or_else(|| anyhow::anyhow!("No speaker endpoint matches the observed earbud container"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Setting {
    enabled: bool,
    volume: u8,
}
impl Setting {
    pub fn new(enabled: bool, volume: u8) -> Result<Self> {
        ensure!(volume <= 100, "Earbud volume is outside 0..100");
        Ok(Self { enabled, volume })
    }
    pub fn enabled(self) -> bool {
        self.enabled
    }
    pub fn volume(self) -> u8 {
        self.volume
    }
    pub fn edit(self, edit: Edit) -> Result<Self> {
        edit.validate()?;
        match edit {
            Edit::Volume(volume) => Ok(Self { volume, ..self }),
            Edit::Enabled(enabled) => Ok(Self { enabled, ..self }),
        }
    }
    pub fn needs_write(self, previous: Self) -> bool {
        self.enabled != previous.enabled || (self.enabled && self.volume != previous.volume)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edit {
    Volume(u8),
    Enabled(bool),
}
impl Edit {
    pub fn validate(self) -> Result<()> {
        if let Self::Volume(volume) = self {
            ensure!(volume <= 100, "Earbud volume is outside 0..100");
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct Snapshot {
    endpoint: SimpleAudioEndpoint,
    setting: Setting,
}
impl Snapshot {
    pub fn from_read(
        endpoint: SimpleAudioEndpoint,
        reading: AudioVolumeReadResult,
    ) -> Result<Self> {
        ensure!(
            reading.device_id == endpoint.id,
            "Volume response belongs to another endpoint"
        );
        ensure!(
            reading.result,
            "Speaker volume read failed: {}",
            reading.reason
        );
        Ok(Self {
            endpoint,
            setting: Setting::new(!reading.muted, reading.volume)?,
        })
    }
    pub fn endpoint(&self) -> &SimpleAudioEndpoint {
        &self.endpoint
    }
    pub fn setting(&self) -> Setting {
        self.setting
    }
}

#[derive(Clone, Debug)]
pub enum Mutation {
    Unchanged,
    Written(AudioVolumeWriteResult),
}

#[derive(Clone, Debug)]
pub struct Update {
    request_id: Option<Uuid>,
    observation: Result<Snapshot, String>,
    mutation: Option<Mutation>,
    error: Option<String>,
}
impl Update {
    pub fn new(observation: Result<Snapshot, String>) -> Self {
        Self {
            request_id: None,
            observation,
            mutation: None,
            error: None,
        }
    }
    pub fn with_request_id(mut self, id: Uuid) -> Self {
        self.request_id = Some(id);
        self
    }
    pub fn with_mutation(mut self, mutation: Option<Mutation>) -> Self {
        self.mutation = mutation;
        self
    }
    pub fn with_error(mut self, error: Option<String>) -> Self {
        self.error = error;
        self
    }
    pub fn request_id(&self) -> Option<Uuid> {
        self.request_id
    }
    pub fn observation(&self) -> &Result<Snapshot, String> {
        &self.observation
    }
    pub fn mutation(&self) -> Option<&Mutation> {
        self.mutation.as_ref()
    }
    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn endpoint(id: &str, container: &str, kind: EndpointType) -> SimpleAudioEndpoint {
        SimpleAudioEndpoint {
            id: id.into(),
            name: "same model name".into(),
            container_id: container.into(),
            endpoint_type: kind,
        }
    }
    #[test]
    fn all_five_variants_use_first_speaker_substring_match_without_name_fallback() {
        for pid in [2657, 2660, 2664, 2668, 2689] {
            let scope = Scope::new(pid, "CONTAINER".into()).unwrap();
            let endpoints = vec![
                endpoint("mic", "CONTAINER", EndpointType::Microphone),
                endpoint("first", "prefix-CONTAINER-suffix", EndpointType::Speaker),
                endpoint("second", "CONTAINER", EndpointType::Speaker),
            ];
            assert_eq!(select(&scope, endpoints).unwrap().id, "first");
            assert!(
                select(
                    &scope,
                    vec![endpoint("other", "unrelated", EndpointType::Speaker)]
                )
                .is_err()
            );
        }
    }
    #[test]
    fn zero_volume_preserves_enabled_and_muted_volume_only_edits_do_not_write() {
        let previous = Setting::new(true, 37).unwrap();
        let zero = previous.edit(Edit::Volume(0)).unwrap();
        assert!(zero.enabled());
        assert!(zero.needs_write(previous));
        let muted = Setting::new(false, 37).unwrap();
        assert!(!muted.edit(Edit::Volume(55)).unwrap().needs_write(muted));
        assert!(muted.edit(Edit::Enabled(true)).unwrap().needs_write(muted));
        assert!(Edit::Volume(101).validate().is_err());
    }
    #[test]
    fn failed_or_misrouted_reads_cannot_become_observations() {
        let ep = endpoint("real", "CONTAINER", EndpointType::Speaker);
        let read = AudioVolumeReadResult {
            result: true,
            reason: String::new(),
            device_id: "other".into(),
            muted: false,
            volume: 87,
        };
        assert!(Snapshot::from_read(ep.clone(), read.clone()).is_err());
        assert!(
            Snapshot::from_read(
                ep.clone(),
                AudioVolumeReadResult {
                    device_id: "real".into(),
                    result: false,
                    ..read.clone()
                }
            )
            .is_err()
        );
        assert_eq!(
            Snapshot::from_read(
                ep,
                AudioVolumeReadResult {
                    device_id: "real".into(),
                    ..read
                }
            )
            .unwrap()
            .setting()
            .volume(),
            87
        );
    }
}
