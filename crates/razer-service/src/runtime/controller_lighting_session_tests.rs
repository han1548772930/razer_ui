use super::*;
use std::{cell::RefCell, rc::Rc};

#[cfg(windows)]
#[test]
fn production_rust_adapter_renders_and_releases_a_controller_without_loading_vendor_code() {
    let renderer = crate::runtime::lighting_engine::Engine::open().unwrap();
    let mut session = Session::new(renderer);
    session
        .initialize(
            &razer_device::controller_lighting::software::led_configuration(128).unwrap(),
            &SoftwareEffect::Static { color: [255, 0, 0] },
        )
        .unwrap();
    let frames = session.engine.frames().unwrap();
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].device_handle, session.device_handle.unwrap());
    assert_eq!((frames[0].num_row, frames[0].num_col), (1, 32));
    assert_eq!(frames[0].frame, [3, 0, 0].repeat(32));
    assert!(session.cleanup().is_empty());
    assert!(session.engine.frames().unwrap().is_empty());
    assert!(session.cleanup().is_empty());
}

#[cfg(windows)]
#[test]
fn production_shared_rust_adapter_keeps_dock_and_peer_lifetimes_separate() {
    let mut runtime = SharedEngine::new(crate::runtime::lighting_engine::Engine::open().unwrap());
    let mut dock = DeviceEngines::new(
        runtime.clone(),
        razer_device::receiver_lighting::led_configuration(164, 0).unwrap(),
    );
    let mut peer = DeviceEngines::new(
        runtime.clone(),
        razer_device::controller_lighting::software::led_configuration(128).unwrap(),
    );
    let effect = SoftwareEffect::Static { color: [255, 0, 0] }.engine_effect();
    dock.apply_normal(&effect).unwrap();
    peer.apply_normal(&effect).unwrap();
    assert_ne!(dock.device_handle, peer.device_handle);
    // Creating the peer can poll and preserve an already due dock frame.
    let frames = runtime.frames().unwrap();
    assert!(
        frames
            .iter()
            .any(|frame| Some(frame.device_handle) == dock.device_handle)
    );
    assert!(
        frames
            .iter()
            .any(|frame| Some(frame.device_handle) == peer.device_handle)
    );
    assert!(dock.cleanup().is_empty());
    assert!(peer.cleanup().is_empty());
    runtime.terminate().unwrap();
    assert!(runtime.frames().unwrap().is_empty());
}

#[derive(Default)]
struct Fake {
    calls: Rc<RefCell<Vec<String>>>,
    fail_destroy: bool,
    fail_terminate: bool,
    pending_frames: Vec<EngineFrame>,
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
        Ok(std::mem::take(&mut self.pending_frames))
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
        anyhow::ensure!(!self.fail_terminate, "native terminate failed");
        Ok(())
    }
}

#[test]
fn source_effect_replacement_keeps_native_handles_and_discards_stale_frames() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut session = Session::new(Fake {
        calls: calls.clone(),
        fail_destroy: false,
        ..Default::default()
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
        ..Default::default()
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
    assert_eq!(session.cleanup(), errors);
    drop(session);
    assert_eq!(calls.borrow().len(), 4);
}

#[test]
fn charging_engine_reuses_device_and_rebuilds_normal_before_global_termination() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut runtime = SharedEngine::new(Fake {
        calls: calls.clone(),
        ..Default::default()
    });
    let mut device = DeviceEngines::new(
        runtime.clone(),
        razer_device::receiver_lighting::led_configuration(164, 0).unwrap(),
    );
    let normal = SoftwareEffect::Spectrum { duration: None }.engine_effect();
    let special = SoftwareEffect::Static { color: [255, 0, 0] }.engine_effect();
    device.apply_normal(&normal).unwrap();
    device.apply_special(&special).unwrap();
    assert_eq!(
        calls
            .borrow()
            .iter()
            .filter(|call| call.starts_with("device:"))
            .count(),
        1
    );
    assert_eq!(
        calls
            .borrow()
            .iter()
            .filter(|call| call.as_str() == "add:9:7")
            .count(),
        2
    );
    assert!(
        !calls
            .borrow()
            .iter()
            .any(|call| call == "destroy-device:7" || call == "terminate")
    );
    device.remove_special(Some(&normal)).unwrap();
    assert_eq!(
        calls
            .borrow()
            .iter()
            .filter(|call| call.as_str() == "destroy-device:7")
            .count(),
        1
    );
    assert_eq!(
        calls
            .borrow()
            .iter()
            .filter(|call| call.starts_with("device:"))
            .count(),
        2
    );
    assert!(device.cleanup().is_empty());
    assert_eq!(
        calls
            .borrow()
            .iter()
            .filter(|call| call.as_str() == "destroy-device:7")
            .count(),
        2
    );
    runtime.terminate().unwrap();
    runtime.terminate().unwrap();
    drop(device);
    drop(runtime);
    assert_eq!(
        calls
            .borrow()
            .iter()
            .filter(|call| call.as_str() == "terminate")
            .count(),
        1
    );
}

#[test]
fn shared_runtime_discard_preserves_other_devices_and_termination_failure_receipt() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let frame = |handle| EngineFrame {
        device_handle: handle,
        region_id: 0,
        num_row: 1,
        num_col: 1,
        frame: vec![1, 2, 3],
    };
    let mut runtime = SharedEngine::new(Fake {
        calls: calls.clone(),
        fail_terminate: true,
        pending_frames: vec![frame(7), frame(12)],
        ..Default::default()
    });
    runtime.discard_frames_for(7).unwrap();
    assert_eq!(runtime.frames().unwrap(), vec![frame(12)]);
    let first = runtime.terminate().unwrap_err().to_string();
    assert_eq!(runtime.terminate().unwrap_err().to_string(), first);
    drop(runtime);
    assert_eq!(
        calls
            .borrow()
            .iter()
            .filter(|call| call.as_str() == "terminate")
            .count(),
        1
    );
}

#[test]
fn shared_device_session_never_destroys_borrowed_device_or_global_runtime() {
    let calls = Rc::new(RefCell::new(Vec::new()));
    let mut session = Session::with_shared_device(
        Fake {
            calls: calls.clone(),
            ..Default::default()
        },
        7,
    )
    .unwrap();
    session
        .initialize_shared(&SoftwareEffect::Static { color: [0, 255, 0] }.engine_effect())
        .unwrap();
    assert!(session.cleanup().is_empty());
    assert!(
        !calls.borrow().iter().any(|call| call.starts_with("device:")
            || call.starts_with("destroy-device:")
            || call == "terminate")
    );
}
