//! Audited local mapping documents and the owner used to publish them.

/// A product with an audited source document compiler entry. Constructing this type
/// does not establish a connection, registration, or successful publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MappingProduct(u32);

impl MappingProduct {
    pub const ALL: [Self; 6] = [
        Self(70),
        Self(515),
        Self(679),
        Self(740),
        Self(746),
        Self(756),
    ];

    pub const fn for_product_id(product_id: u32) -> Option<Self> {
        match product_id {
            70 | 515 | 679 | 740 | 746 | 756 => Some(Self(product_id)),
            _ => None,
        }
    }

    pub const fn product_id(self) -> u32 {
        self.0
    }

    pub const fn storage_field(self) -> &'static str {
        match self.0 {
            70 => "_mouse70_mapping_document",
            515 => "_keyboard515_mapping_document",
            679 => crate::keyboard_current_mapping::STORAGE_FIELD,
            740 | 746 => crate::keyboard_magnetic_mapping::STORAGE_FIELD,
            756 => crate::keyboard_latest_restore::STORAGE_FIELD,
            _ => unreachable!(),
        }
    }

    pub const fn is_keyboard(self) -> bool {
        self.0 != 70
    }

    pub const fn uses_retained_mapping_owner(self) -> bool {
        self.0 != 756
    }

    pub fn owns_storage_field(field: &str) -> bool {
        Self::ALL
            .iter()
            .any(|product| product.storage_field() == field)
    }
}
