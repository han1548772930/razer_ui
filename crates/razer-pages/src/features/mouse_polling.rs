//! Shared current frontend polling selectors and runtime observations.
//! CONFIG describes choices; only real producer observations describe a device.
use gpui_kit::EntityId;
use serde::Deserialize;
use serde_json::Value;
use std::{collections::BTreeMap, sync::OnceLock};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PollingConnection {
    Wired,
    Dongle,
    Ble,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum PollingField {
    Wired,
    Wireless,
}
impl PollingField {
    pub fn key(self) -> &'static str {
        match self {
            Self::Wired => "pollingRate",
            Self::Wireless => "pollingRateWireless",
        }
    }
    pub fn rates(self) -> &'static str {
        match self {
            Self::Wired => "POLLING_RATE",
            Self::Wireless => "POLLING_RATE_WIRELESS",
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct MousePollingScope {
    owner: EntityId,
    profile_epoch: u64,
    connection_epoch: u64,
}
impl MousePollingScope {
    pub fn new(owner: EntityId, profile_epoch: u64, connection_epoch: u64) -> Self {
        Self {
            owner,
            profile_epoch,
            connection_epoch,
        }
    }
}
#[derive(Clone)]
#[allow(dead_code)]
pub enum MousePollingObservation {
    Connection(Option<PollingConnection>),
    Rate(PollingField, u32),
    /// Current ordered Object.values(localStorage["duallink-devices"]).
    DualLinkSnapshot(Vec<Value>),
    /// Actual current product firmware from DEVICE_RUNTIME_DATA.
    FirmwareVersion(Option<String>),
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum PairIdentity {
    StrictDongleId,
    CurrentDeviceIds,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum BleVisibility {
    Hidden,
}
#[derive(Deserialize)]
struct LinkLimits {
    dual_link_hz: u32,
    dock_hz: u32,
    dock_min_devices: usize,
}
#[derive(Deserialize)]
pub struct SourceSpec {
    pub product_id: u32,
    device_ids: Vec<u32>,
    pairing_dongle_id: u32,
    pair_identity: PairIdentity,
    master_supports_8k_flag: bool,
    is_dongle_hyperpolling_device: bool,
    rates: BTreeMap<String, Vec<u32>>,
    firmware_floor: Option<String>,
    hyper_master_ids: Vec<u32>,
    pub high_rate_threshold_hz: u32,
    pub fallback_wireless_hz: u32,
    ble_visibility: BleVisibility,
    limits: Option<LinkLimits>,
}
pub fn source_spec(product_id: u32) -> Option<&'static SourceSpec> {
    static SOURCES: OnceLock<Vec<SourceSpec>> = OnceLock::new();
    SOURCES
        .get_or_init(|| {
            serde_json::from_str(include_str!("mouse_polling_source_data.json"))
                .expect("current source polling data")
        })
        .iter()
        .find(|spec| spec.product_id == product_id)
}

fn ids(value: &Value) -> impl Iterator<Item = &Value> {
    [value.get("productId"), value.get("dongleId")]
        .into_iter()
        .flatten()
        .filter(|id| !id.is_null())
}
fn strict_id(left: &Value, right: &Value) -> bool {
    if left.is_number() && right.is_number() {
        left.as_f64() == right.as_f64()
    } else {
        left == right
    }
}
fn truthy_id(value: &&Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(v) => *v,
        Value::Number(v) => v.as_f64() != Some(0.),
        Value::String(v) => !v.is_empty(),
        _ => true,
    }
}
impl SourceSpec {
    fn current_device(&self, value: &Value) -> bool {
        ids(value).any(|id| {
            self.device_ids.iter().any(|expected| match id {
                Value::String(value) => *value == expected.to_string(),
                Value::Number(value) => value.as_f64() == Some(*expected as f64),
                _ => false,
            })
        })
    }
    fn pair<'a>(&self, rows: &'a [Value]) -> Option<&'a Value> {
        rows.iter().find(|row| match self.pair_identity {
            PairIdentity::StrictDongleId => {
                row["dongleId"].as_f64() == Some(self.pairing_dongle_id as f64)
            }
            PairIdentity::CurrentDeviceIds => {
                let slave = row
                    .get("slave")
                    .filter(|value| !value.is_null())
                    .unwrap_or(row);
                self.current_device(slave) || self.current_device(&row["master"])
            }
        })
    }
    fn hyper_master(&self, pair: &Value) -> bool {
        let master = &pair["master"];
        self.hyper_master_ids
            .iter()
            .any(|id| master["productId"].as_f64() == Some(*id as f64))
            || self.master_supports_8k_flag && master["supports8KHzPollingRate"] == true
    }
    fn rate_choices(&self, key: &str) -> &[u32] {
        self.rates.get(key).map(Vec::as_slice).unwrap_or(&[])
    }
}

#[derive(Clone, Copy)]
pub enum LimitKind {
    DualLink,
    MultiDeviceDock,
}
#[derive(Clone, Copy)]
pub struct Limit {
    pub kind: LimitKind,
    pub hz: u32,
}
impl Limit {
    pub fn label(self) -> &'static str {
        match self.kind {
            LimitKind::DualLink => "POLLING_RATE_DUAL_LINK_LIMITED",
            LimitKind::MultiDeviceDock => "POLLING_RATE_MULTI_DEVICE_DOCK_LIMITED_MOUSE",
        }
    }
}
#[derive(Default)]
pub struct RuntimeState {
    pub connection: Option<PollingConnection>,
    pub profile_epoch: u64,
    pub connection_epoch: u64,
    pub rates: BTreeMap<PollingField, u32>,
    pub topology: Option<Vec<Value>>,
    pub firmware: Option<String>,
}
impl RuntimeState {
    pub fn reset_profile(&mut self) {
        self.profile_epoch = self.profile_epoch.wrapping_add(1);
        self.rates.clear();
    }
    pub fn apply(&mut self, observation: MousePollingObservation) {
        match observation {
            MousePollingObservation::Connection(connection) => {
                self.connection = connection;
                self.connection_epoch = self.connection_epoch.wrapping_add(1);
                self.rates.clear();
                self.topology = None;
                self.firmware = None;
            }
            MousePollingObservation::Rate(field, rate) => {
                if rate > 0 {
                    self.rates.insert(field, rate);
                }
            }
            MousePollingObservation::DualLinkSnapshot(rows) => self.topology = Some(rows),
            MousePollingObservation::FirmwareVersion(firmware) => self.firmware = firmware,
        }
    }
    pub fn field(&self) -> PollingField {
        if self.connection == Some(PollingConnection::Dongle) {
            PollingField::Wireless
        } else {
            PollingField::Wired
        }
    }
    pub fn visible(&self, spec: &SourceSpec) -> bool {
        match spec.ble_visibility {
            BleVisibility::Hidden => self.connection != Some(PollingConnection::Ble),
        }
    }
    pub fn has_hyper_master(&self, spec: &SourceSpec) -> bool {
        self.topology
            .as_deref()
            .and_then(|rows| spec.pair(rows))
            .is_some_and(|pair| spec.hyper_master(pair))
    }
    pub fn limit(&self, spec: &SourceSpec) -> Option<Limit> {
        if self.connection != Some(PollingConnection::Dongle) {
            return None;
        }
        let limits = spec.limits.as_ref()?;
        let rows = self.topology.as_deref()?;
        let pair = spec.pair(rows)?;
        let master = &pair["master"];
        if master["supports8KHzPollingRate"] == true {
            let count = rows
                .iter()
                .filter(|row| {
                    ids(master).filter(truthy_id).any(|id| {
                        ids(&row["master"])
                            .filter(truthy_id)
                            .any(|other| strict_id(id, other))
                    })
                })
                .count();
            if count >= limits.dock_min_devices {
                return Some(Limit {
                    kind: LimitKind::MultiDeviceDock,
                    hz: limits.dock_hz,
                });
            }
        }
        (!spec.hyper_master(pair)).then_some(Limit {
            kind: LimitKind::DualLink,
            hz: limits.dual_link_hz,
        })
    }
    pub fn rate_choices<'a>(&self, spec: &'a SourceSpec) -> &'a [u32] {
        let field = self.field();
        let pair = self.topology.as_deref().and_then(|rows| spec.pair(rows));
        let hyper = pair
            .map(|pair| spec.hyper_master(pair))
            .unwrap_or(spec.is_dongle_hyperpolling_device);
        if field == PollingField::Wireless
            && self.limit(spec).is_none()
            && hyper
            && spec.firmware_floor.is_some()
        {
            return spec.rate_choices(
                if source_firmware_supported(
                    self.firmware.as_deref(),
                    spec.firmware_floor.as_deref(),
                ) {
                    "HYPER_POLLING_RATE_SUPPORT_8K"
                } else {
                    "HYPER_POLLING_RATE"
                },
            );
        }
        spec.rate_choices(field.rates())
    }
}
fn source_firmware_supported(observed: Option<&str>, required: Option<&str>) -> bool {
    fn valid(value: &str) -> Option<semver::Version> {
        // Current npm SemVer modules 8490/8816: MAX_LENGTH and MAX_SAFE_INTEGER.
        if value.encode_utf16().count() > 256 {
            return None;
        }
        let value = value.trim();
        let version = semver::Version::parse(value.strip_prefix('v').unwrap_or(value)).ok()?;
        if [version.major, version.minor, version.patch]
            .iter()
            .any(|part| *part > 9_007_199_254_740_991)
        {
            return None;
        }
        Some(version)
    }
    fn coerce(value: &str) -> Option<semver::Version> {
        // Current 8988 -> 1468 COERCE, leftmost match and ASCII JS digits.
        static COERCE: OnceLock<regex::Regex> = OnceLock::new();
        let regex = COERCE.get_or_init(|| {
            regex::Regex::new(
                r"(^|[^0-9])([0-9]{1,16})(?:\.([0-9]{1,16}))?(?:\.([0-9]{1,16}))?(?:$|[^0-9])",
            )
            .expect("source coerce expression")
        });
        let captures = regex.captures(value)?;
        valid(&format!(
            "{}.{}.{}",
            &captures[2],
            captures.get(3).map_or("0", |v| v.as_str()),
            captures.get(4).map_or("0", |v| v.as_str())
        ))
    }
    let (Some(observed), Some(required)) = (observed.filter(|value| !value.is_empty()), required)
    else {
        return false;
    };
    if required == observed {
        return true;
    }
    if let (Some(required), Some(observed)) = (valid(required), valid(observed)) {
        return !required.cmp_precedence(&observed).is_gt();
    }
    // U5 returns false if coercion fails, and checkFor8KSupport negates it.
    // Preserve that source result; it is not a successful firmware/device read.
    let (Some(required), Some(observed)) = (coerce(required), coerce(observed)) else {
        return true;
    };
    // U5's o===n compares two fresh SemVer objects, not version strings. Its
    // following fourth-segment <= branch is unreachable for these string inputs.
    observed.cmp_precedence(&required).is_gt()
}
