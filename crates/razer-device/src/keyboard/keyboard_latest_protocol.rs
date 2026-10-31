//! Current756/757 rzDevice25 and 758 rzDevice30 profile restore transport.
//! The actual adapter determines retries/delays. Partial AT/RT ACKs remain
//! explicit and never clear a restore journal or produce synthetic readback.
use crate::{
    backend::{FeatureTransport, HidNode},
    device_reads::{DeviceReadCapability, DeviceReadTarget},
    feature_exchange,
    keyboard_analog::{ActuationPoint, RapidTriggerKey},
    keyboard_latest_restore::{self, Slot},
    mouse_runtime_artwork::WindowsBleTarget,
};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Target {
    Collection(HidNode),
    WindowsContainer(DeviceReadTarget),
    WindowsBle(WindowsBleTarget),
}
impl Target {
    pub fn physical_product_id(&self) -> u32 {
        match self {
            Self::Collection(node) => u32::from(node.product_id),
            Self::WindowsContainer(target) => target.physical_product_id,
            Self::WindowsBle(target) => target.physical_product_id,
        }
    }
    pub fn ble(&self) -> bool {
        matches!(self, Self::WindowsBle(_))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Operation {
    Events {
        enabled: Option<bool>,
    },
    Inventory,
    WriteProfileMetadata(ProfileMetadata),
    Actuation {
        slot: u8,
        points: Vec<ActuationPoint>,
    },
    RapidTrigger {
        slot: u8,
        keys: Vec<RapidTriggerKey>,
    },
    ReadActuation {
        slot: u8,
        keys: Vec<u8>,
    },
    ReadRapidTrigger {
        slot: u8,
        keys: Vec<u8>,
    },
    ReadGameModeSelection {
        slot: u8,
    },
    ReadPollingRate {
        slot: u8,
    },
    ReadSnapTap {
        slot: u8,
    },
    ReadSingleKeySnapTap {
        slot: u8,
    },
    ReadTimeToDim {
        slot: u8,
    },
    ReadAutoSleepMode {
        slot: u8,
    },
    ReadTimeToSleep,
    BleSlot {
        set: Option<u8>,
    },
    Setting(Setting),
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Setting {
    /// Current2050 setSingleButtonMapping -> rzDevice25.setSingleKeyAssignment.
    /// AnalogKeyboardV3 uses this ordinary assignment command, with an
    /// 80-byte payload and a separate source packetSizeSet of ten bytes.
    ButtonAssignment {
        slot: u8,
        key_id: u8,
        mode: u8,
        fn_id: u8,
        data_size: u8,
        data: [u8; 5],
    },
    AdjustmentMode {
        slot: u8,
        enabled: bool,
    },
    Brightness {
        slot: u8,
        region: u8,
        percent: f64,
    },
    PollingRate {
        slot: u8,
        hertz: u32,
    },
    GameModeState(u8),
    LedState {
        class: u8,
        led: u8,
        enabled: bool,
    },
    GameModeSelection {
        slot: u8,
        selection: u8,
        extension: Option<u8>,
    },
    CustomGameKeys {
        slot: u8,
        combos: [[u8; 3]; 3],
    },
    TimeToDim {
        slot: u8,
        enabled: bool,
        seconds: u16,
        brightness: u8,
    },
    TimeToSleep {
        seconds: u16,
    },
    AutoSleepMode {
        slot: u8,
        enabled: bool,
    },
    PowerMode(u8),
    InterruptKeys {
        slot: u8,
        dynamic_keystroke: bool,
        enabled: bool,
        mode: u8,
        keys: Vec<u8>,
    },
    ContinuousRapidTrigger {
        slot: u8,
        enabled: bool,
    },
    SnapTap {
        slot: u8,
        enabled: bool,
        pairs: Vec<[u8; 3]>,
    },
    BottomDeadzone {
        slot: u8,
        value: u8,
    },
    SingleKeySnapTap {
        slot: u8,
        enabled: bool,
        keys: Vec<[u8; 2]>,
    },
    DualKeypressPriority {
        slot: u8,
        enabled: bool,
        keys: Vec<u8>,
    },
    AnalogReportMode(u8),
    JoystickReportMode(u8),
    JoystickMappingMode {
        slot: u8,
        mode: u8,
    },
    TriggerPresetEffect {
        slot: u8,
        key: u8,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChunkReceipt {
    pub offset: usize,
    pub count: usize,
    pub acknowledgement: Option<Vec<u8>>,
    pub error: Option<String>,
}

#[derive(Debug)]
struct FailedWrite {
    chunks: Vec<ChunkReceipt>,
}

impl std::fmt::Display for FailedWrite {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Keyboard write failed with retained chunk evidence")
    }
}

/// Retained chunks describe partial transport progress, never a completed write.
pub fn failed_write(error: &anyhow::Error) -> Option<&[ChunkReceipt]> {
    error
        .downcast_ref::<FailedWrite>()
        .map(|failure| failure.chunks.as_slice())
}

fn write_failure(error: anyhow::Error, chunks: &[ChunkReceipt]) -> anyhow::Error {
    error.context(FailedWrite {
        chunks: chunks.to_vec(),
    })
}

fn retain_chunk(
    chunks: &mut Vec<ChunkReceipt>,
    offset: usize,
    count: usize,
    result: anyhow::Result<Vec<u8>>,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let acknowledgement = feature_exchange::acknowledged_reply(&result).map(<[u8]>::to_vec);
    chunks.push(ChunkReceipt {
        offset,
        count,
        acknowledgement,
        error: result.as_ref().err().map(|error| format!("{error:#}")),
    });
    let operation_error = match result {
        Err(error) if crate::backend::exchange_interrupted(&error) => {
            return Err(write_failure(error, chunks));
        }
        Err(error) => Some(error),
        Ok(_) => None,
    };
    if let Err(validation) = validate() {
        let error = match operation_error {
            Some(error) => error.context(format!(
                "Keyboard write owner is no longer current: {validation:#}"
            )),
            None => validation,
        };
        chunks.last_mut().unwrap().error = Some(format!("{error:#}"));
        return Err(write_failure(
            crate::backend::interrupt_exchange(
                error,
                crate::backend::ExchangeInterruption::Validation,
            ),
            chunks,
        ));
    }
    Ok(())
}

/// Current obmEngineKeyboard.getProfileNameStructure, including its UTF-8
/// name field and the separate byte-string owner. This is actual readback.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileMetadata {
    pub slot_id: u8,
    pub guid: String,
    pub name: String,
    pub owner: String,
    pub key_mapping_hash: [u8; 6],
}
pub fn profile_metadata(slot_id: u8, bytes: &[u8]) -> anyhow::Result<ProfileMetadata> {
    ensure!(slot_id > 0, "756 onboard profile metadata slot is zero");
    let identity = crate::keyboard_profile_inventory::decode_identity(bytes)?;
    Ok(ProfileMetadata {
        slot_id,
        guid: identity.guid,
        name: identity.name,
        owner: identity.owner,
        key_mapping_hash: bytes[244..250].try_into()?,
    })
}

/// Current obmEngineKeyboard.setProfileNameStructure. The 16-byte GUID has
/// mixed endianness; UTF-8 name bytes are truncated at 100, owner uses the
/// source byte-string conversion, and the existing six-byte mapping hash is
/// preserved independently of the display name.
pub fn profile_metadata_bytes(metadata: &ProfileMetadata) -> anyhow::Result<[u8; 250]> {
    ensure!(metadata.slot_id > 0, "756 metadata slot is zero");
    let guid = metadata.guid.replace('-', "");
    ensure!(
        guid.len() == 32 && guid.is_ascii(),
        "756 metadata GUID is invalid"
    );
    let mut bytes = [0u8; 250];
    for (index, output) in bytes[..16].iter_mut().enumerate() {
        *output = u8::from_str_radix(&guid[index * 2..index * 2 + 2], 16)?;
    }
    bytes[..4].reverse();
    bytes[4..6].reverse();
    bytes[6..8].reverse();
    let name = metadata.name.as_bytes();
    let count = name.len().min(100);
    bytes[16..16 + count].copy_from_slice(&name[..count]);
    let owner = metadata.owner.encode_utf16().collect::<Vec<_>>();
    ensure!(
        owner.len() <= 128,
        "756 metadata owner exceeds source bounds"
    );
    for (output, unit) in bytes[116..244].iter_mut().zip(owner) {
        *output = unit as u8;
    }
    bytes[244..250].copy_from_slice(&metadata.key_mapping_hash);
    Ok(bytes)
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Reply {
    Events {
        packets: Vec<Vec<u8>>,
    },
    Inventory {
        slots: Vec<Slot>,
        metadata: Vec<Vec<u8>>,
    },
    Write {
        chunks: Vec<ChunkReceipt>,
    },
    Actuation {
        points: Vec<ActuationPoint>,
    },
    RapidTrigger {
        keys: Vec<RapidTriggerKey>,
    },
    GameModeSelection {
        slot: u8,
        selection: u8,
        extension: u8,
        bytes: Vec<u8>,
    },
    PollingRate {
        slot: u8,
        hertz: u32,
        bytes: Vec<u8>,
    },
    SnapTap {
        slot: u8,
        enabled: bool,
        pairs: Vec<[u8; 3]>,
        bytes: Vec<u8>,
    },
    SingleKeySnapTap {
        slot: u8,
        enabled: bool,
        keys: Vec<[u8; 2]>,
        bytes: Vec<u8>,
    },
    TimeToDim {
        slot: u8,
        enabled: bool,
        seconds: u16,
        brightness: u8,
        bytes: Vec<u8>,
    },
    AutoSleepMode {
        slot: u8,
        enabled: bool,
        bytes: Vec<u8>,
    },
    TimeToSleep {
        seconds: u16,
        bytes: Vec<u8>,
    },
    BleSlot {
        slot: Option<u8>,
        acknowledgement: Vec<u8>,
    },
}
impl Reply {
    pub fn acknowledged(&self) -> bool {
        matches!(self,Self::Write{chunks} if !chunks.is_empty()&&chunks.iter().all(|chunk|chunk.acknowledgement.is_some()&&chunk.error.is_none()))
    }
    pub fn require_acknowledged(&self) -> anyhow::Result<()> {
        ensure!(
            self.acknowledged(),
            "756 restore write was not fully acknowledged: {}",
            serde_json::to_string(self)?
        );
        Ok(())
    }
}

/// Current rzDevice30._createDataSend. Command payload length, profile and
/// element are in the eight-byte header; power times use little endian.
pub fn ble_frame(
    transaction: u8,
    command: [u8; 3],
    profile: u8,
    element: u8,
    data: &[u8],
) -> anyhow::Result<Vec<u8>> {
    ensure!(
        transaction < 31 && data.len() <= 255,
        "756 BLE command header is invalid"
    );
    let mut frame = vec![
        transaction,
        data.len() as u8,
        0,
        0,
        command[1],
        command[2],
        profile,
        element,
    ];
    frame.extend_from_slice(data);
    Ok(frame)
}

/// Source75938 active-slot reads/writes and34938 PM/S9 power getters. The
/// platform exchange checks actual GATT status/identity and owns cleanup.
pub fn execute_ble(
    operation: &Operation,
    mut exchange: impl FnMut([u8; 3], u8, u8, &[u8]) -> anyhow::Result<Vec<u8>>,
) -> anyhow::Result<Reply> {
    match operation {
        Operation::BleSlot { set } => {
            ensure!(*set != Some(0), "756 BLE profile slot is zero");
            let data = set.map(|slot| vec![slot]).unwrap_or_default();
            let acknowledgement = exchange(
                [data.len() as u8, 3, if set.is_some() { 2 } else { 130 }],
                0,
                0,
                &data,
            )?;
            let slot = if set.is_some() {
                None
            } else {
                Some(keyboard_latest_restore::ble_profile_response(
                    &acknowledgement,
                )?)
            };
            Ok(Reply::BleSlot {
                slot,
                acknowledgement,
            })
        }
        Operation::ReadTimeToDim { slot } => {
            let batteries = exchange([0, 5, 128], 0, 1, &[])?;
            let battery = batteries.first().copied().unwrap_or(1);
            let bytes = exchange([0, 5, 138], *slot, battery, &[])?;
            ensure!(bytes.len() >= 4, "756 BLE dim response is truncated");
            Ok(Reply::TimeToDim {
                slot: *slot,
                enabled: bytes[0] == 1,
                seconds: u16::from_le_bytes([bytes[1], bytes[2]]),
                brightness: bytes[3],
                bytes,
            })
        }
        Operation::ReadAutoSleepMode { slot } => {
            let bytes = exchange([0, 5, 135], *slot, 1, &[])?;
            let enabled = *bytes
                .first()
                .context("756 BLE auto-sleep response is empty")?
                == 1;
            Ok(Reply::AutoSleepMode {
                slot: *slot,
                enabled,
                bytes,
            })
        }
        Operation::ReadTimeToSleep => {
            let bytes = exchange([0, 5, 132], 0, 0, &[])?;
            ensure!(bytes.len() >= 2, "756 BLE sleep response is truncated");
            Ok(Reply::TimeToSleep {
                seconds: u16::from_le_bytes([bytes[0], bytes[1]]),
                bytes,
            })
        }
        _ => anyhow::bail!(
            "756 BLE command has no current protocol30 adapter: {}",
            serde_json::to_string(operation)?
        ),
    }
}
pub fn capability(physical: u32) -> anyhow::Result<&'static DeviceReadCapability> {
    ensure!(
        keyboard_latest_restore::latest_restore(physical, false),
        "Latest keyboard restore requires the actual756/757 adapter"
    );
    crate::keyboard_metadata::hid_capability(756, physical)
        .context("756 restore physical transport has no current factory")
}
fn key_list(keys: &[u8]) -> anyhow::Result<()> {
    ensure!(
        !keys.is_empty()
            && keys.iter().collect::<std::collections::BTreeSet<_>>().len() == keys.len(),
        "756 restore key list is empty or repeated"
    );
    Ok(())
}
fn settings(setting: &Setting) -> anyhow::Result<([u8; 3], Vec<u8>)> {
    let (command, payload) = match setting {
        Setting::ButtonAssignment {
            slot,
            key_id,
            mode,
            fn_id,
            data_size,
            data,
        } => {
            // The source passes 80 padded bytes with packetSizeSet=10.
            // exchange supplies that zero padding in its full report; pass
            // only the ten populated bytes so its declared-size check agrees.
            (
                [80, 2, 13],
                crate::onboard_assignment::payload(
                    *slot, *key_id, *mode, *fn_id, *data_size, data,
                )?,
            )
        }
        Setting::AdjustmentMode { slot, enabled } => ([2, 2, 31], vec![*slot, u8::from(*enabled)]),
        Setting::Brightness {
            slot,
            region,
            percent,
        } => {
            ensure!(
                percent.is_finite() && *percent >= 0.,
                "756 brightness is not a finite positive percentage"
            );
            (
                [3, 15, 4],
                vec![
                    *slot,
                    *region,
                    ((*percent).min(100.) / 100. * 255.).floor() as u8,
                ],
            )
        }
        Setting::PollingRate { slot, hertz } => {
            let encoded = match hertz {
                8000 => 1,
                4000 => 2,
                2000 => 4,
                1000 => 8,
                500 => 16,
                250 => 32,
                125 => 64,
                _ => anyhow::bail!("756 polling rate is outside current enum"),
            };
            ([2, 0, 64], vec![*slot, encoded])
        }
        Setting::GameModeState(enabled) => ([4, 0, 8], vec![*enabled, 0, 0, 0]),
        Setting::LedState {
            class,
            led,
            enabled,
        } => ([3, 3, 0], vec![*class, *led, u8::from(*enabled)]),
        Setting::GameModeSelection {
            slot,
            selection,
            extension,
        } => {
            if let Some(extension) = extension {
                ([3, 0, 90], vec![*slot, *selection, *extension])
            } else {
                ([2, 0, 90], vec![*slot, *selection])
            }
        }
        Setting::CustomGameKeys { slot, combos } => {
            ensure!(
                combos.iter().all(|combo| combo[0] <= 2),
                "756 custom game combination exceeds two keys"
            );
            let mut data = vec![*slot];
            for combo in combos {
                data.extend_from_slice(combo);
            }
            ([10, 0, 93], data)
        }
        Setting::TimeToDim {
            slot,
            enabled,
            seconds,
            brightness,
        } => {
            let [high, low] = seconds.to_be_bytes();
            (
                [5, 7, 11],
                vec![*slot, u8::from(*enabled), high, low, *brightness],
            )
        }
        Setting::TimeToSleep { seconds } => ([2, 7, 3], seconds.to_be_bytes().to_vec()),
        Setting::AutoSleepMode { slot, enabled } => ([2, 7, 8], vec![*slot, u8::from(*enabled)]),
        Setting::PowerMode(value) => ([1, 7, 15], vec![*value]),
        Setting::InterruptKeys {
            slot,
            dynamic_keystroke,
            enabled,
            mode,
            keys,
        } => crate::onboard_assignment::interrupt_keys_payload(
            *slot,
            *dynamic_keystroke,
            *enabled,
            *mode,
            keys,
        )?,
        Setting::ContinuousRapidTrigger { slot, enabled } => {
            ([2, 2, 32], vec![*slot, u8::from(*enabled)])
        }
        Setting::BottomDeadzone { slot, value } => ([2, 2, 40], vec![*slot, *value]),
        Setting::SnapTap {
            slot,
            enabled,
            pairs,
        } => {
            ensure!(
                pairs.len() <= 4 && pairs.iter().all(|p| p[2] <= 4),
                "Snap Tap pairs exceed current source bounds"
            );
            let mut payload = vec![*slot, pairs.len() as u8, u8::from(*enabled)];
            for pair in pairs {
                payload.extend_from_slice(pair);
            }
            payload.resize(15, 0);
            ([15, 2, 39], payload)
        }
        Setting::SingleKeySnapTap {
            slot,
            enabled,
            keys,
        } => (
            crate::onboard_assignment::SET_SINGLE_KEY_SNAP_TAP,
            crate::onboard_assignment::single_key_snap_tap_payload(*slot, *enabled, keys)?,
        ),
        Setting::DualKeypressPriority {
            slot,
            enabled,
            keys,
        } => {
            let keys = &keys[..keys.len().min(4)];
            let mut payload = vec![*slot, keys.len() as u8, u8::from(*enabled)];
            payload.extend_from_slice(keys);
            payload.resize(7, 0);
            ([7, 2, 49], payload)
        }
        Setting::AnalogReportMode(value) => ([1, 2, 42], vec![*value]),
        Setting::JoystickReportMode(value) => ([1, 2, 56], vec![*value]),
        Setting::JoystickMappingMode { slot, mode } => ([2, 2, 57], vec![*slot, *mode]),
        Setting::TriggerPresetEffect { slot, key } => ([80, 15, 23], vec![1, *slot, *key, 0, 0, 0]),
    };
    let slot = match setting {
        Setting::TimeToDim { .. }
        | Setting::TimeToSleep { .. }
        | Setting::AutoSleepMode { .. }
        | Setting::LedState { .. }
        | Setting::PowerMode(_)
        | Setting::GameModeState(_)
        | Setting::AnalogReportMode(_)
        | Setting::JoystickReportMode(_) => None,
        _ => payload.first().copied(),
    };
    ensure!(slot != Some(0), "756 restore profile ID is zero");
    Ok((command, payload))
}

pub fn execute_hid(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    operation: &Operation,
    mut next: impl FnMut() -> anyhow::Result<u8>,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Reply> {
    ensure!(
        cap.product_id == 756 && cap.direct_pids.iter().any(|pid| matches!(*pid, 756 | 757)),
        "756 restore capability identity differs"
    );
    validate()?;
    if let Operation::WriteProfileMetadata(metadata) = operation {
        let bytes = profile_metadata_bytes(metadata)?;
        let mut transport = cap.clone();
        transport.sleep_between_out_in_ms *= 2;
        transport.sleep_between_in_ms = transport
            .sleep_between_in_ms
            .max(transport.sleep_between_out_in_ms);
        let mut requests = vec![([1, 5, 2], vec![metadata.slot_id], 0usize, 0usize)];
        for (index, chunk) in bytes.chunks(64).enumerate() {
            let offset = index * 64;
            let [high, low] = (offset as u16).to_be_bytes();
            let mut payload = vec![metadata.slot_id, high, low, 0, 250];
            payload.extend_from_slice(chunk);
            requests.push(([80, 5, 8], payload, offset, chunk.len()));
        }
        let mut chunks = Vec::new();
        for (command, payload, offset, count) in requests {
            let result = feature_exchange::exchange_with_report_data_size(
                device,
                &transport,
                command,
                &payload,
                next().map_err(|error| write_failure(error, &chunks))?,
                payload.len() as u8,
                validate,
            );
            let failed = result.is_err();
            retain_chunk(&mut chunks, offset, count, result, validate)?;
            // setProfileNameStructure's catch exits the chunk loop after the
            // first failure; unlike setMulti*, later metadata is not attempted.
            if failed {
                break;
            }
        }
        validate().map_err(|error| write_failure(error, &chunks))?;
        return Ok(Reply::Write { chunks });
    }
    match operation {
        Operation::ReadSingleKeySnapTap { slot } => {
            ensure!(*slot > 0, "756 single-key Snap Tap slot is zero");
            let bytes =
                feature_exchange::exchange(device, cap, [11, 2, 172], &[*slot], next()?, validate)?;
            ensure!(
                bytes.len() >= 11 && bytes[0] == *slot && bytes[1] <= 4 && bytes[2] <= 1,
                "756 single-key Snap Tap response identity/length differs"
            );
            let keys = bytes[3..11]
                .chunks_exact(2)
                .map(|key| [key[0], key[1]])
                .filter(|key| key[0] != 0)
                .collect();
            validate()?;
            return Ok(Reply::SingleKeySnapTap {
                slot: *slot,
                enabled: bytes[2] == 1,
                keys,
                bytes,
            });
        }
        Operation::ReadSnapTap { slot } => {
            ensure!(*slot > 0, "756 Snap Tap slot is zero");
            let bytes =
                feature_exchange::exchange(device, cap, [15, 2, 167], &[*slot], next()?, validate)?;
            ensure!(
                bytes.len() >= 15 && bytes[0] == *slot && bytes[1] <= 4 && bytes[2] <= 1,
                "756 Snap Tap response identity/length differs"
            );
            let pairs = bytes[3..15]
                .chunks_exact(3)
                .map(|pair| [pair[0], pair[1], pair[2]])
                .filter(|pair| *pair != [0, 0, 0])
                .collect();
            validate()?;
            return Ok(Reply::SnapTap {
                slot: *slot,
                enabled: bytes[2] == 1,
                pairs,
                bytes,
            });
        }
        Operation::ReadTimeToDim { slot } => {
            let bytes =
                feature_exchange::exchange(device, cap, [5, 7, 139], &[*slot], next()?, validate)?;
            ensure!(
                bytes.len() >= 5 && bytes[0] == *slot && bytes[1] <= 1,
                "756 dim response identity/length differs"
            );
            validate()?;
            return Ok(Reply::TimeToDim {
                slot: *slot,
                enabled: bytes[1] == 1,
                seconds: u16::from_be_bytes([bytes[2], bytes[3]]),
                brightness: bytes[4],
                bytes,
            });
        }
        Operation::ReadAutoSleepMode { slot } => {
            let bytes =
                feature_exchange::exchange(device, cap, [2, 7, 136], &[*slot], next()?, validate)?;
            ensure!(
                bytes.len() >= 2 && bytes[0] == *slot && bytes[1] <= 1,
                "756 auto-sleep response identity/length differs"
            );
            validate()?;
            return Ok(Reply::AutoSleepMode {
                slot: *slot,
                enabled: bytes[1] == 1,
                bytes,
            });
        }
        Operation::ReadTimeToSleep => {
            let bytes =
                feature_exchange::exchange(device, cap, [2, 7, 131], &[], next()?, validate)?;
            ensure!(bytes.len() >= 2, "756 sleep response is truncated");
            validate()?;
            return Ok(Reply::TimeToSleep {
                seconds: u16::from_be_bytes([bytes[0], bytes[1]]),
                bytes,
            });
        }
        _ => {}
    }
    if let Operation::ReadGameModeSelection { slot } = operation {
        ensure!(*slot > 0, "756 game-mode selection slot is zero");
        let bytes = feature_exchange::exchange(
            device,
            cap,
            [3, 0, 218],
            &[*slot, 0, 0],
            next()?,
            validate,
        )?;
        ensure!(
            bytes.len() >= 2 && bytes[0] == *slot,
            "756 game-mode selection response identity/length differs"
        );
        validate()?;
        return Ok(Reply::GameModeSelection {
            slot: *slot,
            selection: bytes[1],
            extension: bytes.get(2).copied().unwrap_or(0),
            bytes,
        });
    }
    if let Operation::ReadPollingRate { slot } = operation {
        ensure!(*slot > 0, "756 polling getter slot is zero");
        let bytes =
            feature_exchange::exchange(device, cap, [2, 0, 192], &[*slot, 0], next()?, validate)?;
        ensure!(
            bytes.len() >= 2 && bytes[0] == *slot,
            "756 polling response identity/length differs"
        );
        let hertz = match bytes[1] {
            1 => 8000,
            2 => 4000,
            4 => 2000,
            8 => 1000,
            16 => 500,
            32 => 250,
            64 => 125,
            _ => anyhow::bail!("756 polling response has an unknown enum"),
        };
        validate()?;
        return Ok(Reply::PollingRate {
            slot: *slot,
            hertz,
            bytes,
        });
    }
    if matches!(operation, Operation::Inventory) {
        let list = feature_exchange::exchange(device, cap, [80, 5, 129], &[], next()?, validate)?;
        let count = usize::from(*list.first().context("756 onboard profile list is empty")?);
        ensure!(
            count <= 79 && list.len() > count,
            "756 onboard profile list is truncated"
        );
        let ids = &list[1..1 + count];
        ensure!(
            ids.iter().all(|id| *id > 0)
                && ids.iter().collect::<std::collections::BTreeSet<_>>().len() == ids.len(),
            "756 onboard slot IDs are zero or repeated"
        );
        let mut slots = Vec::new();
        let mut metadata = Vec::new();
        for &slot_id in ids {
            let bytes = crate::keyboard_profile_inventory::read_raw_metadata_hid(
                device, cap, slot_id, &mut next, validate,
            )?;
            slots.push(Slot {
                slot_id,
                guid: crate::keyboard_profile_inventory::guid_text(bytes[..16].try_into()?),
            });
            metadata.push(bytes.to_vec());
        }
        validate()?;
        return Ok(Reply::Inventory { slots, metadata });
    }
    let requests = match operation {
        Operation::Actuation { slot, points } => {
            ensure!(*slot > 0, "756 actuation slot is zero");
            key_list(&points.iter().map(|point| point.key_id).collect::<Vec<_>>())?;
            points
                .chunks(15)
                .map(|points| {
                    let mut data = vec![0, *slot];
                    for point in points {
                        data.push(point.key_id);
                        data.extend_from_slice(&point.threshold_high.to_be_bytes());
                        data.extend_from_slice(&point.threshold_low.to_be_bytes());
                    }
                    ([80, 2, 25], data, points.len())
                })
                .collect::<Vec<_>>()
        }
        Operation::RapidTrigger { slot, keys } => {
            ensure!(
                *slot > 0 && keys.iter().all(|key| key.enabled <= 1),
                "756 rapid trigger slot/status is invalid"
            );
            key_list(&keys.iter().map(|key| key.key_id).collect::<Vec<_>>())?;
            keys.chunks(13)
                .map(|keys| {
                    let mut data = vec![0, *slot];
                    for key in keys {
                        data.extend_from_slice(&[key.key_id, key.enabled]);
                        data.extend_from_slice(&key.up_sensitivity.to_be_bytes());
                        data.extend_from_slice(&key.down_sensitivity.to_be_bytes());
                    }
                    ([80, 2, 26], data, keys.len())
                })
                .collect()
        }
        Operation::Setting(setting) => {
            let (command, data) = settings(setting)?;
            vec![(command, data, 1)]
        }
        Operation::ReadActuation { slot, keys } | Operation::ReadRapidTrigger { slot, keys } => {
            ensure!(*slot > 0, "756 analog getter slot is zero");
            key_list(keys)?;
            let act = matches!(operation, Operation::ReadActuation { .. });
            let mut points = Vec::new();
            let mut rapid = Vec::new();
            let stride = if act { 5 } else { 6 };
            let limit = if act { 15 } else { 13 };
            for keys in keys.chunks(limit) {
                let mut payload = vec![0, *slot];
                for key in keys {
                    payload.push(*key);
                    payload.resize(payload.len() + stride - 1, 0);
                }
                let command = [80, 2, if act { 153 } else { 154 }];
                let data = feature_exchange::exchange_with_report_data_size(
                    device,
                    cap,
                    command,
                    &payload,
                    next()?,
                    (2 + stride * keys.len()) as u8,
                    validate,
                )?;
                ensure!(
                    data.len() >= 2 + stride * keys.len() && data[0] == 0 && data[1] == *slot,
                    "756 analog getter returned a truncated/foreign profile"
                );
                let mut seen = std::collections::BTreeSet::new();
                for row in data[2..2 + stride * keys.len()].chunks_exact(stride) {
                    ensure!(
                        keys.contains(&row[0]) && seen.insert(row[0]),
                        "756 analog getter returned an unrequested/repeated key"
                    );
                    if act {
                        points.push(ActuationPoint {
                            key_id: row[0],
                            threshold_high: u16::from_be_bytes([row[1], row[2]]),
                            threshold_low: u16::from_be_bytes([row[3], row[4]]),
                        });
                    } else {
                        rapid.push(RapidTriggerKey {
                            key_id: row[0],
                            enabled: row[1],
                            up_sensitivity: u16::from_be_bytes([row[2], row[3]]),
                            down_sensitivity: u16::from_be_bytes([row[4], row[5]]),
                        });
                    }
                }
            }
            return Ok(if act {
                Reply::Actuation { points }
            } else {
                Reply::RapidTrigger { keys: rapid }
            });
        }
        Operation::BleSlot { .. } => anyhow::bail!("BLE slot operation requires its GATT target"),
        Operation::Events { .. } => {
            anyhow::bail!("756 events require the retained interrupt owner")
        }
        Operation::Inventory
        | Operation::WriteProfileMetadata(_)
        | Operation::ReadGameModeSelection { .. }
        | Operation::ReadPollingRate { .. }
        | Operation::ReadSnapTap { .. }
        | Operation::ReadSingleKeySnapTap { .. }
        | Operation::ReadTimeToDim { .. }
        | Operation::ReadAutoSleepMode { .. }
        | Operation::ReadTimeToSleep => unreachable!(),
    };
    let mut chunks = Vec::new();
    let mut offset = 0;
    for (command, payload, count) in requests {
        // Source's setMulti* task catches each chunk failure and continues.
        let result = feature_exchange::exchange_with_report_data_size(
            device,
            cap,
            command,
            &payload,
            next().map_err(|error| write_failure(error, &chunks))?,
            if matches!(
                operation,
                Operation::Setting(Setting::ButtonAssignment { .. })
            ) {
                10
            } else {
                payload.len() as u8
            },
            validate,
        );
        retain_chunk(&mut chunks, offset, count, result, validate)?;
        offset += count;
    }
    validate().map_err(|error| write_failure(error, &chunks))?;
    Ok(Reply::Write { chunks })
}

#[cfg(test)]
#[path = "keyboard_latest_protocol_tests.rs"]
mod tests;
