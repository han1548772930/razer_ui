//! Original 164/241 ye/ge duallink-devices JSON semantics adapted to a local
//! document. Only caller-confirmed hardware outcomes may add/remove bindings.
//! Runtime serial state is observation-only and is never persisted as a read.
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::{MapAccess, Visitor}, ser::SerializeMap};
use serde_json::{Map, Value, json};
use std::{collections::BTreeMap, path::Path, sync::OnceLock};

#[derive(Default)]
pub struct PairingCache {
    records: Map<String, Value>,
    order: Vec<String>,
    disk_snapshot: Option<Map<String, Value>>,
    disk_order: Option<Vec<String>>,
    serials: BTreeMap<String, String>,
    primary: u16,
    secondary: Option<u16>,
}

// JS Object.values visits non-index keys in insertion order. serde_json's
// default Map sorts keys, which changes UZ/J's first matching role decision.
#[derive(Default)]
struct OrderedRecords { records: Map<String, Value>, order: Vec<String> }
impl<'de> Deserialize<'de> for OrderedRecords {
    fn deserialize<D: Deserializer<'de>>(deserializer:D)->Result<Self,D::Error> {
        struct RecordsVisitor;
        impl<'de> Visitor<'de> for RecordsVisitor {
            type Value=OrderedRecords;
            fn expecting(&self,formatter:&mut std::fmt::Formatter<'_>)->std::fmt::Result {formatter.write_str("source duallink-devices object")}
            fn visit_map<A:MapAccess<'de>>(self,mut map:A)->Result<Self::Value,A::Error> {
                let mut value=OrderedRecords::default();
                while let Some((key,row))=map.next_entry::<String,Value>()? {
                    if !value.records.contains_key(&key) {value.order.push(key.clone());}
                    value.records.insert(key,row);
                }
                // Integer array-index properties precede other string keys.
                fn index(key:&str)->Option<u32> {key.parse::<u32>().ok().filter(|index|*index<u32::MAX&&index.to_string()==key)}
                value.order.sort_by(|a,b|match(index(a),index(b)) {
                    (Some(a),Some(b))=>a.cmp(&b),(Some(_),None)=>std::cmp::Ordering::Less,
                    (None,Some(_))=>std::cmp::Ordering::Greater,(None,None)=>std::cmp::Ordering::Equal,
                });
                Ok(value)
            }
        }
        deserializer.deserialize_map(RecordsVisitor)
    }
}
struct OrderedView<'a> {records:&'a Map<String,Value>,order:&'a [String]}
impl Serialize for OrderedView<'_> {
    fn serialize<S:Serializer>(&self,serializer:S)->Result<S::Ok,S::Error> {
        let mut map=serializer.serialize_map(Some(self.records.len()))?;
        for key in self.order {if let Some(value)=self.records.get(key) {map.serialize_entry(key,value)?;}}
        map.end()
    }
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

fn catalog() -> &'static Value {
    static CATALOG: OnceLock<Value> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../assets/data/receiver-pairing-catalog.json"
        ))
        .expect("current receiver pairing catalog")
    })
}

fn master(receiver_pid: u16) -> anyhow::Result<Value> {
    catalog()["masters"]
        .as_array()
        .context("缺少配对主设备目录")?
        .iter()
        .find(|row| row["productId"] == receiver_pid || row["dongleId"] == receiver_pid)
        .cloned()
        .context("主设备没有当前配对存储证据")
}

impl PairingCache {
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let previous = super::read_document(path, |text| {
            Ok(serde_json::from_str::<OrderedRecords>(text)?)
        })?;
        Ok(Self {
            records: previous.as_ref().map(|value|value.records.clone()).unwrap_or_default(),
            order: previous.as_ref().map(|value|value.order.clone()).unwrap_or_default(),
            disk_snapshot: previous.as_ref().map(|value|value.records.clone()),
            disk_order: previous.map(|value|value.order),
            ..Self::default()
        })
    }

    pub fn save(&mut self, path: &Path) -> anyhow::Result<()> {
        let observed = super::read_document(path, |text| {
            Ok(serde_json::from_str::<OrderedRecords>(text)?)
        })?;
        ensure!(
            observed.as_ref().map(|value|&value.records) == self.disk_snapshot.as_ref()
                && observed.as_ref().map(|value|&value.order) == self.disk_order.as_ref(),
            "配对存储在操作期间发生变化，未覆盖其他物理设备记录"
        );
        super::write_document(path, &OrderedView{records:&self.records,order:&self.order}, |previous| {
            let previous: OrderedRecords = serde_json::from_str(previous)?;
            ensure!(
                Some(&previous.records) == self.disk_snapshot.as_ref()
                    && Some(&previous.order) == self.disk_order.as_ref(),
                "配对存储在保存前发生变化"
            );
            Ok(())
        })?;
        self.disk_snapshot = Some(self.records.clone());
        self.disk_order = Some(self.order.clone());
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
        self.add_confirmed_for_source(
            receiver_pid,
            product_id,
            dongle_id,
            category,
            container,
            None,
        )
    }

    /// Current756 X stores the original scan productId, including zero for a
    /// catalog radio without DeviceSiblings. Its master remains DeviceInfo756
    /// when the actual command uses a cached physical master. The caller must
    /// have confirmed event54 and the source callback's catalog resolution.
    pub fn add_confirmed_for_source(
        &mut self,
        receiver_pid: u16,
        product_id: u32,
        dongle_id: u16,
        category: &str,
        container: &str,
        source_product_id: Option<u16>,
    ) -> anyhow::Result<bool> {
        validate_container(container)?;
        let source756 = source_product_id == Some(756);
        ensure!(
            (product_id > 0 || source756)
                && dongle_id > 0
                && dongle_id != u16::MAX
                && matches!(category, "MOUSE" | "KEYBOARD"),
            "配对缓存缺少真实产品和类别"
        );
        let mut master = master(receiver_pid)?;
        if source756 {
            ensure!(category == "MOUSE", "756 配对缓存类别与原 factory 不符");
            ensure!(
                catalog()["devices"]
                    .as_array()
                    .is_some_and(|rows| rows.iter().any(|row| row["DeviceDonglePid"] == dongle_id)),
                "756 配对回调没有解析当前目录 radio，不能保存源缓存"
            );
            master = self::master(756)?;
        }
        // Source X/ye stores identity and the factory interface only. UI
        // canPairTwoDevices belongs to the source DeviceInfo catalog.
        master.as_object_mut().unwrap().remove("canPairTwoDevices");
        let device_key = key(dongle_id, container);
        if self.records.contains_key(&device_key) {
            self.primary = receiver_pid;
            self.secondary = Some(dongle_id);
            return Ok(false);
        }
        self.order.push(device_key.clone());
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
        self.order.retain(|key|key!=&device_key);
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

    pub fn ordered_records(&self) -> Vec<Value> {
        self.order.iter().filter_map(|key|self.records.get(key).cloned()).collect()
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
    fn source_object_values_order_survives_disk_roundtrip_and_detects_reordering() {
        let path = std::env::temp_dir().join(format!("razer-pairing-order-{}-{}.json",std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        let original = r#"{"z":{"dongleId":193},"10":{"dongleId":10},"2":{"dongleId":2},"a":{"dongleId":757},"z":{"dongleId":195}}"#;
        std::fs::write(&path,original).unwrap();
        let mut cache = PairingCache::load(&path).unwrap();
        assert_eq!(cache.ordered_records(),vec![json!({"dongleId":2}),json!({"dongleId":10}),json!({"dongleId":195}),json!({"dongleId":757})]);
        cache.save(&path).unwrap();
        let mut loaded = PairingCache::load(&path).unwrap();
        assert_eq!(loaded.ordered_records(),cache.ordered_records());
        // Equal object contents with a different insertion order affect the
        // first matching source role and must count as an external mutation.
        std::fs::write(&path,r#"{"2":{"dongleId":2},"10":{"dongleId":10},"a":{"dongleId":757},"z":{"dongleId":195}}"#).unwrap();
        assert!(loaded.save(&path).is_err());
        // write_document retains a deliberate previous-revision backup.
        // This fixture owns both files, so it must remove both on completion.
        std::fs::remove_file(path.with_extension("json.bak")).unwrap();
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn source756_cached_master_saves_scan_product_instead_of_ui_product_zero() {
        let mut cache = PairingCache::default();
        cache
            .add_confirmed_for_source(241, 181, 180, "MOUSE", OWNER, Some(756))
            .unwrap();
        let records = cache.records();
        let row = &records[key(180, OWNER)];
        assert_eq!(row["productId"], 181);
        assert_eq!(
            row["master"],
            json!({"productId":756,"dongleId":757,"bleId":758,"claimInterface":3})
        );
        assert_eq!(cache.state()["dualLinkPrimaryPId"], 241);
        assert_eq!(cache.state()["dualLinkSecondaryPId"], 180);
    }

    #[test]
    fn source756_zero_scan_product_requires_current_catalog_and_mouse_factory() {
        let mut cache = PairingCache::default();
        assert!(cache.add_confirmed(757, 0, 164, "MOUSE", OWNER).is_err());
        assert!(
            cache
                .add_confirmed_for_source(757, 0, 60000, "MOUSE", OWNER, Some(756))
                .is_err()
        );
        assert!(
            cache
                .add_confirmed_for_source(757, 0, 164, "KEYBOARD", OWNER, Some(756))
                .is_err()
        );
        assert!(cache.records().as_object().unwrap().is_empty());
        cache
            .add_confirmed_for_source(757, 0, 164, "MOUSE", OWNER, Some(756))
            .unwrap();
        assert_eq!(cache.records()[key(164, OWNER)]["productId"], 0);
        assert_eq!(cache.records()[key(164, OWNER)]["master"]["productId"], 756);
    }
    #[test]
    fn source756_physical757_keeps_logical_master_and_physical_primary() {
        let mut cache = PairingCache::default();
        cache.add_confirmed(757, 182, 183, "MOUSE", OWNER).unwrap();
        let row = &cache.records()[key(183, OWNER)];
        assert_eq!(
            row["master"],
            json!({"productId":756,"dongleId":757,"bleId":758,"claimInterface":3})
        );
        assert_eq!(cache.state()["dualLinkPrimaryPId"], 757);
        assert_eq!(cache.state()["dualLinkSecondaryPId"], 183);
    }
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
