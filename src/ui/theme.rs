//! Product palette roles that are distinct from GPUI Component's shared surfaces.
use gpui_kit::{Hsla, rgb, rgba};

/// Source numeric stepper outline; separate from the ordinary dropdown border.
pub(crate) fn stepper_border() -> Hsla {
    rgb(0x5d5d5d).into()
}

/// Original `.s3-options` colors. Other popovers retain the application's #111 surface.
pub(crate) struct DropdownColors {
    surface: Hsla,
    hover: Hsla,
}

impl DropdownColors {
    pub(crate) fn new() -> Self {
        Self::default()
    }
}

impl Default for DropdownColors {
    fn default() -> Self {
        Self {
            surface: rgb(0x000000).into(),
            hover: rgba(0xffffff1a).into(),
        }
    }
}

impl DropdownColors {
    pub(crate) fn surface(&self) -> Hsla {
        self.surface
    }

    pub(crate) fn hover(&self) -> Hsla {
        self.hover
    }
}

/// Original `.profile-del` border, title and confirmation button color.
pub(crate) struct ProfileAlertColors {
    danger: Hsla,
}

impl ProfileAlertColors {
    pub(crate) fn new() -> Self {
        Self {
            danger: rgb(0xfd4949).into(),
        }
    }

    pub(crate) fn danger(&self) -> Hsla {
        self.danger
    }
}

/// Original Customize input drawer, including the orange Hypershift assignments.
pub(crate) struct DrawerColors;
impl DrawerColors {
    pub(crate) fn new() -> Self {
        Self
    }
    pub(crate) fn surface(&self) -> Hsla {
        rgb(0x2d2d2d).into()
    }
    pub(crate) fn keyboard_surface(&self) -> Hsla {
        rgb(0x2b2b2b).into()
    }
    pub(crate) fn hover(&self) -> Hsla {
        rgb(0x383838).into()
    }
    pub(crate) fn muted(&self) -> Hsla {
        rgb(0x707070).into()
    }
    pub(crate) fn hypershift(&self) -> Hsla {
        rgb(0xfd8611).into()
    }
    /// `.keyboard-svg .disabled` keeps disabled assignments red in either layer.
    pub(crate) fn disabled_mapping(&self) -> Hsla {
        rgba(0xc8323c80).into()
    }
}
