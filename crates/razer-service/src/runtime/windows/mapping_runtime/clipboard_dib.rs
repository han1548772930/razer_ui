//! Current clipboard bitmap read: 22d730 and 2772b0. GDI owns format
//! conversion; the protocol-sized allocation is validated before passing it.
use anyhow::{Context, Result, ensure};
use std::ptr;
use windows_sys::Win32::{Foundation::GetLastError, Graphics::Gdi::*};

struct Bitmap(HBITMAP);
impl Drop for Bitmap {
    fn drop(&mut self) {
        unsafe {
            DeleteObject(self.0);
        }
    }
}
struct Dc(HDC);
impl Drop for Dc {
    fn drop(&mut self) {
        unsafe {
            DeleteDC(self.0);
        }
    }
}
struct Selection {
    dc: HDC,
    previous: HGDIOBJ,
}
impl Drop for Selection {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.previous);
        }
    }
}

pub(super) fn png(bytes: &[u8]) -> Result<Vec<u8>> {
    ensure!(bytes.len() >= 40, "Truncated clipboard DIB header");
    let dword = |offset| u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap());
    let header = dword(0) as usize;
    let width = dword(4) as i32;
    let height = dword(8) as i32;
    let bpp = u16::from_le_bytes(bytes[14..16].try_into().unwrap());
    ensure!(
        header >= 40 && header <= bytes.len(),
        "Invalid clipboard DIB header size"
    );
    ensure!(
        width > 0 && height > 0,
        "Clipboard DIB dimensions are not source-supported"
    );
    ensure!(
        matches!(bpp, 1 | 4 | 8 | 16 | 24 | 32),
        "Invalid clipboard DIB bit depth"
    );
    let source_size =
        u64::from(width as u32) * u64::from(height as u32) * u64::from((bpp / 8).max(1));
    ensure!(
        source_size <= 0x0800_0000,
        "Clipboard DIB exceeds source memory limit"
    );
    let palette = if bpp <= 8 {
        let used = dword(32);
        if used == 0 { 1u32 << bpp } else { used }
    } else if matches!(bpp, 16 | 32) && dword(16) == 3 {
        3
    } else {
        0
    };
    let offset = header
        .checked_add(palette as usize * 4)
        .context("Clipboard DIB palette overflow")?;
    let row = (width as usize * bpp as usize).div_ceil(32) * 4;
    let length = row
        .checked_mul(height as usize)
        .and_then(|length| length.checked_add(offset))
        .context("Clipboard DIB size overflow")?;
    ensure!(length <= bytes.len(), "Truncated clipboard DIB pixels");
    // A u32 allocation meets BITMAPINFO alignment, including the palette.
    let mut aligned = vec![0u32; bytes.len().div_ceil(4)];
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), aligned.as_mut_ptr().cast(), bytes.len());
    }
    let info = aligned.as_ptr().cast::<BITMAPINFO>();
    let target = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: 40,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut pixels = ptr::null_mut();
    let bitmap = Bitmap(unsafe {
        CreateDIBSection(
            ptr::null_mut(),
            &target,
            DIB_RGB_COLORS,
            &mut pixels,
            ptr::null_mut(),
            0,
        )
    });
    ensure!(
        !bitmap.0.is_null() && !pixels.is_null(),
        "Create clipboard DIB section: {}",
        unsafe { GetLastError() }
    );
    let dc = Dc(unsafe { CreateCompatibleDC(ptr::null_mut()) });
    ensure!(!dc.0.is_null(), "Create clipboard bitmap DC: {}", unsafe {
        GetLastError()
    });
    let previous = unsafe { SelectObject(dc.0, bitmap.0) };
    ensure!(
        !previous.is_null() && previous as isize != -1,
        "Select clipboard bitmap failed"
    );
    let selection = Selection { dc: dc.0, previous };
    let copied = unsafe {
        SetDIBitsToDevice(
            dc.0,
            0,
            0,
            width as u32,
            height as u32,
            0,
            0,
            0,
            height as u32,
            aligned.as_ptr().cast::<u8>().add(offset).cast(),
            info,
            DIB_RGB_COLORS,
        )
    };
    ensure!(
        copied == height,
        "Clipboard bitmap conversion returned {copied} of {height} rows"
    );
    drop(selection);
    let mut rgba = unsafe {
        std::slice::from_raw_parts(pixels.cast::<u8>(), width as usize * height as usize * 4)
            .to_vec()
    };
    let opaque = bpp < 32
        || rgba
            .chunks_exact(4)
            .any(|pixel| pixel[..3].iter().any(|c| *c > pixel[3]));
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.swap(0, 2);
        if opaque {
            pixel[3] = 255;
        }
        // GDI's retained N32 pixels are premultiplied; PNG carries straight
        // channels. Zero-alpha pixels have no observable color channels.
        else if pixel[3] != 0 && pixel[3] != 255 {
            let alpha = u32::from(pixel[3]);
            for channel in &mut pixel[..3] {
                *channel = ((u32::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8;
            }
        }
    }
    super::formats::png_bytes(width as u32, height as u32, &rgba)
}
