//! Original 164/241 ye/ge duallink-devices JSON semantics adapted to a local
//! document. Only caller-confirmed hardware outcomes may add/remove bindings.
//! Runtime serial state is observation-only and is never persisted as a read.
use anyhow::{Context as _, ensure};
use serde_json::{Map, Value, json};
use std::{collections::BTreeMap, path::Path, sync::OnceLock};

#[derive(Default)]
pub struct PairingCache {
    records: Map<String, Value>,
    disk_snapshot: Option<Map<String, Value>>,
    serials: BTreeMap<String, String>,
    primary: u16,
    secondary: Option<u16>,
}

fn key(dongle_id: u16, container: &str) -> String {
    format!("5426/{dongle_id}/{container}")
}

fn validate_container(container: &str) -> anyhow::Result<()> {
    let inner = container
        .strip_prefix('{')
        .and_then(|value| value.strip_suffix('}'))
        .context("配对缓存缺少完整 ContainerId")?;
    ensure!(
        inner.len() == 36
            && inner
                .bytes()
                .enumerate()
                .all(|(ix, ch)| if [8, 13, 18, 23].contains(&ix) {
                    ch == b'-'
                } else {
                    ch.is_ascii_hexdigit()
                })
            && inner.bytes().any(|ch| ch.is_ascii_hexdigit() && ch != b'0'),
        "配对缓存 ContainerId 无效"
    );
    Ok(())
}

fn master(receiver_pid: u16) -> anyhow::Result<Value> {
    static CATALOG: OnceLock<Value> = OnceLock::new();
    let catalog = CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../assets/data/receiver-pairing-catalog.json"
        ))
        .expect("current receiver pairing catalog")
    });
    catalog["masters"]
        .as_array()
        .context("缺少配对主设备目录")?
        .iter()
        .find(|row| row["productId"] == receiver_pid)
        .cloned()
        .context("主设备没有当前配对存储证据")
}

impl PairingCache {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let previous = super::read_document(path, |text| {
            Ok(serde_json::from_str::<Map<String, Value>>(text)?)
        })?;
        Ok(Self {
            records: previous.clone().unwrap_or_default(),
            disk_snapshot: previous,
            ..Self::default()
        })
    }

    pub fn save(&mut self, path: &Path) -> anyhow::Result<()> {
        let observed = super::read_document(path, |text| {
            Ok(serde_json::from_str::<Map<String, Value>>(text)?)
        })?;
        ensure!(
            observed == self.disk_snapshot,
            "配对存储在操作期间发生变化，未覆盖其他物理设备记录"
        );
        super::write_document(path, &self.records, |previous| {
            let previous: Map<String, Value> = serde_json::from_str(previous)?;
            ensure!(
                Some(&previous) == self.disk_snapshot.as_ref(),
                "配对存储在保存前发生变化"
            );
            Ok(())
        })?;
        self.disk_snapshot = Some(self.records.clone());
        Ok(())
    }

    /// Source ye inserts only an absent key. Existing records are not replaced
    /// by a newer candidate or by local labels. This method makes no hardware
    /// claim: caller must have observed event54 success for this exact owner.
    pub fn add_confirmed(
        &mut self,
        receiver_pid: u16,
        product_id: u32,
        dongle_id: u16,
        category: &str,
        container: &str,
    ) -> anyhow::Result<bool> {
        validate_container(container)?;
        ensure!(
            product_id > 0
                && dongle_id > 0
                && dongle_id != u16::MAX
                && matches!(category, "MOUSE" | "KEYBOARD"),
            "配对缓存缺少真实产品和类别"
        );
        let master = master(receiver_pid)?;
        let device_key = key(dongle_id, container);
        if self.records.contains_key(&device_key) {
            self.primary = receiver_pid;
            self.secondary = Some(dongle_id);
            return Ok(false);
        }
        self.records.insert(device_key, json!({"vendorId":5426,"productId":product_id,"dongleId":dongle_id,"deviceContainerId":container,"category":category,"master":master}));
        self.primary = receiver_pid;
        self.secondary = Some(dongle_id);
        Ok(true)
    }

    /// Source ge deletes only the physical vendor/dongle/container key, then
    /// removes its process-local serial entry. No unrelated receiver is reset.
    pub fn remove_confirmed(&mut self, dongle_id: u16, container: &str) -> anyhow::Result<bool> {
        validate_container(container)?;
        let device_key = key(dongle_id, container);
        self.serials.remove(&device_key);
        let removed = self.records.remove(&device_key).is_some();
        self.primary = 0;
        self.secondary = None;
        Ok(removed)
    }

    /// Matching real DEVICE_RUNTIME_DATA observation, never a generated serial.
    /// Returns affected physical keys for the caller's source runtime publisher.
    pub fn observe_runtime_serial(
        &mut self,
        product_id: u32,
        container: &str,
        serial_number: &str,
    ) -> anyhow::Result<Vec<String>> {
        validate_container(container)?;
        ensure!(
            !serial_number.is_empty() && !serial_number.contains('\0'),
            "runtime 未提供真实序列号"
        );
        let mut matched = Vec::new();
        for (device_key, record) in &self.records {
            if record["productId"] == product_id
                && record["deviceContainerId"]
                    .as_str()
                    .is_some_and(|id| id.eq_ignore_ascii_case(container))
            {
                self.serials
                    .insert(device_key.clone(), serial_number.to_owned());
                matched.push(device_key.clone());
            }
        }
        Ok(matched)
    }

    pub fn records(&self) -> Value {
        Value::Object(self.records.clone())
    }

    pub fn state(&self) -> Value {
        json!({"dualLinkPrimaryPId":self.primary,"dualLinkSecondaryPId":self.secondary.unwrap_or(u16::MAX),"observed_serials":self.serials})
    }
}

pub fn local_path() -> std::path::PathBuf {
    super::store_path().with_file_name("duallink-devices.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    const OWNER: &str = "{11111111-2222-3333-4444-555555555555}";
    #[test]
    fn same_radio_pid_in_different_physical_owners_stays_separate() {
        let mut cache = PairingCache::default();
        cache
            .add_confirmed(241, 716, 713, "KEYBOARD", OWNER)
            .unwrap();
        let other = "{aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee}";
        cache
            .add_confirmed(241, 716, 713, "KEYBOARD", other)
            .unwrap();
        assert_eq!(cache.records().as_object().unwrap().len(), 2);
        assert!(cache.remove_confirmed(713, OWNER).unwrap());
        assert_eq!(cache.records().as_object().unwrap().len(), 1);
        assert!(cache.records().get(key(713, other)).is_some());
    }
    #[test]
    fn source_master_and_serial_observation_keep_defaults_and_scope_explicit() {
        let mut cache = PairingCache::default();
        cache.add_confirmed(164, 170, 171, "MOUSE", OWNER).unwrap();
        let records = cache.records();
        let row = &records[key(171, OWNER)];
        assert_eq!(row["master"]["claimInterface"], 0);
        assert_eq!(row["master"]["bleId"], 0);
        assert!(row["master"].get("supports8KHzPollingRate").is_none());
        assert!(row.get("serialNumber").is_none());
        assert!(
            cache
                .observe_runtime_serial(716, OWNER, "real-serial")
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            cache
                .observe_runtime_serial(170, OWNER, "real-serial")
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            cache.state()["observed_serials"][key(171, OWNER)],
            "real-serial"
        );
    }
}
