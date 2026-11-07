//! Earbuds Input Report notifications; these are independent of Feature replies.
use anyhow::{Context as _, Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Target {
    Collection(crate::backend::HidNode),
    WindowsContainer(crate::device_reads::DeviceReadTarget),
}

impl Target {
    pub fn product_id(&self) -> Result<u32> {
        let product_id = match self {
            Self::Collection(node) => {
                ensure!(
                    node.vendor_id == 0x1532,
                    "Earbud notification vendor differs"
                );
                u32::from(node.product_id)
            }
            Self::WindowsContainer(target) => {
                ensure!(
                    target.product_id == target.physical_product_id
                        && target.peer_product_id.is_none()
                        && !target.device_container_id.is_empty(),
                    "Earbud notification target differs"
                );
                target.product_id
            }
        };
        ensure!(
            crate::earbuds_protocol40::supports_auto_pause(product_id),
            "Product does not register earbuds notifications"
        );
        Ok(product_id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Notification {
    Anc { state: crate::earbuds_anc::State },
    Battery { left: u8, right: u8 },
    Connection { status: u8, status_bt: u8 },
}

pub fn notification(bytes: &[u8]) -> Result<Option<Notification>> {
    if bytes.first() != Some(&5) {
        return Ok(None);
    }
    let event = bytes
        .get(1)
        .context("Earbud notification header is truncated")?;
    Ok(match event {
        33 => Some(Notification::Anc { state: crate::earbuds_anc::State::from_raw(
            *bytes.get(2).context("ANC notification omits state")?) }),
        49 => {
            ensure!(bytes.len() >= 4, "Earbud battery notification is truncated");
            Some(Notification::Battery {
                left: bytes[2],
                right: bytes[3],
            })
        }
        56 => {
            let device_type = bytes
                .get(2)
                .context("Audio connection notification omits device type")?;
            if *device_type != 1 {
                return Ok(None);
            }
            ensure!(
                bytes.len() >= 5,
                "Earbud connection notification is truncated"
            );
            Some(Notification::Connection {
                status: bytes[3],
                status_bt: bytes[4],
            })
        }
        _ => None,
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct StartReply {
    pub session_id: String,
    pub started: bool,
}
impl StartReply {
    pub fn started(session_id: String) -> Self {
        Self {
            session_id,
            started: true,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct PollReply {
    pub session_id: String,
    pub events: Vec<Notification>,
    pub terminal: bool,
    pub error: Option<String>,
}
impl PollReply {
    pub fn new(
        session_id: String,
        events: Vec<Notification>,
        terminal: bool,
        error: Option<String>,
    ) -> Self {
        Self {
            session_id,
            events,
            terminal,
            error,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[non_exhaustive]
pub struct StopReply {
    pub session_id: String,
    pub stopped: bool,
}
impl StopReply {
    pub fn stopped(session_id: String) -> Self {
        Self {
            session_id,
            stopped: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anc_status_notifications_retain_unknown_and_reject_missing_bytes() {
        use crate::earbuds_anc::State;
        for (raw,state) in [(0,State::Off),(1,State::Anc),(80,State::Ambient),(99,State::Unknown(99))] {
            assert_eq!(notification(&[5,33,raw]).unwrap(),Some(Notification::Anc{state}));
        }
        assert!(notification(&[5,33]).is_err());
    }

    #[test]
    fn current_reports_keep_raw_battery_and_connection_states() {
        assert_eq!(
            notification(&[5, 49, 0, 255]).unwrap(),
            Some(Notification::Battery {
                left: 0,
                right: 255
            })
        );
        assert_eq!(
            notification(&[5, 56, 1, 2, 7]).unwrap(),
            Some(Notification::Connection {
                status: 2,
                status_bt: 7
            })
        );
        assert_eq!(notification(&[5, 56, 2]).unwrap(), None);
        assert_eq!(notification(&[2, 49]).unwrap(), None);
        assert_eq!(notification(&[5, 200]).unwrap(), None);
    }

    #[test]
    fn truncated_selected_notifications_are_errors() {
        for bytes in [
            &[5][..],
            &[5, 49],
            &[5, 49, 1],
            &[5, 56],
            &[5, 56, 1],
            &[5, 56, 1, 1],
        ] {
            assert!(notification(bytes).is_err(), "{bytes:?}");
        }
    }
}
