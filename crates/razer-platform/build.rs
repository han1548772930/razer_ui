use std::{env, fs, path::Path};

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Installation {
    schema_version: u32,
    hardware_driver: Driver,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Driver {
    service_name: String,
    filename: String,
    directory: String,
    system_directory: String,
    source: String,
    architecture: String,
}

fn main() {
    let manifest = env::var_os("CARGO_MANIFEST_DIR").expect("Missing Cargo manifest directory");
    let root = Path::new(&manifest)
        .parent()
        .and_then(Path::parent)
        .expect("Invalid workspace layout");
    let policy = root.join("packaging/windows/installation.json");
    println!("cargo:rerun-if-changed={}", policy.display());
    let installation: Installation =
        serde_json::from_slice(&fs::read(&policy).expect("Missing Windows installation policy"))
            .expect("Invalid Windows installation policy");
    assert_eq!(installation.schema_version, 1);
    let driver = installation.hardware_driver;
    for value in [
        &driver.service_name,
        &driver.filename,
        &driver.directory,
        &driver.system_directory,
    ] {
        assert!(
            !value.is_empty()
                && value
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
                && !matches!(value.as_str(), "." | ".."),
            "Invalid hardware driver installation name"
        );
    }
    assert!(driver.filename.ends_with(".sys"));
    assert_eq!(driver.architecture, "x64");
    assert!(driver.source.split('/').all(|part| {
        !part.is_empty()
            && part != "."
            && part != ".."
            && part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    }));
    let out = env::var_os("OUT_DIR").expect("Missing Cargo output directory");
    let constants = format!(
        "pub const SERVICE_NAME: &str = {:?};\npub const DRIVER_FILENAME: &str = {:?};\npub const DRIVER_DIRECTORY: &str = {:?};\npub const SYSTEM_DRIVER_DIRECTORY: &str = {:?};\npub const DEVICE_PATH: &str = {:?};\npub const MUTEX_NAME: &str = {:?};\n",
        driver.service_name,
        driver.filename,
        driver.directory,
        driver.system_directory,
        format!(r"\\.\{}", driver.service_name),
        format!(r"Global\{}", driver.service_name)
    );
    fs::write(
        Path::new(&out).join("hardware_driver_installation.rs"),
        constants,
    )
    .expect("Cannot generate driver installation constants");
}
