//! PID2657 ANC observations stay separate from the source runtime skip cache.
use crate::earbuds_protocol40::{
    self as protocol, AncLevel, AncRuntimeState, AncSetting, AncWrite, Protocol40Session, Sleeper,
};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub enum State {
    Off,
    Anc,
    Ambient,
    Unknown(u8),
}
impl State {
    pub fn from_raw(value: u8) -> Self {
        match value {
            0 => Self::Off,
            1 => Self::Anc,
            80 => Self::Ambient,
            value => Self::Unknown(value),
        }
    }
    pub fn raw(self) -> u8 {
        match self {
            Self::Off => 0,
            Self::Anc => 1,
            Self::Ambient => 80,
            Self::Unknown(value) => value,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub enum Observation {
    #[default]
    Unobserved,
    Observed {
        state: State,
        level: Option<AncLevel>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct Report {
    pub runtime: AncRuntimeState,
    pub write: AncWrite,
    pub confirmation: Option<AncSetting>,
}

#[derive(Clone, Copy)]
struct Cached {
    runtime: AncRuntimeState,
    comparable: bool,
}

/// One physical notification owner's source skip cache. UI/profile requests
/// borrow it; their cancellation cannot erase a completed hardware write.
pub struct Cache {
    state: std::cell::Cell<Cached>,
}
impl Default for Cache {
    fn default() -> Self {
        Self::from_runtime(AncRuntimeState::default())
    }
}
impl Cache {
    pub fn from_runtime(runtime: AncRuntimeState) -> Self {
        Self {
            state: std::cell::Cell::new(Cached {
                runtime,
                comparable: true,
            }),
        }
    }
    pub fn runtime(&self) -> AncRuntimeState {
        self.state.get().runtime
    }
    pub fn is_comparable(&self) -> bool {
        self.state.get().comparable
    }

    pub fn execute<S: Sleeper>(
        &self,
        session: &mut Protocol40Session<'_, S>,
        product_id: u32,
        setting: AncSetting,
        validate_request: &dyn Fn() -> Result<()>,
        validate_connection: &dyn Fn() -> Result<()>,
    ) -> Result<Report> {
        ensure!(
            protocol::supports_anc(product_id),
            "Product does not register the PID2657 ANC chain"
        );
        validate_request()?;
        validate_connection()?;
        let previous = self.state.get();
        if previous.comparable && previous.runtime.0 == setting {
            return Ok(Report {
                runtime: previous.runtime,
                write: AncWrite::Skipped,
                confirmation: None,
            });
        }
        let attempted = |_: &[u8]| {
            // Called only for the native owner's terminal send-attempt fact.
            self.state.set(Cached {
                comparable: false,
                ..self.state.get()
            });
        };
        let result = session.send_command_observing(
            protocol::SET_ANC,
            &setting.set_params(),
            validate_request,
            &attempted,
        );
        let acknowledged = result.is_ok()
            || result
                .as_ref()
                .err()
                .and_then(crate::feature_exchange::CompletedFeatureExchange::from_error)
                .is_some_and(|reply| reply.command() == [3, 24, 6]);
        let connection_status = validate_connection();
        if acknowledged && connection_status.is_ok() {
            self.state.set(Cached {
                runtime: AncRuntimeState(setting),
                comparable: true,
            });
        }
        // The physical cache may consume the ACK, while the canceled profile
        // request must still fail and cannot publish a new draft/measurement.
        if let Err(validation) = validate_request() {
            return Err(match result {
                Err(error) => error.context(format!("ANC request was canceled: {validation:#}")),
                Ok(_) => validation,
            });
        }
        connection_status?;
        let (write, confirmation) = match result {
            Ok(_) => (AncWrite::Acknowledged, Some(setting)),
            Err(error) => (AncWrite::Failed(format!("{error:#}")), None),
        };
        Ok(Report {
            runtime: self.runtime(),
            write,
            confirmation,
        })
    }
}

#[cfg(test)]
#[path = "earbuds_anc_tests.rs"]
mod tests;
