//! Shared monotonic timestamp used by physical input and mapping callbacks.

/// The process-wide source clock shares its QPC decision and fallback wrap
/// counter with global shortcuts and macro recording.
pub fn tick() -> anyhow::Result<u64> {
    #[cfg(windows)]
    {
        Ok(crate::platform::windows::input_clock::EventClock::new().tick())
    }
    #[cfg(not(windows))]
    {
        anyhow::bail!("Native mapping event clock requires Windows")
    }
}
