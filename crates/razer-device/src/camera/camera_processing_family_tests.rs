//! Scripted KS transport; never activates COM or contacts a real camera.
use super::*;
use crate::camera_extensions::CameraKsCallOutcome;
use std::cell::Cell;

#[derive(Clone, Copy)]
struct Script {
    response: [u8; 8],
    set_hr: i32,
    get_hr: i32,
    bytes: u32,
    fail: Option<CameraFamilyFocusPhase>,
}
impl Default for Script {
    fn default() -> Self {
        Self {
            response: [0, 0x99, 0x31, 0x72, 0, 0, 0, 0],
            set_hr: 0,
            get_hr: 0,
            bytes: 8,
            fail: None,
        }
    }
}
struct Mock {
    scripts: Vec<Script>,
    index: usize,
    topology: bool,
    node: bool,
    packets: Vec<[u8; 8]>,
    closed: usize,
    headers: Vec<[u8; 32]>,
    inter_waits: usize,
    fail_inter_wait: Option<usize>,
}
impl Mock {
    fn new(scripts: Vec<Script>) -> Self {
        Self {
            scripts,
            index: 0,
            topology: false,
            node: false,
            packets: vec![],
            closed: 0,
            headers: vec![],
            inter_waits: 0,
            fail_inter_wait: None,
        }
    }
    fn clean(&self) {
        assert!(!self.topology && !self.node);
    }
}
impl CameraFamilyFocusTransport for Mock {
    fn begin_topology(&mut self) -> Result<u32> {
        self.clean();
        self.topology = true;
        ensure!(
            self.scripts[self.index].fail != Some(CameraFamilyFocusPhase::Topology),
            "scripted topology failure"
        );
        Ok(1)
    }
    fn begin_node(&mut self, node: u32) -> Result<()> {
        assert!(self.topology && !self.node && node == 0);
        self.node = true;
        Ok(())
    }
    fn ks_property(
        &mut self,
        header: &[u8; 32],
        data: &mut [u8; 8],
    ) -> Result<CameraKsCallOutcome> {
        assert!(self.node);
        self.headers.push(*header);
        let s = self.scripts[self.index];
        let set = header[20] == 2;
        let phase = if set {
            CameraFamilyFocusPhase::Set
        } else {
            CameraFamilyFocusPhase::Get
        };
        if set {
            self.packets.push(*data);
        } else {
            assert_eq!(*data, [255, 0, 0, 0, 0, 0, 0, 0]);
            *data = s.response;
        }
        ensure!(s.fail != Some(phase), "scripted {phase:?} failure");
        Ok(CameraKsCallOutcome {
            hresult: if set { s.set_hr } else { s.get_hr },
            bytes_returned: if set { 0 } else { s.bytes },
        })
    }
    fn wait_ms(&mut self, milliseconds: u32) -> Result<()> {
        assert_eq!(milliseconds, 10);
        if !self.node {
            self.clean();
            let ix = self.inter_waits;
            self.inter_waits += 1;
            ensure!(
                self.fail_inter_wait != Some(ix),
                "scripted inter-call wait failure"
            );
            return Ok(());
        }
        ensure!(
            self.scripts[self.index].fail != Some(CameraFamilyFocusPhase::Wait),
            "scripted wait failure"
        );
        Ok(())
    }
    fn end_node(&mut self) {
        self.node = false;
    }
    fn end_topology(&mut self) {
        assert!(!self.node);
        self.topology = false;
        self.index += 1;
        self.closed += 1;
    }
}

#[test]
fn camera_mic_reboot_writes_both_values_then_flash_then_reboot() {
    for adaptive_noise_reduction in [false, true] {
        for crystal_clear_voice in [false, true] {
            let request = CameraFamilyProcessingRequest::SetAudioFunction {
                adaptive_noise_reduction,
                crystal_clear_voice,
            };
            let mut mock = Mock::new(vec![Script::default(); 3]);
            let p = run(&mut mock, 3595, request);
            assert!(p.completed);
            assert_eq!(p.value, Some(CameraFamilyProcessingValue::Acknowledged));
            assert_eq!(
                mock.packets,
                [
                    [
                        0xc0,
                        0x0e,
                        0xa0,
                        adaptive_noise_reduction.into(),
                        crystal_clear_voice.into(),
                        0,
                        0,
                        0
                    ],
                    [0xc0, 3, 0xa8, 0, 0, 0, 0, 0],
                    [0xc0, 0x0e, 7, 2, 0, 0, 0, 0],
                ]
            );
            assert_eq!(mock.inter_waits, 2);
            assert_eq!(mock.closed, 3);
            assert_eq!(
                p.exchanges
                    .iter()
                    .map(|e| e.command.native_rva)
                    .collect::<Vec<_>>(),
                [0x4e450, 0x4d720, 0x4e650]
            );
            assert!(request.initial_command(3596).is_err());
        }
    }
}

#[test]
fn camera_mic_read_decodes_actual_device_bytes_without_a_write_or_save() {
    let request = CameraFamilyProcessingRequest::GetAudioFunction;
    let mut mock = Mock::new(vec![Script {
        response: [0, 0, 0, 1, 0, 0, 0, 0],
        ..Default::default()
    }]);
    let p = run(&mut mock, 3595, request);
    assert!(p.completed);
    assert_eq!(mock.packets, [[0xc0, 0x0e, 0xa1, 0, 0, 0, 0, 0]]);
    assert_eq!(
        p.value,
        Some(CameraFamilyProcessingValue::AudioFunction {
            adaptive_noise_reduction: 1,
            crystal_clear_voice: 0
        })
    );
    assert!(!p.exchanges[0].command.mutates_state);
    assert_eq!(mock.inter_waits, 0);
}

#[test]
fn camera_mic_reboot_short_circuits_each_failed_stage_and_inter_call_wait() {
    let request = CameraFamilyProcessingRequest::SetAudioFunction {
        adaptive_noise_reduction: true,
        crystal_clear_voice: true,
    };
    for stage in 0..3 {
        let mut scripts = vec![Script::default(); 3];
        scripts[stage].set_hr = -1;
        let mut mock = Mock::new(scripts);
        let p = run(&mut mock, 3595, request);
        assert!(!p.completed && p.value.is_none());
        assert_eq!(mock.packets.len(), stage + 1);
        assert_eq!(mock.closed, stage + 1);
    }
    for wait in 0..2 {
        let mut mock = Mock::new(vec![Script::default(); 3]);
        mock.fail_inter_wait = Some(wait);
        let p = run(&mut mock, 3595, request);
        assert!(!p.completed && p.value.is_none());
        assert_eq!(mock.packets.len(), wait + 1);
        assert_eq!(
            p.failure.as_ref().unwrap().phase,
            CameraFamilyFocusPhase::Wait
        );
    }
}

#[test]
fn camera_mic_reboot_cancellation_releases_every_topology_and_never_invents_success() {
    let request = CameraFamilyProcessingRequest::SetAudioFunction {
        adaptive_noise_reduction: false,
        crystal_clear_voice: true,
    };
    let count = Cell::new(0usize);
    let mut mock = Mock::new(vec![Script::default(); 3]);
    execute_camera_family_processing(&mut mock, 3595, request, &|| {
        count.set(count.get() + 1);
        Ok(())
    });
    for cancel_at in 0..count.get() {
        let ticks = Cell::new(0usize);
        let mut mock = Mock::new(vec![Script::default(); 3]);
        let p = execute_camera_family_processing(&mut mock, 3595, request, &|| {
            let now = ticks.get();
            ticks.set(now + 1);
            ensure!(now < cancel_at, "simulated cancellation");
            Ok(())
        });
        validate_camera_family_processing_progress(3595, request, &p).unwrap();
        mock.clean();
        assert!(!p.completed);
        assert_eq!(
            p.value.is_some(),
            p.exchanges.len() == 3 && p.exchanges[2].response().is_some()
        );
    }
}

#[test]
fn camera_mic_receipts_require_flash_reboot_and_both_native_waits() {
    let request = CameraFamilyProcessingRequest::SetAudioFunction {
        adaptive_noise_reduction: true,
        crystal_clear_voice: false,
    };
    let p = run(&mut Mock::new(vec![Script::default(); 3]), 3595, request);
    for stage in 0..3 {
        let mut altered = p.clone();
        altered.exchanges.remove(stage);
        assert!(validate_camera_family_processing_progress(3595, request, &altered).is_err());
    }
    for wait in 0..2 {
        let mut altered = p.clone();
        altered.delays.remove(wait);
        assert!(validate_camera_family_processing_progress(3595, request, &altered).is_err());
        let mut altered = p.clone();
        altered.delays[wait].milliseconds = 0;
        assert!(validate_camera_family_processing_progress(3595, request, &altered).is_err());
    }
}
fn run(
    mock: &mut Mock,
    pid: u32,
    request: CameraFamilyProcessingRequest,
) -> CameraFamilyProcessingProgress {
    let p = execute_camera_family_processing(mock, pid, request, &|| Ok(()));
    validate_camera_family_processing_progress(pid, request, &p).unwrap();
    mock.clean();
    p
}

#[test]
fn family_processing_save_and_processing_packets_match_each_vtable() {
    use CameraFamilyProcessingRequest::*;
    for (pid, request, bytes, rva) in [
        (
            3595,
            SaveUvcExtCmdToFlash,
            [0xc0, 3, 0xa8, 0, 0, 0, 0, 0],
            0x4d720,
        ),
        (
            3596,
            SaveUvcExtCmdToFlash,
            [0xfb, 0, 2, 0, 0, 0, 0, 0],
            0x53b30,
        ),
        (
            3595,
            SetHdr { enabled: true },
            [0xff, 2, 1, 0, 0, 0, 0, 0],
            0x4d530,
        ),
        (
            3596,
            SetHdrBrightness { brightness: 257 },
            [0xff, 7, 1, 0, 0, 0, 0, 0],
            0x4d660,
        ),
        (
            3595,
            SetLdc {
                enabled: false,
                mode: 2,
            },
            [0xff, 1, 0, 3, 2, 0, 0, 0],
            0x4d590,
        ),
        (
            3595,
            SetDynamicNoiseReduction {
                channel: 2,
                enabled: false,
            },
            [0xc0, 14, 2, 0, 0, 0, 0, 0],
            0x4db80,
        ),
        (
            3595,
            SetMirrorVideo { mode: 1 },
            [0xc0, 14, 3, 1, 0, 0, 0, 0],
            0x4dca0,
        ),
        (
            3596,
            SetMirrorVideo { mode: 1 },
            [0x29, 0, 1, 0, 0, 0, 0, 0],
            0x54240,
        ),
        (
            3595,
            SetAdaptiveMjpegQuality {
                enabled: true,
                quality: 7,
            },
            [0xc0, 14, 9, 1, 0, 0, 7, 0],
            0x4e190,
        ),
        (
            3596,
            SetAdaptiveMjpegQuality {
                enabled: false,
                quality: 3,
            },
            [0x2a, 0, 3, 0, 0, 0, 0, 0],
            0x54e60,
        ),
        (
            3596,
            SetFixedMjpegQuality { quality: 80 },
            [0xc0, 14, 11, 80, 0, 0, 0, 0],
            0x4e290,
        ),
        (
            3595,
            SetWatermark {
                enabled: true,
                position: 5,
            },
            [0xc0, 8, 1, 1, 5, 0, 0, 0],
            0x4d810,
        ),
    ] {
        let mut mock = Mock::new(vec![Script::default()]);
        let p = run(&mut mock, pid, request);
        assert!(p.completed);
        assert_eq!(mock.packets, [bytes]);
        assert_eq!(p.exchanges[0].command.native_rva, rva);
        assert_eq!(mock.closed, 1);
        for (i, header) in mock.headers.iter().enumerate() {
            assert_eq!(
                *header,
                crate::camera_focus_family::camera_family_focus_header(pid, 0, i == 0).unwrap()
            );
        }
    }
}

#[test]
fn family_processing_x_dnr_preserves_the_actual_queried_companion() {
    for (channel, enabled, packet) in [
        (1, true, [0x28, 0, 0x31, 1, 0, 0, 0, 0]),
        (2, false, [0x28, 0, 0, 0x72, 0, 0, 0, 0]),
    ] {
        let request = CameraFamilyProcessingRequest::SetDynamicNoiseReduction { channel, enabled };
        let mut mock = Mock::new(vec![Script::default(); 2]);
        let p = run(&mut mock, 3596, request);
        assert_eq!(mock.packets, [[8, 0, 0, 0, 0, 0, 0, 0], packet]);
        assert_eq!(mock.closed, 2);
        assert_eq!(
            p.value,
            Some(CameraFamilyProcessingValue::NativeValue {
                value: enabled.into()
            })
        );
        assert!(!p.exchanges[0].command.mutates_state && p.exchanges[1].command.mutates_state);
        assert_eq!(p.exchanges[1].command.native_rva, 0x53ed0);
    }
}

#[test]
fn family_processing_rejected_query_never_sends_x_dnr_write() {
    use CameraFamilyFocusPhase as P;
    let request = CameraFamilyProcessingRequest::SetDynamicNoiseReduction {
        channel: 1,
        enabled: true,
    };
    for script in [
        Script {
            fail: Some(P::Topology),
            ..Default::default()
        },
        Script {
            set_hr: -1,
            ..Default::default()
        },
        Script {
            get_hr: 1,
            ..Default::default()
        },
        Script {
            bytes: 7,
            ..Default::default()
        },
        Script {
            response: [1; 8],
            ..Default::default()
        },
        Script {
            fail: Some(P::Wait),
            ..Default::default()
        },
    ] {
        let mut mock = Mock::new(vec![script]);
        let p = run(&mut mock, 3596, request);
        assert!(!p.completed && p.value.is_none());
        assert!(mock.packets.len() <= 1);
        assert!(mock.packets.iter().all(|p| p[0] == 8));
        assert_eq!(mock.closed, 1);
    }
    let mut mock = Mock::new(vec![
        Script::default(),
        Script {
            set_hr: -1,
            ..Default::default()
        },
    ]);
    let p = run(&mut mock, 3596, request);
    assert!(!p.completed && p.value.is_none());
    assert_eq!(p.exchanges.len(), 2);
    assert_eq!(mock.closed, 2);
}

#[test]
fn family_processing_cancel_at_each_boundary_releases_both_scans_and_retains_acks() {
    let request = CameraFamilyProcessingRequest::SetDynamicNoiseReduction {
        channel: 2,
        enabled: true,
    };
    let count = Cell::new(0usize);
    let mut mock = Mock::new(vec![Script::default(); 2]);
    execute_camera_family_processing(&mut mock, 3596, request, &|| {
        count.set(count.get() + 1);
        Ok(())
    });
    for cancel_at in 0..count.get() {
        let ticks = Cell::new(0usize);
        let mut mock = Mock::new(vec![Script::default(); 2]);
        let p = execute_camera_family_processing(&mut mock, 3596, request, &|| {
            let now = ticks.get();
            ticks.set(now + 1);
            ensure!(now < cancel_at, "simulated cancellation");
            Ok(())
        });
        validate_camera_family_processing_progress(3596, request, &p).unwrap();
        mock.clean();
        assert!(!p.completed);
        assert_eq!(
            p.failure.as_ref().unwrap().phase,
            CameraFamilyFocusPhase::Validation
        );
        if p.exchanges.len() == 2 && p.exchanges[1].response().is_some() {
            assert_eq!(
                p.value,
                Some(CameraFamilyProcessingValue::NativeValue { value: 1 })
            );
        } else {
            assert!(p.value.is_none());
        }
    }
}

#[test]
fn family_processing_receipt_rejects_tampered_companion_identity_and_invented_save_ack() {
    let request = CameraFamilyProcessingRequest::SetDynamicNoiseReduction {
        channel: 1,
        enabled: true,
    };
    let mut mock = Mock::new(vec![Script::default(); 2]);
    let p = run(&mut mock, 3596, request);
    let mut altered = p.clone();
    altered.exchanges[1].command.bytes[2] ^= 1;
    assert!(validate_camera_family_processing_progress(3596, request, &altered).is_err());
    let mut altered = p.clone();
    altered.exchanges[0].attempts[0].response.as_mut().unwrap()[2] ^= 1;
    assert!(validate_camera_family_processing_progress(3596, request, &altered).is_err());
    let mut altered = p.clone();
    altered.exchanges.pop();
    altered.completed = false;
    altered.value = None;
    assert!(validate_camera_family_processing_progress(3596, request, &altered).is_err());
    assert!(validate_camera_family_processing_progress(3595, request, &p).is_err());
    let request = CameraFamilyProcessingRequest::SaveUvcExtCmdToFlash;
    let mut mock = Mock::new(vec![Script {
        response: [1; 8],
        ..Default::default()
    }]);
    let mut p = run(&mut mock, 3596, request);
    p.value = Some(CameraFamilyProcessingValue::Acknowledged);
    assert!(validate_camera_family_processing_progress(3596, request, &p).is_err());
}
