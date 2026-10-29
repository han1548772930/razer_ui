//! Current PID226 BLE228 DPI tasks use the retained Protocol30 GATT owner.
use super::mouse_runtime_artwork_adapter::ble::{self as gatt, Apartment, Connection};
use super::receiver;
use anyhow::ensure;
use razer_device::{
    mouse_current_dpi::{self as dpi, Acknowledgement, DpiChange, ble},
    mouse_runtime_artwork::WindowsBleTarget,
};
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
        validate()?;
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
) -> anyhow::Result<Value> {
    ensure!(
        ble::accepts(target.product_id, target.physical_product_id),
        "Current DPI BLE source factory differs"
    );
    if let Some(change) = change {
        dpi::validate_change_for(target.product_id, change)?;
    }
    let _apartment = Apartment::new()?;
    let started = Instant::now();
    let validate = || {
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
    run_session(target, change, &mut session, &validate)
}

fn run_session(
    target: &WindowsBleTarget,
    change: Option<&DpiChange>,
    session: &mut impl SessionTransport,
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Value> {
    let result = (|| {
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
        let report = ble::execute(
            target.product_id,
            target.physical_product_id,
            session,
            change,
            validate,
        )?;
        validate()?;
        Ok(json!({
            "target":target,"source_class":"rzDevice30","transport":"windows_gatt",
            "connection_edition":edition,"report":report,"profile_scope":"stage_1_active_0"
        }))
    })();
    let cleanup = session.finish();
    match (result, cleanup) {
        (Ok(reply), Ok(())) => Ok(reply),
        (Ok(_), Err(cleanup)) => Err(cleanup),
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

    #[test]
    fn current_dpi_ble_session_reads_edition_before_settings_and_finishes_before_success() {
        let mut session = script();
        let reply = run_session(&target(), None, &mut session, &|| Ok(())).unwrap();
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
            let failure = run_session(&target(), None, &mut session, &|| Ok(())).unwrap_err();
            let failure = format!("{failure:#}");
            assert!(failure.contains("edition rejected"));
            assert_eq!(failure.contains("CCCD removal failed"), failed_cleanup);
            assert_eq!(session.calls, ["[0, 1, 134]/0", "finish"]);
            assert_eq!(session.replies.len(), 2);
        }
        let mut session = script();
        session.cleanup_failure = true;
        assert!(
            format!(
                "{:#}",
                run_session(&target(), None, &mut session, &|| Ok(())).unwrap_err()
            )
            .contains("CCCD removal failed")
        );
        assert_eq!(session.calls.last().unwrap(), "finish");
    }

    #[test]
    fn current_dpi_ble_session_preserves_partial_getter_failure_and_still_finishes() {
        let mut session = script();
        session.replies[1].2 = Err("stages rejected");
        let reply = run_session(&target(), None, &mut session, &|| Ok(())).unwrap();
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
        let failure = run_session(&target(), None, &mut session, &|| {
            checks.set(checks.get() + 1);
            ensure!(checks.get() <= 2, "scope replaced");
            Ok(())
        })
        .unwrap_err();
        assert!(format!("{failure:#}").contains("scope replaced"));
        assert_eq!(session.calls, ["[0, 1, 134]/0", "finish"]);
        assert_eq!(session.replies.len(), 2);
    }
}
