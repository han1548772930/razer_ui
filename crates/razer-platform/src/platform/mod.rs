//! OS adapters selected without duplicating shared host contracts.
#[cfg(target_os = "windows")]
#[path = "windows/mod.rs"]
pub(crate) mod windows;
