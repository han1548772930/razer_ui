#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

//! Executable composition entry point.
fn main() -> std::process::ExitCode {
    razer_app::run()
}
