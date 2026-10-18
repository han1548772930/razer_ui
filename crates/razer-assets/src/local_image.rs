//! Bounded user-image import and locally decoded preview ownership.
//! Preview pixels never stand in for an encoded device/service payload.
use gpui_kit::*;
use razer_storage::file_transfer::{Operation, Transfer};
use std::{collections::HashMap, path::Path, sync::Arc};

// Local allocation/renderer safeguards, not vendor import limits.
const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;
const MAX_DECODED_ESTIMATE: u64 = 256 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImportFormat {
    Gif,
    Png,
    Jpeg,
    Bmp,
}

impl ImportFormat {
    pub fn from_extension(extension: &str) -> Option<Self> {
        match extension.to_ascii_lowercase().as_str() {
            "gif" => Some(Self::Gif),
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "bmp" => Some(Self::Bmp),
            _ => None,
        }
    }

    fn format(self) -> ImageFormat {
        match self {
            Self::Gif => ImageFormat::Gif,
            Self::Png => ImageFormat::Png,
            Self::Jpeg => ImageFormat::Jpeg,
            Self::Bmp => ImageFormat::Bmp,
        }
    }
}

/// The page or final displayed-image lease retains cancellation ownership.
#[derive(Clone, Default)]
pub struct ImageRequest(Transfer);

impl ImageRequest {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.0.cancel();
    }
    pub fn is_same(&self, other: &Self) -> bool {
        self.0.is_same(&other.0)
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.check().is_err()
    }
    pub fn accept(&self) -> std::io::Result<ImageOperation> {
        self.0
            .accept()
            .map(ImageOperation)
            .map_err(std::io::Error::other)
    }
}

/// Accepted before background scheduling, retained through decoder cleanup.
pub struct ImageOperation(Operation);

impl ImageOperation {
    fn check(&self) -> std::io::Result<()> {
        self.0
            .check()
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::Interrupted, error))
    }

    fn read_media(&self, path: &Path, format: ImportFormat) -> std::io::Result<Vec<u8>> {
        self.check()?;
        let bytes = self
            .0
            .read(path, MAX_INPUT_BYTES)
            .map_err(std::io::Error::other)?;
        validate_media(&bytes, format.format())?;
        self.check()?;
        Ok(bytes)
    }

    pub fn load(
        &self,
        path: &Path,
        format: ImportFormat,
        renderer: SvgRenderer,
    ) -> std::io::Result<ImportedImage> {
        let bytes = self.read_media(path, format)?;
        let length = bytes.len() as u64;
        let source = format!(
            "data:{};base64,{}",
            format.format().mime_type(),
            base64_encode(&bytes)
        );
        self.check()?;
        // GPUI's decoder preserves GIF frames and applies EXIF orientation.
        let preview = decode_preview(format.format(), bytes, renderer)?;
        self.check()?;
        Ok(ImportedImage {
            source,
            bytes: length,
            preview,
        })
    }

    fn decode_url(&self, source: &str, renderer: SvgRenderer) -> std::io::Result<Arc<RenderImage>> {
        self.check()?;
        let (header, payload) = source
            .split_once(";base64,")
            .ok_or_else(|| std::io::Error::other("Invalid OLED image data URL"))?;
        let format = header
            .strip_prefix("data:")
            .and_then(ImageFormat::from_mime_type)
            .filter(|format| {
                matches!(
                    format,
                    ImageFormat::Gif | ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::Bmp
                )
            })
            .ok_or_else(|| std::io::Error::other("Unsupported OLED image data URL"))?;
        let bytes = decode_base64(payload)?;
        validate_media(&bytes, format)?;
        self.check()?;
        let image = decode_preview(format, bytes, renderer)?;
        self.check()?;
        Ok(image)
    }
}

pub struct ImportedImage {
    source: String,
    bytes: u64,
    preview: Arc<RenderImage>,
}

impl ImportedImage {
    pub fn into_parts(self) -> (String, u64, Arc<RenderImage>) {
        (self.source, self.bytes, self.preview)
    }
}

struct ImportedImageAsset;
impl Asset for ImportedImageAsset {
    type Source = SharedString;
    type Output = Result<Arc<RenderImage>, ImageCacheError>;

    fn load(
        source: Self::Source,
        cx: &mut App,
    ) -> impl Future<Output = Self::Output> + Send + 'static {
        let renderer = cx.svg_renderer();
        let operation = cx
            .default_global::<ImportedImageLeases>()
            .0
            .get(&source)
            .ok_or_else(|| std::io::Error::other("Local image has no displayed-image owner"))
            .and_then(|entry| entry.request.accept());
        cx.background_executor()
            .spawn(async move { operation?.decode_url(&source, renderer).map_err(Into::into) })
    }
}

fn decode_preview(
    format: ImageFormat,
    bytes: Vec<u8>,
    renderer: SvgRenderer,
) -> std::io::Result<Arc<RenderImage>> {
    let preview = Image::from_bytes(format, bytes)
        .to_image_data(renderer)
        .map_err(std::io::Error::other)?;
    if preview.frame_count() == 0 {
        return Err(std::io::Error::other("OLED image has no frames"));
    }
    let size = preview.size(0);
    if size.width.0 == 0 || size.height.0 == 0 {
        return Err(std::io::Error::other("OLED image has no dimensions"));
    }
    let mut resident = 0u64;
    for index in 0..preview.frame_count() {
        let frame = preview
            .as_bytes(index)
            .ok_or_else(|| std::io::Error::other("Missing OLED frame"))?;
        resident = resident
            .checked_add(frame.len() as u64 + 128)
            .ok_or_else(|| std::io::Error::other("OLED frame size overflow"))?;
        if preview.size(index) != size || resident > MAX_DECODED_ESTIMATE {
            return Err(std::io::Error::other(
                "Invalid or oversized decoded OLED frames",
            ));
        }
    }
    Ok(preview)
}

#[derive(Default)]
struct ImportedImageLeases(HashMap<SharedString, LeaseEntry>);
impl Global for ImportedImageLeases {}

struct LeaseEntry {
    count: usize,
    request: ImageRequest,
}

struct ImageLease {
    source: SharedString,
    request: ImageRequest,
}

impl ImportedImageLeases {
    fn retain(&mut self, source: SharedString) -> ImageLease {
        let entry = self.0.entry(source.clone()).or_insert_with(|| LeaseEntry {
            count: 0,
            request: ImageRequest::new(),
        });
        entry.count += 1;
        ImageLease {
            source,
            request: entry.request.clone(),
        }
    }

    /// Only the final lease for this decode identity can remove its asset.
    fn release(&mut self, lease: ImageLease) -> Option<SharedString> {
        let entry = self.0.get_mut(&lease.source)?;
        if !entry.request.is_same(&lease.request) {
            return None;
        }
        entry.count -= 1;
        if entry.count > 0 {
            return None;
        }
        entry.request.cancel();
        self.0.remove(&lease.source);
        Some(lease.source)
    }
}

struct ImportedImageLease {
    current: Option<ImageLease>,
}

impl ImportedImageLease {
    fn acquire(source: SharedString, leases: &mut ImportedImageLeases) -> Self {
        Self {
            current: Some(leases.retain(source)),
        }
    }

    /// Retain the new source before returning the old lease for release.
    fn replace(
        &mut self,
        source: SharedString,
        leases: &mut ImportedImageLeases,
    ) -> Option<ImageLease> {
        if self
            .current
            .as_ref()
            .is_some_and(|lease| lease.source == source)
        {
            return None;
        }
        self.current.replace(leases.retain(source))
    }

    fn new(source: SharedString, cx: &mut Context<Self>) -> Self {
        cx.on_release(|this, cx| {
            if let Some(lease) = this.current.take() {
                Self::release(lease, cx);
            }
        })
        .detach();
        Self::acquire(source, cx.default_global::<ImportedImageLeases>())
    }

    fn set_source(&mut self, source: SharedString, cx: &mut Context<Self>) {
        let previous = self.replace(source, cx.default_global::<ImportedImageLeases>());
        if let Some(previous) = previous {
            Self::release(previous, cx);
        }
    }

    fn release(lease: ImageLease, cx: &mut App) {
        if let Some(source) = cx.default_global::<ImportedImageLeases>().release(lease) {
            // Removing the final pending asset also prevents its late result
            // from publishing into a replacement source's framework cache.
            cx.remove_asset::<ImportedImageAsset>(&source);
        }
    }
}

pub fn preview_source(id: SharedString, source: SharedString) -> ImageSource {
    if source.starts_with("data:") {
        // A data URL passed directly to img is a URI in GPUI's HTTP loader.
        // Keep custom media in the native asset cache and decode it locally.
        ImageSource::Custom(Arc::new(move |window, cx| {
            // Keyed state follows the displayed image, including content
            // replacement. Two simultaneous previews share one decoded asset.
            let lease = window.use_keyed_state(id.clone(), cx, |_, cx| {
                ImportedImageLease::new(source.clone(), cx)
            });
            lease.update(cx, |lease, cx| lease.set_source(source.clone(), cx));
            window.use_asset::<ImportedImageAsset>(&source, cx)
        }))
    } else {
        source.into()
    }
}

fn decode_base64(value: &str) -> std::io::Result<Vec<u8>> {
    let invalid = || std::io::Error::other("Invalid OLED image base64");
    if value.is_empty() || value.len() % 4 != 0 || value.len() > MAX_INPUT_BYTES.div_ceil(3) * 4 {
        return Err(invalid());
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(value.len() / 4 * 3)
        .map_err(std::io::Error::other)?;
    let count = value.len() / 4;
    for (ix, chunk) in value.as_bytes().chunks_exact(4).enumerate() {
        let mut word = 0u32;
        let mut padding = 0;
        for (offset, &byte) in chunk.iter().enumerate() {
            let part = match byte {
                b'A'..=b'Z' if padding == 0 => byte - b'A',
                b'a'..=b'z' if padding == 0 => byte - b'a' + 26,
                b'0'..=b'9' if padding == 0 => byte - b'0' + 52,
                b'+' if padding == 0 => 62,
                b'/' if padding == 0 => 63,
                b'=' if ix + 1 == count && offset >= 2 => {
                    padding += 1;
                    0
                }
                _ => return Err(invalid()),
            };
            word = (word << 6) | u32::from(part);
        }
        if (padding == 1 && word & 0xff != 0) || (padding == 2 && word & 0xffff != 0) {
            return Err(invalid());
        }
        output.push((word >> 16) as u8);
        if padding < 2 {
            output.push((word >> 8) as u8);
        }
        if padding == 0 {
            output.push(word as u8);
        }
    }
    Ok(output)
}

fn validate_media(bytes: &[u8], format: ImageFormat) -> std::io::Result<()> {
    let invalid = || std::io::Error::other("Invalid or oversized OLED media header");
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(invalid());
    }
    let (width, height, frames) = match format {
        ImageFormat::Gif => gif_layout(bytes).ok_or_else(invalid)?,
        ImageFormat::Png
            if bytes.starts_with(b"\x89PNG\r\n\x1a\n")
                && bytes.len() >= 33
                && bytes.get(8..16) == Some(b"\0\0\0\rIHDR") =>
        {
            (
                u32::from_be_bytes(
                    bytes
                        .get(16..20)
                        .ok_or_else(invalid)?
                        .try_into()
                        .map_err(|_| invalid())?,
                ) as u64,
                u32::from_be_bytes(
                    bytes
                        .get(20..24)
                        .ok_or_else(invalid)?
                        .try_into()
                        .map_err(|_| invalid())?,
                ) as u64,
                1,
            )
        }
        ImageFormat::Jpeg => jpeg_dimensions(bytes)
            .map(|(w, h)| (w, h, 1))
            .ok_or_else(invalid)?,
        ImageFormat::Bmp if bytes.starts_with(b"BM") => {
            let dib = u32::from_le_bytes(
                bytes
                    .get(14..18)
                    .ok_or_else(invalid)?
                    .try_into()
                    .map_err(|_| invalid())?,
            );
            if u64::from(dib) + 14 > bytes.len() as u64 {
                return Err(invalid());
            }
            if dib == 12 {
                (
                    le16(bytes, 18).ok_or_else(invalid)? as u64,
                    le16(bytes, 20).ok_or_else(invalid)? as u64,
                    1,
                )
            } else if dib >= 40 {
                let width = i32::from_le_bytes(
                    bytes
                        .get(18..22)
                        .ok_or_else(invalid)?
                        .try_into()
                        .map_err(|_| invalid())?,
                );
                let height = i32::from_le_bytes(
                    bytes
                        .get(22..26)
                        .ok_or_else(invalid)?
                        .try_into()
                        .map_err(|_| invalid())?,
                );
                if width <= 0 {
                    return Err(invalid());
                }
                (width as u64, height.unsigned_abs() as u64, 1)
            } else {
                return Err(invalid());
            }
        }
        _ => return Err(invalid()),
    };
    // Estimate resident RGBA frames and a small per-frame record. Codec
    // working canvases, compressed bytes and metadata are outside this
    // estimate, so this is not a hard process-memory ceiling.
    let estimate = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
        .and_then(|frame| frame.checked_mul(frames))
        .and_then(|pixels| {
            frames
                .checked_mul(128)
                .and_then(|overhead| pixels.checked_add(overhead))
        });
    if width == 0
        || height == 0
        || frames == 0
        || estimate.is_none_or(|bytes| bytes > MAX_DECODED_ESTIMATE)
    {
        return Err(invalid());
    }
    Ok(())
}

fn le16(bytes: &[u8], offset: usize) -> Option<u16> {
    Some(u16::from_le_bytes(
        bytes.get(offset..offset.checked_add(2)?)?.try_into().ok()?,
    ))
}

fn skip_blocks(bytes: &[u8], mut offset: usize) -> Option<usize> {
    loop {
        let length = *bytes.get(offset)? as usize;
        offset = offset.checked_add(1)?;
        if length == 0 {
            return Some(offset);
        }
        offset = offset.checked_add(length)?;
        if offset > bytes.len() {
            return None;
        }
    }
}

fn gif_layout(bytes: &[u8]) -> Option<(u64, u64, u64)> {
    if !bytes.starts_with(b"GIF87a") && !bytes.starts_with(b"GIF89a") {
        return None;
    }
    let width = le16(bytes, 6)? as u64;
    let height = le16(bytes, 8)? as u64;
    let packed = *bytes.get(10)?;
    let mut offset = 13usize;
    if packed & 0x80 != 0 {
        offset += 3usize << ((packed & 7) + 1);
    }
    let mut frames = 0u64;
    loop {
        let tag = *bytes.get(offset)?;
        offset += 1;
        match tag {
            0x3b => return (frames > 0).then_some((width, height, frames)),
            0x21 => {
                bytes.get(offset)?;
                offset = skip_blocks(bytes, offset + 1)?;
            }
            0x2c => {
                let left = le16(bytes, offset)? as u64;
                let top = le16(bytes, offset + 2)? as u64;
                let frame_width = le16(bytes, offset + 4)? as u64;
                let frame_height = le16(bytes, offset + 6)? as u64;
                if frame_width == 0
                    || frame_height == 0
                    || left + frame_width > width
                    || top + frame_height > height
                {
                    return None;
                }
                let packed = *bytes.get(offset + 8)?;
                offset += 9;
                if packed & 0x80 != 0 {
                    offset += 3usize << ((packed & 7) + 1);
                }
                if !(2..=8).contains(bytes.get(offset)?) {
                    return None;
                }
                offset = skip_blocks(bytes, offset + 1)?;
                frames += 1;
                if width * height * 4 * frames + frames * 128 > MAX_DECODED_ESTIMATE {
                    return None;
                }
            }
            _ => return None,
        }
    }
}

fn jpeg_dimensions(bytes: &[u8]) -> Option<(u64, u64)> {
    if !bytes.starts_with(b"\xff\xd8") {
        return None;
    }
    let mut offset = 2usize;
    let mut dimensions = None;
    while offset < bytes.len() {
        if *bytes.get(offset)? != 0xff {
            return None;
        }
        while bytes.get(offset) == Some(&0xff) {
            offset += 1;
        }
        let marker = *bytes.get(offset)?;
        offset += 1;
        if marker == 0xd9 {
            return None;
        }
        if marker == 0x01 || (0xd0..=0xd8).contains(&marker) {
            continue;
        }
        let length = u16::from_be_bytes(bytes.get(offset..offset + 2)?.try_into().ok()?) as usize;
        if length < 2 {
            return None;
        }
        let end = offset.checked_add(length)?;
        if end > bytes.len() {
            return None;
        }
        if marker == 0xda {
            return dimensions;
        }
        if matches!(marker, 0xc0..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf) {
            if length < 8 {
                return None;
            }
            let height =
                u16::from_be_bytes(bytes.get(offset + 3..offset + 5)?.try_into().ok()?) as u64;
            let width =
                u16::from_be_bytes(bytes.get(offset + 5..offset + 7)?.try_into().ok()?) as u64;
            if dimensions.is_some_and(|previous| previous != (width, height)) {
                return None;
            }
            dimensions = Some((width, height));
        }
        offset = end;
    }
    None
}

/// Tiny dependency-free base64 encoder for local preview data URLs.
fn base64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let a = chunk[0] as u32;
        let b = chunk.get(1).copied().unwrap_or(0) as u32;
        let c = chunk.get(2).copied().unwrap_or(0) as u32;
        let value = (a << 16) | (b << 8) | c;
        output.push(TABLE[((value >> 18) & 63) as usize] as char);
        output.push(TABLE[((value >> 12) & 63) as usize] as char);
        output.push(if chunk.len() > 1 {
            TABLE[((value >> 6) & 63) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            TABLE[(value & 63) as usize] as char
        } else {
            '='
        });
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn displayed_request(leases: &ImportedImageLeases, source: &'static str) -> ImageRequest {
        leases.0[&SharedString::from(source)].request.clone()
    }

    fn displayed_count(leases: &ImportedImageLeases, source: &'static str) -> usize {
        leases.0[&SharedString::from(source)].count
    }

    #[::core::prelude::v1::test]
    fn displayed_image_same_slot_same_content_keeps_the_single_decode_lease() {
        let mut leases = ImportedImageLeases::default();
        let mut slot = ImportedImageLease::acquire("A".into(), &mut leases);
        let request = displayed_request(&leases, "A");
        assert!(slot.replace("A".into(), &mut leases).is_none());
        assert_eq!(displayed_count(&leases, "A"), 1);
        assert!(displayed_request(&leases, "A").is_same(&request));
        assert!(!request.is_cancelled());
        assert_eq!(
            leases.release(slot.current.take().unwrap()),
            Some("A".into())
        );
        assert!(request.is_cancelled());
        assert!(leases.0.is_empty());
    }

    #[::core::prelude::v1::test]
    fn displayed_image_replacement_retains_new_content_before_releasing_the_old() {
        let mut leases = ImportedImageLeases::default();
        let mut slot = ImportedImageLease::acquire("A".into(), &mut leases);
        let previous_request = displayed_request(&leases, "A");
        let previous = slot.replace("B".into(), &mut leases).unwrap();
        let current_request = displayed_request(&leases, "B");
        assert_eq!(
            slot.current.as_ref().unwrap().source,
            SharedString::from("B")
        );
        assert_eq!(displayed_count(&leases, "A"), 1);
        assert_eq!(displayed_count(&leases, "B"), 1);
        assert!(!previous_request.is_cancelled());
        assert!(!current_request.is_cancelled());
        assert_eq!(leases.release(previous), Some("A".into()));
        assert!(previous_request.is_cancelled());
        assert!(!current_request.is_cancelled());
        assert_eq!(
            leases.release(slot.current.take().unwrap()),
            Some("B".into())
        );
    }

    #[::core::prelude::v1::test]
    fn displayed_image_a_b_a_rejects_the_old_decode_without_releasing_the_replacement() {
        let mut leases = ImportedImageLeases::default();
        let mut slot = ImportedImageLease::acquire("A".into(), &mut leases);
        let old_request = displayed_request(&leases, "A");
        let old_decode = old_request.accept().unwrap();
        let old_a = slot.replace("B".into(), &mut leases).unwrap();
        assert_eq!(leases.release(old_a), Some("A".into()));
        assert!(old_decode.check().is_err());
        let old_b = slot.replace("A".into(), &mut leases).unwrap();
        let new_request = displayed_request(&leases, "A");
        assert!(!new_request.is_same(&old_request));
        assert_eq!(leases.release(old_b), Some("B".into()));
        drop(old_decode);
        assert_eq!(displayed_count(&leases, "A"), 1);
        assert!(!new_request.is_cancelled());
        assert!(displayed_request(&leases, "A").is_same(&new_request));
        assert_eq!(
            leases.release(slot.current.take().unwrap()),
            Some("A".into())
        );
        assert!(new_request.is_cancelled());
    }

    #[::core::prelude::v1::test]
    fn displayed_image_two_slots_share_content_until_the_final_window_lease_leaves() {
        let mut leases = ImportedImageLeases::default();
        let mut first = ImportedImageLease::acquire("A".into(), &mut leases);
        let mut second = ImportedImageLease::acquire("A".into(), &mut leases);
        let request = displayed_request(&leases, "A");
        assert_eq!(displayed_count(&leases, "A"), 2);
        let old_first = first.replace("B".into(), &mut leases).unwrap();
        assert_eq!(displayed_count(&leases, "A"), 2);
        assert_eq!(displayed_count(&leases, "B"), 1);
        assert_eq!(leases.release(old_first), None);
        assert_eq!(displayed_count(&leases, "A"), 1);
        assert!(!request.is_cancelled());
        let old_second = second.replace("B".into(), &mut leases).unwrap();
        assert_eq!(leases.release(old_second), Some("A".into()));
        assert!(request.is_cancelled());
        assert_eq!(displayed_count(&leases, "B"), 2);
        let b = displayed_request(&leases, "B");
        assert_eq!(leases.release(first.current.take().unwrap()), None);
        assert!(!b.is_cancelled());
        assert_eq!(
            leases.release(second.current.take().unwrap()),
            Some("B".into())
        );
        assert!(b.is_cancelled());
        assert!(leases.0.is_empty());
    }

    fn png_header(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        bytes.extend(width.to_be_bytes());
        bytes.extend(height.to_be_bytes());
        bytes.resize(33, 0);
        bytes
    }

    #[::core::prelude::v1::test]
    fn fixed_media_headers_reject_zero_huge_and_truncated_images() {
        assert!(validate_media(&png_header(232, 64), ImageFormat::Png).is_ok());
        assert!(validate_media(&png_header(0, 64), ImageFormat::Png).is_err());
        assert!(validate_media(&png_header(u32::MAX, u32::MAX), ImageFormat::Png).is_err());
        assert!(validate_media(&png_header(232, 64)[..25], ImageFormat::Png).is_err());
        assert!(validate_media(b"GIF89a", ImageFormat::Gif).is_err());
        assert!(validate_media(b"\xff\xd8\xff\xc0\x00\x08", ImageFormat::Jpeg).is_err());
        assert!(validate_media(b"BM", ImageFormat::Bmp).is_err());
    }

    #[::core::prelude::v1::test]
    fn base64_requires_canonical_padding_and_valid_alphabet() {
        assert_eq!(decode_base64("AAECAw==").unwrap(), [0, 1, 2, 3]);
        for invalid in [
            "", "AAA", "A===", "AB==", "AAB=", "AA==AAAA", "AA A", "____",
        ] {
            assert!(decode_base64(invalid).is_err(), "{invalid}");
        }
        assert_eq!(base64_encode(&[0, 1, 2, 3]), "AAECAw==");
    }

    #[::core::prelude::v1::test]
    fn cancellation_is_owned_by_the_full_request_identity() {
        let first = ImageRequest::new();
        let clone = first.clone();
        let second = ImageRequest::new();
        let accepted = first.accept().unwrap();
        assert!(first.is_same(&clone));
        assert!(!first.is_same(&second));
        clone.cancel();
        assert!(accepted.check().is_err());
        assert!(first.accept().is_err());
        assert!(!second.is_cancelled());
    }

    #[::core::prelude::v1::test]
    fn bounded_file_import_checks_cancellation_format_and_metadata_size() {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "razer-local-image-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::write(&path, png_header(232, 64)).unwrap();
        let request = ImageRequest::new();
        let operation = request.accept().unwrap();
        assert!(operation.read_media(&path, ImportFormat::Png).is_ok());
        assert!(operation.read_media(&path, ImportFormat::Gif).is_err());
        std::fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .unwrap()
            .set_len(MAX_INPUT_BYTES as u64 + 1)
            .unwrap();
        assert!(operation.read_media(&path, ImportFormat::Png).is_err());
        request.cancel();
        std::fs::remove_file(&path).unwrap();
        let error = operation.read_media(&path, ImportFormat::Png).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::Interrupted);
    }

    #[::core::prelude::v1::test]
    fn supported_extensions_are_typed_before_reading_user_files() {
        assert_eq!(
            ImportFormat::from_extension("JPEG"),
            Some(ImportFormat::Jpeg)
        );
        assert_eq!(ImportFormat::from_extension("gif"), Some(ImportFormat::Gif));
        assert_eq!(ImportFormat::from_extension("svg"), None);
    }
}
