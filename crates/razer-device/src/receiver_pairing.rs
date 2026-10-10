//! Current 164/241 Scan/Pair/Unpair command transport and notification lifecycle.
//! Command acknowledgements never establish scan completion or pairing success.
//! A caller must provide a real, retained hardware event subscription before any
//! asynchronous command. Collection selection and profile/runtime publication
//! remain explicit caller obligations; this module never substitutes a query.
use super::{backend::FeatureTransport, receiver_capabilities::ReceiverCapability};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use std::{
    sync::OnceLock,
    time::{Duration, Instant},
};

const PAIR: [u8; 3] = [3, 0, 65];
const UNPAIR: [u8; 3] = [2, 0, 66];
const SCAN: [u8; 3] = [1, 0, 70];
const EDITION: [u8; 3] = [3, 0, 134];
const HARDWARE_MODULE: [u8; 3] = [1, 0, 185];

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum PairingAction {
    Scan,
    Pair { dongle_id: u16 },
    Unpair { dongle_id: u16 },
}

#[derive(Debug, Deserialize)]
pub struct PairingCapability {
    pub product_id: u16,
    pub category: String,
    pub scan_pair_prefix: u8,
    pub unpair_prefix: u8,
    pub cancel_prefix: u8,
    pub metadata_prefix: u8,
    pub transport: ReceiverCapability,
}

pub fn capability(product_id: u16, category: &str) -> Option<&'static PairingCapability> {
    #[derive(Deserialize)]
    struct Catalog {
        schema_version: u8,
        products: Vec<PairingCapability>,
    }
    static CATALOG: OnceLock<Catalog> = OnceLock::new();
    CATALOG
        .get_or_init(|| {
            let c: Catalog = serde_json::from_str(include_str!(
                "../../../assets/data/receiver-pairing-capabilities.json"
            ))
            .expect("current pairing command capabilities");
            assert_eq!(c.schema_version, 1);
            c
        })
        .products
        .iter()
        .find(|c| c.product_id == product_id && c.category == category)
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

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
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
    pub fn new(
        device: &'a dyn FeatureTransport,
        events: &'a mut dyn HardwareEvents,
        cap: &'a PairingCapability,
    ) -> anyhow::Result<Self> {
        ensure!(
            matches!(cap.product_id, 164 | 241)
                && cap.transport.product_id == cap.product_id
                && cap.transport.report_bytes == 91
                && cap.transport.report_id == 0
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
        outgoing[2] = transaction;
        outgoing[6..9].copy_from_slice(&command);
        outgoing[9..9 + payload.len()].copy_from_slice(payload);
        outgoing[89] = outgoing[3..89].iter().fold(0, |sum, byte| sum ^ byte);
        let transport = &self.cap.transport;
        let mut last_error = "没有收到设备应答".to_owned();
        for _ in 0..transport.max_retry_out {
            validate()?;
            std::thread::sleep(Duration::from_millis(transport.sleep_between_out_ms));
            validate()?;
            if let Err(e) = self.device.send_feature(&outgoing) {
                last_error = format!("{e:#}");
                continue;
            }
            validate()?;
            std::thread::sleep(Duration::from_millis(transport.sleep_between_out_in_ms));
            for attempt in 0..transport.max_retry_in {
                if attempt > 0 {
                    std::thread::sleep(Duration::from_millis(transport.sleep_between_in_ms));
                }
                validate()?;
                let mut incoming = vec![0; 91];
                let count = match self.device.get_feature(&mut incoming) {
                    Ok(n) => n,
                    Err(e) => {
                        last_error = format!("{e:#}");
                        break;
                    }
                };
                validate()?;
                ensure!(
                    count == 91 && incoming[0] == 0,
                    "配对命令应答长度或 ReportID 不匹配"
                );
                if incoming[2] != transaction || incoming[7..9] != command[1..] {
                    last_error = "配对命令或事务应答不匹配".into();
                    break;
                }
                match incoming[1] {
                    1 => {
                        last_error = "设备忙".into();
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
                ensure!(length <= 80, "配对应答长度超出协议");
                return Ok(incoming[9..9 + length].to_vec());
            }
        }
        anyhow::bail!("配对命令失败：{last_error}")
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
        let canceled = if result.is_err() && cancel_on_error {
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
    // Static-checkable pure fixtures; do not execute under current AGENTS rules.
    use super::*;
    use std::{collections::VecDeque, sync::Mutex};

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
            transport,
        }
    }
    fn deadline() -> Instant {
        Instant::now() + Duration::from_secs(1)
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
