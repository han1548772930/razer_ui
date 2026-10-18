//! Custom ICO signature: PNG iTXt carries PKCS#7 DER. Native 0x4b78e,
//! 0x4b8eb, 0x49cb0, 0x4a002, 0x4a394, 0x4a857 and 0x4aa08.
use super::{ALLOWED_SIGNERS, Certificate, INVALID_SIGNATURE, NOT_ALLOWED};
use anyhow::{Result, bail, ensure};
use sha2::{Digest as _, Sha224, Sha256, Sha384, Sha512};
use std::ptr;
use windows_sys::Win32::Security::Cryptography::CryptHashCertificate2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Digest {
    Md4,
    Md5,
    Sha1,
    Sha224,
    Sha256,
    Sha384,
    Sha512,
}
impl Digest {
    fn parse(oid: &[u8]) -> Result<Self> {
        Ok(match oid {
            b"\x2a\x86\x48\x86\xf7\x0d\x02\x04" => Self::Md4,
            b"\x2a\x86\x48\x86\xf7\x0d\x02\x05" => Self::Md5,
            b"\x2b\x0e\x03\x02\x1a" => Self::Sha1,
            b"\x60\x86\x48\x01\x65\x03\x04\x02\x01" => Self::Sha256,
            b"\x60\x86\x48\x01\x65\x03\x04\x02\x02" => Self::Sha384,
            b"\x60\x86\x48\x01\x65\x03\x04\x02\x03" => Self::Sha512,
            b"\x60\x86\x48\x01\x65\x03\x04\x02\x04" => Self::Sha224,
            _ => bail!(INVALID_SIGNATURE),
        })
    }
    pub(super) fn hash(self, bytes: &[u8]) -> Result<Vec<u8>> {
        match self {
            Self::Sha224 => Ok(Sha224::digest(bytes).to_vec()),
            Self::Sha256 => Ok(Sha256::digest(bytes).to_vec()),
            Self::Sha384 => Ok(Sha384::digest(bytes).to_vec()),
            Self::Sha512 => Ok(Sha512::digest(bytes).to_vec()),
            _ => {
                let name = match self {
                    Self::Md4 => "MD4",
                    Self::Md5 => "MD5",
                    _ => "SHA1",
                };
                let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
                let mut result = vec![0; if self == Self::Sha1 { 20 } else { 16 }];
                let mut size = result.len() as u32;
                ensure!(bytes.len() <= u32::MAX as usize, INVALID_SIGNATURE);
                // SAFETY: named OS algorithm, bounded input and owned output.
                ensure!(
                    unsafe {
                        CryptHashCertificate2(
                            wide.as_ptr(),
                            0,
                            ptr::null(),
                            bytes.as_ptr(),
                            bytes.len() as u32,
                            result.as_mut_ptr(),
                            &mut size,
                        )
                    } != 0
                        && size as usize == result.len(),
                    INVALID_SIGNATURE
                );
                Ok(result)
            }
        }
    }
}

#[derive(Clone, Copy)]
struct Der<'a> {
    body: &'a [u8],
    encoded: &'a [u8],
}
fn take<'a>(input: &mut &'a [u8], tag: u8) -> Result<Der<'a>> {
    ensure!(input.len() >= 2 && input[0] == tag, INVALID_SIGNATURE);
    let first = input[1];
    let (header, length) = if first < 128 {
        (2, first as usize)
    } else {
        let width = (first & 127) as usize;
        ensure!(
            (1..=4).contains(&width) && input.len() >= 2 + width && input[2] != 0,
            INVALID_SIGNATURE
        );
        let mut length = 0usize;
        for byte in &input[2..2 + width] {
            length = (length << 8) | usize::from(*byte);
        }
        ensure!(length >= 128, INVALID_SIGNATURE);
        (2 + width, length)
    };
    let end = header
        .checked_add(length)
        .ok_or_else(|| anyhow::anyhow!(INVALID_SIGNATURE))?;
    ensure!(end <= input.len(), INVALID_SIGNATURE);
    let result = Der {
        body: &input[header..end],
        encoded: &input[..end],
    };
    *input = &input[end..];
    Ok(result)
}
fn algorithm(input: &mut &[u8]) -> Result<Digest> {
    let mut seq = take(input, 0x30)?.body;
    Digest::parse(take(&mut seq, 6)?.body)
}
fn version(input: &mut &[u8]) -> Result<()> {
    ensure!(
        take(input, 2)?.body.iter().any(|byte| *byte != 0),
        INVALID_SIGNATURE
    );
    Ok(())
}

struct Signed<'a> {
    content: &'a [u8],
    content_digest: Digest,
    expected_content: &'a [u8],
    signer_digest: Digest,
    expected_message: &'a [u8],
    attributes: &'a [u8],
    signature: &'a [u8],
    certificates: &'a [u8],
}
impl<'a> Signed<'a> {
    fn parse(bytes: &'a [u8]) -> Result<Self> {
        let mut outer = take(&mut &*bytes, 0x30)?.body;
        ensure!(
            take(&mut outer, 6)?.body == b"\x2a\x86\x48\x86\xf7\x0d\x01\x07\x02",
            INVALID_SIGNATURE
        );
        let mut explicit = take(&mut outer, 0xa0)?.body;
        let mut data = take(&mut explicit, 0x30)?.body;
        version(&mut data)?;
        take(&mut data, 0x31)?;
        let mut info = take(&mut data, 0x30)?.body;
        take(&mut info, 6)?;
        let mut embedded = take(&mut info, 0xa0)?.body;
        let content = take(&mut embedded, 0x30)?;
        let mut body = content.body;
        take(&mut body, 0x30)?;
        let mut digest_info = take(&mut body, 0x30)?.body;
        let content_digest = algorithm(&mut digest_info)?;
        let expected_content = take(&mut digest_info, 4)?.body;
        let certificates = take(&mut data, 0xa0)?.body;
        let mut signers = take(&mut data, 0x31)?.body;
        let mut signer = take(&mut signers, 0x30)?.body;
        version(&mut signer)?;
        take(&mut signer, 0x30)?;
        let signer_digest = algorithm(&mut signer)?;
        let attributes = take(&mut signer, 0xa0)?;
        let mut attrs = attributes.body;
        let mut expected_message = None;
        while !attrs.is_empty() {
            let mut attr = take(&mut attrs, 0x30)?.body;
            if take(&mut attr, 6)?.body == b"\x2a\x86\x48\x86\xf7\x0d\x01\x09\x04" {
                let mut value = take(&mut attr, 0x31)?.body;
                expected_message = Some(take(&mut value, 4)?.body);
                break;
            }
        }
        take(&mut signer, 0x30)?;
        let signature = take(&mut signer, 4)?.body;
        Ok(Self {
            content: content.encoded,
            content_digest,
            expected_content,
            signer_digest,
            expected_message: expected_message.ok_or_else(|| anyhow::anyhow!(INVALID_SIGNATURE))?,
            attributes: attributes.encoded,
            signature,
            certificates,
        })
    }
}

struct Image {
    entry: [u8; 16],
    offset: usize,
    size: usize,
}
struct SignatureChunk {
    image: usize,
    start: usize,
    total: usize,
    payload: std::ops::Range<usize>,
}
fn u32le(bytes: &[u8]) -> usize {
    u32::from_le_bytes(bytes.try_into().expect("four byte field")) as usize
}
fn images(bytes: &[u8]) -> Result<Vec<Image>> {
    ensure!(
        bytes.len() >= 6 && bytes[..4] == [0, 0, 1, 0],
        "Error: Failed to find signature."
    );
    let count = u16::from_le_bytes([bytes[4], bytes[5]]) as usize;
    ensure!(
        count != 0 && 6 + count * 16 <= bytes.len(),
        "Error: Failed to find signature."
    );
    let mut result = Vec::with_capacity(count);
    for row in bytes[6..6 + count * 16].chunks_exact(16) {
        let size = u32le(&row[8..12]);
        let offset = u32le(&row[12..16]);
        ensure!(
            offset
                .checked_add(size)
                .is_some_and(|end| end <= bytes.len()),
            "Error: Failed to read image data."
        );
        result.push(Image {
            entry: row.try_into().unwrap(),
            offset,
            size,
        });
    }
    Ok(result)
}
fn signature_chunk(bytes: &[u8], images: &[Image]) -> Result<SignatureChunk> {
    for (index, image) in images.iter().enumerate() {
        let image_end = image.offset + image.size;
        if bytes[image.offset..image_end].get(..8) != Some(b"\x89PNG\r\n\x1a\n") {
            continue;
        }
        let mut pos = image.offset + 8;
        while pos.checked_add(12).is_some_and(|end| end <= image_end) {
            let size = u32::from_be_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize;
            let Some(end) = pos
                .checked_add(size)
                .and_then(|value| value.checked_add(12))
                .filter(|end| *end <= image_end)
            else {
                break;
            };
            if &bytes[pos + 4..pos + 8] == b"iTXt" {
                return Ok(SignatureChunk {
                    image: index,
                    start: pos,
                    total: size + 12,
                    payload: pos + 8..pos + 8 + size,
                });
            }
            pos = end;
        }
    }
    bail!("Error: Failed to find signature.")
}
fn reconstruct(bytes: &[u8], images: &[Image], signature: &SignatureChunk) -> Result<Vec<u8>> {
    let mut output = bytes[..6].to_vec();
    let mut write = |position: usize, data: &[u8]| -> Result<()> {
        let end = position
            .checked_add(data.len())
            .ok_or_else(|| anyhow::anyhow!(INVALID_SIGNATURE))?;
        ensure!(end <= bytes.len(), INVALID_SIGNATURE);
        output.resize(output.len().max(end), 0);
        output[position..end].copy_from_slice(data);
        Ok(())
    };
    for (index, image) in images.iter().enumerate() {
        let mut entry = image.entry;
        if index == signature.image {
            let size = image
                .size
                .checked_sub(signature.total)
                .ok_or_else(|| anyhow::anyhow!(INVALID_SIGNATURE))?;
            entry[8..12].copy_from_slice(&(size as u32).to_le_bytes());
            write(6 + index * 16, &entry)?;
            write(image.offset, &bytes[image.offset..signature.start])?;
            write(
                signature.start,
                &bytes[signature.start + signature.total..image.offset + image.size],
            )?;
        } else {
            let offset = if index > signature.image {
                image
                    .offset
                    .checked_sub(signature.total)
                    .ok_or_else(|| anyhow::anyhow!(INVALID_SIGNATURE))?
            } else {
                image.offset
            };
            entry[12..16].copy_from_slice(&(offset as u32).to_le_bytes());
            write(6 + index * 16, &entry)?;
            write(offset, &bytes[image.offset..image.offset + image.size])?;
        }
    }
    Ok(output)
}

pub(super) fn verify(bytes: &[u8]) -> Result<()> {
    let images = images(bytes)?;
    let chunk = signature_chunk(bytes, &images)?;
    let signed = Signed::parse(&bytes[chunk.payload.clone()])?;
    let original = reconstruct(bytes, &images, &chunk)?;
    ensure!(
        signed.content_digest.hash(&original)? == signed.expected_content,
        INVALID_SIGNATURE
    );
    ensure!(
        signed.signer_digest.hash(signed.content)? == signed.expected_message,
        INVALID_SIGNATURE
    );
    let mut certificates = Vec::new();
    let mut encoded = signed.certificates;
    while !encoded.is_empty() {
        let cert = take(&mut encoded, 0x30)?;
        if let Ok(cert) = Certificate::parse(cert.encoded) {
            certificates.push(cert);
        }
    }
    let mut attributes = signed.attributes.to_vec();
    attributes[0] = 0x31;
    let mut last_reason = NOT_ALLOWED;
    // Source tries each allowed name in this order and uses the first matching
    // certificate for each name, independently of SignerInfo issuer/serial.
    for allowed in ALLOWED_SIGNERS {
        if let Some(cert) = certificates
            .iter()
            .find(|cert| cert.common_name().is_ok_and(|name| name == allowed))
        {
            last_reason = INVALID_SIGNATURE;
            if cert
                .verify(signed.signer_digest, &attributes, signed.signature)
                .is_ok()
            {
                return Ok(());
            }
        } else {
            last_reason = NOT_ALLOWED;
        }
    }
    // Each attempt replaces the reason; the final allowed signer owns failure.
    bail!(last_reason)
}

pub(super) fn ecdsa_signature(bytes: &[u8], width: usize) -> Result<Vec<u8>> {
    ensure!((1..=132).contains(&width), INVALID_SIGNATURE);
    let mut sequence = take(&mut &*bytes, 0x30)?.body;
    let mut output = vec![0; width * 2];
    for chunk in output.chunks_exact_mut(width) {
        let integer = take(&mut sequence, 2)?.body;
        ensure!(
            !integer.is_empty() && integer[0] & 0x80 == 0,
            INVALID_SIGNATURE
        );
        let value = integer.strip_prefix(&[0]).unwrap_or(integer);
        ensure!(value.len() <= width, INVALID_SIGNATURE);
        chunk[width - value.len()..].copy_from_slice(value);
    }
    ensure!(sequence.is_empty(), INVALID_SIGNATURE);
    Ok(output)
}

#[cfg(test)]
#[path = "user_app_signature_tests.rs"]
mod tests;
