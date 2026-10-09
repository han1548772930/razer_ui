//! Current product-691 CropperJS viewMode 0 geometry and local media decoding.
//! Decoded previews never stand in for the worker's processed GIF payload.
use gpui_kit::*;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, io::Read, path::Path, sync::Arc};

pub(super) const WIDTH: f32 = 232.;
pub(super) const HEIGHT: f32 = 190.;
pub(super) const CROP_HEIGHT: f32 = 64.;
pub(super) const CROP_TOP: f32 = (HEIGHT - CROP_HEIGHT) / 2.;

// Local allocation/renderer safeguards, not vendor import or zoom limits.
const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;
const MAX_DECODED_ESTIMATE: u64 = 256 * 1024 * 1024;
// Any u32 image aspect at the 64px minimum fits below 2^38. This guard
// therefore accepts every valid initial canvas while rejecting huge saved
// floats and zoom values that would overflow subsequent layout arithmetic.
const MAX_CANVAS_EDGE: f32 = 1_099_511_627_776.; // 2^40 CSS pixels

#[derive(Clone, Copy, Deserialize, Serialize)]
pub(super) struct CropCanvas {
    width: f32,
    height: f32,
    left: f32,
    top: f32,
}

impl CropCanvas {
    pub(super) fn new(width: f32, height: f32) -> Self {
        let aspect = width / height;
        // initCanvas uses contain for viewMode 0. limitCanvas then applies
        // minCanvasHeight 64 and derives minWidth from the image aspect.
        let height = (WIDTH / aspect).min(HEIGHT).max(CROP_HEIGHT);
        let width = height * aspect;
        Self {
            width,
            height,
            left: (WIDTH - width) / 2.,
            top: (HEIGHT - height) / 2.,
        }
    }

    pub(super) fn width(self) -> f32 {
        self.width
    }
    pub(super) fn height(self) -> f32 {
        self.height
    }
    pub(super) fn left(self) -> f32 {
        self.left
    }
    pub(super) fn top(self) -> f32 {
        self.top
    }

    pub(super) fn normalized(mut self) -> Option<Self> {
        if ![self.width, self.height, self.left, self.top]
            .into_iter()
            .all(f32::is_finite)
            || self.width <= 0.
            || self.height < CROP_HEIGHT
            || self.width > MAX_CANVAS_EDGE
            || self.height > MAX_CANVAS_EDGE
        {
            return None;
        }
        self.limit_position();
        Some(self)
    }

    fn limit_position(&mut self) {
        // viewMode 0 deliberately allows blank space in the crop box.
        self.left = self.left.clamp(-self.width, WIDTH);
        self.top = self.top.clamp(-self.height, HEIGHT);
    }

    pub(super) fn move_by(&mut self, x: f32, y: f32) {
        if x.is_finite() && y.is_finite() {
            self.left += x;
            self.top += y;
            self.limit_position();
        }
    }

    pub(super) fn zoom_by(&mut self, delta: f32) {
        let factor = if delta < 0. {
            1. / (1. - delta)
        } else {
            1. + delta
        };
        let width = self.width * factor;
        let height = self.height * factor;
        if !width.is_finite()
            || !height.is_finite()
            || width <= 0.
            || height <= 0.
            || width > MAX_CANVAS_EDGE
            || height > MAX_CANVAS_EDGE
        {
            return;
        }
        // zoomTo without a pointer anchor keeps the canvas centre fixed.
        // renderCanvas restores oldLeft/oldTop when a size falls below its
        // minimum, then clamps the size. It does not recenter the minimum.
        if height >= CROP_HEIGHT {
            self.left -= (width - self.width) / 2.;
            self.top -= (height - self.height) / 2.;
            self.width = width;
            self.height = height;
        } else {
            self.width *= CROP_HEIGHT / self.height;
            self.height = CROP_HEIGHT;
        }
        self.limit_position();
    }
}

pub(super) struct ImportedPreset {
    source: String,
    bytes: u64,
    preview: Arc<RenderImage>,
}

impl ImportedPreset {
    pub(super) fn into_parts(self) -> (String, u64, Arc<RenderImage>) {
        (self.source, self.bytes, self.preview)
    }
}

pub(super) fn load_preset(
    path: &Path,
    extension: &str,
    renderer: SvgRenderer,
) -> std::io::Result<ImportedPreset> {
    let file = std::fs::File::open(path)?;
    if file.metadata()?.len() > MAX_INPUT_BYTES as u64 {
        return Err(std::io::Error::other(
            "OLED media exceeds the local input limit",
        ));
    }
    let mut bytes = Vec::new();
    file.take(MAX_INPUT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    let format = match extension {
        "gif" => ImageFormat::Gif,
        "png" => ImageFormat::Png,
        "jpg" | "jpeg" => ImageFormat::Jpeg,
        "bmp" => ImageFormat::Bmp,
        _ => return Err(std::io::Error::other("Unsupported OLED image format")),
    };
    validate_media(&bytes, format)?;
    let length = bytes.len() as u64;
    let source = format!(
        "data:{};base64,{}",
        format.mime_type(),
        super::base64_encode(&bytes)
    );
    // GPUI's decoder preserves GIF frames and applies image EXIF orientation.
    // The resulting first-frame dimensions are also what the renderer uses.
    let preview = decode_preview(format, bytes, renderer)?;
    Ok(ImportedPreset {
        source,
        bytes: length,
        preview,
    })
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
        async move {
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
            decode_preview(format, bytes, renderer).map_err(Into::into)
        }
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
    let size = preview.size(0);
    if preview.frame_count() == 0 || size.width.0 == 0 || size.height.0 == 0 {
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
struct ImportedImageLeases(HashMap<SharedString, usize>);
impl Global for ImportedImageLeases {}

struct ImportedImageLease {
    source: SharedString,
}

impl ImportedImageLease {
    fn new(source: SharedString, cx: &mut Context<Self>) -> Self {
        *cx.default_global::<ImportedImageLeases>()
            .0
            .entry(source.clone())
            .or_default() += 1;
        cx.on_release(|this, cx| {
            let leases = &mut cx.default_global::<ImportedImageLeases>().0;
            let Some(count) = leases.get_mut(&this.source) else {
                return;
            };
            *count -= 1;
            if *count == 0 {
                leases.remove(&this.source);
                // GPUI retains both pending and completed assets until this
                // call. Removing a pending entry also prevents stale publish.
                cx.remove_asset::<ImportedImageAsset>(&this.source);
            }
        })
        .detach();
        Self { source }
    }
}

pub(super) fn preview_source(id: SharedString, source: SharedString) -> ImageSource {
    if source.starts_with("data:") {
        // A data URL passed directly to img is a URI in GPUI's HTTP loader.
        // Keep custom media in the native asset cache and decode it locally.
        ImageSource::Custom(Arc::new(move |window, cx| {
            // Keyed state follows the displayed image, including content
            // replacement. Two simultaneous previews share one decoded asset.
            let _lease = window.use_keyed_state(id.clone(), cx, |_, cx| {
                ImportedImageLease::new(source.clone(), cx)
            });
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
