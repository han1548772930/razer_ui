//! Explicit hardware receipt for the complete current 679 read consumer.
use anyhow::{Context as _, ensure};
use razer_device::{
    backend::FeatureTransport,
    device_reads::DeviceReadCapability,
    keyboard_onboard_profiles::{self as onboard, CompleteReading, ReadOptions, Reading},
    keyboard_profile_inventory::ProfileMetadataObservation,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    time::{Duration, Instant},
};

pub(super) fn read(
    device: &dyn FeatureTransport,
    cap: &DeviceReadCapability,
    layout: Option<u8>,
    next: &mut impl FnMut() -> anyhow::Result<u8>,
) -> Value {
    let result = (|| {
        let options = ReadOptions {
            layout_id: u32::from(
                layout.context("Onboard reads require the current observed keyboard layout")?,
            ),
            locally_known_macro_guids: BTreeSet::new(),
        };
        let started = Instant::now();
        let validate = || {
            ensure!(
                started.elapsed() < Duration::from_secs(120),
                "Complete onboard profile read deadline expired"
            );
            Ok(())
        };
        onboard::read_complete(device, cap, &options, next, &validate)
    })();
    match result {
        Ok(observation) => {
            let incomplete = incomplete_fields(&observation);
            json!({"query":"keyboard_onboard_profiles", "error":null,
                "value":{"verified":incomplete.is_empty(), "incomplete":incomplete,
                    "observation":observation}})
        }
        Err(error) => json!({"query":"keyboard_onboard_profiles", "value":null,
            "error":format!("{error:#}")}),
    }
}

fn require_observed<T>(reading: &Reading<T>, field: &str, incomplete: &mut Vec<String>) {
    match reading {
        Reading::Observed(_) => {}
        Reading::Unobserved => incomplete.push(format!("{field}: unobserved")),
        Reading::Failed { message } => incomplete.push(format!("{field}: {message}")),
    }
}

fn require_flag(raw: u8, field: &str, incomplete: &mut Vec<String>) {
    if raw > 1 {
        incomplete.push(format!("{field}: unknown flag {raw}"));
    }
}

fn incomplete_fields(reading: &CompleteReading) -> Vec<String> {
    let mut incomplete = Vec::new();
    for slot in &reading.snapshot.inventory.slots {
        match &slot.metadata {
            ProfileMetadataObservation::Observed { .. } => {}
            ProfileMetadataObservation::Failed { message } => {
                incomplete.push(format!("slot {} metadata: {message}", slot.slot_id));
            }
            ProfileMetadataObservation::SourceSkippedFirstSlot => {
                incomplete.push(format!("slot {} metadata: unobserved", slot.slot_id));
            }
        }
    }
    for profile in &reading.snapshot.profiles {
        let field = format!("slot {}", profile.slot_id);
        require_observed(
            &profile.brightness,
            &format!("{field} brightness"),
            &mut incomplete,
        );
        for pass in &profile.passes {
            for batch in &pass.assignments {
                for (attempt, reading) in batch.attempts.iter().enumerate() {
                    require_observed(
                        reading,
                        &format!(
                            "{field} {:?} keys {:?} attempt {attempt}",
                            pass.mode, batch.key_ids
                        ),
                        &mut incomplete,
                    );
                }
            }
        }
        for batch in &profile.rapid_trigger {
            require_observed(
                &batch.reading,
                &format!("{field} rapid trigger keys {:?}", batch.key_ids),
                &mut incomplete,
            );
            if let Some(keys) = batch.reading.observed() {
                for key in keys {
                    require_flag(
                        key.enabled,
                        &format!("{field} rapid trigger key {}", key.key_id),
                        &mut incomplete,
                    );
                }
            }
        }
        for batch in &profile.actuation {
            require_observed(
                &batch.reading,
                &format!("{field} actuation keys {:?}", batch.key_ids),
                &mut incomplete,
            );
        }
        require_observed(
            &profile.kill_switch,
            &format!("{field} kill switch"),
            &mut incomplete,
        );
        if let Some(switch) = profile.kill_switch.observed() {
            require_flag(
                switch.raw_enabled,
                &format!("{field} kill switch"),
                &mut incomplete,
            );
        }
    }
    for detail in &reading.import_details.macros {
        require_observed(
            &detail.content,
            &format!("macro {} content", detail.macro_id),
            &mut incomplete,
        );
    }
    for detail in &reading.import_details.continuous_rapid_trigger {
        require_observed(
            &detail.raw_enabled,
            &format!("slot {} continuous rapid trigger", detail.slot_id),
            &mut incomplete,
        );
        if let Some(raw) = detail.raw_enabled.observed() {
            require_flag(
                *raw,
                &format!("slot {} continuous rapid trigger", detail.slot_id),
                &mut incomplete,
            );
        }
    }
    incomplete
}

#[test]
fn observed_zero_and_empty_bytes_remain_distinct_from_failed_or_unobserved() {
    let mut incomplete = Vec::new();
    require_observed(&Reading::Observed(0u8), "disabled", &mut incomplete);
    require_observed(
        &Reading::Observed(Vec::<u8>::new()),
        "empty macro",
        &mut incomplete,
    );
    assert!(incomplete.is_empty());
    require_observed(
        &Reading::<u8>::Unobserved,
        "slot 1 brightness",
        &mut incomplete,
    );
    require_observed(
        &Reading::<u8>::Failed {
            message: "device rejected".into(),
        },
        "slot 2 brightness",
        &mut incomplete,
    );
    assert_eq!(
        incomplete,
        [
            "slot 1 brightness: unobserved",
            "slot 2 brightness: device rejected"
        ]
    );
}

#[test]
fn unknown_enabled_codes_do_not_pass_as_observed_booleans() {
    let mut incomplete = Vec::new();
    for raw in [0, 1] {
        require_flag(raw, "rapid trigger", &mut incomplete);
    }
    assert!(incomplete.is_empty());
    for raw in [2, 255] {
        require_flag(raw, "rapid trigger", &mut incomplete);
    }
    assert_eq!(
        incomplete,
        [
            "rapid trigger: unknown flag 2",
            "rapid trigger: unknown flag 255"
        ]
    );
}
