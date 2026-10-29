use anyhow::{Context as _, ensure};
use std::{
    os::unix::{
        ffi::OsStrExt,
        fs::{DirBuilderExt, FileTypeExt, MetadataExt, PermissionsExt},
    },
    path::{Path, PathBuf},
};

// uid_t is an unsigned 32-bit integer on the supported Linux and macOS targets.
unsafe extern "C" {
    fn geteuid() -> u32;
}

#[derive(Clone, Debug)]
pub(super) struct Endpoint {
    identity: String,
    user: u32,
    directory: PathBuf,
    path: PathBuf,
}

impl Endpoint {
    pub fn new(installation: &Path) -> anyhow::Result<Self> {
        // SAFETY: geteuid has no pointers or resource ownership.
        let user = unsafe { geteuid() };
        let mut identity = md5::Context::new();
        identity.consume(b"razer-ui-hid-v1\0");
        identity.consume(user.to_be_bytes());
        identity.consume(installation.as_os_str().as_bytes());
        let identity = format!("{:x}", identity.finalize());
        let directory = Path::new("/tmp").join(format!("razer-ui-hid-{user}"));
        let path = directory.join(format!("{identity}.sock"));
        ensure!(
            path.as_os_str().as_bytes().len() < 104,
            "broker Unix socket path exceeds the platform limit"
        );
        Ok(Self {
            identity,
            user,
            directory,
            path,
        })
    }

    pub fn identity(&self) -> &str {
        &self.identity
    }
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn validate_directory(&self) -> anyhow::Result<()> {
        let metadata = std::fs::symlink_metadata(&self.directory)
            .context("broker runtime directory is unavailable")?;
        ensure!(
            metadata.file_type().is_dir()
                && metadata.uid() == self.user
                && metadata.mode() & 0o777 == 0o700,
            "broker runtime directory must be owned by this user with mode 0700"
        );
        Ok(())
    }

    pub fn bind(&self) -> anyhow::Result<tokio::net::UnixListener> {
        match std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&self.directory)
        {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(error).context("broker runtime directory could not be created");
            }
        }
        self.validate_directory()?;
        let listener = tokio::net::UnixListener::bind(&self.path)
            .context("broker Unix socket could not be bound")?;
        let metadata = std::fs::symlink_metadata(&self.path)
            .context("bound broker Unix socket identity is unavailable")?;
        if let Err(error) =
            std::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o600))
        {
            drop(listener);
            return match self.release(metadata.dev(), metadata.ino()) {
                Ok(()) => {
                    Err(error).context("broker Unix socket permissions could not be restricted")
                }
                Err(cleanup) => Err(error).context(format!(
                    "broker Unix socket permissions failed and socket release failed: {cleanup:#}"
                )),
            };
        }
        Ok(listener)
    }

    pub fn release(&self, device: u64, inode: u64) -> anyhow::Result<()> {
        self.validate_directory()?;
        let metadata = std::fs::symlink_metadata(&self.path)
            .context("broker Unix socket identity is unavailable for release")?;
        ensure!(
            metadata.file_type().is_socket()
                && metadata.uid() == self.user
                && metadata.dev() == device
                && metadata.ino() == inode,
            "broker Unix socket owner was replaced; release refused"
        );
        std::fs::remove_file(&self.path).context("broker Unix socket release failed")
    }

    pub fn verify_peer(&self, stream: &tokio::net::UnixStream) -> anyhow::Result<()> {
        self.validate_directory()?;
        ensure!(
            stream.peer_cred()?.uid() == self.user,
            "broker peer belongs to a different user"
        );
        Ok(())
    }
}
