//! Current keyboard Protocol30 brightness, including the separate OLED switch.
//! A setter's echoed request is an acknowledgement, never a brightness reading.
use super::{BrightnessIntent, BrightnessSubmission, KeyboardBrightness, encode_percent};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};

pub const EDITION: [u8; 3] = [0, 1, 134];
const REGIONS: [u8; 3] = [0, 16, 128];
const READ: [u8; 3] = [0, 16, 133];
const WRITE: [u8; 3] = [1, 16, 5];
const OLED_SWITCH: [u8; 3] = [2, 23, 17];
const ACTIVE_PROFILE: u8 = 1;
const ALL_REGIONS: u8 = 0;
const OLED_PRODUCT: u32 = 691;

fn configured_region(product_id: u32) -> u8 {
    // The current585/631 rzDeviceType registrations select BacklightLED;
    // the other actual BLE keyboard registrations leave AllRegion selected.
    if matches!(product_id, 585 | 631) {
        5
    } else {
        ALL_REGIONS
    }
}

pub fn available(product_id: u32, physical_product_id: u32) -> bool {
    crate::keyboard_metadata::ble_product_id(product_id) == Some(physical_product_id)
}

pub fn frame(
    transaction: u8,
    command: [u8; 3],
    profile: u8,
    selector: u8,
    data: &[u8],
) -> anyhow::Result<Vec<u8>> {
    ensure!(transaction < 31, "Keyboard BLE transaction is invalid");
    ensure!(
        usize::from(command[0]) == data.len(),
        "Keyboard BLE payload length differs"
    );
    let mut frame = vec![
        transaction,
        command[0],
        0,
        0,
        command[1],
        command[2],
        profile,
        selector,
    ];
    frame.extend_from_slice(data);
    Ok(frame)
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Acknowledgement {
    transaction: u8,
    command: [u8; 3],
    profile: u8,
    selector: u8,
    data: Vec<u8>,
}

impl Acknowledgement {
    /// The platform calls this only after its real GATT response has settled.
    pub fn new(
        transaction: u8,
        command: [u8; 3],
        profile: u8,
        selector: u8,
        data: Vec<u8>,
    ) -> anyhow::Result<Self> {
        ensure!(
            transaction < 31,
            "Keyboard BLE acknowledgement transaction is invalid"
        );
        Ok(Self {
            transaction,
            command,
            profile,
            selector,
            data,
        })
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn matches(&self, command: [u8; 3], profile: u8, selector: u8) -> bool {
        self.command == command
            && self.profile == profile
            && self.selector == selector
            && self.transaction < 31
    }
}

pub trait Transport {
    fn exchange(
        &mut self,
        command: [u8; 3],
        profile: u8,
        selector: u8,
        data: &[u8],
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Acknowledgement>;
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Report {
    product_id: u32,
    requested: Option<BrightnessSubmission>,
    acknowledgements: Vec<Acknowledgement>,
    reading: Option<KeyboardBrightness>,
    confirmed_enabled: Option<bool>,
    region_query_failure: Option<String>,
}

impl Report {
    pub fn reading(&self) -> Option<&KeyboardBrightness> {
        self.reading.as_ref()
    }
    pub fn confirmed_enabled(&self) -> Option<bool> {
        self.confirmed_enabled
    }

    pub fn check_request(
        &self,
        product_id: u32,
        requested: Option<BrightnessSubmission>,
    ) -> anyhow::Result<()> {
        ensure!(
            self.product_id == product_id && self.requested == requested,
            "Keyboard BLE result belongs to a different request"
        );
        if let Some(requested) = requested {
            requested.validate()?;
        }
        let reading = self
            .reading
            .as_ref()
            .context("Keyboard BLE brightness was not read")?;
        ensure!(
            reading.profile_id == ACTIVE_PROFILE
                && reading.percent == (u16::from(reading.raw_value) * 100).div_ceil(255) as u8,
            "Keyboard BLE brightness decoding differs"
        );
        let prefix = usize::from(requested.is_some_and(|_| product_id == OLED_PRODUCT))
            + usize::from(requested.is_some_and(|value| {
                product_id != OLED_PRODUCT || value.intent() == BrightnessIntent::Value
            }));
        let configured = configured_region(product_id);
        let regions_acknowledged = configured == ALL_REGIONS && self.region_query_failure.is_none();
        ensure!(
            self.acknowledgements.len() == prefix + usize::from(regions_acknowledged) + 1,
            "Keyboard BLE acknowledgement sequence is incomplete"
        );
        let mut offset = 0;
        if requested.is_some() && product_id == OLED_PRODUCT {
            let reply = &self.acknowledgements[offset];
            ensure!(
                reply.command == OLED_SWITCH
                    && reply.profile == ACTIVE_PROFILE
                    && reply.selector == ALL_REGIONS
                    && reply.transaction < 31,
                "Keyboard OLED switch acknowledgement is absent"
            );
            offset += 1;
        } else {
            ensure!(
                self.confirmed_enabled.is_none(),
                "This brightness operation does not observe an enabled switch"
            );
        }
        if requested.is_some_and(|value| {
            product_id != OLED_PRODUCT || value.intent() == BrightnessIntent::Value
        }) {
            let reply = &self.acknowledgements[offset];
            ensure!(
                reply.command == WRITE
                    && reply.profile == ACTIVE_PROFILE
                    && reply.selector == configured
                    && reply.transaction < 31,
                "Keyboard BLE setter acknowledgement is absent"
            );
            offset += 1;
        }
        if regions_acknowledged {
            let regions = &self.acknowledgements[offset];
            ensure!(
                regions.matches(REGIONS, 0, 1)
                    && regions.data.first().map(|value| value & 63) == Some(reading.region_id),
                "Keyboard BLE region does not match the actual list"
            );
        } else if configured != ALL_REGIONS {
            ensure!(
                self.region_query_failure.is_none(),
                "A fixed keyboard region did not query a region list"
            );
            ensure!(
                reading.region_id == configured,
                "Keyboard BLE registered region differs"
            );
        } else {
            ensure!(
                reading.region_id == 1
                    && self
                        .region_query_failure
                        .as_ref()
                        .is_some_and(|reason| !reason.is_empty()),
                "Keyboard source fallback has no failed query or real region-one reading"
            );
        }
        let read = self
            .acknowledgements
            .last()
            .context("Keyboard BLE read acknowledgement is absent")?;
        ensure!(
            read.command == READ
                && read.profile == ACTIVE_PROFILE
                && read.selector == reading.region_id
                && read.transaction < 31
                && read.data.first() == Some(&reading.raw_value),
            "Keyboard BLE reading has no matching response"
        );
        if let Some(requested) = requested {
            if product_id == OLED_PRODUCT {
                ensure!(
                    self.confirmed_enabled == Some(requested.enabled()),
                    "Keyboard OLED brightness switch was not acknowledged"
                );
            }
            if product_id != OLED_PRODUCT || requested.intent() == BrightnessIntent::Value {
                ensure!(
                    reading.percent == requested.effective_percent(),
                    "Keyboard BLE brightness readback differs from the requested value"
                );
            }
        } else {
            ensure!(
                self.confirmed_enabled.is_none(),
                "A brightness read does not observe its switch"
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Failure {
    report: Report,
}
impl Failure {
    pub fn new(report: Report) -> Self {
        Self { report }
    }
    pub fn report(&self) -> &Report {
        &self.report
    }
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Keyboard BLE brightness failed after {} acknowledgements; readback={}",
            self.report.acknowledgements.len(),
            self.report.reading.is_some()
        )
    }
}
impl std::error::Error for Failure {}

#[derive(Debug)]
struct StopRecovery;
impl std::fmt::Display for StopRecovery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Keyboard brightness owner or response contract changed")
    }
}
impl std::error::Error for StopRecovery {}

fn exchange(
    transport: &mut dyn Transport,
    report: &mut Report,
    command: [u8; 3],
    profile: u8,
    selector: u8,
    data: &[u8],
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Vec<u8>> {
    let guarded = || validate().context(StopRecovery);
    guarded()?;
    let reply = transport.exchange(command, profile, selector, data, &guarded)?;
    if !(reply.command == command
        && reply.profile == profile
        && reply.selector == selector
        && reply.transaction < 31)
    {
        return Err(
            anyhow::anyhow!("Keyboard BLE acknowledgement belongs to another command")
                .context(StopRecovery),
        );
    }
    let data = reply.data.clone();
    report.acknowledgements.push(reply);
    guarded()?;
    Ok(data)
}

pub fn execute(
    product_id: u32,
    physical_product_id: u32,
    transport: &mut dyn Transport,
    requested: Option<BrightnessSubmission>,
    validate: &dyn Fn() -> anyhow::Result<()>,
) -> anyhow::Result<Report> {
    ensure!(
        available(product_id, physical_product_id),
        "Keyboard BLE identity is not source-qualified"
    );
    if let Some(requested) = requested {
        requested.validate()?;
    }
    let mut report = Report {
        product_id,
        requested,
        acknowledgements: vec![],
        reading: None,
        confirmed_enabled: None,
        region_query_failure: None,
    };
    let configured = configured_region(product_id);
    let result = (|| {
        if let Some(requested) = requested {
            if product_id == OLED_PRODUCT {
                exchange(
                    transport,
                    &mut report,
                    OLED_SWITCH,
                    ACTIVE_PROFILE,
                    ALL_REGIONS,
                    &[ALL_REGIONS, u8::from(requested.enabled())],
                    validate,
                )?;
                report.confirmed_enabled = Some(requested.enabled());
            }
            if product_id != OLED_PRODUCT || requested.intent() == BrightnessIntent::Value {
                exchange(
                    transport,
                    &mut report,
                    WRITE,
                    ACTIVE_PROFILE,
                    configured,
                    &[encode_percent(requested.effective_percent())?],
                    validate,
                )?;
            }
        }
        // A rejected list query selects the source's region-one GET; the
        // returned brightness must still come from that independent device read.
        let region = if configured == ALL_REGIONS {
            match exchange(transport, &mut report, REGIONS, 0, 1, &[], validate) {
                Ok(regions) => {
                    regions
                        .first()
                        .context("Keyboard BLE region list is empty")?
                        & 63
                }
                Err(error) => {
                    if error.downcast_ref::<StopRecovery>().is_some()
                        || crate::backend::exchange_interrupted(&error)
                    {
                        return Err(error);
                    }
                    validate().context(StopRecovery)?;
                    report.region_query_failure = Some(format!("{error:#}"));
                    1
                }
            }
        } else {
            configured
        };
        let data = exchange(
            transport,
            &mut report,
            READ,
            ACTIVE_PROFILE,
            region,
            &[],
            validate,
        )?;
        let raw = *data
            .first()
            .context("Keyboard BLE brightness response is empty")?;
        report.reading = Some(KeyboardBrightness {
            profile_id: ACTIVE_PROFILE,
            region_id: region,
            raw_value: raw,
            percent: (u16::from(raw) * 100).div_ceil(255) as u8,
        });
        report.check_request(product_id, requested)?;
        validate()?;
        Ok(())
    })();
    result.map_err(|error: anyhow::Error| {
        error.context(Failure {
            report: report.clone(),
        })
    })?;
    Ok(report)
}

#[cfg(test)]
#[path = "keyboard_brightness_ble_tests.rs"]
mod tests;
