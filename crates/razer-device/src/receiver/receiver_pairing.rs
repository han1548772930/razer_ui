//! Current 164/241 and 756 DualLink Scan/Pair/Unpair command lifecycle.
//! Command acknowledgements never establish scan completion or pairing success.
//! A caller must provide a real, retained hardware event subscription before any
//! asynchronous command. Collection selection and profile/runtime publication
//! remain explicit caller obligations; this module never substitutes a query.
use super::{backend::FeatureTransport, receiver_capabilities::ReceiverCapability};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
#[cfg(test)]
use std::collections::BTreeSet;
use std::{
    sync::OnceLock,
    time::{Duration, Instant},
};

#[path = "receiver_binding.rs"]
mod binding;
pub use binding::{project_binding_rows_with_runtime, source192_binding_info};

const PAIR: [u8; 3] = [3, 0, 65];
const UNPAIR: [u8; 3] = [2, 0, 66];
const SCAN: [u8; 3] = [1, 0, 70];
const EDITION: [u8; 3] = [3, 0, 134];
const HARDWARE_MODULE: [u8; 3] = [1, 0, 185];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum PairingAction {
    Scan,
    /// Current DUALLINK_CANCEL sends deviceScan(4), including while idle.
    Cancel,
    Pair {
        dongle_id: u16,
    },
    Unpair {
        dongle_id: u16,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PairingCapability {
    pub product_id: u16,
    pub category: String,
    pub scan_pair_prefix: u8,
    pub unpair_prefix: u8,
    pub cancel_prefix: u8,
    pub metadata_prefix: u8,
    #[serde(default)]
    pub metadata_claim_interface: Option<u8>,
    #[serde(default)]
    pub metadata_source_class: Option<String>,
    pub metadata_mouse_layout: bool,
    pub transport: ReceiverCapability,
}

#[cfg(test)]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CapabilityCatalog {
    schema_version: u8,
    products: Vec<PairingCapability>,
}

#[cfg(test)]
fn decode_capabilities(json: &str) -> anyhow::Result<CapabilityCatalog> {
    let catalog: CapabilityCatalog =
        serde_json::from_str(json).context("Invalid pairing capability catalog")?;
    ensure!(
        catalog.schema_version == 1 && !catalog.products.is_empty(),
        "Invalid pairing capability schema"
    );
    let mut products = BTreeSet::new();
    for capability in &catalog.products {
        capability.transport.validate_source()?;
        ensure!(
            capability.product_id == capability.transport.product_id
                && matches!(capability.category.as_str(), "MOUSE" | "KEYBOARD")
                && products.insert((capability.product_id, capability.category.as_str())),
            "Invalid or repeated pairing product/category branch"
        );
        let bits =
            (0..capability.transport.transaction_modulus).fold(0, |bits, counter| bits | counter);
        ensure!(
            [
                capability.scan_pair_prefix,
                capability.unpair_prefix,
                capability.cancel_prefix,
                capability.metadata_prefix,
            ]
            .into_iter()
            .all(|prefix| prefix & bits == 0),
            "Pairing namespace overlaps its transaction counter"
        );
        if let Some(source_class) = capability.metadata_source_class.as_deref() {
            ensure!(
                matches!(
                    source_class,
                    "rzDevice25" | "rzDevice25DualLinkMouse" | "rzDevice25DualLinkKeyboard"
                ),
                "Unknown pairing metadata factory"
            );
        }
    }
    Ok(catalog)
}

pub fn capability(product_id: u16, category: &str) -> Option<&'static PairingCapability> {
    static CATALOG: OnceLock<Vec<PairingCapability>> = OnceLock::new();
    CATALOG
        .get_or_init(crate::receiver_pairing_capabilities::products)
        .iter()
        .find(|c| c.product_id == product_id && c.category == category)
}

/// Source756's cached-master route is the same DualLinkKeyboard class as its
/// own757 route. The physical PID/interface changes; the class prefixes do not.
pub fn for_source(
    product_id: u16,
    category: &str,
    source_product_id: Option<u16>,
) -> anyhow::Result<PairingCapability> {
    if source_product_id == Some(756) {
        ensure!(category == "MOUSE", "756 DualLink 仅接受当前相反类别 MOUSE");
        let mut selected = capability(757, category)
            .context("缺少756 当前配对 factory")?
            .clone();
        selected.transport = super::receiver_capabilities::for_source(product_id, Some(756))?;
        selected.product_id = product_id;
        if product_id == 756 {
            // Wired756 uses the base class; the UI only reaches this path for
            // its unconditional cancellation, not for a pairing operation.
            selected.scan_pair_prefix = 0;
            selected.unpair_prefix = 0;
            selected.cancel_prefix = 0;
        }
        Ok(selected)
    } else {
        let selected = capability(product_id, category).context("产品和类别没有当前配对证据")?;
        ensure!(
            source_product_id.is_none_or(|source| source == selected.transport.source_product_id),
            "配对 caller 与当前 factory 不匹配"
        );
        Ok(selected.clone())
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PairingProductEdition {
    #[serde(rename = "EID")]
    pub(crate) edition_id: u16,
    #[serde(rename = "Name")]
    pub(crate) name: String,
    #[serde(rename = "CHSName")]
    pub(crate) chinese_name: String,
}

impl PairingProductEdition {
    pub fn edition_id(&self) -> u16 {
        self.edition_id
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn chinese_name(&self) -> &str {
        &self.chinese_name
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PairingCatalogDevice {
    #[serde(rename = "DeviceDonglePid")]
    pub(crate) dongle_id: u16,
    #[serde(rename = "ProductInfo")]
    pub(crate) editions: Vec<PairingProductEdition>,
    #[serde(
        rename = "DeviceSiblings",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) siblings: Option<Vec<u16>>,
    #[serde(rename = "DevicePairingBuddies")]
    pub(crate) pairing_buddies: Vec<u16>,
    #[serde(rename = "DeviceType")]
    pub(crate) category: PairingDeviceCategory,
    #[serde(
        rename = "DeviceFamily",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) family: Option<String>,
}

impl PairingCatalogDevice {
    pub fn dongle_id(&self) -> u16 {
        self.dongle_id
    }
    pub fn editions(&self) -> &[PairingProductEdition] {
        &self.editions
    }
    pub fn siblings(&self) -> Option<&[u16]> {
        self.siblings.as_deref()
    }
    pub fn pairing_buddies(&self) -> &[u16] {
        &self.pairing_buddies
    }

    pub fn category(&self) -> &'static str {
        self.category.name()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) enum PairingDeviceCategory {
    #[serde(rename = "MOUSE")]
    Mouse,
    #[serde(rename = "KEYBOARD")]
    Keyboard,
    #[serde(rename = "ACCESSORY")]
    Accessory,
    #[serde(rename = "MOUSEMAT")]
    MouseMat,
}

impl PairingDeviceCategory {
    fn name(self) -> &'static str {
        match self {
            Self::Mouse => "MOUSE",
            Self::Keyboard => "KEYBOARD",
            Self::Accessory => "ACCESSORY",
            Self::MouseMat => "MOUSEMAT",
        }
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct PairingMaster {
    #[serde(rename = "productId")]
    pub(crate) product_id: u16,
    #[serde(rename = "dongleId")]
    pub(crate) dongle_id: u16,
    #[serde(rename = "bleId")]
    pub(crate) ble_id: u16,
    #[serde(rename = "claimInterface")]
    pub(crate) claim_interface: u8,
    #[serde(
        rename = "supports8KHzPollingRate",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub(crate) supports_eight_khz: Option<bool>,
    #[serde(rename = "canPairTwoDevices")]
    pub(crate) can_pair_two_devices: bool,
}

impl PairingMaster {
    pub fn product_id(&self) -> u16 {
        self.product_id
    }
    pub fn dongle_id(&self) -> u16 {
        self.dongle_id
    }
    pub fn ble_id(&self) -> u16 {
        self.ble_id
    }
}

#[derive(Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(untagged)]
pub(crate) enum CatalogProductIds {
    One(u16),
    Many(Vec<u16>),
}

impl CatalogProductIds {
    fn is_scalar(&self, product_id: u16) -> bool {
        matches!(self, Self::One(id) if *id == product_id)
    }

    fn validate(&self) -> anyhow::Result<()> {
        let valid = |id: &u16| *id > 0;
        ensure!(
            match self {
                Self::One(id) => valid(id),
                Self::Many(ids) => !ids.is_empty() && ids.iter().all(valid),
            },
            "Invalid pairing available radio IDs"
        );
        Ok(())
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct PairingAvailableProduct {
    #[serde(rename = "productId")]
    pub(crate) product_id: u16,
    #[serde(rename = "dongleId", default)]
    pub(crate) dongle_id: Option<CatalogProductIds>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
pub(crate) struct PairingCatalog {
    pub(crate) schema_version: u8,
    pub(crate) masters: Vec<PairingMaster>,
    pub(crate) devices: Vec<PairingCatalogDevice>,
    pub(crate) available: Vec<PairingAvailableProduct>,
}

#[cfg(test)]
fn decode_pairing_catalog(json: &str) -> anyhow::Result<PairingCatalog> {
    let catalog: PairingCatalog =
        serde_json::from_str(json).context("Invalid pairing source catalog")?;
    ensure!(
        catalog.schema_version == 1 && !catalog.devices.is_empty(),
        "Invalid pairing source schema"
    );
    let mut masters = BTreeSet::new();
    for master in &catalog.masters {
        ensure!(
            master.product_id != 0 && masters.insert(master.product_id),
            "Invalid or repeated pairing master"
        );
    }
    let mut devices = BTreeSet::new();
    for device in &catalog.devices {
        ensure!(
            device.dongle_id > 0
                && device.dongle_id != u16::MAX
                && devices.insert(device.dongle_id),
            "Invalid or repeated pairing radio"
        );
        let mut editions = BTreeSet::new();
        for edition in &device.editions {
            ensure!(
                editions.insert(edition.edition_id),
                "Repeated pairing product edition"
            );
        }
        ensure!(
            device
                .pairing_buddies
                .iter()
                .chain(device.siblings.iter().flatten())
                .all(|pid| *pid > 0),
            "Invalid pairing companion product"
        );
    }
    for product in &catalog.available {
        ensure!(product.product_id > 0, "Invalid available pairing product");
        if let Some(ids) = &product.dongle_id {
            ids.validate()?;
        }
    }
    Ok(catalog)
}

fn pairing_catalog() -> &'static PairingCatalog {
    static CATALOG: OnceLock<PairingCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let catalog = crate::receiver_pairing_catalog::catalog();
        assert_eq!(catalog.schema_version, 1, "fixed pairing source schema");
        for product in &catalog.available {
            if let Some(ids) = &product.dongle_id {
                ids.validate().expect("fixed available radio IDs");
            }
        }
        catalog
    })
}

pub fn catalog_master(product_id: u32) -> Option<&'static PairingMaster> {
    pairing_catalog()
        .masters
        .iter()
        .find(|master| u32::from(master.product_id) == product_id)
}

pub fn catalog_device(dongle_id: u16) -> Option<&'static PairingCatalogDevice> {
    pairing_catalog()
        .devices
        .iter()
        .find(|device| device.dongle_id == dongle_id)
}

/// Source UI _5 reads a cached master's DeviceInfo capability, independently
/// of the persisted binding record or any hardware status.
pub fn master_can_pair_two_devices(product_id: u32) -> bool {
    pairing_catalog()
        .masters
        .iter()
        .find(|master| u32::from(master.product_id) == product_id)
        .is_some_and(|master| master.can_pair_two_devices)
}

/// Current756's event54 callback calls ProductInfo.find before getEdition or
/// MW_RESPONSE_UI. A scan may contain an unknown radio, but an absent catalog
/// device aborts this callback; it cannot be projected as a successful UI pair.
pub fn source756_pair_device(dongle_id: u16) -> anyhow::Result<&'static PairingCatalogDevice> {
    ensure!(dongle_id > 0 && dongle_id != u16::MAX, "配对 radio ID 无效");
    let device = pairing_catalog()
        .devices
        .iter()
        .find(|device| device.dongle_id == dongle_id)
        .context("756 原配对回调在 ProductInfo.find 前缺少目录设备，未发布 UI 响应")?;
    Ok(device)
}

/// Compatibility serialization boundary for existing diagnostic callers.
/// Device selection and schema validation use the typed source catalog.
pub fn source756_pair_catalog_device(dongle_id: u16) -> anyhow::Result<&'static serde_json::Value> {
    let selected = source756_pair_device(dongle_id)?;
    static ROWS: OnceLock<Vec<serde_json::Value>> = OnceLock::new();
    let rows = ROWS.get_or_init(|| {
        pairing_catalog()
            .devices
            .iter()
            .map(|device| serde_json::to_value(device).expect("typed pairing source serialization"))
            .collect()
    });
    let index = pairing_catalog()
        .devices
        .iter()
        .position(|device| std::ptr::eq(device, selected))
        .context("Selected pairing device no longer belongs to the source catalog")?;
    Ok(&rows[index])
}

pub fn registered_source756_pair_product(dongle_id: u16) -> Option<u32> {
    pairing_catalog()
        .available
        .iter()
        .find(|product| {
            product
                .dongle_id
                .as_ref()
                .is_some_and(|ids| ids.is_scalar(dongle_id))
        })
        .map(|product| u32::from(product.product_id))
}

/// An interrupt/callback subscription bound to the exact retained physical owner.
/// Implementations must expose the middleware parser's record-first bytes (not a
/// Feature reply or a guessed ReportID prefix), and release every registration
/// and reader on finish. Unsupported event provenance must fail begin.
pub trait HardwareEvents {
    fn begin(&mut self) -> anyhow::Result<()>;
    fn receive(&mut self, timeout: Duration) -> anyhow::Result<Option<Vec<u8>>>;
    fn finish(&mut self) -> anyhow::Result<()>;
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ScanCandidate {
    pub dongle_id: u16,
    /// Current Protocol25 parser uses the low byte as edition, not a BE u16.
    pub edition_id: u8,
    pub keyboard_layout: u8,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct EditionReading {
    pub keyboard_layout: u8,
    pub edition_id: u8,
    pub firmware_id: Option<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Notification {
    Pair {
        status: u8,
    },
    Scan {
        status: u8,
        candidates: Vec<ScanCandidate>,
    },
}

/// Current Protocol25 parser: records 5/9, event 54/55. Preserve raw status;
/// strict truncation/count checks are application validation, not source enums.
pub fn notification(data: &[u8]) -> anyhow::Result<Option<Notification>> {
    ensure!(!data.is_empty(), "硬件事件为空");
    if !matches!(data[0], 5 | 9) {
        return Ok(None);
    }
    ensure!(data.len() >= 2, "硬件通知缺少事件编号");
    match data[1] {
        54 => {
            ensure!(data.len() >= 3, "配对状态事件截断");
            Ok(Some(Notification::Pair { status: data[2] }))
        }
        55 => {
            ensure!(data.len() >= 4, "扫描状态事件截断");
            let count = usize::from(data[3]);
            ensure!(
                count <= 3 && data.len() >= 4 + count * 4,
                "扫描事件记录数量超出当前解析器或数据截断"
            );
            let candidates = data[4..4 + count * 4]
                .chunks_exact(4)
                .map(|r| ScanCandidate {
                    dongle_id: u16::from_be_bytes([r[0], r[1]]),
                    keyboard_layout: r[2],
                    edition_id: r[3],
                })
                .collect();
            Ok(Some(Notification::Scan {
                status: data[2],
                candidates,
            }))
        }
        _ => Ok(None),
    }
}

#[derive(Debug, Serialize)]
pub struct PairSucceeded {
    pub dongle_id: u16,
    pub hardware_status: u8,
}

/// Original Unpair helper constructs productId from its argument after ack.
/// This does not claim post-unpair V2 readback or runtime/storage cleanup.
#[derive(Debug, Serialize)]
pub struct UnpairAcknowledged {
    pub dongle_id: u16,
}

pub struct Session<'a> {
    device: &'a dyn FeatureTransport,
    events: &'a mut dyn HardwareEvents,
    cap: &'a PairingCapability,
    sequence: u8,
}

impl<'a> Session<'a> {
    /// Independent connectDevice follows the same Unpair factory's next
    /// transaction; collection reopening does not restart its counter.
    pub fn next_transaction_sequence(&self) -> u8 {
        self.sequence
    }

    pub fn new(
        device: &'a dyn FeatureTransport,
        events: &'a mut dyn HardwareEvents,
        cap: &'a PairingCapability,
    ) -> anyhow::Result<Self> {
        let source_transport = super::receiver_capabilities::for_source(
            cap.product_id,
            Some(cap.transport.source_product_id),
        )?;
        ensure!(
            matches!(cap.product_id, 164 | 179 | 241 | 756 | 757)
                && cap.transport.product_id == cap.product_id
                && cap.transport.report_bytes == 91
                && cap.transport.report_id == source_transport.report_id
                && cap.transport.transaction_modulus == 31
                && matches!(cap.category.as_str(), "MOUSE" | "KEYBOARD"),
            "配对能力未匹配当前已核验产品"
        );
        Ok(Self {
            device,
            events,
            cap,
            sequence: 0,
        })
    }

    fn exchange(
        &mut self,
        command: [u8; 3],
        payload: &[u8],
        validate: &impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Vec<u8>> {
        let validate = &|| {
            validate().map_err(|error| {
                crate::backend::interrupt_exchange(
                    error,
                    crate::backend::ExchangeInterruption::Validation,
                )
            })
        };
        validate()?;
        ensure!(
            payload.len() <= usize::from(command[0]),
            "配对命令 payload 超出原报文"
        );
        let prefix = if command == SCAN && payload == [4] {
            // DUALLINK_CANCEL uses the primary linker, whereas ordinary scan
            // and pairing use f2(category,true). Keyboard's scan namespace is
            // therefore different from its original cancellation namespace.
            self.cap.cancel_prefix
        } else if command == EDITION || command == HARDWARE_MODULE {
            self.cap.metadata_prefix
        } else if command == UNPAIR {
            self.cap.unpair_prefix
        } else {
            self.cap.scan_pair_prefix
        };
        let transaction = prefix | self.sequence;
        self.sequence = (self.sequence + 1) % self.cap.transport.transaction_modulus;
        let mut outgoing = vec![0; 91];
        let transport = &self.cap.transport;
        let metadata = command == EDITION || command == HARDWARE_MODULE;
        let claim_interface = if metadata {
            self.cap
                .metadata_claim_interface
                .unwrap_or(transport.claim_interface)
        } else {
            transport.claim_interface
        };
        let report_id = if metadata
            && matches!(
                self.cap.metadata_source_class.as_deref(),
                Some("rzDevice25DualLinkMouse" | "rzDevice25DualLinkKeyboard")
            ) {
            claim_interface
        } else {
            transport.report_id
        };
        outgoing[0] = report_id;
        outgoing[2] = transaction;
        outgoing[6..9].copy_from_slice(&command);
        outgoing[9..9 + payload.len()].copy_from_slice(payload);
        outgoing[89] = outgoing[3..89].iter().fold(0, |sum, byte| sum ^ byte);
        crate::feature_exchange::with_validated_feature_reply(
            self.device,
            crate::feature_exchange::with_protocol25_scope(
                crate::backend::ExchangeOptions::mutex_out(Duration::from_millis(
                    transport.sleep_between_out_ms,
                )),
                if metadata {
                    self.cap
                        .metadata_source_class
                        .as_deref()
                        .unwrap_or(&transport.source_class)
                } else {
                    &transport.source_class
                },
            ),
            command,
            transaction,
            validate,
            |device, lease| {
                let mut last_error = "没有收到设备应答".to_owned();
                for out_attempt in 0..transport.max_retry_out {
                    validate()?;
                    let sent = device.send_feature_at_collection_max_counted(&outgoing);
                    if let Err(validation) = validate() {
                        return Err(crate::backend::complete_native_io(sent, validation));
                    }
                    sent.and_then(|count| {
                        crate::backend::require_complete_send(count, outgoing.len())
                    })?;
                    std::thread::sleep(Duration::from_millis(transport.sleep_between_out_in_ms));
                    let mut resend_out_command = false;
                    for attempt in 0..transport.max_retry_in {
                        if attempt > 0 {
                            std::thread::sleep(Duration::from_millis(
                                transport.sleep_between_in_ms,
                            ));
                        }
                        validate()?;
                        let mut incoming = vec![0; 91];
                        incoming[0] = if report_id == 0 { claim_interface } else { report_id };
                        let received = device.get_feature_at_collection_max(&mut incoming);
                        let received = match validate() {
                            Ok(()) => received,
                            Err(validation) => Err(crate::backend::complete_native_read(
                                received, &incoming, validation,
                            )),
                        };
                        let count = match received {
                            Ok(n) => n,
                            Err(error) if crate::backend::exchange_interrupted(&error) => {
                                return Err(crate::receiver_feature_channel::retain_failed_reply(
                                    error,
                                    command,
                                    transaction,
                                ));
                            }
                            Err(e) if resend_out_command => {
                                last_error = format!("{e:#}");
                                break;
                            }
                            Err(e) => return Err(e),
                        };
                        ensure!(
                            (9..=incoming.len()).contains(&count),
                            "配对命令应答长度不匹配"
                        );
                        if incoming[2] != transaction || incoming[7..9] != command[1..] {
                            last_error = "配对命令或事务应答不匹配".into();
                            break;
                        }
                        match incoming[1] {
                            1 => {
                                last_error = "设备忙".into();
                                resend_out_command = true;
                                continue;
                            }
                            2 => {}
                            0 | 3 | 4 => {
                                last_error = format!("设备应答状态 {}", incoming[1]);
                                break;
                            }
                            status => anyhow::bail!("配对命令不支持或状态未知 {status}"),
                        }
                        let length = usize::from(incoming[6]);
                        return Ok(incoming[9..(9 + length).min(count)].to_vec());
                    }
                    if out_attempt + 1 < transport.max_retry_out {
                        lease.retry_release()?;
                    }
                }
                anyhow::bail!("配对命令失败：{last_error}")
            },
        )
    }

    /// Source `getEdition` retries the [3,0,134] helper up to three times with
    /// a 100ms delay between failures. Response data[0] is keyboardLayout and
    /// data[1] is edition; the caller may preserve the firmware byte separately.
    pub fn get_edition(
        &mut self,
        validate: impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<EditionReading> {
        for attempt in 0..3 {
            match self.exchange(EDITION, &[], &validate) {
                Ok(data) if data.len() >= 2 => {
                    return Ok(EditionReading {
                        keyboard_layout: data[0],
                        edition_id: data[1],
                        firmware_id: data.get(2).copied(),
                    });
                }
                Err(error) if crate::backend::exchange_interrupted(&error) => {
                    return Err(error);
                }
                Err(error) if attempt < 2 => {
                    std::thread::sleep(Duration::from_millis(100));
                    validate().with_context(|| {
                        format!("读取 edition 重试前设备身份校验失败：{error:#}")
                    })?;
                }
                Ok(_) => anyhow::bail!("读取 edition 响应截断"),
                Err(error) => return Err(error.context("读取 edition 重试失败")),
            }
        }
        anyhow::bail!("读取 edition 未返回")
    }

    pub fn get_mouse_layout(
        &mut self,
        validate: impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<u8> {
        let data = self.exchange(HARDWARE_MODULE, &[], &validate)?;
        data.first().copied().context("鼠标 sidepad 状态响应截断")
    }

    fn cancel_scan(&mut self, validate: &impl Fn() -> anyhow::Result<()>) -> anyhow::Result<()> {
        self.exchange(SCAN, &[4], validate)?;
        Ok(())
    }

    /// This acknowledgement confirms the actual cancel command only. It does
    /// not establish an unpair, a device state change or storage cleanup.
    pub fn cancel(&mut self, validate: impl Fn() -> anyhow::Result<()>) -> anyhow::Result<()> {
        self.cancel_scan(&validate)
    }

    fn next_event(
        &mut self,
        deadline: Instant,
        validate: &impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Option<Notification>> {
        validate()?;
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .context("等待硬件配对事件超时")?;
        // Short reads allow cancellation/deadline checks; this is application policy.
        let raw = self
            .events
            .receive(remaining.min(Duration::from_millis(50)))?;
        validate()?;
        raw.as_deref()
            .map(notification)
            .transpose()
            .map(Option::flatten)
    }

    fn finish<T>(
        &mut self,
        result: anyhow::Result<T>,
        cancel_on_error: bool,
        cleanup_validate: &impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<T> {
        // Cancellation cannot use the operation's canceled signal. The separate
        // cleanup guard must still verify the original physical owner identity.
        let canceled = if result.is_err() && cancel_on_error && self.cap.product_id != 179 {
            self.cancel_scan(cleanup_validate).err()
        } else {
            None
        };
        let released = self.events.finish().err();
        match result {
            Ok(value) => {
                ensure!(released.is_none(), "硬件事件订阅清理失败：{:?}", released);
                Ok(value)
            }
            Err(error) => Err(error.context(format!(
                "取消扫描结果：{canceled:?}；事件订阅清理结果：{released:?}"
            ))),
        }
    }

    pub fn scan(
        &mut self,
        deadline: Instant,
        validate: impl Fn() -> anyhow::Result<()>,
        cleanup_validate: impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Vec<ScanCandidate>> {
        ensure!(deadline > Instant::now(), "扫描期限已过，未发送命令");
        validate()?;
        if let Err(error) = self.events.begin() {
            let cleanup = self.events.finish().err();
            return Err(error.context(format!("硬件事件订阅失败，清理结果：{cleanup:?}")));
        }
        let result = (|| {
            let ack = self.exchange(SCAN, &[1], &validate)?;
            ensure!(!ack.is_empty(), "扫描命令应答状态截断");
            // An ack (including status End) is never the scan candidate event.
            loop {
                if let Some(Notification::Scan {
                    status: 2,
                    candidates,
                }) = self.next_event(deadline, &validate)?
                {
                    return Ok(candidates);
                }
            }
        })();
        self.finish(result, true, &cleanup_validate)
    }

    pub fn pair(
        &mut self,
        dongle_id: u16,
        mode: u8,
        deadline: Instant,
        validate: impl Fn() -> anyhow::Result<()>,
        cleanup_validate: impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<PairSucceeded> {
        ensure!(dongle_id > 0 && dongle_id != u16::MAX, "配对目标编号无效");
        // Current UI initiates mode 1; cancellation uses deviceScan(4).
        ensure!(mode == 1, "配对 mode 不是当前 UI 的开始配对值");
        ensure!(deadline > Instant::now(), "配对期限已过，未发送命令");
        validate()?;
        if let Err(error) = self.events.begin() {
            let cleanup = self.events.finish().err();
            return Err(error.context(format!("硬件事件订阅失败，清理结果：{cleanup:?}")));
        }
        let result = (|| {
            let pid = dongle_id.to_be_bytes();
            let ack = self.exchange(PAIR, &[mode, pid[0], pid[1]], &validate)?;
            ensure!(!ack.is_empty(), "配对应答 mode 截断");
            loop {
                match self.next_event(deadline, &validate)? {
                    Some(Notification::Pair { status: 2 }) => {
                        return Ok(PairSucceeded {
                            dongle_id,
                            hardware_status: 2,
                        });
                    }
                    Some(Notification::Pair { status: 3 }) => {
                        anyhow::bail!("设备报告 PairingTimeout")
                    }
                    _ => {}
                }
            }
        })();
        self.finish(result, true, &cleanup_validate)
    }

    pub fn unpair(
        &mut self,
        dongle_id: u16,
        validate: impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<UnpairAcknowledged> {
        ensure!(
            dongle_id > 0 && dongle_id != u16::MAX,
            "取消配对目标编号无效"
        );
        self.exchange(UNPAIR, &dongle_id.to_be_bytes(), &validate)?;
        validate()?;
        Ok(UnpairAcknowledged { dongle_id })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::VecDeque, sync::Mutex};

    #[test]
    fn source756_pair_catalog_retains_unregistered_radios_and_callback_failure() {
        let unregistered = source756_pair_catalog_device(180).unwrap();
        assert_eq!(unregistered["DeviceSiblings"][0], 181);
        let no_sibling = source756_pair_catalog_device(241).unwrap();
        assert!(no_sibling.get("DeviceSiblings").is_none());
        let error = source756_pair_catalog_device(60000).unwrap_err();
        assert!(format!("{error:#}").contains("未发布 UI 响应"));
        assert!(source756_pair_catalog_device(0).is_err());
        assert!(source756_pair_catalog_device(u16::MAX).is_err());
    }

    #[test]
    fn typed_pairing_master_preserves_receiver_ids_and_optional_polling_flag() {
        assert!(master_can_pair_two_devices(241));
        assert!(!master_can_pair_two_devices(756));
        assert!(!master_can_pair_two_devices(65000));
        assert_eq!(
            serde_json::to_value(catalog_master(241).unwrap()).unwrap(),
            serde_json::json!({
                "productId":241,"dongleId":241,"bleId":0,"claimInterface":3,
                "canPairTwoDevices":true,"supports8KHzPollingRate":true
            })
        );
        assert!(
            serde_json::to_value(catalog_master(756).unwrap())
                .unwrap()
                .get("supports8KHzPollingRate")
                .is_none()
        );
    }

    #[test]
    fn pairing_catalog_rejects_wrong_shapes_and_repeated_source_branches() {
        let mut value = serde_json::json!({"schema_version":1,"masters":[],"available":[],"devices":[source756_pair_device(757).unwrap()]});
        assert!(decode_pairing_catalog(&value.to_string()).is_ok());
        value["devices"][0]["ProductInfo"] = serde_json::Value::Null;
        assert!(decode_pairing_catalog(&value.to_string()).is_err());
        let mut value =
            serde_json::json!({"schema_version":1,"products":[capability(164,"MOUSE").unwrap()]});
        assert!(decode_capabilities(&value.to_string()).is_ok());
        let repeated = value["products"][0].clone();
        value["products"].as_array_mut().unwrap().push(repeated);
        assert!(decode_capabilities(&value.to_string()).is_err());
        let scalar: CatalogProductIds = serde_json::from_str("757").unwrap();
        let array: CatalogProductIds = serde_json::from_str("[757]").unwrap();
        assert!(scalar.is_scalar(757));
        assert!(!array.is_scalar(757));
    }

    struct Mock {
        sent: Mutex<Vec<Vec<u8>>>,
        replies: Mutex<VecDeque<Vec<u8>>>,
    }
    impl Mock {
        fn new(replies: &[&[u8]]) -> Self {
            Self {
                sent: Mutex::new(Vec::new()),
                replies: Mutex::new(replies.iter().map(|r| r.to_vec()).collect()),
            }
        }
    }
    impl FeatureTransport for Mock {
        fn send_feature(&self, data: &[u8]) -> anyhow::Result<()> {
            self.sent.lock().unwrap().push(data.to_vec());
            Ok(())
        }
        fn get_feature(&self, data: &mut [u8]) -> anyhow::Result<usize> {
            let response = self
                .replies
                .lock()
                .unwrap()
                .pop_front()
                .context("mock exhausted")?;
            let sent = self.sent.lock().unwrap().last().unwrap().clone();
            data.fill(0);
            data[1] = 2;
            data[2] = sent[2];
            data[6] = response.len() as u8;
            data[7..9].copy_from_slice(&sent[7..9]);
            data[9..9 + response.len()].copy_from_slice(&response);
            Ok(91)
        }
        fn metadata(&self) -> serde_json::Value {
            serde_json::json!({"mock":true})
        }
    }
    struct Events {
        pending: VecDeque<Vec<u8>>,
        began: bool,
        ended: bool,
        reads: usize,
    }
    impl Events {
        fn new(data: &[&[u8]]) -> Self {
            Self {
                pending: data.iter().map(|x| x.to_vec()).collect(),
                began: false,
                ended: false,
                reads: 0,
            }
        }
    }
    impl HardwareEvents for Events {
        fn begin(&mut self) -> anyhow::Result<()> {
            self.began = true;
            Ok(())
        }
        fn receive(&mut self, _timeout: Duration) -> anyhow::Result<Option<Vec<u8>>> {
            self.reads += 1;
            self.pending
                .pop_front()
                .map(Some)
                .context("mock event exhausted")
        }
        fn finish(&mut self) -> anyhow::Result<()> {
            self.ended = true;
            Ok(())
        }
    }
    fn cap(pid: u16, category: &str) -> PairingCapability {
        let c = capability(pid, category).unwrap();
        let mut transport = c.transport.clone();
        transport.max_retry_out = 1;
        transport.max_retry_in = 1;
        transport.sleep_between_out_ms = 0;
        transport.sleep_between_out_in_ms = 0;
        transport.sleep_between_in_ms = 0;
        PairingCapability {
            product_id: pid,
            category: category.into(),
            scan_pair_prefix: c.scan_pair_prefix,
            unpair_prefix: c.unpair_prefix,
            cancel_prefix: c.cancel_prefix,
            metadata_prefix: c.metadata_prefix,
            metadata_claim_interface: c.metadata_claim_interface,
            metadata_source_class: c.metadata_source_class.clone(),
            metadata_mouse_layout: c.metadata_mouse_layout,
            transport,
        }
    }
    fn deadline() -> Instant {
        Instant::now() + Duration::from_secs(1)
    }

    #[test]
    fn hyperpolling_receiver_category_factories_adjust_scan_pair_namespace_only() {
        for (category, scan_pair, unpair, metadata) in
            [("MOUSE", 0, 128, 0), ("KEYBOARD", 128, 0, 128)]
        {
            let cap = cap(179, category);
            let device = Mock::new(&[&[1], &[1], &[1], &[0, 0, 0]]);
            let mut events = Events::new(&[]);
            let mut session = Session::new(&device, &mut events, &cap).unwrap();
            session.exchange(SCAN, &[1], &|| Ok(())).unwrap();
            session.exchange(PAIR, &[1, 2, 5], &|| Ok(())).unwrap();
            session.exchange(UNPAIR, &[2, 5], &|| Ok(())).unwrap();
            session.exchange(EDITION, &[], &|| Ok(())).unwrap();
            let sent = device.sent.lock().unwrap();
            assert_eq!(
                sent.iter().map(|packet| packet[2]).collect::<Vec<_>>(),
                [scan_pair, scan_pair | 1, unpair | 2, metadata | 3]
            );
        }
    }

    #[test]
    fn hyperpolling_receiver_category_failure_releases_events_without_a_primary_cancel() {
        let cap = cap(179, "MOUSE");
        let device = Mock::new(&[&[1]]);
        let mut events = Events::new(&[&[5, 54, 3]]);
        assert!(
            Session::new(&device, &mut events, &cap)
                .unwrap()
                .pair(517, 1, deadline(), || Ok(()), || Ok(()))
                .is_err()
        );
        assert!(events.ended);
        assert_eq!(device.sent.lock().unwrap().len(), 1);
    }

    #[test]
    fn source756_keeps_keyboard_prefix_on_a_cached_master_and_idle_cancel_sends() {
        let mut cap = for_source(241, "MOUSE", Some(756)).unwrap();
        cap.transport.sleep_between_out_ms = 0;
        cap.transport.sleep_between_out_in_ms = 0;
        cap.transport.sleep_between_in_ms = 0;
        cap.transport.max_retry_in = 1;
        cap.transport.max_retry_out = 1;
        assert_eq!(cap.transport.claim_interface, 3);
        assert_eq!(cap.metadata_claim_interface, Some(2));
        assert_eq!(cap.transport.transaction_prefix, 128);
        assert!(!cap.metadata_mouse_layout);
        let device = Mock::new(&[&[4]]);
        let mut events = Events::new(&[]);
        Session::new(&device, &mut events, &cap)
            .unwrap()
            .cancel(|| Ok(()))
            .unwrap();
        let sent = device.sent.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0][0], 3);
        assert_eq!(sent[0][2], 128);
        assert_eq!(&sent[0][6..10], &[1, 0, 70, 4]);
        assert!(!events.began);
        let own = super::super::receiver_capabilities::capability(241).unwrap();
        assert_eq!(own.transaction_prefix, 224);
    }

    #[test]
    fn source756_metadata_uses_its_separately_selected_mouse_interface() {
        let mut cap = for_source(241, "MOUSE", Some(756)).unwrap();
        cap.transport.sleep_between_out_ms = 0;
        cap.transport.sleep_between_out_in_ms = 0;
        cap.transport.sleep_between_in_ms = 0;
        let device = Mock::new(&[&[6, 128, 2]]);
        let mut events = Events::new(&[]);
        let reading = Session::new(&device, &mut events, &cap)
            .unwrap()
            .get_edition(|| Ok(()))
            .unwrap();
        assert_eq!(reading.keyboard_layout, 6);
        let sent = device.sent.lock().unwrap();
        assert_eq!(sent[0][0], 2);
        assert_eq!(sent[0][2], 0);
        assert_eq!(&sent[0][6..9], &[3, 0, 134]);
    }

    #[test]
    fn one_shot_edition_admission_failure_stops_all_pairing_factories() {
        use crate::feature_exchange::tests::ExchangeProbe;
        use std::cell::Cell;
        for source in crate::receiver_pairing_capabilities::products() {
            let pid = source.product_id;
            let category = source.category.as_str();
            let cap = cap(pid, category);
            let device = Mock::new(&[]);
            let admissions = Cell::new(0);
            let admit = || {
                let previous = admissions.replace(admissions.get() + 1);
                ensure!(previous != 0, "one-shot pairing edition admission failure");
                Ok(())
            };
            let probe = ExchangeProbe::with_admission_check(&device, &admit);
            let mut events = Events::new(&[]);
            let error = Session::new(&probe, &mut events, &cap)
                .unwrap()
                .get_edition(|| Ok(()))
                .unwrap_err();
            assert!(crate::backend::exchange_interrupted(&error));
            assert_eq!(admissions.get(), 1, "PID{pid} / {category}");
            assert!(device.sent.lock().unwrap().is_empty());
            assert!(!events.began && !events.ended);
        }
    }

    #[test]
    fn edition_release_failure_is_not_retried_or_replaced_by_a_later_success() {
        let cap = cap(164, "MOUSE");
        let device = Mock::new(&[&[6, 128, 2], &[6, 128, 2]]);
        let first = std::cell::Cell::new(true);
        let finish_check = || {
            ensure!(
                !first.replace(false),
                "simulated first edition release failure"
            );
            Ok(())
        };
        let probe = crate::feature_exchange::tests::ExchangeProbe::with_finish_check(
            &device,
            &finish_check,
        );
        let mut events = Events::new(&[]);
        let error = Session::new(&probe, &mut events, &cap)
            .unwrap()
            .get_edition(|| Ok(()))
            .unwrap_err();
        let completed = crate::CompletedFeatureExchange::from_error(&error).unwrap();
        assert_eq!(completed.command(), [3, 0, 134]);
        assert_eq!(completed.data(), [6, 128, 2]);
        assert!(crate::backend::exchange_finalization_failed(&error));
        assert_eq!(device.sent.lock().unwrap().len(), 1);
        assert_eq!(device.replies.lock().unwrap().len(), 1);
        assert!(!events.began && !events.ended);
    }

    #[test]
    fn linker_exchange_retains_busy_state_on_in_exception_and_stops_send_exceptions() {
        struct Faults {
            base: Mock,
            states: Mutex<VecDeque<Result<u8, &'static str>>>,
            fail_send: bool,
        }
        impl FeatureTransport for Faults {
            fn send_feature(&self, data: &[u8]) -> anyhow::Result<()> {
                self.base.send_feature(data)?;
                ensure!(!self.fail_send, "send interrupted");
                Ok(())
            }
            fn get_feature(&self, data: &mut [u8]) -> anyhow::Result<usize> {
                let state = self
                    .states
                    .lock()
                    .unwrap()
                    .pop_front()
                    .expect("extra IN")
                    .map_err(anyhow::Error::msg)?;
                let count = self.base.get_feature(data)?;
                data[1] = state;
                Ok(count)
            }
            fn metadata(&self) -> serde_json::Value {
                serde_json::Value::Null
            }
        }
        let mut cap = cap(164, "MOUSE");
        cap.transport.max_retry_out = 3;
        cap.transport.max_retry_in = 2;
        for (states, fail_send, succeeds, sends) in [
            (vec![Ok(1), Err("IN after busy"), Ok(2)], false, true, 2),
            (vec![Err("first IN")], false, false, 1),
            (vec![], true, false, 1),
        ] {
            let device = Faults {
                base: Mock::new(&[&[1], &[1]]),
                states: Mutex::new(states.into()),
                fail_send,
            };
            let mut events = Events::new(&[]);
            let result =
                Session::new(&device, &mut events, &cap)
                    .unwrap()
                    .exchange(SCAN, &[1], &|| Ok(()));
            assert_eq!(result.is_ok(), succeeds);
            assert_eq!(device.base.sent.lock().unwrap().len(), sends);
        }
    }

    #[test]
    fn protocol25_keeps_edition_byte_separate_from_layout() {
        assert_eq!(
            notification(&[5, 55, 2, 2, 0, 183, 7, 2, 0, 188, 5, 9]).unwrap(),
            Some(Notification::Scan {
                status: 2,
                candidates: vec![
                    ScanCandidate {
                        dongle_id: 183,
                        keyboard_layout: 7,
                        edition_id: 2
                    },
                    ScanCandidate {
                        dongle_id: 188,
                        keyboard_layout: 5,
                        edition_id: 9
                    }
                ]
            })
        );
        assert_eq!(
            notification(&[9, 54, 2]).unwrap(),
            Some(Notification::Pair { status: 2 })
        );
        assert_eq!(notification(&[0, 5, 54, 2]).unwrap(), None);
        assert_eq!(notification(&[5, 9, 3]).unwrap(), None);
        for malformed in [
            &[][..],
            &[5],
            &[5, 54],
            &[5, 55, 2],
            &[5, 55, 2, 4],
            &[5, 55, 2, 1, 0, 183],
        ] {
            assert!(notification(malformed).is_err());
        }
    }

    #[test]
    fn pair_ack_waits_for_real_success_and_releases_subscription() {
        let cap = cap(241, "KEYBOARD");
        let device = Mock::new(&[&[1]]);
        let mut events = Events::new(&[&[5, 55, 2, 0], &[5, 54, 1], &[9, 54, 2]]);
        let result = Session::new(&device, &mut events, &cap)
            .unwrap()
            .pair(183, 1, deadline(), || Ok(()), || Ok(()))
            .unwrap();
        assert_eq!(result.hardware_status, 2);
        assert_eq!(result.dongle_id, 183);
        assert!(events.began && events.ended);
        assert_eq!(events.reads, 3);
        let sent = device.sent.lock().unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(&sent[0][6..12], &[3, 0, 65, 1, 0, 183]);
        assert_eq!(sent[0][2], 128);
        assert_eq!(sent[0][89], sent[0][3..89].iter().fold(0, |sum, b| sum ^ b));
    }

    #[test]
    fn scan_ack_end_is_not_scan_done_and_candidates_require_event55() {
        let cap = cap(164, "MOUSE");
        let device = Mock::new(&[&[2]]);
        let mut events = Events::new(&[&[5, 55, 1, 0], &[5, 54, 2], &[5, 55, 2, 1, 0, 183, 7, 2]]);
        let result = Session::new(&device, &mut events, &cap)
            .unwrap()
            .scan(deadline(), || Ok(()), || Ok(()))
            .unwrap();
        assert_eq!(result[0].dongle_id, 183);
        assert_eq!(events.reads, 3);
        assert!(events.ended);
        assert_eq!(device.sent.lock().unwrap()[0][2], 0);
    }

    #[test]
    fn command_ack_alone_fails_and_sends_cancel_before_release() {
        let cap = cap(241, "MOUSE");
        let device = Mock::new(&[&[1], &[4]]);
        let mut events = Events::new(&[]);
        assert!(
            Session::new(&device, &mut events, &cap)
                .unwrap()
                .pair(183, 1, deadline(), || Ok(()), || Ok(()))
                .is_err()
        );
        assert!(events.ended);
        let sent = device.sent.lock().unwrap();
        assert_eq!(sent.len(), 2);
        assert_eq!(&sent[1][6..10], &[1, 0, 70, 4]);
    }

    #[test]
    fn keyboard_cancel_uses_primary_linker_not_keyboard_scan_namespace() {
        let cap = cap(241, "KEYBOARD");
        let device = Mock::new(&[&[1], &[4]]);
        let mut events = Events::new(&[]);
        assert!(
            Session::new(&device, &mut events, &cap)
                .unwrap()
                .scan(deadline(), || Ok(()), || Ok(()))
                .is_err()
        );
        let sent = device.sent.lock().unwrap();
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0][2], 128);
        assert_eq!(sent[1][2], 1);
        assert_eq!(&sent[1][6..10], &[1, 0, 70, 4]);
        assert!(events.ended);
    }

    #[test]
    fn post_pair_factories_read_edition_and_sidepad_in_original_namespaces() {
        for (pid, category, prefix) in [
            (164, "MOUSE", 0),
            (164, "KEYBOARD", 0),
            (241, "MOUSE", 0),
            (241, "KEYBOARD", 128),
        ] {
            let cap = cap(pid, category);
            let device = Mock::new(&[&[0, 128, 42], &[6]]);
            let mut events = Events::new(&[]);
            let mut session = Session::new(&device, &mut events, &cap).unwrap();
            assert_eq!(
                session.get_edition(|| Ok(())).unwrap(),
                EditionReading {
                    keyboard_layout: 0,
                    edition_id: 128,
                    firmware_id: Some(42)
                }
            );
            assert_eq!(session.get_mouse_layout(|| Ok(())).unwrap(), 6);
            let sent = device.sent.lock().unwrap();
            assert_eq!(sent.len(), 2);
            assert_eq!(sent[0][2], prefix);
            assert_eq!(&sent[0][6..9], &[3, 0, 134]);
            assert_eq!(sent[1][2], prefix | 1);
            assert_eq!(&sent[1][6..9], &[1, 0, 185]);
            assert!(!events.began);
        }
    }

    #[test]
    fn hardware_pair_timeout_cancels_and_cleanup_identity_failure_prevents_send() {
        let cap = cap(164, "MOUSE");
        let device = Mock::new(&[&[1]]);
        let mut events = Events::new(&[&[5, 54, 3]]);
        assert!(
            Session::new(&device, &mut events, &cap)
                .unwrap()
                .pair(
                    183,
                    1,
                    deadline(),
                    || Ok(()),
                    || anyhow::bail!("identity changed")
                )
                .is_err()
        );
        assert_eq!(device.sent.lock().unwrap().len(), 1);
        assert!(events.ended);
    }

    #[test]
    fn all_four_current_routes_use_unpair_prefix_and_return_only_ack() {
        for (pid, category, prefix) in [
            (164, "MOUSE", 224),
            (164, "KEYBOARD", 224),
            (241, "MOUSE", 128),
            (241, "KEYBOARD", 0),
        ] {
            let cap = cap(pid, category);
            let device = Mock::new(&[&[]]);
            let mut events = Events::new(&[]);
            let result = Session::new(&device, &mut events, &cap)
                .unwrap()
                .unpair(183, || Ok(()))
                .unwrap();
            assert_eq!(result.dongle_id, 183);
            assert!(!events.began);
            let sent = device.sent.lock().unwrap();
            assert_eq!(sent[0][2], prefix);
            assert_eq!(&sent[0][6..11], &[2, 0, 66, 0, 183]);
        }
    }

    #[test]
    fn invalid_pair_or_expired_deadline_never_registers_or_sends() {
        let cap = cap(164, "MOUSE");
        let device = Mock::new(&[]);
        let mut events = Events::new(&[]);
        {
            let mut session = Session::new(&device, &mut events, &cap).unwrap();
            for (pid, mode) in [(0, 1), (65535, 1), (183, 0), (183, 4)] {
                assert!(
                    session
                        .pair(pid, mode, deadline(), || Ok(()), || Ok(()))
                        .is_err()
                );
            }
            assert!(
                session
                    .scan(
                        Instant::now() - Duration::from_secs(1),
                        || Ok(()),
                        || Ok(())
                    )
                    .is_err()
            );
        }
        assert!(!events.began);
        assert!(device.sent.lock().unwrap().is_empty());
    }
}
