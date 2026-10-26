//! 4.0.55.0 capture singleton 0x74010 and shared 30 ms gate 0x741e0.
//! All resources stay on the retained lighting owner; observations and cached
//! renderer output are independent facts.
use anyhow::{Context as _, ensure};
use razer_device::lighting_engine::renderer::{
    ScreenBackend, ScreenCaptureStatus, ScreenObservation, ScreenRequest,
};
use std::{collections::BTreeMap, marker::PhantomData, rc::Rc};

#[path = "ambient_capture_duplicate.rs"]
mod duplicate;
#[path = "ambient_capture_dx9.rs"]
mod dx9;
#[path = "ambient_capture_gdi.rs"]
mod gdi;

enum Backend {
    Duplicate(duplicate::Desktop),
    Dx9(dx9::Desktop),
    Gdi(gdi::Desktop),
    #[cfg(test)]
    Scripted(tests::Scripted),
}
impl Backend {
    fn refresh(&mut self) -> anyhow::Result<bool> {
        match self {
            Self::Duplicate(capture) => capture.refresh(),
            Self::Dx9(capture) => capture.refresh(),
            Self::Gdi(_) => Ok(true),
            #[cfg(test)]
            Self::Scripted(capture) => capture.refresh(),
        }
    }
    fn sample(&mut self, request: ScreenRequest) -> anyhow::Result<Vec<[u8; 3]>> {
        match self {
            Self::Duplicate(capture) => capture.sample(request),
            Self::Dx9(capture) => capture.sample(request),
            Self::Gdi(capture) => capture.sample(request),
            #[cfg(test)]
            Self::Scripted(capture) => capture.sample(request),
        }
    }
    fn close(&mut self) -> anyhow::Result<()> {
        match self {
            Self::Gdi(capture) => capture.close(),
            _ => Ok(()),
        }
    }
}

pub(super) struct Capture {
    backend: Option<Backend>,
    status: ScreenCaptureStatus,
    current_frame_id: Option<u32>,
    next_frame_id: u32,
    last_attempt_ms: Option<u64>,
    delivered_frames: BTreeMap<u32, u32>,
    backend_error: Option<String>,
    sample_errors: BTreeMap<u32, String>,
    _owner_thread: PhantomData<Rc<()>>,
}
impl Capture {
    pub(super) fn open() -> anyhow::Result<Self> {
        let (kind, backend) = if supports_duplication() {
            match duplicate::Desktop::open() {
                Ok(desktop) => (ScreenBackend::DesktopDuplication, Backend::Duplicate(desktop)),
                Err(duplicate_error) => (
                    ScreenBackend::Direct3D9,
                    Backend::Dx9(dx9::Desktop::open().with_context(|| {
                        format!("Ambient desktop duplication failed ({duplicate_error:#}); initialize Direct3D 9")
                    })?),
                ),
            }
        } else {
            (ScreenBackend::Gdi, Backend::Gdi(gdi::Desktop::open()?))
        };
        Ok(Self::new(kind, backend))
    }
    fn new(kind: ScreenBackend, backend: Backend) -> Self {
        Self {
            backend: Some(backend),
            status: ScreenCaptureStatus {
                backend: kind,
                frame_id: None,
                refreshed_at_ms: None,
                last_error: None,
                released: false,
            },
            next_frame_id: u32::from(kind != ScreenBackend::Gdi),
            current_frame_id: None,
            last_attempt_ms: None,
            delivered_frames: BTreeMap::new(),
            backend_error: None,
            sample_errors: BTreeMap::new(),
            _owner_thread: PhantomData,
        }
    }
    pub(super) fn status(&self) -> ScreenCaptureStatus {
        self.status.clone()
    }
    pub(super) fn retain_effects(&mut self, handles: &[u32]) {
        self.delivered_frames
            .retain(|handle, _| handles.contains(handle));
        self.sample_errors
            .retain(|handle, _| handles.contains(handle));
        self.publish_errors();
    }
    fn publish_errors(&mut self) {
        let errors = self
            .backend_error
            .iter()
            .cloned()
            .chain(
                self.sample_errors
                    .iter()
                    .map(|(handle, error)| format!("Effect {handle}: {error}")),
            )
            .collect::<Vec<_>>();
        self.status.last_error = (!errors.is_empty()).then(|| errors.join("; "));
    }
    pub(super) fn observe(
        &mut self,
        requests: &[(u32, ScreenRequest)],
        now_ms: u64,
    ) -> BTreeMap<u32, ScreenObservation> {
        let mut output = BTreeMap::new();
        if requests.is_empty() {
            return output;
        }
        if self.status.backend == ScreenBackend::Gdi && self.backend.is_some() {
            return self.observe_gdi(requests, now_ms);
        }
        let Some(backend) = self.backend.as_mut() else {
            let error = "Ambient capture was released".to_owned();
            self.backend_error = Some(error.clone());
            self.publish_errors();
            return requests
                .iter()
                .map(|(id, _)| {
                    (
                        *id,
                        ScreenObservation::Failed {
                            frame_id: None,
                            error: error.clone(),
                        },
                    )
                })
                .collect();
        };
        if self
            .last_attempt_ms
            .is_none_or(|at| now_ms.saturating_sub(at) >= 30)
        {
            self.last_attempt_ms = Some(now_ms);
            match backend.refresh() {
                Ok(true) => {
                    self.current_frame_id = Some(self.next_frame_id);
                    self.next_frame_id = self.next_frame_id.wrapping_add(1);
                    self.status.frame_id = self.current_frame_id;
                    self.status.refreshed_at_ms = Some(now_ms);
                    self.backend_error = None;
                }
                Ok(false) => {}
                Err(error) => {
                    let error = format!("{error:#}");
                    self.backend_error = Some(error.clone());
                    self.publish_errors();
                    return requests
                        .iter()
                        .map(|(id, _)| {
                            (
                                *id,
                                ScreenObservation::Failed {
                                    frame_id: None,
                                    error: error.clone(),
                                },
                            )
                        })
                        .collect();
                }
            }
        }
        for (handle, request) in requests {
            let observation = if let Some(frame_id) = self.current_frame_id {
                if self.delivered_frames.get(handle) == Some(&frame_id) {
                    output.insert(*handle, ScreenObservation::Unchanged);
                    continue;
                }
                // 0x5d0b0 commits this identity before sample, including a
                // failed sample. Each effect consumes a capture ID at most once.
                self.delivered_frames.insert(*handle, frame_id);
                match backend.sample(*request) {
                    Ok(pixels) => {
                        self.sample_errors.remove(handle);
                        ScreenObservation::Captured { frame_id, pixels }
                    }
                    Err(error) => {
                        let error = format!("{error:#}");
                        self.sample_errors.insert(*handle, error.clone());
                        ScreenObservation::Failed {
                            frame_id: Some(frame_id),
                            error,
                        }
                    }
                }
            } else {
                // 0x74490 zeroes the native capture buffer; 0x5cf80 zeroes
                // Ambient's output cache. Before acquisition, the source can
                // therefore emit black while no desktop pixels are observed.
                ScreenObservation::Unchanged
            };
            output.insert(*handle, observation);
        }
        self.publish_errors();
        output
    }
    fn observe_gdi(
        &mut self,
        requests: &[(u32, ScreenRequest)],
        now_ms: u64,
    ) -> BTreeMap<u32, ScreenObservation> {
        let mut output = BTreeMap::new();
        let backend = self.backend.as_mut().unwrap();
        // 0x82260 returns the preincrement counter to the consumer whose
        // 0x741e0 gate passes. Subsequent consumers see the incremented value
        // even inside the same 30 ms interval. Preserve renderer traversal
        // order rather than assigning one identity to the whole batch.
        for (handle, request) in requests {
            let frame_id = self.next_frame_id;
            if self
                .last_attempt_ms
                .is_none_or(|at| now_ms.saturating_sub(at) >= 30)
            {
                self.last_attempt_ms = Some(now_ms);
                match backend.refresh() {
                    Ok(true) => {
                        self.next_frame_id = self.next_frame_id.wrapping_add(1);
                        self.backend_error = None;
                    }
                    Ok(false) => {}
                    Err(error) => {
                        let error = format!("{error:#}");
                        self.backend_error = Some(error.clone());
                        output.insert(
                            *handle,
                            ScreenObservation::Failed {
                                frame_id: None,
                                error,
                            },
                        );
                        continue;
                    }
                }
            }
            if self.delivered_frames.get(handle) == Some(&frame_id) {
                output.insert(*handle, ScreenObservation::Unchanged);
                continue;
            }
            self.delivered_frames.insert(*handle, frame_id);
            let observation = match backend.sample(*request) {
                Ok(pixels) => {
                    self.status.frame_id = Some(frame_id);
                    self.status.refreshed_at_ms = Some(now_ms);
                    self.sample_errors.remove(handle);
                    ScreenObservation::Captured { frame_id, pixels }
                }
                Err(error) => {
                    let error = format!("{error:#}");
                    self.sample_errors.insert(*handle, error.clone());
                    ScreenObservation::Failed {
                        frame_id: Some(frame_id),
                        error,
                    }
                }
            };
            output.insert(*handle, observation);
        }
        self.publish_errors();
        output
    }
    pub(super) fn close(&mut self) -> anyhow::Result<()> {
        if let Some(backend) = self.backend.as_mut() {
            if let Err(error) = backend.close() {
                self.backend_error = Some(format!("{error:#}"));
                self.publish_errors();
                return Err(error);
            }
        }
        self.backend = None;
        self.delivered_frames.clear();
        self.sample_errors.clear();
        self.publish_errors();
        self.status.released = true;
        Ok(())
    }
}
impl Drop for Capture {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

/// Native reconstruction allocates a zero capture buffer, but those bytes do
/// not constitute observed desktop pixels. Effects own their cached output.
enum DesktopPixels {
    Unobserved,
    Captured {
        width: usize,
        height: usize,
        rgb: Vec<[u8; 3]>,
    },
}
impl DesktopPixels {
    fn sample(&self, request: ScreenRequest) -> anyhow::Result<Vec<[u8; 3]>> {
        match self {
            Self::Unobserved => anyhow::bail!(
                "Ambient desktop pixels are unobserved after initialization or reconstruction"
            ),
            Self::Captured { width, height, rgb } => request.sample_rgb(*width, *height, rgb),
        }
    }
}

fn supports_duplication() -> bool {
    use windows_sys::Win32::System::SystemInformation::{
        OSVERSIONINFOEXW, VER_MAJORVERSION, VER_MINORVERSION, VER_SERVICEPACKMAJOR,
        VerSetConditionMask, VerifyVersionInfoW,
    };
    let mut version = OSVERSIONINFOEXW {
        dwOSVersionInfoSize: std::mem::size_of::<OSVERSIONINFOEXW>() as u32,
        dwMajorVersion: 6,
        dwMinorVersion: 2,
        ..Default::default()
    };
    // SAFETY: the complete writable structure remains valid for the call.
    // Condition 3 is VER_GREATER_EQUAL, as at native 0x74010.
    unsafe {
        let mask = VerSetConditionMask(0, VER_MAJORVERSION, 3);
        let mask = VerSetConditionMask(mask, VER_MINORVERSION, 3);
        let mask = VerSetConditionMask(mask, VER_SERVICEPACKMAJOR, 3);
        VerifyVersionInfoW(
            &mut version,
            VER_MAJORVERSION | VER_MINORVERSION | VER_SERVICEPACKMAJOR,
            mask,
        ) != 0
    }
}

/// The mapped GPU region is borrowed only until Unmap/UnlockRect. Callers
/// validate the native pitch and keep its resource alive for this copy.
unsafe fn copy_bgra(
    pointer: *const u8,
    pitch: usize,
    width: usize,
    height: usize,
) -> anyhow::Result<Vec<[u8; 3]>> {
    ensure!(!pointer.is_null(), "Ambient mapped desktop pointer is null");
    let bytes = width.checked_mul(4).context("Ambient row size overflow")?;
    ensure!(
        width > 0 && height > 0 && pitch >= bytes,
        "Ambient desktop pitch or dimensions are invalid"
    );
    let count = width
        .checked_mul(height)
        .context("Ambient desktop size overflow")?;
    ensure!(
        pitch
            .checked_mul(height)
            .is_some_and(|n| n <= isize::MAX as usize),
        "Ambient mapped desktop extent overflow"
    );
    let mut pixels = Vec::with_capacity(count);
    for row in 0..height {
        // SAFETY: the owner supplies a successfully mapped region with at
        // least height rows of native pitch bytes, retained for this copy.
        let line = unsafe { std::slice::from_raw_parts(pointer.add(row * pitch), bytes) };
        pixels.extend(line.chunks_exact(4).map(|p| [p[2], p[1], p[0]]));
    }
    Ok(pixels)
}

fn rotate_rgb(
    width: usize,
    height: usize,
    pixels: &[[u8; 3]],
    angle: u16,
) -> anyhow::Result<(usize, usize, Vec<[u8; 3]>)> {
    ensure!(
        width.checked_mul(height) == Some(pixels.len()),
        "Ambient rotation dimensions differ"
    );
    ensure!(
        [0, 90, 180, 270].contains(&angle),
        "Ambient desktop rotation is unknown"
    );
    let (out_width, out_height) = if angle == 90 || angle == 270 {
        (height, width)
    } else {
        (width, height)
    };
    let output = (0..out_height)
        .flat_map(|y| {
            (0..out_width).map(move |x| {
                let (sx, sy) = match angle {
                    90 => (y, height - 1 - x),
                    180 => (width - 1 - x, height - 1 - y),
                    270 => (width - 1 - y, x),
                    _ => (x, y),
                };
                pixels[sy * width + sx]
            })
        })
        .collect();
    Ok((out_width, out_height, output))
}

#[cfg(test)]
#[path = "ambient_capture_tests.rs"]
mod tests;
