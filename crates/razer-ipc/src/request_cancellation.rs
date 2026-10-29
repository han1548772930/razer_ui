//! Request-owned cancellation observed without waiting for the worker's stdin loop.
use serde::{Deserialize, Serialize};

/// A canceled BLE request's actual response, retained only for validated diagnostics.
#[derive(Debug)]
pub struct CanceledBleReply {
    pub data: serde_json::Value,
    pub error: String,
}

impl std::fmt::Display for CanceledBleReply {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.error)
    }
}

impl std::error::Error for CanceledBleReply {}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct Token(String);

impl TryFrom<String> for Token {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        anyhow::ensure!(
            value.len() == 32
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
            "服务请求取消身份必须是完整的小写随机 UUID"
        );
        let id = uuid::Uuid::parse_str(&value)?;
        anyhow::ensure!(
            id.get_version() == Some(uuid::Version::Random)
                && id.get_variant() == uuid::Variant::RFC4122,
            "服务请求取消身份必须是随机 UUID v4"
        );
        Ok(Self(value))
    }
}

impl From<Token> for String {
    fn from(token: Token) -> Self {
        token.0
    }
}

pub use platform::{CancellationOwner, CancellationReader};

#[cfg(windows)]
mod platform {
    use super::Token;
    use anyhow::{Context as _, bail, ensure};
    use std::{
        os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
        ptr,
    };
    use windows_sys::Win32::{
        Foundation::{
            ERROR_ALREADY_EXISTS, GetLastError, WAIT_FAILED, WAIT_OBJECT_0, WAIT_TIMEOUT,
        },
        System::Threading::{
            CreateEventW, OpenEventW, SYNCHRONIZATION_SYNCHRONIZE, SetEvent, WaitForSingleObject,
        },
    };

    pub struct CancellationOwner {
        token: Token,
        event: OwnedHandle,
    }

    impl CancellationOwner {
        pub fn new() -> anyhow::Result<Self> {
            Self::create(Token(uuid::Uuid::new_v4().simple().to_string()))
        }

        fn create(token: Token) -> anyhow::Result<Self> {
            let name = event_name(&token);
            // SAFETY: name is NUL-terminated and retained for the call. Null
            // SECURITY_ATTRIBUTES prevents inheritance; the reader opens its own handle.
            let raw = unsafe { CreateEventW(ptr::null(), 1, 0, name.as_ptr()) };
            let error = unsafe { GetLastError() };
            if raw.is_null() {
                return Err(std::io::Error::from_raw_os_error(error as i32))
                    .context("无法创建设备服务请求取消事件");
            }
            // SAFETY: CreateEventW returned a valid owned kernel handle.
            let event = unsafe { OwnedHandle::from_raw_handle(raw) };
            ensure!(
                error != ERROR_ALREADY_EXISTS,
                "设备服务请求取消身份已被占用"
            );
            Ok(Self { token, event })
        }

        pub fn token(&self) -> Token {
            self.token.clone()
        }

        pub fn cancel(&self) -> anyhow::Result<()> {
            // SAFETY: this owner retains the valid event handle throughout the call.
            if unsafe { SetEvent(self.event.as_raw_handle()) } == 0 {
                return Err(std::io::Error::last_os_error())
                    .context("无法通知设备服务取消当前请求");
            }
            Ok(())
        }
    }

    impl Drop for CancellationOwner {
        fn drop(&mut self) {
            if let Err(error) = self.cancel() {
                eprintln!("[device service request cancellation] {error:#}");
            }
        }
    }

    pub struct CancellationReader {
        event: OwnedHandle,
    }

    impl CancellationReader {
        pub fn open(token: &Token) -> anyhow::Result<Self> {
            let name = event_name(token);
            // SAFETY: the NUL-terminated name is retained for the call. The
            // non-inheritable handle grants observation, without reset/set access.
            let raw = unsafe { OpenEventW(SYNCHRONIZATION_SYNCHRONIZE, 0, name.as_ptr()) };
            if raw.is_null() {
                return Err(std::io::Error::last_os_error())
                    .context("无法打开设备服务请求取消事件");
            }
            // SAFETY: OpenEventW returned a valid owned kernel handle.
            Ok(Self {
                event: unsafe { OwnedHandle::from_raw_handle(raw) },
            })
        }

        pub fn check(&self) -> anyhow::Result<()> {
            // SAFETY: the handle remains owned; a zero wait only queries state.
            match unsafe { WaitForSingleObject(self.event.as_raw_handle(), 0) } {
                WAIT_TIMEOUT => Ok(()),
                WAIT_OBJECT_0 => bail!("设备服务请求已取消，停止后续设备操作"),
                WAIT_FAILED => {
                    Err(std::io::Error::last_os_error()).context("无法读取设备服务请求取消状态")
                }
                result => bail!("设备服务请求取消事件返回无效等待状态：{result}"),
            }
        }
    }

    fn event_name(token: &Token) -> Vec<u16> {
        format!("Local\\RazerUi.BleMouseCurrentDpi.Cancel.{}\0", token.0)
            .encode_utf16()
            .collect()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn current_dpi_ble_manual_reset_cancellation_remains_signaled_for_every_reader() {
            let owner = CancellationOwner::new().unwrap();
            let reader = CancellationReader::open(&owner.token()).unwrap();
            reader.check().unwrap();
            owner.cancel().unwrap();
            owner.cancel().unwrap();
            assert!(reader.check().is_err());
            assert!(reader.check().is_err());
            assert!(
                CancellationReader::open(&owner.token())
                    .unwrap()
                    .check()
                    .is_err()
            );
        }

        #[test]
        fn current_dpi_ble_old_owner_cannot_cancel_a_successor_request() {
            let first = CancellationOwner::new().unwrap();
            let next = CancellationOwner::new().unwrap();
            assert_ne!(first.token(), next.token());
            let next_reader = CancellationReader::open(&next.token()).unwrap();
            first.cancel().unwrap();
            drop(first);
            next_reader.check().unwrap();
        }

        #[test]
        fn current_dpi_ble_dropping_owner_cancels_and_last_handle_releases_the_event() {
            let owner = CancellationOwner::new().unwrap();
            let token = owner.token();
            let reader = CancellationReader::open(&token).unwrap();
            drop(owner);
            assert!(reader.check().is_err());
            drop(reader);
            assert!(CancellationReader::open(&token).is_err());
        }

        #[test]
        fn current_dpi_ble_reader_cannot_signal_or_reset_an_active_request() {
            let owner = CancellationOwner::new().unwrap();
            let reader = CancellationReader::open(&owner.token()).unwrap();
            // SAFETY: the reader retains a valid event; Windows must reject
            // modification because this handle only grants SYNCHRONIZE.
            assert_eq!(unsafe { SetEvent(reader.event.as_raw_handle()) }, 0);
            assert_eq!(
                unsafe { GetLastError() },
                windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED
            );
            // SAFETY: the owned observation handle remains valid for the call.
            assert_eq!(
                unsafe {
                    windows_sys::Win32::System::Threading::ResetEvent(reader.event.as_raw_handle())
                },
                0
            );
            assert_eq!(
                unsafe { GetLastError() },
                windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED
            );
            reader.check().unwrap();
        }

        #[test]
        fn current_dpi_ble_collision_rejects_borrowed_event_without_resetting_the_owner() {
            let owner = CancellationOwner::new().unwrap();
            let reader = CancellationReader::open(&owner.token()).unwrap();
            assert!(CancellationOwner::create(owner.token()).is_err());
            reader.check().unwrap();
        }
    }
}

#[cfg(not(windows))]
mod platform {
    use super::Token;

    pub enum CancellationOwner {}

    impl CancellationOwner {
        pub fn new() -> anyhow::Result<Self> {
            anyhow::bail!("当前平台不支持 BLE DPI 跨进程请求取消事件")
        }

        pub fn token(&self) -> Token {
            match *self {}
        }

        pub fn cancel(&self) -> anyhow::Result<()> {
            anyhow::bail!("当前平台不支持 BLE DPI 跨进程请求取消事件")
        }
    }

    pub enum CancellationReader {}

    impl CancellationReader {
        pub fn open(_token: &Token) -> anyhow::Result<Self> {
            anyhow::bail!("当前平台不支持 BLE DPI 跨进程请求取消事件")
        }

        pub fn check(&self) -> anyhow::Result<()> {
            anyhow::bail!("当前平台不支持 BLE DPI 跨进程请求取消事件")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_dpi_ble_wire_token_uses_an_independently_fixed_complete_uuid() {
        const WIRE: &str = "\"1234567890ab4def8abc1234567890ab\"";
        let token: Token = serde_json::from_str(WIRE).unwrap();
        assert_eq!(serde_json::to_string(&token).unwrap(), WIRE);
    }

    #[test]
    fn current_dpi_ble_wire_token_rejects_paths_short_identity_non_random_uuid_and_wrong_types() {
        for wire in [
            "\"Local\\\\borrowed\"",
            "\"1234567890ab4def8abc1234567890a\"",
            "\"1234567890ab4def8abc1234567890ab0\"",
            "\"1234567890AB4def8abc1234567890ab\"",
            "\"1234567890ab1def8abc1234567890ab\"",
            "\"1234567890ab4def0abc1234567890ab\"",
            "null",
            "19",
            "{\"id\":\"1234567890ab4def8abc1234567890ab\"}",
        ] {
            assert!(serde_json::from_str::<Token>(wire).is_err(), "{wire}");
        }
    }

    #[cfg(not(windows))]
    #[test]
    fn current_dpi_ble_unsupported_platform_does_not_create_or_open_a_fake_cancellation_channel() {
        let token = Token::try_from("1234567890ab4def8abc1234567890ab".to_owned()).unwrap();
        assert!(CancellationOwner::new().is_err());
        assert!(CancellationReader::open(&token).is_err());
    }
}
