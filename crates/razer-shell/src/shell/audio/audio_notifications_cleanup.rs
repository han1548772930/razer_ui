//! Operation failure and native/service release failures are independent receipts.
#[derive(Debug)]
pub(in crate::shell) struct Completion {
    pub(in crate::shell) result: Result<(), String>,
    pub(in crate::shell) cleanup_failure: Option<String>,
}

pub(super) fn guarded<T>(run: impl FnOnce() -> anyhow::Result<T>) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(run))
        .map_err(|_| "Audio_StreamMixer owner panicked".to_owned())?
        .map_err(|error| format!("{error:#}"))
}

pub(super) fn step(errors: &mut Vec<String>, name: &str, run: impl FnOnce() -> anyhow::Result<()>) {
    if let Err(error) = guarded(run) {
        errors.push(format!("{name}: {error}"));
    }
}

pub(super) fn terminate(
    errors: &mut Vec<String>,
    mut finish: impl FnMut() -> anyhow::Result<()>,
    mut wait: impl FnMut(),
) {
    loop {
        match guarded(&mut finish) {
            Ok(()) => break,
            Err(error) => {
                let error = format!("Audio service termination: {error}");
                if !errors.contains(&error) { errors.push(error); }
                wait();
            }
        }
    }
}

pub(super) fn completed(result: Result<(), String>, errors: Vec<String>) -> Completion {
    Completion { result, cleanup_failure: (!errors.is_empty()).then(|| errors.join("; ")) }
}

#[cfg(test)]
#[path = "audio_notifications_cleanup_tests.rs"]
mod tests;
