//! Current PID226 BLE228 DPI tasks use the retained Protocol30 GATT owner.
use super::mouse_runtime_artwork_adapter::ble::{self as gatt, Apartment, Connection};
use super::receiver;
use anyhow::{Context as _, ensure};
use razer_device::{
    mouse_current_dpi::{self as dpi, Acknowledgement, DpiChange, ble},
    mouse_runtime_artwork::WindowsBleTarget,
};
use razer_ipc::mouse_current_dpi_ble::{Context, Outcome, Profiles};
use serde_json::{Value, json};
use std::time::{Duration, Instant};

struct Session {
    connection: Connection,
    sequence: u8,
}

trait SessionTransport: ble::Transport {
    fn finish(&mut self) -> anyhow::Result<()>;
}

impl SessionTransport for Session {
    fn finish(&mut self) -> anyhow::Result<()> {
        self.connection.finish()
    }
}

impl ble::Transport for Session {
    fn exchange(
        &mut self,
        command: [u8; 3],
        profile: u8,
        payload: &[u8],
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Acknowledgement> {
        let transaction = self.sequence;
        let frame = ble::frame(transaction, command, profile, payload)?;
        self.sequence = if transaction == 30 { 0 } else { transaction + 1 };
        let data = self.connection.exchange_with_retry(&frame, validate)?;
        if !matches!(command, ble::SET_STAGES | ble::SET_LEVEL) {
            validate()?;
        }
        Ok(Acknowledgement {
            command,
            profile: Some(profile),
            transaction,
            response_bytes: data.len(),
            data,
        })
    }
}

pub(super) fn request(
    target: &WindowsBleTarget,
    change: Option<&DpiChange>,
    context: Option<&Context>,
    cancellation: Option<&razer_ipc::request_cancellation::Token>,
) -> anyhow::Result<Value> {
    ensure!(
        ble::accepts(target.product_id, target.physical_product_id),
        "Current DPI BLE source factory differs"
    );
    if let Some(change) = change {
        dpi::validate_change_for(target.product_id, change)?;
    }
    if let Some(context) = context {
        context.validate_request(target, change)?;
    }
    let cancellation = razer_ipc::request_cancellation::CancellationReader::open(
        cancellation.context("Current DPI BLE request has no cancellation owner")?,
    )?;
    let _apartment = Apartment::new()?;
    let started = Instant::now();
    let validate = || {
        cancellation.check()?;
        ensure!(
            started.elapsed() < Duration::from_secs(20),
            "Current DPI BLE owner deadline expired"
        );
        gatt::current_target(target)
    };
    validate()?;
    let _lock = receiver::ReceiverLock::acquire(&target.device_container_id, &validate)?;
    validate()?;
    let mut session = Session {
        connection: Connection::connect_with_retry(target, &validate)?,
        sequence: 0,
    };
    run_session(target, change, context, &mut session, &validate)
}

fn run_session(
    target: &WindowsBleTarget,
    change: Option<&DpiChange>,
    context: Option<&Context>,
    session: &mut impl SessionTransport,
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Value> {
    let result = (|| {
        if let Some(context) = context {
            context.validate_request(target, change)?;
        }
        validate()?;
        let edition = ble::Transport::exchange(session, ble::EDITION, 0, &[], validate)?;
        ensure!(
            edition.command == ble::EDITION
                && edition.profile == Some(0)
                && edition.transaction < 31
                && edition.response_bytes == edition.data.len()
                && (3..=65535).contains(&edition.data.len()),
            "Current DPI BLE connection edition is truncated"
        );
        let mut outcome = Outcome {
            report: None,
            serial_number: None,
            initialization: None,
            observed_slots: None,
            error: None,
            cleanup_error: None,
        };
        if let Some(context) = context {
            match &context.profiles {
                Profiles::ReadIdentity => match ble::read_serial(session, validate) {
                    Ok(serial) => outcome.serial_number = Some(serial),
                    Err(error) => outcome.error = Some(format!("{error:#}")),
                },
                Profiles::Initialize { request } => {
                    let initialization = ble::initialization::execute(
                        target.product_id,
                        target.physical_product_id,
                        session,
                        request,
                        validate,
                    )?;
                    if initialization.completed {
                        outcome.observed_slots = Some(initialization.observed_slots(
                            target.clone(),
                            context.connection_owner.clone(),
                            context.generation,
                        )?);
                    } else {
                        outcome.error = initialization.error.clone();
                    }
                    outcome.initialization = Some(initialization);
                }
                Profiles::Cached { slots } => outcome.observed_slots = Some(slots.clone()),
            }
        }
        if outcome.error.is_none()
            && !context.is_some_and(|context| matches!(context.profiles, Profiles::ReadIdentity))
        {
            match ble::execute_with_slots(
                target.product_id,
                target.physical_product_id,
                session,
                change,
                outcome.observed_slots.as_ref(),
                validate,
            ) {
                Ok(report) => outcome.report = Some(report),
                Err(error) => outcome.error = Some(format!("{error:#}")),
            }
        }
        if let Err(error) = validate() {
            outcome.error = Some(format!("{error:#}"));
        }
        outcome.validate_context(target, context, change)?;
        Ok((edition, outcome))
    })();
    let cleanup = session.finish();
    match (result, cleanup) {
        (Ok((edition, mut outcome)), cleanup) => {
            if let Err(cleanup) = cleanup {
                let cleanup = format!("{cleanup:#}");
                outcome.cleanup_error = Some(cleanup.clone());
                outcome.error = Some(match outcome.error {
                    Some(error) => {
                        format!("{error}; Current DPI BLE cleanup also failed: {cleanup}")
                    }
                    None => format!("Current DPI BLE cleanup failed: {cleanup}"),
                });
            }
            outcome.validate_context(target, context, change)?;
            Ok(json!({
                "target":target,"source_class":"rzDevice30","transport":"windows_gatt",
                "connection_edition":edition,"report":outcome.report,"profile_scope":"stage_1_active_0",
                "context":context,"initialization":outcome.initialization,
                "serial_number":outcome.serial_number,
                "observed_slots":outcome.observed_slots,"operation_error":outcome.error,
                "cleanup_error":outcome.cleanup_error
            }))
        }
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(cleanup)) => {
            Err(error.context(format!("Current DPI BLE cleanup also failed: {cleanup:#}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, collections::VecDeque};

    struct Script {
        replies: VecDeque<([u8; 3], u8, Result<Vec<u8>, &'static str>)>,
        calls: Vec<String>,
        cleanup_failure: bool,
    }

    impl ble::Transport for Script {
        fn exchange(
            &mut self,
            command: [u8; 3],
            profile: u8,
            payload: &[u8],
            validate: &dyn Fn() -> anyhow::Result<()>,
        ) -> anyhow::Result<Acknowledgement> {
            validate()?;
            assert!(payload.is_empty());
            let (expected, expected_profile, data) = self.replies.pop_front().unwrap();
            assert_eq!((command, profile), (expected, expected_profile));
            self.calls.push(format!("{command:?}/{profile}"));
            let data = data.map_err(anyhow::Error::msg)?;
            Ok(Acknowledgement {
                command,
                profile: Some(profile),
                transaction: 1,
                response_bytes: data.len(),
                data,
            })
        }
    }

    impl SessionTransport for Script {
        fn finish(&mut self) -> anyhow::Result<()> {
            self.calls.push("finish".into());
            ensure!(!self.cleanup_failure, "CCCD removal failed");
            Ok(())
        }
    }

    fn target() -> WindowsBleTarget {
        serde_json::from_value(json!({
            "product_id":226,"physical_product_id":228,
            "device_container_id":"{01234567-89AB-CDEF-0123-456789ABCDEF}",
            "hid_path":"hid-ble-dpi","device_instance_id":"HID-INSTANCE",
            "parent_instance_id":"BTHLEDevice-INSTANCE","mac_address":177789161760246u64
        }))
        .unwrap()
    }

    fn script() -> Script {
        Script {
            replies: VecDeque::from([
                ([0, 1, 134], 0, Ok(vec![1, 2, 3])),
                ([0, 11, 132], 1, Ok(vec![1, 1, 1, 144, 1, 244, 1, 0, 0])),
                ([0, 11, 129], 0, Ok(vec![132, 3, 182, 3, 0, 0])),
            ]),
            calls: vec![],
            cleanup_failure: false,
        }
    }

    fn initialization_context() -> Context {
        Context {
            connection_owner: "workspace-42".into(),
            generation: 7,
            profiles: Profiles::Initialize {
                request: ble::initialization::Request {
                    profiles: vec![ble::initialization::LocalProfile {
                        guid: "actual-local-guid".into(),
                        name: "Local".into(),
                        obm_slot_ids: vec![],
                        dpi_stages: None,
                    }],
                    active_profile_guid: "actual-local-guid".into(),
                    owner: "local-guest-owner".into(),
                },
            },
        }
    }

    #[test]
    fn current_dpi_ble_identity_read_has_no_settings_or_initialization_side_effects() {
        let context = Context {
            connection_owner: "workspace-42".into(),
            generation: 7,
            profiles: Profiles::ReadIdentity,
        };
        let mut session = Script {
            replies: VecDeque::from([
                ([0, 1, 134], 0, Ok(vec![1, 2, 3])),
                ([0, 1, 131], 0, Ok(b"ACTUAL-SERIAL\0".to_vec())),
            ]),
            calls: vec![],
            cleanup_failure: false,
        };
        let reply = run_session(&target(), None, Some(&context), &mut session, &|| Ok(())).unwrap();
        assert_eq!(reply["serial_number"], "ACTUAL-SERIAL");
        assert!(reply["report"].is_null());
        assert!(reply["initialization"].is_null());
        assert_eq!(session.calls, ["[0, 1, 134]/0", "[0, 1, 131]/0", "finish"]);
        assert!(session.replies.is_empty());
    }

    #[test]
    fn current_dpi_ble_initialization_precedes_reading_and_returns_the_projected_cache() {
        let context = initialization_context();
        let mut session = script();
        let initialization = [
            ([0, 3, 137], 0, Ok(vec![1])),
            ([0, 3, 128], 0, Ok(vec![1])),
            ([0, 3, 128], 0, Ok(vec![1])),
            ([0, 11, 129], 1, Ok(vec![132, 3, 182, 3, 0, 0])),
            ([0, 11, 132], 1, Ok(vec![1, 1, 1, 144, 1, 244, 1, 0, 0])),
        ];
        for (index, step) in initialization.into_iter().enumerate() {
            session.replies.insert(index + 1, step);
        }
        let reply = run_session(&target(), None, Some(&context), &mut session, &|| Ok(())).unwrap();
        assert_eq!(reply["initialization"]["completed"], true);
        assert_eq!(reply["observed_slots"]["connection_owner"], "workspace-42");
        assert_eq!(reply["observed_slots"]["generation"], 7);
        assert_eq!(reply["observed_slots"]["profiles"], json!([]));
        assert_eq!(reply["report"]["active"], json!({"x":900,"y":950}));
        assert_eq!(
            session.calls,
            [
                "[0, 1, 134]/0",
                "[0, 3, 137]/0",
                "[0, 3, 128]/0",
                "[0, 3, 128]/0",
                "[0, 11, 129]/1",
                "[0, 11, 132]/1",
                "[0, 11, 132]/1",
                "[0, 11, 129]/0",
                "finish",
            ]
        );
        assert!(session.replies.is_empty());
    }

    #[test]
    fn current_dpi_ble_failed_initialization_keeps_its_progress_without_editing() {
        let context = initialization_context();
        let mut session = script();
        session
            .replies
            .insert(1, ([0, 3, 137], 0, Err("capacity rejected")));
        let reply = run_session(&target(), None, Some(&context), &mut session, &|| Ok(())).unwrap();
        assert_eq!(reply["initialization"]["completed"], false);
        assert!(
            reply["operation_error"]
                .as_str()
                .unwrap()
                .contains("capacity rejected")
        );
        assert!(reply["report"].is_null());
        assert!(reply["observed_slots"].is_null());
        assert_eq!(session.calls, ["[0, 1, 134]/0", "[0, 3, 137]/0", "finish"]);
        assert_eq!(session.replies.len(), 2);
    }

    #[test]
    fn current_dpi_ble_cached_route_rejects_stale_generation_before_any_transfer() {
        let context = Context {
            connection_owner: "workspace-42".into(),
            generation: 8,
            profiles: Profiles::Cached {
                slots: ble::initialization::ObservedSlots {
                    target: target(),
                    connection_owner: "workspace-42".into(),
                    generation: 7,
                    profiles: vec![],
                },
            },
        };
        let mut session = script();
        assert!(run_session(&target(), None, Some(&context), &mut session, &|| Ok(())).is_err());
        assert_eq!(session.calls, ["finish"]);
        assert_eq!(session.replies.len(), 3);
    }

    #[test]
    fn current_dpi_ble_session_reads_edition_before_settings_and_finishes_before_success() {
        let mut session = script();
        let reply = run_session(&target(), None, None, &mut session, &|| Ok(())).unwrap();
        let report: dpi::DpiReport = serde_json::from_value(reply["report"].clone()).unwrap();
        assert!(report.verified);
        assert_eq!(report.active, Some(dpi::DpiPair { x: 900, y: 950 }));
        assert_eq!(report.stages.unwrap().records[0].x, 400);
        assert_eq!(
            session.calls,
            [
                "[0, 1, 134]/0",
                "[0, 11, 132]/1",
                "[0, 11, 129]/0",
                "finish"
            ]
        );
        assert!(session.replies.is_empty());
    }

    #[test]
    fn current_dpi_ble_session_stops_failed_edition_and_keeps_cleanup_failure() {
        for failed_cleanup in [false, true] {
            let mut session = script();
            session.replies.front_mut().unwrap().2 = Err("edition rejected");
            session.cleanup_failure = failed_cleanup;
            let failure = run_session(&target(), None, None, &mut session, &|| Ok(())).unwrap_err();
            let failure = format!("{failure:#}");
            assert!(failure.contains("edition rejected"));
            assert_eq!(failure.contains("CCCD removal failed"), failed_cleanup);
            assert_eq!(session.calls, ["[0, 1, 134]/0", "finish"]);
            assert_eq!(session.replies.len(), 2);
        }
        let mut session = script();
        session.cleanup_failure = true;
        let reply = run_session(&target(), None, None, &mut session, &|| Ok(())).unwrap();
        assert!(
            reply["operation_error"]
                .as_str()
                .unwrap()
                .contains("CCCD removal failed")
        );
        assert_eq!(reply["report"]["active"], json!({"x":900,"y":950}));
        assert_eq!(reply["cleanup_error"], "CCCD removal failed");
        assert_eq!(session.calls.last().unwrap(), "finish");
    }

    #[test]
    fn current_dpi_ble_session_preserves_partial_getter_failure_and_still_finishes() {
        let mut session = script();
        session.replies[1].2 = Err("stages rejected");
        let reply = run_session(&target(), None, None, &mut session, &|| Ok(())).unwrap();
        let report: dpi::DpiReport = serde_json::from_value(reply["report"].clone()).unwrap();
        assert!(!report.verified);
        assert!(report.stages.is_none());
        assert!(
            report
                .read_errors
                .stages()
                .unwrap()
                .contains("stages rejected")
        );
        assert_eq!(report.active, Some(dpi::DpiPair { x: 900, y: 950 }));
        assert_eq!(session.calls.last().unwrap(), "finish");
    }

    #[test]
    fn current_dpi_ble_session_scope_cancellation_finishes_without_settings_requests() {
        let mut session = script();
        let checks = Cell::new(0);
        let reply = run_session(&target(), None, None, &mut session, &|| {
            checks.set(checks.get() + 1);
            ensure!(checks.get() <= 2, "scope replaced");
            Ok(())
        })
        .unwrap();
        assert!(
            reply["operation_error"]
                .as_str()
                .unwrap()
                .contains("scope replaced")
        );
        assert!(reply["report"].is_null());
        assert_eq!(session.calls, ["[0, 1, 134]/0", "finish"]);
        assert_eq!(session.replies.len(), 2);
    }
}
