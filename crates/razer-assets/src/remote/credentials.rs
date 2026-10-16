//! Read private-resource credentials locally. Never serialize or log them.
pub(super) fn token() -> Option<String> {
    if let Ok(token) = std::env::var("HF_TOKEN") {
        let token = token.trim();
        if !token.is_empty() {
            return Some(token.to_owned());
        }
    }
    #[cfg(windows)]
    if let Some(token) = windows_token() {
        return Some(token);
    }
    // Also accept the normal local `hf auth login` token file.
    let directory = std::env::var_os("HF_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                .map(|home| std::path::PathBuf::from(home).join(".cache/huggingface"))
        })?;
    let token = std::fs::read_to_string(directory.join("token")).ok()?;
    let token = token.trim();
    (!token.is_empty()).then(|| token.to_owned())
}

#[cfg(windows)]
fn windows_token() -> Option<String> {
    use windows_sys::Win32::Security::Credentials::{
        CRED_TYPE_GENERIC, CREDENTIALW, CredFree, CredReadW,
    };
    let target = "git:https://huggingface.co\0"
        .encode_utf16()
        .collect::<Vec<_>>();
    let mut credential: *mut CREDENTIALW = std::ptr::null_mut();
    // SAFETY: target is terminated, output is valid, and CredFree releases the allocation.
    unsafe {
        if CredReadW(target.as_ptr(), CRED_TYPE_GENERIC, 0, &mut credential) == 0
            || credential.is_null()
        {
            return None;
        }
        let value = &*credential;
        let bytes =
            std::slice::from_raw_parts(value.CredentialBlob, value.CredentialBlobSize as usize);
        let wide = bytes
            .chunks_exact(2)
            .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
            .collect::<Vec<_>>();
        let token = String::from_utf16(&wide)
            .ok()
            .map(|value| value.trim_end_matches('\0').to_owned());
        CredFree(credential.cast());
        token.filter(|token| !token.is_empty())
    }
}
