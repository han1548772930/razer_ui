//! Blocking device/service work. The retained UI coordinator owns ordering;
//! every side effect rechecks its generation and actual transport identity.
use super::publisher::Publisher;
use super::*;
use anyhow::{Context as _, ensure};
use razer_device::{
    keyboard_latest_mapping::{
        self as mapping,
        features::{self, Action},
    },
    keyboard_latest_protocol::{
        self as protocol, Operation, Reply as DeviceReply, Setting, Target,
    },
    keyboard_latest_restore::{self as restore, BleProfiles, BleSwitch, Generation, Slot},
};
use razer_discovery::direct::{keyboard_latest_restore as direct, keyboard_metadata};
use razer_ipc::{ServiceClient, ServiceRequest};
use razer_storage::{keyboard_analog_restore::Journal, source_local};

struct NavigationReset {
    at: Instant,
    document: Value,
    generation: Generation,
    _recovery: PendingRecovery,
}

fn execute(
    client: &mut ServiceClient,
    target: &Target,
    generation: &Generation,
    operation: Operation,
) -> anyhow::Result<DeviceReply> {
    generation.check()?;
    let reply = direct::execute(client, target, operation)?;
    generation.check()?;
    Ok(reply)
}
fn write(
    client: &mut ServiceClient,
    target: &Target,
    generation: &Generation,
    setting: Setting,
) -> anyhow::Result<()> {
    execute(client, target, generation, Operation::Setting(setting))?.require_acknowledged()
}
fn collection_document(
    document: &Value,
    collection: Option<&CollectionUpdate>,
) -> anyhow::Result<Value> {
    match collection {
        Some(collection) => collection.document(document),
        None => Ok(document.clone()),
    }
}

fn retain_collection(
    update: &CollectionUpdate,
    current_ble: &mut Option<(Value, Generation, u64)>,
    ble: &mut BleProfiles,
) -> anyhow::Result<()> {
    let current = current_ble
        .as_ref()
        .map(|(document, generation, revision)| -> anyhow::Result<_> {
            Ok((update.document(document)?, generation.clone(), *revision))
        })
        .transpose()?;
    let mut deferred = ble.clone();
    deferred.append_collection(&update.profiles)?;
    *current_ble = current;
    *ble = deferred;
    Ok(())
}

fn cache_collection(profiles: &ProfileCollection, valid: &AtomicBool) -> anyhow::Result<()> {
    ensure!(valid.load(Ordering::Acquire), "键盘配置集合会话已结束");
    source_local::update(&source_local::path()?, "synapse_756", |stored| {
        ensure!(valid.load(Ordering::Acquire), "键盘配置集合会话已结束");
        let Some(mut document) = stored else {
            return Ok(None);
        };
        if profiles.append_to(&mut document)? {
            Ok(Some(document))
        } else {
            Ok(None)
        }
    })?;
    ensure!(valid.load(Ordering::Acquire), "键盘配置集合会话已结束");
    Ok(())
}

fn cache(
    document: &Value,
    generation: &Generation,
    collection: Option<&CollectionUpdate>,
) -> anyhow::Result<()> {
    generation.check()?;
    let document = collection_document(document, collection)?;
    source_local::update(&source_local::path()?, "synapse_756", |_| {
        generation.check()?;
        Ok(Some(document.clone()))
    })?;
    generation.check()
}
fn cache_compiled(
    expected: &Value,
    document: &Value,
    generation: &Generation,
    collection: Option<&CollectionUpdate>,
) -> anyhow::Result<bool> {
    generation.check()?;
    let document = collection_document(document, collection)?;
    let saved = source_local::update(&source_local::path()?, "synapse_756", |stored| {
        generation.check()?;
        if stored.as_ref().is_none_or(|stored| {
            stored["activeProfile"] != expected["activeProfile"]
                || stored["version"] != expected["version"]
        }) {
            return Ok(None);
        }
        Ok(Some(document.clone()))
    })?;
    generation.check()?;
    Ok(saved)
}

fn observe_power_change(
    client: &mut ServiceClient,
    target: &Target,
    generation: &Generation,
    revision_signal: &AtomicU64,
    revision: u64,
    change: features::PowerChange,
) -> anyhow::Result<Option<Value>> {
    let readings = change
        .operations()
        .into_iter()
        .map(|operation| execute(client, target, generation, operation))
        .collect::<anyhow::Result<Vec<_>>>()?;
    generation.check()?;
    let mut candidate = None;
    source_local::update(&source_local::path()?, "synapse_756", |stored| {
        generation.check()?;
        // A queued local edit keeps ownership until it has reached this
        // worker. Retain the event and read again after that edit completes.
        if revision_signal.load(Ordering::Acquire) != revision {
            return Ok(None);
        }
        let document = stored.context("756 power event has no saved source document")?;
        candidate = Some(change.observe(&document, &readings)?);
        Ok(candidate.clone())
    })?;
    generation.check()?;
    Ok(candidate)
}

fn reset_navigation_feedback(
    client: &mut ServiceClient,
    target: &Target,
    generation: &Generation,
    document: &Value,
    serial: &str,
    journal: &Journal,
    observed: &mut [protocol::ProfileMetadata],
) -> anyhow::Result<()> {
    let guid = features::profile(document)?["guid"]
        .as_str()
        .context("756 navigation reset active GUID is absent")?;
    let operations = features::navigation_reset_operations(document, serial)?;
    let mut failures = Vec::new();
    let mut journal_started = false;
    for operation in operations {
        generation.check()?;
        let physical = matches!(&operation, Operation::Actuation { points, .. }
            if points.iter().any(|point| point.key_id != 255));
        if physical && !journal_started {
            journal.begin(1, guid, &uuid::Uuid::new_v4().to_string())?;
            journal_started = true;
        }
        if let Err(error) = execute(client, target, generation, operation)
            .and_then(|reply| reply.require_acknowledged())
        {
            generation.check()?;
            failures.push(format!("行程重置：{error:#}"));
        }
    }
    // Source reset tasks begin a pending physical restore; they do not mark
    // default-only writes as completion of the profile's saved AT/RT settings.
    generation.check()?;
    let metadata = protocol::ProfileMetadata {
        slot_id: 1,
        guid: guid.into(),
        name: String::new(),
        owner: String::new(),
        key_mapping_hash: observed
            .iter()
            .find(|profile| profile.slot_id == 1)
            .map(|profile| profile.key_mapping_hash)
            .unwrap_or([0; 6]),
    };
    match execute(
        client,
        target,
        generation,
        Operation::WriteProfileMetadata(metadata.clone()),
    )
    .and_then(|reply| reply.require_acknowledged())
    {
        Ok(()) => {
            if let Some(profile) = observed.iter_mut().find(|profile| profile.slot_id == 1) {
                *profile = metadata;
            }
        }
        Err(error) => {
            generation.check()?;
            failures.push(format!("软件槽配置标识：{error:#}"));
        }
    }
    ensure!(failures.is_empty(), "{}", failures.join("; "));
    generation.check()
}

fn run_action(
    action: Action,
    client: &mut ServiceClient,
    target: &Target,
    generation: &Generation,
    journal: Option<&Journal>,
    caps: &mapping::travel::Caps,
) -> anyhow::Result<()> {
    generation.check()?;
    match action {
        Action::Write(setting) => write(client, target, generation, setting),
        Action::Analog {
            slot,
            guid,
            removed,
            mappings,
        } => {
            let pair = mapping::analog_pair_with_caps(slot, &guid, &removed, &mappings, caps)?;
            let journal = journal.context("Analog restore requires the actual serial journal")?;
            let token = uuid::Uuid::new_v4().to_string();
            generation.check()?;
            journal.begin(slot, &guid, &token)?;
            generation.check()?;
            // The source catches chunk failures inside each operation and still
            // attempts RT. Keep the pending journal unless both fully ACK.
            let mut actuation_errors = Vec::new();
            for chunk in pair.actuation.chunks(15) {
                generation.check()?;
                if let Err(error) = execute(
                    client,
                    target,
                    generation,
                    Operation::Actuation {
                        slot,
                        points: chunk.to_vec(),
                    },
                )
                .and_then(|reply| reply.require_acknowledged())
                {
                    actuation_errors.push(format!("{error:#}"));
                }
            }
            generation.check()?;
            let mut rapid_errors = Vec::new();
            for chunk in pair.rapid_trigger.chunks(13) {
                generation.check()?;
                if let Err(error) = execute(
                    client,
                    target,
                    generation,
                    Operation::RapidTrigger {
                        slot,
                        keys: chunk.to_vec(),
                    },
                )
                .and_then(|reply| reply.require_acknowledged())
                {
                    rapid_errors.push(format!("{error:#}"));
                }
            }
            generation.check()?;
            match (actuation_errors.is_empty(), rapid_errors.is_empty()) {
                (true, true) => {
                    ensure!(
                        journal.complete(slot, &token)?,
                        "Analog restore journal token changed before completion"
                    );
                    generation.check()
                }
                _ => anyhow::bail!(
                    "Analog restore failed: actuation=[{}]; rapid trigger=[{}]",
                    actuation_errors.join("; "),
                    rapid_errors.join("; ")
                ),
            }
        }
        Action::Gaming { slots, mode } => {
            let pc = 3
                | if mode["isAltTabDisabled"] == true { 4 } else { 0 }
                | if mode["isAltF4Disabled"] == true { 8 } else { 0 };
            for slot in slots {
                let DeviceReply::GameModeSelection {
                    selection,
                    extension,
                    bytes,
                    ..
                } = execute(
                    client,
                    target,
                    generation,
                    Operation::ReadGameModeSelection { slot },
                )?
                else {
                    anyhow::bail!("756 game mode getter reply differs")
                };
                ensure!(
                    bytes.len() >= 3,
                    "756 extended game mode response lacks extension byte"
                );
                let extension = match mode["isAltSpaceDisabled"].as_bool() {
                    Some(true) => extension | 1,
                    Some(false) => extension & !1,
                    None => extension,
                };
                write(
                    client,
                    target,
                    generation,
                    Setting::GameModeSelection {
                        slot,
                        selection: (pc & 15) | (selection & 240),
                        extension: Some(extension),
                    },
                )?;
            }
            Ok(())
        }
        Action::SnapTap {
            slot,
            enabled,
            pairs,
        } => {
            let DeviceReply::SnapTap {
                enabled: observed,
                pairs: actual,
                ..
            } = execute(
                client,
                target,
                generation,
                Operation::ReadSnapTap { slot: 1 },
            )?
            else {
                anyhow::bail!("756 Snap Tap getter reply differs")
            };
            if observed != enabled || actual != pairs {
                write(
                    client,
                    target,
                    generation,
                    Setting::SnapTap {
                        slot: 1,
                        enabled,
                        pairs: pairs.clone(),
                    },
                )?;
            }
            // Source always writes the first matching OBM slot (or slot1 again).
            write(
                client,
                target,
                generation,
                Setting::SnapTap {
                    slot,
                    enabled,
                    pairs,
                },
            )
        }
        Action::SingleKeySnapTap {
            slots,
            enabled,
            keys,
        } => {
            for slot in slots {
                if keys.is_empty() {
                    let DeviceReply::SingleKeySnapTap { enabled, keys, .. } = execute(
                        client,
                        target,
                        generation,
                        Operation::ReadSingleKeySnapTap { slot },
                    )?
                    else {
                        anyhow::bail!("756 single-key Snap Tap getter reply differs")
                    };
                    if !enabled {
                        continue;
                    }
                    ensure!(
                        !keys.is_empty() && keys.len() <= 4 && keys.iter().all(|key| key[0] > 0),
                        "Cannot safely disable single-key Snap Tap: actual configured keys are invalid"
                    );
                    write(
                        client,
                        target,
                        generation,
                        Setting::SingleKeySnapTap {
                            slot,
                            enabled: false,
                            keys,
                        },
                    )?;
                    ensure!(
                        matches!(
                            execute(
                                client,
                                target,
                                generation,
                                Operation::ReadSingleKeySnapTap { slot }
                            )?,
                            DeviceReply::SingleKeySnapTap { enabled: false, .. }
                        ),
                        "Single-key Snap Tap disable verification failed"
                    );
                } else {
                    write(
                        client,
                        target,
                        generation,
                        Setting::SingleKeySnapTap {
                            slot,
                            enabled,
                            keys: keys.clone(),
                        },
                    )?;
                }
            }
            Ok(())
        }
        Action::CustomGame {
            slots,
            combos,
            extension_updates,
        } => {
            write(
                client,
                target,
                generation,
                Setting::CustomGameKeys { slot: 1, combos },
            )?;
            let DeviceReply::GameModeSelection {
                selection,
                extension,
                ..
            } = execute(
                client,
                target,
                generation,
                Operation::ReadGameModeSelection { slot: 1 },
            )?
            else {
                anyhow::bail!("756 custom game mode getter reply differs")
            };
            let mut extension = extension;
            for (index, enabled) in extension_updates.iter().enumerate() {
                if *enabled {
                    extension |= 1 << (index + 1);
                } else {
                    extension &= !(1 << (index + 1));
                }
            }
            write(
                client,
                target,
                generation,
                Setting::GameModeSelection {
                    slot: 1,
                    selection,
                    extension: Some(extension),
                },
            )?;
            for slot in slots.into_iter().filter(|slot| *slot != 1) {
                write(
                    client,
                    target,
                    generation,
                    Setting::CustomGameKeys { slot, combos },
                )?;
                write(
                    client,
                    target,
                    generation,
                    Setting::GameModeSelection {
                        slot,
                        selection,
                        extension: Some(extension),
                    },
                )?;
            }
            Ok(())
        }
        Action::Dim { enabled, minutes } => {
            let DeviceReply::TimeToDim {
                enabled: actual,
                seconds,
                ..
            } = execute(
                client,
                target,
                generation,
                Operation::ReadTimeToDim { slot: 0 },
            )?
            else {
                anyhow::bail!("756 dim getter reply differs")
            };
            if minutes.is_finite()
                && (1. ..=15.).contains(&minutes)
                && (seconds as f64 != 60. * minutes || actual != enabled)
            {
                write(
                    client,
                    target,
                    generation,
                    Setting::TimeToDim {
                        slot: 0,
                        enabled,
                        seconds: (60. * minutes) as u16,
                        brightness: 20,
                    },
                )?;
            }
            Ok(())
        }
        Action::Sleep { enabled, minutes } => {
            let DeviceReply::AutoSleepMode {
                enabled: actual, ..
            } = execute(
                client,
                target,
                generation,
                Operation::ReadAutoSleepMode { slot: 0 },
            )?
            else {
                anyhow::bail!("756 auto-sleep getter reply differs")
            };
            let DeviceReply::TimeToSleep { seconds, .. } =
                execute(client, target, generation, Operation::ReadTimeToSleep)?
            else {
                anyhow::bail!("756 sleep getter reply differs")
            };
            if enabled != actual {
                write(
                    client,
                    target,
                    generation,
                    Setting::AutoSleepMode { slot: 0, enabled },
                )?;
            }
            if minutes.is_finite()
                && minutes > 0.
                && minutes <= 60.
                && seconds as f64 != 60. * minutes
            {
                write(
                    client,
                    target,
                    generation,
                    Setting::TimeToSleep {
                        seconds: (60. * minutes) as u16,
                    },
                )?;
            }
            Ok(())
        }
    }
}
pub(super) fn run(
    observed: ObservedDevice,
    valid: Arc<AtomicBool>,
    automatic: Arc<AtomicBool>,
    recoveries: Arc<AtomicUsize>,
    macro_exclusive: Arc<AtomicBool>,
    revision_signal: Arc<AtomicU64>,
    commands: mpsc::Receiver<Command>,
    replies: mpsc::Sender<Reply>,
) {
    let mut publisher: Option<Publisher> = None;
    let result = (|| -> anyhow::Result<()> {
        let mut client = ServiceClient::spawn()?;
        client.set_request_guard(valid.clone());
        let mut event_target = None;
        let result = (|| -> anyhow::Result<()> {
            let identity = keyboard_metadata::resolve(&mut client, &observed)?;
            let serial = identity.read_serial(&mut client)?;
            ensure!(!serial.is_empty(), "756 actual serial number is empty");
            let target = direct::resolve(&mut client, &observed)?;
            let obm_assignments = if target.ble() {
                None
            } else {
                Some(mapping::ObmButtonAssignments::from_source()?)
            };
            let slots_key = format!("keyboard756-onboard-slots:{serial}");
            let mut onboard_profiles = Vec::new();
            let mut slots = if target.ble() {
                source_local::get(&source_local::path()?, &slots_key)?
                    .map(serde_json::from_value::<Vec<Slot>>)
                    .transpose()?
                    .unwrap_or_default()
            } else {
                match direct::execute(&mut client, &target, Operation::Inventory) {
                    Ok(DeviceReply::Inventory { slots, metadata }) => {
                        ensure!(
                            slots.len() == metadata.len(),
                            "756 inventory metadata count differs"
                        );
                        onboard_profiles = slots
                            .iter()
                            .zip(&metadata)
                            .map(|(slot, bytes)| protocol::profile_metadata(slot.slot_id, bytes))
                            .collect::<anyhow::Result<Vec<_>>>()?;
                        let inventory = inventory_observation(&target, &slots, &metadata)
                            .map_err(|error| format!("{error:#}"));
                        let _ = replies.send(Reply::Inventory(inventory));
                        source_local::set(
                            &source_local::path()?,
                            &slots_key,
                            &serde_json::to_value(&slots)?,
                        )?;
                        slots
                    }
                    Ok(_) => anyhow::bail!("756 inventory response differs"),
                    Err(error) => {
                        let _ = replies.send(Reply::Inventory(Err(format!("{error:#}"))));
                        let _ = replies.send(Reply::Diagnostic(format!("板载配置清单：{error:#}")));
                        Vec::new()
                    }
                }
            };
            if target.ble() {
                slots = restore::slots(&slots);
            }
            let journal = if target.ble() {
                None
            } else {
                Some(Journal::new(&source_local::path()?, 756, &serial)?)
            };
            let mut ble = BleProfiles::default();
            let mut ble_initialized = false;
            let mut current_ble: Option<(Value, Generation, u64)> = None;
            let mut collection: Option<CollectionUpdate> = None;
            let mut current_work: Option<(Generation, u64)> = None;
            let mut pending_power = BTreeSet::new();
            let mut last_active_profile = None;
            let mut navigation_reset: Option<NavigationReset> = None;
            let mut recording_mode = String::from("kUnknown");
            match direct::execute(
                &mut client,
                &target,
                Operation::Events {
                    enabled: Some(true),
                },
            ) {
                Ok(DeviceReply::Events { .. }) => event_target = Some(target.clone()),
                Ok(_) => {
                    let _ = replies.send(Reply::Diagnostic("键盘硬件事件订阅响应不符".into()));
                }
                Err(error) => {
                    let _ = replies.send(Reply::Diagnostic(format!("键盘硬件事件订阅：{error:#}")));
                }
            }
            let _ = replies.send(Reply::Ready);
            while valid.load(Ordering::Acquire) {
                if let Some(owner) = publisher.as_mut() {
                    owner.apply_control()?;
                }
                if navigation_reset
                    .as_ref()
                    .is_some_and(|reset| !reset.generation.is_current())
                {
                    navigation_reset = None;
                }
                let wait = navigation_reset
                    .as_ref()
                    .map(|reset| {
                        reset
                            .at
                            .saturating_duration_since(Instant::now())
                            .min(Duration::from_millis(100))
                    })
                    .unwrap_or(Duration::from_millis(100));
                let work = match commands.recv_timeout(wait) {
                    Ok(Command::Restore(work)) => {
                        if collection
                            .as_ref()
                            .is_some_and(|update| work.id > update.through_revision)
                        {
                            collection = None;
                        }
                        Some(work)
                    }
                    Ok(Command::CollectionOnly { update, _pending }) => {
                        // Import updates only retained/local collections. It
                        // never enters restore compilation, native publication,
                        // slot selection, or HID writes.
                        let retained = retain_collection(&update, &mut current_ble, &mut ble)
                            .and_then(|_| cache_collection(&update.profiles, &valid));
                        collection = Some(update);
                        if let Err(error) = retained {
                            let _ = replies
                                .send(Reply::Diagnostic(format!("键盘本地配置集合：{error:#}")));
                        }
                        drop(_pending);
                        None
                    }
                    Err(mpsc::RecvTimeoutError::Timeout) => None,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                };
                if let Some(work) = work {
                    let id = work.id;
                    let generation = work.generation.clone();
                    current_work = Some((generation.clone(), id));
                    let intent_generation = work
                        .value
                        .intent
                        .as_ref()
                        .map(|(generation, _)| *generation);
                    let restored = (|| -> anyhow::Result<()> {
                        generation.check()?;
                        let mut request = work.value;
                        if let Some((_,message))=&request.intent {
                            // Current task runners explicitly ignore Sync while
                            // BLE is active; local preview is already retained.
                            ensure!(!target.ble(),"当前源码在 BLE 连接中跳过全局行程写入");
                            let mut failures=Vec::new();
                            for operation in features::global_sync_operations(&request.document,&slots,
                                obm_assignments.as_ref().context("756 OBM assignment owner is absent")?,message,&serial)? {
                                generation.check()?;
                                if let Err(error)=execute(&mut client,&target,&generation,operation).and_then(|reply|reply.require_acknowledged()) {
                                    failures.push(format!("{error:#}"));
                                }
                            }
                            ensure!(failures.is_empty(),"全局行程同步写入失败：{}",failures.join("; "));
                            return generation.check();
                        }
                        if target.ble() {
                            if request.games_only {
                                return Ok(());
                            }
                            if ble_initialized && request.ble_switch.is_none() {
                                // Current50574 collection/edit task makers save
                                // locally before26568.yo ->75938.BH publishes
                                // their UI. Only an explicit ny/sy ->ty request
                                // sends SET, including after active GUID changes.
                                cache(&request.document, &generation, collection.as_ref())?;
                                current_ble = Some((
                                    collection_document(&request.document, collection.as_ref())?,
                                    generation.clone(),
                                    id,
                                ));
                                return generation.check();
                            }
                            let stored = ble.document(&request.document).clone();
                            let mut defer=request.defer_ble_persistence;
                            if defer {
                                defer=publisher.as_mut().and_then(|owner|owner.hypershift(&generation).ok()).unwrap_or(false);
                                ble.hypershift(defer);
                            }
                            let from_notification = !ble_initialized;
                            let slot = if from_notification && request.ble_switch.is_none() {
                                let DeviceReply::BleSlot {
                                    slot: Some(slot), ..
                                } = execute(
                                    &mut client,
                                    &target,
                                    &generation,
                                    Operation::BleSlot { set: None },
                                )?
                                else {
                                    anyhow::bail!("756 BLE active-profile getter reply differs")
                                };
                                ble_initialized = true;
                                ble.target(&stored, &slots, &BleSwitch::Slot(slot))?.clone()
                            } else {
                                let slot = ble
                                    .target(
                                        &stored,
                                        &slots,
                                        request.ble_switch.as_ref().unwrap_or(&BleSwitch::Guid(
                                            request.document["activeProfile"]
                                                .as_str()
                                                .context("756 BLE requested profile is absent")?
                                                .into(),
                                        )),
                                    )?
                                    .clone();
                                let reply = execute(
                                    &mut client,
                                    &target,
                                    &generation,
                                    Operation::BleSlot {
                                        set: Some(slot.slot_id),
                                    },
                                )?;
                                ensure!(
                                    matches!(reply, DeviceReply::BleSlot { .. }),
                                    "756 BLE setter acknowledgement differs"
                                );
                                ble_initialized = true;
                                slot
                            };
                            if let Some(document) = ble.switched_for_serial(
                                &stored,
                                &slot,
                                from_notification && request.ble_switch.is_none(),
                                defer,
                                &serial,
                                mapping::source()["device_name"].as_str().context("756 source device name is absent")?,
                            )? {
                                let _ = replies.send(Reply::Candidate {
                                    revision: id, generation: generation.clone(), document: document.clone(),
                                    persist: !ble.persistence_deferred(),
                                    manual: request.ble_switch.is_some() && !request.defer_ble_persistence,
                                });
                                if !ble.persistence_deferred() {
                                    cache(&document, &generation, collection.as_ref())?;
                                }
                                current_ble = Some((collection_document(&document, collection.as_ref())?, generation.clone(), id));
                            } else {
                                current_ble = Some((collection_document(&stored, collection.as_ref())?, generation.clone(), id));
                            }
                            if publisher.is_none() {
                                match Publisher::start(observed.container()) {
                                    Ok(owner) => publisher = Some(owner),
                                    Err(error) => {
                                        let _ = replies.send(Reply::Diagnostic(format!(
                                            "BLE映射事件注册：{error:#}"
                                        )));
                                    }
                                }
                            }
                            return Ok(());
                        }
                        if let Some(target_profile)=&request.ble_switch {
                            let active=restore::navigate_local(&request.document,&slots,target_profile,true)?;
                            let changed = request.document["activeProfile"] != active;
                            request.document["activeProfile"]=serde_json::json!(active);
                            request.document["version"]=serde_json::json!(razer_device::mapping_submission::next_source_version(request.document["version"].as_u64())?);
                            if let Some(input) = &request.switch_input {
                                let feedback = features::navigation_feedback(&active, input, &onboard_profiles)?;
                                // sy advances lastActive before cy checks exclusive.
                                // This also suppresses a duplicate FN-key effect in
                                // the following full restore of the same profile.
                                last_active_profile = Some(active.clone());
                                navigation_reset = Some(NavigationReset {
                                    at: Instant::now() + Duration::from_millis(200),
                                    document: request.document.clone(),
                                    generation: generation.clone(),
                                    _recovery: PendingRecovery::start(recoveries.clone()),
                                });
                                if !macro_exclusive.load(Ordering::Acquire) {
                                    if let Some(setting) = feedback {
                                        if let Err(error) = write(&mut client, &target, &generation, setting) {
                                            generation.check()?;
                                            let _ = replies.send(Reply::Diagnostic(format!("按键配置切换效果：{error:#}")));
                                        }
                                    }
                                }
                            }
                            if !changed { return generation.check(); }
                            restore::set_active_profile_for_serial(&mut request.document,&serial,
                                mapping::source()["device_name"].as_str().context("756 source device name is absent")?)?;
                            cache(&request.document,&generation,collection.as_ref())?;
                            let _=replies.send(Reply::Candidate {revision:id,generation:generation.clone(),document:request.document.clone(),persist:true,manual:false});
                        }
                        let mut context = mapping::source_context(
                            Some(&request.document),
                            (0..512).map(|_| uuid::Uuid::new_v4().to_string()),
                        );
                        let recovered = mapping::recover_document(&request.document, &mut context)?;
                        let recovered = features::remove_dks_conflicts(&recovered)?;
                        if recovered != request.document {
                            cache(&recovered, &generation, collection.as_ref())?;
                            let _ = replies.send(Reply::Candidate {
                                revision: id, generation: generation.clone(), document: recovered.clone(),
                                persist: true, manual: false,
                            });
                        }
                        generation.check()?;
                        let mut compiled = mapping::compile_for_serial(
                            &recovered,
                            &serial,
                            request.running_games,
                            (0..512).map(|_| uuid::Uuid::new_v4().to_string()),
                        )?;
                        if request
                            .previous
                            .as_ref()
                            .and_then(|previous| features::profile(previous).ok())
                            .is_some_and(|old| {
                                features::profile(&recovered).is_ok_and(|new| {
                                    old["functionKeyPrimary"] != new["functionKeyPrimary"]
                                })
                            })
                        {
                            compiled = mapping::apply_function_key_snap_tap(&compiled)?;
                        }
                        cache(&recovered, &generation, collection.as_ref())?;
                        // Native publication failure does not suppress unrelated
                        // queued HID tasks in current RzGenericFeatureManager.
                        if publisher.is_none() {
                            match Publisher::start(observed.container()) {
                                Ok(owner) => publisher = Some(owner),
                                Err(error) => {
                                    let _ = replies.send(Reply::Diagnostic(format!(
                                        "映射引擎注册：{error:#}"
                                    )));
                                }
                            }
                        }
                        generation.check()?;
                        if let Some(owner) = publisher.as_mut() {
                            recording_mode = match owner.macro_mode(&generation) {
                                Ok(mode) => mode,
                                Err(error) => {
                                    generation.check()?;
                                    let _ = replies.send(Reply::Diagnostic(format!("宏录制模式查询：{error:#}")));
                                    "kUnknown".into()
                                }
                            };
                            let profile = features::profile(&compiled)?;
                            let generic_passes = if request.full {
                                usize::from(profile["isEnableKeyboardKeys"].is_boolean())
                                    + usize::from(profile["isEnableKeyboardKeys"] != false && profile["isGamepadOverride"].is_boolean())
                            } else { 0 };
                            if request.full && generic_passes > 0 {
                                let base = mapping::compile_firmware_for_serial(&recovered, &serial,
                                    request.running_games, (0..512).map(|_|uuid::Uuid::new_v4().to_string()))?;
                                if let Err(error) = owner.publish(base, &generation) {
                                    generation.check()?;
                                    let _ = replies.send(Reply::Diagnostic(format!("固件映射引擎发布：{error:#}")));
                                }
                            }
                            let mut cached = recovered.clone();
                            for _ in 0..generic_passes.max(1) {
                                generation.check()?;
                                if generic_passes > 0 {
                                    compiled["version"] = serde_json::json!(razer_device::mapping_submission::next_source_version(cached["version"].as_u64())?);
                                }
                                match owner.publish(compiled.clone(), &generation) {
                                    Ok(()) => {
                                        if cache_compiled(&cached, &compiled, &generation, collection.as_ref())? {
                                            cached = compiled.clone();
                                            let _ = replies.send(Reply::Candidate {
                                                revision: id, generation: generation.clone(), document: cached.clone(),
                                                persist: true, manual: false,
                                            });
                                        }
                                    }
                                    Err(error) => {
                                        generation.check()?;
                                        let _ = replies.send(Reply::Diagnostic(format!("映射引擎发布：{error:#}")));
                                    }
                                }
                            }
                        }
                        let active = compiled["activeProfile"].as_str().context("756 preset-effect active profile is absent")?;
                        let exclusive = macro_exclusive.load(Ordering::Acquire);
                        if !exclusive && last_active_profile.as_deref() != Some(active) {
                            // Source advances this guard before looking up the real
                            // OBM name; a failed/missing effect is not retried here.
                            last_active_profile = Some(active.to_owned());
                            if let Some(profile) = onboard_profiles.iter().find(|profile| profile.guid == active && !profile.name.is_empty()) {
                                if let Err(error) = write(&mut client, &target, &generation,
                                    Setting::TriggerPresetEffect { slot: profile.slot_id, key: 59 }) {
                                    generation.check()?;
                                    let _ = replies.send(Reply::Diagnostic(format!("板载配置效果：{error:#}")));
                                }
                            }
                        }
                        if exclusive && request.full {
                            eprintln!("[keyboard756] brightness restore held by recorder mode {recording_mode}");
                        }
                        let mut previous = request.previous.as_ref();
                        if request.full {
                            previous = None;
                        }
                        let mut actions = features::actions(
                            &compiled,
                            previous,
                            &slots,
                            request.running_games,
                            observed.physical_product_id() == 757,
                        )?;
                        if request.games_only {
                            actions = features::actions(
                                &compiled,
                                None,
                                &slots,
                                request.running_games,
                                observed.physical_product_id() == 757,
                            )?
                            .into_iter()
                            .filter(|action| {
                                matches!(
                                    action,
                                    Action::Gaming { .. } | Action::Write(Setting::LedState { .. })
                                )
                            })
                            .collect();
                        }
                        for action in actions {
                            generation.check()?;
                            if macro_exclusive.load(Ordering::Acquire) && matches!(&action, Action::Write(Setting::Brightness { .. })) {
                                continue;
                            }
                            if let Err(error) = run_action(
                                action,
                                &mut client,
                                &target,
                                &generation,
                                journal.as_ref(),
                                &mapping::travel::Caps::for_serial(&compiled, &serial)?,
                            ) {
                                generation.check()?;
                                let _ =
                                    replies.send(Reply::Diagnostic(format!("配置恢复：{error:#}")));
                            }
                        }
                        generation.check()
                    })()
                    .map_err(|error| format!("{error:#}"));
                    if let Some(intent) = intent_generation {
                        let _ = replies.send(Reply::IntentFinished(intent, restored.clone()));
                    }
                    let _ = replies.send(Reply::Finished(id, generation, restored));
                }
                if !valid.load(Ordering::Acquire) {
                    break;
                }
                if let Some(owner) = publisher.as_mut() {
                    owner.apply_control()?;
                }
                if navigation_reset
                    .as_ref()
                    .is_some_and(|reset| reset.at <= Instant::now())
                {
                    let reset = navigation_reset.take().expect("due navigation reset");
                    if reset.generation.is_current() {
                        if let Err(error) = reset_navigation_feedback(
                            &mut client,
                            &target,
                            &reset.generation,
                            &reset.document,
                            &serial,
                            journal
                                .as_ref()
                                .context("756 navigation reset journal is absent")?,
                            &mut onboard_profiles,
                        ) {
                            if reset.generation.is_current() {
                                let _ = replies.send(Reply::Diagnostic(format!(
                                    "配置切换行程恢复：{error:#}"
                                )));
                            }
                        }
                    }
                }
                if !automatic.load(Ordering::Acquire) {
                    continue;
                }
                if event_target.is_some() {
                    match direct::execute(&mut client, &target, Operation::Events { enabled: None })
                    {
                        Ok(DeviceReply::Events { packets }) => {
                            for packet in packets {
                                if let Some(change) = features::PowerChange::from_packet(&packet) {
                                    pending_power.insert(change);
                                }
                                if target.ble()
                                    && packet.first() == Some(&5)
                                    && packet.get(1) == Some(&1)
                                {
                                    if let (Some(slot), Some((stored, generation, revision))) = (
                                        packet.get(2).copied().filter(|slot| *slot > 0),
                                        current_ble.clone(),
                                    ) {
                                        if generation.is_current() {
                                            let changed = (|| -> anyhow::Result<()> {
                                                let slot = ble
                                                    .target(
                                                        &stored,
                                                        &slots,
                                                        &BleSwitch::Slot(slot),
                                                    )?
                                                    .clone();
                                                if let Some(document) = ble.switched_for_serial(
                                                    &stored,
                                                    &slot,
                                                    true,
                                                    false,
                                                    &serial,
                                                    mapping::source()["device_name"]
                                                        .as_str()
                                                        .context(
                                                            "756 source device name is absent",
                                                        )?,
                                                )? {
                                                    let persist = !ble.persistence_deferred();
                                                    let _ = replies.send(Reply::Candidate {
                                                        revision,
                                                        generation: generation.clone(),
                                                        document: document.clone(),
                                                        persist,
                                                        manual: false,
                                                    });
                                                    if persist {
                                                        cache(
                                                            &document,
                                                            &generation,
                                                            collection.as_ref(),
                                                        )?;
                                                    }
                                                    current_ble =
                                                        Some((document, generation, revision));
                                                }
                                                Ok(())
                                            })(
                                            );
                                            if let Err(error) = changed {
                                                let _ = replies.send(Reply::Diagnostic(format!(
                                                    "BLE板载配置通知：{error:#}"
                                                )));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        Ok(_) => {
                            let _ = replies.send(Reply::Diagnostic("键盘硬件事件响应不符".into()));
                        }
                        Err(error) => {
                            let _ =
                                replies.send(Reply::Diagnostic(format!("键盘硬件事件：{error:#}")));
                        }
                    }
                }
                if let Some((generation, revision)) =
                    current_work.as_ref().filter(|(generation, revision)| {
                        generation.is_current()
                            && revision_signal.load(Ordering::Acquire) == *revision
                    })
                {
                    for change in std::mem::take(&mut pending_power) {
                        match observe_power_change(
                            &mut client,
                            &target,
                            generation,
                            &revision_signal,
                            *revision,
                            change,
                        ) {
                            Ok(Some(document)) => {
                                if target.ble() {
                                    current_ble =
                                        Some((document.clone(), generation.clone(), *revision));
                                }
                                let _ = replies.send(Reply::Candidate {
                                    revision: *revision,
                                    generation: generation.clone(),
                                    document,
                                    persist: true,
                                    manual: false,
                                });
                            }
                            Ok(None) => {
                                pending_power.insert(change);
                            }
                            Err(_)
                                if !generation.is_current()
                                    || revision_signal.load(Ordering::Acquire) != *revision =>
                            {
                                pending_power.insert(change);
                            }
                            Err(error) => {
                                let _ = replies.send(Reply::Diagnostic(format!(
                                    "设备省电状态回读：{error:#}"
                                )));
                            }
                        }
                    }
                }
                if let Some(owner) = publisher.as_mut() {
                    match owner.events() {
                        Ok(events) => {
                            for event in events {
                                if target.ble()
                                    && event["source"] == "devicemodechanged"
                                    && event["eventType"] == 3
                                {
                                    let state = event["event"]
                                        .as_str()
                                        .and_then(|text| serde_json::from_str::<Value>(text).ok())
                                        .unwrap_or_else(|| event["event"].clone());
                                    if let Some(enabled) = state["isEnabled"].as_bool() {
                                        ble.hypershift(enabled);
                                        if !enabled {
                                            if let Some(document) = ble.deferred().cloned() {
                                                if let Some((_, generation, revision)) =
                                                    current_ble.as_ref()
                                                {
                                                    match cache(
                                                        &document,
                                                        generation,
                                                        collection.as_ref(),
                                                    ) {
                                                        Ok(()) => {
                                                            ble.persisted();
                                                            let _ =
                                                                replies.send(Reply::Candidate {
                                                                    revision: *revision,
                                                                    generation: generation.clone(),
                                                                    document,
                                                                    persist: true,
                                                                    manual: false,
                                                                });
                                                        }
                                                        Err(error) => {
                                                            let _ = replies.send(
                                                                Reply::Diagnostic(format!(
                                                                    "BLE临时配置持久化：{error:#}"
                                                                )),
                                                            );
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                let _ = replies.send(Reply::Event(event));
                            }
                        }
                        Err(error) => {
                            let _ = replies.send(Reply::Diagnostic(format!("映射事件：{error:#}")));
                        }
                    }
                }
            }
            ble.reset();
            Ok(())
        })();
        client.set_request_guard(Arc::new(AtomicBool::new(true)));
        // The callback subscription survives an interrupted command. Disable it
        // with the actual retained target before the transport is released.
        if let Some(target) = event_target {
            let cleanup = direct::execute(
                &mut client,
                &target,
                Operation::Events {
                    enabled: Some(false),
                },
            );
            match cleanup {
                Ok(DeviceReply::Events { .. }) => {}
                Ok(_) => {
                    let _ = replies.send(Reply::Diagnostic("键盘硬件事件取消订阅响应不符".into()));
                }
                Err(error) => {
                    let _ = replies.send(Reply::Diagnostic(format!(
                        "键盘硬件事件取消订阅：{error:#}"
                    )));
                }
            }
        }
        match client.request(ServiceRequest::Shutdown) {
            Ok(receipt) if receipt["stopped"] == true => {}
            Ok(_) => {
                let _ = replies.send(Reply::Diagnostic("键盘恢复通信清理未确认".into()));
            }
            Err(error) => {
                let _ = replies.send(Reply::Diagnostic(format!("键盘恢复通信清理：{error:#}")));
            }
        }
        result
    })();
    if let Some(owner) = publisher {
        if let Err(error) = owner.finish() {
            let _ = replies.send(Reply::Diagnostic(format!("映射引擎退出：{error:#}")));
        }
    }
    if let Err(error) = result {
        let _ = replies.send(Reply::Diagnostic(format!("{error:#}")));
    }
    let _ = replies.send(Reply::Stopped);
}

fn inventory_observation(
    target: &Target,
    slots: &[Slot],
    metadata: &[Vec<u8>],
) -> anyhow::Result<razer_ipc::keyboard_profile_inventory::SlotsObservation> {
    use razer_ipc::keyboard_profile_inventory::{SlotsObservation, Target as InventoryTarget};
    let target = match target {
        Target::Collection(node) => InventoryTarget::Collection {
            node: node.clone(),
            product_id: 756,
            peer_product_id: None,
        },
        Target::WindowsContainer(target) => InventoryTarget::WindowsContainer {
            target: target.clone(),
        },
        Target::WindowsBle(_) => anyhow::bail!("BLE runtime does not observe HID profile slots"),
    };
    let slots = razer_device::keyboard_profile_inventory::decode_756_slots(slots, metadata)?;
    let observation = SlotsObservation { target, slots };
    observation.validate_for(&observation.target)?;
    Ok(observation)
}
