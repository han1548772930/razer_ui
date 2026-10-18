//! Mapping display operations: driver input or local WMI brightness service.
use super::driver;
use anyhow::{Context, Result, ensure};
use razer_device::{
    mapping_native_action::DisplayAction,
    mapping_native_input::{DisplayInput, encode_display},
    mapping_native_output::{CompletionPoll, MappingOutputTransport},
};
use std::time::Duration;
use windows::{
    Win32::System::{
        Com::{
            CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
            CoSetProxyBlanket, CoUninitialize, EOAC_NONE, RPC_C_AUTHN_LEVEL_PKT,
            RPC_C_IMP_LEVEL_IMPERSONATE,
        },
        Ole::{SafeArrayGetDim, SafeArrayGetElement, SafeArrayGetLBound, SafeArrayGetUBound},
        Variant::{VARIANT, VT_ARRAY, VT_BSTR, VT_UI1, VariantClear},
        Wmi::{
            IWbemClassObject, IWbemLocator, IWbemServices, WBEM_FLAG_FORWARD_ONLY,
            WBEM_FLAG_RETURN_IMMEDIATELY, WBEM_GENERIC_FLAG_TYPE, WbemLocator,
        },
    },
    core::{BSTR, PCWSTR, w},
};

pub(super) struct Display {
    output: Option<driver::Output>,
    closed: bool,
}
impl Display {
    pub fn open(route: Option<driver::Route>) -> Result<Self> {
        Ok(Self {
            output: route.map(|route| route.output()),
            closed: false,
        })
    }
    pub fn execute(&mut self, action: DisplayAction) -> Result<()> {
        ensure!(!self.closed, "Mapping display owner is closed");
        let input = match action {
            DisplayAction::DriverBrightnessDown => Some(DisplayInput::SourceId3),
            DisplayAction::DriverBrightnessUp => Some(DisplayInput::SourceId4),
            DisplayAction::DriverBrightnessStop => Some(DisplayInput::SourceId5),
            _ => None,
        };
        if let Some(input) = input {
            // The native failure callback has no OS display-input conversion.
            let output = self
                .output
                .as_mut()
                .context("Display mapping has no filter-driver route")?;
            let id = output.submit(encode_display(input))?;
            loop {
                match output.poll_completion(id, Duration::from_millis(16))? {
                    CompletionPoll::Pending => {}
                    CompletionPoll::Completed(result) => return result.map_err(Into::into),
                }
            }
        }
        let _apartment = Apartment::open()?;
        let service = connect()?;
        let state = first(&service, "SELECT * FROM WmiMonitorBrightness")?;
        let levels = property(&state, w!("Level"))?.levels()?;
        let current = match action {
            DisplayAction::Brightness(_) => None,
            _ => Some(property(&state, w!("CurrentBrightness"))?.byte()?),
        };
        let value = target(action, &levels, current)?;
        set_brightness(&service, value)
    }
    pub fn close(&mut self) -> Result<()> {
        if self.closed {
            return Ok(());
        }
        if let Some(output) = &mut self.output {
            output.close()?;
        }
        self.closed = true;
        Ok(())
    }
}

fn target(action: DisplayAction, levels: &[u8], current: Option<u8>) -> Result<u8> {
    let first = *levels.first().context("WMI brightness levels are empty")?;
    let last = *levels.last().unwrap();
    Ok(match action {
        // RVA 0x1321d0 clamps only the upper endpoint for absolute writes.
        DisplayAction::Brightness(value) => value.min(last),
        DisplayAction::BrightnessDown(step) => {
            let value = i16::from(current.context("Current brightness was not observed")?)
                - i16::from(step);
            value.max(i16::from(first)) as u8
        }
        DisplayAction::BrightnessUp(step) => {
            let value = u16::from(current.context("Current brightness was not observed")?)
                + u16::from(step);
            value.min(u16::from(last)) as u8
        }
        _ => anyhow::bail!("Driver display input reached WMI brightness calculation"),
    })
}

struct Apartment(bool);
impl Apartment {
    fn open() -> Result<Self> {
        let result = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
        if result.0 == 0x80010106u32 as i32 {
            return Ok(Self(false));
        }
        result.ok()?;
        Ok(Self(true))
    }
}
impl Drop for Apartment {
    fn drop(&mut self) {
        if self.0 {
            unsafe { CoUninitialize() };
        }
    }
}
struct Property(VARIANT);
impl Drop for Property {
    fn drop(&mut self) {
        let _ = unsafe { VariantClear(&mut self.0) };
    }
}
impl Property {
    fn byte(&self) -> Result<u8> {
        let inner = unsafe { &self.0.Anonymous.Anonymous };
        ensure!(inner.vt == VT_UI1, "WMI brightness is not uint8");
        Ok(unsafe { inner.Anonymous.bVal })
    }
    fn string(&self) -> Result<String> {
        let inner = unsafe { &self.0.Anonymous.Anonymous };
        ensure!(inner.vt == VT_BSTR, "WMI path is not a string");
        Ok(String::from_utf16(unsafe { &inner.Anonymous.bstrVal })?)
    }
    fn levels(&self) -> Result<Vec<u8>> {
        let inner = unsafe { &self.0.Anonymous.Anonymous };
        ensure!(
            inner.vt.0 == VT_ARRAY.0 | VT_UI1.0,
            "WMI brightness levels are not a byte array"
        );
        let array = unsafe { inner.Anonymous.parray };
        ensure!(!array.is_null(), "WMI brightness array is null");
        unsafe {
            ensure!(
                SafeArrayGetDim(array) == 1,
                "WMI brightness array is not one dimensional"
            );
            let lower = SafeArrayGetLBound(array, 1)?;
            let upper = SafeArrayGetUBound(array, 1)?;
            ensure!(
                lower == 0 && upper > 0,
                "WMI brightness level bounds differ from the source contract"
            );
            let mut values = Vec::new();
            for index in lower..=upper {
                let mut value = 0u8;
                SafeArrayGetElement(array, &index, (&mut value as *mut u8).cast())?;
                values.push(value);
            }
            Ok(values)
        }
    }
}
fn property(object: &IWbemClassObject, name: PCWSTR) -> Result<Property> {
    let mut value = Property(VARIANT::default());
    unsafe {
        object.Get(name, 0, &mut value.0, None, None)?;
    }
    Ok(value)
}
fn connect() -> Result<IWbemServices> {
    unsafe {
        let locator: IWbemLocator = CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER)?;
        let empty = BSTR::new();
        let service = locator.ConnectServer(
            &BSTR::from("ROOT\\WMI"),
            &empty,
            &empty,
            &empty,
            0,
            &empty,
            None,
        )?;
        CoSetProxyBlanket(
            &service,
            10,
            0,
            PCWSTR::null(),
            RPC_C_AUTHN_LEVEL_PKT,
            RPC_C_IMP_LEVEL_IMPERSONATE,
            None,
            EOAC_NONE,
        )?;
        Ok(service)
    }
}
fn first(service: &IWbemServices, query: &str) -> Result<IWbemClassObject> {
    unsafe {
        let rows = service.ExecQuery(
            &BSTR::from("WQL"),
            &BSTR::from(query),
            WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY,
            None,
        )?;
        let mut row = [None];
        let mut count = 0;
        rows.Next(30_000, &mut row, &mut count).ok()?;
        ensure!(
            count == 1,
            "WMI brightness query returned no object: {query}"
        );
        row[0]
            .take()
            .context("WMI brightness query omitted its object")
    }
}
fn set_brightness(service: &IWbemServices, value: u8) -> Result<()> {
    let method_object = first(service, "SELECT * FROM WmiMonitorBrightnessMethods")?;
    let path = property(&method_object, w!("__PATH"))?.string()?;
    unsafe {
        let mut class = None;
        service.GetObject(
            &BSTR::from("WmiMonitorBrightnessMethods"),
            WBEM_GENERIC_FLAG_TYPE(0),
            None,
            Some(&mut class),
            None,
        )?;
        let class = class.context("WMI omitted brightness method class")?;
        let mut signature = None;
        class.GetMethod(
            w!("WmiSetBrightness"),
            0,
            &mut signature,
            std::ptr::null_mut(),
        )?;
        let parameters = signature
            .context("WMI omitted brightness method signature")?
            .SpawnInstance(0)?;
        // RVA 0x132438 uses VT_I4 for both arguments, including Timeout=10.
        parameters.Put(w!("Timeout"), 0, &VARIANT::from(10i32), 0)?;
        parameters.Put(w!("Brightness"), 0, &VARIANT::from(i32::from(value)), 0)?;
        let mut result = None;
        service.ExecMethod(
            &BSTR::from(path),
            &BSTR::from("WmiSetBrightness"),
            WBEM_GENERIC_FLAG_TYPE(0),
            None,
            &parameters,
            None,
            Some(&mut result),
        )?;
        let status = result
            .context("WMI omitted brightness call result")?
            .GetCallStatus(0)?;
        ensure!(status == 0, "WmiSetBrightness call status: {status:#x}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn brightness_uses_observed_value_and_source_endpoints() {
        let levels = [10, 25, 50, 90];
        assert_eq!(
            target(DisplayAction::Brightness(0), &levels, None).unwrap(),
            0
        );
        assert_eq!(
            target(DisplayAction::Brightness(255), &levels, None).unwrap(),
            90
        );
        assert_eq!(
            target(DisplayAction::BrightnessDown(50), &levels, Some(37)).unwrap(),
            10
        );
        assert_eq!(
            target(DisplayAction::BrightnessUp(255), &levels, Some(37)).unwrap(),
            90
        );
        assert_eq!(
            target(DisplayAction::BrightnessUp(5), &levels, Some(37)).unwrap(),
            42
        );
        assert!(target(DisplayAction::BrightnessUp(5), &levels, None).is_err());
        assert!(target(DisplayAction::Brightness(5), &[], None).is_err());
    }
    #[test]
    fn driverless_owner_can_open_but_never_claims_driver_delivery() {
        let mut owner = Display::open(None).unwrap();
        assert!(owner.execute(DisplayAction::DriverBrightnessDown).is_err());
        owner.close().unwrap();
        assert!(owner.execute(DisplayAction::Brightness(20)).is_err());
    }
}
