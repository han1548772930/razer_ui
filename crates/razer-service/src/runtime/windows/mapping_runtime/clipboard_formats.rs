//! Clipboard semantic formats: 22d050/276b0b, 276918, 276818 and 276f20.
use anyhow::{Context, Result, ensure};
use image::{ImageEncoder, codecs::png::PngEncoder};

pub(super) enum Snapshot {
    Text(Vec<u16>),
    Html { html: String, source_url: String },
    Rtf(Vec<u8>),
    Files(Vec<Vec<u16>>),
    Png(Vec<u8>),
}

pub(super) fn terminated_utf16(bytes: &[u8]) -> Result<Vec<u16>> {
    ensure!(bytes.len().is_multiple_of(2), "Odd UTF-16 clipboard payload length");
    Ok(bytes.chunks_exact(2).map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
        .take_while(|unit| *unit != 0).collect())
}

pub(super) fn nul_bytes(bytes: &[u8]) -> &[u8] {
    &bytes[..bytes.iter().position(|byte| *byte == 0).unwrap_or(bytes.len())]
}

pub(super) fn text_bytes(text: &[u16]) -> Vec<u8> {
    text.iter().copied().chain(Some(0)).flat_map(u16::to_le_bytes).collect()
}

pub(super) fn parse_html(bytes: &[u8]) -> Result<Snapshot> {
    let bytes = nul_bytes(bytes);
    let lower = bytes.to_ascii_lowercase();
    let start = lower.windows(5).position(|part| part == b"<html").context("Clipboard HTML has no html start")?;
    let find = |needle: &[u8]| bytes.windows(needle.len()).position(|part| part == needle);
    let source_url = find(b"SourceURL:").and_then(|offset| {
        let tail = &bytes[offset + 10..];
        let end = tail.iter().position(|byte| *byte == b'\n')?;
        Some(String::from_utf8_lossy(&tail[..end]).trim_matches(|c: char| c.is_ascii_whitespace()).to_owned())
    }).unwrap_or_default();
    let decimal = |label: &[u8]| -> Option<usize> {
        let offset = find(label)? + label.len();
        let text = std::str::from_utf8(&bytes[offset..]).ok()?.trim_start();
        let end = text.bytes().take_while(u8::is_ascii_digit).count();
        text[..end].parse().ok()
    };
    let (fragment_start, fragment_end) = if let Some(mark) = bytes[start..].windows(17).position(|part| part == b"<!--StartFragment") {
        let mark = start + mark;
        let begin = bytes[mark..].iter().position(|byte| *byte == b'>').map(|offset| mark + offset + 1);
        let end_mark = bytes.windows(15).rposition(|part| part == b"<!--EndFragment").unwrap_or(bytes.len());
        let end = bytes[..end_mark.min(bytes.len().saturating_sub(1)) + 1].iter().rposition(|byte| *byte == b'<');
        (begin, end)
    } else {
        (decimal(b"StartFragment:"), decimal(b"EndFragment:"))
    };
    let (begin, end) = fragment_start.zip(fragment_end).context("Clipboard HTML fragment bounds are missing")?;
    ensure!(begin >= start && end >= begin, "Clipboard HTML fragment bounds are reversed");
    // ReadHTML retains the entire HTML body starting at <html, rather than
    // extracting only the fragment indicated by the header or comments.
    Ok(Snapshot::Html { html: String::from_utf8_lossy(&bytes[start..]).into_owned(), source_url })
}

pub(super) fn html_bytes(html: &str, source_url: &str) -> Result<Vec<u8>> {
    if html.is_empty() { return Ok(Vec::new()); }
    let start_html = 105usize.checked_add(if source_url.is_empty() { 0 } else { source_url.len() + 12 }).context("Clipboard HTML size overflow")?;
    let start_fragment = start_html + 36;
    let end_fragment = start_fragment.checked_add(html.len()).context("Clipboard HTML size overflow")?;
    let end_html = end_fragment + 36;
    ensure!(end_html <= u32::MAX as usize, "Clipboard HTML exceeds source offset width");
    let mut output = format!("Version:0.9\r\nStartHTML:{start_html:010}\r\nEndHTML:{end_html:010}\r\nStartFragment:{start_fragment:010}\r\nEndFragment:{end_fragment:010}\r\n").into_bytes();
    if !source_url.is_empty() { output.extend_from_slice(format!("SourceURL:{source_url}\r\n").as_bytes()); }
    output.extend_from_slice(b"<html>\r\n<body>\r\n<!--StartFragment-->");
    output.extend_from_slice(html.as_bytes());
    output.extend_from_slice(b"<!--EndFragment-->\r\n</body>\r\n</html>");
    output.push(0);
    Ok(output)
}

pub(super) fn files_bytes(paths: &[Vec<u16>]) -> Vec<u8> {
    let mut bytes = vec![0; 20];
    bytes[..4].copy_from_slice(&20u32.to_le_bytes());
    bytes[16..20].copy_from_slice(&1u32.to_le_bytes());
    for path in paths { bytes.extend(text_bytes(path)); }
    bytes.extend_from_slice(&[0, 0]);
    bytes
}

pub(super) fn png_bytes(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>> {
    let mut png = Vec::new();
    PngEncoder::new(&mut png).write_image(rgba, width, height, image::ExtendedColorType::Rgba8)?;
    Ok(png)
}

pub(super) fn image_payloads(png: &[u8]) -> Result<(Vec<u8>, Vec<u8>)> {
    let image = image::load_from_memory_with_format(png, image::ImageFormat::Png)?.into_rgba8();
    let (width, height) = image.dimensions();
    ensure!(width <= i32::MAX as u32 && height <= i32::MAX as u32, "Clipboard bitmap dimensions exceed source range");
    let encoded = png_bytes(width, height, image.as_raw())?;
    let mut dib = vec![0; 124];
    dib[..4].copy_from_slice(&124u32.to_le_bytes());
    dib[4..8].copy_from_slice(&width.to_le_bytes());
    dib[8..12].copy_from_slice(&height.to_le_bytes());
    dib[12..14].copy_from_slice(&1u16.to_le_bytes());
    dib[14..16].copy_from_slice(&32u16.to_le_bytes());
    dib[52..56].copy_from_slice(&0xff00_0000u32.to_le_bytes());
    dib[56..60].copy_from_slice(&0x5769_6e20u32.to_le_bytes());
    dib[108..112].copy_from_slice(&4u32.to_le_bytes());
    for row in image.rows().rev() {
        for pixel in row { let [r,g,b,a] = pixel.0; dib.extend_from_slice(&[b,g,r,a]); }
    }
    Ok((encoded, dib))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn html_capture_retains_whole_body_and_rebuilds_byte_offsets() {
        let original = b"Version:0.9\r\nSourceURL: https://local.invalid/a \r\n<HTML><body><!--StartFragment-->x<!--EndFragment--></body></HTML>\0unused";
        let Snapshot::Html {html, source_url} = parse_html(original).unwrap() else { panic!() };
        assert_eq!(html, "<HTML><body><!--StartFragment-->x<!--EndFragment--></body></HTML>");
        assert_eq!(source_url, "https://local.invalid/a");
        let bytes = html_bytes("é", &source_url).unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert!(text.starts_with("Version:0.9\r\nStartHTML:0000000140\r\nEndHTML:0000000214\r\nStartFragment:0000000176\r\nEndFragment:0000000178\r\n"));
        assert_eq!(&bytes[176..178], "é".as_bytes());
    }
    #[test]
    fn dropfiles_rebuilds_wide_paths_with_final_double_terminator() {
        let bytes = files_bytes(&[vec![65, 58, 92, 0x4e00], vec![66,58,92,98]]);
        assert_eq!(&bytes[..4], &[20,0,0,0]);
        assert_eq!(&bytes[16..20], &[1,0,0,0]);
        assert_eq!(&bytes[20..], &[65,0,58,0,92,0,0,78,0,0,66,0,58,0,92,0,98,0,0,0,0,0]);
    }
    #[test]
    fn image_restore_publishes_png_and_bottom_up_bgra_dibv5() {
        let png = png_bytes(1,2,&[255,0,0,255,0,0,255,128]).unwrap();
        let (restored,dib) = image_payloads(&png).unwrap();
        assert_eq!(image::load_from_memory_with_format(&restored,image::ImageFormat::Png).unwrap().into_rgba8().as_raw(), &[255,0,0,255,0,0,255,128]);
        assert_eq!(&dib[124..], &[255,0,0,128,0,0,255,255]);
        assert_eq!(&dib[52..60], &[0,0,0,255,32,110,105,87]);
        assert_eq!(&dib[108..112], &[4,0,0,0]);
    }
}
