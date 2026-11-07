use super::*;
use crate::backend::{FeatureTransport, NativeIoOutcome, native_io_failure};
use std::cell::{Cell, RefCell};

struct NoSleep;
impl Sleeper for NoSleep {
    fn sleep(&self, _: std::time::Duration) {}
}
struct Io {
    sent: Cell<usize>,
    last: RefCell<Vec<u8>>,
    request: Cell<bool>,
    connection: Cell<bool>,
    cancel_read: Cell<bool>,
    disconnect_read: Cell<bool>,
    missing: Cell<bool>,
    not_called: Cell<bool>,
    wrong: Cell<bool>,
}
impl Default for Io {
    fn default() -> Self {
        Self {
            sent: Cell::new(0),
            last: RefCell::new(vec![]),
            request: Cell::new(true),
            connection: Cell::new(true),
            cancel_read: Cell::new(false),
            disconnect_read: Cell::new(false),
            missing: Cell::new(false),
            not_called: Cell::new(false),
            wrong: Cell::new(false),
        }
    }
}
impl FeatureTransport for Io {
    fn send_feature(&self, bytes: &[u8]) -> Result<()> {
        if self.not_called.get() {
            return Err(native_io_failure(
                NativeIoOutcome::NotCalled,
                anyhow::anyhow!("admission canceled"),
            ));
        }
        self.sent.set(self.sent.get() + 1);
        self.last.replace(bytes.to_vec());
        Ok(())
    }
    fn get_feature(&self, bytes: &mut [u8]) -> Result<usize> {
        if self.missing.get() {
            return Err(native_io_failure(
                NativeIoOutcome::Rejected,
                anyhow::anyhow!("reply lost"),
            ));
        }
        let last = self.last.borrow();
        bytes.fill(0);
        bytes[0] = 2;
        bytes[1] = 2;
        bytes[2] = last[2];
        bytes[7] = 24;
        bytes[8] = if self.wrong.get() { 7 } else { 6 };
        if self.cancel_read.get() {
            self.request.set(false);
        }
        if self.disconnect_read.get() {
            self.connection.set(false);
        }
        Ok(64)
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}
fn run(cache: &Cache, io: &Io, setting: AncSetting) -> Result<Report> {
    let request = || {
        ensure!(io.request.get(), "profile request canceled");
        Ok(())
    };
    let connection = || {
        ensure!(io.connection.get(), "physical owner ended");
        Ok(())
    };
    cache.execute(
        &mut Protocol40Session::with_sleeper(io, NoSleep),
        2657,
        setting,
        &request,
        &connection,
    )
}
fn off() -> AncSetting {
    AncSetting {
        enabled: false,
        ..protocol::ANC_PROFILE_DEFAULT
    }
}

#[test]
fn earbuds_anc_cache_distinguishes_native_attempt_from_unsent_work() {
    let io = Io::default();
    let cache = Cache::default();
    assert_eq!(
        run(&cache, &io, protocol::ANC_PROFILE_DEFAULT)
            .unwrap()
            .write,
        AncWrite::Skipped
    );
    io.request.set(false);
    assert!(run(&cache, &io, off()).is_err());
    assert!(cache.is_comparable());
    io.request.set(true);
    io.not_called.set(true);
    assert!(matches!(
        run(&cache, &io, off()).unwrap().write,
        AncWrite::Failed(_)
    ));
    assert!(cache.is_comparable());
    assert_eq!(io.sent.get(), 0);
    io.not_called.set(false);
    io.missing.set(true);
    assert!(matches!(
        run(&cache, &io, off()).unwrap().write,
        AncWrite::Failed(_)
    ));
    assert!(!cache.is_comparable());
    assert_eq!(io.sent.get(), 1);
    io.missing.set(false);
    assert_eq!(
        run(&cache, &io, protocol::ANC_PROFILE_DEFAULT)
            .unwrap()
            .write,
        AncWrite::Acknowledged
    );
    assert_eq!(io.sent.get(), 2);
    assert!(cache.is_comparable());
}

#[test]
fn earbuds_anc_cache_keeps_validated_ack_after_profile_cancel_but_not_wrong_reply() {
    for wrong in [false, true] {
        let io = Io::default();
        let cache = Cache::default();
        io.cancel_read.set(true);
        io.wrong.set(wrong);
        assert!(run(&cache, &io, off()).is_err());
        assert_eq!(cache.is_comparable(), !wrong);
        assert_eq!(
            cache.runtime(),
            if wrong {
                AncRuntimeState::default()
            } else {
                AncRuntimeState(off())
            }
        );
        io.request.set(true);
        io.cancel_read.set(false);
        io.wrong.set(false);
        assert_eq!(
            run(&cache, &io, protocol::ANC_PROFILE_DEFAULT)
                .unwrap()
                .write,
            AncWrite::Acknowledged
        );
        assert_eq!(io.sent.get(), 2);
    }
}

#[test]
fn earbuds_anc_cache_rejects_confirmation_after_physical_owner_ended() {
    let io = Io::default();
    let cache = Cache::default();
    io.cancel_read.set(true);
    io.disconnect_read.set(true);
    assert!(run(&cache, &io, off()).is_err());
    assert!(!cache.is_comparable());
    assert_eq!(cache.runtime(), AncRuntimeState::default());
}
