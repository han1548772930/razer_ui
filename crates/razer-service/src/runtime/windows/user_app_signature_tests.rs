use super::*;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("testdata/apps-signature")
            .join(name),
    )
    .unwrap()
}

#[test]
fn actual_signatures_allow_source_names_and_digest_branches() {
    for name in [
        "rsa-sha256.ico",
        "rsa-sha1.ico",
        "thx-sha256.ico",
        "ec-sha256.ico",
    ] {
        verify(&fixture(name)).unwrap();
    }
    assert_eq!(
        verify(&fixture("other-signer.ico"))
            .unwrap_err()
            .to_string(),
        NOT_ALLOWED
    );
}

#[test]
fn content_message_and_signature_tampering_are_independently_rejected() {
    let bytes = fixture("rsa-sha256.ico");
    let image_rows = images(&bytes).unwrap();
    let chunk = signature_chunk(&bytes, &image_rows).unwrap();
    let signed = Signed::parse(&bytes[chunk.payload.clone()]).unwrap();
    let offset = |slice: &[u8]| slice.as_ptr() as usize - bytes.as_ptr() as usize;
    for position in [
        bytes.len() - 8,
        offset(signed.expected_content),
        offset(signed.expected_message),
    ] {
        let mut invalid = bytes.clone();
        invalid[position] ^= 1;
        assert_eq!(verify(&invalid).unwrap_err().to_string(), INVALID_SIGNATURE);
    }
    for (name, reason) in [
        ("rsa-sha256.ico", NOT_ALLOWED),
        ("thx-sha256.ico", INVALID_SIGNATURE),
    ] {
        let mut bytes = fixture(name);
        let rows = images(&bytes).unwrap();
        let chunk = signature_chunk(&bytes, &rows).unwrap();
        let signed = Signed::parse(&bytes[chunk.payload]).unwrap();
        let position = signed.signature.as_ptr() as usize - bytes.as_ptr() as usize;
        bytes[position] ^= 1;
        assert_eq!(verify(&bytes).unwrap_err().to_string(), reason);
    }
}

#[test]
fn truncated_ico_der_and_invalid_lengths_never_escape_bounds() {
    let bytes = fixture("rsa-sha256.ico");
    for end in 0..bytes.len() {
        assert!(verify(&bytes[..end]).is_err());
    }
    let mut invalid = bytes.clone();
    invalid[14..18].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(verify(&invalid).is_err());
    let rows = images(&bytes).unwrap();
    let chunk = signature_chunk(&bytes, &rows).unwrap();
    for value in [0x80, 0x84, 0xff] {
        let mut invalid = bytes.clone();
        invalid[chunk.payload.start + 1] = value;
        assert!(verify(&invalid).is_err());
    }
}

#[test]
fn ec_der_integers_keep_fixed_width_and_reject_negative_values() {
    assert_eq!(
        ecdsa_signature(&[0x30, 7, 2, 2, 0, 128, 2, 1, 1], 2).unwrap(),
        [0, 128, 0, 1]
    );
    assert!(ecdsa_signature(&[0x30, 6, 2, 1, 128, 2, 1, 1], 2).is_err());
    assert!(ecdsa_signature(&[0x30, 7, 2, 2, 1, 128, 2, 1, 1], 1).is_err());
}
