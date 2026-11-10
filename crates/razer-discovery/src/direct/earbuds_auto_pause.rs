use super::*;
use razer_device::earbuds_auto_pause::{self, Operation, Report};

pub fn event_target(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
) -> anyhow::Result<razer_device::earbuds_events::Target> {
    let cap = earbuds_auto_pause::transport(observed.product_id())?;
    ensure!(
        observed.physical_product_id() == observed.product_id()
            && observed.peer_product_id().is_none(),
        "Earbud event owner differs from its direct dongle"
    );
    if let Some(node) = observed.hid_node() {
        return Ok(razer_device::earbuds_events::Target::Collection(
            node.clone(),
        ));
    }
    #[cfg(windows)]
    {
        Ok(razer_device::earbuds_events::Target::WindowsContainer(
            windows::resolve(client, observed, &cap)?,
        ))
    }
    #[cfg(not(windows))]
    {
        let _ = (client, cap);
        anyhow::bail!("Earbud events require an observed collection");
    }
}

pub fn request(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
    operation: Operation,
) -> anyhow::Result<Report> {
    let cap = earbuds_auto_pause::transport(observed.product_id())?;
    ensure!(
        observed.physical_product_id() == observed.product_id()
            && observed.peer_product_id().is_none()
            && observed.transport() != Some(crate::discovery::ObservedTransport::Ble),
        "Earbud auto-pause requires its current USB dongle"
    );
    let request = if let Some(node) = observed.hid_node() {
        ensure!(
            node.vendor_id == cap.vendor_id
                && u32::from(node.product_id) == cap.product_id
                && node.interface_number == i32::from(cap.claim_interface),
            "Earbud collection differs"
        );
        source_reports_match(client, node, cap.report_bytes)?;
        ServiceRequest::HidNodeEarbudsAutoPause {
            node: node.clone(),
            product_id: cap.product_id,
            auto_pause_operation: operation,
        }
    } else {
        #[cfg(windows)]
        {
            ServiceRequest::DeviceEarbudsAutoPause {
                target: windows::resolve(client, observed, &cap)?,
                auto_pause_operation: operation,
            }
        }
        #[cfg(not(windows))]
        {
            anyhow::bail!("Earbud observation has no portable HID collection");
        }
    };
    let reply: Report = match client.request(request) {
        Ok(data) => serde_json::from_value(data)?,
        Err(error) => match equalizer_failure(&error, observed.product_id(), operation)? {
            Some(report) => report,
            None => return Err(error),
        },
    };
    ensure!(
        reply.product_id == observed.product_id() && reply.operation == operation,
        "Earbud result belongs to another task"
    );
    Ok(reply)
}

fn equalizer_failure(
    error: &anyhow::Error,
    product_id: u32,
    operation: Operation,
) -> anyhow::Result<Option<Report>> {
    let Operation::Equalizer { setting } = operation else {
        return Ok(None);
    };
    let mut report = if let Some(razer_ipc::FailureDiagnostics::EarbudsEqualizer(failure)) =
        razer_ipc::FailureDiagnostics::from_error(error)
    {
        Report::from_equalizer(failure.product_id, failure.submission)?
    } else if let Some(canceled) =
        error.downcast_ref::<razer_ipc::request_cancellation::CanceledResponse>()
    {
        serde_json::from_value::<Report>(canceled.data.clone())?
    } else {
        return Ok(None);
    };
    ensure!(
        report.product_id == product_id && report.operation == operation,
        "Canceled earbud EQ diagnostics belong to another request"
    );
    let submission = report
        .equalizer
        .as_ref()
        .context("EQ failure omitted its command confirmations")?;
    submission.validate()?;
    ensure!(
        submission.setting == setting,
        "EQ failure belongs to another setting"
    );
    ensure!(
        report.connected.is_none()
            && report.left_battery.is_none()
            && report.right_battery.is_none(),
        "EQ failure cannot supply a radio observation"
    );
    ensure!(
        report.observation == razer_device::earbuds_protocol40::AutoPauseObservation::Unobserved,
        "EQ failure cannot supply an auto-pause observation"
    );
    report.write = razer_device::earbuds_protocol40::WriteOutcome::Failed(format!("{error:#}"));
    Ok(Some(report))
}

#[cfg(test)]
mod tests {
    use super::*;
    use razer_device::earbuds_equalizer::{Failure, Mode, Preset, Setting, Submission};
    fn submission() -> razer_device::earbuds_equalizer::Report {
        razer_device::earbuds_equalizer::Report {
            setting: Setting {
                mode: Mode::Standard,
                preset: Preset::Music,
                bands: [1; 10],
            },
            preset: Submission::Confirmed,
            bands: Submission::Cancelled {
                message: "scope cancelled".into(),
                acknowledged: false,
            },
        }
    }
    #[test]
    fn earbuds_equalizer_typed_failure_preserves_real_ack_and_rejects_other_request() {
        for product_id in [2657, 2660, 2664, 2668, 2689] {
            let submission = submission();
            let operation = Operation::Equalizer {
                setting: submission.setting,
            };
            let error = anyhow::Error::new(razer_ipc::ServiceFailure {
                message: "cancelled after preset".into(),
                diagnostics: razer_ipc::FailureDiagnostics::EarbudsEqualizer(Failure {
                    product_id,
                    submission,
                    message: "cancelled after preset".into(),
                }),
            });
            let report = equalizer_failure(&error, product_id, operation)
                .unwrap()
                .unwrap();
            assert!(matches!(
                report.write,
                razer_device::earbuds_protocol40::WriteOutcome::Failed(_)
            ));
            assert!(report.equalizer.as_ref().unwrap().preset.acknowledged());
            assert!(!report.equalizer.as_ref().unwrap().bands.acknowledged());
            assert!(equalizer_failure(&error, product_id + 1, operation).is_err());
            assert!(
                equalizer_failure(&error, product_id, Operation::Read)
                    .unwrap()
                    .is_none()
            );
        }
    }
    #[test]
    fn earbuds_equalizer_cancelled_raw_response_remains_failure_and_validates_the_payload() {
        let submission = submission();
        let operation = Operation::Equalizer {
            setting: submission.setting,
        };
        let report = Report::from_equalizer(2657, submission).unwrap();
        let data = serde_json::to_value(report).unwrap();
        let error = anyhow::Error::new(razer_ipc::request_cancellation::CanceledResponse {
            data: data.clone(),
            error: "late cancellation".into(),
        });
        let report = equalizer_failure(&error, 2657, operation).unwrap().unwrap();
        assert!(matches!(
            report.write,
            razer_device::earbuds_protocol40::WriteOutcome::Failed(_)
        ));
        let mut foreign = data;
        foreign["equalizer"]["setting"]["preset"] = serde_json::json!("movie");
        let error = anyhow::Error::new(razer_ipc::request_cancellation::CanceledResponse {
            data: foreign,
            error: "late cancellation".into(),
        });
        assert!(equalizer_failure(&error, 2657, operation).is_err());
    }
}
