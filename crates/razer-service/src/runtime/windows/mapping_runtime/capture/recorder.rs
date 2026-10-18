use super::{Host, State, confirmed};
use anyhow::{Context, Result, ensure};
use razer_ipc::{ServiceRequest, macro_recording::{ActionReply, RestoreReceipt, SuspensionReceipt}};
use razer_platform::macro_recorder::{MacroRecorder, Mode, Events};
use serde_json::{Value, json};
use uuid::Uuid;

/// The simulated owner exercises the same lifecycle without installing hooks.
pub(crate) trait InputRecorder: Default {
    fn prepare(&mut self) -> Result<()>;
    fn start(&mut self, mode: Mode) -> Result<()>;
    fn mode(&self) -> Result<Option<Mode>>;
    fn stop(&mut self) -> Result<()>;
    fn events(&self) -> Result<Events>;
    fn shutdown(&mut self) -> Result<()>;
}
impl InputRecorder for MacroRecorder {
    fn prepare(&mut self) -> Result<()> { MacroRecorder::prepare(self) }
    fn start(&mut self, mode: Mode) -> Result<()> { MacroRecorder::start(self, mode) }
    fn mode(&self) -> Result<Option<Mode>> { MacroRecorder::mode(self) }
    fn stop(&mut self) -> Result<()> { MacroRecorder::stop(self) }
    fn events(&self) -> Result<Events> { MacroRecorder::events(self) }
    fn shutdown(&mut self) -> Result<()> { MacroRecorder::shutdown(self) }
}

pub(super) struct Recorder<R> {
    scope: Uuid,
    input: R,
    registered: bool,
    mappings_suspended: bool,
    shortcuts_suspended: bool,
}
impl<R: InputRecorder> Recorder<R> {
    fn start(&mut self, mode: Mode) -> Result<Value> {
        ensure!(self.registered && self.input.mode()?.is_none(), "Recorder requires a prepared inactive listener owner");
        match self.input.start(mode) {
            Ok(()) => Ok(json!({"accepted":true,"result":true,"reason":""})),
            Err(error) => Ok(json!({"accepted":false,"result":false,"reason":format!("{error:#}")})),
        }
    }
    fn stop(&mut self) -> Result<Value> {
        ensure!(self.registered, "Recorder listener owner is absent");
        if self.input.mode()?.is_some() { self.input.stop()?; }
        // The final input batch and stopped event remain owned until release.
        Ok(json!({"accepted":true}))
    }
    fn restore(&mut self, host: &mut impl Host) -> Result<(ActionReply, ActionReply)> {
        let mappings = host.set_mappings(self.scope, true);
        if mappings.as_ref().is_ok_and(ActionReply::result) { self.mappings_suspended=false; }
        // Source Promise.all submits both operations even if the first fails.
        let shortcuts = host.set_shortcuts(self.scope, true);
        if shortcuts.as_ref().is_ok_and(ActionReply::result) { self.shortcuts_suspended=false; }
        match (mappings, shortcuts) {
            (Ok(mapping), Ok(shortcuts)) => Ok((mapping, shortcuts)),
            (Err(error), Ok(_)) | (Ok(_), Err(error)) => Err(error),
            (Err(mapping), Err(shortcuts)) => Err(anyhow::anyhow!("mapping: {mapping:#}; shortcuts: {shortcuts:#}")),
        }
    }
    fn set_suspended(&mut self, suspended: bool, host: &mut impl Host) -> Result<()> {
        ensure!(self.registered, "OTFM listener owner is absent");
        // Preserve ownership of partially applied controls until restoration.
        if suspended { self.mappings_suspended=true; self.shortcuts_suspended=true; }
        let mapping = host.set_mappings(self.scope, !suspended).and_then(confirmed);
        if mapping.is_ok() { self.mappings_suspended=suspended; }
        let shortcuts = host.set_shortcuts(self.scope, !suspended).and_then(confirmed);
        if shortcuts.is_ok() { self.shortcuts_suspended=suspended; }
        match (mapping, shortcuts) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
            (Err(a), Err(b)) => Err(anyhow::anyhow!("mapping: {a:#}; shortcuts: {b:#}")),
        }
    }
}

impl<R: InputRecorder> State<R> {
    fn recorder(&mut self, scope: Uuid) -> Result<&mut Recorder<R>> {
        let recorder = self.recorder.as_mut().context("Recorder listener owner is absent")?;
        ensure!(recorder.scope==scope, "Recorder scope changed");
        Ok(recorder)
    }
    fn prepare(&mut self, scope: Uuid) -> Result<()> {
        self.vacant()?;
        // Keep the owner before acquisition so failed preparation can release
        // its partially acquired thread and input handles.
        self.recorder = Some(Recorder { scope, input: R::default(), registered:true,
            mappings_suspended:false, shortcuts_suspended:false });
        self.recorder.as_mut().unwrap().input.prepare()
    }
    pub(super) fn recorder_request(&mut self, scope: Uuid, request: &ServiceRequest, host: &mut impl Host) -> Option<Result<Value>> {
        use ServiceRequest::*;
        if !matches!(request, StartMacroRecording | PrepareOtfmRecorder | StartOtfmRecording | QueryMacroRecordingStarted | MacroRecordingMode | MacroRecorderGlobalState | SetOtfmMappingsSuspended{..} | MacroRecorderStorageWrite{..} | StopMacroRecording | MacroRecordingEvents | SuspendMacroMappings | ResumeMacroMappings) { return None; }
        Some((|| match request {
            StartMacroRecording => {
                self.prepare(scope)?;
                let value = self.recorder(scope)?.start(Mode::Software)?;
                ensure!(value.get("accepted")==Some(&Value::Bool(true)), "Software recorder rejected start: {}", value["reason"]);
                Ok(value)
            }
            PrepareOtfmRecorder => { self.prepare(scope)?; Ok(json!({"registered":true})) }
            StartOtfmRecording => self.recorder(scope)?.start(Mode::Otfm),
            QueryMacroRecordingStarted => Ok(json!({"result":self.recorder(scope)?.input.mode()?.is_some(),"reason":""})),
            MacroRecordingMode => Ok(json!({"mode":self.recorder(scope)?.input.mode()?.context("Recorder has not started")?.as_str()})),
            MacroRecorderGlobalState => {
                let mode = self.recorder(scope)?.input.mode()?.context("Recorder has not started")?;
                Ok(json!({"macroRecordingMode":mode.as_str(),"globalMode":host.global_mode()?}))
            }
            SetOtfmMappingsSuspended { suspended } => {
                self.recorder(scope)?.set_suspended(*suspended, host)?;
                Ok(json!({"suspended":suspended}))
            }
            MacroRecorderStorageWrite { key, value } => {
                ensure!(self.recorder(scope)?.registered, "Recorder listener owner is absent");
                ensure!(key=="synapseMacros" || key.starts_with("synapse_"), "Recorder publication key is outside the current local chain");
                host.publish_storage(scope, key, value)?;
                Ok(json!({"published":true,"key":key}))
            }
            StopMacroRecording => self.recorder(scope)?.stop(),
            MacroRecordingEvents => Ok(serde_json::to_value(self.recorder(scope)?.input.events()?)?),
            SuspendMacroMappings => {
                let recorder = self.recorder(scope)?;
                ensure!(recorder.input.mode()?.is_some(), "Macro recorder has not started");
                if !recorder.mappings_suspended {
                    recorder.mappings_suspended=true;
                    confirmed(host.set_mappings(scope, false)?)?;
                }
                Ok(serde_json::to_value(SuspensionReceipt::new(true))?)
            }
            ResumeMacroMappings => {
                let recorder = self.recorder(scope)?;
                let (mapping, shortcuts) = recorder.restore(host)?;
                Ok(serde_json::to_value(RestoreReceipt::new(recorder.mappings_suspended, mapping, shortcuts))?)
            }
            _ => unreachable!(),
        })())
    }

    pub(super) fn release_recorder(&mut self, scope: Uuid, host: &mut impl Host) -> Result<()> {
        let Some(recorder) = self.recorder.as_mut().filter(|owner|owner.scope==scope) else { return Ok(()); };
        let mut errors = Vec::new();
        if recorder.registered {
            if let Err(error) = recorder.stop() { errors.push(format!("Stop recorder: {error:#}")); }
        }
        if recorder.mappings_suspended {
            match recorder.restore(host) {
                Ok((mapping, _)) => if let Err(error) = confirmed(mapping) { errors.push(format!("Restore mappings: {error:#}")); },
                Err(error) => errors.push(format!("Restore recorder controls: {error:#}")),
            }
        }
        if recorder.shortcuts_suspended {
            match host.set_shortcuts(scope, true).and_then(confirmed) {
                Ok(()) => recorder.shortcuts_suspended=false,
                Err(error) => errors.push(format!("Restore recorder shortcuts: {error:#}")),
            }
        }
        if recorder.registered {
            match recorder.input.shutdown() {
                Ok(()) => recorder.registered=false,
                Err(error) => errors.push(format!("Release recorder hooks: {error:#}")),
            }
        }
        if !recorder.registered && !recorder.mappings_suspended && !recorder.shortcuts_suspended { self.recorder=None; }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
}
