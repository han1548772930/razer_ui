//! Current756 module32637 analog-restore-v1 journal, stored before AT/RT writes.
//! Malformed truthy records remain pending and prevent adopting partial device
//! values; completion compares the token before deleting the record.
use anyhow::ensure;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Pending {
    pub guid: String,
    pub token: String,
}
pub struct Journal {
    path: PathBuf,
    device_key: String,
}
impl Journal {
    pub fn new(path: &Path, product_id: u32, serial: &str) -> anyhow::Result<Self> {
        ensure!(
            product_id == 756 && !serial.is_empty() && !serial.contains('\0'),
            "Cannot journal analog restore without an observed device serial number"
        );
        Ok(Self {
            path: path.into(),
            device_key: format!("{product_id}:{serial}"),
        })
    }
    fn key(&self, slot: u8) -> anyhow::Result<String> {
        ensure!(slot > 0, "Analog restore slot is invalid");
        Ok(format!("analog-restore-v1:{}:{slot}", self.device_key))
    }
    pub fn pending(&self, slot: u8) -> anyhow::Result<Option<Pending>> {
        let Some(raw) = crate::source_local::get_raw(&self.path, &self.key(slot)?)? else {
            return Ok(None);
        };
        if raw.is_empty() {
            return Ok(None);
        }
        Ok(Some(serde_json::from_str(&raw).unwrap_or_default()))
    }
    pub fn begin(&self, slot: u8, guid: &str, token: &str) -> anyhow::Result<()> {
        ensure!(
            !guid.is_empty() && !token.is_empty(),
            "Analog restore GUID/token is missing"
        );
        crate::source_local::set(
            &self.path,
            &self.key(slot)?,
            &serde_json::to_value(Pending {
                guid: guid.into(),
                token: token.into(),
            })?,
        )
    }
    pub fn complete(&self, slot: u8, token: &str) -> anyhow::Result<bool> {
        ensure!(
            !token.is_empty(),
            "Analog restore completion token is missing"
        );
        crate::source_local::remove_raw_if(&self.path, &self.key(slot)?, |raw| {
            Ok(serde_json::from_str::<Pending>(raw).is_ok_and(|pending| pending.token == token))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interrupted_restore_and_stale_completion_preserve_the_correct_pending_owner() {
        let path = std::env::temp_dir().join(format!(
            "razer-analog-journal-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let journal = Journal::new(&path, 756, "REAL-SERIAL").unwrap();
        crate::source_local::set(&path, "other", &serde_json::json!({"retained":true})).unwrap();
        journal.begin(1, "first", "1:random-a").unwrap();
        journal.begin(1, "second", "2:random-b").unwrap();
        assert!(!journal.complete(1, "1:random-a").unwrap());
        assert_eq!(journal.pending(1).unwrap().unwrap().guid, "second");
        assert!(journal.complete(1, "2:random-b").unwrap());
        assert_eq!(journal.pending(1).unwrap(), None);
        crate::source_local::set(
            &path,
            &journal.key(2).unwrap(),
            &serde_json::json!({"invalid":true}),
        )
        .unwrap();
        assert_eq!(journal.pending(2).unwrap(), Some(Pending::default()));
        assert!(!journal.complete(2, "other").unwrap());
        assert_eq!(
            crate::source_local::get(&path, "other").unwrap(),
            Some(serde_json::json!({"retained":true}))
        );
        assert!(Journal::new(&path, 756, "").is_err());
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_file(path.with_extension("json.lock")).unwrap();
    }
}
