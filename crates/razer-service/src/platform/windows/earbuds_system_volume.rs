//! Mapping AudioService's default-render monitor, implemented with system COM.
use super::{Backend, Notifications};
use anyhow::{Context as _, Result};
use std::{
    marker::PhantomData,
    rc::Rc,
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use windows::{
    Win32::{
        Foundation::PROPERTYKEY,
        Media::Audio::{
            AUDIO_VOLUME_NOTIFICATION_DATA, DEVICE_STATE, EDataFlow, ERole,
            Endpoints::{
                IAudioEndpointVolume, IAudioEndpointVolumeCallback,
                IAudioEndpointVolumeCallback_Impl,
            },
            IMMDeviceEnumerator, IMMNotificationClient, IMMNotificationClient_Impl,
            MMDeviceEnumerator, eMultimedia, eRender,
        },
        System::Com::{
            CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx, CoTaskMemFree,
            CoUninitialize,
        },
    },
    core::{PCWSTR, implement},
};

struct Apartment(PhantomData<Rc<()>>);
impl Apartment {
    fn open() -> Result<Self> {
        // All COM work and release occur in the service session's one thread.
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() }
            .context("Initialize earbud volume COM")?;
        Ok(Self(PhantomData))
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

#[derive(Default)]
struct Dirty {
    speaker: AtomicBool,
    default_device: AtomicBool,
    topology: AtomicBool,
}
struct Activity {
    state: Mutex<(bool, usize)>,
    drained: Condvar,
}
impl Activity {
    fn new() -> Self {
        Self {
            state: Mutex::new((true, 0)),
            drained: Condvar::new(),
        }
    }
    fn enter(&self) -> Option<Call<'_>> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        if !state.0 {
            return None;
        }
        state.1 += 1;
        Some(Call(self))
    }
    fn stop(&self) {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).0 = false;
    }
    fn wait(&self) {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        while state.1 != 0 {
            state = self.drained.wait(state).unwrap_or_else(|e| e.into_inner());
        }
    }
}
struct Call<'a>(&'a Activity);
impl Drop for Call<'_> {
    fn drop(&mut self) {
        let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        state.1 -= 1;
        if state.1 == 0 {
            self.0.drained.notify_all();
        }
    }
}

#[implement(IAudioEndpointVolumeCallback)]
struct VolumeNotice {
    dirty: Arc<Dirty>,
    activity: Arc<Activity>,
}
#[allow(non_snake_case)]
impl IAudioEndpointVolumeCallback_Impl for VolumeNotice_Impl {
    fn OnNotify(&self, _: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> windows::core::Result<()> {
        let Some(_call) = self.activity.enter() else {
            return Ok(());
        };
        // No borrowed notification pointer crosses this callback. The session
        // re-reads its product endpoint as audioBasicFeature's consumer does.
        self.dirty.speaker.store(true, Ordering::Release);
        Ok(())
    }
}
#[implement(IMMNotificationClient)]
struct DeviceNotice {
    dirty: Arc<Dirty>,
    activity: Arc<Activity>,
}
#[allow(non_snake_case)]
impl IMMNotificationClient_Impl for DeviceNotice_Impl {
    fn OnDeviceStateChanged(&self, _: &PCWSTR, _: DEVICE_STATE) -> windows::core::Result<()> {
        if let Some(_call) = self.activity.enter() {
            self.dirty.topology.store(true, Ordering::Release);
        }
        Ok(())
    }
    fn OnDeviceAdded(&self, _: &PCWSTR) -> windows::core::Result<()> {
        if let Some(_call) = self.activity.enter() {
            self.dirty.topology.store(true, Ordering::Release);
        }
        Ok(())
    }
    fn OnDeviceRemoved(&self, _: &PCWSTR) -> windows::core::Result<()> {
        if let Some(_call) = self.activity.enter() {
            self.dirty.topology.store(true, Ordering::Release);
        }
        Ok(())
    }
    fn OnDefaultDeviceChanged(
        &self,
        flow: EDataFlow,
        _: ERole,
        _: &PCWSTR,
    ) -> windows::core::Result<()> {
        if let Some(_call) = self.activity.enter() {
            // Current b6710 checks flow, ignores the notified role, and then
            // b5ff2 resolves the current eMultimedia default.
            if flow == eRender {
                self.dirty.default_device.store(true, Ordering::Release);
            }
        }
        Ok(())
    }
    fn OnPropertyValueChanged(&self, _: &PCWSTR, _: &PROPERTYKEY) -> windows::core::Result<()> {
        Ok(())
    }
}

struct DefaultEndpoint {
    id: String,
    volume: IAudioEndpointVolume,
    callback: IAudioEndpointVolumeCallback,
    activity: Arc<Activity>,
}
pub(super) struct Monitor {
    endpoint: Option<DefaultEndpoint>,
    enumerator: IMMDeviceEnumerator,
    callback: Option<IMMNotificationClient>,
    activity: Arc<Activity>,
    dirty: Arc<Dirty>,
    _apartment: Apartment,
}
impl Monitor {
    pub(super) fn open() -> Result<Self> {
        let apartment = Apartment::open()?;
        // Typed bindings target the system MMDeviceEnumerator; no business DLL.
        let enumerator: IMMDeviceEnumerator =
            unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
                .context("Create volume endpoint enumerator")?;
        let dirty = Arc::new(Dirty::default());
        let activity = Arc::new(Activity::new());
        let callback: IMMNotificationClient = DeviceNotice {
            dirty: dirty.clone(),
            activity: activity.clone(),
        }
        .into();
        unsafe { enumerator.RegisterEndpointNotificationCallback(&callback) }
            .context("Register volume endpoint notifications")?;
        dirty.default_device.store(true, Ordering::Release);
        Ok(Self {
            endpoint: None,
            enumerator,
            callback: Some(callback),
            activity,
            dirty,
            _apartment: apartment,
        })
    }
    fn close_endpoint(&mut self) -> Result<()> {
        if let Some(endpoint) = &self.endpoint {
            endpoint.activity.stop();
            unsafe {
                endpoint
                    .volume
                    .UnregisterControlChangeNotify(&endpoint.callback)
            }
            .context("Unregister speaker volume callback")?;
            endpoint.activity.wait();
            self.endpoint = None;
        }
        Ok(())
    }
    fn rebind(&mut self) -> Result<Option<String>> {
        let device = match unsafe {
            self.enumerator
                .GetDefaultAudioEndpoint(eRender, eMultimedia)
        } {
            Ok(device) => device,
            Err(error) if error.code().0 as u32 == 0x80070490 => {
                self.close_endpoint()?;
                return Ok(None);
            }
            Err(error) => return Err(error).context("Resolve default multimedia speaker"),
        };
        let pointer = unsafe { device.GetId() }.context("Read default speaker ID")?;
        let id = unsafe { pointer.to_string() };
        unsafe { CoTaskMemFree(Some(pointer.0.cast())) };
        let id = id.context("Default speaker ID is not UTF-16")?;
        if self
            .endpoint
            .as_ref()
            .is_some_and(|endpoint| endpoint.id == id)
        {
            return Ok(None);
        }
        self.close_endpoint()?;
        let volume = unsafe { device.Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None) }
            .context("Activate default speaker volume")?;
        let activity = Arc::new(Activity::new());
        let callback: IAudioEndpointVolumeCallback = VolumeNotice {
            dirty: self.dirty.clone(),
            activity: activity.clone(),
        }
        .into();
        unsafe { volume.RegisterControlChangeNotify(&callback) }
            .context("Register default speaker volume callback")?;
        self.endpoint = Some(DefaultEndpoint {
            id: id.clone(),
            volume,
            callback,
            activity,
        });
        Ok(Some(id))
    }
}
impl Backend for Monitor {
    fn enumerate(&mut self) -> Result<Vec<razer_device::simple_audio::SimpleAudioEndpoint>> {
        crate::simple_audio::enumerate()
    }
    fn read(
        &mut self,
        id: &str,
    ) -> Result<razer_device::simple_audio_volume::AudioVolumeReadResult> {
        crate::simple_audio_volume::read(id)
    }
    fn write(
        &mut self,
        id: &str,
        mute: bool,
        volume: u8,
    ) -> Result<razer_device::simple_audio_volume::AudioVolumeWriteResult> {
        crate::simple_audio_volume::write(id, mute, volume)
    }
    fn notifications(&mut self) -> Result<Notifications> {
        let topology = self.dirty.topology.swap(false, Ordering::AcqRel);
        let default_endpoint =
            if self.dirty.default_device.swap(false, Ordering::AcqRel) || topology {
                self.rebind()?
            } else {
                None
            };
        Ok(Notifications {
            speaker: self.dirty.speaker.swap(false, Ordering::AcqRel),
            default_endpoint,
            topology,
        })
    }
    fn close(&mut self) -> Result<()> {
        self.close_endpoint()?;
        if let Some(callback) = &self.callback {
            self.activity.stop();
            unsafe {
                self.enumerator
                    .UnregisterEndpointNotificationCallback(callback)
            }
            .context("Unregister volume endpoint notifications")?;
            self.activity.wait();
            self.callback = None;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Media::Audio::{eCapture, eCommunications, eConsole};

    #[test]
    fn stopped_com_callbacks_ignore_late_notifications_and_render_roles_share_one_refresh() {
        let dirty = Arc::new(Dirty::default());
        let activity = Arc::new(Activity::new());
        let volume: IAudioEndpointVolumeCallback = VolumeNotice {
            dirty: dirty.clone(),
            activity: activity.clone(),
        }
        .into();
        let device: IMMNotificationClient = DeviceNotice {
            dirty: dirty.clone(),
            activity: activity.clone(),
        }
        .into();
        unsafe {
            device
                .OnDefaultDeviceChanged(eCapture, eConsole, PCWSTR::null())
                .unwrap();
        }
        assert!(!dirty.default_device.load(Ordering::Acquire));
        for role in [eConsole, eMultimedia, eCommunications] {
            unsafe {
                device
                    .OnDefaultDeviceChanged(eRender, role, PCWSTR::null())
                    .unwrap();
            }
            assert!(dirty.default_device.swap(false, Ordering::AcqRel));
        }
        unsafe {
            volume.OnNotify(std::ptr::null_mut()).unwrap();
        }
        assert!(dirty.speaker.swap(false, Ordering::AcqRel));
        let in_flight = activity.enter().unwrap();
        activity.stop();
        assert!(activity.enter().is_none());
        unsafe {
            volume.OnNotify(std::ptr::null_mut()).unwrap();
            device
                .OnDefaultDeviceChanged(eRender, eMultimedia, PCWSTR::null())
                .unwrap();
        }
        assert!(!dirty.speaker.load(Ordering::Acquire));
        assert!(!dirty.default_device.load(Ordering::Acquire));
        assert_eq!(activity.state.lock().unwrap().1, 1);
        drop(in_flight);
        activity.wait();
        assert_eq!(activity.state.lock().unwrap().1, 0);
    }
}
