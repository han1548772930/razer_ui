//! GenericCamera 1.0.36.1 4cd30/4d010/4d1d0: ISpecifyPropertyPages,
//! native modal property frame, dedicated owner, DPI context and close cleanup.
use super::*;
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM},
    System::{Com::CoTaskMemFree, Ole::OleCreatePropertyFrame, Threading::GetCurrentThreadId},
    UI::{
        HiDpi::{SetThreadDpiAwarenessContext, SetThreadDpiHostingBehavior},
        WindowsAndMessaging::{
            CW_USEDEFAULT, CreateWindowExW, DestroyWindow, EnumThreadWindows, GW_OWNER,
            GWLP_USERDATA, GetWindow, GetWindowLongPtrW, KillTimer, PostMessageW, SetTimer,
            SetWindowLongPtrW, WM_CLOSE,
        },
    },
};
use windows_sys::core::BOOL;

#[repr(C)]
#[derive(Default)]
struct Pages {
    count: u32,
    ids: *mut GUID,
}
impl Drop for Pages {
    fn drop(&mut self) {
        unsafe {
            CoTaskMemFree(self.ids.cast());
        }
    }
}

struct Owner(HWND);
impl Drop for Owner {
    fn drop(&mut self) {
        unsafe {
            KillTimer(self.0, 1);
            SetWindowLongPtrW(self.0, GWLP_USERDATA, 0);
            DestroyWindow(self.0);
        }
    }
}
struct Dpi {
    awareness: *mut c_void,
    hosting: i32,
}
impl Dpi {
    fn enter() -> Self {
        unsafe {
            let hosting = SetThreadDpiHostingBehavior(0);
            let awareness = SetThreadDpiAwarenessContext(-2isize as *mut c_void);
            Self { awareness, hosting }
        }
    }
}
impl Drop for Dpi {
    fn drop(&mut self) {
        unsafe {
            if !self.awareness.is_null() {
                SetThreadDpiAwarenessContext(self.awareness);
            }
            if self.hosting != -1 {
                SetThreadDpiHostingBehavior(self.hosting);
            }
        }
    }
}

struct Validation<'a> {
    check: &'a dyn Fn() -> Result<()>,
    failure: Option<String>,
}
unsafe extern "system" fn close_owned(window: HWND, owner: LPARAM) -> BOOL {
    if unsafe { GetWindow(window, GW_OWNER) } == owner as HWND {
        unsafe {
            PostMessageW(window, WM_CLOSE, 0, 0);
        }
    }
    1
}
unsafe extern "system" fn tick(window: HWND, _: u32, _: usize, _: u32) {
    // The pointer lives until OleCreatePropertyFrame returns on this same STA.
    // Destroying the owner clears the timer before the stack context disappears.
    let pointer = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) } as *mut Validation<'_>;
    if pointer.is_null() {
        return;
    }
    let context = unsafe { &mut *pointer };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (context.check)()));
    match result {
        Ok(Ok(())) if context.failure.is_none() => return,
        Ok(Err(error)) => {
            context.failure.get_or_insert_with(|| format!("{error:#}"));
        }
        Err(_) => {
            context
                .failure
                .get_or_insert_with(|| "camera property validation panicked".into());
        }
        _ => {}
    }
    unsafe {
        EnumThreadWindows(GetCurrentThreadId(), Some(close_owned), window as LPARAM);
    }
}

impl CameraComSession {
    pub(crate) fn show_properties(&mut self, validate: &impl Fn() -> Result<()>) -> Result<()> {
        validate()?;
        let pages_interface = self.filter.query(
            &GUID::from_u128(0xb196b28b_bab4_101a_b69c_00aa00341d07),
            "ISpecifyPropertyPages",
        )?;
        let unknown = self.filter.query(
            &GUID::from_u128(0x00000000_0000_0000_c000_000000000046),
            "IUnknown",
        )?;
        type GetPages = unsafe extern "system" fn(*mut c_void, *mut Pages) -> i32;
        let get: GetPages = unsafe { pages_interface.method(3) };
        let mut pages = Pages::default();
        let hr = unsafe { get(pages_interface.raw(), &mut pages) };
        ensure!(
            hr >= 0 && pages.count > 0 && !pages.ids.is_null(),
            "camera property pages are unavailable (0x{:08x})",
            hr as u32
        );
        // Use the filter's actual OS name as the native window caption.
        #[repr(C)]
        struct FilterInfo {
            name: [u16; 128],
            graph: *mut c_void,
        }
        type QueryInfo = unsafe extern "system" fn(*mut c_void, *mut FilterInfo) -> i32;
        let query: QueryInfo = unsafe { self.filter.method(12) };
        let mut info = FilterInfo {
            name: [0; 128],
            graph: null_mut(),
        };
        let hr = unsafe { query(self.filter.raw(), &mut info) };
        if !info.graph.is_null() {
            drop(unsafe { Com::from_raw(info.graph, "IFilterGraph")? });
        }
        ensure!(
            hr >= 0,
            "IBaseFilter::QueryFilterInfo failed: 0x{:08x}",
            hr as u32
        );
        info.name[127] = 0;
        let _dpi = Dpi::enter();
        let owner = Owner(unsafe {
            CreateWindowExW(
                0,
                wide("STATIC").as_ptr(),
                wide("Host").as_ptr(),
                0,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                10,
                10,
                null_mut(),
                null_mut(),
                null_mut(),
                null(),
            )
        });
        ensure!(
            !owner.0.is_null(),
            "camera property owner creation failed: {}",
            std::io::Error::last_os_error()
        );
        let mut context = Validation {
            check: validate,
            failure: None,
        };
        unsafe {
            SetWindowLongPtrW(
                owner.0,
                GWLP_USERDATA,
                &mut context as *mut Validation<'_> as isize,
            );
        }
        ensure!(
            unsafe { SetTimer(owner.0, 1, 100, Some(tick)) } != 0,
            "camera property cancellation timer failed"
        );
        validate()?;
        let raw_unknown = unknown.raw();
        let hr = unsafe {
            OleCreatePropertyFrame(
                owner.0,
                0,
                0,
                info.name.as_ptr(),
                1,
                &raw_unknown,
                pages.count,
                pages.ids,
                0,
                0,
                null(),
            )
        };
        // Clear the pointer before examining/dropping its stack owner.
        drop(owner);
        if let Some(error) = context.failure {
            anyhow::bail!(error);
        }
        ensure!(
            hr >= 0,
            "OleCreatePropertyFrame failed: 0x{:08x}",
            hr as u32
        );
        validate()
    }
}
