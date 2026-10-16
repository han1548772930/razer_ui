//! Independently recovered PID 3595/3596 topology-scanning KS adapter.
//! Public COM interfaces only; target DLLs are never loaded or called.

use super::windows::{CameraComSession, Com};
use anyhow::{Context as _, Result, ensure};
use razer_device::camera_extensions::CameraKsCallOutcome;
use razer_device::camera_focus_family::CameraFamilyFocusTransport;
use razer_device::camera_submission::CameraScope;
use std::{ffi::c_void, ptr::null_mut};
use windows_sys::core::GUID;

const IID_IKS_TOPOLOGY_INFO: GUID = GUID::from_u128(0x720d4ac0_7533_11d0_a5d6_28db04c10000);
const IID_IKS_CONTROL: GUID = GUID::from_u128(0x28f54685_06fd_11d2_b27a_00a0c9223196);

#[repr(C, align(4))]
struct KsNodeHeader([u8; 32]);

/// An exclusive borrow serializes the entire node scan and SET/wait/GET.
/// Interfaces remain on the filter's owning COM thread.
pub(super) struct CameraFamilyKsSession<'a> {
    control: Option<Com>,
    topology: Option<Com>,
    nodes: Option<u32>,
    filter: &'a Com,
    locking: Option<(&'a CameraScope, &'a dyn Fn() -> Result<()>)>,
    native_lock: Option<super::lock::Guard>,
}
impl<'a> CameraFamilyKsSession<'a> {
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
            nodes: None,
            filter,
            locking: None,
            native_lock: None,
        }
    }
}
impl CameraFamilyFocusTransport for CameraFamilyKsSession<'_> {
    fn begin_topology(&mut self) -> Result<u32> {
        ensure!(
            self.control.is_none() && self.topology.is_none(),
            "camera family topology is already open"
        );
        // Base and X hold the same object mutex for the complete node scan.
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
        type GetNumNodes = unsafe extern "system" fn(*mut c_void, *mut u32) -> i32;
        // Both original helpers use +64, not a fixed extension node.
        let count: GetNumNodes = unsafe { topology.method(8) };
        let mut nodes = 0;
        let hr = unsafe { count(topology.raw(), &mut nodes) };
        // Native ignores this HRESULT. Reject a failed count before using its
        // output, and let the shared executor check the bounded node count.
        ensure!(
            hr >= 0,
            "IKsTopologyInfo::GetNumNodes failed: 0x{:08x}",
            hr as u32
        );
        self.nodes = Some(nodes);
        Ok(nodes)
    }
    fn begin_node(&mut self, node_id: u32) -> Result<()> {
        ensure!(
            self.control.is_none(),
            "camera family KS node is already open"
        );
        ensure!(
            self.nodes.is_some_and(|n| node_id < n),
            "camera family KS node is outside the returned topology"
        );
        let topology = self
            .topology
            .as_ref()
            .context("camera family topology is not open")?;
        type CreateNodeInstance =
            unsafe extern "system" fn(*mut c_void, u32, *const GUID, *mut *mut c_void) -> i32;
        let create: CreateNodeInstance = unsafe { topology.method(10) };
        let mut output = null_mut();
        let hr = unsafe { create(topology.raw(), node_id, &IID_IKS_CONTROL, &mut output) };
        self.control =
            Some(unsafe { Com::from_result(output, hr, "IKsTopologyInfo::CreateNodeInstance")? });
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
            .context("camera family KS control is not open")?;
        type KsProperty = unsafe extern "system" fn(
            *mut c_void,
            *mut c_void,
            u32,
            *mut c_void,
            u32,
            *mut u32,
        ) -> i32;
        let property: KsProperty = unsafe { control.method(3) };
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
    }
    fn end_topology(&mut self) {
        self.end_node();
        drop(self.topology.take());
        self.nodes = None;
        drop(self.native_lock.take());
    }
}
impl Drop for CameraFamilyKsSession<'_> {
    fn drop(&mut self) {
        self.end_topology();
    }
}

#[cfg(test)]
#[path = "camera_focus_family_tests.rs"]
mod tests;
