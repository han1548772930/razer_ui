//! Current RzPowerTool --get-service-status, implemented without its executable.
//! Source: WinMain RVA 0x36430, service gate 0x356e0, default-C comparison 0x6e960.
//! Query and source-verified local start/stop operations share the service gate.

/// Start an allowed service, then recursively start all dependent services.
/// Returns the original helper's exit code: 1 on success, 0 on failure.
/// Child failures are ignored by the source after the parent succeeds.
pub fn start(service_name: &str) -> anyhow::Result<u32> {
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::services::start(service_name)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = service_name;
        anyhow::bail!("Razer Windows service control is unsupported on this platform")
    }
}

/// Recursively stop active dependent services, then stop the allowed parent.
/// Returns the original helper's exit code: 1 on success, 0 on failure.
/// This operation has no source-defined cancellation or rollback semantics.
pub fn stop(service_name: &str) -> anyhow::Result<u32> {
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::services::stop(service_name)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = service_name;
        anyhow::bail!("Razer Windows service control is unsupported on this platform")
    }
}

/// Return the helper's exit-code semantics: 0 on unsupported service/API failure,
/// 1 for stopped, 3 for stop-pending, and 4 for every other queried state.
/// In particular, the original also collapses paused/start-pending to 4.
pub fn query(service_name: &str) -> anyhow::Result<u32> {
    #[cfg(target_os = "windows")]
    {
        crate::platform::windows::services::query(service_name)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = service_name;
        anyhow::bail!("Razer Windows service status is unsupported on this platform")
    }
}
