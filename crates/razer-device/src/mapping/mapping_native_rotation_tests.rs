use super::*;
use std::{cell::Cell, collections::VecDeque};

enum Step {
    Submit(RotationRequest, Result<SubmissionId, RotationError>),
    Poll(SubmissionId, Result<RotationPoll, RotationError>),
    Cancel(SubmissionId, Result<(), RotationError>),
    Close(Result<(), RotationError>),
}

struct Device {
    steps: VecDeque<Step>,
    waits: Vec<Duration>,
}

impl Device {
    fn new(steps: impl IntoIterator<Item = Step>) -> Self {
        Self {
            steps: steps.into_iter().collect(),
            waits: Vec::new(),
        }
    }
    fn done(self) {
        assert!(self.steps.is_empty());
    }
}

impl RotationTransport for Device {
    fn submit(&mut self, request: RotationRequest) -> Result<SubmissionId, RotationError> {
        let Some(Step::Submit(expected, result)) = self.steps.pop_front() else {
            panic!("unexpected submit {request:?}");
        };
        assert_eq!(request, expected);
        result
    }
    fn poll(&mut self, id: SubmissionId, wait: Duration) -> Result<RotationPoll, RotationError> {
        let Some(Step::Poll(expected, result)) = self.steps.pop_front() else {
            panic!("unexpected poll {id:?}");
        };
        assert_eq!(id, expected);
        self.waits.push(wait);
        result
    }
    fn cancel(&mut self, id: SubmissionId) -> Result<(), RotationError> {
        let Some(Step::Cancel(expected, result)) = self.steps.pop_front() else {
            panic!("unexpected cancel {id:?}");
        };
        assert_eq!(id, expected);
        result
    }
    fn close(&mut self) -> Result<(), RotationError> {
        let Some(Step::Close(result)) = self.steps.pop_front() else {
            panic!("unexpected close");
        };
        result
    }
}

fn set() -> Step {
    Step::Submit(RotationRequest::SetAngle(316), Ok(SubmissionId(1)))
}
fn set_complete() -> Step {
    Step::Poll(
        SubmissionId(1),
        Ok(RotationPoll::Completed(Ok(RotationReply::SetComplete))),
    )
}
fn get_complete(raw: i32) -> Step {
    Step::Poll(
        SubmissionId(2),
        Ok(RotationPoll::Completed(Ok(RotationReply::Angle {
            raw,
            bytes: raw.to_le_bytes(),
        }))),
    )
}
fn valid() -> Result<(), RotationError> {
    Ok(())
}

#[test]
fn ioctl_and_four_byte_layout_match_native_calls() {
    let get = RotationRequest::GetAngle;
    assert_eq!(get.ioctl(), 0x8888_3184);
    assert_eq!(get.input(), None);
    assert_eq!(get.output_size(), 4);
    let positive = RotationRequest::SetAngle(316);
    assert_eq!(positive.ioctl(), 0x8888_3180);
    assert_eq!(positive.input(), Some([60, 1, 0, 0]));
    assert_eq!(positive.output_size(), 0);
    assert_eq!(
        RotationRequest::SetAngle(-44).input(),
        Some([212, 255, 255, 255])
    );
}

#[test]
fn native_task_normalizes_enabled_angle_and_zeros_disabled_rotation() {
    assert_eq!(native_angle_for_task(true, -44).unwrap(), 316);
    assert_eq!(native_angle_for_task(true, -1).unwrap(), 359);
    assert_eq!(native_angle_for_task(true, 0).unwrap(), 0);
    assert_eq!(native_angle_for_task(true, 44).unwrap(), 44);
    assert_eq!(native_angle_for_task(false, -44).unwrap(), 0);
    assert_eq!(native_angle_for_task(false, 44).unwrap(), 0);
    for degrees in [-128, -45, 45, 127] {
        assert_eq!(
            native_angle_for_task(true, degrees),
            Err(RotationError::EditorAngleOutOfRange(degrees))
        );
    }
}

#[test]
fn reader_preserves_raw_int32_without_normalizing_device_values() {
    for (bytes, raw) in [
        ([60, 1, 0, 0], 316),
        ([212, 255, 255, 255], -44),
        ([255, 255, 255, 127], i32::MAX),
        ([0, 0, 0, 128], i32::MIN),
    ] {
        assert_eq!(
            RotationReply::from_read_bytes(&bytes).unwrap(),
            RotationReply::Angle { raw, bytes }
        );
    }
    for length in [0, 1, 3, 5, 91] {
        assert_eq!(
            RotationReply::from_read_bytes(&vec![0; length]),
            Err(RotationError::ReadLength(length))
        );
    }
}

#[test]
fn queue_acceptance_pending_and_final_completion_are_distinct() {
    let mut device = Device::new([
        set(),
        Step::Poll(SubmissionId(1), Ok(RotationPoll::Pending)),
        set_complete(),
    ]);
    let mut execution =
        RotationExecution::start(&mut device, RotationRequest::SetAngle(316), &valid).unwrap();
    assert!(execution.pending());
    assert!(!execution.primary.driver_completed());
    execution.poll(&mut device, Duration::from_millis(9), &valid);
    assert!(execution.pending());
    execution.poll(&mut device, Duration::from_millis(5), &valid);
    assert!(execution.finished());
    assert!(execution.primary.driver_completed());
    assert_eq!(
        device.waits,
        [Duration::from_millis(9), Duration::from_millis(5)]
    );
    device.done();
}

#[test]
fn read_only_get_is_an_observation() {
    let mut device = Device::new([
        Step::Submit(RotationRequest::GetAngle, Ok(SubmissionId(2))),
        get_complete(-44),
    ]);
    let mut execution =
        RotationExecution::start(&mut device, RotationRequest::GetAngle, &valid).unwrap();
    execution.poll(&mut device, Duration::ZERO, &valid);
    assert!(execution.finished());
    assert_eq!(
        execution.primary.delivery,
        RotationDelivery::Completed(RotationReply::Angle {
            raw: -44,
            bytes: [212, 255, 255, 255]
        })
    );
    device.done();
}

#[test]
fn unsupported_platform_and_driver_rejection_do_not_start_a_getter() {
    let mut platform = UnsupportedRotationTransport;
    let mut execution =
        RotationExecution::start(&mut platform, RotationRequest::SetAngle(316), &valid).unwrap();
    assert_eq!(
        execution.primary.delivery,
        RotationDelivery::Failed(DriverOutputError::UnsupportedPlatform.into())
    );
    execution.poll(&mut platform, Duration::ZERO, &valid);
    assert!(execution.finished());
    let error = win32_error(DriverOperation::Submit, 5);
    let mut device = Device::new([Step::Submit(
        RotationRequest::SetAngle(316),
        Err(error.clone()),
    )]);
    let mut execution =
        RotationExecution::start(&mut device, RotationRequest::SetAngle(316), &valid).unwrap();
    execution.poll(&mut device, Duration::ZERO, &valid);
    assert_eq!(execution.primary.delivery, RotationDelivery::Failed(error));
    device.done();
}

#[test]
fn completed_failure_is_retained_without_another_request() {
    let error = win32_error(DriverOperation::Complete, 995);
    let mut device = Device::new([
        set(),
        Step::Poll(
            SubmissionId(1),
            Ok(RotationPoll::Completed(Err(error.clone()))),
        ),
    ]);
    let mut execution =
        RotationExecution::start(&mut device, RotationRequest::SetAngle(316), &valid).unwrap();
    execution.poll(&mut device, Duration::ZERO, &valid);
    assert_eq!(execution.primary.delivery, RotationDelivery::Failed(error));
    assert!(execution.finished());
    device.done();
}

#[test]
fn polling_error_keeps_pending_request_and_can_observe_later_completion() {
    let error = win32_error(DriverOperation::Complete, 6);
    let mut device = Device::new([
        set(),
        Step::Poll(SubmissionId(1), Err(error.clone())),
        set_complete(),
    ]);
    let mut execution =
        RotationExecution::start(&mut device, RotationRequest::SetAngle(316), &valid).unwrap();
    execution.poll(&mut device, Duration::ZERO, &valid);
    assert!(execution.pending());
    assert_eq!(execution.primary.errors, [error]);
    execution.poll(&mut device, Duration::ZERO, &valid);
    assert!(execution.primary.driver_completed());
    device.done();
}

#[test]
fn cancel_preserves_pending_buffers_until_drain() {
    let mut device = Device::new([
        set(),
        Step::Cancel(SubmissionId(1), Ok(())),
        Step::Poll(SubmissionId(1), Ok(RotationPoll::Pending)),
        set_complete(),
        Step::Close(Ok(())),
    ]);
    let mut execution =
        RotationExecution::start(&mut device, RotationRequest::SetAngle(316), &valid).unwrap();
    execution.cancel(&mut device);
    execution.cancel(&mut device);
    assert_eq!(execution.stopped, Some(RotationError::Canceled));
    assert!(execution.pending());
    assert_eq!(
        execution.close_if_drained(&mut device),
        Err(DriverOutputError::ExecutionPending.into())
    );
    execution.poll(&mut device, Duration::ZERO, &valid);
    assert!(execution.pending());
    execution.poll(&mut device, Duration::ZERO, &valid);
    assert!(execution.primary.driver_completed());
    execution.close_if_drained(&mut device).unwrap();
    device.done();
}

#[test]
fn cancel_error_is_observable_and_does_not_retire_the_kernel_operation() {
    let error = win32_error(DriverOperation::Cancel, 5);
    let mut device = Device::new([
        set(),
        Step::Cancel(SubmissionId(1), Err(error.clone())),
        set_complete(),
    ]);
    let mut execution =
        RotationExecution::start(&mut device, RotationRequest::SetAngle(316), &valid).unwrap();
    execution.cancel(&mut device);
    assert!(execution.pending());
    assert_eq!(execution.primary.errors, [error]);
    execution.poll(&mut device, Duration::ZERO, &valid);
    assert!(execution.finished());
    device.done();
}

#[test]
fn invalid_retained_identity_before_submission_sends_nothing() {
    let mut device = Device::new([]);
    assert_eq!(
        RotationExecution::start(&mut device, RotationRequest::SetAngle(316), &|| Err(
            RotationError::SessionInvalid
        )),
        Err(RotationError::SessionInvalid)
    );
    device.done();
}

#[test]
fn identity_change_after_submit_cancels_and_retains_receipt() {
    let mut device = Device::new([set(), Step::Cancel(SubmissionId(1), Ok(())), set_complete()]);
    let calls = Cell::new(0);
    let mut execution =
        RotationExecution::start(&mut device, RotationRequest::SetAngle(316), &|| {
            let call = calls.get() + 1;
            calls.set(call);
            if call == 1 {
                Ok(())
            } else {
                Err(RotationError::SessionInvalid)
            }
        })
        .unwrap();
    assert!(execution.pending());
    assert_eq!(execution.stopped, Some(RotationError::SessionInvalid));
    execution.poll(&mut device, Duration::ZERO, &valid);
    assert!(execution.primary.driver_completed());
    device.done();
}

#[test]
fn identity_change_after_completed_write_preserves_ack_without_another_request() {
    let mut device = Device::new([set(), set_complete()]);
    let mut execution =
        RotationExecution::start(&mut device, RotationRequest::SetAngle(316), &valid).unwrap();
    let calls = Cell::new(0);
    execution.poll(&mut device, Duration::ZERO, &|| {
        let call = calls.get() + 1;
        calls.set(call);
        if call == 1 {
            Ok(())
        } else {
            Err(RotationError::SessionInvalid)
        }
    });
    assert!(execution.primary.driver_completed());
    assert_eq!(execution.stopped, Some(RotationError::SessionInvalid));
    device.done();
}

#[test]
fn close_failure_preserves_execution_without_resetting_angle() {
    let error = win32_error(DriverOperation::Close, 6);
    let mut device = Device::new([set(), set_complete(), Step::Close(Err(error.clone()))]);
    let mut execution =
        RotationExecution::start(&mut device, RotationRequest::SetAngle(316), &valid).unwrap();
    execution.poll(&mut device, Duration::ZERO, &valid);
    assert_eq!(execution.close_if_drained(&mut device), Err(error));
    assert_eq!(execution.primary.request, RotationRequest::SetAngle(316));
    assert!(execution.primary.driver_completed());
    device.done();
}
