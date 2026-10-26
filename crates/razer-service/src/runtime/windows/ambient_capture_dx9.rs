//! Direct3D 9 fallback 0x81920/0x81ba0/0x81d40. Source pixels are copied
//! before UnlockRect and all three COM resources are dropped before recovery.
use super::{DesktopPixels, ScreenRequest, copy_bgra};
use anyhow::{Context as _, ensure};
use windows::Win32::{
    Foundation::HWND,
    Graphics::Direct3D9::{
        D3DDEVTYPE_HAL, D3DDISPLAYMODE, D3DFMT_A8R8G8B8, D3DFMT_UNKNOWN, D3DLOCKED_RECT,
        D3DPOOL_SYSTEMMEM, D3DPRESENT_PARAMETERS, D3DSWAPEFFECT_DISCARD, Direct3DCreate9,
        IDirect3D9, IDirect3DDevice9, IDirect3DSurface9,
    },
};

struct Resources {
    surface: IDirect3DSurface9,
    device: IDirect3DDevice9,
    _d3d: IDirect3D9,
    width: usize,
    height: usize,
}
impl Resources {
    fn open() -> anyhow::Result<Self> {
        // SAFETY: source SDK version is 32; all resources remain on the worker.
        let d3d = unsafe { Direct3DCreate9(32) }.context("Create Ambient Direct3D 9")?;
        let mut mode = D3DDISPLAYMODE::default();
        unsafe { d3d.GetAdapterDisplayMode(0, &mut mode) }
            .context("Read Ambient Direct3D display mode")?;
        ensure!(
            mode.Width > 0 && mode.Height > 0,
            "Ambient Direct3D desktop dimensions are empty"
        );
        let mut present = D3DPRESENT_PARAMETERS {
            BackBufferWidth: mode.Width,
            BackBufferHeight: mode.Height,
            BackBufferFormat: D3DFMT_UNKNOWN,
            BackBufferCount: 1,
            SwapEffect: D3DSWAPEFFECT_DISCARD,
            Windowed: true.into(),
            ..Default::default()
        };
        let mut device = None;
        unsafe {
            d3d.CreateDevice(
                0,
                D3DDEVTYPE_HAL,
                HWND::default(),
                32,
                &mut present,
                &mut device,
            )
        }
        .context("Create Ambient Direct3D 9 device")?;
        let device = device.context("Ambient Direct3D 9 returned no device")?;
        let mut surface = None;
        unsafe {
            device.CreateOffscreenPlainSurface(
                mode.Width,
                mode.Height,
                D3DFMT_A8R8G8B8,
                D3DPOOL_SYSTEMMEM,
                &mut surface,
                std::ptr::null_mut(),
            )
        }
        .context("Create Ambient Direct3D 9 readback surface")?;
        Ok(Self {
            surface: surface.context("Ambient Direct3D 9 returned no surface")?,
            device,
            _d3d: d3d,
            width: mode.Width as usize,
            height: mode.Height as usize,
        })
    }
    fn read(&self) -> anyhow::Result<Vec<[u8; 3]>> {
        unsafe { self.device.GetFrontBufferData(0, &self.surface) }
            .context("Read Ambient front buffer")?;
        let mut locked = D3DLOCKED_RECT::default();
        unsafe { self.surface.LockRect(&mut locked, std::ptr::null(), 0) }
            .context("Lock Ambient front buffer")?;
        let guard = Locked(&self.surface);
        let result = (|| {
            ensure!(
                locked.Pitch > 0,
                "Ambient Direct3D 9 returned a nonpositive pitch"
            );
            // SAFETY: the complete system-memory surface remains locked and
            // alive while its dimensions and reported pitch are copied.
            unsafe {
                copy_bgra(
                    locked.pBits.cast(),
                    locked.Pitch as usize,
                    self.width,
                    self.height,
                )
            }
        })();
        let unlocked = guard.unlock();
        match (result, unlocked) {
            (Ok(pixels), Ok(())) => Ok(pixels),
            (Err(error), Ok(())) => Err(error),
            (Ok(_), Err(error)) => Err(error).context("Unlock Ambient front buffer"),
            (Err(error), Err(unlock)) => {
                Err(error).context(format!("Unlock Ambient front buffer also failed: {unlock}"))
            }
        }
    }
}
struct Locked<'a>(&'a IDirect3DSurface9);
impl Locked<'_> {
    fn unlock(self) -> windows::core::Result<()> {
        let result = unsafe { self.0.UnlockRect() };
        std::mem::forget(self);
        result
    }
}
impl Drop for Locked<'_> {
    fn drop(&mut self) {
        let _ = unsafe { self.0.UnlockRect() };
    }
}

pub(super) struct Desktop {
    resources: Option<Resources>,
    pixels: DesktopPixels,
}
impl Desktop {
    pub(super) fn open() -> anyhow::Result<Self> {
        Ok(Self {
            resources: Some(Resources::open()?),
            pixels: DesktopPixels::Unobserved,
        })
    }
    pub(super) fn refresh(&mut self) -> anyhow::Result<bool> {
        if self.resources.is_none() {
            self.resources = Some(Resources::open()?);
        }
        let resources = self.resources.as_ref().unwrap();
        match resources.read() {
            Ok(pixels) => {
                self.pixels = DesktopPixels::Captured {
                    width: resources.width,
                    height: resources.height,
                    rgb: pixels,
                };
                Ok(true)
            }
            Err(error) => {
                self.resources = None;
                self.pixels = DesktopPixels::Unobserved;
                let recovered = Resources::open();
                match recovered {
                    Ok(resources) => {
                        self.resources = Some(resources);
                        Err(error)
                    }
                    Err(recovery) => Err(error).context(format!(
                        "Restore Ambient Direct3D 9 also failed: {recovery:#}"
                    )),
                }
            }
        }
    }
    pub(super) fn sample(&self, request: ScreenRequest) -> anyhow::Result<Vec<[u8; 3]>> {
        self.pixels.sample(request)
    }
}
