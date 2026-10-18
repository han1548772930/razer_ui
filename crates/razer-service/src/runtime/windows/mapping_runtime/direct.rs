//! Device-thread primitive output execution through the retained driver route.
use super::{Cancellation, State, audio, driver, input, launch, timing};
use anyhow::{Result, ensure};
use razer_device::{
    macro_runtime::{Event, Keyboard, KeyboardExecutor},
    mapping_native_analog::NativeAnalogOutput,
    mapping_native_digital::{NativeDigitalMappingState, NativeDigitalOutput, NativeMappingDelta},
    mapping_native_input::NativeInput,
    mapping_native_lookup::InputIdentity,
    mapping_native_output::{CompletionPoll, DriverInputReport, MappingOutputTransport},
};
use std::{collections::BTreeMap, sync::{Arc, Mutex}, time::{Duration, Instant}};

pub(super) struct Direct {
    input: input::Input,
    output: driver::Output,
    route: driver::Route,
    keyboard: KeyboardExecutor,
    digital: NativeDigitalMappingState,
    identities: BTreeMap<InputIdentity, u64>,
    analog: BTreeMap<InputIdentity, NativeAnalogOutput>,
    next_identity: u64,
    cancel: Arc<Cancellation>,
    manager: Arc<Mutex<State>>,
    epoch: Instant,
    cleanup: bool,
}
impl Direct {
    pub fn open(route: driver::Route, cancel: Arc<Cancellation>, manager: Arc<Mutex<State>>) -> Result<Self> {
        let acceleration = manager.lock().map_err(|_| anyhow::anyhow!("Mapping manager poisoned"))?.acceleration.clone();
        Ok(Self { input: input::Input::open(Some(&route), cancel.clone(), acceleration)?,
            output: route.output(), route, keyboard: KeyboardExecutor::new(timing()),
            digital: NativeDigitalMappingState::default(), identities: BTreeMap::new(), analog: BTreeMap::new(), next_identity: 0,
            cancel, manager, epoch: Instant::now(), cleanup: false })
    }
    fn tick(&self) -> u64 { self.epoch.elapsed().as_micros().try_into().unwrap_or(u64::MAX) }
    fn identity(&mut self, key: &InputIdentity) -> String {
        let id = self.identities.entry(key.clone()).or_insert_with(|| {
            self.next_identity += 1;
            self.next_identity
        });
        format!("input:{id}")
    }
    pub fn poll(&mut self) -> Vec<String> {
        let now = self.tick();
        self.keyboard.poll(now, &mut |key| self.input.keyboard(key))
    }
    pub fn next_wait(&self) -> Duration {
        self.keyboard.next_deadline_us().map_or(Duration::from_millis(4),
            |deadline| Duration::from_micros(deadline.saturating_sub(self.tick())).min(Duration::from_millis(4)))
    }
    pub fn native(&mut self, input: NativeInput) -> Result<()> { self.input.send(input) }
    pub fn cleanup(&mut self, cleanup: bool) { self.cleanup = cleanup; self.input.cancellable = !cleanup; }
    pub fn action(&mut self, action: razer_device::macro_runtime::Action) -> Result<()> {
        use razer_device::macro_runtime::Action;
        match action {
            Action::Keyboard(key) => self.keyboard(key, false),
            Action::Mouse(mouse) => self.native(NativeInput::Mouse(mouse.input())),
            Action::MouseMove {mode,x,y,origin_epoch} => self.input.movement(mode,x,y,origin_epoch),
            Action::Launch(action) => self.event(&Event::Launch(action),false),
            Action::Clipboard(text) => self.event(&Event::Clipboard(text),false),
            Action::Audio(action) => self.event(&Event::Audio(action),false),
        }
    }
    pub fn keyboard(&mut self, key: Keyboard, smart_stop: bool) -> Result<()> {
        self.keyboard.execute(key, smart_stop, self.tick(), &mut |key| self.input.keyboard(key)).map_err(anyhow::Error::msg)
    }
    pub fn event(&mut self, event: &Event, smart_stop: bool) -> Result<()> {
        match event {
            Event::Keyboard(key) => self.keyboard(key.clone(), smart_stop),
            Event::Mouse(mouse) => self.native(NativeInput::Mouse(mouse.input())),
            Event::Clipboard(text) => {
                let client = self.manager.lock().map_err(|_| anyhow::anyhow!("Mapping manager poisoned"))?.clipboard_client()?;
                client.paste(text.clone(), Some(self.route.clone()))
            }
            Event::Launch(action) => launch::execute(action),
            Event::Audio(action) => {
                let report = audio::execute(*action)?;
                ensure!(report.errors.is_empty(), "{}", report.errors.join("; "));
                Ok(())
            }
            Event::Delay { milliseconds } => self.wait(*milliseconds),
            Event::RandomDelay { min_ms, max_ms } => {
                if min_ms > max_ms { return Ok(()); }
                self.wait(super::random(*min_ms, *max_ms)?)
            }
            Event::MouseMove { .. } | Event::LoopStart { .. } | Event::LoopEnd | Event::Macro { .. } => {
                anyhow::bail!("Macro-only event reached the direct primitive executor")
            }
        }
    }
    fn wait(&self, milliseconds: u32) -> Result<()> {
        self.cancel.wait(Duration::from_millis(milliseconds.into()));
        ensure!(!self.cancel.is_cancelled(), "Mapping sequence was canceled");
        Ok(())
    }
    pub fn digital(&mut self, key: &InputIdentity, output: NativeDigitalOutput) -> Result<()> {
        let identity = self.identity(key);
        let delta = self.digital.apply_digital(&identity, output)?;
        self.delta(delta)
    }
    pub fn combination(&mut self, key: &str, output: NativeDigitalOutput) -> Result<()> {
        let delta = self.digital.apply_digital(&format!("combination:{key}"), output)?;
        self.delta(delta)
    }
    pub fn analog(&mut self, key: &InputIdentity, output: &NativeAnalogOutput, actuation: u16) -> Result<()> {
        let identity = self.identity(key);
        if actuation != 0 { self.analog.insert(key.clone(), output.clone()); }
        else { self.analog.remove(key); }
        let delta = self.digital.apply_analog(&identity, output, actuation);
        self.delta(delta)
    }
    pub fn stop_keyboard(&mut self) { self.keyboard.stop(); }
    pub fn release_held(&mut self) -> Result<()> {
        self.cleanup(true);
        let mut errors=Vec::new();
        if let Err(error)=self.input.release_held(){errors.push(format!("{error:#}"));}
        for (key,output) in std::mem::take(&mut self.analog) {
            if let Err(error)=self.analog(&key,&output,0){errors.push(format!("Analog raw-zero release: {error:#}"));}
        }
        ensure!(errors.is_empty(),"{}",errors.join("; "));Ok(())
    }
    fn delta(&mut self, delta: NativeMappingDelta) -> Result<()> {
        let mut errors = Vec::new();
        for report in delta.encoded_reports() {
            if let Err(error) = self.report(report) { errors.push(format!("{error:#}")); }
        }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
    pub fn report(&mut self, report: DriverInputReport) -> Result<()> {
        let id = self.output.submit(report)?;
        let mut canceled = false;
        loop {
            if !canceled && !self.cleanup && self.cancel.is_cancelled() {
                self.output.cancel(id)?;
                canceled = true;
            }
            match self.output.poll_completion(id, Duration::from_millis(5))? {
                CompletionPoll::Pending => {}
                CompletionPoll::Completed(result) => return result.map_err(Into::into),
            }
        }
    }
    pub fn close(&mut self) -> Result<()> {
        self.keyboard.stop();
        let mut errors = Vec::new();
        if let Err(error) = self.release_held() { errors.push(format!("{error:#}")); }
        if let Err(error) = self.input.close() { errors.push(format!("{error:#}")); }
        if let Err(error) = self.output.close() { errors.push(error.to_string()); }
        ensure!(errors.is_empty(), "{}", errors.join("; "));
        Ok(())
    }
}
impl Drop for Direct {
    fn drop(&mut self) { if let Err(error) = self.close() { eprintln!("Mapping direct-output cleanup: {error:#}"); } }
}
