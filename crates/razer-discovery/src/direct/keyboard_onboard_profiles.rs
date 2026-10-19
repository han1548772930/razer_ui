//! Complete current keyboard OBM read over the observed direct HID owner.
use super::keyboard_profile_inventory;
use crate::discovery::ObservedDevice;
pub use razer_ipc::keyboard_onboard_profiles::{CompleteReading, ReadOptions, Target};
use razer_ipc::{
    ServiceClient, ServiceRequest,
    keyboard_onboard_profiles::{Observation, validate_target},
};

pub fn resolve(client: &mut ServiceClient, observed: &ObservedDevice) -> anyhow::Result<Target> {
    let target = keyboard_profile_inventory::resolve(client, observed)?;
    validate_target(&target)?;
    Ok(target)
}

pub fn read(
    client: &mut ServiceClient,
    target: &Target,
    options: &ReadOptions,
) -> anyhow::Result<CompleteReading> {
    read_with(target, options, |request| client.request(request))
}

fn read_with(
    target: &Target,
    options: &ReadOptions,
    mut request: impl FnMut(ServiceRequest) -> anyhow::Result<serde_json::Value>,
) -> anyhow::Result<CompleteReading> {
    validate_target(target)?;
    options.validate()?;
    let reply = request(ServiceRequest::KeyboardOnboardProfilesRead {
        target: target.clone(),
        options: options.clone(),
    })?;
    let observation: Observation = serde_json::from_value(reply)?;
    observation.validate_for(target, options)?;
    Ok(observation.reading)
}

#[cfg(test)]
#[path = "keyboard_onboard_profiles_tests.rs"]
mod tests;
