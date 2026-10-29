use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[cfg(windows)]
#[path = "windows.rs"]
mod platform;
#[cfg(unix)]
#[path = "unix.rs"]
mod platform;

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct EndpointId(String);

impl TryFrom<String> for EndpointId {
    type Error = anyhow::Error;

    fn try_from(value: String) -> anyhow::Result<Self> {
        ensure!(
            value.len() == 32
                && value
                    .bytes()
                    .all(|byte| { byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte) }),
            "broker endpoint identity must contain 32 lowercase hexadecimal digits"
        );
        Ok(Self(value))
    }
}

impl From<EndpointId> for String {
    fn from(value: EndpointId) -> Self {
        value.0
    }
}

impl EndpointId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The digest partitions installation paths; OS credentials authenticate peers.
#[derive(Clone, Debug)]
pub struct BrokerEndpoint {
    id: EndpointId,
    installation: PathBuf,
    platform: platform::Endpoint,
}

impl BrokerEndpoint {
    pub fn for_installation(executable: &Path) -> anyhow::Result<Self> {
        let executable = executable
            .canonicalize()
            .context("broker executable path could not be resolved")?;
        let installation = executable
            .parent()
            .context("broker executable has no installation directory")?
            .to_path_buf();
        let platform = platform::Endpoint::new(&installation)?;
        let id = EndpointId::try_from(platform.identity().to_owned())?;
        Ok(Self {
            id,
            installation,
            platform,
        })
    }

    pub fn id(&self) -> &EndpointId {
        &self.id
    }

    pub fn installation(&self) -> &Path {
        &self.installation
    }

    #[cfg(windows)]
    pub fn as_windows_pipe(&self) -> &std::ffi::OsStr {
        self.platform.name()
    }

    #[cfg(windows)]
    pub fn bind_windows_pipe(
        &self,
        first_instance: bool,
    ) -> anyhow::Result<tokio::net::windows::named_pipe::NamedPipeServer> {
        self.platform.bind(first_instance)
    }

    #[cfg(windows)]
    pub fn verify_windows_client(
        &self,
        pipe: &tokio::net::windows::named_pipe::NamedPipeServer,
    ) -> anyhow::Result<()> {
        self.platform.verify_client(pipe)
    }

    #[cfg(windows)]
    pub(super) fn verify_windows_server(
        &self,
        pipe: &tokio::net::windows::named_pipe::NamedPipeClient,
    ) -> anyhow::Result<()> {
        self.platform.verify_server(pipe)
    }

    #[cfg(unix)]
    pub fn as_unix_path(&self) -> &Path {
        self.platform.path()
    }

    #[cfg(unix)]
    pub fn bind_unix_listener(&self) -> anyhow::Result<tokio::net::UnixListener> {
        self.platform.bind()
    }

    /// The listener owner calls this after its listener and accepted streams are
    /// released. File identity prevents an old owner unlinking a successor.
    #[cfg(unix)]
    pub fn release_unix_listener(&self, device: u64, inode: u64) -> anyhow::Result<()> {
        self.platform.release(device, inode)
    }

    #[cfg(unix)]
    pub fn verify_unix_peer(&self, stream: &tokio::net::UnixStream) -> anyhow::Result<()> {
        self.platform.verify_peer(stream)
    }
}
