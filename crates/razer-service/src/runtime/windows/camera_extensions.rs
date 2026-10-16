//! Direct private KS camera requests from the audited Pro native vtable.
//! QI/CreateNodeInstance/KsProperty use public Windows COM interfaces. The
//! vendor native module is never loaded or called.

use super::windows::{CameraComSession, Com};
use anyhow::{Context as _, Result, ensure};
use razer_device::camera_extensions::{
    CAMERA_EXTENSION_NODE, CameraExtensionTransport, CameraKsCallOutcome,
};
use razer_device::camera_submission::CameraScope;
use std::{ffi::c_void, ptr::null_mut};
use windows_sys::core::GUID;

const IID_IKS_TOPOLOGY_INFO: GUID = GUID::from_u128(0x720d4ac0_7533_11d0_a5d6_28db04c10000);
const IID_IKS_CONTROL: GUID = GUID::from_u128(0x28f54685_06fd_11d2_b27a_00a0c9223196);

#[repr(C, align(4))]
struct KsNodeHeader([u8; 32]);

/// The exclusive session borrow serializes SET/wait/GET and compound chains.
/// Drop releases IKsControl before IKsTopologyInfo before the retained filter.
pub(super) struct CameraKsSession<'a> {
    control: Option<Com>,
    topology: Option<Com>,
    filter: &'a Com,
    locking: Option<(&'a CameraScope, &'a dyn Fn() -> Result<()>)>,
    native_lock: Option<super::lock::Guard>,
}

impl<'a> CameraKsSession<'a> {
    pub(super) fn new(
        session: &'a mut CameraComSession,
        scope: &'a CameraScope,
        validate: &'a dyn Fn() -> Result<()>,
    ) -> Self {
        let mut transport = Self::from_filter(&session.filter);
        transport.locking = Some((scope, validate));
        transport
    }

    fn from_filter(filter: &'a Com) -> Self {
        Self {
            control: None,
            topology: None,
            filter,
            locking: None,
            native_lock: None,
        }
    }
}

impl CameraExtensionTransport for CameraKsSession<'_> {
    fn begin_node(&mut self, node_id: u32) -> Result<()> {
        ensure!(
            self.control.is_none() && self.topology.is_none(),
            "camera KS node is already open"
        );
        ensure!(
            node_id == CAMERA_EXTENSION_NODE,
            "camera KS node is outside the native audit"
        );
        // Current native IspCommand owns the object's +136 mutex across
        // SET/wait/GET. Release between the native calls of a compound request.
        if let Some((scope, validate)) = self.locking {
            self.native_lock = Some(super::lock::acquire(
                &super::lock::native_name(scope),
                validate,
            )?);
        }
        self.topology = Some(
            self.filter
                .query(&IID_IKS_TOPOLOGY_INFO, "IKsTopologyInfo")?,
        );
        let topology = self.topology.as_ref().unwrap();
        type CreateNodeInstance =
            unsafe extern "system" fn(*mut c_void, u32, *const GUID, *mut *mut c_void) -> i32;
        // Actual CRzDeviceAccess_0E0A helper at RVA 0x52f30 uses +80.
        let create: CreateNodeInstance = unsafe { topology.method(10) };
        let mut output = null_mut();
        let hr = unsafe { create(topology.raw(), node_id, &IID_IKS_CONTROL, &mut output) };
        self.control = Some(unsafe {
            Com::from_result(output, hr, "IKsTopologyInfo::CreateNodeInstance(3)")?
        });
        Ok(())
    }

    fn ks_property(
        &mut self,
        header: &[u8; 32],
        data: &mut [u8; 8],
    ) -> Result<CameraKsCallOutcome> {
        let control = self
            .control
            .as_ref()
            .context("camera KS control is not open")?;
        type KsProperty = unsafe extern "system" fn(
            *mut c_void,
            *mut c_void,
            u32,
            *mut c_void,
            u32,
            *mut u32,
        ) -> i32;
        let property: KsProperty = unsafe { control.method(3) }; // +24
        let mut header = KsNodeHeader(*header);
        let mut bytes_returned = 0;
        let hresult = unsafe {
            property(
                control.raw(),
                header.0.as_mut_ptr().cast(),
                32,
                data.as_mut_ptr().cast(),
                8,
                &mut bytes_returned,
            )
        };
        Ok(CameraKsCallOutcome {
            hresult,
            bytes_returned,
        })
    }

    fn end_node(&mut self) {
        drop(self.control.take());
        drop(self.topology.take());
        drop(self.native_lock.take());
    }
}

impl Drop for CameraKsSession<'_> {
    fn drop(&mut self) {
        self.end_node();
    }
}

#[cfg(test)]
#[path = "camera_extensions_tests.rs"]
mod tests;
