//! Audited local mapping documents and the owner used to publish them.

/// A product with an audited source document compiler entry. Constructing this type
/// does not establish a connection, registration, or successful publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MappingProduct {
    mounted: u32,
    source: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mounted_identity_and_source_identity_follow_current_device_info() {
        let product = MappingProduct::for_product_id(105).unwrap();
        assert_eq!(product.product_id(), 105);
        assert_eq!(product.source_product_id(), 104);
        assert!(product.is_ordinary_mouse());
        assert_eq!(
            product.storage_field(),
            MappingProduct::for_product_id(104).unwrap().storage_field()
        );
        let original = crate::device_identity::middleware_product_identity(105).unwrap();
        assert_eq!(original.product_id, product.source_product_id());
        assert_eq!(original.dongle_id, Some(105));
        assert_eq!(
            crate::device_identity::ui_product_identity(105)
                .unwrap()
                .product_id,
            104
        );
        for product in MappingProduct::ALL {
            assert_eq!(
                MappingProduct::for_product_id(product.product_id()),
                Some(product)
            );
        }
    }
}

impl MappingProduct {
    pub const ALL: [Self; 36] = [
        Self::same(70),
        Self::same(89),
        Self::same(96),
        Self::same(104),
        Self {
            mounted: 105,
            source: 104,
        },
        Self::same(112),
        Self::same(115),
        Self::same(140),
        Self::same(145),
        Self::same(148),
        Self::same(161),
        Self::same(178),
        Self::same(185),
        Self::same(194),
        Self::same(196),
        Self::same(224),
        Self::same(158),
        Self::same(190),
        Self::same(192),
        Self::same(222),
        Self::same(229),
        Self::same(239),
        Self::same(515),
        Self::same(679),
        Self::same(740),
        Self::same(746),
        Self::same(756),
        Self::same(600),
        Self::same(602),
        Self::same(658),
        Self::same(664),
        Self::same(697),
        Self::same(716),
        Self::same(717),
        Self::same(724),
        Self::same(727),
    ];

    pub const fn for_product_id(product_id: u32) -> Option<Self> {
        let mut ix = 0;
        while ix < Self::ALL.len() {
            let product = Self::ALL[ix];
            if product.mounted == product_id {
                return Some(product);
            }
            ix += 1;
        }
        None
    }

    pub const fn product_id(self) -> u32 {
        self.mounted
    }

    /// Current mounted105 uses the same source104 middleware, document and
    /// protocol owners. The mounted page identity remains105 for UI lifetime.
    pub const fn source_product_id(self) -> u32 {
        self.source
    }

    const fn same(product: u32) -> Self {
        Self {
            mounted: product,
            source: product,
        }
    }

    pub const fn is_ordinary_mouse(self) -> bool {
        crate::mouse_receiver_mapping::supports_ordinary(self.source)
    }

    pub const fn storage_field(self) -> &'static str {
        match self.source {
            70 => "_mouse70_mapping_document",
            158 => crate::mouse_receiver_mapping::STORAGE_FIELD,
            pid if crate::mouse_receiver_mapping::supports_ordinary(pid) => {
                "_mouse_mapping_document"
            }
            515 => "_keyboard515_mapping_document",
            679 => crate::keyboard_current_mapping::STORAGE_FIELD,
            740 | 746 => crate::keyboard_magnetic_mapping::STORAGE_FIELD,
            756 => crate::keyboard_latest_restore::STORAGE_FIELD,
            pid if crate::keyboard_receiver_mapping::supports(pid) => crate::keyboard_receiver_mapping::STORAGE_FIELD,
            _ => unreachable!(),
        }
    }

    pub const fn is_keyboard(self) -> bool {
        self.source != 70 && !self.is_ordinary_mouse()
    }

    pub const fn uses_retained_mapping_owner(self) -> bool {
        self.source != 756
    }

    pub fn owns_storage_field(field: &str) -> bool {
        Self::ALL
            .iter()
            .any(|product| product.storage_field() == field)
    }
}
