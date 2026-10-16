//! Current SysUtilsNative keyboardLayout export, independent of vendor loading.
//! This is the calling thread's layout query, not a foreground-window monitor.
pub fn get() -> anyhow::Result<i32> {
    #[cfg(windows)]
    {
        Ok(crate::platform::windows::keyboard_layout::get())
    }
    #[cfg(not(windows))]
    {
        anyhow::bail!("SysUtilsNative keyboardLayout 使用 Windows KLID，此平台不支持")
    }
}

/// `sscanf("%x")` over the original zero-initialized ANSI KLID buffer. FFI
/// declares int, so preserve the UINT bit pattern, including a negative i32.
pub(crate) fn parse_klid(bytes: &[u8]) -> i32 {
    let bytes = bytes.split(|byte| *byte == 0).next().unwrap_or_default();
    let mut cursor = 0;
    while bytes.get(cursor).is_some_and(u8::is_ascii_whitespace) {
        cursor += 1;
    }
    let negative = bytes.get(cursor) == Some(&b'-');
    if matches!(bytes.get(cursor), Some(b'+' | b'-')) {
        cursor += 1;
    }
    if bytes.get(cursor) == Some(&b'0') && matches!(bytes.get(cursor + 1), Some(b'x' | b'X')) {
        cursor += 2;
    }
    let mut value = 0u32;
    while let Some(digit) = bytes.get(cursor).and_then(|byte| match byte {
        b'0'..=b'9' => Some(u32::from(byte - b'0')),
        b'a'..=b'f' => Some(u32::from(byte - b'a') + 10),
        b'A'..=b'F' => Some(u32::from(byte - b'A') + 10),
        _ => None,
    }) {
        value = value.wrapping_mul(16).wrapping_add(digit);
        cursor += 1;
    }
    if negative {
        value.wrapping_neg() as i32
    } else {
        value as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn klid_preserves_native_hex_and_signed_ffi_bits() {
        assert_eq!(parse_klid(b"00000409\0"), 0x409);
        assert_eq!(parse_klid(b"E0010804\0"), 0xe0010804u32 as i32);
        assert_eq!(parse_klid(b" \t0x00000411\0"), 0x411);
        assert_eq!(parse_klid(b"\0"), 0);
        assert_eq!(parse_klid(b"invalid\0"), 0);
        assert_eq!(parse_klid(b"00000409\0ffffffff"), 0x409);
    }
}
