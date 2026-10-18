//! Clipboard paste key tasks from 11b849/11be52/11c460. The injected
//! Ctrl+V keys and temporarily released user modifiers have separate owners.
use razer_device::macro_runtime::Keyboard;

#[derive(Clone, Debug)]
pub(super) enum Operation {
    Key { key: Keyboard, retain: bool },
    ReleaseHeld,
}
fn key(scancode: u16, flag: u16, retain: bool) -> Operation {
    Operation::Key {
        key: Keyboard {
            scancode,
            flag,
            unit: None,
            modifiers: 0,
            delay_ms: None,
            release_after_ms: None,
        },
        retain,
    }
}

pub(super) fn guarded<T>(call: impl FnOnce() -> T) -> Result<T, String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(call))
        .map_err(|_| "Clipboard operation owner panicked".to_owned())
}

pub(super) fn run(
    modifiers: &[(u16, u16)],
    mut execute: impl FnMut(Operation) -> Result<(), String>,
    mut wait: impl FnMut(u64),
) -> Result<(), String> {
    let mut errors = Vec::new();
    let mut released = Vec::new();
    let result = guarded(|| {
        for (scan, flag) in modifiers.iter().rev() {
            match execute(key(*scan, *flag | 1, false)) {
                Ok(()) => released.push((*scan, *flag)),
                Err(error) => errors.push(error),
            }
        }
        for (scan, flag, delay) in [(29, 0, 0), (47, 0, 50), (47, 1, 0), (29, 1, 10)] {
            match execute(key(scan, flag, true)) {
                Ok(()) if delay != 0 => wait(delay),
                Ok(()) => (),
                Err(error) => errors.push(error),
            }
        }
    });
    let restore = if let Err(error) = result {
        errors.push(error);
        // Release confirmed injected keys before making the user's modifiers;
        // otherwise a later injected Ctrl release could undo their restore.
        if let Err(error) = guarded(|| execute(Operation::ReleaseHeld)).and_then(|result| result) {
            errors.push(error);
        }
        released
    } else {
        modifiers.iter().rev().copied().collect()
    };
    for (scan, flag) in restore {
        if let Err(error) = guarded(|| execute(key(scan, flag, false))).and_then(|result| result) {
            errors.push(error);
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paste_retains_only_injected_keys_and_preserves_source_order() {
        let mut events = Vec::new();
        let mut delays = Vec::new();
        run(
            &[(56, 0), (29, 2)],
            |operation| {
                let Operation::Key { key, retain } = operation else {
                    panic!("unexpected cleanup")
                };
                events.push((key.scancode, key.flag, retain));
                Ok(())
            },
            |ms| delays.push(ms),
        )
        .unwrap();
        assert_eq!(
            events,
            [
                (29, 3, false),
                (56, 1, false),
                (29, 0, true),
                (47, 0, true),
                (47, 1, true),
                (29, 1, true),
                (29, 2, false),
                (56, 0, false)
            ]
        );
        assert_eq!(delays, [50, 10]);
    }
    #[test]
    fn panic_releases_injected_keys_before_restoring_confirmed_user_modifiers() {
        let mut events = Vec::new();
        let result = run(
            &[(56, 0), (29, 2)],
            |operation| {
                match operation {
                    Operation::Key { key, retain } => {
                        events.push((key.scancode, key.flag, retain));
                        if key.scancode == 47 && key.flag == 0 {
                            panic!("injected execution panic")
                        }
                    }
                    Operation::ReleaseHeld => events.push((0, 0, true)),
                }
                Ok(())
            },
            |_| (),
        );
        assert!(result.is_err());
        assert_eq!(
            &events[events.len() - 3..],
            &[(0, 0, true), (29, 2, false), (56, 0, false)]
        );
    }
    #[test]
    fn failed_outputs_continue_source_sequence_without_retry_or_successful_delay() {
        let mut calls = 0;
        let mut delays = Vec::new();
        let result = run(
            &[(56, 0)],
            |_| {
                calls += 1;
                Err("rejected".into())
            },
            |ms| delays.push(ms),
        );
        assert!(result.is_err());
        assert_eq!(calls, 6);
        assert!(delays.is_empty());
    }
}
