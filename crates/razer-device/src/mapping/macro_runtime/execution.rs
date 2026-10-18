use super::held::HeldOutputs;
use super::{AudioAction, Definition, Event, Keyboard, Launch, Library, Mouse, MovementMode};
use anyhow::{Context, Result, ensure};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Keyboard(Keyboard),
    Mouse(Mouse),
    MouseMove {
        mode: MovementMode,
        x: i32,
        y: i32,
        origin_epoch: u64,
    },
    Clipboard(String),
    Launch(Launch),
    Audio(AudioAction),
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Completion {
    pub outcome: Outcome,
    pub execution_error: Option<String>,
    pub operation_errors: Vec<String>,
    pub omitted_operation_errors: u64,
    pub cleanup_errors: Vec<String>,
}

impl Completion {
    pub fn record_operation_error(&mut self, error: String) {
        record_error(
            &mut self.operation_errors,
            &mut self.omitted_operation_errors,
            error,
        );
    }
}

fn record_error(errors: &mut Vec<String>, omitted: &mut u64, error: String) {
    if errors.len() < 64 {
        errors.push(error);
    } else {
        *omitted = omitted.saturating_add(1);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Poll {
    Action(PendingAction),
    WaitingForCompletion,
    WaitingUntil(u64),
    Progress,
    Complete(Completion),
}

struct LoopFrame {
    start: usize,
    next: usize,
    remaining: u32,
}
struct Frame {
    definition: Arc<Definition>,
    index: usize,
    repeat: u32,
    loop_frame: Option<LoopFrame>,
}

pub struct Execution {
    identity: Uuid,
    sequence: u64,
    stack: Vec<Frame>,
    held: HeldOutputs,
    pending: Option<PendingAction>,
    delayed_action: Option<Action>,
    deadline: Option<u64>,
    previous_keyboard: bool,
    movement_epoch: u64,
    cancelled: bool,
    error: Option<String>,
    operation_errors: Vec<String>,
    omitted_operation_errors: u64,
    cleanup_errors: Vec<String>,
    finishing: bool,
}

impl Execution {
    pub fn start(library: &Library, guid: &str, repeat: u32) -> Result<Self> {
        let definition = library
            .get(guid)
            .with_context(|| format!("Cannot find macro {guid}"))?;
        ensure!(!definition.events.is_empty(), "Macro has no events");
        Ok(Self {
            identity: Uuid::new_v4(),
            sequence: 0,
            stack: vec![Frame {
                definition,
                index: 0,
                repeat,
                loop_frame: None,
            }],
            held: HeldOutputs::default(),
            pending: None,
            delayed_action: None,
            deadline: None,
            previous_keyboard: false,
            movement_epoch: 1,
            cancelled: false,
            error: None,
            operation_errors: Vec::new(),
            omitted_operation_errors: 0,
            cleanup_errors: Vec::new(),
            finishing: false,
        })
    }

    pub fn cancel(&mut self) {
        self.cancelled = true;
        self.deadline = None;
    }

    /// The I/O owner can fail between actions without skipping releases for
    /// outputs whose actual completion has already been acknowledged.
    pub fn fail(&mut self, error: String) -> Result<()> {
        ensure!(
            self.pending.is_none(),
            "Pending macro output must complete before failure"
        );
        self.error.get_or_insert(error);
        self.deadline = None;
        self.delayed_action = None;
        Ok(())
    }

    /// A submission remains owned until its real completion. Cancellation does
    /// not discard a late successful make: that output must still be released.
    pub fn acknowledge(&mut self, ticket: Ticket, result: Result<(), String>) -> Result<()> {
        let pending = self
            .pending
            .as_ref()
            .context("Macro has no pending output")?;
        ensure!(pending.ticket == ticket, "Stale macro output completion");
        let pending = self.pending.take().unwrap();
        if pending.cleanup {
            if let Err(error) = result {
                self.cleanup_errors.push(error);
            }
        } else {
            match result {
                Ok(()) => self.held.track(&pending.action),
                // The source audio/clipboard cases advance regardless of their
                // operation result. Preserve that control flow and retain the
                // real failure separately from its macro completion status.
                Err(error) if matches!(pending.action, Action::Audio(_) | Action::Clipboard(_)) => {
                    record_error(
                        &mut self.operation_errors,
                        &mut self.omitted_operation_errors,
                        error,
                    )
                }
                Err(error) => self.error = Some(error),
            }
        }
        Ok(())
    }

    /// Logical microseconds come from the owned service clock. Randomness is
    /// supplied by the platform boundary and validated before scheduling.
    pub fn poll(
        &mut self,
        now_us: u64,
        library: &Library,
        random: &mut impl FnMut(u32, u32) -> Result<u32>,
    ) -> Poll {
        if self.pending.is_some() {
            return Poll::WaitingForCompletion;
        }
        // The source's consecutive-key wait is interruptible but its already
        // dispatched key is sent after the wait returns, before the next step
        // observes cancellation. That key therefore also owns a release.
        if self.cancelled
            && let Some(action) = self.delayed_action.take()
        {
            return self.issue(action, false);
        }
        if self.cancelled || self.error.is_some() {
            self.finishing = true;
        }
        if self.finishing {
            return self.finish();
        }
        if let Some(deadline) = self.deadline {
            if now_us < deadline {
                return Poll::WaitingUntil(deadline);
            }
            self.deadline = None;
        }
        if let Some(action) = self.delayed_action.take() {
            return self.issue(action, false);
        }
        let Some(frame) = self.stack.last_mut() else {
            self.finishing = true;
            return self.finish();
        };
        if frame.index == frame.definition.events.len() {
            if let Some(loop_frame) = frame.loop_frame.as_mut() {
                loop_frame.remaining -= 1;
                if loop_frame.remaining != 0 {
                    frame.index = loop_frame.start;
                } else {
                    frame.index = loop_frame.next;
                    frame.loop_frame = None;
                }
            } else if frame.repeat == 0 || frame.repeat > 1 {
                if frame.repeat != 0 {
                    frame.repeat -= 1;
                }
                frame.index = 0;
                self.movement_epoch = self.movement_epoch.wrapping_add(1);
            } else {
                self.stack.pop();
            }
            return Poll::Progress;
        }
        let event = frame.definition.events[frame.index].clone();
        frame.index += 1;
        let event = match event {
            Ok(event) => event,
            Err(error) => {
                self.error = Some(error);
                return Poll::Progress;
            }
        };
        let was_keyboard = self.previous_keyboard;
        self.previous_keyboard = matches!(event, Event::Keyboard(_));
        let action = match event {
            Event::Keyboard(keyboard) => {
                let action = Action::Keyboard(keyboard);
                if was_keyboard {
                    self.delayed_action = Some(action);
                    self.deadline = Some(now_us.saturating_add(10_000));
                    return Poll::WaitingUntil(self.deadline.unwrap());
                }
                action
            }
            Event::Mouse(mouse) => {
                // MappingDataMouseWin+224 is precisely packed_buttons==16.
                // Only duplicate middle-button makes take this source branch.
                if mouse.input().packed_buttons() == 16 && self.held.has_middle_make(mouse.unit) {
                    self.previous_keyboard = was_keyboard;
                    return Poll::Progress;
                }
                Action::Mouse(mouse)
            }
            Event::MouseMove { x, y } => Action::MouseMove {
                mode: frame.definition.movement,
                x,
                y,
                origin_epoch: self.movement_epoch,
            },
            Event::Clipboard(text) => Action::Clipboard(text),
            Event::Launch(launch) => Action::Launch(launch),
            Event::Audio(audio) => Action::Audio(audio),
            Event::Delay { milliseconds } => return self.delay(now_us, milliseconds),
            Event::RandomDelay { min_ms, max_ms } => {
                if max_ms < min_ms {
                    return Poll::Progress;
                }
                return match random(min_ms, max_ms) {
                    Ok(milliseconds) if (min_ms..=max_ms).contains(&milliseconds) => {
                        self.delay(now_us, milliseconds)
                    }
                    Ok(_) => {
                        self.error =
                            Some("Random delay source returned an out-of-range value".into());
                        Poll::Progress
                    }
                    Err(error) => {
                        self.error = Some(format!("{error:#}"));
                        Poll::Progress
                    }
                };
            }
            Event::LoopStart { repeat } => {
                frame.loop_frame = Some(LoopFrame {
                    start: frame.index,
                    next: frame.definition.events.len(),
                    remaining: repeat,
                });
                return Poll::Progress;
            }
            Event::LoopEnd => {
                if let Some(loop_frame) = frame.loop_frame.as_mut() {
                    loop_frame.next = frame.index;
                    frame.index = frame.definition.events.len();
                }
                return Poll::Progress;
            }
            Event::Macro { guid, repeat } => {
                if self.stack.len() >= 100 {
                    self.error = Some("max macro recursion".into());
                    return Poll::Progress;
                }
                match library.get(&guid) {
                    Some(definition) if !definition.events.is_empty() => {
                        self.stack.push(Frame {
                            definition,
                            index: 0,
                            repeat,
                            loop_frame: None,
                        });
                        self.movement_epoch = self.movement_epoch.wrapping_add(1);
                    }
                    Some(_) => self.error = Some("no macro event(s)".into()),
                    None => self.error = Some(format!("Cannot find macro {guid}")),
                }
                return Poll::Progress;
            }
        };
        self.issue(action, false)
    }

    fn delay(&mut self, now: u64, milliseconds: u32) -> Poll {
        if milliseconds == 0 {
            return Poll::Progress;
        }
        let deadline = now.saturating_add(u64::from(milliseconds) * 1000);
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
        if let Some(action) = self.held.pop_release() {
            return self.issue(action, true);
        }
        Poll::Complete(Completion {
            outcome: if self.error.is_some() {
                Outcome::Failed
            } else if self.cancelled {
                Outcome::Cancelled
            } else {
                Outcome::Completed
            },
            execution_error: self.error.clone(),
            operation_errors: self.operation_errors.clone(),
            omitted_operation_errors: self.omitted_operation_errors,
            cleanup_errors: self.cleanup_errors.clone(),
        })
    }
}
