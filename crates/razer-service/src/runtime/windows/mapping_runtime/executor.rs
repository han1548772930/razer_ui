//! Source output dispatch with one retained physical-device lifetime.
use super::{
    Parameters, Program, Start,
    backend::{ExecutionContext, Notification},
    device,
    direct::Direct,
    display, system,
};
use anyhow::{Context, Result, ensure};
use razer_device::{
    macro_runtime::{
        scheduler::Mode,
        sequence::{self, Sequences},
    },
    mapping_native_action::Output,
    mapping_native_input::{KeyboardInput, MouseInput, NativeInput},
    mapping_native_lookup::{Input, InputKind, Resolved, ResolvedOutput},
};

pub(super) struct Executor {
    context: ExecutionContext,
    direct: Direct,
    display: display::Display,
    sequences: Sequences,
    contribution_id: Option<String>,
}
impl Executor {
    pub fn open(context: ExecutionContext) -> Result<Self> {
        let direct = Direct::open(
            context.route.clone(),
            context.cancellation.clone(),
            context.macros.clone(),
        )?;
        let display = display::Display::open(Some(context.route.clone()))?;
        Ok(Self {
            context,
            direct,
            display,
            sequences: Sequences::default(),
            contribution_id: None,
        })
    }
    fn notify(&self, input: &Input, payload: &str, tick: u64, global: bool) -> Result<()> {
        self.context
            .notifications
            .try_send(Notification {
                caller: self.context.caller,
                input: input.to_native_string(),
                output: payload.to_owned(),
                tick,
                global,
            })
            .context("Mapping notification consumer fell behind")
    }
    fn macro_action(
        &mut self,
        guid: &str,
        flag: u32,
        repeat: u32,
        program: Program,
        mapped_key: Resolved<Output>,
    ) -> Result<()> {
        if matches!(program, Program::Macro) && flag == 3 {
            self.sequences.begin(guid)?;
            return self.sequence_poll();
        }
        let mode = match flag {
            0 => Mode::Start,
            1 => Mode::Cancel,
            2 => Mode::Toggle,
            4 => Mode::Queue,
            _ => anyhow::bail!("Invalid execution flag {flag}"),
        };
        self.context
            .macros
            .lock()
            .map_err(|_| anyhow::anyhow!("Mapping macro owner poisoned"))?
            .submit(
                Start {
                    caller: self.context.caller,
                    guid: guid.into(),
                    parameters: Parameters {
                        repeat,
                        program,
                        driver_route: Some(self.context.route.clone()),
                        mapped_key: Some(mapped_key),
                    },
                },
                mode,
            )?;
        Ok(())
    }
    fn sequence_poll(&mut self) -> Result<()> {
        // Advance a bounded number of source cursor steps per device turn so
        // cancellation and new physical input retain their actual owner.
        for _ in 0..64 {
            if !self.sequences.is_active() {
                break;
            }
            let state = {
                let macros = self
                    .context
                    .macros
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Mapping macro owner poisoned"))?;
                let library = macros
                    .library
                    .read()
                    .map_err(|_| anyhow::anyhow!("Macro library poisoned"))?;
                self.sequences.poll(&library)?
            };
            match state {
                sequence::Poll::Progress => (),
                sequence::Poll::Actions(actions) => {
                    let mut errors = Vec::new();
                    for action in actions {
                        if let Err(error) = self.direct.action(action) {
                            errors.push(format!("{error:#}"));
                        }
                    }
                    ensure!(errors.is_empty(), "{}", errors.join("; "));
                    break;
                }
                sequence::Poll::Complete => break,
            }
        }
        Ok(())
    }
    fn output(
        &mut self,
        resolved: &Resolved<Output>,
        output: &Output,
        tick: u64,
        smart_stop: bool,
    ) -> Result<()> {
        let input = &resolved.input;
        let global = resolved.global_shortcut;
        let mapped_key = || {
            let mut key = resolved.clone();
            key.output = ResolvedOutput::Mapped(output.clone());
            key
        };
        match output {
            Output::Event(event) => self.direct.event(event, smart_stop),
            Output::Digital(output) => match &self.contribution_id {
                Some(id) => self.direct.combination(id, *output),
                None => self.direct.digital(&input.identity(), *output),
            },
            Output::Disabled => Ok(()),
            Output::Display(action) => self.display.execute(*action),
            Output::System(action) => system::execute(*action),
            Output::Macro { guid, flag, repeat } => {
                self.macro_action(guid, *flag, *repeat, Program::Macro, mapped_key())
            }
            Output::PhasedMacro {
                guid,
                flag,
                delay_ms,
            } => self.macro_action(
                guid,
                *flag,
                0,
                Program::Phased {
                    delay_ms: *delay_ms,
                },
                mapped_key(),
            ),
            Output::Turbo {
                guid,
                flag,
                repeat,
                delay_ms,
            } => self.macro_action(
                guid,
                *flag,
                *repeat,
                Program::Turbo {
                    delay_ms: *delay_ms,
                },
                mapped_key(),
            ),
            Output::Hypershift { flag } => {
                self.context
                    .modes
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Mapping modes poisoned"))?
                    .apply_hypershift(self.context.caller, *flag, tick)?;
                Ok(())
            }
            Output::Otfs { flag } => {
                self.context
                    .modes
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Mapping modes poisoned"))?
                    .apply_otfs(self.context.caller, *flag, tick)?;
                Ok(())
            }
            Output::Mode { payload, .. } => {
                self.notify(input, payload.as_json(), tick, global)?;
                self.context
                    .modes
                    .lock()
                    .map_err(|_| anyhow::anyhow!("Mapping modes poisoned"))?
                    .apply_mode_action(self.context.caller, input, output, tick)?;
                Ok(())
            }
            Output::CopyText(payload) | Output::Forwarded { payload, .. } => {
                self.notify(input, payload.as_json(), tick, global)
            }
            Output::Multi(outputs) => {
                let mut errors = Vec::new();
                for output in outputs {
                    if let Err(error) = self.output(resolved, output, tick, false) {
                        errors.push(format!("{error:#}"));
                    }
                }
                ensure!(errors.is_empty(), "{}", errors.join("; "));
                Ok(())
            }
            Output::Analog(_)
            | Output::DoubleMapping { .. }
            | Output::RapidTrigger { .. }
            | Output::Triggers(_) => {
                anyhow::bail!("Analog output requires a decoded hardware actuation observation")
            }
            Output::Cascade(_) | Output::Dpad(_) | Output::ModTap { .. } => {
                anyhow::bail!("Composite output requires the retained combination dispatcher")
            }
        }
    }
    fn passthrough(&mut self, input: &Input, tick: u64, global: bool) -> Result<()> {
        match input.kind {
            InputKind::Keyboard { scancode, flag } => {
                self.direct.native(NativeInput::Keyboard(KeyboardInput {
                    unit_id: input.unit_id.unwrap_or(0),
                    scancode,
                    flag,
                    modifiers: input.modifiers,
                    reserved: 0,
                }))
            }
            InputKind::Mouse { id, data } => self.direct.native(NativeInput::Mouse(MouseInput {
                unit_id: input.unit_id.unwrap_or(0),
                id,
                data,
                modifiers: input.modifiers,
                ..Default::default()
            })),
            // AD38C sends non-driver input types (including RazerKey/type35)
            // to the registered unsupported-output consumer via 858FE.
            _ => self.notify(input, &input.to_native_string(), tick, global),
        }
    }
}
impl device::Executor for Executor {
    fn next_wait(&self) -> std::time::Duration {
        self.direct.next_wait()
    }
    fn gamepad_observation(
        &mut self,
        control: device::gamepad_observation::Control,
        sink: device::gamepad_observation::Sink,
    ) -> Result<()> {
        self.direct.gamepad_observation(control, sink)
    }
    fn pause_mapping_replay(&mut self) -> Result<()> {
        // 82443/11aa78 pause the device thread between opposing edges.
        self.context
            .cancellation
            .wait(std::time::Duration::from_millis(
                super::random(15, 35)?.into(),
            ));
        Ok(())
    }
    fn interrupt_macro(&mut self, resolved: &Resolved<Output>) -> Result<()> {
        self.context
            .macros
            .lock()
            .map_err(|_| anyhow::anyhow!("Mapping macro owner poisoned"))?
            .interrupt_macro(self.context.caller, resolved)
    }
    fn begin_hardware_frame(&mut self, cleanup: bool) -> Result<()> {
        self.direct.cleanup(cleanup);
        self.direct.begin_hardware_frame()
    }
    fn finish_hardware_frame(&mut self) -> Result<()> {
        self.direct.finish_hardware_frame()
    }
    fn analog(&mut self, resolved: Resolved<Output>, actuation: u16) -> Result<()> {
        let ResolvedOutput::Mapped(Output::Analog(output)) = resolved.output else {
            anyhow::bail!("Hardware analog dispatch has no compiled analog output");
        };
        self.direct
            .analog(&resolved.input.identity(), &output, actuation)
    }
    fn execute(
        &mut self,
        resolved: Resolved<Output>,
        tick: u64,
        cleanup: bool,
        contribution_id: Option<String>,
    ) -> Result<()> {
        self.direct.cleanup(cleanup);
        self.contribution_id = contribution_id;
        match &resolved.output {
            ResolvedOutput::Mapped(output) => self.output(&resolved, output, tick, true),
            ResolvedOutput::Passthrough(input) => {
                self.passthrough(input, tick, resolved.global_shortcut)
            }
        }
    }
    fn prepare_redirect(&mut self) -> Result<()> {
        self.direct.stop_keyboard();
        self.sequences.cancel();
        Ok(())
    }
    fn redirect_activated(&mut self) -> Result<()> {
        self.direct.release_held()
    }
    fn poll(&mut self) -> Result<()> {
        self.direct.cleanup(false);
        let mut errors = self.direct.poll();
        if let Err(error) = self.sequence_poll() {
            errors.push(format!("Sequence output: {error:#}"));
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
    fn close(&mut self) -> Result<()> {
        self.sequences.clear();
        let mut errors = Vec::new();
        if let Err(error) = self.direct.close() {
            errors.push(format!("{error:#}"));
        }
        if let Err(error) = self.display.close() {
            errors.push(format!("{error:#}"));
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
}
