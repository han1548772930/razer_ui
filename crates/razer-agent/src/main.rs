//! Headless service entrypoint. GPUI is not a dependency.
fn main() {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    let status = if arguments.len() == 1 && arguments[0] == "--hid-broker" {
        razer_service::runtime::run_hid_broker()
    } else if arguments.len() == 1 && arguments[0] == "--service-worker" {
        razer_service::runtime::run_worker()
    } else {
        eprintln!("Expected exactly one headless service entrypoint");
        2
    };
    std::process::exit(status);
}
