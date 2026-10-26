//! GDI branch 0x821e0/0x82290 uses HALFTONE StretchBlt, rather than the
//! Direct3D paths' nearest resize. Selected objects never outlive their DC.
use super::ScreenRequest;
use anyhow::{Context as _, ensure};
use windows::{
    Win32::Graphics::Gdi::{
        CLR_INVALID, CreateCompatibleBitmap, CreateCompatibleDC, CreateDCW, DeleteDC, DeleteObject,
        GetPixel, HALFTONE, HBITMAP, HDC, HGDIOBJ, SRCCOPY, SelectObject, SetStretchBltMode,
        StretchBlt,
    },
    core::{PCWSTR, w},
};

pub(super) struct Desktop {
    display: Option<HDC>,
    memory: Option<HDC>,
    retained_bitmap: Option<HBITMAP>,
    width: usize,
    height: usize,
}
impl Desktop {
    pub(super) fn open() -> anyhow::Result<Self> {
        // SAFETY: metrics do not acquire resources or execute vendor code.
        let (width, height) = unsafe {
            use windows_sys::Win32::UI::WindowsAndMessaging::{
                GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN,
            };
            (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN))
        };
        ensure!(
            width > 0 && height > 0,
            "Ambient GDI desktop dimensions are empty"
        );
        // SAFETY: constant terminated driver name; no borrowed DEVMODE.
        let display = unsafe { CreateDCW(w!("DISPLAY"), PCWSTR::null(), PCWSTR::null(), None) };
        ensure!(
            !display.0.is_null(),
            "Create Ambient GDI display context failed"
        );
        let mut desktop = Self {
            display: Some(display),
            memory: None,
            retained_bitmap: None,
            width: width as usize,
            height: height as usize,
        };
        desktop.memory()?;
        Ok(desktop)
    }
    fn memory(&mut self) -> anyhow::Result<HDC> {
        ensure!(
            self.retained_bitmap.is_none(),
            "Ambient GDI bitmap cleanup is pending"
        );
        if let Some(memory) = self.memory {
            return Ok(memory);
        }
        let display = self
            .display
            .context("Ambient GDI display context was released")?;
        let memory = unsafe { CreateCompatibleDC(Some(display)) };
        ensure!(
            !memory.0.is_null(),
            "Create Ambient GDI memory context failed"
        );
        self.memory = Some(memory);
        Ok(memory)
    }
    pub(super) fn sample(&mut self, request: ScreenRequest) -> anyhow::Result<Vec<[u8; 3]>> {
        let [left, top, right, bottom] = request.pixel_rect(self.width, self.height)?;
        let columns =
            i32::try_from(request.columns).context("Ambient GDI columns exceed native size")?;
        let rows = i32::try_from(request.rows).context("Ambient GDI rows exceed native size")?;
        ensure!(columns > 0 && rows > 0, "Ambient GDI request is empty");
        let display = self
            .display
            .context("Ambient GDI display context was released")?;
        let memory = self.memory()?;
        let bitmap = unsafe { CreateCompatibleBitmap(display, columns, rows) };
        ensure!(
            !bitmap.0.is_null(),
            "Create Ambient GDI sample bitmap failed"
        );
        self.retained_bitmap = Some(bitmap);
        let previous = unsafe { SelectObject(memory, HGDIOBJ(bitmap.0)) };
        if previous.0.is_null() || previous.0 as isize == -1 {
            if unsafe { DeleteObject(HGDIOBJ(bitmap.0)) }.as_bool() {
                self.retained_bitmap = None;
            }
            anyhow::bail!("Select Ambient GDI sample bitmap failed");
        }
        let result = (|| -> anyhow::Result<Vec<[u8; 3]>> {
            ensure!(
                unsafe { SetStretchBltMode(memory, HALFTONE) } != 0,
                "Set Ambient GDI HALFTONE mode failed"
            );
            ensure!(
                unsafe {
                    StretchBlt(
                        memory,
                        0,
                        0,
                        columns,
                        rows,
                        Some(display),
                        left as i32,
                        top as i32,
                        (right - left) as i32,
                        (bottom - top) as i32,
                        SRCCOPY,
                    )
                }
                .as_bool(),
                "Stretch Ambient GDI desktop crop failed"
            );
            let mut output = Vec::with_capacity(request.rows * request.columns);
            for row in 0..rows {
                for column in 0..columns {
                    let color = unsafe { GetPixel(memory, column, row) }.0;
                    ensure!(color != CLR_INVALID, "Read Ambient GDI sample pixel failed");
                    output.push([color as u8, (color >> 8) as u8, (color >> 16) as u8]);
                }
            }
            Ok(output)
        })();
        let restored = unsafe { SelectObject(memory, previous) };
        let restored = !restored.0.is_null() && restored.0 as isize != -1;
        let mut cleanup_errors = Vec::new();
        if !restored {
            cleanup_errors.push("Restore Ambient GDI selected object failed");
            // Deleting the DC releases its selection, allowing safe bitmap
            // deletion. A failed DC release retains both owned handles.
            if unsafe { DeleteDC(memory) }.as_bool() {
                self.memory = None;
            } else {
                cleanup_errors.push("Release Ambient GDI memory context failed");
            }
        }
        if restored || self.memory.is_none() {
            if unsafe { DeleteObject(HGDIOBJ(bitmap.0)) }.as_bool() {
                self.retained_bitmap = None;
            } else {
                cleanup_errors.push("Delete Ambient GDI sample bitmap failed");
            }
        }
        if !cleanup_errors.is_empty() {
            return match result {
                Ok(_) => Err(anyhow::anyhow!(cleanup_errors.join("; "))),
                Err(error) => Err(error).context(cleanup_errors.join("; ")),
            };
        }
        result
    }
    pub(super) fn close(&mut self) -> anyhow::Result<()> {
        let mut errors = Vec::new();
        if let Some(memory) = self.memory {
            if unsafe { DeleteDC(memory) }.as_bool() {
                self.memory = None;
            } else {
                errors.push("Release Ambient GDI memory context failed");
            }
        }
        if self.memory.is_none() {
            if let Some(bitmap) = self.retained_bitmap {
                if unsafe { DeleteObject(HGDIOBJ(bitmap.0)) }.as_bool() {
                    self.retained_bitmap = None;
                } else {
                    errors.push("Release Ambient GDI retained bitmap failed");
                }
            }
        }
        if let Some(display) = self.display {
            if unsafe { DeleteDC(display) }.as_bool() {
                self.display = None;
            } else {
                errors.push("Release Ambient GDI display context failed");
            }
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
}
impl Drop for Desktop {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
