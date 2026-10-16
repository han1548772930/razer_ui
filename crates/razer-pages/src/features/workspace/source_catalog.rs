//! Index unchanged source records without allocating every product's full
//! specification during startup. Each typed record is retained on first use.
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::value::RawValue;
use std::{collections::BTreeMap, sync::OnceLock};

struct Record<T> {
    source: &'static RawValue,
    decoded: OnceLock<T>,
}

pub(super) struct SourceCatalog<T> {
    source: &'static str,
    index: OnceLock<BTreeMap<u32, Record<T>>>,
}

impl<T: DeserializeOwned> SourceCatalog<T> {
    pub(super) const fn new(source: &'static str) -> Self {
        Self {
            source,
            index: OnceLock::new(),
        }
    }

    pub(super) fn get(&self, product_id: u32) -> Option<&T> {
        let index = self.index.get_or_init(|| {
            #[derive(Deserialize)]
            struct Identity {
                product_id: u32,
            }
            let records: Vec<&RawValue> =
                serde_json::from_str(self.source).expect("validated source product array");
            let mut index = BTreeMap::new();
            for source in records {
                let identity: Identity =
                    serde_json::from_str(source.get()).expect("validated source product identity");
                // Match the old array's first-record selection. The source
                // catalog verification separately rejects duplicate identities.
                index.entry(identity.product_id).or_insert_with(|| Record {
                    source,
                    decoded: OnceLock::new(),
                });
            }
            index
        });
        let record = index.get(&product_id)?;
        Some(record.decoded.get_or_init(|| {
            serde_json::from_str(record.source.get()).unwrap_or_else(|error| {
                panic!("Invalid retained source specification for product {product_id}: {error}")
            })
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::SourceCatalog;
    use serde::{Deserialize, Deserializer};
    use serde_json::Value;
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[test]
    fn source_catalog_decodes_only_requested_products_and_retains_the_same_owner() {
        static DECODED: AtomicUsize = AtomicUsize::new(0);
        #[derive(Debug, PartialEq)]
        struct Spec {
            product_id: u32,
            payload: String,
        }
        impl<'de> Deserialize<'de> for Spec {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                #[derive(Deserialize)]
                struct Data {
                    product_id: u32,
                    payload: String,
                }
                let data = Data::deserialize(deserializer)?;
                DECODED.fetch_add(1, Ordering::SeqCst);
                Ok(Self {
                    product_id: data.product_id,
                    payload: data.payload,
                })
            }
        }
        let catalog = SourceCatalog::<Spec>::new(
            r#"[{"product_id":579,"payload":"first"},
                {"product_id":720,"payload":"second"}]"#,
        );
        assert_eq!(DECODED.load(Ordering::SeqCst), 0);
        assert!(catalog.get(182).is_none());
        assert_eq!(DECODED.load(Ordering::SeqCst), 0);
        let first = catalog.get(579).unwrap();
        assert_eq!(first.payload, "first");
        assert_eq!(DECODED.load(Ordering::SeqCst), 1);
        assert!(std::ptr::eq(first, catalog.get(579).unwrap()));
        assert_eq!(DECODED.load(Ordering::SeqCst), 1);
        assert_eq!(catalog.get(720).unwrap().payload, "second");
        assert_eq!(DECODED.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn source_catalog_preserves_every_record_in_all_six_product_families() {
        fn verify_family<T: 'static>(source: &'static str, lookup: fn(u32) -> Option<&'static T>) {
            let expected: Vec<Value> = serde_json::from_str(source).unwrap();
            let catalog = SourceCatalog::<Value>::new(source);
            let mut ids = std::collections::BTreeSet::new();
            for product in expected {
                let id = u32::try_from(product["product_id"].as_u64().unwrap()).unwrap();
                assert!(ids.insert(id), "duplicate source product {id}");
                assert_eq!(catalog.get(id), Some(&product), "source product {id}");
                let typed = lookup(id).expect("every source record must decode through its owner");
                assert!(std::ptr::eq(typed, lookup(id).unwrap()));
            }
            assert!(catalog.get(u32::MAX).is_none());
            assert!(lookup(u32::MAX).is_none());
        }
        use crate::features::{
            accessory_system_products, audio_products, gamepad_products, keyboard_products,
            mouse_products, system_products,
        };
        verify_family(
            include_str!("../mouse/mouse_products_data.json"),
            mouse_products::source_product,
        );
        verify_family(
            include_str!("../keyboard/keyboard_products_data.json"),
            keyboard_products::source_product,
        );
        verify_family(
            include_str!("../audio/audio_products_data.json"),
            audio_products::source_product,
        );
        verify_family(
            include_str!("../gamepad/gamepad_products_data.json"),
            gamepad_products::source_product,
        );
        verify_family(
            include_str!("../system/system_products_data.json"),
            system_products::source_product,
        );
        verify_family(
            include_str!("../accessories/accessory_system_products_data.json"),
            accessory_system_products::source_product,
        );
    }
}
