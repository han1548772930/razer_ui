use super::*;
use razer_device::{
    mapping_native_input::{KeyboardInput, encode_keyboard},
    mapping_native_session::{
        NativeDeviceKind, NativeDriverPath, NativeProductIdentity, NativeTransportError,
    },
};
use std::{collections::BTreeMap, sync::Mutex};

#[derive(Default)]
struct Observed {
    opens: usize,
    closes: usize,
    next: u64,
    operations: Vec<NativeOperation>,
    pending: BTreeMap<NativeSubmissionId, Option<Result<Vec<u8>, NativeTransportError>>>,
    canceled: Vec<NativeSubmissionId>,
    fail_hook: bool,
    rotation_response: Option<Result<Vec<u8>, NativeTransportError>>,
    hold_rotation: bool,
}
struct Sim(Arc<Mutex<Observed>>);
impl NativeSessionTransport for Sim {
    fn open(&mut self, _: &NativeDriverPath) -> Result<(), NativeTransportError> {
        self.0.lock().unwrap().opens += 1;
        Ok(())
    }
    fn submit(
        &mut self,
        operation: NativeOperation,
    ) -> Result<NativeSubmissionId, NativeTransportError> {
        let mut s = self.0.lock().unwrap();
        s.next += 1;
        let id = NativeSubmissionId(s.next);
        let completion = match &operation {
            NativeOperation::ReadInput { .. } => None,
            NativeOperation::Ioctl { ioctl, .. } if matches!(*ioctl, 0x8888_3180 | 0x8888_3184) => {
                if s.hold_rotation {
                    None
                } else {
                    Some(s.rotation_response.take().unwrap_or_else(|| {
                        Ok(if *ioctl == 0x8888_3184 {
                            vec![60, 1, 0, 0]
                        } else {
                            Vec::new()
                        })
                    }))
                }
            }
            NativeOperation::Ioctl { ioctl, .. }
                if *ioctl == hooks::SET_HOOK_IOCTL && s.fail_hook =>
            {
                s.fail_hook = false;
                Some(Err(NativeTransportError::Poll { code: 31 }))
            }
            NativeOperation::Ioctl { ioctl, .. }
                if *ioctl == razer_device::mapping_native_output::DRIVER_SEND_INPUT_IOCTL =>
            {
                None
            }
            _ => Some(Ok(Vec::new())),
        };
        s.operations.push(operation);
        s.pending.insert(id, completion);
        Ok(id)
    }
    fn poll(
        &mut self,
        id: NativeSubmissionId,
        _: Duration,
    ) -> Result<NativeCompletion, NativeTransportError> {
        let mut s = self.0.lock().unwrap();
        let slot = s
            .pending
            .get_mut(&id)
            .ok_or(NativeTransportError::UnknownSubmission(id))?;
        if let Some(result) = slot.take() {
            s.pending.remove(&id);
            Ok(NativeCompletion::Completed(result))
        } else {
            Ok(NativeCompletion::Pending)
        }
    }
    fn cancel(&mut self, id: NativeSubmissionId) -> Result<(), NativeTransportError> {
        let mut s = self.0.lock().unwrap();
        s.canceled.push(id);
        *s.pending
            .get_mut(&id)
            .ok_or(NativeTransportError::UnknownSubmission(id))? =
            Some(Err(NativeTransportError::Poll { code: 995 }));
        Ok(())
    }
    fn close(&mut self) -> Result<(), NativeTransportError> {
        let mut s = self.0.lock().unwrap();
        assert!(s.pending.is_empty());
        s.closes += 1;
        Ok(())
    }
}
fn route() -> NativeRoute {
    NativeRoute::windows(
        NativeDriverPath::new("simulated-mapping-endpoint").unwrap(),
        NativeProductIdentity::razer(515, NativeDeviceKind::Keyboard),
        NativeCapabilitySet::empty(),
    )
}
#[test]
fn outputs_share_one_handle_and_cannot_consume_another_lease() {
    let observed = Arc::new(Mutex::new(Observed::default()));
    let capture = observed.clone();
    let mut owner = Owner::open_with(route(), move || Sim(capture)).unwrap();
    let mut a = owner.route().output();
    let mut b = owner.route().output();
    let id = a
        .submit(encode_keyboard(KeyboardInput {
            scancode: 30,
            ..Default::default()
        }))
        .unwrap();
    assert!(matches!(
        b.poll_completion(id, Duration::ZERO),
        Err(DriverOutputError::UnknownSubmission(_))
    ));
    assert_eq!(
        a.poll_completion(id, Duration::ZERO).unwrap(),
        CompletionPoll::Pending
    );
    assert_eq!(a.close(), Err(DriverOutputError::ExecutionPending));
    a.cancel(id).unwrap();
    assert!(matches!(
        a.poll_completion(id, Duration::ZERO).unwrap(),
        CompletionPoll::Completed(Err(_))
    ));
    a.close().unwrap();
    b.close().unwrap();
    assert_eq!(observed.lock().unwrap().closes, 0);
    owner.shutdown().unwrap();
    let s = observed.lock().unwrap();
    assert_eq!((s.opens, s.closes), (1, 1));
    assert_eq!(s.canceled.len(), 2); // output plus the persistent read
}
#[test]
fn partial_hook_failure_is_owned_and_removed_before_handle_close() {
    let observed = Arc::new(Mutex::new(Observed {
        fail_hook: true,
        ..Default::default()
    }));
    let capture = observed.clone();
    let mut owner = Owner::open_with(route(), move || Sim(capture)).unwrap();
    let hook = HookInput::Keyboard {
        input: KeyboardInput {
            scancode: 30,
            ..Default::default()
        },
        extra_information: 0,
    };
    assert!(owner.route().replace_hooks(vec![hook]).is_err());
    owner.shutdown().unwrap();
    let s = observed.lock().unwrap();
    assert!(s.operations.iter().any(|op| *op == hook.remove()));
    assert_eq!(s.closes, 1);
}
#[test]
fn dropping_pending_output_cancels_and_drains_before_releasing_lease() {
    let observed = Arc::new(Mutex::new(Observed::default()));
    let capture = observed.clone();
    let mut owner = Owner::open_with(route(), move || Sim(capture)).unwrap();
    let mut output = owner.route().output();
    let id = output
        .submit(encode_keyboard(KeyboardInput {
            scancode: 31,
            ..Default::default()
        }))
        .unwrap();
    drop(output);
    assert!(
        !observed
            .lock()
            .unwrap()
            .pending
            .contains_key(&NativeSubmissionId(id.0))
    );
    assert_eq!(observed.lock().unwrap().closes, 0);
    owner.shutdown().unwrap();
}

#[test]
fn rotation_and_output_use_one_retained_handle_and_set_has_no_hidden_readback() {
    use razer_device::mapping_native_rotation::*;
    let observed = Arc::new(Mutex::new(Observed::default()));
    let capture = observed.clone();
    let mut owner = Owner::open_with(route(), move || Sim(capture)).unwrap();
    let mut rotation = owner.route().rotation();
    let mut other = owner.route().rotation();
    let mut output = owner.route().output();
    let id = rotation.submit(RotationRequest::GetAngle).unwrap();
    assert!(matches!(
        other.poll(id, Duration::ZERO),
        Err(RotationError::Driver(DriverOutputError::UnknownSubmission(
            _
        )))
    ));
    assert_eq!(
        rotation.poll(id, Duration::ZERO).unwrap(),
        RotationPoll::Completed(Ok(RotationReply::Angle {
            raw: 316,
            bytes: [60, 1, 0, 0]
        }))
    );
    let id = rotation.submit(RotationRequest::SetAngle(0)).unwrap();
    assert_eq!(
        rotation.poll(id, Duration::ZERO).unwrap(),
        RotationPoll::Completed(Ok(RotationReply::SetComplete))
    );
    rotation.close().unwrap();
    other.close().unwrap();
    let id = output
        .submit(encode_keyboard(KeyboardInput {
            scancode: 30,
            ..Default::default()
        }))
        .unwrap();
    output.cancel(id).unwrap();
    assert!(matches!(
        output.poll_completion(id, Duration::ZERO).unwrap(),
        CompletionPoll::Completed(Err(_))
    ));
    output.close().unwrap();
    assert_eq!(observed.lock().unwrap().closes, 0);
    owner.shutdown().unwrap();
    let state = observed.lock().unwrap();
    assert_eq!((state.opens, state.closes), (1, 1));
    let rotation_ops = state
        .operations
        .iter()
        .filter(|op| {
            matches!(
                op,
                NativeOperation::Ioctl {
                    ioctl: 0x8888_3180 | 0x8888_3184,
                    ..
                }
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        rotation_ops,
        vec![
            NativeOperation::ioctl(0x8888_3184, vec![], 4),
            NativeOperation::ioctl(0x8888_3180, vec![0, 0, 0, 0], 0)
        ]
    );
}

#[test]
fn rotation_cancellation_drains_only_its_operation_before_lease_close() {
    use razer_device::mapping_native_rotation::*;
    let observed = Arc::new(Mutex::new(Observed {
        hold_rotation: true,
        ..Default::default()
    }));
    let capture = observed.clone();
    let mut owner = Owner::open_with(route(), move || Sim(capture)).unwrap();
    let mut rotation = owner.route().rotation();
    let id = rotation.submit(RotationRequest::SetAngle(316)).unwrap();
    assert_eq!(
        rotation.poll(id, Duration::ZERO).unwrap(),
        RotationPoll::Pending
    );
    assert_eq!(
        rotation.close(),
        Err(RotationError::Driver(DriverOutputError::ExecutionPending))
    );
    rotation.cancel(id).unwrap();
    assert!(matches!(
        rotation.poll(id, Duration::ZERO).unwrap(),
        RotationPoll::Completed(Err(_))
    ));
    rotation.close().unwrap();
    assert_eq!(
        observed.lock().unwrap().canceled,
        vec![NativeSubmissionId(id.0)]
    );
    assert_eq!(observed.lock().unwrap().closes, 0);
    owner.shutdown().unwrap();
    assert_eq!(observed.lock().unwrap().closes, 1);
}

#[test]
fn rotation_rejects_truncated_get_and_preserves_driver_failure() {
    use razer_device::mapping_native_rotation::*;
    for response in [
        Ok(vec![1, 2, 3]),
        Err(NativeTransportError::Poll { code: 31 }),
    ] {
        let observed = Arc::new(Mutex::new(Observed {
            rotation_response: Some(response),
            ..Default::default()
        }));
        let capture = observed.clone();
        let mut owner = Owner::open_with(route(), move || Sim(capture)).unwrap();
        let mut rotation = owner.route().rotation();
        let id = rotation.submit(RotationRequest::GetAngle).unwrap();
        assert!(matches!(
            rotation.poll(id, Duration::ZERO).unwrap(),
            RotationPoll::Completed(Err(_))
        ));
        rotation.close().unwrap();
        owner.shutdown().unwrap();
    }
}

#[test]
fn rotation_lost_driver_reply_retires_pending_only_after_owner_cleanup() {
    use razer_device::mapping_native_rotation::*;
    let observed = Arc::new(Mutex::new(Observed {
        hold_rotation: true,
        ..Default::default()
    }));
    let capture = observed.clone();
    let mut owner = Owner::open_with(route(), move || Sim(capture)).unwrap();
    let mut rotation = owner.route().rotation();
    let mut execution =
        RotationExecution::start(&mut rotation, RotationRequest::SetAngle(316), &|| Ok(()))
            .unwrap();
    assert!(execution.pending());
    owner.shutdown().unwrap();
    assert_eq!(observed.lock().unwrap().closes, 1);
    assert!(observed.lock().unwrap().pending.is_empty());
    execution.cancel(&mut rotation);
    execution.poll(&mut rotation, Duration::ZERO, &|| Ok(()));
    assert!(execution.finished());
    assert!(matches!(
        execution.primary.delivery,
        RotationDelivery::Failed(RotationError::Driver(DriverOutputError::Owner(_)))
    ));
    assert!(!execution.primary.errors.is_empty());
    rotation.close().unwrap();
}
