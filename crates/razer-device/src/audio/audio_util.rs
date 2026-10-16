//! Current RzAudioUtil 1.0.3.1 AudioEnumerator semantics, independent of COM.
//!
//! Evidence: docs/re/audio-util-enumerator-current-evidence.json. These are
//! system audio endpoints, not USB products or Mixer jack identifiers.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioFlow {
    Playback,
    Record,
}

impl AudioFlow {
    /// Verified EDataFlow passed to EnumAudioEndpoints and default queries.
    pub const fn native_value(self) -> i32 {
        match self {
            Self::Playback => 0,
            Self::Record => 1,
        }
    }

    pub const fn source_command(self) -> &'static str {
        match self {
            Self::Playback => "GetWindowsPlaybackDevices",
            Self::Record => "GetWindowsRecordDevices",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AudioEndpoint {
    pub id: String,
    pub friendly_name: String,
    pub description: String,
    pub default_console: bool,
    pub default_communications: bool,
}

impl AudioEndpoint {
    /// RVA 0x4542A..0x45499 compares full UTF-16 IDs without case folding.
    pub fn observed(
        id: String,
        friendly_name: String,
        description: String,
        console_id: Option<&str>,
        communications_id: Option<&str>,
    ) -> Self {
        // The native temporary default IDs start empty, even when a default
        // endpoint lookup fails. Retain that exact empty-ID comparison.
        let default_console = console_id.unwrap_or_default() == id;
        let default_communications = communications_id.unwrap_or_default() == id;
        Self {
            id,
            friendly_name,
            description,
            default_console,
            default_communications,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioEnumeration {
    pub flow: AudioFlow,
    pub endpoints: Vec<AudioEndpoint>,
    /// Source skips unavailable/default endpoint and invalid item properties.
    /// Preserve those observations alongside the source-compatible name list.
    pub diagnostics: Vec<String>,
}

impl AudioEnumeration {
    /// RVA 0x21E0 / 0x2750 reads each record +0x20 friendly name, preserving
    /// collection order, empty names, and duplicates. No implicit PID filter.
    pub fn source_response(&self) -> Value {
        json!({"devices": self.endpoints.iter().map(|e| e.friendly_name.as_str()).collect::<Vec<_>>()})
    }
}

/// The binary accepts S_OK and INPLACE_S_TRUNCATED for GetValue; other positive
/// HRESULTs are not accepted by its exact comparisons (0x45511 / 0x4568F).
pub const fn accepts_property_result(hresult: i32) -> bool {
    hresult == 0 || hresult == 0x0004_01a0
}
