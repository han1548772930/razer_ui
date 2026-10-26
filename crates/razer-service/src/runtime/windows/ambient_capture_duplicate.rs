//! Desktop duplication 0x74490/0x74d70/0x75260. Frame leases and mappings
//! are released on every error before access-loss reconstruction is attempted.
use super::{DesktopPixels, ScreenRequest, copy_bgra, rotate_rgb};
use anyhow::{Context as _, ensure};
use windows::{
    Win32::{
        Foundation::HMODULE,
        Graphics::{
            Direct3D::{
                D3D_DRIVER_TYPE_HARDWARE, D3D_FEATURE_LEVEL_9_1, D3D_FEATURE_LEVEL_10_0,
                D3D_FEATURE_LEVEL_10_1, D3D_FEATURE_LEVEL_11_0,
            },
            Direct3D11::{
                D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_FLAG, D3D11_MAP_READ,
                D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC,
                D3D11_USAGE_STAGING, D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext,
                ID3D11Texture2D,
            },
            Dxgi::{
                Common::{DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_SAMPLE_DESC},
                DXGI_ERROR_ACCESS_LOST, DXGI_ERROR_INVALID_CALL, DXGI_ERROR_WAIT_TIMEOUT,
                DXGI_OUTDUPL_FRAME_INFO, IDXGIDevice, IDXGIOutput1, IDXGIOutputDuplication,
                IDXGIResource,
            },
        },
    },
    core::Interface as _,
};

struct Resources {
    // Fields drop in this order, after every frame lease and mapping ends.
    staging: ID3D11Texture2D,
    context: ID3D11DeviceContext,
    duplication: IDXGIOutputDuplication,
    width: usize,
    height: usize,
    rotation: u16,
}
impl Resources {
    fn open() -> anyhow::Result<Self> {
        let mut device = None;
        let mut context = None;
        // SAFETY: output pointers refer to live Options; the selected feature
        // list is the four-element list at source RVA 0x1ad120.
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_HARDWARE,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_FLAG(0),
                Some(&[
                    D3D_FEATURE_LEVEL_11_0,
                    D3D_FEATURE_LEVEL_10_1,
                    D3D_FEATURE_LEVEL_10_0,
                    D3D_FEATURE_LEVEL_9_1,
                ]),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut context),
            )
        }
        .context("Create Ambient D3D11 device")?;
        let device: ID3D11Device = device.context("Ambient D3D11 returned no device")?;
        let context = context.context("Ambient D3D11 returned no context")?;
        let dxgi: IDXGIDevice = device.cast().context("Query Ambient DXGI device")?;
        // SAFETY: all COM interfaces are retained on this owner thread.
        let adapter = unsafe { dxgi.GetAdapter() }.context("Read Ambient DXGI adapter")?;
        let output = unsafe { adapter.EnumOutputs(0) }.context("Read Ambient primary output")?;
        let output: IDXGIOutput1 = output
            .cast()
            .context("Query Ambient output duplication support")?;
        let duplication =
            unsafe { output.DuplicateOutput(&device) }.context("Duplicate Ambient desktop")?;
        let desc = unsafe { duplication.GetDesc() };
        let rotation = match desc.Rotation.0 {
            0 | 1 => 0,
            2 => 90,
            3 => 180,
            4 => 270,
            _ => anyhow::bail!("Ambient desktop has unknown rotation {}", desc.Rotation.0),
        };
        let (width, height) = if rotation == 90 || rotation == 270 {
            (desc.ModeDesc.Height, desc.ModeDesc.Width)
        } else {
            (desc.ModeDesc.Width, desc.ModeDesc.Height)
        };
        ensure!(
            width > 0 && height > 0,
            "Ambient desktop dimensions are empty"
        );
        let mut staging = None;
        let texture = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_B8G8R8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_STAGING,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            ..Default::default()
        };
        unsafe { device.CreateTexture2D(&texture, None, Some(&mut staging)) }
            .context("Create Ambient staging texture")?;
        Ok(Self {
            staging: staging.context("Ambient D3D11 returned no staging texture")?,
            context,
            duplication,
            width: width as usize,
            height: height as usize,
            rotation,
        })
    }
    fn acquire(&self) -> windows::core::Result<Option<Vec<[u8; 3]>>> {
        let mut info = DXGI_OUTDUPL_FRAME_INFO::default();
        let mut resource: Option<IDXGIResource> = None;
        // A zero timeout is required: the lighting owner must never wait for
        // desktop activity or delay its command cancellation/cleanup loop.
        unsafe {
            self.duplication
                .AcquireNextFrame(0, &mut info, &mut resource)
        }?;
        let lease = FrameLease(&self.duplication);
        let result = (|| -> anyhow::Result<Vec<[u8; 3]>> {
            let texture: ID3D11Texture2D = resource
                .context("Ambient acquired no desktop resource")?
                .cast()?;
            let mut actual = D3D11_TEXTURE2D_DESC::default();
            unsafe { texture.GetDesc(&mut actual) };
            ensure!(
                actual.Width as usize == self.width
                    && actual.Height as usize == self.height
                    && actual.Format == DXGI_FORMAT_B8G8R8A8_UNORM
                    && actual.SampleDesc.Count == 1,
                "Ambient acquired desktop texture differs from its staging dimensions or format"
            );
            unsafe { self.context.CopyResource(&self.staging, &texture) };
            let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
            unsafe {
                self.context
                    .Map(&self.staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))
            }?;
            let mapping = Mapping {
                context: &self.context,
                texture: &self.staging,
            };
            // SAFETY: successful Map holds a staging texture of width/height
            // with its reported RowPitch until the mapping guard is dropped.
            let copied = unsafe {
                copy_bgra(
                    mapped.pData.cast(),
                    mapped.RowPitch as usize,
                    self.width,
                    self.height,
                )
            };
            drop(mapping);
            copied
        })();
        let released = lease.release();
        if let Err(error) = released {
            return Err(error);
        }
        result.map(Some).map_err(|error| {
            windows::core::Error::new(
                windows::core::HRESULT(0x80004005u32 as i32),
                format!("{error:#}"),
            )
        })
    }
}

struct FrameLease<'a>(&'a IDXGIOutputDuplication);
impl FrameLease<'_> {
    fn release(self) -> windows::core::Result<()> {
        let result = unsafe { self.0.ReleaseFrame() };
        std::mem::forget(self);
        result
    }
}
impl Drop for FrameLease<'_> {
    fn drop(&mut self) {
        let _ = unsafe { self.0.ReleaseFrame() };
    }
}
struct Mapping<'a> {
    context: &'a ID3D11DeviceContext,
    texture: &'a ID3D11Texture2D,
}
impl Drop for Mapping<'_> {
    fn drop(&mut self) {
        unsafe { self.context.Unmap(self.texture, 0) };
    }
}

pub(super) struct Desktop {
    resources: Option<Resources>,
    pixels: DesktopPixels,
}
impl Desktop {
    pub(super) fn open() -> anyhow::Result<Self> {
        let resources = Resources::open()?;
        Ok(Self {
            resources: Some(resources),
            pixels: DesktopPixels::Unobserved,
        })
    }
    pub(super) fn refresh(&mut self) -> anyhow::Result<bool> {
        if self.resources.is_none() {
            self.resources = Some(Resources::open()?);
        }
        let mut restored = false;
        loop {
            let resources = self.resources.as_ref().unwrap();
            match resources.acquire() {
                Ok(Some(pixels)) => {
                    let (width, height, pixels) = rotate_rgb(
                        resources.width,
                        resources.height,
                        &pixels,
                        resources.rotation,
                    )?;
                    self.pixels = DesktopPixels::Captured {
                        width,
                        height,
                        rgb: pixels,
                    };
                    return Ok(true);
                }
                Ok(None) => return Ok(false),
                Err(error) if error.code() == DXGI_ERROR_WAIT_TIMEOUT => return Ok(false),
                Err(error)
                    if error.code() == DXGI_ERROR_ACCESS_LOST
                        || error.code() == DXGI_ERROR_INVALID_CALL =>
                {
                    self.resources = None;
                    // 0x74490 replaces and zeroes the capture buffer while
                    // preserving its counter. Old pixels cannot serve a new
                    // effect after that reconstruction.
                    self.pixels = DesktopPixels::Unobserved;
                    // The native loop reconstructs after loss. A single retry
                    // per poll preserves that chain while bounding cancellation.
                    ensure!(
                        !restored,
                        "Ambient desktop access was lost again after reconstruction: {error}"
                    );
                    self.resources =
                        Some(Resources::open().context("Restore Ambient desktop duplication")?);
                    restored = true;
                }
                Err(error) => return Err(error).context("Acquire Ambient desktop frame"),
            }
        }
    }
    pub(super) fn sample(&self, request: ScreenRequest) -> anyhow::Result<Vec<[u8; 3]>> {
        self.pixels.sample(request)
    }
}
