//! Current connection observations, independent of profile drafts and routing intent.
use super::{EndpointObservation, HEADPHONES, MICROPHONE};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum Reading<T> {
    #[default]
    Unobserved,
    Reading,
    Observed(T),
    Failed(String),
    Disconnected,
}
impl<T> Reading<T> {
    pub fn value(&self) -> Option<&T> {
        match self {
            Self::Observed(value) => Some(value),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LiveState {
    microphone: Reading<EndpointObservation>,
    headphones: Reading<EndpointObservation>,
    meters: Reading<()>,
}
impl LiveState {
    pub fn endpoint(&self, id: u32) -> Option<&Reading<EndpointObservation>> {
        match id {
            MICROPHONE => Some(&self.microphone),
            HEADPHONES => Some(&self.headphones),
            _ => None,
        }
    }
    pub fn meters(&self) -> &Reading<()> {
        &self.meters
    }
    pub fn observe_endpoint(&mut self, id: u32, value: EndpointObservation) -> Result<()> {
        ensure!(
            value.level <= 100,
            "Marie endpoint volume exceeds source range"
        );
        self.set_endpoint(id, Reading::Observed(value));
        Ok(())
    }
    pub fn set_endpoint(&mut self, id: u32, value: Reading<EndpointObservation>) {
        match id {
            MICROPHONE => self.microphone = value,
            HEADPHONES => self.headphones = value,
            _ => {}
        }
    }
    pub fn set_meters(&mut self, value: Reading<()>) {
        self.meters = value;
    }
    pub fn validate(&self) -> Result<()> {
        for value in [&self.microphone, &self.headphones] {
            if let Some(value) = value.value() {
                ensure!(value.level <= 100, "Invalid Marie volume observation");
            }
        }
        Ok(())
    }
    pub fn disconnected() -> Self {
        Self {
            microphone: Reading::Disconnected,
            headphones: Reading::Disconnected,
            meters: Reading::Disconnected,
        }
    }
    pub fn failed(error: String) -> Self {
        Self {
            microphone: Reading::Failed(error.clone()),
            headphones: Reading::Failed(error.clone()),
            meters: Reading::Failed(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn seiren_live_failure_never_keeps_a_previous_value_or_invents_a_peer() {
        let mut state = LiveState::default();
        state
            .observe_endpoint(
                MICROPHONE,
                EndpointObservation {
                    level: 37,
                    muted: true,
                },
            )
            .unwrap();
        assert_eq!(
            state.endpoint(MICROPHONE).unwrap().value().unwrap().level,
            37
        );
        assert_eq!(state.endpoint(HEADPHONES), Some(&Reading::Unobserved));
        state.set_endpoint(MICROPHONE, Reading::Failed("endpoint removed".into()));
        assert!(state.endpoint(MICROPHONE).unwrap().value().is_none());
        assert!(
            LiveState::disconnected()
                .endpoint(MICROPHONE)
                .unwrap()
                .value()
                .is_none()
        );
        state
            .observe_endpoint(
                MICROPHONE,
                EndpointObservation {
                    level: 61,
                    muted: false,
                },
            )
            .unwrap();
        assert_eq!(
            state.endpoint(MICROPHONE).unwrap().value().unwrap().level,
            61
        );
    }
    #[test]
    fn seiren_live_ipc_requires_fields_and_validates_ranges() {
        assert!(serde_json::from_str::<LiveState>(r#"{"meters":"Unobserved"}"#).is_err());
        let state: LiveState = serde_json::from_str(r#"{"microphone":{"Observed":{"level":101,"muted":false}},"headphones":"Unobserved","meters":"Unobserved"}"#).unwrap();
        assert!(state.validate().is_err());
    }
}
