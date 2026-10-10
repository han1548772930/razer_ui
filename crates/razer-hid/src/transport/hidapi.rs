//! Feature/Output/control-Input/interrupt backend. No vendor binary or direct Win32 FFI.
use anyhow::{Context as _, ensure};
use hidapi::{DeviceInfo, HidApi, HidDevice};
use razer_device::backend::{FeatureTransport, HidBackend, HidNode, ReportLengths};
use serde_json::{Value, json};
use std::ffi::CString;
use std::fs::{File, OpenOptions};

pub struct NativeBackend {
    api: HidApi,
}

impl NativeBackend {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            api: HidApi::new().context("初始化跨平台 HID 后端失败")?,
        })
    }
}

fn node(info: &DeviceInfo) -> HidNode {
    HidNode {
        path: info.path().to_bytes().to_vec(),
        vendor_id: info.vendor_id(),
        product_id: info.product_id(),
        usage_page: info.usage_page(),
        usage: info.usage(),
        interface_number: info.interface_number(),
        release_number: info.release_number(),
        serial_number: info.serial_number().map(str::to_owned),
        manufacturer: info.manufacturer_string().map(str::to_owned),
        product: info.product_string().map(str::to_owned),
    }
}

impl HidBackend for NativeBackend {
    fn enumerate(&mut self) -> anyhow::Result<Vec<HidNode>> {
        self.api.refresh_devices().context("刷新 HID 节点失败")?;
        Ok(self.api.device_list().map(node).collect())
    }

    fn open(&mut self, selected: &HidNode) -> anyhow::Result<Box<dyn FeatureTransport>> {
        ensure!(!selected.path.is_empty(), "HID 节点路径为空");
        let path = CString::new(selected.path.clone()).context("HID 节点路径包含 NUL")?;
        // Application coordination only; not an original firmware command.
        // Hold the OS file lock for the whole handle lifetime, across workers.
        let directory = std::env::temp_dir().join("razer-ui-hid-query-locks");
        std::fs::create_dir_all(&directory).context("创建 HID 查询锁目录失败")?;
        let lock_path = directory.join(format!("{:x}.lock", md5::compute(&selected.path)));
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(lock_path)
            .context("打开 HID 查询锁失败")?;
        lock.try_lock()
            .context("HID 节点正被其他查询使用或查询锁不可用")?;
        let current = self.enumerate()?;
        ensure!(
            current.iter().filter(|item| *item == selected).count() == 1,
            "HID 节点身份已变化或 collection 不唯一"
        );
        // Never use VID/PID or serial fallback: retain the selected collection.
        let device = self.api.open_path(&path).context("打开 HID 节点失败")?;
        // Some OS paths identify a device shared by several collections. hidapi
        // must expose the exact selected collection on the opened handle too.
        let opened = node(
            &device
                .get_device_info()
                .context("读取已打开 HID 节点失败")?,
        );
        ensure!(
            opened == *selected,
            "已打开 HID collection 与选择的节点不一致"
        );
        let mut descriptor = [0u8; 4096];
        let count = device
            .get_report_descriptor(&mut descriptor)
            .context("读取 HID Report Descriptor 失败")?;
        ensure!(
            count > 0 && count <= descriptor.len(),
            "HID Report Descriptor 长度无效"
        );
        let lengths = super::descriptor::report_lengths(&descriptor[..count])?;
        Ok(Box::new(NativeTransport {
            device,
            lengths,
            _lock: lock,
        }))
    }
}

struct NativeTransport {
    device: HidDevice,
    lengths: ReportLengths,
    _lock: File,
}

// HIDP_CAPS uses the collection maximum report length; Linux/macOS can also
// return the selected ID's exact length. Both must come from the descriptor.
fn valid_io_length(lengths: &std::collections::BTreeMap<u8, usize>, id: u8, size: usize) -> bool {
    lengths
        .get(&id)
        .is_some_and(|observed| size == *observed || lengths.values().max().copied() == Some(size))
}

impl FeatureTransport for NativeTransport {
    fn send_feature(&self, report: &[u8]) -> anyhow::Result<()> {
        ensure!(!report.is_empty(), "Feature 查询报文为空");
        ensure!(
            self.lengths.feature.get(&report[0]) == Some(&report.len()),
            "Feature 报文与实际 descriptor 的 Report ID/长度不一致"
        );
        self.device
            .send_feature_report(report)
            .context("HID Feature 查询发送失败")
    }

    fn get_feature(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        ensure!(!report.is_empty(), "Feature 响应缓冲区为空");
        ensure!(
            self.lengths.feature.get(&report[0]) == Some(&report.len()),
            "Feature 响应与实际 descriptor 的 Report ID/长度不一致"
        );
        let count = self
            .device
            .get_feature_report(report)
            .context("HID Feature 响应读取失败")?;
        ensure!(count <= report.len(), "HID 返回字节数超过缓冲区长度");
        Ok(count)
    }

    fn write_output(&self, report: &[u8]) -> anyhow::Result<()> {
        ensure!(!report.is_empty(), "Output Report 为空");
        ensure!(
            valid_io_length(&self.lengths.output, report[0], report.len()),
            "Output Report 与实际 descriptor 的 ID/长度不一致"
        );
        let sent = self
            .device
            .write(report)
            .context("HID Output Report 发送失败")?;
        ensure!(sent == report.len(), "HID Output Report 未完整发送");
        Ok(())
    }

    fn get_input(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        ensure!(!report.is_empty(), "Input Report 缓冲区为空");
        ensure!(
            valid_io_length(&self.lengths.input, report[0], report.len()),
            "Input Report 与实际 descriptor 的 ID/长度不一致"
        );
        let count = self
            .device
            .get_input_report(report)
            .context("HID 控制 Input Report 读取失败")?;
        ensure!(count <= report.len(), "HID Input Report 超过缓冲区长度");
        Ok(count)
    }

    fn report_lengths(&self) -> anyhow::Result<ReportLengths> {
        Ok(self.lengths.clone())
    }

    fn read_interrupt(&self, report: &mut [u8], timeout_ms: u32) -> anyhow::Result<usize> {
        let timeout = i32::try_from(timeout_ms).context("Interrupt 等待时间超出有限超时范围")?;
        let maximum = self
            .lengths
            .input
            .values()
            .max()
            .copied()
            .context("此 collection 未声明 Input Report")?;
        // Our descriptor lengths include the API ID slot even for ID zero.
        // hid_read_timeout omits that slot for unnumbered interrupt reports.
        let unnumbered = self.lengths.input.contains_key(&0);
        ensure!(
            !unnumbered || self.lengths.input.len() == 1,
            "此 collection 混合无编号与编号 Input Report，无法确定事件格式"
        );
        let raw_maximum = maximum - usize::from(unnumbered);
        ensure!(
            raw_maximum > 0 && report.len() >= raw_maximum,
            "Interrupt 缓冲区小于 descriptor 声明的最大报告"
        );
        let count = self
            .device
            .read_timeout(report, timeout)
            .context("HID Interrupt Input Report 读取失败")?;
        ensure!(count <= report.len(), "Interrupt 实际字节数超出缓冲区");
        if count == 0 {
            return Ok(0);
        }
        let id = if unnumbered { 0 } else { report[0] };
        let api_count = count + usize::from(unnumbered);
        ensure!(
            valid_io_length(&self.lengths.input, id, api_count),
            "Interrupt Report 的实际 ID/长度与 descriptor 不一致"
        );
        Ok(count)
    }

    fn metadata(&self) -> Value {
        json!({"transport":"hidapi", "transport_version":"2.6.7",
            "transport_platform":std::env::consts::OS, "vendor_hid_module_loaded":false})
    }
}
