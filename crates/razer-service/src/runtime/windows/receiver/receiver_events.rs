//! Current mapping_engine selector 0x119de6 / reader 0x1232f8 reconstructed
//! with portable interrupt reads. Windows supplies physical ContainerId only.
use anyhow::{Context as _, ensure};
use razer_device::{
    backend::{HidBackend, InterruptTransport},
    receiver_pairing::HardwareEvents,
};
use razer_hid::with_backend;
use serde_json::Value;
use std::time::{Duration, Instant};

pub(super) struct Events {
    product_id: u16,
    container: String,
    retained: Vec<(String, String)>,
    readers: Vec<Box<dyn InterruptTransport>>,
    cursor: usize,
}

impl Events {
    pub(super) fn new(product_id: u16, container: &str) -> Self {
        Self {
            product_id,
            container: container.to_owned(),
            retained: Vec::new(),
            readers: Vec::new(),
            cursor: 0,
        }
    }

    fn paths(&self) -> anyhow::Result<Vec<(String, String)>> {
        let observation = super::hid::enumerate_for_product(0x1532, self.product_id)?;
        ensure!(observation["complete"] == true, "硬件事件接口枚举未完成");
        let mut selected = observation["interfaces"]
            .as_array()
            .context("缺少硬件事件接口观察")?
            .iter()
            // Native selector matches ContainerId and PID across every HID
            // collection, without a Feature interface or usage-page predicate.
            .filter(|item| {
                item["device_container_id"]
                    .as_str()
                    .is_some_and(|id| id.eq_ignore_ascii_case(&self.container))
            })
            .map(|item| {
                Ok((
                    item["path"]
                        .as_str()
                        .context("事件接口缺少路径")?
                        .to_owned(),
                    item["device_instance_id"]
                        .as_str()
                        .filter(|value| !value.is_empty())
                        .context("事件接口缺少真实实例 ID")?
                        .to_owned(),
                ))
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        selected.sort_by_key(|(path, _)| path.to_ascii_lowercase());
        ensure!(!selected.is_empty(), "原始物理接收器没有可订阅的 HID 集合");
        ensure!(
            selected
                .windows(2)
                .all(|pair| !pair[0].0.eq_ignore_ascii_case(&pair[1].0)),
            "事件接口路径不唯一"
        );
        Ok(selected)
    }

    pub(super) fn validate(&self) -> anyhow::Result<()> {
        let current = self.paths()?;
        ensure!(
            current.len() == self.retained.len()
                && current
                    .iter()
                    .zip(&self.retained)
                    .all(|(a, b)| a.0.eq_ignore_ascii_case(&b.0) && a.1.eq_ignore_ascii_case(&b.1)),
            "配对期间硬件事件集合或物理实例发生变化"
        );
        Ok(())
    }

    pub(super) fn metadata(&self) -> Value {
        serde_json::json!({"provider":"direct_hid_interrupt", "selection":"mapping_engine_container_id_product_id_all_collections", "collections":self.retained.iter().map(|(path,instance)|serde_json::json!({"path":path,"device_instance_id":instance})).collect::<Vec<_>>(), "native_read_capacity":100})
    }

    fn receive_pending(&mut self) -> anyhow::Result<Option<Vec<u8>>> {
        for _ in 0..self.readers.len() {
            let reader = &self.readers[self.cursor];
            self.cursor = (self.cursor + 1) % self.readers.len();
            let mut data = [0u8; 100];
            let count = reader.read(&mut data, 0)?;
            ensure!(count <= data.len(), "硬件事件实际字节数超出读取缓冲区");
            if count > 0 {
                // OnInputReport/serializer pass every actual byte. No
                // synthetic ReportID or Feature response substitution.
                return Ok(Some(data[..count].to_vec()));
            }
        }
        Ok(None)
    }
}

fn open_readers(
    backend: &mut impl HidBackend,
    product_id: u16,
    selected: &[(String, String)],
) -> anyhow::Result<Vec<Box<dyn InterruptTransport>>> {
    let nodes = backend.enumerate()?;
    let mut readers = Vec::new();
    for (path, _) in selected {
        ensure!(
            path.is_ascii() && !path.contains('\0'),
            "事件路径无法由当前身份适配器保真转换"
        );
        let exact: Vec<_> = nodes
            .iter()
            .filter(|node| {
                node.path.eq_ignore_ascii_case(path.as_bytes())
                    && node.vendor_id == 0x1532
                    && node.product_id == product_id
            })
            .collect();
        ensure!(exact.len() == 1, "事件路径未对应唯一的实际 HID 集合");
        // A long-lived Feature handle owns the control transaction lock.
        // Interrupt-only handles leave initial queries and later writes free.
        let reader = backend.open_interrupt(exact[0])?;
        let lengths = reader.report_lengths();
        // A collection without Input descriptors cannot produce an
        // interrupt. It remains in retained identity observation.
        if let Some(maximum) = lengths.input.values().max() {
            let unnumbered = lengths.input.contains_key(&0);
            ensure!(
                !unnumbered || lengths.input.len() == 1,
                "事件集合混合无编号与编号 Input Report"
            );
            let raw_length = maximum
                .checked_sub(usize::from(unnumbered))
                .context("事件报告长度无效")?;
            ensure!(
                raw_length > 0 && raw_length <= 100,
                "事件报告超出原生 100 字节读取容量"
            );
            readers.push(reader);
        }
    }
    ensure!(!readers.is_empty(), "实际 HID 集合没有 Input Report");
    Ok(readers)
}

impl HardwareEvents for Events {
    fn begin(&mut self) -> anyhow::Result<()> {
        ensure!(
            self.readers.is_empty() && self.retained.is_empty(),
            "硬件事件订阅已经开始"
        );
        let selected = self.paths()?;
        let readers = with_backend(|backend| open_readers(backend, self.product_id, &selected))?;
        self.retained = selected;
        self.readers = readers;
        self.validate()?;
        Ok(())
    }

    fn receive(&mut self, timeout: Duration) -> anyhow::Result<Option<Vec<u8>>> {
        ensure!(!self.readers.is_empty(), "硬件事件订阅未开始");
        self.validate()?;
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(data) = self.receive_pending()? {
                return Ok(Some(data));
            }
            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return Ok(None);
            };
            std::thread::sleep(remaining.min(Duration::from_millis(2)));
        }
    }

    fn finish(&mut self) -> anyhow::Result<()> {
        self.readers.clear();
        self.cursor = 0;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use razer_device::backend::{FeatureTransport, HidNode, ReportLengths};
    use std::{cell::Cell, collections::VecDeque, rc::Rc};

    fn node(path: &str) -> HidNode {
        HidNode {
            path: path.as_bytes().to_vec(),
            vendor_id: 0x1532,
            product_id: 653,
            usage_page: 1,
            usage: 6,
            interface_number: 3,
            release_number: 1,
            serial_number: None,
            manufacturer: None,
            product: None,
        }
    }

    enum Report {
        Bytes(Vec<u8>),
        ExcessiveCount,
    }

    struct Interrupt {
        lengths: ReportLengths,
        reports: Rc<std::cell::RefCell<VecDeque<Report>>>,
        active: Rc<Cell<usize>>,
    }

    impl InterruptTransport for Interrupt {
        fn report_lengths(&self) -> ReportLengths {
            self.lengths.clone()
        }

        fn read(&self, report: &mut [u8], timeout_ms: u32) -> anyhow::Result<usize> {
            assert_eq!(timeout_ms, 0);
            assert_eq!(report.len(), 100);
            match self.reports.borrow_mut().pop_front() {
                Some(Report::Bytes(bytes)) => {
                    report[..bytes.len()].copy_from_slice(&bytes);
                    Ok(bytes.len())
                }
                Some(Report::ExcessiveCount) => Ok(report.len() + 1),
                None => Ok(0),
            }
        }
    }

    impl Drop for Interrupt {
        fn drop(&mut self) {
            self.active.set(self.active.get() - 1);
        }
    }

    struct Feature(Rc<Cell<bool>>);

    impl FeatureTransport for Feature {
        fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
            anyhow::bail!("mock does not fabricate a device acknowledgement")
        }

        fn get_feature(&self, _: &mut [u8]) -> anyhow::Result<usize> {
            anyhow::bail!("mock does not fabricate a device observation")
        }

        fn metadata(&self) -> Value {
            serde_json::json!({"transport":"mock"})
        }
    }

    impl Drop for Feature {
        fn drop(&mut self) {
            self.0.set(false);
        }
    }

    struct Backend {
        nodes: Vec<HidNode>,
        lengths: ReportLengths,
        reports: Rc<std::cell::RefCell<VecDeque<Report>>>,
        active: Rc<Cell<usize>>,
        interrupt_opens: usize,
        feature_opens: usize,
        feature_busy: Rc<Cell<bool>>,
    }

    impl Backend {
        fn new(input: &[(u8, usize)]) -> Self {
            Self {
                nodes: vec![node("event-path"), node("feature-only-path")],
                lengths: ReportLengths {
                    input: input.iter().copied().collect(),
                    ..Default::default()
                },
                reports: Default::default(),
                active: Default::default(),
                interrupt_opens: 0,
                feature_opens: 0,
                feature_busy: Default::default(),
            }
        }

        fn selected(&self) -> Vec<(String, String)> {
            self.nodes
                .iter()
                .map(|node| {
                    (
                        String::from_utf8(node.path.to_ascii_uppercase()).unwrap(),
                        "retained-instance".to_owned(),
                    )
                })
                .collect()
        }
    }

    impl HidBackend for Backend {
        fn enumerate(&mut self) -> anyhow::Result<Vec<HidNode>> {
            Ok(self.nodes.clone())
        }

        fn open_interrupt(
            &mut self,
            node: &HidNode,
        ) -> anyhow::Result<Box<dyn InterruptTransport>> {
            self.interrupt_opens += 1;
            self.active.set(self.active.get() + 1);
            Ok(Box::new(Interrupt {
                lengths: if node.path == b"event-path" {
                    self.lengths.clone()
                } else {
                    ReportLengths::default()
                },
                reports: self.reports.clone(),
                active: self.active.clone(),
            }))
        }

        fn open(&mut self, _: &HidNode) -> anyhow::Result<Box<dyn FeatureTransport>> {
            ensure!(
                !self.feature_busy.get(),
                "mock Feature transaction lock is held"
            );
            self.feature_busy.set(true);
            self.feature_opens += 1;
            Ok(Box::new(Feature(self.feature_busy.clone())))
        }
    }

    #[test]
    fn retained_events_allow_feature_queries_and_release_all_interrupt_handles() {
        let mut backend = Backend::new(&[(0, 101)]);
        let selected = backend.selected();
        let readers = open_readers(&mut backend, 653, &selected).unwrap();
        assert_eq!(backend.feature_opens, 0);
        assert_eq!(backend.interrupt_opens, 2);
        assert_eq!(backend.active.get(), 1);
        let control_node = backend.nodes[0].clone();
        let feature = backend.open(&control_node).unwrap();
        assert!(backend.feature_busy.get());
        drop(feature);
        assert!(!backend.feature_busy.get());

        let mut events = Events::new(653, "physical-container");
        events.retained = selected;
        events.readers = readers;
        events.finish().unwrap();
        assert_eq!(backend.active.get(), 0);
        assert_eq!(events.retained.len(), 2);
        assert_eq!(events.cursor, 0);
    }

    #[test]
    fn events_pass_actual_bytes_and_never_synthesize_a_report_id_or_timeout_event() {
        for (input, bytes) in [
            (vec![(0, 4)], vec![5, 14, 2]),
            (vec![(7, 4)], vec![7, 5, 14, 0]),
        ] {
            let mut backend = Backend::new(&input);
            backend
                .reports
                .borrow_mut()
                .push_back(Report::Bytes(bytes.clone()));
            let selected = backend.selected();
            let mut events = Events::new(653, "physical-container");
            events.readers = open_readers(&mut backend, 653, &selected).unwrap();
            assert_eq!(events.receive_pending().unwrap(), Some(bytes));
            assert_eq!(events.receive_pending().unwrap(), None);
        }
    }

    #[test]
    fn events_reject_invalid_input_capacity_and_drop_partial_subscriptions() {
        for input in [
            vec![],
            vec![(0, 0)],
            vec![(0, 1)],
            vec![(0, 102)],
            vec![(5, 101)],
            vec![(0, 4), (5, 4)],
        ] {
            let mut backend = Backend::new(&input);
            let selected = backend.selected();
            assert!(open_readers(&mut backend, 653, &selected).is_err());
            assert_eq!(backend.active.get(), 0);
            assert_eq!(backend.feature_opens, 0);
        }
    }

    #[test]
    fn events_reject_missing_ambiguous_or_unrepresentable_selected_collections() {
        for path in ["missing", "non-ascii-é", "event\0path"] {
            let mut backend = Backend::new(&[(0, 4)]);
            let selected = vec![(path.to_owned(), "instance".to_owned())];
            assert!(open_readers(&mut backend, 653, &selected).is_err());
            assert_eq!(backend.interrupt_opens, 0);
        }
        let mut backend = Backend::new(&[(0, 4)]);
        let selected = backend.selected();
        backend.nodes[0].product_id = 654;
        assert!(open_readers(&mut backend, 653, &selected).is_err());
        assert_eq!(backend.interrupt_opens, 0);
        backend.nodes[0].product_id = 653;
        backend.nodes.push(backend.nodes[0].clone());
        assert!(open_readers(&mut backend, 653, &selected).is_err());
        assert_eq!(backend.interrupt_opens, 0);
    }

    #[test]
    fn events_reject_a_transport_byte_count_beyond_the_native_buffer() {
        let mut backend = Backend::new(&[(0, 4)]);
        backend
            .reports
            .borrow_mut()
            .push_back(Report::ExcessiveCount);
        let selected = backend.selected();
        let mut events = Events::new(653, "physical-container");
        events.readers = open_readers(&mut backend, 653, &selected).unwrap();
        assert!(events.receive_pending().is_err());
    }
}
