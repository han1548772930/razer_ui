//! Apps signature policy from Simple Service 0x49448 and 0x4ae43.
//! Only Windows cryptographic providers are called; vendor code is not loaded.
use anyhow::{Result, bail, ensure};
use std::{
    ffi::CStr,
    fs::File,
    mem::size_of,
    os::windows::{ffi::OsStrExt, io::AsRawHandle},
    path::Path,
    ptr,
};
use windows_sys::Win32::Security::{Cryptography::*, WinTrust::*};

#[path = "user_app_signature_ico.rs"]
mod ico;

pub(super) const ALLOWED_SIGNERS: [&str; 2] = ["Razer USA Ltd.", "THX LTD."];
pub(super) const NOT_TRUSTED: &str = "Error: This PE file is not trusted";
pub(super) const NOT_ALLOWED: &str = "Error: This PE file is not signed by allowed signer";
pub(super) const INVALID_SIGNATURE: &str = "Error: Invalid signature.";

/// The owned file and path must identify the same immutable staged bytes.
pub(super) fn verify(file: &File, path: &Path, bytes: &[u8], extension: &str) -> Result<()> {
    match extension {
        "zip" => Ok(()),
        "ico" => ico::verify(bytes),
        _ => authenticode(file, path, matches!(extension, "msi" | "msix")),
    }
}

struct TrustState(WINTRUST_DATA);
impl Drop for TrustState {
    fn drop(&mut self) {
        self.0.dwStateAction = WTD_STATEACTION_CLOSE;
        // SAFETY: the state was initialized by WinVerifyTrust; file buffers are
        // retained by the enclosing call until after this close operation.
        unsafe {
            WinVerifyTrust(
                -1isize as _,
                &mut WINTRUST_ACTION_GENERIC_VERIFY_V2.clone(),
                (&mut self.0 as *mut WINTRUST_DATA).cast(),
            );
        }
    }
}

fn authenticode(file: &File, path: &Path, by_path: bool) -> Result<()> {
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut input = WINTRUST_FILE_INFO {
        cbStruct: size_of::<WINTRUST_FILE_INFO>() as u32,
        pcwszFilePath: if by_path { wide.as_ptr() } else { ptr::null() },
        hFile: if by_path {
            ptr::null_mut()
        } else {
            file.as_raw_handle()
        },
        ..Default::default()
    };
    let mut state = TrustState(WINTRUST_DATA {
        cbStruct: size_of::<WINTRUST_DATA>() as u32,
        dwUIChoice: WTD_UI_NONE,
        dwUnionChoice: WTD_CHOICE_FILE,
        Anonymous: WINTRUST_DATA_0 { pFile: &mut input },
        dwStateAction: WTD_STATEACTION_VERIFY,
        dwProvFlags: WTD_CACHE_ONLY_URL_RETRIEVAL,
        ..Default::default()
    });
    // SAFETY: all nested input buffers remain live, the API owns its returned
    // provider graph until TrustState closes it, and no pointer escapes.
    unsafe {
        let status = WinVerifyTrust(
            -1isize as _,
            &mut WINTRUST_ACTION_GENERIC_VERIFY_V2.clone(),
            (&mut state.0 as *mut WINTRUST_DATA).cast(),
        );
        let provider = WTHelperProvDataFromStateData(state.0.hWVTStateData);
        ensure!(status == 0 && !provider.is_null(), NOT_TRUSTED);
        let provider = &*provider;
        ensure!(
            provider.csSigners != 0 && !provider.pasSigners.is_null(),
            NOT_TRUSTED
        );
        let mut certificates = 0usize;
        for index in 0..provider.csSigners as usize {
            let chain = (*provider.pasSigners.add(index)).pChainContext;
            if chain.is_null() {
                break;
            }
            if (*chain).rgpChain.is_null() {
                continue;
            }
            for chain_index in 0..(*chain).cChain as usize {
                let simple = *(*chain).rgpChain.add(chain_index);
                if simple.is_null() {
                    break;
                }
                if (*simple).rgpElement.is_null() {
                    continue;
                }
                for element_index in 0..(*simple).cElement as usize {
                    let element = *(*simple).rgpElement.add(element_index);
                    if element.is_null() || (*element).pCertContext.is_null() {
                        continue;
                    }
                    certificates += 1;
                    if let Ok(name) = common_name((*element).pCertContext) {
                        if ALLOWED_SIGNERS.contains(&name.as_str()) {
                            return Ok(());
                        }
                    }
                }
            }
        }
        ensure!(certificates != 0, "Error: This PE file has no certificate");
    }
    bail!(NOT_ALLOWED)
}

pub(super) struct Certificate(*const CERT_CONTEXT);
impl Certificate {
    pub(super) fn parse(bytes: &[u8]) -> Result<Self> {
        ensure!(bytes.len() <= u32::MAX as usize, INVALID_SIGNATURE);
        // SAFETY: Windows copies the DER certificate into an owned context.
        let cert = unsafe {
            CertCreateCertificateContext(X509_ASN_ENCODING, bytes.as_ptr(), bytes.len() as u32)
        };
        ensure!(!cert.is_null(), INVALID_SIGNATURE);
        Ok(Self(cert))
    }
    pub(super) fn common_name(&self) -> Result<String> {
        // SAFETY: this context remains owned by self for the entire call.
        unsafe { common_name(self.0) }
    }
    fn verify(&self, algorithm: ico::Digest, bytes: &[u8], signature: &[u8]) -> Result<()> {
        let digest = algorithm.hash(bytes)?;
        // SAFETY: CERT_CONTEXT owns its CERT_INFO and public key throughout
        // the import; BCrypt returns an independently owned key handle.
        unsafe {
            ensure!(!(*self.0).pCertInfo.is_null(), INVALID_SIGNATURE);
            let info = &(*(*self.0).pCertInfo).SubjectPublicKeyInfo;
            ensure!(!info.Algorithm.pszObjId.is_null(), INVALID_SIGNATURE);
            let oid = CStr::from_ptr(info.Algorithm.pszObjId.cast()).to_bytes();
            let rsa = oid == b"1.2.840.113549.1.1.1";
            let ec = oid == b"1.2.840.10045.2.1";
            ensure!(
                (rsa && matches!(algorithm, ico::Digest::Sha1 | ico::Digest::Sha256))
                    || (ec && algorithm == ico::Digest::Sha256),
                INVALID_SIGNATURE
            );
            let mut key = ptr::null_mut();
            ensure!(
                CryptImportPublicKeyInfoEx2(X509_ASN_ENCODING, info, 0, ptr::null(), &mut key) != 0,
                INVALID_SIGNATURE
            );
            let key = PublicKey(key);
            let padding = BCRYPT_PKCS1_PADDING_INFO {
                pszAlgId: if algorithm == ico::Digest::Sha1 {
                    BCRYPT_SHA1_ALGORITHM
                } else {
                    BCRYPT_SHA256_ALGORITHM
                },
            };
            let converted;
            let (signature, flags, padding_ptr) = if rsa {
                (
                    signature,
                    BCRYPT_PAD_PKCS1,
                    (&padding as *const BCRYPT_PKCS1_PADDING_INFO).cast(),
                )
            } else {
                let mut bits = 0u32;
                let mut count = 0;
                ensure!(
                    BCryptGetProperty(
                        key.0,
                        BCRYPT_KEY_LENGTH,
                        (&mut bits as *mut u32).cast(),
                        4,
                        &mut count,
                        0
                    ) >= 0
                        && count == 4,
                    INVALID_SIGNATURE
                );
                converted = ico::ecdsa_signature(signature, bits.div_ceil(8) as usize)?;
                (converted.as_slice(), 0, ptr::null())
            };
            ensure!(
                BCryptVerifySignature(
                    key.0,
                    padding_ptr,
                    digest.as_ptr(),
                    digest.len() as u32,
                    signature.as_ptr(),
                    signature.len() as u32,
                    flags
                ) >= 0,
                INVALID_SIGNATURE
            );
        }
        Ok(())
    }
}
impl Drop for Certificate {
    fn drop(&mut self) {
        unsafe {
            CertFreeCertificateContext(self.0);
        }
    }
}
struct PublicKey(BCRYPT_KEY_HANDLE);
impl Drop for PublicKey {
    fn drop(&mut self) {
        unsafe {
            BCryptDestroyKey(self.0);
        }
    }
}

unsafe fn common_name(cert: *const CERT_CONTEXT) -> Result<String> {
    // SAFETY: caller guarantees a live certificate context; query/read use the
    // same immutable context and a correctly sized owned UTF-16 output buffer.
    let len = unsafe {
        CertGetNameStringW(
            cert,
            CERT_NAME_ATTR_TYPE,
            0,
            szOID_COMMON_NAME.cast(),
            ptr::null_mut(),
            0,
        )
    };
    ensure!(len > 1, "certificate common name is absent");
    let mut value = vec![0; len as usize];
    let written = unsafe {
        CertGetNameStringW(
            cert,
            CERT_NAME_ATTR_TYPE,
            0,
            szOID_COMMON_NAME.cast(),
            value.as_mut_ptr(),
            len,
        )
    };
    ensure!(
        written == len && value.last() == Some(&0),
        "certificate common name changed"
    );
    Ok(String::from_utf16(&value[..value.len() - 1])?)
}
