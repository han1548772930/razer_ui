//! Product palette roles that are distinct from GPUI Component's shared surfaces.
use gpui_kit::{Hsla, rgb, rgba};

/// Settings `.thx-btn.test` has its own gray fill and translucent black border.
pub(crate) struct SettingsButtonColors;
impl SettingsButtonColors {
    pub(crate) fn background() -> Hsla {
        rgb(0x707070).into()
    }
    pub(crate) fn foreground() -> Hsla {
        rgb(0xffffff).into()
    }
    pub(crate) fn border() -> Hsla {
        rgba(0x0000004d).into()
    }
}

/// Electron host tab strip, distinct from the product frontend buttons.
pub(crate) struct HostColors;
impl HostColors {
    pub(crate) fn background() -> Hsla {
        rgb(0x000000).into()
    }
    pub(crate) fn surface() -> Hsla {
        rgb(0x222222).into()
    }
    pub(crate) fn hover() -> Hsla {
        rgb(0x444444).into()
    }
    pub(crate) fn accent() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub(crate) fn active_text() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub(crate) fn inactive_text() -> Hsla {
        rgb(0x999999).into()
    }
}

/// Source `[tooltip]` and `.tip` surfaces, independent of the application title bar.
pub(crate) struct TooltipColors;
impl TooltipColors {
    pub(crate) fn help_background() -> Hsla {
        rgb(0x4a4a4a).into()
    }
    pub(crate) fn help_hover() -> Hsla {
        rgb(0x6c6c6c).into()
    }
    pub(crate) fn background() -> Hsla {
        rgb(0x000000).into()
    }
    pub(crate) fn border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub(crate) fn foreground() -> Hsla {
        rgb(0xcccccc).into()
    }
}

/// `.keymap-head .close` in the product frontend.
pub(crate) struct KeymapCloseColors;
impl KeymapCloseColors {
    pub(crate) fn modal_pressed() -> Hsla {
        rgba(0x0000004d).into()
    }
    pub(crate) fn idle() -> Hsla {
        rgba(0x00000000).into()
    }
    pub(crate) fn hover() -> Hsla {
        rgba(0xffffff1a).into()
    }
    pub(crate) fn pressed() -> Hsla {
        rgba(0x0000001a).into()
    }
}

/// Introduction Tour's `.rz-btn`, `.rz-dot` and black photographic surface.
pub(crate) struct TourColors;
impl TourColors {
    pub(crate) fn background() -> Hsla {
        rgb(0x000000).into()
    }
    pub(crate) fn secondary() -> Hsla {
        rgb(0x707070).into()
    }
    pub(crate) fn border() -> Hsla {
        rgba(0x0000004d).into()
    }
    pub(crate) fn button_text() -> Hsla {
        rgb(0xffffff).into()
    }
    pub(crate) fn primary_text() -> Hsla {
        rgb(0x222222).into()
    }
    pub(crate) fn paragraph() -> Hsla {
        rgb(0xcccccc).into()
    }
}

/// Main frontend's photographic banner, module details, and tutorial emphasis.
pub(crate) struct MainPageColors;
impl MainPageColors {
    pub(crate) fn card_caption(&self) -> Hsla {
        rgb(0x707070).into()
    }
    pub(crate) fn detail_surface(&self) -> Hsla {
        rgb(0x2d2d2d).into()
    }
    pub(crate) fn tutorial_accent(&self) -> Hsla {
        rgb(0xfd8611).into()
    }
    pub(crate) fn banner_shade(&self) -> Hsla {
        rgb(0x000000).into()
    }
    pub(crate) fn banner_heading(&self) -> Hsla {
        rgb(0xffffff).into()
    }
    pub(crate) fn empty_group_text(&self) -> Hsla {
        rgb(0x999999).into()
    }
    pub(crate) fn mobile_action(&self) -> Hsla {
        rgb(0x2b2b2b).into()
    }
    pub(crate) fn tutorial_hover(&self) -> Hsla {
        rgb(0xfda044).into()
    }
}

/// Independent pairing workflow's original modal palette.
pub(crate) struct PairingColors;
impl PairingColors {
    pub(crate) fn dialog_surface(&self) -> Hsla {
        rgb(0x1a1a1a).into()
    }
    pub(crate) fn button_hover(&self) -> Hsla {
        rgb(0xff9530).into()
    }
    pub(crate) fn device_name(&self) -> Hsla {
        rgb(0xe3e3e3).into()
    }
    pub(crate) fn empty_border(&self) -> Hsla {
        rgb(0x666666).into()
    }
}

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
    /// 777 overrides the confirmation border/button, while retaining the red title.
    pub(crate) fn headphone_danger(&self) -> Hsla {
        rgb(0xc8323c).into()
    }
}

/// Final `.color-options` / `.picker-container` colors in the product stylesheets.
#[derive(Clone, Copy)]
pub(crate) struct PaletteColors;
impl PaletteColors {
    pub(crate) fn surface(&self) -> Hsla {
        rgb(0x111111).into()
    }
    pub(crate) fn border(&self) -> Hsla {
        rgb(0x515151).into()
    }
    pub(crate) fn picker_border(&self) -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub(crate) fn swatch_border(&self) -> Hsla {
        rgba(0x0000004d).into()
    }
    pub(crate) fn white(&self) -> Hsla {
        rgb(0xffffff).into()
    }
    pub(crate) fn selected_dot(&self) -> Hsla {
        rgba(0x000000b3).into()
    }
    pub(crate) fn secondary(&self) -> Hsla {
        rgb(0x707070).into()
    }
}

/// 653 Kt.slotColors: the hardware slot identifiers are 2 through 5.
pub(crate) struct OnboardMemoryColors;
impl OnboardMemoryColors {
    pub(crate) fn trigger_slot(slot_id: u8) -> Hsla {
        if slot_id == 3 {
            rgb(0x008000).into()
        } else {
            Self::slot(slot_id)
        }
    }
    pub(crate) fn secondary() -> Hsla {
        rgb(0x707070).into()
    }
    pub(crate) fn white() -> Hsla {
        rgb(0xffffff).into()
    }
    pub(crate) fn slot(slot_id: u8) -> Hsla {
        rgb(match slot_id {
            2 => 0xff0000,
            3 => 0x00ff00,
            4 => 0x0000ff,
            5 => 0x00ffff,
            _ => 0x999999,
        })
        .into()
    }
}

pub(crate) struct CommandDialColors;
impl CommandDialColors {
    pub(crate) fn hover_border(&self) -> Hsla {
        rgb(0x1b5811).into()
    }
    pub(crate) fn detail_border(&self) -> Hsla {
        rgb(0x3a3a3a).into()
    }
}

pub(crate) struct IotColors;
impl IotColors {
    pub(crate) fn help_link() -> Hsla {
        rgb(0x30d5ff).into()
    }
}

pub(crate) struct HeaderStatusColors;
impl HeaderStatusColors {
    pub(crate) fn status_surface() -> Hsla {
        rgb(0x212121).into()
    }
    pub(crate) fn offline_hover() -> Hsla {
        rgb(0x3cbf27).into()
    }
    pub(crate) fn update_tooltip_border() -> Hsla {
        rgb(0x383838).into()
    }
}

/// rz-user-profile-menu's .dropdown-razer and the host avatar trigger.
pub(crate) struct AccountMenuColors;
impl AccountMenuColors {
    pub(crate) fn surface() -> Hsla {
        rgb(0x000000).into()
    }
    pub(crate) fn border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub(crate) fn hover() -> Hsla {
        rgb(0x1f1f1f).into()
    }
    pub(crate) fn trigger_active() -> Hsla {
        rgb(0x2d2d2d).into()
    }
}

pub(crate) struct MigrationColors;
impl MigrationColors {
    pub(crate) fn checkbox_border() -> Hsla {
        rgb(0x737373).into()
    }
}

pub(crate) struct SettingsColors;
impl SettingsColors {
    pub(crate) fn tree_note() -> Hsla {
        rgb(0x707070).into()
    }
    pub(crate) fn tooltip_surface() -> Hsla {
        rgb(0x000000).into()
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
