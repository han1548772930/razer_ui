use super::*;
use std::cell::RefCell;
use std::collections::VecDeque;

struct Simulated {
    sent: RefCell<Vec<Vec<u8>>>,
    replies: RefCell<VecDeque<(u8, Vec<u8>)>>,
}
impl Simulated {
    fn new(replies: impl IntoIterator<Item = (u8, Vec<u8>)>) -> Self {
        Self {
            sent: RefCell::new(Vec::new()),
            replies: RefCell::new(replies.into_iter().collect()),
        }
    }
}
impl FeatureTransport for Simulated {
    fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
        anyhow::bail!("Source controller requires collection Feature OUT")
    }
    fn get_feature(&self, _: &mut [u8]) -> anyhow::Result<usize> {
        anyhow::bail!("Source controller requires collection Feature IN")
    }
    fn send_feature_at_collection_max(&self, bytes: &[u8]) -> anyhow::Result<()> {
        self.sent.borrow_mut().push(bytes.to_vec());
        Ok(())
    }
    fn get_feature_at_collection_max(&self, bytes: &mut [u8]) -> anyhow::Result<usize> {
        let (status, data) = self
            .replies
            .borrow_mut()
            .pop_front()
            .context("No simulated controller response")?;
        let sent = self.sent.borrow();
        bytes.copy_from_slice(sent.last().context("IN before OUT")?);
        bytes[0] = 0xff; // The current host discards this prefix before parsing.
        bytes[1] = status;
        bytes[6] = u8::try_from(data.len())?;
        bytes[9..89].fill(0);
        bytes[9..9 + data.len()].copy_from_slice(&data);
        Ok(bytes.len())
    }
    fn metadata(&self) -> serde_json::Value {
        serde_json::json!({"simulated": true})
    }
}
fn run(io: &Simulated, operation: &LightingOperation) -> anyhow::Result<LightingResult> {
    LightingProtocol::new(io, capability(2636).unwrap(), |_| Ok(())).execute(operation)
}

#[test]
fn source_spectrum_rate_and_static_custom_palette_use_exact_payloads() {
    let io = Simulated::new((0..4).map(|_| (2, vec![])));
    let mut protocol = LightingProtocol::new(&io, capability(2636).unwrap(), |_| Ok(()));
    for rate in [SpectrumRate::Slow, SpectrumRate::Medium, SpectrumRate::Fast] {
        let result = protocol
            .execute(&LightingOperation::Set {
                profile_slot: 2,
                effect: ControllerEffect::Spectrum { rate },
            })
            .unwrap();
        assert_eq!(result.acknowledged_writes, 1);
        assert!(result.effect.is_none());
    }
    protocol
        .execute(&LightingOperation::Set {
            profile_slot: 3,
            effect: ControllerEffect::Static {
                index: 9,
                custom_colors: vec![[0x11, 0x22, 0x33], [0xaa, 0xbb, 0xcc]],
            },
        })
        .unwrap();
    let sent = io.sent.borrow();
    for (index, rate) in [1, 0, 2].into_iter().enumerate() {
        assert_eq!(&sent[index][6..15], &[6, 15, 2, 2, 0, 3, 0, rate, 0]);
    }
    assert_eq!(
        &sent[3][6..21],
        &[
            12, 15, 2, 3, 0, 13, 9, 1, 2, 0x11, 0x22, 0x33, 0xaa, 0xbb, 0xcc
        ]
    );
    for (index, report) in sent.iter().enumerate() {
        assert_eq!(report.len(), 91);
        assert_eq!(report[0], 4);
        assert_eq!(report[2], index as u8);
        assert_eq!(report[89], report[3..89].iter().fold(0, |a, b| a ^ b));
    }
}

#[test]
fn source_extended_static_read_requires_second_observed_reply() {
    let io = Simulated::new([
        (2, vec![2, 0, 13, 0, 1, 0]),
        (2, vec![2, 0, 13, 9, 1, 2, 1, 2, 3, 4, 5, 6]),
    ]);
    let result = run(&io, &LightingOperation::Read { profile_slot: 2 }).unwrap();
    let effect = result.effect.unwrap();
    assert_eq!(effect.flags, 9);
    assert_eq!(effect.color_bytes, vec![1, 2, 3, 4, 5, 6]);
    let sent = io.sent.borrow();
    assert_eq!(&sent[0][6..11], &[80, 15, 130, 2, 0]);
    assert_eq!(&sent[1][6..14], &[5, 15, 130, 2, 0, 13, 0, 1]);
    assert_eq!(result.acknowledged_writes, 0);
    let truncated = Simulated::new([(2, vec![2, 0, 13, 0, 1])]);
    assert!(run(&truncated, &LightingOperation::Read { profile_slot: 2 }).is_err());
}

#[test]
fn initialization_corrects_pc_mode_before_replaying_local_effect() {
    let io = Simulated::new([(2, vec![1]), (2, vec![]), (2, vec![])]);
    let result = run(
        &io,
        &LightingOperation::Initialize {
            profile_slot: 1,
            effect: ControllerEffect::Off,
        },
    )
    .unwrap();
    let init = result.initialization.unwrap();
    assert_eq!(init.attempts, 1);
    assert_eq!(init.observed_option, Some(1));
    assert!(init.correction_acknowledged);
    assert!(init.failures.is_empty());
    assert_eq!(result.acknowledged_writes, 2);
    let sent = io.sent.borrow();
    assert_eq!(&sent[0][6..10], &[1, 0, 214, 0]);
    assert_eq!(&sent[1][6..10], &[1, 0, 86, 0]);
    assert_eq!(&sent[2][6..15], &[6, 15, 2, 1, 0, 0, 0, 0, 0]);
}

#[test]
fn initialization_exhaustion_is_diagnostic_and_never_a_fake_correction() {
    let io = Simulated::new([(5, vec![]), (5, vec![]), (5, vec![]), (2, vec![])]);
    let result = run(
        &io,
        &LightingOperation::Initialize {
            profile_slot: 1,
            effect: ControllerEffect::Off,
        },
    )
    .unwrap();
    let init = result.initialization.unwrap();
    assert_eq!(init.attempts, 3);
    assert_eq!(init.failures.len(), 3);
    assert_eq!(init.observed_option, None);
    assert!(!init.correction_acknowledged);
    assert_eq!(result.acknowledged_writes, 1);
    assert_eq!(io.sent.borrow().len(), 4);
}

#[test]
fn cancellation_propagates_outside_initialization_native_error_catch() {
    let io = Simulated::new([(5, vec![])]);
    let mut protocol = LightingProtocol::new(&io, capability(2636).unwrap(), |duration| {
        anyhow::ensure!(duration != Duration::from_millis(100), "canceled");
        Ok(())
    });
    let error = protocol
        .execute(&LightingOperation::Initialize {
            profile_slot: 1,
            effect: ControllerEffect::Off,
        })
        .unwrap_err();
    assert!(error.to_string().contains("canceled"));
    assert_eq!(io.sent.borrow().len(), 1);
}

#[test]
fn brightness_uses_observed_region_and_source_floor_and_ceil_conversions() {
    let io = Simulated::new([(2, vec![4, 60, 0, 1, 1]), (2, vec![1, 4, 127]), (2, vec![])]);
    let result = run(&io, &LightingOperation::Brightness { value: 50 }).unwrap();
    assert_eq!(result.acknowledged_writes, 1);
    assert_eq!(
        result.brightness,
        Some(ObservedBrightness {
            profile_slot: 1,
            region_id: 4,
            value: 50,
        })
    );
    let sent = io.sent.borrow();
    assert_eq!(&sent[0][6..9], &[80, 15, 128]);
    assert_eq!(&sent[1][6..12], &[3, 15, 132, 1, 4, 0]);
    assert_eq!(&sent[2][6..12], &[3, 15, 4, 1, 0, 127]);
    let io = Simulated::new([(2, vec![]), (2, vec![1, 0, 127])]);
    let result = run(&io, &LightingOperation::ReadBrightness).unwrap();
    assert_eq!(result.brightness.unwrap().value, 50);
    let io = Simulated::new([(2, vec![]), (2, vec![1, 0, 127])]);
    let result = run(&io, &LightingOperation::Brightness { value: 50 }).unwrap();
    assert_eq!(result.acknowledged_writes, 0);
    assert_eq!(result.brightness.unwrap().value, 50);
    assert_eq!(io.sent.borrow().len(), 2);
}

#[test]
fn malformed_palette_or_unmounted_product_never_reaches_transport() {
    let io = Simulated::new([]);
    assert!(
        run(
            &io,
            &LightingOperation::Set {
                profile_slot: 1,
                effect: ControllerEffect::Static {
                    index: 16,
                    custom_colors: vec![],
                }
            }
        )
        .is_err()
    );
    assert!(
        run(
            &io,
            &LightingOperation::Set {
                profile_slot: 1,
                effect: ControllerEffect::Static {
                    index: 0,
                    custom_colors: vec![[0, 0, 0]; 9],
                }
            }
        )
        .is_err()
    );
    assert!(capability(2676).is_none());
    assert!(
        LightingOperation::Read { profile_slot: 1 }
            .validate_product(4115)
            .is_err()
    );
    assert!(io.sent.borrow().is_empty());
}
