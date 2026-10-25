//! DriverImpl RVA110830/110e20/111150/111b40/112340. Hook installation
//! uses a 292-byte record; removal uses the original 32-byte input record.
use super::NativeOperation;
use crate::mapping_native_input::{KeyboardInput, MouseInput, encode_keyboard, encode_mouse};

pub const ENABLE_HOOKS_IOCTL: u32 = 0x8888_3034;
pub const ENABLE_NOTIFY_IOCTL: u32 = 0x8888_3038;
pub const ENABLE_REDIRECT_IOCTL: u32 = 0x8888_301c;
pub const SET_HOOK_IOCTL: u32 = 0x8888_3024;
pub const CLEAR_HOOK_IOCTL: u32 = 0x8888_302c;
pub const ENUMERATE_HOOK_IOCTL: u32 = 0x8888_3030;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HookInput {
    Keyboard {
        input: KeyboardInput,
        extra_information: u32,
    },
    Mouse {
        input: MouseInput,
        extra_information: u32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedirectKind {
    Keyboard,
    Mouse,
    MouseMove,
}

pub fn enable_hooks(enabled: bool) -> NativeOperation {
    NativeOperation::ioctl(ENABLE_HOOKS_IOCTL, u32::from(enabled).to_le_bytes(), 0)
}

pub fn enable_notifications(enabled: bool) -> NativeOperation {
    NativeOperation::ioctl(ENABLE_NOTIFY_IOCTL, u32::from(enabled).to_le_bytes(), 0)
}

pub fn redirect(kind: RedirectKind, enabled: bool) -> NativeOperation {
    let kind: u32 = match kind {
        RedirectKind::Keyboard => 1,
        RedirectKind::Mouse => 2,
        RedirectKind::MouseMove => 4,
    };
    let mut bytes = kind.to_le_bytes().to_vec();
    bytes.push(u8::from(enabled));
    NativeOperation::ioctl(ENABLE_REDIRECT_IOCTL, bytes, 0)
}

pub fn enumerate(index: u32) -> NativeOperation {
    NativeOperation::ioctl(ENUMERATE_HOOK_IOCTL, index.to_le_bytes(), 292)
}

impl HookInput {
    pub fn install(self) -> NativeOperation {
        let mut bytes = vec![0; 292];
        match self {
            Self::Keyboard { mut input, .. } => {
                input.flag &= !1;
                bytes[..32].copy_from_slice(encode_keyboard(input).bytes());
            }
            Self::Mouse { input, .. } => {
                bytes[..32].copy_from_slice(encode_mouse(input).bytes());
                let pair = match input.packed_buttons() {
                    1 | 2 => 3,
                    4 | 8 => 12,
                    16 | 32 => 48,
                    64 | 128 => 192,
                    256 | 512 => 768,
                    0x10000 | 0x20000 => 0x30000,
                    0x40000 | 0x80000 => 0xc0000,
                    value => value,
                };
                bytes[12..16].copy_from_slice(&pair.to_le_bytes());
            }
        }
        NativeOperation::ioctl(SET_HOOK_IOCTL, bytes, 0)
    }

    pub fn remove(self) -> NativeOperation {
        let mut bytes = match self {
            Self::Keyboard { input, .. } => encode_keyboard(input).bytes().to_vec(),
            Self::Mouse { input, .. } => encode_mouse(input).bytes().to_vec(),
        };
        let (at, extra) = match self {
            Self::Keyboard {
                extra_information, ..
            } => (16, extra_information),
            Self::Mouse {
                extra_information, ..
            } => (28, extra_information),
        };
        bytes[at..at + 4].copy_from_slice(&extra.to_le_bytes());
        NativeOperation::ioctl(CLEAR_HOOK_IOCTL, bytes, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(operation: NativeOperation, expected_ioctl: u32) -> Vec<u8> {
        match operation {
            NativeOperation::Ioctl {
                ioctl,
                input,
                output_bytes,
            } => {
                assert_eq!(ioctl, expected_ioctl);
                assert_eq!(output_bytes, 0);
                input
            }
            NativeOperation::ReadInput { .. } => panic!("Expected driver control operation"),
        }
    }

    #[test]
    fn hook_install_pairs_break_and_removal_keeps_original_native_identity() {
        let key = HookInput::Keyboard {
            input: KeyboardInput {
                unit_id: 3,
                scancode: 29,
                flag: 3,
                modifiers: 0x10,
                ..Default::default()
            },
            extra_information: 0x12345678,
        };
        let installed = bytes(key.install(), 0x88883024);
        assert_eq!(installed.len(), 292);
        assert_eq!(
            &installed[..20],
            &[0, 0, 0, 0, 1, 0, 0, 0, 3, 0, 29, 0, 2, 0, 0, 0, 0, 1, 0, 0]
        );
        assert!(installed[20..].iter().all(|byte| *byte == 0));
        let removed = bytes(key.remove(), 0x8888302c);
        assert_eq!(removed.len(), 32);
        assert_eq!(&removed[12..20], &[3, 0, 0, 0, 0x78, 0x56, 0x34, 0x12]);
    }

    #[test]
    fn hook_mouse_extended_button_pair_and_redirect_width_follow_driver_record() {
        let mouse = HookInput::Mouse {
            input: MouseInput {
                id: 0,
                data: 8,
                ..Default::default()
            },
            extra_information: 42,
        };
        let installed = bytes(mouse.install(), 0x88883024);
        assert_eq!(&installed[12..16], &[0, 0, 12, 0]);
        assert_eq!(&bytes(mouse.remove(), 0x8888302c)[28..], &[42, 0, 0, 0]);
        assert_eq!(
            bytes(redirect(RedirectKind::MouseMove, true), 0x8888301c),
            [4, 0, 0, 0, 1]
        );
        assert_eq!(bytes(enable_hooks(false), 0x88883034), [0, 0, 0, 0]);
        assert_eq!(bytes(enable_notifications(true), 0x88883038), [1, 0, 0, 0]);
    }
}
