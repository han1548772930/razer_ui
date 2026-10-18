//! Phased/turbo publication and execution (1080b2/10a1aa, 129998/129e92).
//! Logical time and acknowledged output keep device I/O with the service owner.
use super::{Completion, Event, Outcome, Publication, held::HeldOutputs};
use crate::mapping_native_action::{DisplayAction, Output, parse_output};
use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;
use std::{collections::BTreeMap, sync::Arc};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurboSource {
    Default,
    User,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Output(super::Action),
    Display(DisplayAction),
    BeginTurboCycle,
    EndTurboCycle,
}

#[derive(Clone, Debug)]
enum ParsedEvent {
    Action(Action),
    Delay(u32),
    Invalid { error: String, object: bool },
}

#[derive(Debug)]
struct Definition {
    hash: String,
    phases: Vec<Vec<ParsedEvent>>,
}

#[derive(Default)]
pub struct Library {
    phased: BTreeMap<String, Arc<Definition>>,
    default_turbos: BTreeMap<String, Arc<Definition>>,
    user_turbos: BTreeMap<String, Arc<Definition>>,
}

impl Library {
    pub fn publish_phased(&mut self, document: &Value) -> Result<Publication> {
        publish(&mut self.phased, document, true)
    }
    pub fn publish_turbos(&mut self, source: TurboSource, document: &Value) -> Result<Publication> {
        publish(
            match source {
                TurboSource::Default => &mut self.default_turbos,
                TurboSource::User => &mut self.user_turbos,
            },
            document,
            false,
        )
    }
    pub fn clear(&mut self) {
        self.phased.clear();
        self.default_turbos.clear();
        self.user_turbos.clear();
    }
}

fn publish(
    current: &mut BTreeMap<String, Arc<Definition>>,
    document: &Value,
    phased: bool,
) -> Result<Publication> {
    let rows = document
        .as_array()
        .context("Advanced macro data must be an array")?;
    let mut next = BTreeMap::new();
    let mut changed = Vec::new();
    for row in rows {
        let row = row
            .as_object()
            .context("Advanced macro row must be an object")?;
        let guid = row
            .get("guid")
            .and_then(Value::as_str)
            .context("Advanced macro GUID is missing")?;
        ensure!(
            !next.contains_key(guid),
            "Duplicate advanced macro GUID {guid}"
        );
        let engine = row
            .get("appEngine")
            .filter(|value| value.is_object())
            .context("Advanced macro appEngine is missing")?;
        let hash = engine
            .get("hash")
            .and_then(Value::as_str)
            .context("Advanced macro hash is missing")?;
        if let Some(existing) = current.get(guid).filter(|item| item.hash == hash) {
            next.insert(guid.to_owned(), existing.clone());
            continue;
        }
        ensure!(
            crate::mapping_engine_hash::source_hash(engine)? == hash,
            "Advanced macro hash is invalid: {guid}"
        );
        let names: &[&str] = if phased {
            &["pressEvents", "holdEvents", "releaseEvents"]
        } else {
            &["events"]
        };
        let phases = names
            .iter()
            .map(|name| {
                engine
                    .get(name)
                    .and_then(Value::as_array)
                    .with_context(|| format!("Advanced macro {name} is missing"))
                    .map(|events| {
                        events
                            .iter()
                            .map(|event| match parse_event(event, phased) {
                                Ok(event) => event,
                                Err(error) => ParsedEvent::Invalid {
                                    error: format!("{error:#}"),
                                    object: event.is_object(),
                                },
                            })
                            .collect()
                    })
            })
            .collect::<Result<Vec<_>>>()?;
        next.insert(
            guid.to_owned(),
            Arc::new(Definition {
                hash: hash.to_owned(),
                phases,
            }),
        );
        changed.push(guid.to_owned());
    }
    let count = next.len();
    *current = next;
    Ok(Publication { changed, count })
}

fn parse_event(value: &Value, phased: bool) -> Result<ParsedEvent> {
    let kind = value
        .get("type")
        .and_then(Value::as_str)
        .context("Advanced macro event type is missing")?;
    if kind == "display" && !phased {
        let Output::Display(display) = parse_output(value)? else {
            unreachable!()
        };
        // 1296a0 executes driver-brightness variants through the display
        // controller as one step; the continuous driver mapping path is not used.
        let display = match display {
            DisplayAction::DriverBrightnessDown => DisplayAction::BrightnessDown(10),
            DisplayAction::DriverBrightnessUp => DisplayAction::BrightnessUp(10),
            DisplayAction::DriverBrightnessStop => return Ok(ParsedEvent::Delay(0)),
            display => display,
        };
        return Ok(ParsedEvent::Action(Action::Display(display)));
    }
    let event = super::parse_event(value)?;
    Ok(match event {
        Event::Keyboard(key) => ParsedEvent::Action(Action::Output(super::Action::Keyboard(key))),
        Event::Mouse(mouse) => ParsedEvent::Action(Action::Output(super::Action::Mouse(mouse))),
        Event::Audio(audio) if !phased => {
            ParsedEvent::Action(Action::Output(super::Action::Audio(audio)))
        }
        Event::Delay { milliseconds } if !phased => ParsedEvent::Delay(milliseconds),
        _ => bail!(
            "Invalid {} event type {kind}",
            if phased { "phased macro" } else { "turbo" }
        ),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket {
    execution: Uuid,
    sequence: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingAction {
    pub ticket: Ticket,
    pub action: Action,
    pub cleanup: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Poll {
    Action(PendingAction),
    WaitingForCompletion,
    WaitingUntil(u64),
    WaitingForCancellation,
    Progress,
    Complete(Completion),
}

enum Kind {
    Phased {
        phase: usize,
    },
    Turbo {
        remaining: u32,
        started: bool,
        in_cycle: bool,
        cycle_finished: bool,
        last_success: bool,
    },
}

pub struct Execution {
    identity: Uuid,
    sequence: u64,
    definition: Arc<Definition>,
    kind: Kind,
    index: usize,
    delay_us: u64,
    deadline: Option<u64>,
    gap_pending: bool,
    previous: Option<Action>,
    pending: Option<PendingAction>,
    held: HeldOutputs,
    cancelled: bool,
    finishing: bool,
    completion: Completion,
}

impl Execution {
    pub fn phased(library: &Library, guid: &str, delay_ms: u32) -> Result<Self> {
        let definition = library
            .phased
            .get(guid)
            .cloned()
            .with_context(|| format!("Cannot find phased macro {guid}"))?;
        Ok(Self::new(definition, Kind::Phased { phase: 0 }, delay_ms))
    }
    pub fn turbo(library: &Library, guid: &str, repeat: u32, delay_ms: u32) -> Result<Self> {
        let definition = library
            .default_turbos
            .get(guid)
            .or_else(|| library.user_turbos.get(guid))
            .cloned()
            .with_context(|| format!("Cannot find turbo {guid}"))?;
        ensure!(!definition.phases[0].is_empty(), "Turbo has no event");
        Ok(Self::new(
            definition,
            Kind::Turbo {
                remaining: repeat,
                started: false,
                in_cycle: false,
                cycle_finished: false,
                last_success: true,
            },
            delay_ms,
        ))
    }
    fn new(definition: Arc<Definition>, kind: Kind, delay_ms: u32) -> Self {
        Self {
            identity: Uuid::new_v4(),
            sequence: 0,
            definition,
            kind,
            index: 0,
            delay_us: u64::from(delay_ms) * 1000,
            deadline: None,
            gap_pending: false,
            previous: None,
            pending: None,
            held: HeldOutputs::default(),
            cancelled: false,
            finishing: false,
            completion: Completion {
                outcome: Outcome::Completed,
                execution_error: None,
                operation_errors: Vec::new(),
                omitted_operation_errors: 0,
                cleanup_errors: Vec::new(),
            },
        }
    }
    pub fn cancel(&mut self) {
        self.cancelled = true;
        self.deadline = None;
    }
    pub fn fail(&mut self, error: String) -> Result<()> {
        ensure!(
            self.pending.is_none(),
            "Pending advanced output must complete before failure"
        );
        self.completion.execution_error.get_or_insert(error);
        self.finishing = true;
        self.deadline = None;
        Ok(())
    }
    pub fn acknowledge(&mut self, ticket: Ticket, result: Result<(), String>) -> Result<()> {
        let pending = self
            .pending
            .as_ref()
            .context("No pending advanced macro output")?;
        ensure!(
            pending.ticket == ticket,
            "Stale advanced macro output completion"
        );
        let pending = self.pending.take().unwrap();
        if pending.cleanup {
            if let Err(error) = result {
                self.completion.cleanup_errors.push(error);
            }
            return Ok(());
        }
        if let Kind::Turbo {
            in_cycle, started, ..
        } = &mut self.kind
        {
            match pending.action {
                Action::BeginTurboCycle => {
                    *in_cycle = true;
                    *started = true;
                }
                Action::EndTurboCycle => *in_cycle = false,
                _ => (),
            }
        }
        match result {
            Ok(()) => {
                if let Action::Output(action) = &pending.action {
                    self.held.track(action);
                }
                if matches!(self.kind, Kind::Phased { .. }) {
                    self.previous = Some(pending.action);
                    self.gap_pending = true;
                } else if let Kind::Turbo { last_success, .. } = &mut self.kind
                    && !matches!(
                        pending.action,
                        Action::BeginTurboCycle | Action::EndTurboCycle
                    )
                {
                    *last_success = true;
                }
            }
            Err(error) => match &mut self.kind {
                Kind::Phased { .. } => {
                    self.completion.execution_error = Some(error);
                    self.finishing = true;
                }
                Kind::Turbo { last_success, .. } => {
                    if matches!(
                        pending.action,
                        Action::BeginTurboCycle | Action::EndTurboCycle
                    ) {
                        self.completion.execution_error = Some(error);
                        self.finishing = true;
                    } else {
                        *last_success = matches!(
                            pending.action,
                            Action::Display(_) | Action::Output(super::Action::Audio(_))
                        );
                        self.completion.record_operation_error(error);
                    }
                }
            },
        }
        Ok(())
    }
    pub fn poll(&mut self, now_us: u64) -> Poll {
        if self.pending.is_some() {
            return Poll::WaitingForCompletion;
        }
        if self.finishing {
            return self.finish();
        }
        if self.cancelled
            && let Kind::Phased { phase } = &mut self.kind
            && *phase < 2
        {
            *phase = 2;
            self.index = 0;
            self.gap_pending = false;
        }
        if let Some(deadline) = self.deadline {
            if !self.cancelled && now_us < deadline {
                return Poll::WaitingUntil(deadline);
            }
            self.deadline = None;
        }
        if self.gap_pending {
            self.gap_pending = false;
            if !self.cancelled && self.delay_us > 0 {
                return self.wait(now_us, self.delay_us);
            }
        }
        match &mut self.kind {
            Kind::Phased { phase } => {
                if self.index == self.definition.phases[*phase].len() {
                    match *phase {
                        0 => {
                            *phase = 1;
                            self.index = 0;
                            return Poll::Progress;
                        }
                        1 => {
                            self.index = 0;
                            if self.definition.phases[1].is_empty() {
                                return if self.delay_us == 0 {
                                    Poll::WaitingForCancellation
                                } else {
                                    self.wait(now_us, self.delay_us)
                                };
                            }
                            return Poll::Progress;
                        }
                        _ => {
                            self.finishing = true;
                            return self.finish();
                        }
                    }
                }
            }
            Kind::Turbo {
                remaining,
                started,
                in_cycle,
                cycle_finished,
                last_success,
            } => {
                if *cycle_finished {
                    if *in_cycle {
                        return self.issue(Action::EndTurboCycle, false);
                    }
                    *cycle_finished = false;
                    if *remaining > 0 {
                        *remaining -= 1;
                        if *remaining == 0 {
                            self.finishing = true;
                        }
                    }
                    if !*last_success {
                        self.completion
                            .execution_error
                            .get_or_insert("Turbo event failed".into());
                        self.finishing = true;
                    }
                    if self.cancelled {
                        self.finishing = true;
                    }
                    if self.finishing {
                        return self.finish();
                    }
                    self.index = 0;
                    if self.delay_us > 0 {
                        return self.wait(now_us, self.delay_us);
                    }
                    return Poll::Progress;
                }
                if self.index == self.definition.phases[0].len() {
                    *cycle_finished = true;
                    return Poll::Progress;
                }
                if !*in_cycle && *started && self.cancelled {
                    self.finishing = true;
                    return self.finish();
                }
                if !*in_cycle {
                    return self.issue(Action::BeginTurboCycle, false);
                }
            }
        }
        let phase = match self.kind {
            Kind::Phased { phase } => phase,
            Kind::Turbo { .. } => 0,
        };
        let event = self.definition.phases[phase][self.index].clone();
        self.index += 1;
        match event {
            ParsedEvent::Action(action) => {
                if matches!(self.kind, Kind::Phased { .. })
                    && self.previous.as_ref() == Some(&action)
                {
                    self.gap_pending = true;
                    Poll::Progress
                } else {
                    self.issue(action, false)
                }
            }
            ParsedEvent::Delay(milliseconds) => {
                if let Kind::Turbo { last_success, .. } = &mut self.kind {
                    *last_success = true;
                }
                if self.cancelled || milliseconds == 0 {
                    Poll::Progress
                } else {
                    self.wait(now_us, u64::from(milliseconds) * 1000)
                }
            }
            ParsedEvent::Invalid { error, object } => {
                if matches!(self.kind, Kind::Phased { .. }) && object {
                    self.completion.record_operation_error(error);
                } else {
                    self.completion.execution_error = Some(error);
                    self.finishing = true;
                }
                Poll::Progress
            }
        }
    }
    fn wait(&mut self, now: u64, duration: u64) -> Poll {
        let deadline = now.saturating_add(duration);
        self.deadline = Some(deadline);
        Poll::WaitingUntil(deadline)
    }
    fn issue(&mut self, action: Action, cleanup: bool) -> Poll {
        self.sequence += 1;
        let pending = PendingAction {
            ticket: Ticket {
                execution: self.identity,
                sequence: self.sequence,
            },
            action,
            cleanup,
        };
        self.pending = Some(pending.clone());
        Poll::Action(pending)
    }
    fn finish(&mut self) -> Poll {
        if let Kind::Turbo { in_cycle, .. } = &mut self.kind
            && *in_cycle
        {
            *in_cycle = false;
            return self.issue(Action::EndTurboCycle, true);
        }
        if let Some(release) = self.held.pop_release() {
            return self.issue(Action::Output(release), true);
        }
        self.completion.outcome = if self.completion.execution_error.is_some() {
            Outcome::Failed
        } else if self.cancelled {
            Outcome::Cancelled
        } else {
            Outcome::Completed
        };
        Poll::Complete(self.completion.clone())
    }
}

#[cfg(test)]
#[path = "advanced/tests.rs"]
mod tests;
