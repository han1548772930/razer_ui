//! Existing callers use the verified per-file store; no pack is opened or staged.
pub(super) fn validate() -> anyhow::Result<()> {
    super::remote::manifest()?;
    Ok(())
}

pub(super) fn load(key: &str) -> anyhow::Result<Option<Vec<u8>>> {
    super::remote::load(key)
}
