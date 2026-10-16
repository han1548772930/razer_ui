//! Current SysUtilsNative/Studio screen picker adapter. Native resources belong
//! exclusively to the sampling worker, including the transparent input window.
use crate::screen_color::{Completion, Worker, colorref_rgb, cursor_images};
use anyhow::{Context as _, ensure};
use std::{
    cell::{Cell, RefCell},
    mem::size_of,
    ptr::{null, null_mut},
};
use windows_sys::Win32::{
    Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM},
    Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateDCW, DeleteDC, DeleteObject,
        HBITMAP, HDC, HGDIOBJ, SRCCOPY, SelectObject,
    },
    System::LibraryLoader::GetModuleHandleW,
    UI::{
        HiDpi::{
            DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
            SetThreadDpiAwarenessContext,
        },
        WindowsAndMessaging::{
            CREATESTRUCTW, CreateIconFromResourceEx, CreateWindowExW, DefWindowProcW,
            DestroyCursor, DestroyWindow, DispatchMessageW, GWLP_USERDATA, GetCursor, GetCursorPos,
            GetMessageW, GetSystemMetrics, GetWindowLongPtrW, HCURSOR, HWND_NOTOPMOST,
            HWND_TOPMOST, IsWindow, KillTimer, LWA_ALPHA, LookupIconIdFromDirectoryEx, MSG,
            PostQuitMessage, RegisterClassExW, SM_CXCURSOR, SM_CXVIRTUALSCREEN, SM_CYCURSOR,
            SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SW_SHOW, SWP_NOMOVE,
            SWP_NOSIZE, SetCursor, SetLayeredWindowAttributes, SetTimer, SetWindowLongPtrW,
            SetWindowPos, ShowWindow, TranslateMessage, UnregisterClassW, WM_CLOSE, WM_DESTROY,
            WM_KILLFOCUS, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_NCCREATE, WM_NCDESTROY,
            WM_SETCURSOR, WM_TIMER, WNDCLASSEXW, WS_EX_LAYERED, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
            WS_POPUP,
        },
    },
};

const CANCEL_TIMER: usize = 1;

fn last_error(operation: &str) -> anyhow::Error {
    anyhow::anyhow!("{operation}: {}", std::io::Error::last_os_error())
}

struct DpiContext(Option<DPI_AWARENESS_CONTEXT>);
impl DpiContext {
    fn physical() -> anyhow::Result<Self> {
        let previous =
            unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        ensure!(
            !previous.is_null(),
            "Cannot select physical screen coordinates: {}",
            std::io::Error::last_os_error()
        );
        Ok(Self(Some(previous)))
    }
    fn close(&mut self) -> anyhow::Result<()> {
        if let Some(previous) = self.0 {
            ensure!(
                !unsafe { SetThreadDpiAwarenessContext(previous) }.is_null(),
                "Cannot restore screen coordinate context: {}",
                std::io::Error::last_os_error()
            );
            self.0 = None;
        }
        Ok(())
    }
}
impl Drop for DpiContext {
    fn drop(&mut self) {
        if let Err(error) = self.close() {
            eprintln!("{error:#}");
        }
    }
}

struct Cursor {
    handle: HCURSOR,
    previous: Cell<Option<HCURSOR>>,
}
impl Cursor {
    fn create(bytes: &[u8]) -> anyhow::Result<Self> {
        let images = cursor_images(bytes)?;
        // Rebuild only the native group directory used by LoadCursor(102).
        // Pixel and hotspot bytes remain the requested source CUR payload.
        let mut directory = vec![0, 0, 2, 0];
        directory.extend_from_slice(&u16::try_from(images.len())?.to_le_bytes());
        for (index, image) in images.iter().enumerate() {
            for word in [image.width, image.height * 2, 1, 32] {
                directory.extend_from_slice(&word.to_le_bytes());
            }
            directory.extend_from_slice(&u32::try_from(image.dib.len() + 4)?.to_le_bytes());
            directory.extend_from_slice(&u16::try_from(index + 1)?.to_le_bytes());
        }
        let width = unsafe { GetSystemMetrics(SM_CXCURSOR) };
        let height = unsafe { GetSystemMetrics(SM_CYCURSOR) };
        ensure!(
            width > 0 && height > 0,
            "System cursor dimensions are absent"
        );
        let selected =
            unsafe { LookupIconIdFromDirectoryEx(directory.as_ptr(), 0, width, height, 0) };
        let image = usize::try_from(selected)
            .ok()
            .and_then(|id| id.checked_sub(1))
            .and_then(|id| images.get(id))
            .context("System could not select a source cursor frame")?;
        let mut resource = Vec::with_capacity(image.dib.len() + 4);
        for hotspot in image.hotspot {
            resource.extend_from_slice(&hotspot.to_le_bytes());
        }
        resource.extend_from_slice(image.dib);
        let handle = unsafe {
            CreateIconFromResourceEx(
                resource.as_ptr(),
                u32::try_from(resource.len())?,
                0,
                0x0003_0000,
                width,
                height,
                0,
            )
        };
        ensure!(
            !handle.is_null(),
            "Cannot create source color-picker cursor: {}",
            std::io::Error::last_os_error()
        );
        Ok(Self {
            handle,
            previous: Cell::new(None),
        })
    }
    fn select(&self) {
        let previous = unsafe { SetCursor(self.handle) };
        if self.previous.get().is_none() {
            self.previous.set(Some(previous));
        }
    }
    fn close(&mut self) -> anyhow::Result<()> {
        if !self.handle.is_null() {
            // Unlike LoadCursor's shared resource, CreateIconFromResourceEx
            // gives this owner a cursor that must be deselected before release.
            if unsafe { GetCursor() } == self.handle {
                unsafe {
                    SetCursor(self.previous.get().unwrap_or(null_mut()));
                }
                ensure!(
                    unsafe { GetCursor() } != self.handle,
                    "Cannot restore the cursor before releasing the screen picker"
                );
            }
            ensure!(
                unsafe { DestroyCursor(self.handle) } != 0,
                "Cannot release color-picker cursor: {}",
                std::io::Error::last_os_error()
            );
            self.handle = null_mut();
        }
        Ok(())
    }
}
impl Drop for Cursor {
    fn drop(&mut self) {
        if let Err(error) = self.close() {
            eprintln!("{error:#}");
        }
    }
}

struct Snapshot {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
}
impl Snapshot {
    fn capture(left: i32, top: i32, width: i32, height: i32) -> anyhow::Result<Self> {
        let display_name: Vec<u16> = "DISPLAY\0".encode_utf16().collect();
        let display = unsafe { CreateDCW(display_name.as_ptr(), null(), null(), null()) };
        ensure!(
            !display.is_null(),
            "Cannot open screen snapshot DC: {}",
            std::io::Error::last_os_error()
        );
        let mut snapshot = Self {
            dc: null_mut(),
            bitmap: null_mut(),
            previous: null_mut(),
        };
        let result = (|| -> anyhow::Result<()> {
            snapshot.dc = unsafe { CreateCompatibleDC(display) };
            ensure!(
                !snapshot.dc.is_null(),
                "Cannot create screen snapshot DC: {}",
                std::io::Error::last_os_error()
            );
            snapshot.bitmap = unsafe { CreateCompatibleBitmap(display, width, height) };
            ensure!(
                !snapshot.bitmap.is_null(),
                "Cannot create screen snapshot bitmap: {}",
                std::io::Error::last_os_error()
            );
            snapshot.previous = unsafe { SelectObject(snapshot.dc, snapshot.bitmap) };
            ensure!(
                !snapshot.previous.is_null() && snapshot.previous as isize != -1,
                "Cannot select screen snapshot bitmap: {}",
                std::io::Error::last_os_error()
            );
            ensure!(
                unsafe {
                    BitBlt(
                        snapshot.dc,
                        0,
                        0,
                        width,
                        height,
                        display,
                        left,
                        top,
                        SRCCOPY,
                    )
                } != 0,
                "Cannot capture actual screen pixels: {}",
                std::io::Error::last_os_error()
            );
            Ok(())
        })();
        let release = if unsafe { DeleteDC(display) } == 0 {
            Err(last_error("Release display DC"))
        } else {
            Ok(())
        };
        combine_cleanup(result, [release])?;
        Ok(snapshot)
    }
    fn close(&mut self) -> anyhow::Result<()> {
        let mut failures = Vec::new();
        if !self.dc.is_null() && !self.previous.is_null() && self.previous as isize != -1 {
            let old = unsafe { SelectObject(self.dc, self.previous) };
            if old.is_null() || old as isize == -1 {
                failures.push(last_error("Restore screen snapshot selection"));
            } else {
                self.previous = null_mut();
            }
        }
        if !self.dc.is_null() {
            if unsafe { DeleteDC(self.dc) } == 0 {
                failures.push(last_error("Release screen snapshot DC"));
            } else {
                self.dc = null_mut();
                self.previous = null_mut();
            }
        }
        // Release the DC first even if restoring its selection failed, so the
        // bitmap is no longer selected when DeleteObject tries to release it.
        if !self.bitmap.is_null() {
            if unsafe { DeleteObject(self.bitmap) } == 0 {
                failures.push(last_error("Release screen snapshot bitmap"));
            } else {
                self.bitmap = null_mut();
            }
        }
        ensure!(
            failures.is_empty(),
            "{}",
            failures
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ")
        );
        Ok(())
    }
}
impl Drop for Snapshot {
    fn drop(&mut self) {
        if let Err(error) = self.close() {
            eprintln!("{error:#}");
        }
    }
}

struct State<'a> {
    worker: &'a Worker,
    dc: Cell<HDC>,
    cursor: &'a Cursor,
    selecting: Cell<bool>,
    completion: Cell<Option<Completion>>,
    failure: RefCell<Option<String>>,
}
impl State<'_> {
    fn begin(&self, window: HWND, point: POINT) -> anyhow::Result<()> {
        if !self.selecting.replace(true) {
            self.cursor.select();
            self.sample(point)?;
            // HandleLButtonDown uses HWND_NOTOPMOST; subsequent mouse moves
            // and focus loss restore HWND_TOPMOST in both current originals.
            self.stack(window, HWND_NOTOPMOST)?;
        }
        Ok(())
    }
    fn stack(&self, window: HWND, after: HWND) -> anyhow::Result<()> {
        ensure!(
            unsafe { SetWindowPos(window, after, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE) } != 0,
            "Cannot set source screen-picker stacking: {}",
            std::io::Error::last_os_error()
        );
        Ok(())
    }
    fn sample(&self, point: POINT) -> anyhow::Result<()> {
        ensure!(!self.dc.get().is_null(), "Screen snapshot is unavailable");
        let color =
            unsafe { windows_sys::Win32::Graphics::Gdi::GetPixel(self.dc.get(), point.x, point.y) };
        self.worker.color(colorref_rgb(color)?)
    }
    fn fail(&self, window: HWND, error: anyhow::Error) {
        self.record_failure(error);
        self.destroy(window);
    }
    fn record_failure(&self, error: anyhow::Error) {
        let mut failure = self.failure.borrow_mut();
        if let Some(previous) = failure.as_mut() {
            previous.push_str(&format!("; {error:#}"));
        } else {
            *failure = Some(format!("{error:#}"));
        }
    }
    fn destroy(&self, window: HWND) {
        if unsafe { DestroyWindow(window) } == 0 {
            self.record_failure(last_error("Destroy screen picker window"));
            // Return control to the owner for another cleanup attempt instead
            // of leaving GetMessage blocked after the failed close request.
            unsafe {
                PostQuitMessage(1);
            }
        }
    }
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_NCCREATE {
        let create = unsafe { &*(lparam as *const CREATESTRUCTW) };
        unsafe {
            SetWindowLongPtrW(window, GWLP_USERDATA, create.lpCreateParams as isize);
        }
    }
    let pointer = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) };
    if pointer == 0 {
        return unsafe { DefWindowProcW(window, message, wparam, lparam) };
    }
    // The pinned Box remains on this worker until DestroyWindow returns. Cells
    // allow synchronous nested WM_DESTROY without aliasing mutable references.
    let state = unsafe { &*(pointer as *const State<'_>) };
    match message {
        WM_MOUSEMOVE if state.selecting.get() => {
            let point = POINT {
                x: (lparam as u16 as i16).into(),
                y: ((lparam >> 16) as u16 as i16).into(),
            };
            if let Err(error) = state.sample(point) {
                state.fail(window, error);
                return 0;
            }
            state.cursor.select();
            if let Err(error) = state.stack(window, HWND_TOPMOST) {
                state.fail(window, error);
            }
            0
        }
        WM_LBUTTONDOWN if wparam == 1 => {
            let point = POINT {
                x: (lparam as u16 as i16).into(),
                y: ((lparam >> 16) as u16 as i16).into(),
            };
            if let Err(error) = state.begin(window, point) {
                state.fail(window, error);
            }
            0
        }
        WM_LBUTTONUP => {
            if state.selecting.replace(false) {
                state.completion.set(Some(Completion::Done));
                state.destroy(window);
            }
            0
        }
        WM_CLOSE => {
            state.completion.set(Some(Completion::Canceled));
            state.destroy(window);
            0
        }
        WM_TIMER if wparam == CANCEL_TIMER => {
            if state.worker.canceled() {
                state.completion.set(Some(Completion::Canceled));
                state.destroy(window);
            }
            0
        }
        WM_KILLFOCUS | WM_SETCURSOR => {
            state.cursor.select();
            if message == WM_KILLFOCUS {
                if let Err(error) = state.stack(window, HWND_TOPMOST) {
                    state.fail(window, error);
                }
            }
            1
        }
        WM_DESTROY => {
            unsafe {
                PostQuitMessage(0);
            }
            0
        }
        WM_NCDESTROY => {
            unsafe {
                SetWindowLongPtrW(window, GWLP_USERDATA, 0);
            }
            unsafe { DefWindowProcW(window, message, wparam, lparam) }
        }
        _ => unsafe { DefWindowProcW(window, message, wparam, lparam) },
    }
}

struct Window {
    handle: HWND,
    instance: HINSTANCE,
    class: Vec<u16>,
    registered: bool,
    timer: bool,
}
impl Window {
    fn create(state: &State<'_>, bounds: [i32; 4]) -> anyhow::Result<Self> {
        let instance = unsafe { GetModuleHandleW(null()) };
        ensure!(!instance.is_null(), "Screen picker module is absent");
        let mut owner = Self {
            handle: null_mut(),
            instance,
            class: "RazerUiScreenColorPicker\0".encode_utf16().collect(),
            registered: false,
            timer: false,
        };
        let definition = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: 3,
            lpfnWndProc: Some(window_proc),
            hInstance: instance,
            hbrBackground: 4_usize as _,
            lpszClassName: owner.class.as_ptr(),
            ..unsafe { std::mem::zeroed() }
        };
        ensure!(
            unsafe { RegisterClassExW(&definition) } != 0,
            "Cannot register screen picker window: {}",
            std::io::Error::last_os_error()
        );
        owner.registered = true;
        let title: Vec<u16> = "RzColorPickerWindow\0".encode_utf16().collect();
        owner.handle = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_TOPMOST,
                owner.class.as_ptr(),
                title.as_ptr(),
                WS_POPUP,
                bounds[0],
                bounds[1],
                bounds[2],
                bounds[3],
                null_mut(),
                null_mut(),
                instance,
                state as *const State<'_> as _,
            )
        };
        ensure!(
            !owner.handle.is_null(),
            "Cannot create screen picker window: {}",
            std::io::Error::last_os_error()
        );
        ensure!(
            unsafe { SetLayeredWindowAttributes(owner.handle, 0, 1, LWA_ALPHA) } != 0,
            "Cannot set source picker transparency: {}",
            std::io::Error::last_os_error()
        );
        unsafe {
            ShowWindow(owner.handle, SW_SHOW);
        }
        ensure!(
            unsafe {
                SetWindowPos(
                    owner.handle,
                    HWND_TOPMOST,
                    bounds[0],
                    bounds[1],
                    bounds[2],
                    bounds[3],
                    0,
                )
            } != 0,
            "Cannot position screen picker: {}",
            std::io::Error::last_os_error()
        );
        ensure!(
            unsafe { SetTimer(owner.handle, CANCEL_TIMER, 16, None) } != 0,
            "Cannot install screen picker cancellation wakeup: {}",
            std::io::Error::last_os_error()
        );
        owner.timer = true;
        Ok(owner)
    }
    fn close(&mut self) -> anyhow::Result<()> {
        let mut failures = Vec::new();
        if !self.handle.is_null() {
            if unsafe { IsWindow(self.handle) } != 0 {
                if self.timer {
                    unsafe {
                        KillTimer(self.handle, CANCEL_TIMER);
                    }
                }
                if unsafe { DestroyWindow(self.handle) } == 0 {
                    failures.push(last_error("Destroy screen picker window"));
                    unsafe {
                        SetWindowLongPtrW(self.handle, GWLP_USERDATA, 0);
                    }
                }
            }
            if unsafe { IsWindow(self.handle) } == 0 {
                self.handle = null_mut();
                self.timer = false;
            }
        }
        if self.registered {
            if unsafe { UnregisterClassW(self.class.as_ptr(), self.instance) } == 0 {
                failures.push(last_error("Unregister screen picker window"));
            } else {
                self.registered = false;
            }
        }
        ensure!(
            failures.is_empty(),
            "{}",
            failures
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ")
        );
        Ok(())
    }
}
impl Drop for Window {
    fn drop(&mut self) {
        if let Err(error) = self.close() {
            eprintln!("{error:#}");
        }
    }
}

fn combine_cleanup<T, const N: usize>(
    outcome: anyhow::Result<T>,
    cleanup: [anyhow::Result<()>; N],
) -> anyhow::Result<T> {
    let failures = cleanup
        .into_iter()
        .filter_map(Result::err)
        .map(|error| format!("{error:#}"))
        .collect::<Vec<_>>();
    if failures.is_empty() {
        return outcome;
    }
    let cleanup = format!("Screen picker cleanup failed: {}", failures.join("; "));
    match outcome {
        Ok(_) => anyhow::bail!(cleanup),
        Err(error) => Err(anyhow::anyhow!("{error:#}; {cleanup}")),
    }
}

pub(crate) fn run(cursor_bytes: &[u8], worker: &Worker) -> anyhow::Result<Completion> {
    let mut dpi = DpiContext::physical()?;
    let mut cursor = Cursor::create(cursor_bytes)?;
    if worker.canceled() {
        return combine_cleanup(Ok(Completion::Canceled), [cursor.close(), dpi.close()]);
    }
    let state = Box::new(State {
        worker,
        dc: Cell::new(null_mut()),
        cursor: &cursor,
        selecting: Cell::new(false),
        completion: Cell::new(None),
        failure: RefCell::new(None),
    });
    let bounds = unsafe {
        [
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        ]
    };
    ensure!(
        bounds[2] > 0 && bounds[3] > 0,
        "Virtual screen bounds are empty"
    );
    let mut window = Window::create(&state, bounds)?;
    let mut snapshot = Snapshot::capture(bounds[0], bounds[1], bounds[2], bounds[3])?;
    state.dc.set(snapshot.dc);
    let outcome = (|| -> anyhow::Result<Completion> {
        if worker.canceled() {
            return Ok(Completion::Canceled);
        }
        let mut point = POINT { x: 0, y: 0 };
        ensure!(
            unsafe { GetCursorPos(&mut point) } != 0,
            "Cannot observe picker cursor position: {}",
            std::io::Error::last_os_error()
        );
        // WM_START_COLOR_PICKER passes GetCursorPos directly in both originals;
        // subsequent WM_MOUSEMOVE coordinates already belong to the window.
        state.begin(window.handle, point)?;
        let mut message: MSG = unsafe { std::mem::zeroed() };
        loop {
            let received = unsafe { GetMessageW(&mut message, null_mut(), 0, 0) };
            ensure!(
                received != -1,
                "Screen picker message read failed: {}",
                std::io::Error::last_os_error()
            );
            if received == 0 {
                break;
            }
            unsafe {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        if let Some(error) = state.failure.borrow_mut().take() {
            anyhow::bail!(error);
        }
        state
            .completion
            .get()
            .context("Screen picker stopped without completion or cancellation")
    })();
    let window_cleanup = window.close();
    state.dc.set(null_mut());
    let snapshot_cleanup = snapshot.close();
    drop(state);
    let cursor_cleanup = cursor.close();
    let dpi_cleanup = dpi.close();
    combine_cleanup(
        outcome,
        [
            window_cleanup,
            snapshot_cleanup,
            cursor_cleanup,
            dpi_cleanup,
        ],
    )
}
