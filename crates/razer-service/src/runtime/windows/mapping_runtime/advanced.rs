//! A retained worker executes phased/turbo actions and their actual receipts.
use super::{Cancellation, Finished, acceleration, audio, clipboard, display, driver, input, launch, timing};
use anyhow::{Result, ensure};
use razer_device::macro_runtime::{Action, Completion, Keyboard, KeyboardExecutor, advanced::{Action as AdvancedAction, Execution, Poll}};
use std::{sync::Arc, thread, time::{Duration, Instant}};
use uuid::Uuid;

/// Source 11c460/11b314/11b654 share this ordering for each turbo cycle.
const MODIFIERS: [(i32, u16, u16); 8] = [
    (164, 56, 0), (165, 56, 2), (162, 29, 0), (163, 29, 2),
    (160, 42, 0), (161, 54, 0), (91, 91, 2), (92, 92, 2),
];

fn modifier(scan: u16, flag: u16) -> Keyboard {
    Keyboard { scancode: scan, flag, modifiers: 0, unit: None, delay_ms: None, release_after_ms: None }
}

struct Cycle {
    held: Option<Vec<(u16, u16)>>,
}
impl Cycle {
    fn begin(&mut self, input: &mut input::Input) -> Result<()> {
        ensure!(self.held.is_none(), "Turbo modifier cycle is already active");
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::GetKeyState;
        let held = MODIFIERS.into_iter().filter(|(vk, _, _)|unsafe { GetKeyState(*vk) } < 0)
            .map(|(_, scan, flag)|(scan, flag)).collect::<Vec<_>>();
        self.held = Some(held.clone());
        if !input.has_driver() { return Ok(()); }
        let mut errors = Vec::new();
        input.retain_held_outputs = false;
        for (scan, flag) in held {
            if let Err(error) = input.keyboard(&modifier(scan, flag | 1)) { errors.push(error); }
        }
        input.retain_held_outputs = true;
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
    fn end(&mut self, input: &mut input::Input) -> Result<()> {
        let Some(held) = self.held.take() else { return Ok(()); };
        if !input.has_driver() { return Ok(()); }
        input.cancellable = false;
        input.retain_held_outputs = false;
        let mut errors = Vec::new();
        for (scan, flag) in held {
            if let Err(error) = input.keyboard(&modifier(scan, flag)) { errors.push(error); }
        }
        input.retain_held_outputs = true;
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
}

pub(super) fn run(
    id: Uuid,
    mut execution: Execution,
    route: Option<driver::Route>,
    cancel: Arc<Cancellation>,
    clipboard: clipboard::Client,
    acceleration: acceleration::Owner,
) -> Result<Finished> {
    let epoch = Instant::now();
    let tick = || epoch.elapsed().as_micros().try_into().unwrap_or(u64::MAX);
    let mut input = input::Input::open(route.as_ref(), cancel.clone(), acceleration)?;
    let mut keyboard = KeyboardExecutor::new(timing());
    let mut display = display::Display::open(route.clone())?;
    let mut cycle = Cycle { held: None };
    let mut audio_observations = Vec::new();
    let mut timer_errors = Vec::new();
    let mut omitted_errors = 0u64;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| -> Result<Completion> {
        loop {
            if cancel.is_cancelled() { execution.cancel(); }
            for error in keyboard.poll(tick(), &mut |key| input.keyboard(key)) {
                if timer_errors.len() < 64 { timer_errors.push(error); }
                else { omitted_errors = omitted_errors.saturating_add(1); }
            }
            match execution.poll(tick()) {
                Poll::Action(pending) => {
                    // Turbo cancellation is observed at its cycle boundary;
                    // a pending cycle still delivers its source event sequence.
                    input.cancellable = false;
                    let result = match pending.action {
                        AdvancedAction::Output(Action::Keyboard(key)) => keyboard.execute(key, false, tick(), &mut |key|input.keyboard(key)).map_err(anyhow::Error::msg),
                        AdvancedAction::Output(Action::Mouse(mouse)) => input.send(razer_device::mapping_native_input::NativeInput::Mouse(mouse.input())),
                        AdvancedAction::Output(Action::MouseMove { mode,x,y,origin_epoch }) => input.movement(mode,x,y,origin_epoch),
                        AdvancedAction::Output(Action::Launch(action)) => launch::execute(&action),
                        AdvancedAction::Output(Action::Clipboard(text)) => clipboard.paste(text, route.clone()),
                        AdvancedAction::Output(Action::Audio(action)) => audio::execute(action).and_then(|report| {
                            audio_observations.extend(report.observations);
                            ensure!(report.errors.is_empty(), "{}", report.errors.join("; "));
                            Ok(())
                        }),
                        AdvancedAction::Display(action) => display.execute(action),
                        AdvancedAction::BeginTurboCycle => cycle.begin(&mut input),
                        AdvancedAction::EndTurboCycle => cycle.end(&mut input),
                    };
                    execution.acknowledge(pending.ticket, result.map_err(|error|format!("{error:#}")))?;
                }
                Poll::WaitingUntil(deadline) => {
                    let deadline = keyboard.next_deadline_us().map_or(deadline, |value|value.min(deadline));
                    cancel.wait(Duration::from_micros(deadline.saturating_sub(tick())));
                }
                Poll::WaitingForCancellation => cancel.wait(Duration::from_millis(16)),
                Poll::Progress => thread::yield_now(),
                Poll::WaitingForCompletion => anyhow::bail!("Advanced macro lost its pending output owner"),
                Poll::Complete(completion) => return Ok(completion),
            }
        }
    })).unwrap_or_else(|_|Err(anyhow::anyhow!("Advanced macro worker panicked")));
    keyboard.stop();
    let mut cleanup_errors = Vec::new();
    if let Err(error) = cycle.end(&mut input) { cleanup_errors.push(format!("Restore turbo modifiers: {error:#}")); }
    if let Err(error) = input.close() { cleanup_errors.push(format!("Close advanced input: {error:#}")); }
    if let Err(error) = display.close() { cleanup_errors.push(format!("Close advanced display: {error:#}")); }
    let mut completion = match result {
        Ok(value) => value,
        Err(error) => anyhow::bail!("{error:#}; cleanup: {}", cleanup_errors.join("; ")),
    };
    completion.cleanup_errors.extend(cleanup_errors);
    for error in input.operation_errors.drain(..).chain(timer_errors) { completion.record_operation_error(error); }
    completion.omitted_operation_errors = completion.omitted_operation_errors.saturating_add(omitted_errors);
    Ok(Finished { id, completion, last_input: input.last_receipt.take(), completed_inputs: input.completed, audio_observations })
}
