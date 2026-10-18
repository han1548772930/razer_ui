//! Rotation requests borrow the retained driver; closing a lease never closes
//! another device, output or capture owner's handle.
use super::*;
use razer_device::mapping_native_rotation::{
    RotationError, RotationPoll, RotationReply, RotationTransport,
};
use std::collections::BTreeMap;

pub(crate) struct Rotation {
    sender: mpsc::Sender<Command>,
    pending: BTreeMap<SubmissionId, RotationRequest>,
    closed: bool,
    lifetime: Arc<Lifetime>,
}
impl Rotation {
    pub(super) fn new(sender: mpsc::Sender<Command>, lifetime: Arc<Lifetime>) -> Self {
        Self {
            sender,
            pending: BTreeMap::new(),
            closed: false,
            lifetime,
        }
    }
    fn retired(&mut self, id: SubmissionId, error: impl std::fmt::Display) -> RotationPoll {
        // Channel disconnection alone does not prove kernel buffers are free.
        // The worker publishes this only after its transport has been dropped.
        self.lifetime.wait();
        self.pending.remove(&id);
        RotationPoll::Completed(Err(output_error(error).into()))
    }
}
impl RotationTransport for Rotation {
    fn submit(&mut self, request: RotationRequest) -> Result<SubmissionId, RotationError> {
        if self.closed {
            return Err(DriverOutputError::Closed.into());
        }
        let (reply, receive) = mpsc::sync_channel(1);
        self.sender
            .send(Command::RotationSubmit(request, reply))
            .map_err(output_error)?;
        let id = receive.recv().map_err(output_error)??;
        self.pending.insert(id, request);
        Ok(id)
    }
    fn poll(&mut self, id: SubmissionId, wait: Duration) -> Result<RotationPoll, RotationError> {
        let request = self
            .pending
            .get(&id)
            .copied()
            .ok_or(DriverOutputError::UnknownSubmission(id))?;
        let (reply, receive) = mpsc::sync_channel(1);
        if let Err(error) = self.sender.send(Command::RotationPoll(id, wait, reply)) {
            return Ok(self.retired(id, error));
        }
        let received = match receive.recv() {
            Ok(result) => result?,
            Err(error) => return Ok(self.retired(id, error)),
        };
        match received {
            NativeCompletion::Pending => Ok(RotationPoll::Pending),
            NativeCompletion::Completed(result) => {
                self.pending.remove(&id);
                let reply = result
                    .map_err(|error| RotationError::from(output_error(error)))
                    .and_then(|bytes| match request {
                        RotationRequest::GetAngle => RotationReply::from_read_bytes(&bytes),
                        RotationRequest::SetAngle(_) if bytes.is_empty() => {
                            Ok(RotationReply::SetComplete)
                        }
                        RotationRequest::SetAngle(_) => Err(RotationError::UnexpectedReply),
                    });
                Ok(RotationPoll::Completed(reply))
            }
        }
    }
    fn cancel(&mut self, id: SubmissionId) -> Result<(), RotationError> {
        if !self.pending.contains_key(&id) {
            return Err(DriverOutputError::UnknownSubmission(id).into());
        }
        let (reply, receive) = mpsc::sync_channel(1);
        self.sender
            .send(Command::Cancel(id, reply))
            .map_err(output_error)?;
        receive.recv().map_err(output_error)??;
        Ok(())
    }
    fn close(&mut self) -> Result<(), RotationError> {
        if !self.pending.is_empty() {
            return Err(DriverOutputError::ExecutionPending.into());
        }
        self.closed = true;
        Ok(())
    }
}
impl Drop for Rotation {
    fn drop(&mut self) {
        for id in self.pending.keys().copied().collect::<Vec<_>>() {
            if let Err(error) = self.cancel(id) {
                eprintln!("Cancel mapping rotation: {error}");
            }
            loop {
                match self.poll(id, Duration::from_millis(16)) {
                    Ok(RotationPoll::Pending) => {}
                    Ok(RotationPoll::Completed(_)) => break,
                    Err(error) => {
                        eprintln!("Drain mapping rotation: {error}");
                        break;
                    }
                }
            }
        }
    }
}
