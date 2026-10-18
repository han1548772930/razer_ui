//! Shared source-defined input timestamps.
use std::sync::{Mutex, OnceLock};
#[link(name = "kernel32")]
unsafe extern "system" {
    fn QueryPerformanceFrequency(frequency: *mut i64) -> i32;
    fn QueryPerformanceCounter(counter: *mut i64) -> i32;
}
#[link(name = "winmm")]
unsafe extern "system" {
    fn timeGetTime() -> u32;
}

// Current 1f0d40 writes CPU offset41 from the invariant-TSC bit, or from
// Microsoft's hypervisor signature when the hypervisor-present bit is set.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
fn stable_performance_counter() -> bool {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::__cpuid;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::__cpuid;
    let extended = __cpuid(0x8000_0000).eax;
    if extended >= 0x8000_0007 && __cpuid(0x8000_0007).edx & 0x100 != 0 {
        return true;
    }
    if __cpuid(0).eax == 0 || __cpuid(1).ecx & 0x8000_0000 == 0 {
        return false;
    }
    let hypervisor = __cpuid(0x4000_0000);
    hypervisor.ebx == 1_919_117_645
        && hypervisor.ecx == 1_718_580_079
        && hypervisor.edx == 1_984_438_388
}
#[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
fn stable_performance_counter() -> bool {
    false
}

/// c94c0 -> 1b7f50 -> 1a8c30/1a8d30 captures QPC microseconds before
/// side-modifier lookup. 1a9170 extends the fallback millisecond clock through
/// wrap and converts it to microseconds. No vendor export is executed here.
struct MonotonicClock {
    frequency: Option<i64>,
    previous_high: u8,
    wraps: u16,
}
impl MonotonicClock {
    fn new() -> Self {
        let mut frequency = 0;
        let available = unsafe { QueryPerformanceFrequency(&mut frequency) } != 0
            && frequency > 0
            && stable_performance_counter();
        Self {
            frequency: available.then_some(frequency),
            previous_high: 0,
            wraps: 0,
        }
    }
    fn tick(&mut self) -> u64 {
        if let Some(frequency) = self.frequency {
            let mut counter = 0;
            unsafe { QueryPerformanceCounter(&mut counter) };
            // Integer division, including large counters, matches the native
            // quotient/remainder conversion without overflowing its product.
            return ((i128::from(counter) * 1_000_000) / i128::from(frequency)) as u64;
        }
        let now = unsafe { timeGetTime() };
        let high = (now >> 24) as u8;
        if high < self.previous_high {
            self.wraps = self.wraps.wrapping_add(1);
        }
        self.previous_high = high;
        ((u64::from(self.wraps) << 32) | u64::from(now)) * 1000
    }
}

static MONOTONIC: OnceLock<Mutex<MonotonicClock>> = OnceLock::new();

/// The native clock's wrap counter is process-wide. Restarting a recorder or
/// opening a shortcut owner must not create a different fallback epoch.
pub(crate) struct EventClock;
impl EventClock {
    pub(crate) fn new() -> Self {
        MONOTONIC.get_or_init(|| Mutex::new(MonotonicClock::new()));
        Self
    }
    pub(crate) fn tick(&mut self) -> u64 {
        MONOTONIC
            .get()
            .expect("Input clock is initialized")
            .lock()
            .expect("Input clock state poisoned")
            .tick()
    }
}

/// 1a8750 samples FILETIME once per minute, interpolating with the same
/// monotonic counter as lifecycle events. FILETIME's epoch is retained.
pub(crate) struct RecorderClock {
    monotonic: EventClock,
    baseline: Option<(i64, i64)>,
}
impl RecorderClock {
    pub(crate) fn new() -> Self {
        Self {
            monotonic: EventClock::new(),
            baseline: None,
        }
    }
    pub(crate) fn tick(&mut self) -> u64 {
        self.monotonic.tick()
    }
    pub(crate) fn wall_tick(&mut self) -> i64 {
        use windows_sys::Win32::{
            Foundation::FILETIME, System::SystemInformation::GetSystemTimeAsFileTime,
        };
        loop {
            let (base, wall) = *self.baseline.get_or_insert_with(|| {
                let base = self.monotonic.tick() as i64;
                let mut filetime = FILETIME {
                    dwLowDateTime: 0,
                    dwHighDateTime: 0,
                };
                // The API initializes the complete FILETIME out parameter.
                unsafe { GetSystemTimeAsFileTime(&mut filetime) };
                let value =
                    (u64::from(filetime.dwHighDateTime) << 32) | u64::from(filetime.dwLowDateTime);
                (base, (value as i64) / 10)
            });
            let delta = (self.monotonic.tick() as i64).saturating_sub(base);
            if delta > 60_000_000 {
                self.baseline = None;
                continue;
            }
            return wall.saturating_add(delta);
        }
    }
}
