use anyhow::{Context as _, ensure};

/// Current Series10NewFeature chooses the first matching x64 package in
/// registry enumeration order, strips the publisher suffix, then separates
/// the version and platform. An existing key without a match reports blanks.
pub(super) fn package_version<'a>(
    names: impl IntoIterator<Item = &'a str>,
    product: &str,
) -> (String, String) {
    let Some(name) = names
        .into_iter()
        .find(|name| name.contains(product) && name.contains("x64"))
    else {
        return (String::new(), String::new());
    };
    let stem = name.split_once("__").map_or(name, |(stem, _)| stem);
    let tail = stem.split_once('_').map_or(stem, |(_, tail)| tail);
    match tail.split_once('_') {
        Some((version, platform)) => (version.to_owned(), platform.to_owned()),
        None => (String::new(), String::new()),
    }
}

pub(super) fn manufacturer(vendor: u32) -> &'static str {
    match vendor {
        0x1002 => "AMD",
        0x10de => "NVIDIA",
        0x8086 => "Intel",
        _ => "Unknown Manufacturer",
    }
}

/// std::stringstream's defaultfloat uses six significant digits. The input
/// is actual GlobalMemoryStatusEx.ullTotalPhys, scaled three times by 1/1024.
pub(super) fn memory_size(bytes: u64) -> String {
    let size = bytes as f64 * (1.0 / 1024.0) * (1.0 / 1024.0) * (1.0 / 1024.0);
    let scientific = format!("{size:.5e}");
    let (mantissa, exponent) = scientific.split_once('e').expect("Rust scientific format");
    let exponent: i32 = exponent.parse().expect("Rust exponent");
    let text = if !(-4..6).contains(&exponent) {
        format!(
            "{}e{exponent:+03}",
            mantissa.trim_end_matches('0').trim_end_matches('.')
        )
    } else {
        let rounded = scientific.parse::<f64>().expect("Rust scientific number");
        let precision = (5 - exponent).max(0) as usize;
        let text = format!("{rounded:.precision$}");
        if text.contains('.') {
            text.trim_end_matches('0').trim_end_matches('.').to_owned()
        } else {
            text
        }
    };
    format!("{text} GB")
}

pub(super) fn monitor_edid(bytes: &[u8]) -> anyhow::Result<(Option<String>, String)> {
    ensure!(
        bytes.len() >= 126,
        "Monitor EDID is shorter than its four descriptors"
    );
    let manufacturer = String::from_utf8(vec![
        (bytes[8] >> 2) + 64,
        (((bytes[8] << 3) | (bytes[9] >> 5)) & 31) + 64,
        (bytes[9] & 31) + 64,
    ])
    .context("Monitor EDID manufacturer is not UTF-8")?;
    let mut name = None;
    let mut serial_seen = false;
    for descriptor in bytes[54..126].chunks_exact(18) {
        if descriptor[..3] != [0, 0, 0] {
            continue;
        }
        if descriptor[3] == 0xff {
            serial_seen = true;
        }
        if descriptor[3] == 0xfc {
            let value = &descriptor[5..18];
            let end = value
                .iter()
                .position(|byte| matches!(byte, 0 | 10))
                .unwrap_or(value.len());
            name = Some(
                std::str::from_utf8(&value[..end])
                    .context("Monitor name is not UTF-8")?
                    .to_owned(),
            );
        }
        if serial_seen && name.is_some() {
            break;
        }
    }
    Ok((name, manufacturer))
}

pub(super) fn monitor_fallback(device_id: &str) -> &str {
    let tail = device_id
        .split_once('\\')
        .map_or(device_id, |(_, tail)| tail);
    tail.split_once('\\').map_or(tail, |(name, _)| name)
}

pub(super) fn camera_revision(hardware_id: &str) -> anyhow::Result<Option<String>> {
    let Some((_, rest)) = hardware_id.split_once("REV_") else {
        return Ok(None);
    };
    let Some((revision, _)) = rest.split_once('&') else {
        return Ok(None);
    };
    ensure!(
        revision.encode_utf16().count() <= 4,
        "Camera revision exceeds the source buffer"
    );
    Ok(Some(revision.to_owned()))
}

pub(super) fn driver_vendor(name: &str) -> bool {
    ["Intel", "NVIDIA", "THX", "Realtek", "AMD"]
        .iter()
        .any(|vendor| name.contains(vendor))
}
pub(super) fn driver_version(version: u64) -> String {
    format!(
        "{}.{}.{}.{}",
        version >> 48,
        (version >> 32) & 0xffff,
        (version >> 16) & 0xffff,
        version & 0xffff
    )
}
