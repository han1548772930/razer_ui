//! Local Rust panic records; native failfast and abort may bypass this hook.
use std::{
    backtrace::Backtrace, fmt::Display, fs::OpenOptions, io::Write, path::Path, sync::Mutex,
    time::SystemTime,
};

pub(super) fn install_panic_log() {
    let path = razer_storage::store_path().with_file_name("panic.log");
    let opened = (|| {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        OpenOptions::new().create(true).append(true).open(&path)
    })();
    let output = match opened {
        Ok(output) => Mutex::new(output),
        Err(error) => {
            report_failure(&path, error);
            return;
        }
    };
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // A panic hook must not wait for another panic writer or re-enter its lock.
        match output.try_lock() {
            Ok(mut output) => {
                let thread = std::thread::current();
                let written = writeln!(
                    output,
                    "\n[panic] process={} thread={:?} name={:?} time={:?}\n{info}\n{}",
                    std::process::id(),
                    thread.id(),
                    thread.name(),
                    SystemTime::now(),
                    Backtrace::force_capture(),
                );
                let flushed = output.flush();
                if let Err(error) = written.and(flushed) {
                    report_failure(&path, error);
                }
            }
            Err(error) => report_failure(&path, error),
        }
        previous(info);
    }));
}

fn report_failure(path: &Path, error: impl Display) {
    let _ = writeln!(std::io::stderr(), "[panic log] {}: {error}", path.display());
}
