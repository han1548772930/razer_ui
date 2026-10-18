//! Worker-owned local hardware monitor. No vendor code runs during static tests.
use super::{CALLBACK_TIMEOUT, EngineLibrary, NativeRuntime};
use anyhow::{Context as _, bail, ensure};
use razer_device::system_monitor::{
    self, CALLBACK_INTERVAL_MS, HARDWARE_MASK, MonitorObservation, SystemSample,
};
use razer_platform::native_paths::SystemMonitorPaths;
use std::{
    collections::VecDeque,
    ffi::{CStr, CString, c_char},
    mem::ManuallyDrop,
    ptr,
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};
#[cfg(target_pointer_width = "64")]
use windows_sys::Win32::UI::WindowsAndMessaging::GetClassLongPtrW;
#[cfg(target_pointer_width = "32")]
use windows_sys::Win32::UI::WindowsAndMessaging::GetClassLongW;
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, HINSTANCE, WAIT_OBJECT_0, WAIT_TIMEOUT},
    System::Threading::{GetCurrentProcessId, OpenThread, THREAD_SYNCHRONIZE, WaitForSingleObject},
    UI::WindowsAndMessaging::{
        FindWindowExW, GCLP_HMODULE, GetWindowThreadProcessId, UnregisterClassW,
    },
};

const WINDOW_CLASS: &str = "RazerSystemCommonHidenWindow";
const MAX_POLL_SAMPLES: usize = 256;
const WINDOW_READY_TIMEOUT: Duration = Duration::from_secs(3);
const WINDOW_STOP_TIMEOUT_MS: u32 = 5000;

// The native callback has no context argument. Its one process-local lease is
// replaced only after the preceding DLL's window thread has been joined.
static CALLBACK_OWNER: Mutex<Option<Arc<Mutex<CallbackQueue>>>> = Mutex::new(None);
static LAUNCH_INITIALIZED: Mutex<Option<mpsc::Sender<()>>> = Mutex::new(None);
static LAUNCH_COMPLETED: Mutex<Option<mpsc::Sender<LaunchReply>>> = Mutex::new(None);

struct LaunchReply {
    success: bool,
    reason: String,
}

struct PendingLaunch {
    receiver: mpsc::Receiver<LaunchReply>,
    folder: CString,
    name: CString,
    params: CString,
}

unsafe extern "C" fn launcher_initialized() {
    if let Ok(mut sender) = LAUNCH_INITIALIZED.lock() {
        if let Some(sender) = sender.take() {
            let _ = sender.send(());
        }
    }
}

unsafe extern "C" fn launcher_completed(success: u8, reason: *const c_char) {
    let _ = std::panic::catch_unwind(|| {
        let reason = if reason.is_null() {
            String::new()
        } else {
            // SAFETY: the verified simple_service callback keeps this UTF-8
            // NUL-terminated reason alive until the callback returns.
            unsafe { CStr::from_ptr(reason) }
                .to_string_lossy()
                .into_owned()
        };
        if let Ok(mut sender) = LAUNCH_COMPLETED.lock() {
            if let Some(sender) = sender.take() {
                let _ = sender.send(LaunchReply {
                    success: success != 0,
                    reason,
                });
            }
        }
    });
}

#[derive(Default)]
struct CallbackQueue {
    container_id: String,
    samples: VecDeque<SystemSample>,
    diagnostics: VecDeque<String>,
}

impl CallbackQueue {
    fn accept(&mut self, bytes: &[u8]) {
        match system_monitor::parse_callback(bytes, &self.container_id) {
            Ok(sample) => self.samples.push_back(sample),
            Err(error) => self.diagnostics.push_back(format!("{error:#}")),
        }
    }

    fn drain(&mut self, owner_guid: &str, limit: usize) -> MonitorObservation {
        let count = self.samples.len().min(limit);
        let diagnostics_count = self.diagnostics.len().min(limit);
        MonitorObservation {
            owner_guid: owner_guid.into(),
            samples: self.samples.drain(..count).collect(),
            diagnostics: self.diagnostics.drain(..diagnostics_count).collect(),
        }
    }
}

unsafe extern "C" fn hardware_callback(value: *const c_char) {
    // No panic may unwind into the DLL. Both mutexes hold Rust-owned data; the
    // native timer thread never receives a pointer to a temporary context.
    let _ = std::panic::catch_unwind(|| {
        let owner = CALLBACK_OWNER.lock().ok().and_then(|owner| owner.clone());
        let Some(owner) = owner else { return };
        let Ok(mut queue) = owner.lock() else { return };
        if value.is_null() {
            queue
                .diagnostics
                .push_back("Null system monitor callback".into());
            return;
        }
        // SAFETY: the current native emitter calls this synchronously with its
        // NUL-terminated UTF-8 string, retaining the bytes until we return.
        let bytes = unsafe { CStr::from_ptr(value) }.to_bytes();
        queue.accept(bytes);
    });
}

type NativeBool = unsafe extern "C" fn() -> u8;
type SetCallback = unsafe extern "C" fn(Option<unsafe extern "C" fn(*const c_char)>) -> u8;
type Register = unsafe extern "C" fn(*const c_char, i32, i32) -> u8;
type StringAction = unsafe extern "C" fn(*const c_char) -> u8;

#[derive(Clone, Copy)]
struct Api {
    initialize: NativeBool,
    terminate: NativeBool,
    device_init: NativeBool,
    device_terminate: NativeBool,
    set_callback: SetCallback,
    register: Register,
    unregister: StringAction,
    server_running: NativeBool,
    set_server_path: StringAction,
}

impl Api {
    fn load(library: &EngineLibrary) -> anyhow::Result<Self> {
        unsafe fn symbol<T: Copy>(library: &EngineLibrary, name: &str) -> anyhow::Result<T> {
            // SAFETY: every caller below uses the current x64 wrapper declaration
            // and complete native body. Native booleans are observed as low bytes.
            unsafe { library.func(name) }
                .with_context(|| format!("System monitor DLL is missing {name}"))
        }
        // SAFETY: the current wrapper and native captures establish these exact
        // signatures. The caller keeps the library for every copied pointer.
        unsafe {
            Ok(Self {
                initialize: symbol(library, "Initialize")?,
                terminate: symbol(library, "Terminate")?,
                device_init: symbol(library, "DeviceInit")?,
                device_terminate: symbol(library, "DeviceTerminate")?,
                set_callback: symbol(library, "SetNodeFFIEvent")?,
                register: symbol(library, "RegisterFFIEvent")?,
                unregister: symbol(library, "UnRegisterFFIEvent")?,
                server_running: symbol(library, "IsAPPServerRunning")?,
                set_server_path: symbol(library, "SetServerDLLPath")?,
            })
        }
    }
}

struct WindowThread {
    handle: HANDLE,
    class_instance: HINSTANCE,
    class_unregistered: bool,
}

impl WindowThread {
    fn discover() -> anyhow::Result<Option<Self>> {
        let class: Vec<u16> = WINDOW_CLASS.encode_utf16().chain([0]).collect();
        let mut after = ptr::null_mut();
        loop {
            // SAFETY: the UTF-16 class name is terminated and lives across each
            // call. Only windows belonging to this worker process are accepted.
            let window =
                unsafe { FindWindowExW(ptr::null_mut(), after, class.as_ptr(), ptr::null()) };
            if window.is_null() {
                return Ok(None);
            }
            after = window;
            let mut process_id = 0;
            let thread_id = unsafe { GetWindowThreadProcessId(window, &mut process_id) };
            if process_id != unsafe { GetCurrentProcessId() } {
                continue;
            }
            ensure!(thread_id != 0, "System monitor window has no owning thread");
            let handle = unsafe { OpenThread(THREAD_SYNCHRONIZE, 0, thread_id) };
            ensure!(
                !handle.is_null(),
                "Cannot retain system monitor window thread: {}",
                std::io::Error::last_os_error()
            );
            #[cfg(target_pointer_width = "64")]
            let class_instance = unsafe { GetClassLongPtrW(window, GCLP_HMODULE) } as HINSTANCE;
            #[cfg(target_pointer_width = "32")]
            let class_instance = unsafe { GetClassLongW(window, GCLP_HMODULE) } as HINSTANCE;
            if class_instance.is_null() {
                unsafe { CloseHandle(handle) };
                bail!("Cannot observe system monitor window class instance");
            }
            return Ok(Some(Self {
                handle,
                class_instance,
                class_unregistered: false,
            }));
        }
    }

    fn wait_and_unregister(&mut self) -> anyhow::Result<()> {
        // SAFETY: OpenThread supplied this owned SYNCHRONIZE handle. It remains
        // live until this object drops; a timeout never releases the DLL.
        match unsafe { WaitForSingleObject(self.handle, WINDOW_STOP_TIMEOUT_MS) } {
            WAIT_OBJECT_0 => {}
            WAIT_TIMEOUT => bail!("System monitor window thread has not stopped"),
            _ => bail!(
                "System monitor window thread wait failed: {}",
                std::io::Error::last_os_error()
            ),
        }
        if !self.class_unregistered {
            let class: Vec<u16> = WINDOW_CLASS.encode_utf16().chain([0]).collect();
            // SAFETY: the joined thread can no longer use this class. Its exact
            // instance was read from the owned window, never another process.
            ensure!(
                unsafe { UnregisterClassW(class.as_ptr(), self.class_instance) } != 0,
                "System monitor window class cleanup failed: {}",
                std::io::Error::last_os_error()
            );
            self.class_unregistered = true;
        }
        Ok(())
    }
}

impl Drop for WindowThread {
    fn drop(&mut self) {
        // SAFETY: this wrapper uniquely owns the synchronization handle.
        unsafe { CloseHandle(self.handle) };
    }
}

struct Session {
    owner_guid: String,
    container: CString,
    paths: SystemMonitorPaths,
    library: ManuallyDrop<EngineLibrary>,
    api: Api,
    queue: Arc<Mutex<CallbackQueue>>,
    window_thread: Option<WindowThread>,
    lifecycle: Lifecycle,
}

#[derive(Default)]
struct Lifecycle {
    initialize_attempted: bool,
    device_initialized: bool,
    callback_attached: bool,
    registration_attempted: bool,
    terminated: bool,
    released: bool,
}

trait Cleanup {
    fn unregister(&mut self) -> anyhow::Result<()>;
    fn detach_callback(&mut self) -> anyhow::Result<()>;
    fn device_terminate(&mut self) -> anyhow::Result<()>;
    fn retain_window(&mut self) -> anyhow::Result<()>;
    fn terminate(&mut self) -> anyhow::Result<()>;
    fn join_and_release(&mut self) -> anyhow::Result<()>;
}

fn cleanup(lifecycle: &mut Lifecycle, operations: &mut impl Cleanup) -> anyhow::Result<()> {
    if lifecycle.released {
        return Ok(());
    }
    let mut errors = Vec::new();
    if lifecycle.registration_attempted && !lifecycle.terminated {
        match operations.unregister() {
            Ok(()) => lifecycle.registration_attempted = false,
            Err(error) => errors.push(format!("{error:#}")),
        }
    }
    if lifecycle.callback_attached && !lifecycle.terminated {
        match operations.detach_callback() {
            Ok(()) => lifecycle.callback_attached = false,
            Err(error) => errors.push(format!("{error:#}")),
        }
    }
    if lifecycle.device_initialized && !lifecycle.terminated {
        match operations.device_terminate() {
            Ok(()) => lifecycle.device_initialized = false,
            Err(error) => errors.push(format!("{error:#}")),
        }
    }
    if lifecycle.initialize_attempted && !lifecycle.terminated {
        // Failure to find the window cannot skip termination of the receiver,
        // pipe and buffer created by a partially failed Initialize.
        if let Err(error) = operations.retain_window() {
            errors.push(format!("{error:#}"));
        }
        match operations.terminate() {
            Ok(()) => lifecycle.terminated = true,
            Err(error) => errors.push(format!("{error:#}")),
        }
    }
    if lifecycle.terminated {
        match operations.join_and_release() {
            Ok(()) => lifecycle.released = true,
            Err(error) => errors.push(format!("{error:#}")),
        }
    }
    if !errors.is_empty() {
        bail!("{}", errors.join("; "));
    }
    ensure!(lifecycle.released, "System monitor resources remain owned");
    Ok(())
}

impl Session {
    fn create(
        owner_guid: String,
        container_id: String,
        paths: SystemMonitorPaths,
    ) -> anyhow::Result<Self> {
        let container = CString::new(container_id.clone())?;
        let library = EngineLibrary::load(&paths.client_dll)?;
        let api = Api::load(&library)?;
        let queue = Arc::new(Mutex::new(CallbackQueue {
            container_id,
            ..Default::default()
        }));
        let mut owner = CALLBACK_OWNER
            .lock()
            .map_err(|_| anyhow::anyhow!("System monitor callback owner is unavailable"))?;
        ensure!(
            owner.is_none(),
            "System monitor callback already has an owner"
        );
        *owner = Some(queue.clone());
        Ok(Self {
            owner_guid,
            container,
            paths,
            library: ManuallyDrop::new(library),
            api,
            queue,
            window_thread: None,
            lifecycle: Lifecycle::default(),
        })
    }

    fn retain_window_thread(&mut self) -> anyhow::Result<()> {
        if self.window_thread.is_some() {
            return Ok(());
        }
        let started = Instant::now();
        loop {
            if let Some(thread) = WindowThread::discover()? {
                self.window_thread = Some(thread);
                return Ok(());
            }
            ensure!(
                started.elapsed() < WINDOW_READY_TIMEOUT,
                "System monitor hidden window was not published"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    fn initialize(&mut self) -> anyhow::Result<bool> {
        self.lifecycle.initialize_attempted = true;
        // SAFETY: API signatures were checked before entering this session. The
        // retained library and callback lease outlive initialization and cleanup.
        let initialized = unsafe { (self.api.initialize)() } != 0;
        self.retain_window_thread()?;
        ensure!(initialized, "System monitor Initialize failed");
        ensure!(
            unsafe { (self.api.device_init)() } != 0,
            "System monitor DeviceInit failed"
        );
        self.lifecycle.device_initialized = true;
        let server_path = CString::new(
            self.paths
                .server_dll
                .to_str()
                .context("System monitor server path is not valid Unicode")?,
        )?;
        // SetServerDLLPath copies the UTF-8 string before returning.
        ensure!(
            unsafe { (self.api.set_server_path)(server_path.as_ptr()) } != 0,
            "System monitor server path was rejected"
        );
        ensure!(
            unsafe { (self.api.set_callback)(Some(hardware_callback)) } != 0,
            "System monitor callback registration failed"
        );
        self.lifecycle.callback_attached = true;
        // The native Register can insert the container before returning false.
        // Cleanup therefore unregisters every attempted registration.
        self.lifecycle.registration_attempted = true;
        ensure!(
            unsafe {
                (self.api.register)(self.container.as_ptr(), HARDWARE_MASK, CALLBACK_INTERVAL_MS)
            } != 0,
            "System monitor event registration failed"
        );
        Ok(unsafe { (self.api.server_running)() } == 0)
    }

    fn observation(&self, limit: usize) -> anyhow::Result<MonitorObservation> {
        self.queue
            .lock()
            .map_err(|_| anyhow::anyhow!("System monitor callback queue is unavailable"))
            .map(|mut queue| queue.drain(&self.owner_guid, limit))
    }

    fn shutdown(&mut self) -> anyhow::Result<()> {
        let mut lifecycle = std::mem::take(&mut self.lifecycle);
        let result = cleanup(&mut lifecycle, self);
        self.lifecycle = lifecycle;
        result
    }
}

impl Cleanup for Session {
    fn unregister(&mut self) -> anyhow::Result<()> {
        ensure!(
            unsafe { (self.api.unregister)(self.container.as_ptr()) } != 0,
            "System monitor event unregister failed"
        );
        Ok(())
    }

    fn detach_callback(&mut self) -> anyhow::Result<()> {
        ensure!(
            unsafe { (self.api.set_callback)(None) } != 0,
            "System monitor callback detach failed"
        );
        Ok(())
    }

    fn device_terminate(&mut self) -> anyhow::Result<()> {
        ensure!(
            unsafe { (self.api.device_terminate)() } != 0,
            "System monitor DeviceTerminate failed"
        );
        Ok(())
    }

    fn retain_window(&mut self) -> anyhow::Result<()> {
        self.retain_window_thread()
    }

    fn terminate(&mut self) -> anyhow::Result<()> {
        ensure!(
            unsafe { (self.api.terminate)() } != 0,
            "System monitor Terminate failed"
        );
        Ok(())
    }

    fn join_and_release(&mut self) -> anyhow::Result<()> {
        self.window_thread
            .as_mut()
            .context(
                "System monitor window thread exit has not been observed; DLL remains retained",
            )?
            .wait_and_unregister()?;
        let mut owner = CALLBACK_OWNER
            .lock()
            .map_err(|_| anyhow::anyhow!("System monitor callback owner is unavailable"))?;
        ensure!(
            owner
                .as_ref()
                .is_some_and(|owner| Arc::ptr_eq(owner, &self.queue)),
            "System monitor callback lease changed during cleanup"
        );
        owner.take();
        // SAFETY: receiver joined inside Terminate; the window thread is now
        // joined and its class is unregistered. No native code can still run.
        unsafe { ManuallyDrop::drop(&mut self.library) };
        Ok(())
    }
}

// A failed or unrequested shutdown must keep executable code alive. Session's
// ManuallyDrop library and global Arc lease then remain until worker process exit.
#[derive(Default)]
pub(super) struct State {
    active: Option<Session>,
    simple_initialization: Option<mpsc::Receiver<()>>,
    server_launch: Option<PendingLaunch>,
}

impl Drop for State {
    fn drop(&mut self) {
        if let Some(session) = self.active.as_mut() {
            if let Err(error) = session.shutdown() {
                // The worker's EOF/error exits have no reply channel. Retained
                // resources remain safe until process exit; report real cleanup
                // failure separately from a successful device response.
                eprintln!("System monitor final cleanup failed: {error:#}");
            }
        }
        if let Some(pending) = self.server_launch.take() {
            // A missing native completion cannot establish that argument memory
            // is no longer in use. Keep it alive with the retained simple DLL
            // until the owned worker process exits.
            std::mem::forget(pending);
        }
    }
}

impl State {
    fn prepare(
        &mut self,
        owner_guid: String,
        container_id: String,
        product_id: u32,
    ) -> anyhow::Result<bool> {
        system_monitor::validate_identity(&owner_guid)?;
        system_monitor::validate_identity(&container_id)?;
        ensure!(
            product_id == system_monitor::HANBO_PRODUCT_ID,
            "System monitor product is not verified"
        );
        ensure!(
            cfg!(target_arch = "x86_64"),
            "The current system monitor requires x64 Windows"
        );
        ensure!(self.active.is_none(), "System monitor already has an owner");
        let paths = razer_platform::native_paths::system_monitor_paths(product_id)?;
        self.active = Some(Session::create(owner_guid, container_id, paths)?);
        self.active
            .as_mut()
            .context("System monitor owner missing")?
            .initialize()
    }

    fn owned(&self, owner_guid: &str) -> anyhow::Result<&Session> {
        system_monitor::validate_identity(owner_guid)?;
        let session = self
            .active
            .as_ref()
            .context("System monitor has not started")?;
        ensure!(
            session.owner_guid == owner_guid,
            "System monitor owner changed"
        );
        Ok(session)
    }

    pub(super) fn poll(&self, owner_guid: &str) -> anyhow::Result<MonitorObservation> {
        self.owned(owner_guid)?.observation(MAX_POLL_SAMPLES)
    }

    pub(super) fn stop(&mut self, owner_guid: &str) -> anyhow::Result<MonitorObservation> {
        self.owned(owner_guid)?;
        let session = self
            .active
            .as_mut()
            .context("System monitor owner missing")?;
        session.shutdown()?;
        let observation = session.observation(usize::MAX)?;
        self.active.take();
        Ok(observation)
    }

    pub(super) fn shutdown(&mut self) -> anyhow::Result<Option<MonitorObservation>> {
        let Some(owner_guid) = self
            .active
            .as_ref()
            .map(|session| session.owner_guid.clone())
        else {
            return Ok(None);
        };
        self.stop(&owner_guid).map(Some)
    }
}

impl NativeRuntime {
    pub(super) fn system_monitor_launch_pending(&self) -> bool {
        self.system_monitor.server_launch.is_some()
    }

    pub(super) fn complete_system_monitor_simple_init(
        &mut self,
        timeout: Duration,
    ) -> anyhow::Result<()> {
        let Some(receiver) = self.system_monitor.simple_initialization.as_ref() else {
            return Ok(());
        };
        // Keep the receiver after a timeout. A late completion belongs to this
        // initialization, and cannot satisfy the ordinary native callback slot.
        receiver
            .recv_timeout(timeout)
            .context("System monitor launcher initialization remains unconfirmed")?;
        self.system_monitor.simple_initialization = None;
        self.simple_initialized = true;
        Ok(())
    }

    pub(super) fn complete_system_monitor_launch(
        &mut self,
        timeout: Duration,
    ) -> anyhow::Result<()> {
        let Some(pending) = self.system_monitor.server_launch.as_ref() else {
            return Ok(());
        };
        let reply = pending
            .receiver
            .recv_timeout(timeout)
            .context("System monitor server launch remains unconfirmed")?;
        self.system_monitor.server_launch = None;
        ensure!(
            reply.success,
            "System monitor server launch failed: {}",
            reply.reason
        );
        Ok(())
    }

    fn launch_system_monitor_server(
        &mut self,
        folder: CString,
        name: CString,
    ) -> anyhow::Result<()> {
        self.complete_system_monitor_simple_init(Duration::ZERO)?;
        self.complete_system_monitor_launch(Duration::ZERO)?;
        if !self.simple_initialized {
            if self.simple.is_none() {
                let path = self.paths.resolve_host_service("simple_service")?;
                self.simple = Some(ManuallyDrop::new(EngineLibrary::load(&path)?));
            }
            let initialize: super::Initialize =
                unsafe { self.simple_symbol("simpleServiceInitialize")? };
            let (sender, receiver) = mpsc::channel();
            {
                let mut pending = LAUNCH_INITIALIZED.lock().map_err(|_| {
                    anyhow::anyhow!("System monitor launcher callback is unavailable")
                })?;
                ensure!(
                    pending.is_none(),
                    "System monitor launcher initialization already pending"
                );
                *pending = Some(sender);
            }
            self.system_monitor.simple_initialization = Some(receiver);
            // SAFETY: current simple_service initializes with void(callback0).
            // The dedicated static callback and retained module outlive timeout.
            unsafe { initialize(launcher_initialized) };
            self.complete_system_monitor_simple_init(CALLBACK_TIMEOUT)?;
        }
        // Current host wrapper: void(string,string,string,Callback(void,
        // [bool,string])). A launcher timeout never poisons device/HID dispatch.
        type Launch = unsafe extern "C" fn(
            *const c_char,
            *const c_char,
            *const c_char,
            unsafe extern "C" fn(u8, *const c_char),
        );
        let launch: Launch = unsafe { self.simple_symbol("simpleLaunchUserAppProcessNoWait")? };
        let (sender, receiver) = mpsc::channel();
        {
            let mut pending = LAUNCH_COMPLETED
                .lock()
                .map_err(|_| anyhow::anyhow!("System monitor launch callback is unavailable"))?;
            ensure!(
                pending.is_none(),
                "System monitor server launch already pending"
            );
            *pending = Some(sender);
        }
        self.system_monitor.server_launch = Some(PendingLaunch {
            receiver,
            folder,
            name,
            params: CString::new("")?,
        });
        let pending = self
            .system_monitor
            .server_launch
            .as_ref()
            .context("System monitor launch owner missing")?;
        // SAFETY: all arguments and the callback survive until this invocation
        // completes, including an unconfirmed asynchronous completion/timeout.
        unsafe {
            launch(
                pending.folder.as_ptr(),
                pending.name.as_ptr(),
                pending.params.as_ptr(),
                launcher_completed,
            )
        };
        self.complete_system_monitor_launch(CALLBACK_TIMEOUT)
    }

    pub(super) fn start_system_monitor(
        &mut self,
        owner_guid: String,
        container_id: String,
        product_id: u32,
    ) -> anyhow::Result<serde_json::Value> {
        // The monitoring route belongs to the physical device's real container;
        // its IPC owner remains a separate session identity.
        ensure!(
            self.system_monitor.active.is_none(),
            "System monitor already has an owner"
        );
        super::native_target(&container_id, Some(product_id))?;
        let result = (|| {
            if self
                .system_monitor
                .prepare(owner_guid.clone(), container_id, product_id)?
            {
                let paths = &self.system_monitor.owned(&owner_guid)?.paths;
                let folder = CString::new(paths.helper_folder.as_str())?;
                let name = CString::new(paths.helper_name.as_str())?;
                self.launch_system_monitor_server(folder, name)?;
            }
            self.system_monitor.poll(&owner_guid)
        })();
        match result {
            Ok(observation) => serde_json::to_value(observation).map_err(Into::into),
            Err(error) => {
                // A rejected old owner's Start must not stop its successor.
                if self.system_monitor.owned(&owner_guid).is_err() {
                    return Err(error);
                }
                match self.system_monitor.stop(&owner_guid) {
                    Ok(_) => Err(error),
                    Err(cleanup) => Err(error.context(format!(
                        "System monitor startup cleanup also failed: {cleanup:#}"
                    ))),
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "system_monitor_tests.rs"]
mod tests;
