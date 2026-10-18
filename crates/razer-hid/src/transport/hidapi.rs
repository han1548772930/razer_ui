//! Feature/Output/control-Input/interrupt backend. No vendor binary or direct Win32 FFI.
use anyhow::{Context as _, ensure};
use hidapi::{DeviceInfo, HidApi, HidDevice};
use razer_device::backend::{
    FeatureTransport, HidBackend, HidNode, InterruptTransport, ReportLengths,
};
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
    fn open_interrupt(
        &mut self,
        selected: &HidNode,
    ) -> anyhow::Result<Box<dyn InterruptTransport>> {
        // An interrupt-only handle cannot issue a control transaction. Holding
        // the Feature lock for its lifetime would block all mixer writes/meters.
        ensure!(
            self.enumerate()?.iter().filter(|n| *n == selected).count() == 1,
            "Interrupt collection identity changed"
        );
        let path = CString::new(selected.path.clone())?;
        let device = self.api.open_path(&path)?;
        ensure!(
            node(&device.get_device_info()?) == *selected,
            "Opened interrupt collection differs"
        );
        let mut descriptor = [0; 4096];
        let count = device.get_report_descriptor(&mut descriptor)?;
        ensure!(
            count > 0 && count <= descriptor.len(),
            "Interrupt descriptor invalid"
        );
        let lengths = super::descriptor::report_lengths(&descriptor[..count])?;
        Ok(Box::new(NativeInterrupt { device, lengths }))
    }

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
            node: opened,
            lengths,
            _lock: lock,
        }))
    }
}

struct NativeTransport {
    device: HidDevice,
    node: HidNode,
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

fn send_collection_feature(
    lengths: &ReportLengths,
    report: &[u8],
    send: impl FnOnce(&[u8]) -> anyhow::Result<()>,
) -> anyhow::Result<()> {
    ensure!(
        !report.is_empty() && lengths.feature.values().max().copied() == Some(report.len()),
        "Collection Feature send differs from its observed maximum length"
    );
    send(report)
}

fn write_counted_output(
    lengths: &ReportLengths,
    report: &[u8],
    write: impl FnOnce(&[u8]) -> anyhow::Result<usize>,
) -> anyhow::Result<usize> {
    ensure!(!report.is_empty(), "Output Report is empty");
    let minimum = lengths
        .output
        .values()
        .max()
        .copied()
        .filter(|length| *length > 0)
        .context("Collection has no observed Output Report")?;
    // Current HID.node hid_write pads to OutputReportByteLength and leaves a
    // larger caller buffer intact. Its return count is a separate observation.
    let padded;
    let bytes = if report.len() < minimum {
        padded = {
            let mut bytes = vec![0; minimum];
            bytes[..report.len()].copy_from_slice(report);
            bytes
        };
        padded.as_slice()
    } else {
        report
    };
    let count = write(bytes)?;
    ensure!(
        count <= bytes.len(),
        "Output count exceeds the submitted buffer"
    );
    Ok(count)
}

fn read_collection_feature(
    lengths: &ReportLengths,
    report: &mut [u8],
    windows: bool,
    read: impl FnOnce(&mut [u8]) -> anyhow::Result<usize>,
) -> anyhow::Result<usize> {
    ensure!(
        !report.is_empty() && lengths.feature.values().max().copied() == Some(report.len()),
        "Collection Feature buffer differs from its observed maximum length"
    );
    let count = read(report)?;
    source_control_report_count(report, count, windows)
}

fn read_input_report(
    lengths: &ReportLengths,
    report: &mut [u8],
    windows: bool,
    read: impl FnOnce(&mut [u8]) -> anyhow::Result<usize>,
) -> anyhow::Result<usize> {
    ensure!(
        !report.is_empty() && valid_io_length(&lengths.input, report[0], report.len()),
        "Input report differs from its observed ID and length"
    );
    let count = read(report)?;
    source_control_report_count(report, count, windows)
}

fn source_control_report_count(
    report: &[u8],
    count: usize,
    windows: bool,
) -> anyhow::Result<usize> {
    // Locked hidapi 2.6.7 Windows C uses the same IOCTL/OVERLAPPED sequence as
    // current HID.node, but adds the prefix only when returned data[0] is zero.
    // HID.node's Feature 0x17ac0 and Input 0x17c50 return BytesReturned+1.
    let count = if windows && report[0] != 0 {
        count
            .checked_add(1)
            .context("Source control report count overflow")?
    } else {
        count
    };
    ensure!(
        count <= report.len(),
        "Source control report count exceeds its buffer"
    );
    Ok(count)
}

impl FeatureTransport for NativeTransport {
    fn hid_node(&self) -> Option<&HidNode> {
        Some(&self.node)
    }

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

    fn send_feature_at_collection_max(&self, report: &[u8]) -> anyhow::Result<()> {
        send_collection_feature(&self.lengths, report, |buffer| {
            self.device
                .send_feature_report(buffer)
                .context("HID collection Feature send failed")
        })
    }

    fn get_feature_at_collection_max(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        read_collection_feature(&self.lengths, report, cfg!(windows), |buffer| {
            self.device
                .get_feature_report(buffer)
                .context("HID collection Feature response failed")
        })
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

    fn write_output_counted(&self, report: &[u8]) -> anyhow::Result<usize> {
        write_counted_output(&self.lengths, report, |bytes| {
            self.device
                .write(bytes)
                .context("HID counted Output Report write failed")
        })
    }

    fn get_input(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        read_input_report(&self.lengths, report, cfg!(windows), |buffer| {
            self.device
                .get_input_report(buffer)
                .context("HID 控制 Input Report 读取失败")
        })
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

struct NativeInterrupt {
    device: HidDevice,
    lengths: ReportLengths,
}

fn read_interrupt_report(
    lengths: &ReportLengths,
    report: &mut [u8],
    timeout_ms: u32,
    read: impl FnOnce(&mut [u8], i32) -> anyhow::Result<usize>,
) -> anyhow::Result<usize> {
    let timeout = i32::try_from(timeout_ms).context("Interrupt 等待时间超出有限超时范围")?;
    let maximum = lengths
        .input
        .values()
        .max()
        .copied()
        .context("此 collection 未声明 Input Report")?;
    // Descriptor lengths include the API ID slot for ID zero; hidapi's
    // interrupt read returns only the real bytes of an unnumbered report.
    let unnumbered = lengths.input.contains_key(&0);
    ensure!(
        !unnumbered || lengths.input.len() == 1,
        "此 collection 混合无编号与编号 Input Report，无法确定事件格式"
    );
    let raw_maximum = maximum
        .checked_sub(usize::from(unnumbered))
        .context("Interrupt descriptor 报告长度无效")?;
    ensure!(
        raw_maximum > 0 && report.len() >= raw_maximum,
        "Interrupt 缓冲区小于 descriptor 声明的最大报告"
    );
    let count = read(report, timeout)?;
    ensure!(count <= report.len(), "Interrupt 实际字节数超出缓冲区");
    if count == 0 {
        return Ok(0);
    }
    let id = if unnumbered { 0 } else { report[0] };
    let api_count = count + usize::from(unnumbered);
    ensure!(
        valid_io_length(&lengths.input, id, api_count),
        "Interrupt Report 的实际 ID/长度与 descriptor 不一致"
    );
    Ok(count)
}

impl InterruptTransport for NativeInterrupt {
    fn report_lengths(&self) -> ReportLengths {
        self.lengths.clone()
    }
    fn read(&self, report: &mut [u8], timeout_ms: u32) -> anyhow::Result<usize> {
        read_interrupt_report(&self.lengths, report, timeout_ms, |data, timeout| {
            self.device
                .read_timeout(data, timeout)
                .context("HID Interrupt Input Report 读取失败")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counted_output_preserves_elsa_bytes_and_native_length_disagreement() {
        let observed = ReportLengths {
            output: [(0, 65)].into(),
            ..Default::default()
        };
        let mut report = [0; 66];
        report[..6].copy_from_slice(&[0, 32, 1, 0, 0, 0]);
        report[65] = 0x7a;
        for native_count in [0, 64, 65, 66] {
            let count = write_counted_output(&observed, &report, |sent| {
                assert_eq!(sent, report);
                Ok(native_count)
            })
            .unwrap();
            assert_eq!(count, native_count);
        }
    }

    #[test]
    fn counted_output_pads_only_the_collection_minimum_and_retains_failures() {
        let observed = ReportLengths {
            output: [(0, 65), (4, 33)].into(),
            ..Default::default()
        };
        let count = write_counted_output(&observed, &[4, 10, 20], |sent| {
            assert_eq!(sent.len(), 65);
            assert_eq!(&sent[..3], &[4, 10, 20]);
            assert!(sent[3..].iter().all(|byte| *byte == 0));
            Ok(65)
        })
        .unwrap();
        assert_eq!(count, 65);
        assert!(write_counted_output(&observed, &[0; 66], |_| Ok(67)).is_err());
        assert!(
            write_counted_output(&observed, &[0; 66], |_| anyhow::bail!(
                "native write failed"
            ))
            .is_err()
        );
        for invalid in [
            ReportLengths::default(),
            ReportLengths {
                output: [(0, 0)].into(),
                ..Default::default()
            },
        ] {
            assert!(
                write_counted_output(&invalid, &[0; 66], |_| panic!(
                    "invalid collection must not write"
                ))
                .is_err()
            );
        }
        assert!(
            write_counted_output(&observed, &[], |_| panic!("empty output must not write"))
                .is_err()
        );
    }

    #[test]
    fn collection_feature_preserves_host_id_and_windows_native_prefix_count() {
        let observed = ReportLengths {
            feature: [(0, 91)].into_iter().collect(),
            ..Default::default()
        };
        for (windows, id, count, expected) in
            [(true, 3, 90, 91), (true, 0, 91, 91), (false, 3, 91, 91)]
        {
            let mut report = [0; 91];
            report[0] = 3;
            let result = read_collection_feature(&observed, &mut report, windows, |buffer| {
                assert_eq!(buffer[0], 3);
                buffer[0] = id;
                buffer[1] = 2;
                Ok(count)
            })
            .unwrap();
            assert_eq!(result, expected);
            assert_eq!(report[0], id);
            assert_eq!(report[1], 2);
        }
        let mut report = [3; 91];
        assert!(read_collection_feature(&observed, &mut report, true, |_| Ok(91)).is_err());
        let mut report = [3; 90];
        assert!(
            read_collection_feature(&observed, &mut report, true, |_| panic!(
                "wrong collection size must not read"
            ))
            .is_err()
        );
    }

    #[test]
    fn mixer_feature_get18_keeps_native_count_on_a_feature7_collection() {
        let observed = ReportLengths {
            feature: [(7, 64)].into(),
            ..Default::default()
        };
        assert!(!observed.feature.contains_key(&18));
        let mut report = [0; 64];
        report[0] = 18;
        let count = read_collection_feature(&observed, &mut report, true, |buffer| {
            assert_eq!(buffer[0], 18);
            buffer[0..4].copy_from_slice(&[18, 10, 20, 30]);
            Ok(63)
        })
        .unwrap();
        assert_eq!(count, 64);
        assert_eq!(report[..4], [18, 10, 20, 30]);

        // Preserve a short native result; the protocol owner decides whether
        // that particular source task requires a complete payload.
        report[0] = 18;
        let count = read_collection_feature(&observed, &mut report, true, |_| Ok(2)).unwrap();
        assert_eq!(count, 3);
        report[0] = 18;
        assert!(read_collection_feature(&observed, &mut report, true, |_| Ok(64)).is_err());
    }

    #[test]
    fn mixer_input_keeps_source_native_count_for_numbered_and_unnumbered_reports() {
        for (selector, windows, native_count, expected_count) in [
            (18, true, 15, 16),
            (9, true, 7, 8),
            (18, true, 2, 3),
            (18, true, 0, 1),
            (0, true, 16, 16),
            (18, false, 16, 16),
        ] {
            let observed = ReportLengths {
                input: [(selector, 16)].into(),
                ..Default::default()
            };
            let mut report = [0; 16];
            report[0] = selector;
            let count = read_input_report(&observed, &mut report, windows, |buffer| {
                assert_eq!(buffer[0], selector);
                buffer[1] = 0x5a;
                Ok(native_count)
            })
            .unwrap();
            assert_eq!(count, expected_count);
            assert_eq!(report[1], 0x5a);
        }
    }

    #[test]
    fn mixer_input_rejects_native_errors_overflow_and_unobserved_reports() {
        let observed = ReportLengths {
            input: [(18, 16)].into(),
            ..Default::default()
        };
        let mut report = [0; 16];
        report[0] = 18;
        assert!(
            read_input_report(&observed, &mut report, true, |_| {
                anyhow::bail!("Simulated native Input failure")
            })
            .is_err()
        );
        assert!(read_input_report(&observed, &mut report, true, |_| Ok(16)).is_err());
        assert!(read_input_report(&observed, &mut report, true, |_| Ok(usize::MAX)).is_err());
        report[0] = 9;
        assert!(
            read_input_report(&observed, &mut report, true, |_| {
                panic!("Unobserved Input ID must fail before native I/O")
            })
            .is_err()
        );
    }

    fn lengths(input: &[(u8, usize)]) -> ReportLengths {
        ReportLengths {
            input: input.iter().copied().collect(),
            ..Default::default()
        }
    }

    #[test]
    fn collection_send_preserves_source_interface_prefix_without_padding() {
        let lengths = ReportLengths {
            feature: [(0, 91)].into(),
            ..Default::default()
        };
        for id in [0, 2, 3] {
            let mut report = [0; 91];
            report[0] = id;
            send_collection_feature(&lengths, &report, |sent| {
                assert_eq!(sent, report);
                Ok(())
            })
            .unwrap();
        }
        for size in [0, 90, 92] {
            assert!(
                send_collection_feature(&lengths, &vec![0; size], |_| panic!(
                    "invalid source length must not send"
                ))
                .is_err()
            );
        }
        assert!(
            send_collection_feature(&lengths, &[3; 91], |_| anyhow::bail!(
                "simulated OS send failure"
            ))
            .is_err()
        );
    }

    #[test]
    fn unnumbered_interrupt_keeps_actual_hundred_bytes_without_an_id_slot() {
        let mut data = [0; 100];
        let count =
            read_interrupt_report(&lengths(&[(0, 101)]), &mut data, 50, |buffer, timeout| {
                assert_eq!(timeout, 50);
                buffer.copy_from_slice(&[14; 100]);
                Ok(100)
            })
            .unwrap();
        assert_eq!(count, 100);
        assert_eq!(data, [14; 100]);
    }

    #[test]
    fn numbered_interrupt_accepts_exact_or_collection_padded_lengths() {
        for count in [3, 8] {
            let mut data = [0; 8];
            assert_eq!(
                read_interrupt_report(&lengths(&[(5, 3), (7, 8)]), &mut data, 0, |buffer, _| {
                    buffer[..3].copy_from_slice(&[5, 14, 2]);
                    Ok(count)
                })
                .unwrap(),
                count
            );
            assert_eq!(data[..3], [5, 14, 2]);
        }
    }

    #[test]
    fn interrupt_rejects_invalid_descriptor_buffer_and_timeout_before_read() {
        for (input, capacity, timeout) in [
            (vec![], 100, 0),
            (vec![(0, 0)], 100, 0),
            (vec![(0, 1)], 100, 0),
            (vec![(0, 4), (5, 4)], 100, 0),
            (vec![(0, 101)], 99, 0),
            (vec![(5, 101)], 100, 0),
            (vec![(5, 4)], 100, u32::MAX),
        ] {
            let mut buffer = vec![0; capacity];
            assert!(
                read_interrupt_report(&lengths(&input), &mut buffer, timeout, |_, _| {
                    panic!("invalid descriptor/buffer must not perform an interrupt read")
                })
                .is_err()
            );
        }
    }

    #[test]
    fn interrupt_rejects_unknown_ids_truncation_and_an_excessive_byte_count() {
        for (input, bytes, count) in [
            (vec![(5, 3)], vec![7, 14, 2], 3),
            (vec![(5, 3)], vec![5, 14], 2),
            (vec![(0, 4)], vec![5, 14], 2),
            (vec![(0, 4)], vec![5, 14, 2], 101),
        ] {
            let mut buffer = [0; 100];
            assert!(
                read_interrupt_report(&lengths(&input), &mut buffer, 0, |buffer, _| {
                    buffer[..bytes.len()].copy_from_slice(&bytes);
                    Ok(count)
                })
                .is_err()
            );
        }
    }

    #[test]
    fn interrupt_timeout_returns_no_fabricated_report() {
        let mut data = [0; 100];
        assert_eq!(
            read_interrupt_report(&lengths(&[(0, 4)]), &mut data, 0, |_, _| Ok(0)).unwrap(),
            0
        );
    }
}
