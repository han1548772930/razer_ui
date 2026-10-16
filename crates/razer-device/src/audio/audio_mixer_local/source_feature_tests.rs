//! Current AudioMixer host report channels, with no device or service process.
use super::*;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
};

enum Step {
    Send(Vec<u8>),
    SendError(Vec<u8>),
    Get { selector: u8, bytes: Vec<u8> },
    GetError { selector: u8 },
    Output(Vec<u8>),
    OutputError(Vec<u8>),
    Input { selector: u8, bytes: Vec<u8> },
    InputError { selector: u8 },
}

struct Transport {
    steps: RefCell<VecDeque<Step>>,
    sends: Cell<usize>,
}

impl Transport {
    fn new(steps: impl IntoIterator<Item = Step>) -> Self {
        Self {
            steps: RefCell::new(steps.into_iter().collect()),
            sends: Cell::new(0),
        }
    }

    fn finish(&self) {
        assert!(
            self.steps.borrow().is_empty(),
            "Unconsumed source transfers"
        );
    }
}

impl FeatureTransport for Transport {
    fn send_feature(&self, _: &[u8]) -> anyhow::Result<()> {
        panic!("AudioMixer source SET uses the collection Feature buffer")
    }

    fn get_feature(&self, _: &mut [u8]) -> anyhow::Result<usize> {
        panic!("AudioMixer source GET selector is independent of descriptor IDs")
    }

    fn send_feature_at_collection_max(&self, report: &[u8]) -> anyhow::Result<()> {
        self.sends.set(self.sends.get() + 1);
        match self.steps.borrow_mut().pop_front() {
            Some(Step::Send(expected)) => {
                assert_eq!(report, expected);
                Ok(())
            }
            Some(Step::SendError(expected)) => {
                assert_eq!(report, expected);
                anyhow::bail!("Simulated Feature OUT failure")
            }
            _ => panic!("Unexpected source SET"),
        }
    }

    fn get_feature_at_collection_max(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        assert_eq!(report.len(), 64);
        match self
            .steps
            .borrow_mut()
            .pop_front()
            .expect("Unexpected source GET")
        {
            Step::Get { selector, bytes } => {
                assert_eq!(report[0], selector);
                assert!(report[1..].iter().all(|byte| *byte == 0));
                report[..bytes.len()].copy_from_slice(&bytes);
                Ok(bytes.len())
            }
            Step::GetError { selector } => {
                assert_eq!(report[0], selector);
                anyhow::bail!("Simulated Feature I/O failure")
            }
            _ => panic!("Expected Feature GET"),
        }
    }

    fn write_output(&self, report: &[u8]) -> anyhow::Result<()> {
        self.sends.set(self.sends.get() + 1);
        match self.steps.borrow_mut().pop_front() {
            Some(Step::Output(expected)) => {
                assert_eq!(report, expected);
                Ok(())
            }
            Some(Step::OutputError(expected)) => {
                assert_eq!(report, expected);
                anyhow::bail!("Simulated Output failure")
            }
            _ => panic!("Unexpected source Output"),
        }
    }

    fn get_input(&self, report: &mut [u8]) -> anyhow::Result<usize> {
        assert_eq!(report.len(), 16);
        match self
            .steps
            .borrow_mut()
            .pop_front()
            .expect("Unexpected source Input")
        {
            Step::Input { selector, bytes } => {
                assert_eq!(report[0], selector);
                assert!(report[1..].iter().all(|byte| *byte == 0));
                report[..bytes.len()].copy_from_slice(&bytes);
                Ok(bytes.len())
            }
            Step::InputError { selector } => {
                assert_eq!(report[0], selector);
                anyhow::bail!("Simulated Input failure")
            }
            _ => panic!("Expected source Input"),
        }
    }

    fn report_lengths(&self) -> anyhow::Result<crate::backend::ReportLengths> {
        Ok(crate::backend::ReportLengths {
            feature: [(7, 64)].into(),
            ..Default::default()
        })
    }

    fn metadata(&self) -> Value {
        json!({"simulated":true})
    }
}

fn packet(prefix: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 64];
    bytes[..prefix.len()].copy_from_slice(prefix);
    bytes
}

fn output_packet(prefix: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0; 66];
    bytes[..prefix.len()].copy_from_slice(prefix);
    bytes
}

fn execute_command(io: &Transport, command: Command) -> anyhow::Result<Receipt> {
    execute(io, &command, &|| Ok(()), &|_| {
        panic!("Feature source task must not issue an unrelated control output")
    })
}

#[test]
fn meter_color_keeps_source_set7_get18_even_when_only_feature7_is_advertised() {
    let returned = packet(&[0x7f, 12, 34, 56]);
    let io = Transport::new([
        Step::Send(packet(&[7, 0, 31, 0, 0, 0, 5, 15, 18, 1, 7, 10, 20, 30])),
        Step::Get {
            selector: 18,
            bytes: returned.clone(),
        },
    ]);
    assert!(!io.report_lengths().unwrap().feature.contains_key(&18));
    let receipt = execute_command(
        &io,
        Command::Lighting {
            action: lighting::Action::Effect {
                region: 7,
                effect: 10,
                flags: 0,
                colors: vec![10, 20, 30],
            },
        },
    )
    .unwrap();
    assert!(receipt.transport_completed);
    assert_eq!(
        receipt.response["observations"][0]["feature_reply"],
        json!(returned)
    );
    assert!(receipt.profile_updates.is_empty());
    io.finish();
}

#[test]
fn bleep_fader_and_channel_mute_preserve_their_independent_source_tasks() {
    for enabled in [false, true] {
        let returned = packet(&[7, 1, 2, 3]);
        let io = Transport::new([
            Step::Send(packet(&[
                7,
                0,
                0,
                0,
                0,
                0,
                0,
                2,
                12,
                0,
                112,
                0,
                20,
                5,
                u8::from(!enabled),
            ])),
            Step::Get {
                selector: 7,
                bytes: returned.clone(),
            },
            Step::Send(packet(&[7, 0, 31, 0, 0, 0, 4, 15, 17, 1, 6, 2])),
            Step::Send(packet(&[
                7,
                0,
                31,
                0,
                0,
                0,
                3,
                8,
                16,
                0,
                5,
                u8::from(enabled),
            ])),
        ]);
        let bleep = execute_command(&io, Command::Bleep { enabled }).unwrap();
        assert_eq!(bleep.response["feature_reply"], json!(returned));
        let fader = execute_command(&io, Command::Fader { region: 6, mode: 2 }).unwrap();
        assert!(fader.transport_completed && fader.response.is_null());
        let mute = execute_command(
            &io,
            Command::ChannelMute {
                channel: 5,
                enabled,
                profile_updates: vec![("/device/micSetup/isMuted".into(), json!(enabled))],
            },
        )
        .unwrap();
        assert!(mute.transport_completed && mute.response.is_null());
        assert_eq!(
            mute.profile_updates,
            vec![("/device/micSetup/isMuted".into(), json!(enabled))]
        );
        io.finish();
    }
}

#[test]
fn failed_feature_read_and_cancellation_cannot_be_reported_as_completed_tasks() {
    let action = lighting::Action::Effect {
        region: 7,
        effect: 10,
        flags: 0,
        colors: vec![10, 20, 30],
    };
    let io = Transport::new((0..20).flat_map(|_| {
        [
            Step::Send(packet(&[7, 0, 31, 0, 0, 0, 5, 15, 18, 1, 7, 10, 20, 30])),
            Step::GetError { selector: 18 },
        ]
    }));
    assert!(
        execute_command(
            &io,
            Command::Lighting {
                action: action.clone()
            }
        )
        .is_err()
    );
    io.finish();

    let io = Transport::new([]);
    assert!(
        execute(
            &io,
            &Command::Lighting { action },
            &|| anyhow::bail!("Canceled before source SET"),
            &|_| panic!("Unrelated control output"),
        )
        .is_err()
    );
    io.finish();

    let io = Transport::new([Step::Send(packet(&[
        7, 0, 31, 0, 0, 0, 5, 15, 18, 1, 7, 10, 20, 30,
    ]))]);
    assert!(
        execute(
            &io,
            &Command::Lighting {
                action: lighting::Action::Effect {
                    region: 7,
                    effect: 10,
                    flags: 0,
                    colors: vec![10, 20, 30],
                },
            },
            &|| {
                ensure!(io.sends.get() == 0, "Canceled after source SET");
                Ok(())
            },
            &|_| panic!("Unrelated control output"),
        )
        .is_err()
    );
    io.finish();
}

#[test]
fn transient_host_feature_failures_resend_the_exact_source_packet() {
    let report = packet(&[7, 0, 31, 0, 0, 0, 5, 15, 18, 1, 7, 10, 20, 30]);
    let returned = packet(&[0x7f, 12, 34, 56]);
    let io = Transport::new([
        Step::SendError(report.clone()),
        Step::Send(report.clone()),
        Step::GetError { selector: 18 },
        Step::Send(report),
        Step::Get {
            selector: 18,
            bytes: returned.clone(),
        },
    ]);
    let receipt = execute_command(
        &io,
        Command::Lighting {
            action: lighting::Action::Effect {
                region: 7,
                effect: 10,
                flags: 0,
                colors: vec![10, 20, 30],
            },
        },
    )
    .unwrap();
    assert_eq!(io.sends.get(), 3);
    assert!(receipt.transport_completed);
    assert_eq!(
        receipt.response["observations"][0]["feature_reply"],
        json!(returned)
    );
    io.finish();
}

#[test]
fn optical_gain_retries_its_query_and_copies_only_an_actual_input_byte() {
    let query = output_packet(&[4, 95, 252, 0, 44]);
    let input = vec![18, 0x11, 0x22, 0xa5];
    let io = Transport::new([
        Step::OutputError(query.clone()),
        Step::Output(query.clone()),
        Step::InputError { selector: 18 },
        Step::Output(query),
        Step::Input {
            selector: 18,
            bytes: input.clone(),
        },
        Step::Output(output_packet(&[19, 95, 252, 0, 44, 0, 0xa5, 12, 12])),
    ]);
    let receipt = execute_command(
        &io,
        Command::AnalogGain {
            key: "optical".into(),
            value: -12.9,
        },
    )
    .unwrap();
    assert!(receipt.transport_completed);
    assert_eq!(receipt.response["query"], json!(&input[1..]));
    assert_eq!(io.sends.get(), 4);
    io.finish();
}

#[test]
fn optical_gain_never_sends_a_copy_write_after_an_exhausted_query() {
    let io = Transport::new((0..20).flat_map(|_| {
        [
            Step::Output(output_packet(&[4, 95, 252, 0, 44])),
            Step::InputError { selector: 18 },
        ]
    }));
    assert!(
        execute_command(
            &io,
            Command::AnalogGain {
                key: "optical".into(),
                value: -12.,
            }
        )
        .is_err()
    );
    assert_eq!(io.sends.get(), 20);
    io.finish();
}

#[test]
fn phantom_power_preserves_source_output_input_and_actual_response() {
    for enabled in [false, true] {
        let input = vec![18, 0x42, 0x31];
        let report = output_packet(&[10, if enabled { 5 } else { 2 }]);
        let io = Transport::new([
            Step::Output(report.clone()),
            Step::InputError { selector: 18 },
            Step::Output(report),
            Step::Input {
                selector: 18,
                bytes: input.clone(),
            },
        ]);
        let receipt = execute_command(&io, Command::PhantomPower { enabled }).unwrap();
        assert!(receipt.transport_completed);
        assert_eq!(receipt.response["input"], json!(input));
        io.finish();
    }
}

#[test]
fn source_analog_and_microphone_output_controls_do_not_add_getters() {
    for (command, prefix) in [
        (
            Command::AnalogGain {
                key: "lineIn".into(),
                value: -4.,
            },
            vec![3, 24, 0, 192, 14, 10, 10],
        ),
        (
            Command::AnalogGain {
                key: "playbackMixReducer".into(),
                value: -31.125,
            },
            vec![3, 24, 0, 192, 40, 41, 41],
        ),
        (
            Command::AnalogGain {
                key: "lineOutReducer".into(),
                value: -32.8125,
            },
            vec![3, 24, 0, 192, 44, 87, 87],
        ),
        (
            Command::InputMicGain { value: 0. },
            vec![3, 24, 0, 192, 16, 16, 16],
        ),
        (
            Command::InputMicMute { enabled: true },
            vec![3, 24, 0, 192, 148, 196, 192],
        ),
    ] {
        let report = output_packet(&prefix);
        let io = Transport::new([Step::OutputError(report.clone()), Step::Output(report)]);
        let receipt = execute_command(&io, command).unwrap();
        assert!(receipt.transport_completed && receipt.response.is_null());
        io.finish();
    }
}

#[test]
fn cancellation_after_source_output_does_not_read_or_retry() {
    let io = Transport::new([Step::Output(output_packet(&[10, 5]))]);
    let result = execute(
        &io,
        &Command::PhantomPower { enabled: true },
        &|| {
            ensure!(io.sends.get() == 0, "Canceled after source Output");
            Ok(())
        },
        &|_| panic!("Unrelated control output"),
    );
    assert!(result.is_err());
    io.finish();
}
