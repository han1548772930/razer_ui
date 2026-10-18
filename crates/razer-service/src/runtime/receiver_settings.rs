//! Shared protocol task; OS adapters retain handle, lock, identity and deadline.
use anyhow::Context as _;
use razer_device::{backend::FeatureTransport, receiver_session::ReceiverSession};
use serde_json::{Value, json};

pub(super) enum Setting<'a> {
    MouseBrightness(&'a razer_device::mouse_brightness::Submission),
    KeyboardBrightness(Option<u8>),
    DpiStages(Option<&'a razer_device::mouse_dpi_stages::DpiStagesDraft>),
    CurrentDpi(Option<&'a razer_device::mouse_current_dpi::DpiChange>),
    Dynamic(Option<&'a razer_device::mouse_dynamic::Setting>),
    Rotation(Option<&'a razer_device::mouse_rotation::Setting>),
    CurrentPower(&'a razer_device::mouse_current_power::Change),
    Scroll(&'a razer_device::mouse_scroll::Submission),
}

impl Setting<'_> {
    pub(super) fn session(
        &self,
        product_id: u32,
        physical_product_id: u32,
        peer: u32,
    ) -> anyhow::Result<ReceiverSession> {
        match self {
            Self::Rotation(change) => {
                if let Some(change) = change {
                    change.validate()?;
                }
                razer_device::mouse_rotation::capability(product_id)
                    .context("Rotation current factory is absent")?
                    .receiver_session(physical_product_id, peer)
            }
            Self::MouseBrightness(submission) => {
                submission.validate(product_id)?;
                ReceiverSession::mouse(product_id, physical_product_id, peer)
            }
            Self::Dynamic(change) => {
                if let Some(change) = change {
                    change.validate()?;
                }
                razer_device::mouse_dynamic::capability(product_id)
                    .context("Dynamic current factory is absent")?
                    .receiver_session(physical_product_id, peer)
            }
            Self::CurrentPower(change) => {
                razer_device::mouse_current_power::validate_change(product_id, change)?;
                razer_device::mouse_current_power::capability(product_id)
                    .context("Current power source is absent")?
                    .receiver_session(physical_product_id, peer)
            }
            Self::Scroll(submission) => {
                submission.validate(product_id)?;
                razer_device::mouse_current_dpi::capability(product_id)
                    .context("Scroll physical factory is absent")?
                    .receiver_session(physical_product_id, peer)
            }
            Self::KeyboardBrightness(percent) => {
                if let Some(percent) = percent {
                    razer_device::keyboard_settings::encode_percent(*percent)?;
                }
                ReceiverSession::keyboard(product_id, physical_product_id, peer)
            }
            Self::DpiStages(draft) => {
                let settings = razer_device::mouse_dpi_stages::capability(product_id)
                    .context("DPI stage source is absent")?;
                if let Some(draft) = draft {
                    razer_device::mouse_dpi_stages::pack(settings, draft)?;
                }
                ReceiverSession::mouse(product_id, physical_product_id, peer)
            }
            Self::CurrentDpi(change) => {
                if let Some(change) = change {
                    razer_device::mouse_current_dpi::validate_change_for(product_id, change)?;
                }
                razer_device::mouse_current_dpi::capability(product_id)
                    .context("Current DPI source is absent")?
                    .receiver_session(physical_product_id, peer)
            }
        }
    }

    pub(super) fn execute(
        &self,
        session: &ReceiverSession,
        device: &dyn FeatureTransport,
        next: &mut dyn FnMut() -> anyhow::Result<u8>,
        validate: &dyn Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Value> {
        let product_id = session.transport().product_id;
        if let Self::Rotation(change) = self {
            let observation = session.observe(device, next, validate, |next, guard| {
                razer_device::mouse_rotation::execute(
                    device,
                    session.transport(),
                    *change,
                    next,
                    guard,
                )
            })?;
            return Ok(
                json!({"report":observation.reading,"receiver_before":observation.receiver_before,
                "receiver_after":observation.receiver_after,"profile_scope":"rotation_profile_1",
                "source_class":razer_device::mouse_rotation::capability(product_id)
                    .context("Rotation source is absent")?.transport().source_class,
                "transport_source_class":session.transport().source_class}),
            );
        }
        if let Self::MouseBrightness(submission) = self {
            let observation = session.observe(device, next, validate, |next, guard| {
                razer_device::mouse_brightness::execute(
                    device,
                    session.transport(),
                    submission,
                    &mut || next(),
                    &|| guard(),
                )
            })?;
            return Ok(
                json!({"report":observation.reading,"receiver_before":observation.receiver_before,"receiver_after":observation.receiver_after,"setting_scope":"active_and_matching_obm_brightness","source_class":razer_device::device_reads::capability(product_id).context("Brightness source is absent")?.source_class,"transport_source_class":session.transport().source_class}),
            );
        }
        if let Self::Dynamic(change) = self {
            let observation = session.observe(device, next, validate, |next, guard| {
                razer_device::mouse_dynamic::execute(
                    device,
                    session.transport(),
                    *change,
                    next,
                    guard,
                    &mut |duration| {
                        std::thread::sleep(duration);
                        guard()
                    },
                )
            })?;
            return Ok(
                json!({"report":observation.reading,"receiver_before":observation.receiver_before,
                "receiver_after":observation.receiver_after,"profile_scope":"dynamic_profile_1",
                "source_class":razer_device::mouse_dynamic::capability(product_id)
                    .context("Dynamic source is absent")?.transport().source_class,
                "transport_source_class":session.transport().source_class}),
            );
        }
        if let Self::CurrentPower(change) = self {
            let observation = session.observe(device, next, validate, |next, guard| {
                razer_device::mouse_current_power::execute(
                    product_id,
                    device,
                    session.transport(),
                    change,
                    next,
                    guard,
                )
            })?;
            return Ok(json!({
                "report": observation.reading,
                "receiver_before": observation.receiver_before,
                "receiver_after": observation.receiver_after,
                "setting_scope": "basic_device",
                "source_class": session.transport().source_class,
                "transport_source_class": session.transport().source_class,
            }));
        }
        if let Self::Scroll(submission) = self {
            let executor =
                razer_device::mouse_scroll::MouseScroll::new(device, session.transport())?;
            let observation = session.observe(device, next, validate, |next, guard| {
                executor.submit(submission, next, guard)
            })?;
            return Ok(
                json!({"report":observation.reading,"receiver_before":observation.receiver_before,
                "receiver_after":observation.receiver_after,"setting_scope":"source_scroll_profile",
                "source_class":razer_device::mouse_current_dpi::capability(product_id)
                    .context("Current scroll command owner is absent")?.transport().source_class,
                "transport_source_class":session.transport().source_class}),
            );
        }
        if let Self::CurrentDpi(change) = self {
            let observation = razer_device::mouse_current_dpi::execute_in_session(
                session, device, *change, next, validate,
            )?;
            return Ok(json!({
                "report": observation.reading,
                "receiver_before": observation.receiver_before,
                "receiver_after": observation.receiver_after,
                "profile_scope": "stage_1_active_0",
                "obm_persistence_verified": false,
                "source_class": razer_device::mouse_current_dpi::capability(product_id)
                    .context("Current DPI source is absent")?.transport().source_class,
                "transport_source_class": session.transport().source_class,
            }));
        }
        let (mut response, write, owner, method) = match self {
            Self::MouseBrightness(_) => {
                unreachable!("handled above with its brightness report schema")
            }
            Self::Dynamic(_) => unreachable!("handled above with its dynamic report schema"),
            Self::Rotation(_) => unreachable!("handled above with its rotation report schema"),
            Self::CurrentPower(_) => unreachable!("handled above with its basic power task schema"),
            Self::CurrentDpi(_) => unreachable!("handled above with its distinct report schema"),
            Self::Scroll(_) => unreachable!("handled above with its source scroll task schema"),
            Self::KeyboardBrightness(percent) => {
                let mut cap = razer_device::keyboard_settings::capability(product_id)
                    .context("Keyboard brightness source is absent")?
                    .clone();
                cap.transport = session.transport().clone();
                let observation = if let Some(percent) = percent {
                    serde_json::to_value(session.observe(
                        device,
                        next,
                        validate,
                        |next, guard| {
                            razer_device::keyboard_settings::apply(
                                device, &cap, *percent, next, guard,
                            )
                        },
                    )?)?
                } else {
                    serde_json::to_value(session.observe(
                        device,
                        next,
                        validate,
                        |next, guard| {
                            razer_device::keyboard_settings::read(device, &cap, next, guard)
                        },
                    )?)?
                };
                (
                    observation,
                    percent.is_some(),
                    cap.source_class.to_owned(),
                    if percent.is_some() {
                        "setBrightness"
                    } else {
                        "getBrightness"
                    },
                )
            }
            Self::DpiStages(draft) => {
                let mut cap = razer_device::mouse_dpi_stages::capability(product_id)
                    .context("DPI stage source is absent")?
                    .clone();
                cap.transport = session.transport().clone();
                let mut observation = if let Some(draft) = draft {
                    serde_json::to_value(session.observe(
                        device,
                        next,
                        validate,
                        |next, guard| {
                            razer_device::mouse_dpi_stages::apply_current(
                                device, &cap, draft, next, guard,
                            )
                        },
                    )?)?
                } else {
                    serde_json::to_value(session.observe(
                        device,
                        next,
                        validate,
                        |next, guard| {
                            razer_device::mouse_dpi_stages::read_current(device, &cap, next, guard)
                        },
                    )?)?
                };
                observation["profile_scope"] = json!("current_active_table");
                observation["obm_profiles_written"] = json!(false);
                (
                    observation,
                    draft.is_some(),
                    cap.source_class.to_owned(),
                    if draft.is_some() {
                        "setDPIStages"
                    } else {
                        "getDPIStages"
                    },
                )
            }
        };
        if write {
            response["result"] = response["reading"].take();
            response
                .as_object_mut()
                .context("Receiver observation is not an object")?
                .remove("reading");
        }
        response["source_class"] = json!(owner);
        response["transport_source_class"] = json!(session.transport().source_class);
        response["method"] = json!(method);
        Ok(response)
    }
}
