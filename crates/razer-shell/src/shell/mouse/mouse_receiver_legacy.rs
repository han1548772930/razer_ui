//! The legacy MW actor persists allowed events before its skippable SET queue.
//! GET -> local save -> source UI publication -> SET is deliberately explicit.
use super::*;
use razer_device::mouse_receiver_legacy::{Change, Target};
use razer_discovery::direct::mouse_receiver_legacy as direct;
use razer_storage::receiver_legacy as storage;

struct Pending {
    request: Request,
    byte: u8,
}
struct LedSession {
    route: Target,
    serial: String,
}
fn source_slave(records: &[Value], container: &str, dongle: u32) -> bool {
    // UZ/J uses the first matching Object.values record and exact source
    // ContainerId. This is a local role decision, not a hardware observation.
    records
        .iter()
        .find(|row| {
            row["deviceContainerId"] == container
                && (row["dongleId"] == dongle || row["master"]["dongleId"] == dongle)
        })
        .is_some_and(|row| row["dongleId"] == dongle)
}
fn binding_info(
    client: &mut ServiceClient,
    observed: &ObservedDevice,
    runtime: &[Value],
) -> anyhow::Result<Value> {
    ensure!(
        observed.product_id() == 192,
        "This source TopView has no binding-info caller"
    );
    let route = direct::resolve_bindings(client, observed)?;
    let report = direct::execute(client, &route, Change::ReadBindings)?;
    let rows = report
        .bindings
        .context("Runtime binding getter omitted its response")?;
    razer_device::receiver_pairing::source192_binding_info(
        observed.container(),
        &rows,
        || source_local::get_raw(&source_local::path()?, "connectedDeviceInfo"),
        || client.request(ServiceRequest::HidDevices),
        runtime,
    )
}
fn source_byte(value: &Value) -> u8 {
    // This protocol uses a Uint8Array assignment without an enum guard.
    let number = razer_device::mouse_indicator_led::source_configuration_number(Some(value));
    if number.is_finite() {
        number.trunc().rem_euclid(256.) as u8
    } else {
        0
    }
}
fn prepare(
    client: &mut ServiceClient,
    led: &mut Option<LedSession>,
    observed: &ObservedDevice,
    request: Request,
    profile: Value,
    runtime: &[Value],
    macro_exclusive: &AtomicBool,
    updates: &Sender<Update>,
) -> anyhow::Result<Option<Pending>> {
    ensure!(request.is_valid(), "Single LED event owner canceled");
    client.set_request_guard(request.guard());
    if matches!(request.action(), Action::LegacyBindingInfo) {
        let paired = match binding_info(client, observed, runtime) {
            Ok(paired) => paired,
            Err(error) => {
                diagnostic(updates, format!("DUALLINK_BIND_INFO: {error:#}"));
                // The current error response explicitly publishes [] and
                // TopView consumes it before inspecting the error field.
                serde_json::json!([])
            }
        };
        ensure!(request.is_valid(), "Binding-info owner canceled");
        updates.send(Update::LegacyBindingInfo(request, paired))?;
        return Ok(None);
    }
    // The independent mounted TopView getter above must not wait for the
    // receiver-only setupDevice serial/LED initialization path.
    if led.is_none() {
        let route = direct::resolve(client, observed)?;
        let serial = match direct::execute(client, &route, Change::ReadSerial) {
            Ok(report) => {
                let serial = report.serial_number()?;
                updates.send(Update::LegacyReport(report))?;
                serial
            }
            Err(error) => {
                diagnostic(updates, format!("Receiver serial query: {error:#}"));
                String::new()
            }
        };
        ensure!(request.is_valid(), "Single LED owner canceled during setup");
        *led = Some(LedSession { route, serial });
    }
    let LedSession { route, serial } = led.as_ref().context("Single LED setup is absent")?;
    let (status, persist, initialization) = match request.action() {
        Action::LegacyInitialize => {
            // 194 registers an empty config. Its source setup requires the
            // actual disableInDuallinkSlave flag before even invoking GET.
            if serial.is_empty()
                || !razer_device::mouse_receiver_legacy::initial_indicator_getter_enabled(
                    observed.product_id(),
                )
            {
                return Ok(None);
            }
            {
                let records = razer_storage::receiver_pairing::PairingCache::load(
                    &razer_storage::receiver_pairing::local_path()?,
                )?
                .ordered_records();
                if source_slave(
                    &records,
                    observed.container(),
                    observed.physical_product_id(),
                ) {
                    return Ok(None);
                }
            }
            let report = direct::execute(client, route, Change::Read)?;
            let status = report.queried_status;
            updates.send(Update::LegacyReport(report))?;
            // The source tests jsonData?.status for truthiness. Zero and an
            // absent byte never manufacture a successful default selection.
            let Some(status) = status.filter(|status| *status != 0) else {
                return Ok(None);
            };
            (serde_json::json!(status), true, true)
        }
        Action::LegacySet { status, persist } => (status.clone(), *persist, false),
        Action::Initialize | Action::Set { .. } => {
            anyhow::bail!("Three-LED request reached the single-LED owner")
        }
        Action::LegacyBindingInfo => unreachable!("handled before receiver LED setup"),
    };
    // Source w/k grants only updateLocalStorage during exclusive recording.
    // yo is wholly guarded by deviceAccess, including its local save and MW
    // publication. The setup getter above precedes that decision. UI changes
    // already made by the optimistic reducer remain untouched.
    if macro_exclusive.load(Ordering::Acquire) {
        return Ok(None);
    }
    ensure!(
        request.is_valid(),
        "Single LED event canceled before persistence"
    );
    if persist {
        storage::save_selection(
            &source_local::path()?,
            observed.product_id(),
            serial,
            &profile,
            &status,
        )?;
    }
    ensure!(
        request.is_valid(),
        "Single LED event canceled before source publication"
    );
    if initialization {
        updates.send(Update::Initialized(request.clone(), status.clone()))?;
    }
    Ok(Some(Pending {
        request,
        byte: source_byte(&status),
    }))
}
pub(super) fn run(
    observed: &ObservedDevice,
    guard: Arc<AtomicBool>,
    macro_exclusive: Arc<AtomicBool>,
    commands: Receiver<Command>,
    updates: &Sender<Update>,
) -> anyhow::Result<()> {
    validate(&guard)?;
    let mut client = ServiceClient::spawn()?;
    client.set_request_guard(guard.clone());
    let result = (|| {
        let mut led = None;
        validate(&guard)?;
        loop {
            validate(&guard)?;
            let command = match commands.recv_timeout(Duration::from_millis(20)) {
                Ok(command) => command,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            };
            let mut command = Some(command);
            let mut pending: Option<Pending> = None;
            let mut stop = false;
            while let Some(next) = command.take() {
                match next {
                    Command::Stop => {
                        stop = true;
                        break;
                    }
                    Command::Request {
                        request,
                        profile,
                        runtime,
                    } => {
                        let prepared = prepare(
                            &mut client,
                            &mut led,
                            observed,
                            request.clone(),
                            profile,
                            &runtime,
                            &macro_exclusive,
                            updates,
                        );
                        match prepared {
                            Ok(Some(next)) => {
                                if let Some(skipped) = pending.replace(next) {
                                    // Its actor/local-storage operation already
                                    // ran; NORMAL_SKIPPABLE skips only the SET.
                                    updates.send(Update::Finished(skipped.request, None))?;
                                }
                            }
                            Ok(None) => {
                                updates.send(Update::Finished(request, None))?;
                            }
                            Err(error) => {
                                diagnostic(
                                    updates,
                                    format!("Single receiver LED event: {error:#}"),
                                );
                                updates.send(Update::Finished(request, None))?;
                            }
                        }
                    }
                }
                command = match commands.try_recv() {
                    Ok(command) => Some(command),
                    Err(mpsc::TryRecvError::Empty) => None,
                    Err(mpsc::TryRecvError::Disconnected) => {
                        stop = true;
                        None
                    }
                };
            }
            if stop || !guard.load(Ordering::Acquire) {
                break;
            }
            if let Some(pending) = pending {
                client.set_request_guard(pending.request.guard());
                let result = (|| {
                    ensure!(pending.request.is_valid(), "Single LED SET canceled");
                    validate(&guard)?;
                    let route = &led
                        .as_ref()
                        .context("Single LED setter has no initialized receiver")?
                        .route;
                    let report = direct::execute(&mut client, route, Change::Set(pending.byte))?;
                    updates.send(Update::LegacyReport(report))?;
                    Ok::<_, anyhow::Error>(())
                })();
                if let Err(error) = result {
                    diagnostic(updates, format!("Single receiver LED SET: {error:#}"));
                }
                updates.send(Update::Finished(pending.request, None))?;
            }
            client.set_request_guard(guard.clone());
            if client.is_stopped() {
                anyhow::bail!("Single receiver LED service stopped");
            }
        }
        Ok::<_, anyhow::Error>(())
    })();
    client.set_request_guard(Arc::new(AtomicBool::new(true)));
    if !client.is_stopped() {
        if let Err(error) = client.request(ServiceRequest::Shutdown) {
            diagnostic(updates, format!("Single LED service cleanup: {error:#}"));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::source_slave;
    #[test]
    fn role_uses_exact_container_and_first_matching_local_record() {
        let rows = serde_json::json!([
            {"deviceContainerId":"{owner}","dongleId":713,"master":{"dongleId":193}},
            {"deviceContainerId":"{owner}","dongleId":193,"master":{"dongleId":757}}
        ]);
        assert!(!source_slave(rows.as_array().unwrap(), "{owner}", 193));
        assert!(source_slave(&[rows[1].clone()], "{owner}", 193));
        assert!(!source_slave(rows.as_array().unwrap(), "{OWNER}", 193));
        assert!(!source_slave(&[], "{owner}", 193));
    }
}
