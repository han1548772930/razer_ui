//! PID653 current Protocol25 hardware module query and event interpretation.
//! The initial getter and subsequent notifications use different predicates.
use crate::{
    backend::{FeatureTransport, HidNode},
    device_reads::{DeviceReadCapability, DeviceReadTarget},
    receiver_pairing::HardwareEvents,
};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::OnceLock, time::Duration};

pub const PRODUCT_ID: u32 = 653;
pub const GET_HW_MODULE: [u8; 3] = [1, 0, 185];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
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

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    InitialQuery,
    HardwareNotification,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Observation {
    pub origin: Origin,
    pub status: u8,
}

impl Observation {
    pub fn initial(status: u8) -> Self {
        Self {
            origin: Origin::InitialQuery,
            status,
        }
    }

    pub fn notification(status: u8) -> Self {
        Self {
            origin: Origin::HardwareNotification,
            status,
        }
    }

    pub fn connected(self) -> bool {
        match self.origin {
            Origin::InitialQuery => self.status == 1,
            Origin::HardwareNotification => self.status != 0,
        }
    }
}

/// Diagnostics never supply a source UI boolean when an actual read failed.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Update {
    Observed { observation: Observation },
    Diagnostic { message: String },
}

pub fn capability(product_id: u32) -> Option<&'static DeviceReadCapability> {
    if product_id != PRODUCT_ID {
        return None;
    }
    static CAP: OnceLock<DeviceReadCapability> = OnceLock::new();
    Some(CAP.get_or_init(|| DeviceReadCapability {
        product_id: PRODUCT_ID,
        source_class: "Protocol25".into(),
        direct_pids: vec![PRODUCT_ID],
        vendor_id: 0x1532,
        claim_interface: 3,
        claim_interfaces_by_pid: BTreeMap::new(),
        physical_transports_by_pid: Default::default(),
        report_id: 0,
        report_bytes: 91,
        transaction_prefix: 0,
        transaction_modulus: 31,
        queries: vec![],
        polling_codes: BTreeMap::new(),
        polling_value_offset: None,
        polling_default_hz: None,
        polling_physical_product_id: None,
        charging_codes: BTreeMap::new(),
        max_retry_in: 10,
        max_retry_out: 20,
        sleep_between_out_ms: 5,
        sleep_between_out_in_ms: 5,
        sleep_between_in_ms: 5,
    }))
}

pub fn query(
    device: &dyn FeatureTransport,
    transaction: u8,
    validate: &impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Observation> {
    let data = crate::feature_exchange::exchange(
        device,
        capability(PRODUCT_ID).unwrap(),
        GET_HW_MODULE,
        &[],
        transaction,
        validate,
    )?;
    Ok(Observation::initial(
        *data
            .first()
            .context("Hardware module response has no status byte")?,
    ))
}

/// Current rzHardwareEventsProtocol25Parser consumes the native callback's
/// actual bytes. Byte zero is the record, not a synthetic HID report ID.
pub fn parse_notification(data: &[u8]) -> anyhow::Result<Option<Observation>> {
    if data.first() != Some(&5) {
        return Ok(None);
    }
    let event = *data
        .get(1)
        .context("Protocol25 notification has no event ID")?;
    if event != 14 {
        return Ok(None);
    }
    Ok(Some(Observation::notification(*data.get(2).context(
        "Hardware module notification has no status byte",
    )?)))
}

/// The OS adapter selects and owns the source's actual input collections.
/// `finish` runs after cancellation, query failure and interrupt failure.
pub fn monitor(
    events: &mut dyn HardwareEvents,
    query_initial: impl FnOnce() -> anyhow::Result<Observation>,
    validate: &impl Fn() -> anyhow::Result<()>,
    publish: &mut impl FnMut(Update) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    let result: anyhow::Result<()> = (|| {
        validate()?;
        let registration = events.begin();
        validate()?;
        match query_initial() {
            Ok(observation) => {
                ensure!(
                    observation.origin == Origin::InitialQuery,
                    "Wrong initial observation origin"
                );
                validate()?;
                publish(Update::Observed { observation })?;
            }
            Err(error) => {
                if crate::backend::exchange_interrupted(&error) {
                    return Err(error);
                }
                validate()?;
                publish(Update::Diagnostic {
                    message: format!("Hardware module initial query failed: {error:#}"),
                })?;
            }
        }
        registration?;
        loop {
            validate()?;
            let data = events.receive(Duration::from_millis(50))?;
            validate()?;
            if let Some(data) = data {
                if let Some(observation) = parse_notification(&data)? {
                    publish(Update::Observed { observation })?;
                }
            }
        }
    })();
    let cleanup = events.finish();
    match (result, cleanup) {
        (Ok(()), Err(error)) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Err(error), Err(cleanup)) => {
            Err(error.context(format!("Hardware event cleanup also failed: {cleanup:#}")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keyboard_control_test_transport::{Mock, assert_report_integrity, reply};
    use std::{cell::Cell, collections::VecDeque};

    #[test]
    fn initial_and_notification_status_have_distinct_source_predicates() {
        let mock = Mock::new([reply(&[1]), reply(&[2])]);
        assert!(query(&mock, 0, &|| Ok(())).unwrap().connected());
        assert!(!query(&mock, 1, &|| Ok(())).unwrap().connected());
        assert!(
            parse_notification(&[5, 14, 2])
                .unwrap()
                .unwrap()
                .connected()
        );
        assert!(
            !parse_notification(&[5, 14, 0])
                .unwrap()
                .unwrap()
                .connected()
        );
        assert!(parse_notification(&[0, 5, 14, 1]).unwrap().is_none());
        assert!(parse_notification(&[5, 14]).is_err());
        let sent = mock.sent();
        assert_report_integrity(&sent);
        assert_eq!(&sent[0][6..9], &GET_HW_MODULE);
        assert_eq!((sent[0][2], sent[1][2]), (0, 1));
    }

    struct Events {
        reports: VecDeque<Vec<u8>>,
        finished: bool,
    }
    impl HardwareEvents for Events {
        fn begin(&mut self) -> anyhow::Result<()> {
            Ok(())
        }
        fn receive(&mut self, _: Duration) -> anyhow::Result<Option<Vec<u8>>> {
            Ok(self.reports.pop_front())
        }
        fn finish(&mut self) -> anyhow::Result<()> {
            self.finished = true;
            Ok(())
        }
    }

    #[test]
    fn failed_initial_read_remains_diagnostic_and_notifications_continue() {
        let mut events = Events {
            reports: [vec![5, 14, 2]].into(),
            finished: false,
        };
        let canceled = Cell::new(false);
        let mut updates = Vec::new();
        let result = monitor(
            &mut events,
            || anyhow::bail!("scripted read failure"),
            &|| {
                ensure!(!canceled.get(), "canceled");
                Ok(())
            },
            &mut |update| {
                if matches!(update, Update::Observed { .. }) {
                    canceled.set(true);
                }
                updates.push(update);
                Ok(())
            },
        );
        assert!(result.is_err());
        assert!(events.finished);
        assert!(matches!(&updates[0], Update::Diagnostic { .. }));
        assert_eq!(updates.len(), 2);
        assert!(matches!(updates[1], Update::Observed { observation } if observation.connected()));
    }

    #[test]
    fn cancellation_before_open_produces_no_source_observation() {
        let mut events = Events {
            reports: VecDeque::new(),
            finished: false,
        };
        let mut updates = Vec::new();
        assert!(
            monitor(
                &mut events,
                || unreachable!(),
                &|| anyhow::bail!("canceled"),
                &mut |update| {
                    updates.push(update);
                    Ok(())
                }
            )
            .is_err()
        );
        assert!(updates.is_empty());
        assert!(events.finished);
        assert!(query(&Mock::new([reply(&[])]), 0, &|| Ok(())).is_err());
    }
}
