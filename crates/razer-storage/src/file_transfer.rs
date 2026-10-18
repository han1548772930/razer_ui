//! Bounded local file transfers with request-owned cancellation and publication.
use crate::TemporaryDocument;
use std::{
    io::Read as _,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicU8, AtomicUsize, Ordering},
    },
};

static ACTIVE_IO: AtomicUsize = AtomicUsize::new(0);
const OPEN: u8 = 0;
const CANCELLED: u8 = 1;
const PUBLISHING: u8 = 2;
const FINISHED: u8 = 3;

/// Accepted operations, including work queued for an executor and final cleanup.
pub fn active_io() -> usize {
    ACTIVE_IO.load(Ordering::Acquire)
}

#[derive(Clone, Default)]
pub struct Transfer {
    state: Arc<AtomicU8>,
}

#[derive(Clone, Copy)]
pub enum Publication {
    CreateNew,
    Replace,
}

/// Acquire this permit before scheduling background work. Dropping a GUI task
/// cannot make queued I/O invisible to shutdown while its worker still owns it.
pub struct Operation {
    transfer: Transfer,
}

impl Transfer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        // A publication which has already started finishes in the background.
        // Cancellation never blocks the GUI on a disk operation.
        let _ = self
            .state
            .compare_exchange(OPEN, CANCELLED, Ordering::AcqRel, Ordering::Acquire);
    }

    pub fn is_same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.state, &other.state)
    }

    pub fn accept(&self) -> Result<Operation, String> {
        self.check()?;
        ACTIVE_IO.fetch_add(1, Ordering::AcqRel);
        Ok(Operation {
            transfer: self.clone(),
        })
    }

    pub fn check(&self) -> Result<(), String> {
        match self.state.load(Ordering::Acquire) {
            OPEN => Ok(()),
            CANCELLED => Err("文件传输任务已取消。".into()),
            _ => Err("文件传输任务已提交。".into()),
        }
    }

    fn with_publication<T>(&self, commit: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        self.state
            .compare_exchange(OPEN, PUBLISHING, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|state| {
                if state == CANCELLED {
                    "文件传输任务已取消。".to_owned()
                } else {
                    "文件传输任务已提交。".to_owned()
                }
            })?;
        let result = commit();
        self.state.store(FINISHED, Ordering::Release);
        result
    }
}

impl Operation {
    pub fn check(&self) -> Result<(), String> {
        self.transfer.check()
    }

    pub fn read(&self, path: &Path, maximum: usize) -> Result<Vec<u8>, String> {
        self.check()?;
        let mut file = std::fs::File::open(path).map_err(|error| error.to_string())?;
        let metadata = file.metadata().map_err(|error| error.to_string())?;
        if !metadata.is_file() {
            return Err("请选择一个普通文件。".into());
        }
        if metadata.len() > maximum as u64 {
            return Err(format!("文件超出 {maximum} 字节限制。"));
        }
        let mut bytes = Vec::with_capacity((metadata.len() as usize).min(maximum));
        let mut chunk = [0; 64 * 1024];
        loop {
            self.check()?;
            // The metadata may change while the file is read. Read one extra
            // byte to detect growth without allocating an unbounded buffer.
            let remaining = maximum.saturating_sub(bytes.len()).saturating_add(1);
            let amount = chunk.len().min(remaining);
            let count = file
                .read(&mut chunk[..amount])
                .map_err(|error| error.to_string())?;
            if count == 0 {
                break;
            }
            bytes.extend_from_slice(&chunk[..count]);
            if bytes.len() > maximum {
                return Err(format!("文件超出 {maximum} 字节限制。"));
            }
        }
        self.check()?;
        Ok(bytes)
    }

    pub fn write(
        &self,
        path: &Path,
        bytes: &[u8],
        maximum: usize,
        publication: Publication,
    ) -> Result<(), String> {
        self.check()?;
        if bytes.len() > maximum {
            return Err(format!("文件超出 {maximum} 字节导出限制。"));
        }
        let mut staged = TemporaryDocument::create(path).map_err(|error| error.to_string())?;
        staged.write(bytes).map_err(|error| error.to_string())?;
        self.publish(staged, path, publication)
    }

    fn publish(
        &self,
        staged: TemporaryDocument,
        path: &Path,
        publication: Publication,
    ) -> Result<(), String> {
        self.transfer.with_publication(|| {
            match publication {
                Publication::Replace => staged.install(path),
                Publication::CreateNew => staged.install_new(path),
            }
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    "文件已存在，请选择新的文件名。".into()
                } else {
                    error.to_string()
                }
            })
        })
    }
}

impl Drop for Operation {
    fn drop(&mut self) {
        ACTIVE_IO.fetch_sub(1, Ordering::AcqRel);
    }
}

#[cfg(test)]
mod tests;
