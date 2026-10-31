//! Help's current WindowStorage selection and SDK application record consumer.
use super::HelpRuntimeVersionStatus;
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use url::Url;

#[derive(Debug, Default)]
enum TextField {
    #[default]
    Missing,
    Text(String),
    Falsy,
    NonString,
}
impl<'de> Deserialize<'de> for TextField {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // JSON is confined to this external storage boundary. Unobserved and
        // malformed fields remain distinct before entering Help state.
        Ok(match Value::deserialize(deserializer)? {
            Value::Null => Self::Missing,
            Value::String(value) => Self::Text(value),
            Value::Bool(false) => Self::Falsy,
            Value::Number(value) if value.as_f64() == Some(0.) => Self::Falsy,
            _ => Self::NonString,
        })
    }
}
impl TextField {
    fn text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text),
            _ => None,
        }
    }
    fn version(&self) -> Result<Option<String>, String> {
        match self {
            Self::Text(text) if !text.is_empty() => Ok(Some(text.clone())),
            Self::Missing | Self::Falsy | Self::Text(_) => Ok(None),
            Self::NonString => Err("Runtime version is not a string".into()),
        }
    }
}

#[derive(Debug, Deserialize)]
struct WindowRecord {
    #[serde(default, rename = "windowName")]
    window_name: TextField,
    #[serde(default)]
    value: TextField,
}
#[derive(Debug)]
enum WindowEntry {
    Record(WindowRecord),
    Other,
}
impl<'de> Deserialize<'de> for WindowEntry {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Value::deserialize(deserializer)? {
            value @ Value::Object(_) => serde_json::from_value(value)
                .map(Self::Record)
                .map_err(serde::de::Error::custom),
            _ => Ok(Self::Other),
        }
    }
}
impl WindowEntry {
    fn url(&self) -> Option<Url> {
        let Self::Record(record) = self else {
            return None;
        };
        Url::parse(record.window_name.text()?).ok()
    }
    fn version(&self) -> Result<Option<String>, String> {
        match self {
            Self::Record(record) => record.value.version(),
            Self::Other => Ok(None),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Selection {
    First,
    Scoped,
}
fn selection(product_id: u32) -> Selection {
    match product_id {
        658 | 708 | 747 | 752 | 1442 | 1451 | 1453 | 2660 | 2664 | 2668 | 2689 | 3594 | 4144
        | 45066 => Selection::First,
        _ => Selection::Scoped,
    }
}
fn normalize_container(container: &str) -> String {
    container.replace(['{', '}'], "").to_lowercase()
}
fn source_product(url: &Url) -> Option<&str> {
    let mut segments = url.path().split('/').filter(|segment| !segment.is_empty());
    segments.find(|segment| segment.eq_ignore_ascii_case("products"))?;
    let product = segments.next()?;
    product
        .bytes()
        .all(|byte| byte.is_ascii_digit())
        .then_some(product)
}
fn scoped_version(
    entries: &[WindowEntry],
    container_id: &str,
    source_identity: Option<(u32, Option<u32>)>,
) -> Result<Option<String>, String> {
    let container = normalize_container(container_id);
    if !container.is_empty()
        && let Some(entry) = entries.iter().find(|entry| {
            entry.url().is_some_and(|url| {
                let query = url
                    .query_pairs()
                    .find(|(key, _)| key == "containerId")
                    .map(|(_, value)| value.into_owned());
                query.is_some_and(|value| normalize_container(&value) == container)
            })
        })
    {
        // A container match takes precedence even when its version is absent
        // or its producer URL names a different product.
        return entry.version();
    }
    let (product_id, dongle_id) = source_identity
        .ok_or_else(|| "Current UI DeviceInfo identity is unavailable".to_owned())?;
    let product = product_id.to_string();
    let dongle = dongle_id.map(|id| id.to_string());
    entries
        .iter()
        .find(|entry| {
            entry.url().is_some_and(|url| {
                source_product(&url).is_some_and(|candidate| {
                    candidate == product || dongle.as_deref() == Some(candidate)
                })
            })
        })
        .map(WindowEntry::version)
        .unwrap_or(Ok(None))
}

fn read(
    raw: Result<Option<String>, String>,
    parse: impl FnOnce(&str) -> Result<Option<String>, String>,
) -> HelpRuntimeVersionStatus {
    let result = match raw {
        Ok(Some(raw)) if !raw.is_empty() => parse(&raw),
        Ok(_) => Ok(None),
        Err(error) => Err(error),
    };
    match result {
        Ok(Some(version)) => HelpRuntimeVersionStatus::Observed(version),
        Ok(None) => HelpRuntimeVersionStatus::Missing,
        Err(error) => HelpRuntimeVersionStatus::Failed(error),
    }
}

pub(super) fn middleware_version(
    product_id: u32,
    container_id: &str,
    raw: Result<Option<String>, String>,
) -> HelpRuntimeVersionStatus {
    read(raw, |raw| {
        let document: Value = serde_json::from_str(raw)
            .map_err(|error| format!("Invalid middleware WindowStorage records: {error}"))?;
        // The scoped source helper checks Array.isArray before examining the
        // records. A valid non-array document supplies no observed version.
        if selection(product_id) == Selection::Scoped && !document.is_array() {
            return Ok(None);
        }
        let entries: Vec<WindowEntry> = serde_json::from_value(document)
            .map_err(|error| format!("Invalid middleware WindowStorage records: {error}"))?;
        match selection(product_id) {
            Selection::First => entries
                .first()
                .map(WindowEntry::version)
                .unwrap_or(Ok(None)),
            Selection::Scoped => {
                let source = razer_device::device_identity::ui_product_identity(product_id);
                scoped_version(
                    &entries,
                    container_id,
                    source.map(|source| (source.product_id, source.dongle_id)),
                )
            }
        }
    })
}

#[derive(Deserialize)]
struct AppRecord {
    #[serde(default)]
    name: TextField,
    #[serde(default)]
    version: TextField,
}
enum AppEntry {
    Record(AppRecord),
    Null,
    Other,
}
impl<'de> Deserialize<'de> for AppEntry {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        match Value::deserialize(deserializer)? {
            value @ Value::Object(_) => serde_json::from_value(value)
                .map(Self::Record)
                .map_err(serde::de::Error::custom),
            Value::Null => Ok(Self::Null),
            _ => Ok(Self::Other),
        }
    }
}
pub(super) fn application_version(raw: Result<Option<String>, String>) -> HelpRuntimeVersionStatus {
    read(raw, |raw| {
        let entries: Vec<AppEntry> = serde_json::from_str(raw)
            .map_err(|error| format!("Invalid SDK application storage records: {error}"))?;
        for entry in entries {
            match entry {
                AppEntry::Record(app) if app.name.text() == Some("synapse") => {
                    // Source Array.find keeps the first same-name record even
                    // when it has no version. It never supplies the UI build.
                    return app.version.version().map(|version| {
                        version.map(|version| {
                            std::iter::once("4")
                                .chain(version.split('.').skip(1))
                                .collect::<Vec<_>>()
                                .join(".")
                        })
                    });
                }
                AppEntry::Null => {
                    return Err("SDK application lookup encountered a null record".into());
                }
                _ => {}
            }
        }
        Ok(None)
    })
}

#[cfg(test)]
#[path = "source_help_version_storage_tests.rs"]
mod tests;
