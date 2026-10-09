//! Headless child-process entrypoint. GPUI is not a dependency.
fn main() {
    std::process::exit(razer_service::runtime::run_worker());
}
