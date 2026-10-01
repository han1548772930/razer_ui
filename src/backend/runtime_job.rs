//! Parent-owned Windows job: a worker cannot outlive the application process.
use anyhow::Context as _;
use std::{
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    process::Child,
    ptr,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation,
    SetInformationJobObject,
};

pub(super) struct WorkerJob(OwnedHandle);

impl WorkerJob {
    pub(super) fn new() -> anyhow::Result<Self> {
        // Null SECURITY_ATTRIBUTES creates a non-inheritable handle. The worker
        // must not inherit a second handle that would keep this job alive.
        let raw = unsafe { CreateJobObjectW(ptr::null(), ptr::null()) };
        if raw.is_null() {
            return Err(std::io::Error::last_os_error()).context("无法创建设备服务进程作业");
        }
        // SAFETY: CreateJobObjectW returned a new, valid, owned kernel handle.
        let job = Self(unsafe { OwnedHandle::from_raw_handle(raw) });
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if unsafe {
            SetInformationJobObject(
                job.0.as_raw_handle(),
                JobObjectExtendedLimitInformation,
                ptr::from_ref(&limits).cast(),
                size_of_val(&limits) as u32,
            )
        } == 0
        {
            return Err(std::io::Error::last_os_error()).context("无法设置设备服务退出清理规则");
        }
        Ok(job)
    }

    pub(super) fn assign(&self, child: &Child) -> anyhow::Result<()> {
        // The child only reads stdin before its first request. Assign it before
        // starting request threads, so no vendor DLL can run outside this job.
        if unsafe { AssignProcessToJobObject(self.0.as_raw_handle(), child.as_raw_handle()) } == 0 {
            return Err(std::io::Error::last_os_error()).context("无法将设备服务加入退出清理作业");
        }
        Ok(())
    }
}
