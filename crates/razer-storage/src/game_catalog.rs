//! Read-only current GMS3 local catalog format. The installed encrypted XML
//! uses IoUtil.CreateAes and CryptoXmlSerializer; no service, cloud identity or
//! vendor code is needed to read it. This module never modifies vendor files.
use aes::{Aes256, cipher::{BlockDecrypt, KeyInit, generic_array::GenericArray}};
use anyhow::{Context, Result, ensure};
use base64::Engine as _;
use std::{fs::File, io::Read, path::Path};

const MAX_CATALOG_BYTES: usize = 64 * 1024 * 1024;
const EMPTY_GUID: &str = "00000000-0000-0000-0000-000000000000";
// Current installed GameManagerServiceLib IoUtil.CreateAes file-format keys.
// These are catalog-format constants, not user credentials or transport keys.
const KEY: &str = "rDhJZhjM/6cKT1IqcFFu9pJdbUFNnfr4ESynV9e3oRY=";
const IV: &str = "jmh7bP2j7I7vvblKGzbRfA==";

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GameSignature {
    entry_guid: String,
    display_name: String,
    default_exe: String,
    default_exe32: String,
    default_exe64: String,
    default_folder: String,
    default_folder32: String,
    alternative_exe: String,
    default_folder_exe: String,
    default_folder_exe32: String,
    running_exe: String,
    launch_exe: String,
    launch_helper: String,
    launch_helper_excludables: String,
    overlay_tracking: String,
    source_ex: String,
}

macro_rules! readers {
    ($($field:ident),* $(,)?) => {$(
        pub fn $field(&self) -> &str { &self.$field }
    )*};
}

impl GameSignature {
    readers!(entry_guid, display_name, default_folder, default_folder32, alternative_exe, default_folder_exe,
        default_folder_exe32, running_exe, launch_exe, launch_helper,
        launch_helper_excludables, overlay_tracking, source_ex);

    pub fn default_executables(&self) -> [&str; 3] {
        [&self.default_exe, &self.default_exe32, &self.default_exe64]
    }

    pub fn has_identity(&self) -> bool {
        !self.entry_guid.is_empty() && self.entry_guid != EMPTY_GUID
    }

    /// App-owned local profile associations have an explicit local identity;
    /// they are never presented as vendor catalog entries or service replies.
    pub fn linked(display_name: &str, executable: &str) -> Result<Self> {
        ensure!(!executable.is_empty(), "Linked executable is empty");
        let filename = executable.rsplit(['\\', '/']).next().unwrap_or_default();
        ensure!(!filename.is_empty(), "Linked executable has no filename");
        let digest = md5::compute(executable.replace('/', "\\").to_uppercase().as_bytes());
        Ok(Self {
            entry_guid: format!("local-executable:{digest:x}"),
            display_name: display_name.into(),
            default_exe: filename.into(),
            launch_exe: executable.into(),
            ..Default::default()
        })
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GameCatalog {
    signatures: Vec<GameSignature>,
}

impl GameCatalog {
    pub fn signatures(&self) -> &[GameSignature] { &self.signatures }
    pub fn into_signatures(self) -> Vec<GameSignature> { self.signatures }
}

/// The MD5 is the original local filename convention, not a compatibility or
/// integrity gate. .NET ASCIIEncoding replaces each non-ASCII UTF-16 unit.
pub fn machine_catalog_id(machine_name: &str) -> String {
    let input: Vec<u8> = machine_name.encode_utf16()
        .map(|unit| if unit < 128 { unit as u8 } else { b'?' })
        .chain(b"Value_Not_Found".iter().copied())
        .collect();
    format!("{:X}", md5::compute(input))
}

pub fn read_game_catalog(path: &Path) -> Result<Option<GameCatalog>> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).with_context(|| format!("Open local catalog {}", path.display())),
    };
    ensure!(file.metadata()?.len() <= MAX_CATALOG_BYTES as u64, "Local catalog is too large");
    let mut bytes = Vec::new();
    file.take((MAX_CATALOG_BYTES + 1) as u64).read_to_end(&mut bytes)?;
    decode_game_catalog(&bytes).with_context(|| format!("Decode local catalog {}", path.display())).map(Some)
}

/// Current GameManagerServiceLib.Resources.InternalScanList. Kept as product
/// input, independently of static-analysis output and runtime installation.
pub fn internal_scan_list() -> Result<GameCatalog> {
    decode_game_catalog(include_bytes!("game_catalog/internal_scan_list.xml"))
}

pub fn decode_game_catalog(bytes: &[u8]) -> Result<GameCatalog> {
    ensure!(bytes.len() <= MAX_CATALOG_BYTES, "Local catalog is too large");
    ensure!(!bytes.is_empty(), "Local catalog is empty");
    let plaintext;
    let bytes = if bytes.starts_with(b"\xef\xbb\xbf") || bytes.starts_with(b"\xff\xfe")
        || bytes.starts_with(b"\xfe\xff") || bytes.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'<') {
        bytes
    } else {
        plaintext = decrypt(bytes)?;
        &plaintext
    };
    let text = decode_text(bytes)?;
    let document = roxmltree::Document::parse_with_options(&text, roxmltree::ParsingOptions {
        nodes_limit: 2_000_000,
        ..Default::default()
    }).context("Invalid game catalog XML")?;
    let root = document.root_element();
    ensure!(root.tag_name().name() == "GameSignatureList", "Unexpected game catalog root");
    let list = root.children().find(|n| n.is_element() && n.tag_name().name() == "GameSignatureList")
        .context("Game catalog has no signature list")?;
    let mut signatures = Vec::new();
    for node in list.children().filter(|n| n.is_element() && n.tag_name().name() == "GameSignature") {
        ensure!(signatures.len() < 100_000, "Too many game signatures");
        let field = |name| node.children().find(|n| n.is_element() && n.tag_name().name() == name)
            .and_then(|n| n.text()).unwrap_or_default().to_owned();
        let guid = field("EntryGUID");
        let entry_guid = if guid.is_empty() { EMPTY_GUID.into() } else { canonical_guid(&guid)? };
        signatures.push(GameSignature {
            entry_guid,
            display_name: field("DisplayName"),
            default_exe: field("DefaultExe"),
            default_exe32: field("DefaultExe32"),
            default_exe64: field("DefaultExe64"),
            default_folder: field("DefaultFolder"),
            default_folder32: field("DefaultFolder32"),
            alternative_exe: field("AlternativeExe"),
            default_folder_exe: field("DefaultFolderExe"),
            default_folder_exe32: field("DefaultFolderExe32"),
            running_exe: field("RunningExe"),
            launch_exe: field("LaunchExe"),
            launch_helper: field("LaunchHelper"),
            launch_helper_excludables: field("LaunchHelperExcludables"),
            overlay_tracking: field("OverlayTrackingOption01"),
            source_ex: field("SourceEx"),
        });
    }
    Ok(GameCatalog { signatures })
}

fn canonical_guid(text: &str) -> Result<String> {
    let bytes = text.as_bytes();
    ensure!(bytes.len() == 36 && bytes.iter().enumerate().all(|(index, byte)| {
        if [8, 13, 18, 23].contains(&index) { *byte == b'-' } else { byte.is_ascii_hexdigit() }
    }), "Invalid game entry GUID");
    Ok(text.to_ascii_lowercase())
}

fn decode_text(bytes: &[u8]) -> Result<String> {
    if bytes.starts_with(b"\xff\xfe") || bytes.starts_with(b"\xfe\xff") {
        ensure!(bytes.len() % 2 == 0, "Truncated UTF-16 catalog");
        let little = bytes[0] == 0xff;
        let units = bytes[2..].chunks_exact(2).map(|b| {
            if little { u16::from_le_bytes([b[0], b[1]]) } else { u16::from_be_bytes([b[0], b[1]]) }
        }).collect::<Vec<_>>();
        String::from_utf16(&units).context("Invalid UTF-16 game catalog")
    } else {
        String::from_utf8(bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes).to_vec())
            .context("Invalid UTF-8 game catalog")
    }
}

fn decrypt(bytes: &[u8]) -> Result<Vec<u8>> {
    let key = base64::engine::general_purpose::STANDARD.decode(KEY)?;
    let iv = base64::engine::general_purpose::STANDARD.decode(IV)?;
    decrypt_cbc(bytes, &key, &iv)
}

fn decrypt_cbc(bytes: &[u8], key: &[u8], iv: &[u8]) -> Result<Vec<u8>> {
    ensure!(!bytes.is_empty() && bytes.len() % 16 == 0, "Truncated encrypted game catalog");
    ensure!(iv.len() == 16, "Invalid catalog IV length");
    let cipher = Aes256::new_from_slice(key).map_err(|_| anyhow::anyhow!("Invalid catalog key length"))?;
    let mut previous = iv.to_vec();
    let mut output = Vec::with_capacity(bytes.len());
    for input in bytes.chunks_exact(16) {
        let mut block = GenericArray::clone_from_slice(input);
        cipher.decrypt_block(&mut block);
        output.extend(block.iter().zip(&previous).map(|(a, b)| a ^ b));
        previous.copy_from_slice(input);
    }
    let padding = usize::from(*output.last().context("Missing catalog padding")?);
    ensure!(padding > 0 && padding <= 16 && output[output.len() - padding..].iter().all(|b| usize::from(*b) == padding), "Invalid game catalog PKCS7 padding");
    output.truncate(output.len() - padding);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_decodes_both_original_file_formats_and_preserves_matching_fields() {
        let plain = decode_game_catalog(include_bytes!("../testdata/games/current-game-list.xml")).unwrap();
        let encrypted = decode_game_catalog(include_bytes!("../testdata/games/current-game-list.enc")).unwrap();
        assert_eq!(plain, encrypted);
        assert_eq!(plain.signatures().len(), 3);
        let game = &plain.signatures()[0];
        assert_eq!(game.entry_guid(), "11111111-2222-3333-4444-555555555555");
        assert_eq!(game.display_name(), "Synthetic & Local");
        assert_eq!(game.default_executables(), ["game.exe", "game32.exe", "game64.exe"]);
        assert_eq!(game.overlay_tracking(), r"C:\Games\Synthetic\overlay.exe; C:\Games\Synthetic\game.exe");
        assert!(!plain.signatures()[2].has_identity());
    }

    #[test]
    fn corrupt_encrypted_and_invalid_xml_catalogs_are_errors_not_empty_lists() {
        let encrypted = include_bytes!("../testdata/games/current-game-list.enc");
        assert!(decode_game_catalog(&encrypted[..encrypted.len() - 1]).is_err());
        let mut damaged = encrypted.to_vec();
        let end = damaged.len();
        damaged[end - 17] ^= 0x80;
        assert!(decode_game_catalog(&damaged).is_err());
        for invalid in [b"".as_slice(), b"<wrong/>", b"<GameSignatureList/>", b"<!DOCTYPE a [<!ENTITY x 'expansion'>]><GameSignatureList>&x;</GameSignatureList>"] {
            assert!(decode_game_catalog(invalid).is_err());
        }
        let invalid = include_str!("../testdata/games/current-game-list.xml").replace("11111111-2222-3333-4444-555555555555", "not-a-guid");
        assert!(decode_game_catalog(invalid.as_bytes()).is_err());
    }

    #[test]
    fn streamreader_boms_and_prefixed_data_contract_elements_decode() {
        let xml = "<g:GameSignatureList xmlns:g=''><g:GameSignatureList><g:GameSignature><g:EntryGUID>AAAAAAAA-BBBB-CCCC-DDDD-EEEEEEEEEEEE</g:EntryGUID><g:DisplayName>本地</g:DisplayName></g:GameSignature></g:GameSignatureList></g:GameSignatureList>";
        // XML prefixes require a nonempty namespace; DataContract local names
        // remain the same when the serializer supplies one.
        let xml = xml.replace("xmlns:g=''", "xmlns:g='urn:synthetic'");
        let mut little = vec![0xff, 0xfe];
        let mut big = vec![0xfe, 0xff];
        for unit in xml.encode_utf16() {
            little.extend(unit.to_le_bytes());
            big.extend(unit.to_be_bytes());
        }
        for bytes in [&little, &big] {
            let catalog = decode_game_catalog(bytes).unwrap();
            assert_eq!(catalog.signatures()[0].display_name(), "本地");
            assert_eq!(catalog.signatures()[0].entry_guid(), "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee");
        }
    }

    #[test]
    fn local_identity_and_machine_filenames_have_stable_format_separate_from_vendor_guids() {
        let first = GameSignature::linked("A", r"C:\Games\Game.EXE").unwrap();
        let same = GameSignature::linked("renamed", "c:/games/game.exe").unwrap();
        assert_eq!(first.entry_guid(), same.entry_guid());
        assert!(first.entry_guid().starts_with("local-executable:"));
        // Independent System.Security.Cryptography.MD5 result for this ASCII name.
        assert_eq!(machine_catalog_id("SYNTHETIC-PC"), "99FB7B7D21453AEC95311D94CD58171F");
    }
}
