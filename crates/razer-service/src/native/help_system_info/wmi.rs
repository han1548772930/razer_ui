use anyhow::ensure;
use windows::{
    Win32::System::{
        Com::{
            CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx,
            CoInitializeSecurity, CoSetProxyBlanket, CoUninitialize, EOAC_NONE,
            RPC_C_AUTHN_LEVEL_CALL, RPC_C_AUTHN_LEVEL_DEFAULT, RPC_C_IMP_LEVEL_IMPERSONATE,
        },
        Variant::{VARIANT, VT_BSTR, VariantClear},
        Wmi::{
            IWbemLocator, IWbemServices, WBEM_FLAG_FORWARD_ONLY, WBEM_FLAG_RETURN_IMMEDIATELY,
            WbemLocator,
        },
    },
    core::{BSTR, PCWSTR},
};

struct Apartment(bool);
impl Apartment {
    fn open() -> anyhow::Result<Self> {
        // The synchronous report owner never moves these interfaces to another
        // thread. RPC_E_CHANGED_MODE permits the caller's existing apartment.
        let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
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

/// One real WMI enumeration per source helper, with last-row overwrite only
/// for properties actually returned. Absent properties never become defaults.
pub(super) fn strings<const N: usize>(
    class: &str,
    fields: [&str; N],
    diagnostics: &mut Vec<String>,
) -> anyhow::Result<[Option<String>; N]> {
    let _apartment = Apartment::open()?;
    unsafe {
        if let Err(error) = CoInitializeSecurity(
            None,
            -1,
            None,
            None,
            RPC_C_AUTHN_LEVEL_DEFAULT,
            RPC_C_IMP_LEVEL_IMPERSONATE,
            None,
            EOAC_NONE,
            None,
        ) {
            // COM security is process-wide and may already be initialized.
            if error.code().0 != 0x80010119u32 as i32 {
                return Err(error.into());
            }
        }
        let locator: IWbemLocator = CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER)?;
        let empty = BSTR::new();
        let service: IWbemServices = locator.ConnectServer(
            &BSTR::from("ROOT\\CIMV2"),
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
            RPC_C_AUTHN_LEVEL_CALL,
            RPC_C_IMP_LEVEL_IMPERSONATE,
            None,
            EOAC_NONE,
        )?;
        let query = BSTR::from(format!("SELECT * FROM {class}"));
        let rows = service.ExecQuery(
            &BSTR::from("WQL"),
            &query,
            WBEM_FLAG_FORWARD_ONLY | WBEM_FLAG_RETURN_IMMEDIATELY,
            None,
        )?;
        let mut values = std::array::from_fn(|_| None);
        loop {
            let mut row = [None];
            let mut count = 0;
            let result = rows.Next(30_000, &mut row, &mut count);
            if let Err(error) = result.ok() {
                diagnostics.push(format!("WMI {class} enumeration: {error}"));
                break;
            }
            if result.0 == 0x40004 {
                diagnostics.push(format!("WMI {class} enumeration timed out"));
                break;
            }
            if count == 0 {
                break;
            }
            let Some(row) = row[0].as_ref() else {
                diagnostics.push(format!("WMI {class} omitted its returned object"));
                break;
            };
            for (field, observed) in fields.iter().zip(&mut values) {
                let name: Vec<u16> = field.encode_utf16().chain(Some(0)).collect();
                let mut value = Property(VARIANT::default());
                let result: anyhow::Result<String> = (|| {
                    row.Get(PCWSTR(name.as_ptr()), 0, &mut value.0, None, None)?;
                    let inner = &value.0.Anonymous.Anonymous;
                    ensure!(inner.vt == VT_BSTR, "property is not a string");
                    Ok(String::from_utf16(&inner.Anonymous.bstrVal)?)
                })();
                match result {
                    Ok(value) => *observed = Some(value),
                    Err(error) => diagnostics.push(format!("WMI {class}.{field}: {error:#}")),
                }
            }
        }
        Ok(values)
    }
}
