//! Current AudioEffectsTHXCarol ConfigureFFI table for the 1392/1442/3942
//! THX VAD native resources. Loaded only in the isolated service process.
//! Development verification never loads the DLL or calls its exports.
use crate::dll::EngineLibrary;
use anyhow::{Context as _, ensure};
use razer_device::audio_surround::{
    self as surround, Change, Channel, Identity, NativeCall, Outcome, Product, Runtime, Transport,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    ffi::{CStr, CString, c_char, c_void},
    path::PathBuf,
};

// ffi table: bool(string), bool(string,bool), bool(string,float),
// bool(string,string), pointer(), void(pointer), bool(pointer). All three
// current Hex-Rays captures take a narrow char* container in RCX, the value in
// DL/XMM1/RDX and return the C++ bool in AL.
type Owner = unsafe extern "C" fn(*const c_char) -> u8;
type Flag = unsafe extern "C" fn(*const c_char, u8) -> u8;
type Level = unsafe extern "C" fn(*const c_char, f32) -> u8;
type Text = unsafe extern "C" fn(*const c_char, *const c_char) -> u8;
type Version = unsafe extern "C" fn() -> *mut c_char;
type FreeMalloc = unsafe extern "C" fn(*mut c_void);
type Event = unsafe extern "C" fn(*const c_void) -> u8;

/// The current wrapper consumes these callbacks only as logged notify events;
/// the source never feeds them into combo state. The borrowed buffer is not
/// retained or freed here.
unsafe extern "C" fn native_event(_pointer: *const c_char) {}

struct Session {
    _library: EngineLibrary,
    owner: CString,
    terminate: Owner,
    set_combo: Flag,
    set_front_endpoint: Text,
    set_front_volume: Level,
    set_rear_volume: Level,
    set_rear_mute: Flag,
    set_front_eq: Text,
    set_rear_eq: Text,
    set_delay: Level,
    set_replicate: Flag,
    play_signal: Owner,
    stop_signal: Owner,
    version: Option<String>,
    initialized: bool,
}

fn resource(product: Product) -> anyhow::Result<PathBuf> {
    let library = razer_catalog::native_library::find(product.library_id())
        .context("Current THX VAD native resource declaration is absent")?;
    let candidates =
        razer_platform::native_paths::resource_candidates(library, Some(u32::from(product)));
    let present = candidates
        .into_iter()
        .map(|(_, path)| path)
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();
    ensure!(
        present.len() == 1,
        "Installed THX VAD native resource is absent or ambiguous"
    );
    Ok(present.into_iter().next().unwrap())
}

impl Session {
    fn open(identity: &Identity) -> anyhow::Result<Self> {
        let owner = CString::new(identity.container_id())
            .context("Surround container contains a NUL")?;
        let library = EngineLibrary::load(&resource(identity.product())?)?;
        // SAFETY: names and signatures come from the current ConfigureFFI
        // table and match all three retained Hex-Rays export prototypes.
        let session = unsafe {
            let initialize = library
                .func::<Owner>("DeviceInitialize")
                .context("THX VAD DeviceInitialize export is absent")?;
            let event = library
                .func::<Event>("SetNodeFFIEvent")
                .context("THX VAD SetNodeFFIEvent export is absent")?;
            let get_version = library
                .func::<Version>("GetDLLVersion")
                .context("THX VAD GetDLLVersion export is absent")?;
            let free = library
                .func::<FreeMalloc>("FreeMalloc")
                .context("THX VAD FreeMalloc export is absent")?;
            let mut session = Self {
                terminate: library.func("DeviceTerminate").context("DeviceTerminate absent")?,
                set_combo: library.func("SetComboMode").context("SetComboMode absent")?,
                set_front_endpoint: library
                    .func("SetFrontEndpoint")
                    .context("SetFrontEndpoint absent")?,
                set_front_volume: library.func("SetFrontVolume").context("SetFrontVolume absent")?,
                set_rear_volume: library.func("SetRearVolume").context("SetRearVolume absent")?,
                set_rear_mute: library.func("SetRearMute").context("SetRearMute absent")?,
                set_front_eq: library.func("SetFrontEqBand").context("SetFrontEqBand absent")?,
                set_rear_eq: library.func("SetRearEqBand").context("SetRearEqBand absent")?,
                set_delay: library.func("SetComboDelay").context("SetComboDelay absent")?,
                set_replicate: library.func("SetReplicate").context("SetReplicate absent")?,
                play_signal: library
                    .func("PlayDelayTestSignal")
                    .context("PlayDelayTestSignal absent")?,
                stop_signal: library
                    .func("StopDelayTestSignal")
                    .context("StopDelayTestSignal absent")?,
                version: None,
                initialized: false,
                owner,
                _library: library,
            };
            // initElectron: GetDLLVersion, SetNodeFFIEvent, DeviceInitialize.
            let pointer = get_version();
            if !pointer.is_null() {
                session.version = CStr::from_ptr(pointer).to_str().ok().map(str::to_owned);
                free(pointer.cast());
            }
            if event(native_event as *const () as *const c_void) == 0 {
                eprintln!("[THX VAD] SetNodeFFIEvent returned false");
            }
            session.initialized = initialize(session.owner.as_ptr()) != 0;
            session
        };
        Ok(session)
    }
}

impl Transport for Session {
    fn call(&mut self, call: &NativeCall) -> anyhow::Result<bool> {
        let owner = self.owner.as_ptr();
        let band = |channel: Channel, equalizer: surround::Equalizer| {
            // setFrontEqBand/setRearEqBand pass JSON.stringify(bands).
            CString::new(serde_json::to_string(&equalizer.bands())?)
                .map(|json| (channel, json))
                .map_err(anyhow::Error::from)
        };
        // SAFETY: exports were resolved from this live library; owner and any
        // argument CString stay alive for the synchronous call.
        let accepted = unsafe {
            match call {
                NativeCall::SetEnabled(enabled) => (self.set_combo)(owner, u8::from(*enabled)),
                NativeCall::SetFrontEndpoint(id) => {
                    let id = CString::new(id.as_str()).context("Endpoint id contains a NUL")?;
                    (self.set_front_endpoint)(owner, id.as_ptr())
                }
                NativeCall::SetVolume { channel, volume } => match channel {
                    Channel::Front => (self.set_front_volume)(owner, volume.native_value()),
                    Channel::Rear => (self.set_rear_volume)(owner, volume.native_value()),
                },
                NativeCall::SetRearMute(muted) => (self.set_rear_mute)(owner, u8::from(*muted)),
                NativeCall::SetEqualizer { channel, equalizer } => {
                    let (channel, json) = band(*channel, *equalizer)?;
                    match channel {
                        Channel::Front => (self.set_front_eq)(owner, json.as_ptr()),
                        Channel::Rear => (self.set_rear_eq)(owner, json.as_ptr()),
                    }
                }
                NativeCall::SetDelay(delay) => (self.set_delay)(owner, delay.native_value()),
                NativeCall::SetReplicate(enabled) => {
                    (self.set_replicate)(owner, u8::from(*enabled))
                }
                NativeCall::SetTestSignal(true) => (self.play_signal)(owner),
                NativeCall::SetTestSignal(false) => (self.stop_signal)(owner),
            }
        };
        Ok(accepted != 0)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if self.initialized {
            // SAFETY: the library outlives this call; the owner is still valid.
            unsafe {
                (self.terminate)(self.owner.as_ptr());
            }
        }
    }
}

struct Owned {
    identity: Identity,
    session: Session,
    runtime: Runtime,
}

/// One native session per ContainerId. A generation change retires the old
/// owner before a new library session is created.
#[derive(Default)]
pub(crate) struct State {
    sessions: BTreeMap<String, Owned>,
}

impl State {
    fn key(identity: &Identity) -> String {
        identity.container_id().to_ascii_lowercase()
    }
    pub(crate) fn open(
        &mut self,
        identity: &Identity,
        settings: &surround::Settings,
        validate: impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Value> {
        validate()?;
        let key = Self::key(identity);
        if let Some(owned) = self.sessions.get(&key) {
            if owned.identity == *identity {
                return Ok(json!({"identity":identity,"initialized":owned.session.initialized,
                    "version":owned.session.version,"retained":true,"setting_scope":"surround"}));
            }
            self.sessions.remove(&key);
        }
        let session = Session::open(identity)?;
        let initialized = session.initialized;
        let version = session.version.clone();
        self.sessions.insert(
            key,
            Owned {
                identity: identity.clone(),
                session,
                runtime: Runtime::with_settings(settings.clone()),
            },
        );
        validate()?;
        Ok(json!({"identity":identity,"initialized":initialized,"version":version,
            "retained":false,"setting_scope":"surround"}))
    }
    pub(crate) fn change(
        &mut self,
        identity: &Identity,
        change: &Change,
        validate: impl Fn() -> anyhow::Result<()>,
    ) -> anyhow::Result<Value> {
        let owned = self
            .sessions
            .get_mut(&Self::key(identity))
            .filter(|owned| owned.identity == *identity)
            .context("Surround native owner is not open for this generation")?;
        let mut outcomes: Vec<Outcome> = Vec::new();
        let mut queue = vec![change.clone()];
        // eo starts Xn without awaiting it; the follow-up runs after the
        // endpoint task in this serialized owner, never before it.
        while let Some(next) = queue.pop() {
            let outcome = owned.runtime.execute(&next, &mut owned.session, &validate);
            queue.extend(outcome.follow_ups().iter().rev().cloned());
            let stop = !outcome.is_completed();
            outcomes.push(outcome);
            if stop {
                break;
            }
        }
        Ok(json!({"identity":identity,"requested":change,"outcomes":outcomes,
            "runtime":owned.runtime,"setting_scope":"surround"}))
    }
    pub(crate) fn close(&mut self, identity: &Identity) -> anyhow::Result<Value> {
        let key = Self::key(identity);
        let current = self
            .sessions
            .get(&key)
            .is_some_and(|owned| owned.identity == *identity);
        let mut stopped = None;
        if let Some(mut owned) = current.then(|| self.sessions.remove(&key)).flatten() {
            // Current cleanup sends setComboDelayTestSignal(false) on Carol
            // teardown before the library owner is released.
            stopped = Some(owned.session.call(&NativeCall::SetTestSignal(false))?);
        }
        Ok(json!({"identity":identity,"closed":current,"test_signal_stopped":stopped,
            "setting_scope":"surround"}))
    }
}
