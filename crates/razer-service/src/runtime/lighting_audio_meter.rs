//! 4.0.55.0 Audio VU (0x5f0b0/0x5f3e0) observes the default console
//! endpoint's IAudioMeterInformation. COM ownership stays on the lighting worker.
use anyhow::{Context as _, ensure};
use std::{
    marker::PhantomData,
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use windows::{
    Win32::{
        Foundation::PROPERTYKEY,
        Media::Audio::{
            DEVICE_STATE, EDataFlow, ERole, Endpoints::IAudioMeterInformation, IMMDeviceEnumerator,
            IMMNotificationClient, IMMNotificationClient_Impl, MMDeviceEnumerator, eCapture,
            eConsole, eRender,
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
    fn open() -> anyhow::Result<Self> {
        // SAFETY: called and balanced on the retained lighting worker only.
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok() }
            .context("Initialize lighting audio COM apartment")?;
        Ok(Self(PhantomData))
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

#[implement(IMMNotificationClient)]
struct Notification {
    dirty: Arc<AtomicBool>,
}
#[allow(non_snake_case)]
impl IMMNotificationClient_Impl for Notification_Impl {
    fn OnDeviceStateChanged(&self, _: &PCWSTR, _: DEVICE_STATE) -> windows::core::Result<()> {
        self.dirty.store(true, Ordering::Release);
        Ok(())
    }
    fn OnDeviceAdded(&self, _: &PCWSTR) -> windows::core::Result<()> {
        self.dirty.store(true, Ordering::Release);
        Ok(())
    }
    fn OnDeviceRemoved(&self, _: &PCWSTR) -> windows::core::Result<()> {
        self.dirty.store(true, Ordering::Release);
        Ok(())
    }
    fn OnDefaultDeviceChanged(
        &self,
        _: EDataFlow,
        role: ERole,
        _: &PCWSTR,
    ) -> windows::core::Result<()> {
        if role == eConsole {
            self.dirty.store(true, Ordering::Release);
        }
        Ok(())
    }
    fn OnPropertyValueChanged(&self, _: &PCWSTR, _: &PROPERTYKEY) -> windows::core::Result<()> {
        Ok(())
    }
}
struct Endpoint {
    id: String,
    meter: IAudioMeterInformation,
}
pub(super) struct Meter {
    endpoints: [Option<Endpoint>; 2],
    enumerator: IMMDeviceEnumerator,
    callback: Option<IMMNotificationClient>,
    dirty: Arc<AtomicBool>,
    checked: Instant,
    // Field order releases all COM interfaces before apartment shutdown.
    _apartment: Apartment,
}
impl Meter {
    pub(super) fn open() -> anyhow::Result<Self> {
        let apartment = Apartment::open()?;
        // SAFETY: verified system MMDeviceEnumerator CLSID and typed interface,
        // with a live apartment retained below for every call and release.
        let enumerator: IMMDeviceEnumerator =
            unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
                .context("Create lighting audio endpoint enumerator")?;
        let dirty = Arc::new(AtomicBool::new(true));
        let callback: IMMNotificationClient = Notification {
            dirty: dirty.clone(),
        }
        .into();
        unsafe { enumerator.RegisterEndpointNotificationCallback(&callback) }
            .context("Register lighting audio endpoint notifications")?;
        Ok(Self {
            endpoints: [None, None],
            enumerator,
            callback: Some(callback),
            dirty,
            checked: Instant::now(),
            _apartment: apartment,
        })
    }
    pub(super) fn peaks(&mut self, flows: [bool; 2]) -> anyhow::Result<[Option<f32>; 2]> {
        let refresh = self.dirty.swap(false, Ordering::AcqRel)
            || self.checked.elapsed() >= Duration::from_secs(1);
        let mut peaks = [None, None];
        for flow in 0..2 {
            if !flows[flow] {
                self.endpoints[flow] = None;
                continue;
            }
            if refresh || self.endpoints[flow].is_none() {
                // SAFETY: owner apartment is live and default endpoint identity
                // is rechecked after notifications and at bounded intervals.
                let device = unsafe {
                    self.enumerator.GetDefaultAudioEndpoint(
                        if flow == 1 { eCapture } else { eRender },
                        eConsole,
                    )
                }
                .context("Read default lighting audio endpoint")?;
                let pointer =
                    unsafe { device.GetId() }.context("Read lighting audio endpoint identity")?;
                let id = unsafe { pointer.to_string() };
                unsafe { CoTaskMemFree(Some(pointer.0.cast())) };
                let id = id.context("Lighting audio endpoint identity is not UTF-16")?;
                if !self.endpoints[flow]
                    .as_ref()
                    .is_some_and(|endpoint| endpoint.id == id)
                {
                    self.endpoints[flow] = None;
                    let meter =
                        unsafe { device.Activate::<IAudioMeterInformation>(CLSCTX_ALL, None) }
                            .context("Activate lighting audio peak meter")?;
                    self.endpoints[flow] = Some(Endpoint { id, meter });
                }
            }
            let endpoint = self.endpoints[flow]
                .as_ref()
                .context("Lighting audio endpoint was released")?;
            let observed =
                unsafe { endpoint.meter.GetPeakValue() }.context("Read lighting audio peak")?;
            ensure!(
                observed.is_finite() && (0.0..=1.0).contains(&observed),
                "Lighting audio peak is outside the system contract"
            );
            peaks[flow] = Some(observed);
        }
        if refresh {
            self.checked = Instant::now();
        }
        Ok(peaks)
    }
    pub(super) fn close(&mut self) -> anyhow::Result<()> {
        if let Some(callback) = self.callback.as_ref() {
            // SAFETY: registration uses this same retained callback. It only
            // owns an Arc flag, so an in-flight invocation cannot borrow us.
            unsafe {
                self.enumerator
                    .UnregisterEndpointNotificationCallback(callback)
            }
            .context("Unregister lighting audio endpoint notifications")?;
            self.callback = None;
        }
        self.endpoints = [None, None];
        Ok(())
    }
}
impl Drop for Meter {
    fn drop(&mut self) {
        let _ = self.close();
    }
}
