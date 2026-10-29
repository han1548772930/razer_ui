//! Same-user, same-installation transport for the shared headless HID owner.
mod bootstrap;
mod client;
mod endpoint;
mod wire;

pub use bootstrap::{BrokerBootstrap, BrokerExit, connect_or_start};
#[cfg(test)]
pub(crate) use client::simulated_client;
pub use client::{BrokerClient, CanceledRequest};
pub use endpoint::{BrokerEndpoint, EndpointId};
pub use wire::{
    BrokerInstanceId, BrokerRequestId, ClientFrame, ClientHello, ClientSessionId,
    CloseSessionReceipt, FrameReader, FrameWriter, HandshakeNonce, PROTOCOL_VERSION,
    RestorationOutcome, ServerFrame, ServerHello, SessionIdentity, read_frame, write_frame,
};

#[cfg(test)]
mod tests;
