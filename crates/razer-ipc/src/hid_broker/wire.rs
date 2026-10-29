use super::EndpointId;
use crate::{RequestEnvelope, ResponseEnvelope};
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const PROTOCOL_VERSION: u32 = 3;

macro_rules! random_identity {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Deserialize, Serialize)]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(uuid::Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(uuid::Uuid::new_v4())
            }

            pub fn as_uuid(self) -> uuid::Uuid {
                self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl TryFrom<String> for $name {
            type Error = anyhow::Error;

            fn try_from(value: String) -> anyhow::Result<Self> {
                ensure!(
                    value.len() == 32
                        && value
                            .bytes()
                            .all(|byte| { byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte) }),
                    "broker identity must be a complete lowercase UUID"
                );
                let id = uuid::Uuid::parse_str(&value)?;
                ensure!(
                    id.get_version() == Some(uuid::Version::Random)
                        && id.get_variant() == uuid::Variant::RFC4122,
                    "broker identity must be a random UUID v4"
                );
                Ok(Self(id))
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0.simple().to_string()
            }
        }
    };
}

random_identity!(BrokerInstanceId);
random_identity!(ClientSessionId);
random_identity!(BrokerRequestId);
random_identity!(HandshakeNonce);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ClientHello {
    pub version: u32,
    pub endpoint: EndpointId,
    pub nonce: HandshakeNonce,
    pub expected_instance: Option<BrokerInstanceId>,
}

impl ClientHello {
    pub fn new(endpoint: EndpointId, expected_instance: Option<BrokerInstanceId>) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            endpoint,
            nonce: HandshakeNonce::new(),
            expected_instance,
        }
    }

    pub fn validate(
        &self,
        endpoint: &EndpointId,
        instance: BrokerInstanceId,
    ) -> anyhow::Result<()> {
        ensure!(
            self.version == PROTOCOL_VERSION,
            "broker protocol version mismatch"
        );
        ensure!(
            &self.endpoint == endpoint,
            "broker endpoint identity mismatch"
        );
        ensure!(
            self.expected_instance
                .is_none_or(|expected| expected == instance),
            "broker instance was replaced"
        );
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct SessionIdentity {
    pub endpoint: EndpointId,
    pub instance: BrokerInstanceId,
    pub session: ClientSessionId,
}

impl SessionIdentity {
    pub fn new(endpoint: EndpointId, instance: BrokerInstanceId, session: ClientSessionId) -> Self {
        Self {
            endpoint,
            instance,
            session,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
#[non_exhaustive]
pub struct ServerHello {
    pub version: u32,
    pub nonce: HandshakeNonce,
    pub identity: SessionIdentity,
}

impl ServerHello {
    pub fn new(hello: &ClientHello, instance: BrokerInstanceId) -> anyhow::Result<Self> {
        hello.validate(&hello.endpoint, instance)?;
        Ok(Self {
            version: PROTOCOL_VERSION,
            nonce: hello.nonce,
            identity: SessionIdentity {
                endpoint: hello.endpoint.clone(),
                instance,
                session: ClientSessionId::new(),
            },
        })
    }

    pub fn validate(&self, hello: &ClientHello) -> anyhow::Result<()> {
        hello.validate(&self.identity.endpoint, self.identity.instance)?;
        ensure!(
            self.version == PROTOCOL_VERSION,
            "broker protocol version mismatch"
        );
        ensure!(self.nonce == hello.nonce, "broker handshake nonce mismatch");
        Ok(())
    }
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClientFrame {
    Request {
        identity: SessionIdentity,
        request_id: BrokerRequestId,
        envelope: RequestEnvelope,
    },
    Cancel {
        identity: SessionIdentity,
        request_id: BrokerRequestId,
    },
    CloseSession {
        identity: SessionIdentity,
        request_id: BrokerRequestId,
    },
}

impl ClientFrame {
    pub fn identity(&self) -> &SessionIdentity {
        match self {
            Self::Request { identity, .. }
            | Self::Cancel { identity, .. }
            | Self::CloseSession { identity, .. } => identity,
        }
    }

    pub fn request_id(&self) -> BrokerRequestId {
        match self {
            Self::Request { request_id, .. }
            | Self::Cancel { request_id, .. }
            | Self::CloseSession { request_id, .. } => *request_id,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "result", rename_all = "snake_case", deny_unknown_fields)]
#[non_exhaustive]
pub enum RestorationOutcome {
    Confirmed {},
    Failed { error: String },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "resource_state", rename_all = "snake_case", deny_unknown_fields)]
#[non_exhaustive]
pub enum CloseSessionReceipt {
    Pending { error: String },
    Released { outcome: RestorationOutcome },
}

impl CloseSessionReceipt {
    pub fn confirmed() -> Self {
        Self::Released {
            outcome: RestorationOutcome::Confirmed {},
        }
    }

    pub fn pending(error: String) -> Self {
        Self::Pending { error }
    }

    pub fn restoration_failed(error: String) -> Self {
        Self::Released {
            outcome: RestorationOutcome::Failed { error },
        }
    }

    pub fn resources_released(&self) -> bool {
        matches!(self, Self::Released { .. })
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        match self {
            Self::Pending { error } => anyhow::bail!(
                "broker session resources remain owned; cleanup is unconfirmed: {error}"
            ),
            Self::Released {
                outcome: RestorationOutcome::Confirmed {},
            } => Ok(()),
            Self::Released {
                outcome: RestorationOutcome::Failed { error },
            } => anyhow::bail!(
                "broker session resources were released with restoration failure: {error}"
            ),
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ServerFrame {
    Response {
        identity: SessionIdentity,
        request_id: BrokerRequestId,
        envelope: ResponseEnvelope,
    },
    SessionClosed {
        identity: SessionIdentity,
        request_id: BrokerRequestId,
        receipt: CloseSessionReceipt,
    },
}

impl ServerFrame {
    pub fn identity(&self) -> &SessionIdentity {
        match self {
            Self::Response { identity, .. } | Self::SessionClosed { identity, .. } => identity,
        }
    }

    pub fn request_id(&self) -> BrokerRequestId {
        match self {
            Self::Response { request_id, .. } | Self::SessionClosed { request_id, .. } => {
                *request_id
            }
        }
    }

    pub fn validate(
        &self,
        identity: &SessionIdentity,
        request_id: BrokerRequestId,
    ) -> anyhow::Result<()> {
        ensure!(
            self.identity() == identity,
            "broker response session identity mismatch"
        );
        ensure!(
            self.request_id() == request_id,
            "broker response request UUID mismatch"
        );
        Ok(())
    }
}

/// Each message is a four-byte big-endian length followed by one JSON value.
/// A missing or partial frame is an error, including EOF before the prefix.
pub async fn read_frame<R, T>(reader: &mut R) -> anyhow::Result<T>
where
    R: AsyncRead + Unpin + ?Sized,
    T: DeserializeOwned,
{
    FrameReader::new(reader).read().await
}

/// Read progress belongs to the connection so a request deadline cannot discard
/// an already consumed prefix or body before the cleanup receipt is drained.
pub struct FrameReader<R> {
    reader: R,
    prefix: [u8; 4],
    prefix_read: usize,
    body: Vec<u8>,
    body_read: usize,
}

impl<R: AsyncRead + Unpin> FrameReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            prefix: [0; 4],
            prefix_read: 0,
            body: Vec::new(),
            body_read: 0,
        }
    }

    pub async fn read<T: DeserializeOwned>(&mut self) -> anyhow::Result<T> {
        while self.prefix_read < self.prefix.len() {
            let count = self
                .reader
                .read(&mut self.prefix[self.prefix_read..])
                .await
                .context("broker frame prefix read failed")?;
            ensure!(
                count > 0,
                "broker frame prefix was not received completely before EOF"
            );
            self.prefix_read += count;
        }
        if self.body.is_empty() {
            let length = u32::from_be_bytes(self.prefix) as usize;
            ensure!(
                length > 0 && length < crate::MAX_FRAME_BYTES,
                "broker frame length is out of bounds"
            );
            self.body.resize(length, 0);
        }
        while self.body_read < self.body.len() {
            let count = self
                .reader
                .read(&mut self.body[self.body_read..])
                .await
                .context("broker frame body read failed")?;
            ensure!(count > 0, "broker frame body was truncated before EOF");
            self.body_read += count;
        }
        let response =
            serde_json::from_slice(&self.body).context("broker frame payload is invalid");
        self.prefix_read = 0;
        self.body_read = 0;
        self.body.clear();
        response
    }
}

pub async fn write_frame<W, T>(writer: &mut W, value: &T) -> anyhow::Result<()>
where
    W: AsyncWrite + Unpin + ?Sized,
    T: Serialize + ?Sized,
{
    FrameWriter::new(writer).write(value).await
}

/// A control frame finishes any interrupted preceding frame before being sent.
pub struct FrameWriter<W> {
    writer: W,
    pending: Vec<u8>,
    written: usize,
}

impl<W: AsyncWrite + Unpin> FrameWriter<W> {
    pub fn new(writer: W) -> Self {
        Self {
            writer,
            pending: Vec::new(),
            written: 0,
        }
    }

    pub async fn write<T: Serialize + ?Sized>(&mut self, value: &T) -> anyhow::Result<()> {
        let bytes = serde_json::to_vec(value).context("broker frame could not be encoded")?;
        ensure!(
            !bytes.is_empty() && bytes.len() < crate::MAX_FRAME_BYTES,
            "broker frame length is out of bounds"
        );
        self.finish().await?;
        self.pending
            .extend_from_slice(&(bytes.len() as u32).to_be_bytes());
        self.pending.extend_from_slice(&bytes);
        self.finish().await
    }

    pub async fn flush_pending(&mut self) -> anyhow::Result<()> {
        self.finish().await
    }

    async fn finish(&mut self) -> anyhow::Result<()> {
        while self.written < self.pending.len() {
            let count = self
                .writer
                .write(&self.pending[self.written..])
                .await
                .context("broker frame write failed")?;
            ensure!(
                count > 0,
                "broker frame write returned zero before completion"
            );
            self.written += count;
        }
        self.writer
            .flush()
            .await
            .context("broker frame flush failed")?;
        self.pending.clear();
        self.written = 0;
        Ok(())
    }
}
