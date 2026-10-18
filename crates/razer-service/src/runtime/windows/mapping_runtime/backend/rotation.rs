//! Source rotation operations retain one physical driver registration and do
//! not publish mapping tables or reset the sensor on registration cleanup.
use super::*;
use razer_device::mapping_native_rotation::{
    RotationDelivery, RotationError, RotationExecution, RotationRequest, RotationTransport,
};
use razer_ipc::mouse_rotation::{NativeProgress, NativeRegistration, NativeTarget, NativeUpdate};
use std::time::Duration;

pub(super) struct Registration {
    pub(super) scope: Uuid,
    pub(super) guid: Uuid,
    target: NativeTarget,
    physical: Value,
    transport: driver::Rotation,
    operation: Option<RotationExecution>,
    pub(super) failure: Option<String>,
}

fn observe(target: &NativeTarget) -> Result<Value> {
    target.validate()?;
    let (product, physical) =
        super::super::super::native_target(target.container_id(), Some(target.product_id()))?;
    ensure!(
        product == target.product_id(),
        "Rotation physical product changed"
    );
    Ok(physical)
}

impl Registration {
    fn validate(&self, scope: Uuid) -> Result<()> {
        ensure!(self.scope == scope, "Rotation registration scope changed");
        if let Some(error) = &self.failure {
            anyhow::bail!("{error}");
        }
        ensure!(
            observe(&self.target)? == self.physical,
            "Rotation physical device changed"
        );
        Ok(())
    }
    fn close(&mut self) -> Result<()> {
        if let Some(execution) = &mut self.operation {
            if execution.pending() {
                execution.cancel(&mut self.transport);
            }
            while execution.pending() {
                execution.poll(&mut self.transport, Duration::from_millis(16), &|| Ok(()));
            }
        }
        self.transport.close()?;
        let errors = self
            .operation
            .as_ref()
            .map(|execution| &execution.primary.errors);
        if let Some(errors) = errors {
            ensure!(errors.is_empty(), "Rotation cleanup: {errors:?}");
        }
        Ok(())
    }
}

impl<E: device::Executor> Core<E> {
    pub(super) fn start_rotation(
        &mut self,
        scope: Uuid,
        owner: String,
        target: NativeTarget,
    ) -> Result<Value> {
        let guid = Uuid::parse_str(&owner)?;
        ensure!(!guid.is_nil(), "Rotation owner is nil");
        ensure!(
            !self.rotations.contains_key(&owner),
            "Rotation owner already registered"
        );
        let physical = observe(&target)?;
        let route =
            self.connections
                .rotation(scope, guid, target.product_id(), target.container_id())?;
        let unchanged = observe(&target).and_then(|current| {
            ensure!(
                current == physical,
                "Rotation physical device changed during connection"
            );
            Ok(())
        });
        if let Err(error) = unchanged {
            let cleanup = self.connections.release(&Lease::Rotation { scope, guid });
            anyhow::bail!("{error:#}; rotation connection cleanup: {cleanup:?}");
        }
        self.rotations.insert(
            owner.clone(),
            Registration {
                scope,
                guid,
                target,
                physical,
                transport: route.rotation(),
                operation: None,
                failure: None,
            },
        );
        Ok(serde_json::to_value(NativeRegistration::new(owner, true))?)
    }
    pub(super) fn request_rotation(
        &mut self,
        scope: Uuid,
        owner: &str,
        request: RotationRequest,
    ) -> Result<Value> {
        let row = self
            .rotations
            .get_mut(owner)
            .context("Rotation owner is not registered")?;
        row.validate(scope)?;
        ensure!(
            row.operation.is_none(),
            "Rotation completion must be consumed before another request"
        );
        row.operation = Some(RotationExecution::start(
            &mut row.transport,
            request,
            &|| Ok(()),
        )?);
        Ok(serde_json::to_value(NativeUpdate::new(
            owner.into(),
            NativeProgress::Pending,
        ))?)
    }
    pub(super) fn poll_rotation(&mut self, scope: Uuid, owner: &str) -> Result<Value> {
        let row = self
            .rotations
            .get_mut(owner)
            .context("Rotation owner is not registered")?;
        ensure!(row.scope == scope, "Rotation registration scope changed");
        let valid = row.validate(scope);
        let execution = row
            .operation
            .as_mut()
            .context("Rotation has no pending operation")?;
        execution.poll(&mut row.transport, Duration::ZERO, &|| {
            if valid.is_ok() {
                Ok(())
            } else {
                Err(RotationError::SessionInvalid)
            }
        });
        if !execution.finished() {
            return Ok(serde_json::to_value(NativeUpdate::new(
                owner.into(),
                NativeProgress::Pending,
            ))?);
        }
        let execution = row.operation.take().unwrap();
        valid?;
        let mut errors: Vec<_> = execution
            .primary
            .errors
            .iter()
            .map(ToString::to_string)
            .collect();
        if let Some(error) = execution.stopped {
            errors.push(error.to_string());
        }
        let reply = match execution.primary.delivery {
            RotationDelivery::Completed(reply) => Some(reply),
            RotationDelivery::Failed(error) => {
                errors.push(error.to_string());
                None
            }
            RotationDelivery::Pending(_) => unreachable!(),
        };
        // The operation was consumed above. A real device failure is terminal
        // for this request, while the same registered device may be retried.
        let progress = if errors.is_empty() {
            NativeProgress::Completed(reply.expect("completed rotation without errors"))
        } else {
            NativeProgress::Failed {
                reason: errors.join("; "),
            }
        };
        Ok(serde_json::to_value(NativeUpdate::new(
            owner.into(),
            progress,
        ))?)
    }
    pub(super) fn stop_rotation(&mut self, scope: Uuid, owner: &str) -> Result<Value> {
        let Some(row) = self.rotations.get_mut(owner) else {
            return Ok(serde_json::to_value(NativeRegistration::new(
                owner.into(),
                false,
            ))?);
        };
        ensure!(row.scope == scope, "Rotation registration scope changed");
        let guid = row.guid;
        let closed = row.close();
        // A failed operation does not skip independent lease cleanup. close
        // has already drained kernel work before releasing the physical owner.
        let released = self.connections.release(&Lease::Rotation { scope, guid });
        if released.is_ok() {
            self.rotations.remove(owner);
        }
        match (closed, released) {
            (Ok(()), Ok(())) => Ok(serde_json::to_value(NativeRegistration::new(
                owner.into(),
                false,
            ))?),
            (closed, released) => {
                anyhow::bail!("Rotation cleanup: {closed:?}; lease release: {released:?}")
            }
        }
    }
}
