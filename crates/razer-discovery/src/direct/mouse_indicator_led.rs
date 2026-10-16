//! Current three-LED mouse receiver route. The receiver has its own physical
//! PID and is not a Linker peer or a wired mouse transaction.
use super::*;
use crate::discovery::ObservedTransport;
use razer_device::mouse_indicator_led::{
    self as indicator, Change, Configuration, EventBatch, Report, SerialObservation, Target,
};

pub type Route = DirectRoute;
pub fn resolve(client: &mut ServiceClient, observed: &ObservedDevice) -> anyhow::Result<Route> {
    resolve_with(observed, &mut |request| client.request(request))
}
fn resolve_with(
    observed: &ObservedDevice,
    request: &mut impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<Route> {
    let source = indicator::capability(observed.product_id())
        .context("Three-LED receiver source is absent")?;
    ensure!(
        source.accepts(observed.physical_product_id(), observed.peer_product_id())
            && matches!(
                observed.transport(),
                Some(ObservedTransport::Wired | ObservedTransport::Dongle)
            ),
        "Three-LED request has no current physical receiver route"
    );
    let cap = source.transport(observed.physical_product_id())?;
    if let Some(node) = observed.hid_node() {
        return resolve_collection(
            observed.product_id(),
            observed.physical_product_id(),
            node,
            request,
        );
    }
    #[cfg(windows)]
    {
        Ok(Route::Container(windows::resolve_with(
            observed, &cap, request,
        )?))
    }
    #[cfg(not(windows))]
    {
        anyhow::bail!("Three-LED ContainerId route needs its platform adapter")
    }
}
fn resolve_collection(
    product_id: u32,
    physical: u32,
    node: &HidNode,
    request: &mut impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<Route> {
    let cap = indicator::capability(product_id)
        .context("Three-LED source is absent")?
        .transport(physical)?;
    ensure!(
        node.vendor_id == cap.vendor_id
            && u32::from(node.product_id) == physical
            && node.interface_number == i32::from(cap.claim_interface),
        "Three-LED receiver collection interface differs"
    );
    let nodes: Vec<HidNode> =
        serde_json::from_value(request(ServiceRequest::HidNodes)?["nodes"].clone())?;
    ensure!(
        nodes.iter().filter(|candidate| *candidate == node).count() == 1,
        "Three-LED receiver collection changed or is ambiguous"
    );
    super::source_reports_match_with(node, cap.report_bytes, request)?;
    Ok(Route::Collection {
        node: node.clone(),
        product_id,
    })
}
pub fn execute(
    client: &mut ServiceClient,
    route: &Route,
    change: Change,
) -> anyhow::Result<Report> {
    execute_with(route, change, &mut |request| client.request(request))
}
pub fn read_serial_number(
    client: &mut ServiceClient,
    route: &Route,
) -> anyhow::Result<SerialObservation> {
    SerialObservation::from_report(&execute(client, route, Change::ReadSerialNumber)?)
}
#[derive(Clone, Debug)]
pub struct BatteryObservation {
    pub level: Option<u8>,
    /// Source helper starts at -1. Setup publishes that failure state without
    /// the event caller's isSuccess gate.
    pub source_level: i16,
    pub charging_status: Option<String>,
    pub is_success: bool,
    /// These additional Rust diagnostics never replace source reducer fields.
    pub queries: Vec<Report>,
    pub error: Option<String>,
}
/// nr() owns this counter for the middleware session. Initial setup passes
/// false; ordinary event and timer calls increment it. A complete successful
/// query resets it. A thrown query never receives the fifth-return fallback.
#[derive(Clone, Debug, Default)]
pub struct BatterySession {
    source_counter: u64,
}
impl BatterySession {
    pub fn source_counter(&self) -> u64 {
        self.source_counter
    }
    fn begin(&mut self, count_attempt: bool) {
        if count_attempt {
            self.source_counter = self.source_counter.saturating_add(1);
        }
    }
    fn returned_failure(&self, mut observation: BatteryObservation) -> BatteryObservation {
        observation.is_success = self.source_counter == 5;
        observation
    }
}

/// Preserve the source method's return/throw boundary separately from the
/// query's protocol data. The verified current Electron Protocol25 adapter
/// returns SUCCESS or throws RzProtocolError; other returned statuses must not
/// be inferred from a transport exception or a failed acknowledgement.
enum BatterySourceReply {
    Returned {
        success: bool,
        report: Report,
    },
    Thrown {
        report: Option<Report>,
        message: String,
    },
}

/// Complete current nr() path for these three Protocol25 factories. Charge
/// getter failure on wireless performs an extra battery query to confirm power;
/// wired falls back to Charging. Query errors retain off/unsuccessful source
/// semantics; the caller only publishes successful results to its reducer.
pub fn read_battery_state(
    client: &mut ServiceClient,
    route: &Route,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<BatteryObservation> {
    read_battery_state_in_session(
        client,
        route,
        &mut BatterySession::default(),
        true,
        validate,
    )
}
pub fn read_battery_state_in_session(
    client: &mut ServiceClient,
    route: &Route,
    session: &mut BatterySession,
    count_attempt: bool,
    validate: impl Fn() -> anyhow::Result<()>,
) -> anyhow::Result<BatteryObservation> {
    read_battery_session_with(route, session, count_attempt, &validate, &mut |request| {
        client.request(request)
    })
}
#[cfg(test)]
fn read_battery_with(
    route: &Route,
    validate: &impl Fn() -> anyhow::Result<()>,
    request: &mut impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<BatteryObservation> {
    read_battery_session_with(
        route,
        &mut BatterySession::default(),
        true,
        validate,
        request,
    )
}
fn read_battery_session_with(
    route: &Route,
    session: &mut BatterySession,
    count_attempt: bool,
    validate: &impl Fn() -> anyhow::Result<()>,
    request: &mut impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<BatteryObservation> {
    let (product, physical) = match route {
        Route::Collection { node, product_id } => (*product_id, u32::from(node.product_id)),
        #[cfg(windows)]
        Route::Container(target) => (target.product_id, target.physical_product_id),
    };
    let source = indicator::capability(product).context("Receiver battery source is absent")?;
    ensure!(
        source.accepts(physical, None),
        "Receiver battery physical factory differs"
    );
    let wireless = physical == source.dongle_id();
    read_battery_source_with(session, count_attempt, wireless, validate, &mut |change| {
        Ok(match execute_with(route, change, request) {
            Ok(report) => match report.error.clone() {
                Some(message) => BatterySourceReply::Thrown {
                    report: Some(report),
                    message,
                },
                None => BatterySourceReply::Returned {
                    success: true,
                    report,
                },
            },
            Err(error) => BatterySourceReply::Thrown {
                report: None,
                message: format!("{error:#}"),
            },
        })
    })
}
fn read_battery_source_with(
    session: &mut BatterySession,
    count_attempt: bool,
    wireless: bool,
    validate: &impl Fn() -> anyhow::Result<()>,
    query: &mut impl FnMut(Change) -> anyhow::Result<BatterySourceReply>,
) -> anyhow::Result<BatteryObservation> {
    validate()?;
    session.begin(count_attempt);
    let mut observation = BatteryObservation {
        level: None,
        source_level: -1,
        charging_status: Some("NoCharge_BatteryFull".into()),
        is_success: false,
        queries: vec![],
        error: None,
    };
    let mut run =
        |change, observation: &mut BatteryObservation| -> anyhow::Result<BatterySourceReply> {
            validate()?;
            let result = query(change)?;
            validate()?;
            match &result {
                BatterySourceReply::Returned { report, .. } => {
                    observation.queries.push(report.clone());
                }
                BatterySourceReply::Thrown { report, message } => {
                    observation.error = Some(message.clone());
                    observation.queries.extend(report.iter().cloned());
                }
            }
            Ok(result)
        };
    let status = match run(Change::ReadChargingStatus, &mut observation)? {
        BatterySourceReply::Returned {
            success: true,
            report,
        } => report.charging_status,
        BatterySourceReply::Returned { success: false, .. } => {
            return Ok(session.returned_failure(observation));
        }
        BatterySourceReply::Thrown { .. } if wireless => {
            if !matches!(
                run(Change::ReadBatteryLevel, &mut observation)?,
                BatterySourceReply::Returned { success: true, .. }
            ) {
                observation.charging_status = Some("off".into());
                return Ok(observation);
            }
            Some("NoCharge_BatteryFull".into())
        }
        BatterySourceReply::Thrown { .. } => Some("Charging".into()),
    };
    // The successful charging value is assigned before the final level query;
    // an ordinary non-SUCCESS return preserves it, unlike an exception's off.
    observation.charging_status = status;
    match run(Change::ReadBatteryLevel, &mut observation)? {
        BatterySourceReply::Returned {
            success: true,
            report,
        } => {
            session.source_counter = 0;
            observation.level = report.battery_level;
            observation.source_level = report.battery_level.map(i16::from).unwrap_or(-1);
            observation.is_success = true;
        }
        BatterySourceReply::Returned { success: false, .. } => {
            return Ok(session.returned_failure(observation));
        }
        BatterySourceReply::Thrown { .. } => observation.charging_status = Some("off".into()),
    }
    Ok(observation)
}

/// Source init posts the requested configuration immediately after its SET ACK,
/// starts local persistence without awaiting it, then performs the quality GET.
/// The callback owns that UI post and persistence task. A later query failure
/// retains the real setter ACK in the combined diagnostic report.
pub fn initialize(
    client: &mut ServiceClient,
    route: &Route,
    configuration: Configuration,
    after_set: impl FnOnce(&Report) -> anyhow::Result<()>,
) -> anyhow::Result<Report> {
    initialize_with(route, configuration, after_set, &mut |request| {
        client.request(request)
    })
}
pub fn initialize_raw(
    client: &mut ServiceClient,
    route: &Route,
    bytes: Vec<u8>,
    after_set: impl FnOnce(&Report) -> anyhow::Result<()>,
) -> anyhow::Result<Report> {
    initialize_change_with(
        route,
        Change::SetRaw(bytes.clone()),
        Change::InitializeRaw(bytes),
        after_set,
        &mut |request| client.request(request),
    )
}
fn initialize_with(
    route: &Route,
    configuration: Configuration,
    after_set: impl FnOnce(&Report) -> anyhow::Result<()>,
    request: &mut impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<Report> {
    initialize_change_with(
        route,
        Change::Set(configuration),
        Change::Initialize(configuration),
        after_set,
        request,
    )
}
fn initialize_change_with(
    route: &Route,
    setter: Change,
    initialization: Change,
    after_set: impl FnOnce(&Report) -> anyhow::Result<()>,
    request: &mut impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<Report> {
    let mut report = execute_with(route, setter, request)?;
    report.requested = initialization.clone();
    if report.error.is_some() || !report.setter_acknowledged {
        return Ok(report);
    }
    if let Err(error) = after_set(&report) {
        report.error = Some(format!(
            "Receiver initialization callback failed: {error:#}"
        ));
        return Ok(report);
    }
    match execute_with(route, Change::ReadConnectionQuality, request) {
        Ok(quality) => {
            report.acknowledgements.extend(quality.acknowledgements);
            report.quality = quality.quality;
            report.error = quality.error;
        }
        Err(error) => {
            report.error = Some(format!(
                "Receiver initialization quality query failed: {error:#}"
            ))
        }
    }
    validate_report(&initialization, &report)?;
    Ok(report)
}
fn execute_with(
    route: &Route,
    change: Change,
    request: &mut impl FnMut(ServiceRequest) -> anyhow::Result<Value>,
) -> anyhow::Result<Report> {
    let query = match route {
        Route::Collection { node, product_id } => ServiceRequest::HidNodeMouseIndicatorLed {
            node: node.clone(),
            product_id: *product_id,
            change: change.clone(),
        },
        #[cfg(windows)]
        Route::Container(target) => ServiceRequest::DeviceMouseIndicatorLed {
            target: target.clone(),
            change: change.clone(),
        },
    };
    let reply = request(query)?;
    route.check_reply(&reply)?;
    ensure!(
        reply["setting_scope"] == "active_profile_receiver_indicator"
            && reply["source_class"] == "rzDevice25",
        "Three-LED response source/setting scope differs"
    );
    let report: Report = serde_json::from_value(reply["report"].clone())?;
    validate_report(&change, &report)?;
    Ok(report)
}
fn validate_report(change: &Change, report: &Report) -> anyhow::Result<()> {
    ensure!(
        report.requested == *change
            && report.source_quality_update.is_none()
            && (!report.setter_sent || report.setter_attempted)
            && (!report.setter_acknowledged || report.setter_sent),
        "Three-LED response does not belong to its source request"
    );
    let expected: &[[u8; 3]] = match change {
        Change::Initialize(_) | Change::InitializeRaw(_) => &[
            indicator::SET_CONFIGURATION,
            indicator::GET_CONNECTION_QUALITY,
        ],
        Change::Set(_) | Change::SetRaw(_) => &[indicator::SET_CONFIGURATION],
        Change::ReadConfiguration => &[indicator::GET_CONFIGURATION],
        Change::ReadConnectionQuality => &[indicator::GET_CONNECTION_QUALITY],
        Change::ReadSerialNumber => &[indicator::GET_SERIAL_NUMBER],
        Change::ReadBatteryLevel => &[indicator::GET_BATTERY_LEVEL],
        Change::ReadChargingStatus => &[indicator::GET_CHARGING_STATUS],
    };
    ensure!(
        report.acknowledgements.len() <= expected.len()
            && report
                .acknowledgements
                .iter()
                .zip(expected)
                .all(|(ack, expected)| ack.command == *expected && ack.transaction < 31)
            && report.setter_acknowledged
                == (change.writes() && !report.acknowledgements.is_empty())
            && (report.error.is_some() || report.acknowledgements.len() == expected.len())
            && (change.writes() || !report.setter_attempted && !report.setter_sent)
            && (matches!(change, Change::ReadConfiguration) || report.configuration.is_none())
            && (matches!(change, Change::ReadConfiguration) || report.raw_configuration.is_none())
            && (matches!(change, Change::ReadSerialNumber) || report.serial_number.is_none())
            && (matches!(change, Change::ReadBatteryLevel) || report.battery_level.is_none())
            && (matches!(change, Change::ReadChargingStatus) || report.charging_status.is_none())
            && (matches!(
                change,
                Change::Initialize(_) | Change::InitializeRaw(_) | Change::ReadConnectionQuality
            ) || report.quality.is_none()),
        "Three-LED service does not confirm the exact ordered commands"
    );
    if matches!(change, Change::ReadSerialNumber) {
        let ascii = report.acknowledgements.first().map(|ack| {
            ack.data
                .iter()
                .copied()
                .filter(|byte| (32..127).contains(byte))
                .map(char::from)
                .collect::<String>()
        });
        ensure!(
            report.serial_number == ascii,
            "Receiver protocol serial differs from its acknowledged bytes"
        );
    }
    if matches!(change, Change::ReadConfiguration) {
        ensure!(
            report.raw_configuration == report.acknowledgements.first().map(|ack| ack.data.clone()),
            "Receiver configuration differs from its actual getter bytes"
        );
    }
    if matches!(change, Change::ReadBatteryLevel) {
        let value = report
            .acknowledgements
            .first()
            .and_then(|ack| ack.data.get(1))
            .map(|value| (u16::from(*value) * 100 / 255) as u8);
        ensure!(
            report.battery_level == value,
            "Receiver battery level differs from its actual getter bytes"
        );
    }
    if matches!(change, Change::ReadChargingStatus) {
        let value = report
            .acknowledgements
            .first()
            .and_then(|ack| ack.data.get(1))
            .and_then(|value| indicator::charging_status(*value))
            .map(str::to_owned);
        ensure!(
            report.charging_status == value,
            "Receiver charging state differs from its actual getter bytes"
        );
    }
    Ok(())
}
fn target(route: &Route) -> Target {
    match route {
        Route::Collection { node, product_id } => Target::Collection {
            node: node.clone(),
            product_id: *product_id,
        },
        #[cfg(windows)]
        Route::Container(target) => Target::WindowsContainer(target.clone()),
    }
}
pub fn start(
    client: &mut ServiceClient,
    route: &Route,
    operation_id: String,
) -> anyhow::Result<()> {
    let target = target(route);
    let reply = client.request(ServiceRequest::MouseIndicatorLedStart {
        operation_id: operation_id.clone(),
        target: target.clone(),
    })?;
    ensure!(
        reply["operation_id"] == operation_id
            && reply["target"] == serde_json::to_value(&target)?
            && reply["started"] == true,
        "Receiver quality owner start differs"
    );
    Ok(())
}
pub fn poll(client: &mut ServiceClient, operation_id: &str) -> anyhow::Result<EventBatch> {
    let reply = client.request(ServiceRequest::MouseIndicatorLedPoll {
        operation_id: operation_id.into(),
    })?;
    let batch: EventBatch = serde_json::from_value(reply)?;
    ensure!(
        batch.operation_id == operation_id,
        "Receiver quality owner poll differs"
    );
    Ok(batch)
}
pub fn stop(client: &mut ServiceClient, operation_id: &str) -> anyhow::Result<()> {
    let reply = client.request(ServiceRequest::MouseIndicatorLedStop {
        operation_id: operation_id.into(),
    })?;
    ensure!(
        reply["operation_id"] == operation_id && reply["stopped"] == true,
        "Receiver quality owner stop differs"
    );
    if let Some(error) = reply["cleanup_error"].as_str() {
        anyhow::bail!("Receiver quality cleanup failed: {error}");
    }
    Ok(())
}

#[cfg(test)]
#[path = "mouse_indicator_led_tests.rs"]
mod tests;
