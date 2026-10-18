use super::Transport;
use razer_ipc::{
    ServiceRequest,
    mapping_control::{Owner, Receipt},
};

pub(super) fn apply(client: &mut impl Transport, owner: &Owner) -> anyhow::Result<()> {
    apply_with(owner, |request| client.request(request))
}

pub(super) fn apply_with(
    owner: &Owner,
    mut request: impl FnMut(ServiceRequest) -> anyhow::Result<serde_json::Value>,
) -> anyhow::Result<()> {
    let mut errors = Vec::new();
    for command in owner.drain() {
        let result = super::guarded(|| -> anyhow::Result<()> {
            let receipt: Receipt =
                serde_json::from_value(request(ServiceRequest::MappingExecution {
                    owner_guid: command.owner_guid().to_owned(),
                    revision: command.revision(),
                    target: command.target(),
                })?)?;
            anyhow::ensure!(
                receipt.confirms(command.owner_guid(), command.revision(), command.target()),
                "Mapping execution receipt does not match owner, revision and target"
            );
            Ok(())
        })
        .map_err(|error| format!("{error:#}"));
        if let Err(error) = &result {
            errors.push(error.clone());
        }
        command.acknowledge(result);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        anyhow::bail!("{}", errors.join("; "))
    }
}
