use std::{
    env,
    fs::{self, File, OpenOptions},
    io::{self, Read},
    path::{Path, PathBuf},
};

fn identical(source: &Path, destination: &Path) -> io::Result<bool> {
    let mut expected = File::open(source)?;
    let mut actual = match File::open(destination) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    let mut remaining = expected.metadata()?.len();
    if remaining != actual.metadata()?.len() {
        return Ok(false);
    }
    let mut expected_buffer = [0u8; 65536];
    let mut actual_buffer = [0u8; 65536];
    while remaining != 0 {
        let length = remaining.min(expected_buffer.len() as u64) as usize;
        expected.read_exact(&mut expected_buffer[..length])?;
        actual.read_exact(&mut actual_buffer[..length])?;
        if expected_buffer[..length] != actual_buffer[..length] {
            return Ok(false);
        }
        remaining -= length as u64;
    }
    Ok(true)
}

fn stage_pack() -> Result<(), Box<dyn std::error::Error>> {
    let source = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").ok_or("Missing crate directory")?)
        .join("../../target/asset-pack/synapse.pack");
    println!("cargo:rerun-if-changed={}", source.display());
    if !source.is_file() {
        return Err("Missing target/asset-pack/synapse.pack; prepare the local product resources with python tools/workspace.py prepare-asset-pack before compiling".into());
    }
    let output = PathBuf::from(env::var_os("OUT_DIR").ok_or("Missing Cargo output directory")?);
    let profile = output
        .ancestors()
        .nth(3)
        .ok_or("Invalid Cargo output directory")?;
    let directory = profile.join("assets");
    fs::create_dir_all(&directory)?;
    let destination = directory.join("synapse.pack");
    if identical(&source, &destination)? {
        return Ok(());
    }
    let temporary = directory.join(format!("synapse.pack.{}.tmp", std::process::id()));
    let staged = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = (|| -> io::Result<()> {
        let mut input = File::open(&source)?;
        let mut output = staged;
        let expected_length = input.metadata()?.len();
        let copied = io::copy(&mut input, &mut output)?;
        if copied != expected_length {
            return Err(io::Error::other("Product asset pack changed while staging"));
        }
        output.sync_all()?;
        drop(output);
        if !identical(&source, &temporary)? {
            return Err(io::Error::other("Product asset pack changed while staging"));
        }
        fs::rename(&temporary, &destination)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result?;
    Ok(())
}

fn main() {
    if let Err(error) = stage_pack() {
        panic!("Cannot stage the product asset pack: {error}");
    }
}
