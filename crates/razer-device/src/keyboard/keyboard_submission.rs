//! Current local scalar analog tasks, including observed OBM GUID slot selection.
//! Evidence: 742 MW module-34737::{j8,TB}, module-38657::getProfileNameStructure,
//! module-74503::{getProfileIDList,getProfileName}, module-48967::PeF.
pub use crate::keyboard_magnetic_mapping::InterruptSettings;
use crate::keyboard_magnetic_mapping::controls::{self, EnginePreparation};
use crate::keyboard_magnetic_mapping::{ButtonAssignment, PreparedAssignment};
use crate::{
    backend::{FeatureTransport, HidNode},
    device_reads::DeviceReadTarget,
    feature_exchange,
    keyboard_analog::{
        self, ActuationPoint, KeyboardAnalogChange, KeyboardAnalogTransportCapability,
        RapidTriggerKey,
    },
};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum Intent {
    /// The source select-all reducer persists/publishes the mapping before a
    /// separate global analog task; it does not enqueue the per-key OBM task.
    MappingDraftOnly,
    /// The native mapping owner publishes the captured engine document. This
    /// operation has no device Feature setter and cannot produce a HID ACK.
    AppEngineOnly,
    /// Source disabled-keyboard override only persists its profile flag.
    ProfileDraftOnly,
    GlobalActuation {
        points: Vec<ActuationPoint>,
    },
    GlobalRapidTrigger {
        keys: Vec<RapidTriggerKey>,
    },
    MappingChanges {
        assignments: Vec<PreparedAssignment>,
        points: Vec<ActuationPoint>,
        keys: Vec<RapidTriggerKey>,
        macro_sync: Option<crate::onboard_macro_codec::MacroSynchronization>,
        single_key_snap_tap: Option<SingleKeySnapTap>,
    },
    SingleKeySnapTap {
        settings: SingleKeySnapTap,
        global: bool,
    },
    ContinuousRapidTrigger {
        enabled: bool,
    },
    BottomDeadzone {
        value: String,
    },
    InterruptKeys {
        settings: InterruptSettings,
        /// The source enqueues an independent mapping event before awaiting
        /// its interrupt setters. Preserve that follow-up even if a setter fails.
        mapping: Option<Box<Intent>>,
    },
    CustomGamingCombinations {
        settings: Value,
    },
    SnapTap {
        settings: Value,
    },
}

impl Intent {
    pub fn requires_device(&self) -> bool {
        !matches!(
            self,
            Self::MappingDraftOnly | Self::AppEngineOnly | Self::ProfileDraftOnly
        )
    }

    pub fn publishes_mapping(&self) -> bool {
        matches!(
            self,
            Self::MappingDraftOnly
                | Self::MappingChanges { .. }
                | Self::AppEngineOnly
                | Self::InterruptKeys {
                    mapping: Some(_),
                    ..
                }
        )
    }

    pub fn macro_synchronization_mut(
        &mut self,
    ) -> Option<&mut Option<crate::onboard_macro_codec::MacroSynchronization>> {
        match self {
            Self::MappingChanges { macro_sync, .. } => Some(macro_sync),
            Self::InterruptKeys {
                mapping: Some(mapping),
                ..
            } => mapping.macro_synchronization_mut(),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum Target {
    Collection { node: HidNode, product_id: u32 },
    WindowsContainer(DeviceReadTarget),
}

impl Target {
    pub fn product_id(&self) -> u32 {
        match self {
            Self::Collection { product_id, .. } => *product_id,
            Self::WindowsContainer(target) => target.product_id,
        }
    }
}

/// Exact caller payload parsing; profile matching requires actual device inventory.
pub fn intent(product_id: u32, message: &Value) -> anyhow::Result<Intent> {
    let payload = &message["payload"];
    match message["type"]
        .as_str()
        .context("Keyboard intent type is missing")?
    {
        "ON_TOGGLE_CONTINUOUS_RAPID_TRIGGER" => {
            let enabled = payload["value"]
                .as_bool()
                .context("Continuous rapid trigger value is missing")?;
            validate(product_id, &Intent::ContinuousRapidTrigger { enabled })?;
            Ok(Intent::ContinuousRapidTrigger { enabled })
        }
        "ON_SET_BOTTOM_DEADZONE" => {
            let value = payload["bottomDeadzone"]["value"]
                .as_str()
                .context("Bottom deadzone value is missing")?
                .to_owned();
            let intent = Intent::BottomDeadzone { value };
            validate(product_id, &intent)?;
            Ok(intent)
        }
        "ON_SET_GAME_MODE_COMBINATION_KEYS" => {
            let intent = Intent::CustomGamingCombinations {
                settings: payload.clone(),
            };
            validate(product_id, &intent)?;
            Ok(intent)
        }
        "ON_SET_SNAP_TAP" => {
            let intent = Intent::SnapTap {
                settings: snap::local_settings(payload)?,
            };
            validate(product_id, &intent)?;
            Ok(intent)
        }
        event => anyhow::bail!("Keyboard task {event} requires its mapping/OBM submission chain"),
    }
}

/// Mapping/global tasks consume the caller's captured profile and observed
/// layout. Queued edits retain those inputs independently of later drafts.
pub fn intent_with_profile(
    product_id: u32,
    message: &Value,
    profile: &Value,
    previous: &Value,
    layout: u32,
) -> anyhow::Result<Intent> {
    let intent = match message["type"].as_str() {
        Some("ON_SET_ENABLE_KEYBOARD_KEYS") => {
            profile["isEnableKeyboardKeys"]
                .as_bool()
                .context("Keyboard input enabled state is absent")?;
            Intent::AppEngineOnly
        }
        Some("ON_SET_GAMEPAD_OVERRIDE_KEYS") => {
            profile["isGamepadOverride"]
                .as_bool()
                .context("Gamepad override state is absent")?;
            if profile["isEnableKeyboardKeys"] == false {
                Intent::ProfileDraftOnly
            } else {
                Intent::AppEngineOnly
            }
        }
        Some("ON_SET_INTERRUPT_KEYS") => {
            let settings = InterruptSettings::parse(&message["payload"])?;
            settings.firmware_keys(product_id)?;
            let mapping = if controls::has_interruptible(profile)? {
                let assignments = crate::keyboard_magnetic_mapping::prepare_assignments(
                    product_id, layout, profile, previous,
                )?;
                let (points, keys) = crate::keyboard_magnetic_mapping::per_key_changes(
                    product_id, profile, previous,
                )?;
                Some(Box::new(Intent::MappingChanges {
                    assignments,
                    points,
                    keys,
                    macro_sync: None,
                    single_key_snap_tap: None,
                }))
            } else {
                None
            };
            Intent::InterruptKeys { settings, mapping }
        }
        Some("ON_SET_OBM_SINGLE_KEY_SNAP_TAP" | "ON_TOGGLE_SINGLE_KEY_SNAP_TAP") => {
            let global = message["type"] == "ON_TOGGLE_SINGLE_KEY_SNAP_TAP";
            let settings = magnetic_snap::settings(product_id, &profile["singleKeySnapTap"])?;
            ensure!(
                !global || !settings.enabled || !settings.keys.is_empty(),
                "Enabled single-key Snap Tap requires at least one key entry"
            );
            Intent::SingleKeySnapTap { settings, global }
        }
        Some("ON_SYNC_GLOBAL_ACTUATION") => Intent::GlobalActuation {
            points: crate::keyboard_magnetic_mapping::global_actuation(
                product_id,
                layout,
                profile,
                &message["payload"]["value"],
            )?,
        },
        Some("ON_SYNC_GLOBAL_RAPID_TRIGGER") => Intent::GlobalRapidTrigger {
            keys: crate::keyboard_magnetic_mapping::global_rapid_trigger(
                product_id,
                layout,
                profile,
                &message["payload"]["value"],
            )?,
        },
        Some("ON_SET_KEYMAPPING") => {
            if message["payload"]["isSelectAll"] == true {
                validate(product_id, &Intent::MappingDraftOnly)?;
                return Ok(Intent::MappingDraftOnly);
            }
            let assignments = crate::keyboard_magnetic_mapping::prepare_assignments(
                product_id, layout, profile, previous,
            )?;
            let (points, keys) =
                crate::keyboard_magnetic_mapping::per_key_changes(product_id, profile, previous)?;
            Intent::MappingChanges {
                assignments,
                points,
                keys,
                macro_sync: None,
                single_key_snap_tap: None,
            }
        }
        _ => return intent(product_id, message),
    };
    validate(product_id, &intent)?;
    Ok(intent)
}

pub fn prepare_local(
    product: u32,
    layout: u32,
    message: &Value,
    profile: &Value,
    previous: &Value,
) -> anyhow::Result<(Value, Intent)> {
    let mut settings = profile.clone();
    if crate::keyboard_magnetic_mapping::supports(product) && message["type"] == "ON_SET_KEYMAPPING"
    {
        let rows = message["payload"]["mappingList"]
            .as_array()
            .context("Magnetic mapping payload is absent")?;
        settings["mappings"] = Value::Array(crate::keyboard_magnetic_mapping::normalize_mappings(
            product, layout, profile, previous, rows,
        )?);
    } else if crate::keyboard_magnetic_mapping::supports(product)
        && matches!(
            message["type"].as_str(),
            Some(
                "ON_SET_ENABLE_KEYBOARD_KEYS"
                    | "ON_SET_GAMEPAD_OVERRIDE_KEYS"
                    | "ON_SET_BOTTOM_DEADZONE"
                    | "ON_SET_INTERRUPT_KEYS"
            )
        )
    {
        persist_magnetic_controls(product, message, &mut settings)?;
    } else {
        persist_local(message, &mut settings)?;
    }
    if crate::keyboard_magnetic_mapping::supports(product) {
        match message["type"].as_str() {
            Some("ON_SET_KEYMAPPING") => {
                controls::set_preparation(&mut settings, EnginePreparation::Firmware)
            }
            Some("ON_SET_INTERRUPT_KEYS") if controls::has_interruptible(&settings)? => {
                let rows = settings["mappings"]
                    .as_array()
                    .context("Magnetic mapping list is absent")?
                    .clone();
                settings["mappings"] =
                    serde_json::json!(crate::keyboard_magnetic_mapping::normalize_mappings(
                        product, layout, &settings, previous, &rows
                    )?);
                controls::set_preparation(&mut settings, EnginePreparation::Firmware);
            }
            _ => {}
        }
    }
    let cleanup = if crate::keyboard_magnetic_mapping::supports(product)
        && matches!(
            message["type"].as_str(),
            Some("ON_SET_KEYMAPPING" | "ON_SET_INTERRUPT_KEYS")
        ) {
        magnetic_snap::remove_dks_conflicts(product, &mut settings)?
    } else {
        None
    };
    let mut intent = intent_with_profile(product, message, &settings, previous, layout)?;
    if let Some(cleanup) = cleanup {
        match &mut intent {
            Intent::MappingDraftOnly => {
                intent = Intent::SingleKeySnapTap {
                    settings: cleanup,
                    global: false,
                }
            }
            Intent::MappingChanges {
                single_key_snap_tap,
                ..
            } => *single_key_snap_tap = Some(cleanup),
            Intent::InterruptKeys {
                mapping: Some(mapping),
                ..
            } => {
                let Intent::MappingChanges {
                    single_key_snap_tap,
                    ..
                } = mapping.as_mut()
                else {
                    unreachable!()
                };
                *single_key_snap_tap = Some(cleanup);
            }
            _ => unreachable!(),
        }
    }
    Ok((settings, intent))
}

pub fn validate(
    product_id: u32,
    intent: &Intent,
) -> anyhow::Result<KeyboardAnalogTransportCapability> {
    let cap = keyboard_analog::transport_capability(product_id)
        .context("Keyboard analog transport has not been captured")?;
    match intent {
        Intent::GlobalActuation { points } => {
            ensure!(
                crate::keyboard_magnetic_mapping::supports(product_id),
                "Global magnetic actuation caller differs"
            );
            ensure!(
                points.first().is_some_and(|point| point.key_id == 255),
                "Global actuation indicator row is absent"
            );
        }
        Intent::GlobalRapidTrigger { keys } => {
            ensure!(
                crate::keyboard_magnetic_mapping::supports(product_id),
                "Global magnetic Rapid Trigger caller differs"
            );
            ensure!(
                keys.first().is_some_and(|key| key.key_id == 255),
                "Global Rapid Trigger indicator row is absent"
            );
        }
        Intent::SingleKeySnapTap { .. }
        | Intent::AppEngineOnly
        | Intent::ProfileDraftOnly
        | Intent::MappingDraftOnly
        | Intent::MappingChanges { .. } => ensure!(
            crate::keyboard_magnetic_mapping::supports(product_id),
            "Magnetic mapping caller differs"
        ),
        Intent::InterruptKeys { settings, mapping } => {
            ensure!(
                crate::keyboard_magnetic_mapping::supports(product_id),
                "Magnetic interruption caller differs"
            );
            settings.firmware_keys(product_id)?;
            if let Some(mapping) = mapping {
                ensure!(
                    matches!(mapping.as_ref(), Intent::MappingChanges { .. }),
                    "Interruption follow-up is not a mapping task"
                );
                validate(product_id, mapping)?;
            }
        }
        Intent::ContinuousRapidTrigger { .. } => ensure!(
            cap.analog_v3,
            "Product has no continuous rapid trigger branch"
        ),
        Intent::BottomDeadzone { .. } => ensure!(
            [740, 742, 746, 756].contains(&product_id),
            "Product has no captured bottom deadzone caller"
        ),
        Intent::CustomGamingCombinations { settings } => {
            ensure!(
                [742, 756].contains(&product_id),
                "Product has no captured custom gaming combinations task"
            );
            gaming::custom_keys(product_id, settings)?;
        }
        Intent::SnapTap { settings } => {
            snap::validate(product_id, &cap, settings)?;
        }
    }
    Ok(cap)
}

#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
pub struct ProfileIdentity {
    pub profile_id: u8,
    pub guid: String,
    pub raw_metadata: Vec<u8>,
    pub metadata_attempts: u8,
}

#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
pub struct Submission {
    pub profile_id: u8,
    pub command: [u8; 3],
    pub payload: Vec<u8>,
    pub attempted: bool,
    pub acknowledgement: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Serialize)]
#[non_exhaustive]
pub struct Receipt {
    pub intent: Intent,
    pub profile_guid: String,
    pub inventory: Vec<ProfileIdentity>,
    pub matched_slots: Vec<u8>,
    pub submissions: Vec<Submission>,
    pub macro_synchronization: Option<MacroProgress>,
    pub mapping_followup: Option<Box<Receipt>>,
    pub completed: bool,
}

#[derive(Debug)]
#[non_exhaustive]
pub struct Failure {
    pub receipt: Receipt,
    pub error: anyhow::Error,
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Keyboard submission: {:#}; progress: {}",
            self.error,
            serde_json::to_string(&self.receipt).unwrap_or_default()
        )
    }
}
impl std::error::Error for Failure {}

fn guid_text(bytes: &[u8]) -> String {
    format!(
        "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        u32::from_le_bytes(bytes[..4].try_into().unwrap()),
        u16::from_le_bytes(bytes[4..6].try_into().unwrap()),
        u16::from_le_bytes(bytes[6..8].try_into().unwrap()),
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15]
    )
}

fn inventory(
    device: &dyn FeatureTransport,
    cap: &KeyboardAnalogTransportCapability,
    next: &mut impl FnMut() -> anyhow::Result<u8>,
    guard: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Vec<ProfileIdentity>> {
    let transport = cap.device_read_capability();
    let list = feature_exchange::exchange(device, &transport, [80, 5, 129], &[], next()?, guard)?;
    let count = usize::from(*list.first().context("Profile ID list is empty")?);
    ensure!(
        count <= usize::from(cap.hardware_profile_slots) && list.len() > count,
        "Profile ID list is truncated or exceeds the captured inventory"
    );
    let ids = &list[1..1 + count];
    ensure!(
        ids.iter().all(|id| *id > 0)
            && ids.iter().collect::<std::collections::BTreeSet<_>>().len() == ids.len(),
        "Profile IDs are invalid or repeated"
    );
    let mut metadata_transport = transport;
    metadata_transport.sleep_between_out_in_ms *= 2;
    metadata_transport.sleep_between_in_ms = metadata_transport
        .sleep_between_in_ms
        .max(metadata_transport.sleep_between_out_in_ms);
    let mut profiles = Vec::new();
    for &profile_id in ids {
        let mut metadata = Vec::new();
        let mut attempts = 0;
        let mut read_error = None;
        for _ in 0..10 {
            guard()?;
            attempts += 1;
            metadata.clear();
            read_error = None;
            for offset in (0u16..250).step_by(64) {
                let bytes = usize::from((250 - offset).min(64));
                let offset_bytes = offset.to_be_bytes();
                let chunk = (|| -> anyhow::Result<Vec<u8>> {
                    let data = feature_exchange::exchange_with_report_data_size(
                        device,
                        &metadata_transport,
                        [80, 5, 136],
                        &[profile_id, offset_bytes[0], offset_bytes[1], 0, 0],
                        next()?,
                        (bytes + 5) as u8,
                        guard,
                    )?;
                    ensure!(
                        data.len() >= bytes + 5
                            && data[0] == profile_id
                            && data[1..3] == offset_bytes,
                        "Profile metadata response is truncated or belongs to another slot/offset"
                    );
                    Ok(data[5..5 + bytes].to_vec())
                })();
                match chunk {
                    Ok(chunk) => metadata.extend_from_slice(&chunk),
                    Err(error) => {
                        guard()?;
                        read_error = Some(format!("{error:#}"));
                        metadata.clear();
                        break;
                    }
                }
            }
            if metadata.len() == 250 {
                let guid = guid_text(&metadata[..16]);
                if !guid.contains("ffff") && !guid.contains("0000") {
                    break;
                }
            }
        }
        // The source retains an invalid GUID after ten retries. Preserve those
        // actual bytes, so an absent match cannot be replaced by a local index.
        ensure!(
            metadata.len() == 250,
            "Profile metadata failed after {attempts} attempts: {}",
            read_error.unwrap_or_default()
        );
        profiles.push(ProfileIdentity {
            profile_id,
            guid: guid_text(&metadata[..16]),
            raw_metadata: metadata,
            metadata_attempts: attempts,
        });
    }
    Ok(profiles)
}

/// Setter ACKs satisfy these source tasks. No synthetic getter-to-UI update occurs.
pub fn submit(
    device: &dyn FeatureTransport,
    product_id: u32,
    profile_guid: &str,
    intent: Intent,
    mut next: impl FnMut() -> anyhow::Result<u8>,
    guard: impl Fn() -> anyhow::Result<()>,
) -> Result<Receipt, Failure> {
    let mut receipt = Receipt {
        intent,
        profile_guid: profile_guid.into(),
        inventory: Vec::new(),
        matched_slots: Vec::new(),
        submissions: Vec::new(),
        macro_synchronization: None,
        mapping_followup: None,
        completed: false,
    };
    let mut result = (|| -> anyhow::Result<()> {
        let cap = validate(product_id, &receipt.intent)?;
        ensure!(
            receipt.intent.requires_device(),
            "Local mapping/profile preparation has no device submission"
        );
        ensure!(
            profile_guid.len() == 36
                && profile_guid.bytes().enumerate().all(|(ix, byte)| {
                    if [8, 13, 18, 23].contains(&ix) {
                        byte == b'-'
                    } else {
                        byte.is_ascii_hexdigit()
                    }
                }),
            "Active profile GUID is invalid"
        );
        guard()?;
        receipt.inventory = inventory(device, &cap, &mut next, &guard)?;
        receipt.matched_slots = receipt
            .inventory
            .iter()
            .filter(|profile| profile.guid == profile_guid)
            .map(|profile| profile.profile_id)
            .collect();
        if let Intent::InterruptKeys { settings, .. } = &receipt.intent {
            let (mode, keys) = settings.firmware_keys(product_id)?;
            let enabled = settings.is_enabled;
            let slots = std::iter::once(1)
                .chain(receipt.matched_slots.iter().copied())
                .collect::<Vec<_>>();
            // Current56839 completes all macro slots before the DKS setter.
            // Matching slot1 is intentionally repeated by the source.
            for dynamic in [false, true] {
                for &profile_id in &slots {
                    let (command, payload) = crate::onboard_assignment::interrupt_keys_payload(
                        profile_id, dynamic, enabled, mode, &keys,
                    )?;
                    scalar_step(
                        device,
                        &cap,
                        profile_id,
                        command,
                        payload,
                        &mut receipt,
                        &mut next,
                        &guard,
                    )?;
                }
            }
            guard()?;
            receipt.completed = true;
            return Ok(());
        }
        if let Intent::SingleKeySnapTap { settings, global } = &receipt.intent {
            let settings = settings.clone();
            let global = *global;
            magnetic_snap::submit(
                device,
                &cap,
                &settings,
                global,
                &mut receipt,
                &mut next,
                &guard,
            )?;
            guard()?;
            receipt.completed = true;
            return Ok(());
        }
        if let Intent::MappingChanges {
            single_key_snap_tap: Some(settings),
            ..
        } = &receipt.intent
        {
            let settings = settings.clone();
            magnetic_snap::submit(
                device,
                &cap,
                &settings,
                false,
                &mut receipt,
                &mut next,
                &guard,
            )?;
        }
        if matches!(
            receipt.intent,
            Intent::GlobalActuation { .. }
                | Intent::GlobalRapidTrigger { .. }
                | Intent::MappingChanges { .. }
        ) {
            let ordered = receipt
                .matched_slots
                .iter()
                .copied()
                .chain([1])
                .filter({
                    let mut seen = std::collections::BTreeSet::new();
                    move |slot| seen.insert(*slot)
                })
                .collect::<Vec<_>>();
            let (assignments, points, keys) = match &receipt.intent {
                Intent::GlobalActuation { points } => (Vec::new(), points.clone(), Vec::new()),
                Intent::GlobalRapidTrigger { keys } => (Vec::new(), Vec::new(), keys.clone()),
                Intent::MappingChanges {
                    assignments,
                    points,
                    keys,
                    ..
                } => (assignments.clone(), points.clone(), keys.clone()),
                _ => unreachable!(),
            };
            let macro_sync = match &receipt.intent {
                Intent::MappingChanges { macro_sync, .. } => macro_sync.clone(),
                _ => None,
            };
            let macros = if let Some(synchronization) = &macro_sync {
                Some(macro_submission::synchronize(
                    device,
                    &cap,
                    profile_guid,
                    synchronization,
                    &mut receipt,
                    &mut next,
                    &guard,
                )?)
            } else if assignments
                .iter()
                .any(|assignment| matches!(assignment, PreparedAssignment::Macro { .. }))
            {
                Some(
                    crate::onboard_macro::OnboardMacro::for_keyboard(device, &cap)?
                        .load_macros(&mut next, &guard)?,
                )
            } else {
                None
            };
            // Vc queues one mapping task at a time; H6 visits that mapping's
            // observed slots before the next mapping. Analog batches follow.
            for assignment in assignments {
                let assignment = match assignment {
                    PreparedAssignment::Button(assignment) => assignment,
                    PreparedAssignment::Macro {
                        key_id,
                        mode,
                        guid,
                        playback,
                    } => {
                        if macro_sync.as_ref().is_some_and(|context| {
                            !context.macros.iter().any(|request| request.guid == guid)
                        }) {
                            for &profile_id in &ordered {
                                assignment_step(
                                    device,
                                    &cap,
                                    profile_id,
                                    ButtonAssignment {
                                        key_id,
                                        mode,
                                        function_id: 0,
                                        data_size: 0,
                                        data: [0; 5],
                                    },
                                    &mut receipt,
                                    &mut next,
                                    &guard,
                                )?;
                            }
                            continue;
                        }
                        let matches = macros
                            .as_ref()
                            .unwrap()
                            .macros
                            .iter()
                            .filter(|entry| entry.metadata.guid == guid)
                            .collect::<Vec<_>>();
                        let [entry] = matches.as_slice() else {
                            anyhow::bail!(
                                "Macro GUID {guid} is not unique in observed firmware storage"
                            )
                        };
                        let (function_id, data_size, data) =
                            crate::onboard_assignment::macro_function(
                                entry.metadata.macro_id,
                                playback,
                            );
                        ButtonAssignment {
                            key_id,
                            mode,
                            function_id,
                            data_size,
                            data,
                        }
                    }
                };
                for &profile_id in &ordered {
                    assignment_step(
                        device,
                        &cap,
                        profile_id,
                        assignment,
                        &mut receipt,
                        &mut next,
                        &guard,
                    )?;
                }
            }
            // jc queues actuation for every affected slot, followed by Rapid
            // Trigger for every slot. Source ep/Xd use the same ordered slots.
            for rapid in [false, true] {
                for &profile_id in &ordered {
                    let changes = if rapid {
                        unique_chunks(&keys, 13, |key| key.key_id)
                            .into_iter()
                            .map(|keys| KeyboardAnalogChange::RapidTrigger {
                                profile_id,
                                keys: keys.to_vec(),
                            })
                            .collect::<Vec<_>>()
                    } else {
                        unique_chunks(&points, 15, |point| point.key_id)
                            .into_iter()
                            .map(|points| KeyboardAnalogChange::Actuation {
                                profile_id,
                                points: points.to_vec(),
                            })
                            .collect::<Vec<_>>()
                    };
                    for change in changes {
                        analog_step(
                            device,
                            &cap,
                            profile_id,
                            change,
                            &mut receipt,
                            &mut next,
                            &guard,
                        )?;
                    }
                }
            }
            guard()?;
            receipt.completed = true;
            return Ok(());
        }
        if let Intent::SnapTap { settings } = &receipt.intent {
            let settings = settings.clone();
            snap::submit(
                device,
                product_id,
                &cap,
                &settings,
                &mut receipt,
                &mut next,
                &guard,
            )?;
            guard()?;
            receipt.completed = true;
            return Ok(());
        }
        if let Intent::CustomGamingCombinations { settings } = &receipt.intent {
            let settings = settings.clone();
            gaming::submit(
                device,
                product_id,
                &cap,
                &settings,
                &mut receipt,
                &mut next,
                &guard,
            )?;
            guard()?;
            receipt.completed = true;
            return Ok(());
        }
        let slots = std::iter::once(1)
            .chain(receipt.matched_slots.iter().copied())
            .collect::<Vec<_>>();
        for profile_id in slots {
            guard()?;
            match &receipt.intent {
                Intent::ContinuousRapidTrigger { enabled } => {
                    receipt.submissions.push(Submission {
                        profile_id,
                        command: [2, 2, 32],
                        payload: vec![profile_id, u8::from(*enabled)],
                        attempted: false,
                        acknowledgement: None,
                    });
                    let outcome = keyboard_analog::write(
                        device,
                        &cap,
                        KeyboardAnalogChange::ContinuousRapidTrigger {
                            profile_id,
                            enabled: *enabled,
                        },
                        false,
                        &mut next,
                        &guard,
                    );
                    let step = receipt.submissions.last_mut().unwrap();
                    match outcome {
                        Ok(report) => {
                            step.attempted = report.attempted_writes > 0;
                            step.acknowledgement = report
                                .acknowledgements
                                .into_iter()
                                .next()
                                .map(|ack| ack.data);
                        }
                        Err(error) => {
                            step.attempted = error.report.attempted_writes > 0;
                            step.acknowledgement = error
                                .report
                                .acknowledgements
                                .first()
                                .map(|ack| ack.data.clone());
                            return Err(error.into());
                        }
                    }
                }
                Intent::BottomDeadzone { value } => {
                    let raw = if crate::keyboard_magnetic_mapping::supports(product_id) {
                        crate::keyboard_magnetic_mapping::bottom_deadzone(value)
                    } else {
                        match value.to_lowercase().as_str() {
                            "responsive" => 10,
                            "stable" => 14,
                            _ => 12,
                        }
                    };
                    receipt.submissions.push(Submission {
                        profile_id,
                        command: [2, 2, 40],
                        payload: vec![profile_id, raw],
                        attempted: false,
                        acknowledgement: None,
                    });
                    let step = receipt.submissions.last_mut().unwrap();
                    let transaction = next()?;
                    let result = feature_exchange::exchange_with_report_data_size_observing_send(
                        device,
                        &cap.device_read_capability(),
                        step.command,
                        &step.payload,
                        transaction,
                        2,
                        &guard,
                        &mut || step.attempted = true,
                    );
                    step.acknowledgement =
                        feature_exchange::acknowledged_reply(&result).map(<[u8]>::to_vec);
                    result?;
                }
                Intent::CustomGamingCombinations { .. }
                | Intent::SnapTap { .. }
                | Intent::GlobalActuation { .. }
                | Intent::GlobalRapidTrigger { .. }
                | Intent::MappingDraftOnly
                | Intent::AppEngineOnly
                | Intent::ProfileDraftOnly
                | Intent::InterruptKeys { .. }
                | Intent::SingleKeySnapTap { .. }
                | Intent::MappingChanges { .. } => {
                    unreachable!("keyboard task has an independent slot policy")
                }
            }
        }
        guard()?;
        receipt.completed = true;
        Ok(())
    })();
    if let Intent::InterruptKeys {
        mapping: Some(mapping),
        ..
    } = &receipt.intent
    {
        let mapping = (**mapping).clone();
        let finalization_failed = result
            .as_ref()
            .err()
            .is_some_and(crate::backend::exchange_finalization_failed);
        if !finalization_failed && validate(product_id, &receipt.intent).is_ok() {
            match guard() {
                Ok(()) => {
                    let followup_next: &mut dyn FnMut() -> anyhow::Result<u8> = &mut next;
                    let followup_guard: &dyn Fn() -> anyhow::Result<()> = &guard;
                    match submit(
                        device,
                        product_id,
                        profile_guid,
                        mapping,
                        followup_next,
                        followup_guard,
                    ) {
                        Ok(followup) => receipt.mapping_followup = Some(Box::new(followup)),
                        Err(failure) => {
                            receipt.mapping_followup = Some(Box::new(failure.receipt));
                            result = Err(match result {
                                Ok(()) => failure.error,
                                Err(error) => anyhow::anyhow!(
                                    "Interrupt setters: {error:#}; independent mapping task: {:#}",
                                    failure.error
                                ),
                            });
                        }
                    }
                }
                Err(error) if result.is_ok() => result = Err(error),
                Err(_) => {}
            }
        }
    }
    receipt.completed = result.is_ok();
    match result {
        Ok(()) => Ok(receipt),
        Err(error) => Err(Failure { receipt, error }),
    }
}

/// A changed analog row restores its old setting before applying the new one.
/// Keep both operations in source order while satisfying the transport's
/// concrete-key uniqueness invariant in each command.
fn unique_chunks<T>(rows: &[T], limit: usize, key: impl Fn(&T) -> u8) -> Vec<&[T]> {
    let mut chunks = Vec::new();
    let mut start = 0;
    let mut seen = std::collections::BTreeSet::new();
    for (index, row) in rows.iter().enumerate() {
        if index - start == limit || seen.contains(&key(row)) {
            chunks.push(&rows[start..index]);
            start = index;
            seen.clear();
        }
        seen.insert(key(row));
    }
    if start < rows.len() {
        chunks.push(&rows[start..]);
    }
    chunks
}

fn assignment_step(
    device: &dyn FeatureTransport,
    cap: &KeyboardAnalogTransportCapability,
    profile_id: u8,
    assignment: ButtonAssignment,
    receipt: &mut Receipt,
    next: &mut impl FnMut() -> anyhow::Result<u8>,
    guard: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    guard()?;
    receipt.submissions.push(Submission {
        profile_id,
        command: [80, 2, 13],
        payload: assignment.payload(profile_id)?,
        attempted: false,
        acknowledgement: None,
    });
    let transaction = next()?;
    let step = receipt.submissions.last_mut().unwrap();
    let result = feature_exchange::exchange_with_report_data_size_observing_send(
        device,
        &cap.device_read_capability(),
        step.command,
        &step.payload,
        transaction,
        10,
        guard,
        &mut || step.attempted = true,
    );
    step.acknowledgement = feature_exchange::acknowledged_reply(&result).map(<[u8]>::to_vec);
    result?;
    Ok(())
}

/// Current profile reducer values are persisted before the hardware task.
pub fn persist_magnetic_controls(
    product: u32,
    message: &Value,
    profile: &mut Value,
) -> anyhow::Result<()> {
    ensure!(
        crate::keyboard_magnetic_mapping::supports(product),
        "Magnetic controls product differs"
    );
    persist_local(message, profile)?;
    match message["type"].as_str() {
        Some("ON_SET_KEYMAPPING") => {
            controls::set_preparation(profile, EnginePreparation::Firmware)
        }
        Some("ON_SET_ENABLE_KEYBOARD_KEYS") => {
            controls::set_preparation(profile, EnginePreparation::Gamepad)
        }
        Some("ON_SET_GAMEPAD_OVERRIDE_KEYS") if profile["isEnableKeyboardKeys"] != false => {
            controls::set_preparation(profile, EnginePreparation::Gamepad)
        }
        Some("ON_SET_INTERRUPT_KEYS") if controls::has_interruptible(profile)? => {
            controls::set_preparation(profile, EnginePreparation::Firmware)
        }
        Some(
            "ON_SET_GAMEPAD_OVERRIDE_KEYS" | "ON_SET_BOTTOM_DEADZONE" | "ON_SET_INTERRUPT_KEYS",
        ) => {}
        _ => anyhow::bail!("Message is not a magnetic control event"),
    }
    Ok(())
}

/// Current profile reducer values are persisted before the hardware task.
pub fn persist_local(message: &Value, profile: &mut Value) -> anyhow::Result<()> {
    let payload = &message["payload"];
    match message["type"]
        .as_str()
        .context("Keyboard local intent type is missing")?
    {
        "ON_SET_KEYMAPPING" => {
            let rows = payload["mappingList"]
                .as_array()
                .context("Keyboard mapping list is absent")?;
            ensure!(
                rows.iter().all(Value::is_object),
                "Keyboard mapping record is not an object"
            );
            profile["mappings"] = Value::Array(rows.clone());
        }
        "ON_SYNC_GLOBAL_ACTUATION" | "ON_SYNC_GLOBAL_RAPID_TRIGGER" => {}
        "ON_TOGGLE_CONTINUOUS_RAPID_TRIGGER" => {
            profile["continuousRapidTrigger"] = payload["value"].clone()
        }
        "ON_SET_BOTTOM_DEADZONE" => profile["bottomDeadzone"] = payload["bottomDeadzone"].clone(),
        "ON_SET_ENABLE_KEYBOARD_KEYS" => {
            profile["isEnableKeyboardKeys"] = Value::Bool(
                payload["isEnableKeyboardKeys"]
                    .as_bool()
                    .context("Keyboard input enabled state is absent")?,
            );
        }
        "ON_SET_GAMEPAD_OVERRIDE_KEYS" => {
            profile["isGamepadOverride"] = Value::Bool(
                payload["isGamepadOverride"]
                    .as_bool()
                    .context("Gamepad override state is absent")?,
            );
        }
        "ON_SET_INTERRUPT_KEYS" => controls::persist_interrupt(profile, payload)?,
        "ON_SET_GAME_MODE_COMBINATION_KEYS" => {
            if !profile["gamingMode"].is_object() {
                profile["gamingMode"] = serde_json::json!({});
            }
            profile["gamingMode"]["customGameMode"] = payload.clone();
        }
        "ON_SET_SNAP_TAP" => profile["snapTap"] = snap::local_settings(payload)?,
        "ON_SET_OBM_SINGLE_KEY_SNAP_TAP" => {
            let enabled = payload["isEnabled"]
                .as_bool()
                .context("Single-key Snap Tap enabled state is absent")?;
            let keys = payload["keyList"]
                .as_array()
                .context("Single-key Snap Tap keys are absent")?;
            profile["singleKeySnapTap"] = serde_json::json!({"isEnabled":enabled,"keyList":keys});
        }
        "ON_TOGGLE_SINGLE_KEY_SNAP_TAP" => {
            let enabled = payload["isEnabled"]
                .as_bool()
                .context("Single-key Snap Tap enabled state is absent")?;
            ensure!(
                profile["singleKeySnapTap"].is_object(),
                "Single-key Snap Tap local state is absent"
            );
            profile["singleKeySnapTap"]["isEnabled"] = Value::Bool(enabled);
        }
        event => {
            anyhow::bail!("Keyboard local intent {event} requires its captured profile reducer")
        }
    }
    Ok(())
}

fn scalar_step(
    device: &dyn FeatureTransport,
    cap: &KeyboardAnalogTransportCapability,
    profile_id: u8,
    command: [u8; 3],
    payload: Vec<u8>,
    receipt: &mut Receipt,
    next: &mut impl FnMut() -> anyhow::Result<u8>,
    guard: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    guard()?;
    receipt.submissions.push(Submission {
        profile_id,
        command,
        payload,
        attempted: false,
        acknowledgement: None,
    });
    let transaction = next()?;
    let step = receipt.submissions.last_mut().unwrap();
    let result = feature_exchange::exchange_with_report_data_size_observing_send(
        device,
        &cap.device_read_capability(),
        command,
        &step.payload,
        transaction,
        command[0],
        guard,
        &mut || step.attempted = true,
    );
    step.acknowledgement = feature_exchange::acknowledged_reply(&result).map(<[u8]>::to_vec);
    result?;
    Ok(())
}

fn analog_step(
    device: &dyn FeatureTransport,
    cap: &KeyboardAnalogTransportCapability,
    profile_id: u8,
    change: KeyboardAnalogChange,
    receipt: &mut Receipt,
    next: &mut impl FnMut() -> anyhow::Result<u8>,
    guard: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let mut prepared = keyboard_analog::prepared_write_payloads(cap, &change)?;
    ensure!(
        prepared.len() == 1,
        "Analog submission must be one source chunk"
    );
    let (command, payload) = prepared.pop().unwrap();
    receipt.submissions.push(Submission {
        profile_id,
        command,
        payload,
        attempted: false,
        acknowledgement: None,
    });
    let outcome = keyboard_analog::write(device, cap, change, false, next, guard);
    let step = receipt.submissions.last_mut().unwrap();
    match outcome {
        Ok(report) => {
            step.attempted = report.attempted_writes > 0;
            step.acknowledgement = report
                .acknowledgements
                .into_iter()
                .next()
                .map(|ack| ack.data);
            Ok(())
        }
        Err(error) => {
            step.attempted = error.report.attempted_writes > 0;
            step.acknowledgement = error
                .report
                .acknowledgements
                .first()
                .map(|ack| ack.data.clone());
            Err(error.into())
        }
    }
}

#[path = "keyboard_gaming_submission.rs"]
mod gaming;

#[path = "keyboard_magnetic_snap.rs"]
mod magnetic_snap;
#[path = "keyboard_snap_submission.rs"]
mod snap;
pub use magnetic_snap::Settings as SingleKeySnapTap;

#[path = "keyboard_macro_submission.rs"]
mod macro_submission;
pub use macro_submission::MacroProgress;

#[cfg(test)]
#[path = "keyboard_submission_tests.rs"]
mod tests;
