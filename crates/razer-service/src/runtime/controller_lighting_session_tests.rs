use super::*;
use std::{cell::RefCell, rc::Rc};

#[derive(Default)]
struct Fake {
    calls: Rc<RefCell<Vec<String>>>,
    fail_destroy: bool,
}
impl EngineAdapter for Fake {
    fn create_device(&mut self, c: &LedConfiguration) -> anyhow::Result<u32> {
        self.calls
            .borrow_mut()
            .push(format!("device:{}", c.edition));
        Ok(7)
    }
    fn create_engine(&mut self) -> anyhow::Result<u32> {
        self.calls.borrow_mut().push("engine:25".into());
        Ok(9)
    }
    fn add_device(&mut self, e: u32, d: u32) -> anyhow::Result<()> {
        self.calls.borrow_mut().push(format!("add:{e}:{d}"));
        Ok(())
    }
    fn add_effect(&mut self, _: u32, f: &EngineEffect) -> anyhow::Result<u32> {
        self.calls.borrow_mut().push(format!("effect:{}", f.effect));
        Ok(11)
    }
    fn remove_effect(&mut self, e: u32, f: u32) -> anyhow::Result<()> {
        self.calls.borrow_mut().push(format!("remove:{e}:{f}"));
        Ok(())
    }
    fn enable(&mut self, e: u32, enabled: bool) -> anyhow::Result<()> {
        self.calls
            .borrow_mut()
            .push(format!("enable:{e}:{enabled}"));
        Ok(())
    }
    fn input(&mut self, d: u32) -> anyhow::Result<()> {
        self.calls.borrow_mut().push(format!("input:{d}"));
        Ok(())
    }
    fn frames(&mut self) -> anyhow::Result<Vec<EngineFrame>> {
        self.calls.borrow_mut().push("poll".into());
        Ok(Vec::new())
    }
    fn destroy_engine(&mut self, e: u32) -> anyhow::Result<()> {
        self.calls.borrow_mut().push(format!("destroy-engine:{e}"));
        if self.fail_destroy {
            anyhow::bail!("native destroy failed");
        }
        Ok(())
    }
    fn destroy_device(&mut self, d: u32) -> anyhow::Result<()> {
        self.calls.borrow_mut().push(format!("destroy-device:{d}"));
        Ok(())
    }
    fn terminate(&mut self) -> anyhow::Result<()> {
        self.calls.borrow_mut().push("terminate".into());
        Ok(())
    }
}

#[test]
fn source_effect_replacement_keeps_native_handles_and_discards_stale_frames() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut session = Session::new(Fake {
        calls: calls.clone(),
        fail_destroy: false,
    });
    session
        .initialize(
            &razer_device::controller_lighting::software::led_configuration(128).unwrap(),
            &SoftwareEffect::Static { color: [1, 2, 3] },
        )
        .unwrap();
    session
        .update(&SoftwareEffect::Spectrum { duration: None })
        .unwrap();
    assert_eq!(
        &*calls.borrow(),
        &[
            "engine:25",
            "device:128",
            "add:9:7",
            "poll",
            "effect:6",
            "enable:9:true",
            "remove:9:11",
            "poll",
            "effect:4",
            "enable:9:true",
        ]
    );
    session.suspend(true).unwrap();
    assert!(session.suspended);
    session.suspend(false).unwrap();
    session.input().unwrap();
    assert!(!session.suspended);
}

#[test]
fn cleanup_continues_after_native_failure_and_releases_each_handle_once() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut session = Session::new(Fake {
        calls: calls.clone(),
        fail_destroy: true,
    });
    session
        .initialize(
            &razer_device::controller_lighting::software::led_configuration(0).unwrap(),
            &SoftwareEffect::Spectrum { duration: None },
        )
        .unwrap();
    calls.borrow_mut().clear();
    let errors = session.cleanup();
    assert_eq!(errors.len(), 1);
    assert!(errors[0].contains("native destroy failed"));
    assert_eq!(
        &*calls.borrow(),
        &[
            "enable:9:false",
            "destroy-engine:9",
            "destroy-device:7",
            "terminate"
        ]
    );
    assert!(session.cleanup().is_empty());
    drop(session);
    assert_eq!(calls.borrow().len(), 4);
}
