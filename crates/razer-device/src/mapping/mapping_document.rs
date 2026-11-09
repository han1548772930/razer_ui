//! Audited local mapping documents and the owner used to publish them.

/// A product with an audited source document compiler entry. Constructing this type
/// does not establish a connection, registration, or successful publication.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MappingProduct(u32);

impl MappingProduct {
    pub const ALL: [Self; 26] = [
        Self(70),
        Self(89),
        Self(96),
        Self(104),
        Self(112),
        Self(115),
        Self(140),
        Self(145),
        Self(148),
        Self(161),
        Self(178),
        Self(185),
        Self(194),
        Self(196),
        Self(224),
        Self(158),
        Self(190),
        Self(192),
        Self(222),
        Self(229),
        Self(239),
        Self(515),
        Self(679),
        Self(740),
        Self(746),
        Self(756),
    ];

    pub const fn for_product_id(product_id: u32) -> Option<Self> {
        match product_id {
            70 | 515 | 679 | 740 | 746 | 756 => Some(Self(product_id)),
            pid if crate::mouse_receiver_mapping::supports_ordinary(pid) => Some(Self(pid)),
            _ => None,
        }
    }

    pub const fn product_id(self) -> u32 {
        self.0
    }

    pub const fn storage_field(self) -> &'static str {
        match self.0 {
            70 => "_mouse70_mapping_document",
            158 => crate::mouse_receiver_mapping::STORAGE_FIELD,
            pid if crate::mouse_receiver_mapping::supports_ordinary(pid) => {
                "_mouse_mapping_document"
            }
            515 => "_keyboard515_mapping_document",
            679 => crate::keyboard_current_mapping::STORAGE_FIELD,
            740 | 746 => crate::keyboard_magnetic_mapping::STORAGE_FIELD,
            756 => crate::keyboard_latest_restore::STORAGE_FIELD,
            _ => unreachable!(),
        }
    }

    pub const fn is_keyboard(self) -> bool {
        self.0 != 70 && !crate::mouse_receiver_mapping::supports_ordinary(self.0)
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
