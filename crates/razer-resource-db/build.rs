use sha2::{Digest as _, Sha256};
use std::{env, fs, path::Path};

#[path = "src/publication.rs"]
mod publication;

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct InstallationPolicy {
    schema_version: u32,
    catalog_source: String,
    catalog_filename: String,
    cache_relative: String,
}

fn stage(directory: &Path, filename: &str, bytes: &[u8]) {
    fs::create_dir_all(directory).expect("Cannot create catalog output directory");
    let target = directory.join(filename);
    if fs::read(&target).ok().as_deref() == Some(bytes) {
        return;
    }
    let staged =
        publication::StagedResource::write(&target, bytes).expect("Cannot stage packaged catalog");
    if let Err(error) = staged.publish() {
        assert!(
            fs::read(&target).ok().as_deref() == Some(bytes),
            "Cannot publish packaged catalog beside Cargo output: {error}"
        );
    }
}

fn main() {
    let manifest = env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest directory is missing");
    let manifest = Path::new(&manifest);
    let workspace = manifest
        .parent()
        .and_then(Path::parent)
        .expect("Invalid workspace layout");
    let policy_path = manifest.join("installation.json");
    let policy: InstallationPolicy =
        serde_json::from_slice(&fs::read(&policy_path).expect("Missing installation policy"))
            .expect("Invalid installation policy");
    assert_eq!(policy.schema_version, 1);
    let source = policy.catalog_source.as_str();
    let filename = policy.catalog_filename.as_str();
    let cache = policy.cache_relative.as_str();
    for value in [source, filename, cache] {
        assert!(
            !value.is_empty()
                && value
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_-/.".contains(&byte))
                && !value.contains(' ')
                && Path::new(value).is_relative()
        );
        assert!(
            value
                .split('/')
                .all(|part| !matches!(part, "" | "." | ".."))
        );
    }
    assert!(!filename.contains('/'));
    let source = workspace.join(source);
    println!("cargo:rerun-if-changed={}", policy_path.display());
    println!("cargo:rerun-if-changed={}", source.display());
    let bytes = fs::read(&source)
        .expect("Missing packaged catalog; run tools/workspace.py prepare-resource-database");
    let digest: [u8; 32] = Sha256::digest(&bytes).into();
    let output = env::var_os("OUT_DIR").expect("Cargo output directory is missing");
    let output = Path::new(&output);
    let constants = format!(
        "pub(crate) const CATALOG_FILENAME: &str = {filename:?};\n\
         pub const CACHE_RELATIVE: &str = {cache:?};\n\
         pub(crate) const CATALOG_LENGTH: u64 = {};\n\
         pub(crate) const CATALOG_SHA256: [u8; 32] = {:?};\n",
        bytes.len(),
        digest
    );
    fs::write(output.join("installation.rs"), constants).expect("Cannot write catalog metadata");
    // OUT_DIR is <target>/<profile>/build/<package>/out, including cross targets.
    let profile = output
        .ancestors()
        .nth(3)
        .expect("Invalid Cargo output layout");
    stage(profile, filename, &bytes);
    stage(&profile.join("deps"), filename, &bytes);
}
