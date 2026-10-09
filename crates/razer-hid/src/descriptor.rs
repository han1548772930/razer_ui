//! HID 1.11 short-item Feature sizes, used only to prevent backend padding.
//! This describes the OS transport, not Razer protocol fields or capabilities.
use anyhow::{bail, ensure};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Default)]
struct Globals {
    size: u32,
    count: u32,
    id: u8,
}

pub(super) fn feature_lengths(bytes: &[u8]) -> anyhow::Result<BTreeMap<u8, usize>> {
    let mut globals = Globals::default();
    let mut stack = Vec::new();
    let mut bits = BTreeMap::<u8, u64>::new();
    let mut offset = 0;
    while offset < bytes.len() {
        let prefix = bytes[offset];
        offset += 1;
        if prefix == 0xfe {
            ensure!(offset + 2 <= bytes.len(), "HID long item 头不完整");
            let length = bytes[offset] as usize;
            offset += 2;
            ensure!(offset + length <= bytes.len(), "HID long item 内容不完整");
            offset += length;
            continue;
        }
        let length = match prefix & 3 {
            3 => 4,
            length => usize::from(length),
        };
        ensure!(offset + length <= bytes.len(), "HID short item 内容不完整");
        let mut value = 0u32;
        for (index, byte) in bytes[offset..offset + length].iter().enumerate() {
            value |= u32::from(*byte) << (index * 8);
        }
        offset += length;
        match (prefix & 0x0c, prefix >> 4) {
            (4, 7) => globals.size = value,
            (4, 8) => {
                ensure!((1..=255).contains(&value), "HID Report ID 无效");
                globals.id = value as u8;
            }
            (4, 9) => globals.count = value,
            (4, 10) => {
                ensure!(stack.len() < 64, "HID global stack 过深");
                stack.push(globals);
            }
            (4, 11) => {
                globals = stack
                    .pop()
                    .ok_or_else(|| anyhow::anyhow!("HID global stack 下溢"))?
            }
            (0, 11) => {
                let total = bits.entry(globals.id).or_default();
                *total = total
                    .checked_add(u64::from(globals.size) * u64::from(globals.count))
                    .ok_or_else(|| anyhow::anyhow!("HID Feature 位数溢出"))?;
                ensure!(*total <= 65535 * 8, "HID Feature 报告过长");
            }
            _ => {}
        }
    }
    if bits.is_empty() {
        bail!("HID descriptor 未声明 Feature Report");
    }
    Ok(bits
        .into_iter()
        .map(|(id, bits)| (id, bits.div_ceil(8) as usize + 1))
        .collect())
}
