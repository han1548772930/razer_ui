//! Current mapping_engine selector 0x119de6 / reader 0x1232f8 reconstructed
//! with portable interrupt reads. Windows supplies physical ContainerId only.
use anyhow::{Context as _, ensure};
use razer_device::{
    backend::{FeatureTransport, HidBackend},
    receiver_pairing::HardwareEvents,
};
use razer_hid::with_backend;
use serde_json::Value;
use std::time::{Duration, Instant};

pub(super) struct Events {
    product_id: u16,
    container: String,
    retained: Vec<(String, String)>,
    readers: Vec<Box<dyn FeatureTransport>>,
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
}

impl HardwareEvents for Events {
    fn begin(&mut self) -> anyhow::Result<()> {
        ensure!(
            self.readers.is_empty() && self.retained.is_empty(),
            "硬件事件订阅已经开始"
        );
        let selected = self.paths()?;
        let readers = with_backend(|backend| {
            let nodes = backend.enumerate()?;
            let mut readers = Vec::new();
            for (path, _) in &selected {
                ensure!(
                    path.is_ascii() && !path.contains('\0'),
                    "事件路径无法由当前身份适配器保真转换"
                );
                let exact: Vec<_> = nodes
                    .iter()
                    .filter(|node| {
                        node.path.eq_ignore_ascii_case(path.as_bytes())
                            && node.vendor_id == 0x1532
                            && node.product_id == self.product_id
                    })
                    .collect();
                ensure!(exact.len() == 1, "事件路径未对应唯一的实际 HID 集合");
                let reader = backend.open(exact[0])?;
                let lengths = reader.report_lengths()?;
                // A collection without Input descriptors cannot produce an
                // interrupt. It remains in retained identity observation.
                if let Some(maximum) = lengths.input.values().max() {
                    let raw_length = maximum
                        .checked_sub(usize::from(lengths.input.contains_key(&0)))
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
        })?;
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
            for _ in 0..self.readers.len() {
                let reader = &self.readers[self.cursor];
                self.cursor = (self.cursor + 1) % self.readers.len();
                let mut data = [0u8; 100];
                let count = reader.read_interrupt(&mut data, 0)?;
                ensure!(count <= data.len(), "硬件事件实际字节数超出读取缓冲区");
                if count > 0 {
                    // OnInputReport/serializer pass every actual byte. No
                    // synthetic ReportID or Feature response substitution.
                    return Ok(Some(data[..count].to_vec()));
                }
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
