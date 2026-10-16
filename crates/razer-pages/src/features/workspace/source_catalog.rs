//! Index source-derived constructors without allocating unused product state.
//! Each typed product is constructed and retained on its first request.
use std::{collections::BTreeMap, sync::OnceLock};

struct Record<T> {
    factory: fn() -> T,
    decoded: OnceLock<T>,
}

pub(super) struct SourceCatalog<T> {
    factories: fn() -> Vec<(u32, fn() -> T)>,
    index: OnceLock<BTreeMap<u32, Record<T>>>,
}

impl<T> SourceCatalog<T> {
    pub(super) const fn new(factories: fn() -> Vec<(u32, fn() -> T)>) -> Self {
        Self {
            factories,
            index: OnceLock::new(),
        }
    }

    pub(super) fn get(&self, product_id: u32) -> Option<&T> {
        let index = self.index.get_or_init(|| {
            let mut index = BTreeMap::new();
            for (product_id, factory) in (self.factories)() {
                index.entry(product_id).or_insert_with(|| Record {
                    factory,
                    decoded: OnceLock::new(),
                });
            }
            index
        });
        let record = index.get(&product_id)?;
        Some(record.decoded.get_or_init(record.factory))
    }
}

#[cfg(test)]
mod tests {
    use super::SourceCatalog;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn source_catalog_constructs_only_requested_products_and_retains_the_same_owner() {
        static CONSTRUCTED: AtomicUsize = AtomicUsize::new(0);
        #[derive(Debug, PartialEq)]
        struct Spec {
            product_id: u32,
            payload: String,
        }
        fn first() -> Spec {
            CONSTRUCTED.fetch_add(1, Ordering::SeqCst);
            Spec {
                product_id: 579,
                payload: "first".into(),
            }
        }
        fn second() -> Spec {
            CONSTRUCTED.fetch_add(1, Ordering::SeqCst);
            Spec {
                product_id: 720,
                payload: "second".into(),
            }
        }
        let catalog = SourceCatalog::new(|| vec![(579, first), (720, second)]);
        assert_eq!(CONSTRUCTED.load(Ordering::SeqCst), 0);
        assert!(catalog.get(182).is_none());
        assert_eq!(CONSTRUCTED.load(Ordering::SeqCst), 0);
        let first = catalog.get(579).unwrap();
        assert_eq!(first.payload, "first");
        assert_eq!(first.product_id, 579);
        assert_eq!(CONSTRUCTED.load(Ordering::SeqCst), 1);
        assert!(std::ptr::eq(first, catalog.get(579).unwrap()));
        assert_eq!(CONSTRUCTED.load(Ordering::SeqCst), 1);
        assert_eq!(catalog.get(720).unwrap().payload, "second");
        assert_eq!(CONSTRUCTED.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn source_catalog_preserves_source_identities_in_all_six_product_families() {
        mod fixtures {
            include!("../../../testdata/catalog_cases.rs");
        }
        let family = |name: &str| {
            fixtures::PRODUCT_IDENTITIES
                .iter()
                .find(|(family, _)| *family == name)
                .unwrap()
                .1
        };
        fn verify_family<T: 'static>(expected: &[u32], lookup: fn(u32) -> Option<&'static T>) {
            let mut ids = std::collections::BTreeSet::new();
            for &id in expected {
                assert!(ids.insert(id), "duplicate source product {id}");
                let product = lookup(id).expect("source product factory");
                assert!(std::ptr::eq(product, lookup(id).unwrap()));
            }
            assert!(lookup(u32::MAX).is_none());
        }
        use crate::features::{
            accessory_system_products, audio_products, gamepad_products, keyboard_products,
            mouse_products, system_products,
        };
        verify_family(family("mouse"), mouse_products::source_product);
        verify_family(family("keyboard"), keyboard_products::source_product);
        verify_family(family("audio"), audio_products::source_product);
        verify_family(family("gamepad"), gamepad_products::source_product);
        verify_family(family("system"), system_products::source_product);
        verify_family(
            family("accessories"),
            accessory_system_products::source_product,
        );
    }
}
