//! Macro XML file boundaries reuse the local transfer lifecycle owner.
use crate::file_transfer::Publication;
pub use crate::file_transfer::{Operation, Transfer, active_io};
use razer_model::macro_library::{Entry, xml};
use std::path::Path;

impl Operation {
    pub fn read_xml(&self, path: &Path) -> Result<Entry, String> {
        self.check()?;
        if !path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("xml"))
        {
            return Err("请选择宏 XML 文件。".into());
        }
        let bytes = self.read(path, xml::MAX_BYTES)?;
        let text = std::str::from_utf8(&bytes).map_err(|_| "宏 XML 不是有效的 UTF-8 文件。")?;
        let entry = xml::decode(text.trim_start_matches('\u{feff}'))?;
        self.check()?;
        Ok(entry)
    }

    pub fn write_xml(&self, path: &Path, text: &str) -> Result<(), String> {
        self.write(path, text.as_bytes(), xml::MAX_BYTES, Publication::Replace)
    }
}

#[cfg(test)]
mod tests;
