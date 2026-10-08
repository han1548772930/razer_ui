//! Product palette roles that are distinct from GPUI Component's shared surfaces.
use gpui_kit::{Hsla, rgb, rgba};

/// Current Profiles 43/Ia CSS; see profiles-transfer-current-evidence.json.
pub(crate) struct ProfilesTransferColors;
impl ProfilesTransferColors {
    pub(crate) fn panel() -> Hsla {
        rgb(0x111111).into()
    }
    pub(crate) fn chrome() -> Hsla {
        rgb(0x222222).into()
    }
    pub(crate) fn border() -> Hsla {
        rgb(0x515151).into()
    }
    pub(crate) fn input_border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub(crate) fn text() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub(crate) fn title() -> Hsla {
        rgb(0x999999).into()
    }
    pub(crate) fn selected() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub(crate) fn browse_hover() -> Hsla {
        rgb(0x44c62d).into()
    }
    pub(crate) fn selected_text() -> Hsla {
        rgb(0x212121).into()
    }
    pub(crate) fn button_text() -> Hsla {
        rgb(0x000000).into()
    }
    pub(crate) fn secondary() -> Hsla {
        rgb(0x707070).into()
    }
    pub(crate) fn secondary_text() -> Hsla {
        rgb(0xffffff).into()
    }
    pub(crate) fn error() -> Hsla {
        rgb(0xfd4949).into()
    }
    pub(crate) fn warning() -> Hsla {
        rgb(0xfd8611).into()
    }
    pub(crate) fn backdrop() -> Hsla {
        rgba(0x000000b3).into()
    }
    pub(crate) fn button_border() -> Hsla {
        rgba(0x0000004d).into()
    }
}

/// 1382 MapAudio inline declarations and shared radio CSS.
pub(crate) struct ControlPodAudioColors;
impl ControlPodAudioColors {
    pub(crate) fn warning_panel() -> Hsla {
        rgb(0x111111).into()
    }
    pub(crate) fn warning_backdrop() -> Hsla {
        rgba(0x00000080).into()
    }
    pub(crate) fn warning_button_border() -> Hsla {
        rgb(0x000000).into()
    }
    pub(crate) fn warning_secondary() -> Hsla {
        rgb(0x707070).into()
    }
    pub(crate) fn warning_secondary_text() -> Hsla {
        rgb(0xffffff).into()
    }
    pub(crate) fn text() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub(crate) fn secondary() -> Hsla {
        rgb(0x999999).into()
    }
    pub(crate) fn error() -> Hsla {
        rgb(0xfd4949).into()
    }
    pub(crate) fn radio_border() -> Hsla {
        rgb(0x737373).into()
    }
    pub(crate) fn selected() -> Hsla {
        rgb(0x44d62c).into()
    }
}

/// Current 2636 `.alert-backdrop`, `.alert-container` and deadzone disclosure.
pub(crate) struct GamepadDialogColors;
impl GamepadDialogColors {
    pub(crate) fn backdrop() -> Hsla {
        rgba(0x111111b3).into()
    }
    pub(crate) fn panel() -> Hsla {
        rgb(0x111111).into()
    }
    pub(crate) fn text() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub(crate) fn warning() -> Hsla {
        rgb(0xfd8611).into()
    }
    pub(crate) fn detail() -> Hsla {
        rgb(0x222222).into()
    }
    pub(crate) fn secondary() -> Hsla {
        rgb(0x707070).into()
    }
    pub(crate) fn secondary_text() -> Hsla {
        rgb(0xffffff).into()
    }
    pub(crate) fn primary() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub(crate) fn primary_text() -> Hsla {
        rgb(0x000000).into()
    }
    pub(crate) fn primary_border() -> Hsla {
        rgba(0x0000004d).into()
    }
    pub(crate) fn shadow() -> Hsla {
        rgba(0x00000033).into()
    }
}

/// Current independent Settings `.installed-software .action`.
pub(crate) struct SettingsWindowColors;
impl SettingsWindowColors {
    pub(crate) fn navigation_border() -> Hsla {
        rgb(0x000000).into()
    }
    pub(crate) fn installed_action() -> Hsla {
        rgb(0x707070).into()
    }
}

/// Current Feedback application's mounted form and privacy-overlay CSS.
pub(crate) struct FeedbackColors;
impl FeedbackColors {
    pub(crate) fn background() -> Hsla {
        rgb(0x222222).into()
    }
    pub(crate) fn input() -> Hsla {
        rgb(0x000000).into()
    }
    pub(crate) fn foreground() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub(crate) fn border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub(crate) fn secondary() -> Hsla {
        rgb(0x999999).into()
    }
    pub(crate) fn placeholder() -> Hsla {
        rgb(0x707070).into()
    }
    pub(crate) fn error() -> Hsla {
        rgb(0xfd4949).into()
    }
    pub(crate) fn warning() -> Hsla {
        rgb(0xfd8611).into()
    }
    pub(crate) fn privacy_text() -> Hsla {
        rgb(0xeeeeee).into()
    }
    pub(crate) fn primary() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub(crate) fn primary_foreground() -> Hsla {
        rgb(0x000000).into()
    }
}

/// Current Dock Pro HyperPolling and Dock V2 Pro Duallink CSS modules.
pub(crate) struct DockPairingColors;
impl DockPairingColors {
    pub(crate) fn single_border() -> Hsla {
        rgb(0x515151).into()
    }
    pub(crate) fn unpair_hover() -> Hsla {
        rgb(0x9b9b9b).into()
    }
    pub(crate) fn unpair_pressed() -> Hsla {
        rgb(0x4e4e4e).into()
    }
    pub(crate) fn panel() -> Hsla {
        rgb(0x222222).into()
    }
    pub(crate) fn card() -> Hsla {
        rgb(0x111111).into()
    }
    pub(crate) fn category() -> Hsla {
        rgb(0x707070).into()
    }
    pub(crate) fn secondary() -> Hsla {
        rgb(0x707070).into()
    }
    pub(crate) fn primary_text() -> Hsla {
        rgb(0x212121).into()
    }
    pub(crate) fn secondary_text() -> Hsla {
        rgb(0xffffff).into()
    }
    pub(crate) fn warning() -> Hsla {
        rgb(0xfd8611).into()
    }
    /// `.HyperPollingWirelessMouseDock_dongleWarningText` /
    /// `_bothDevicesPollingCappedText`: 12px `#999` on the pairing page.
    pub(crate) fn warning_text() -> Hsla {
        rgb(0x999999).into()
    }
    /// `.Duallink_bothDevicesConnectedWarningText`: 14px/17px `#ccc` inside the
    /// pairing utility dialog, which is lighter than the page warnings.
    pub(crate) fn dialog_warning_text() -> Hsla {
        rgb(0xcccccc).into()
    }
    /// Current 179 `.HyperPollingWirelessUma_pairInfoBox .unpairbutton`.
    pub(crate) fn receiver_unpair_border() -> Hsla {
        rgb(0xcfcfcf).into()
    }
    pub(crate) fn border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub(crate) fn backdrop() -> Hsla {
        rgba(0x000000b3).into()
    }
}

/// Current `OTA` slider declarations (`.slider-container`, `.slider`,
/// `.slider-tip`) shared by the accessory, audio and OLED pages.
pub(crate) struct SliderColors;
impl SliderColors {
    /// `.slider-container{opacity:.3}` inverts with `.on{opacity:1}`.
    pub(crate) fn track() -> Hsla {
        rgba(0x44d62c4d).into()
    }
    /// `.slider-container .left` and `.slider::-webkit-slider-thumb`.
    pub(crate) fn fill() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub(crate) fn thumb() -> Hsla {
        rgb(0x44d62c).into()
    }
    /// `.on .slider::-webkit-slider-thumb:hover{background:#5d5d5d}`.
    pub(crate) fn thumb_hover() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    /// `.on .slider::-webkit-slider-thumb:active{background:#383838}`.
    pub(crate) fn thumb_active() -> Hsla {
        rgb(0x383838).into()
    }
    pub(crate) fn thumb_border() -> Hsla {
        rgb(0x44d62c).into()
    }
    /// `.slider-tip{color:#212121}`.
    pub(crate) fn tip_text() -> Hsla {
        rgb(0x212121).into()
    }
}

/// Current product 769's Home, Bridge and Brightness CSS modules.
pub(crate) struct HueColors;
impl HueColors {
    pub(crate) fn error() -> Hsla {
        rgb(0xc8323c).into()
    }
    pub(crate) fn secondary() -> Hsla {
        rgb(0x707070).into()
    }
    pub(crate) fn secondary_text() -> Hsla {
        rgb(0xffffff).into()
    }
    pub(crate) fn primary_text() -> Hsla {
        rgb(0x212121).into()
    }
    pub(crate) fn button_border() -> Hsla {
        rgba(0x0000004d).into()
    }
    pub(crate) fn ip_border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub(crate) fn icon_border() -> Hsla {
        rgba(0xffffff80).into()
    }
    pub(crate) fn warning() -> Hsla {
        rgb(0xfd8611).into()
    }
    pub(crate) fn remove() -> Hsla {
        rgb(0xfd4949).into()
    }
    pub(crate) fn light_hover() -> Hsla {
        rgb(0x222222).into()
    }
}

/// Current audio demo .demo-preview > .box > .icon, shared by 1392/1442/3942.
pub(crate) struct AudioDemoColors;
impl AudioDemoColors {
    pub(crate) fn unchecked_background() -> Hsla {
        Hsla::transparent_black()
    }
    /// Current audio demo `.custom-control-bar`: translucent black #0003.
    pub(crate) fn control_background() -> Hsla {
        rgba(0x00000033).into()
    }
    pub(crate) fn play_foreground() -> Hsla {
        rgb(0xffffff).into()
    }
    pub(crate) fn play_background() -> Hsla {
        rgba(0x000000cc).into()
    }
}

/// Current 740/746 KeyboardSwitchCalibrationModal and introduction source CSS.
pub(crate) struct KeyboardCalibrationColors;
impl KeyboardCalibrationColors {
    pub(crate) fn introduction() -> Hsla {
        rgb(0x2d2d2d).into()
    }
    pub(crate) fn panel() -> Hsla {
        rgb(0x2b2b2b).into()
    }
    pub(crate) fn key() -> Hsla {
        rgb(0x444444).into()
    }
    pub(crate) fn step() -> Hsla {
        rgba(0xffffff40).into()
    }
    pub(crate) fn step_text() -> Hsla {
        rgb(0x111111).into()
    }
    pub(crate) fn instruction() -> Hsla {
        rgba(0xffffffbf).into()
    }
    pub(crate) fn button() -> Hsla {
        rgb(0x707070).into()
    }
    pub(crate) fn button_text() -> Hsla {
        rgb(0xffffff).into()
    }
    pub(crate) fn button_border() -> Hsla {
        rgba(0x0000004d).into()
    }
    pub(crate) fn button_hover() -> Hsla {
        rgb(0x6ade57).into()
    }
}

/// Alexa's account button and sample response bubbles, from main.bbca16c4.css.
pub(crate) struct AlexaColors;
impl AlexaColors {
    pub(crate) fn amazon_button() -> Hsla {
        rgb(0x31c4f3).into()
    }
    pub(crate) fn amazon_button_hover() -> Hsla {
        rgb(0x6fd6f7).into()
    }
    pub(crate) fn amazon_button_active() -> Hsla {
        rgb(0x3589aa).into()
    }
    pub(crate) fn amazon_button_text() -> Hsla {
        rgb(0x232f3e).into()
    }
    pub(crate) fn bubble_blue() -> Hsla {
        rgb(0x2445f7).into()
    }
    pub(crate) fn skill_text() -> Hsla {
        rgb(0xdadada).into()
    }
    pub(crate) fn checkbox_border() -> Hsla {
        rgb(0x737373).into()
    }
    pub(crate) fn checkbox_hover() -> Hsla {
        rgb(0x7ce26b).into()
    }
    pub(crate) fn checkbox_active() -> Hsla {
        rgb(0x2f951e).into()
    }
    pub(crate) fn progress_track() -> Hsla {
        rgb(0x2c5824).into()
    }
    pub(crate) fn improvement() -> Hsla {
        rgb(0x8b7add).into()
    }
    pub(crate) fn fixed() -> Hsla {
        rgb(0x28aadc).into()
    }
    pub(crate) fn dropdown_hover() -> Hsla {
        rgb(0x1a1a1a).into()
    }
    pub(crate) fn dropdown_disabled() -> Hsla {
        rgb(0x454545).into()
    }
    pub(crate) fn dropdown_arrow() -> Hsla {
        rgb(0x999999).into()
    }
    pub(crate) fn tooltip_background() -> Hsla {
        rgb(0x5c5c5c).into()
    }
    pub(crate) fn tooltip_text() -> Hsla {
        rgb(0xf4f4f4).into()
    }
    pub(crate) fn bubble_text() -> Hsla {
        rgb(0x212121).into()
    }
}

/// Independent `/rz-app-menu/` group surfaces and the source New badge.
pub(crate) struct AppPickerColors;
impl AppPickerColors {
    pub(crate) fn surface() -> Hsla {
        rgb(0x000000).into()
    }
    pub(crate) fn border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub(crate) fn hover() -> Hsla {
        rgb(0x2d2d2d).into()
    }
    pub(crate) fn title() -> Hsla {
        rgb(0x999999).into()
    }
    pub(crate) fn text() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub(crate) fn badge_background() -> Hsla {
        rgb(0xffffff).into()
    }
    pub(crate) fn badge_text() -> Hsla {
        rgb(0x000000).into()
    }
}

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
    /// Current 55 CSS .progress-bar and its .progress child.
    pub(crate) fn service_progress_track(&self) -> Hsla {
        rgb(0x2c5824).into()
    }
    pub(crate) fn service_progress_fill(&self) -> Hsla {
        rgb(0x44d62c).into()
    }
    pub(crate) fn module_action_gray(&self) -> Hsla {
        rgb(0x555555).into()
    }
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

/// The independent firmware updater's current main.3e3077d6.css palette.
pub(crate) struct FirmwareColors;
impl FirmwareColors {
    pub(crate) fn warning_cancel_hover() -> Hsla {
        rgb(0x555555).into()
    }
    pub(crate) fn warning_continue_hover() -> Hsla {
        rgb(0x00e600).into()
    }
    pub(crate) fn background() -> Hsla {
        rgb(0x222222).into()
    }
    pub(crate) fn foreground() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub(crate) fn highlight() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub(crate) fn track() -> Hsla {
        rgb(0x2c5824).into()
    }
    pub(crate) fn tile() -> Hsla {
        rgb(0x111111).into()
    }
    pub(crate) fn warning() -> Hsla {
        rgb(0xfd8611).into()
    }
    pub(crate) fn border() -> Hsla {
        rgb(0x000000).into()
    }
    pub(crate) fn button_gray() -> Hsla {
        rgb(0x707070).into()
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
    /// Current Settings 720: `.widget .help:hover`.
    pub(crate) fn help_hover() -> Hsla {
        rgba(0xffffff4d).into()
    }
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
/// Current product 691 OLED screensaver CSS, independent of app theme.
pub(crate) struct OledColors;
impl OledColors {
    pub(crate) fn crop_face() -> Hsla {
        rgb(0xffffff).into()
    }
    pub(crate) fn screen() -> Hsla {
        rgb(0x000000).into()
    }
    pub(crate) fn muted() -> Hsla {
        rgb(0x999999).into()
    }
    pub(crate) fn border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub(crate) fn hover_border() -> Hsla {
        rgb(0x166809).into()
    }
}

/// Current camera CSS `.advanced-camera-container .default-button`.
pub(crate) struct CameraProductColors;
impl CameraProductColors {
    pub(crate) fn focus() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub(crate) fn spinner_hover() -> Hsla {
        rgba(0xffffff1a).into()
    }
    pub(crate) fn spinner_pressed() -> Hsla {
        rgba(0x0000001a).into()
    }
    pub(crate) fn border() -> gpui_kit::Hsla {
        gpui_kit::rgb(0x5d5d5d).into()
    }
    pub(crate) fn text() -> gpui_kit::Hsla {
        gpui_kit::rgb(0xcccccc).into()
    }
    pub(crate) fn selected() -> gpui_kit::Hsla {
        gpui_kit::rgb(0x292929).into()
    }
    pub(crate) fn hover() -> gpui_kit::Hsla {
        gpui_kit::rgb(0x111111).into()
    }
    pub(crate) fn pressed() -> gpui_kit::Hsla {
        gpui_kit::rgb(0xffffff).into()
    }
    /// `.advanced-camera-container .camera-container` background and the
    /// `.keyboard_listen` shortcut field share `#111`.
    pub(crate) fn background() -> gpui_kit::Hsla {
        gpui_kit::rgb(0x111111).into()
    }
    /// `.advanced-camera-container .camera-divider` border.
    pub(crate) fn divider() -> gpui_kit::Hsla {
        gpui_kit::rgb(0x222222).into()
    }
    /// The shortcut field's inline `color:"#707070"` placeholder.
    pub(crate) fn placeholder() -> gpui_kit::Hsla {
        gpui_kit::rgb(0x707070).into()
    }
    /// `.direction-container .direction-item` background `#222`.
    pub(crate) fn swatch() -> gpui_kit::Hsla {
        gpui_kit::rgb(0x222222).into()
    }
}

/// Current product 784 layout controls and its offline message.
pub(crate) struct AetherStripColors;
impl AetherStripColors {
    pub(crate) fn border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub(crate) fn selected() -> Hsla {
        rgb(0x292929).into()
    }
    pub(crate) fn warning() -> Hsla {
        rgb(0xfd8611).into()
    }
    pub(crate) fn background() -> Hsla {
        rgb(0x111111).into()
    }
    pub(crate) fn dialog() -> Hsla {
        rgb(0x000000).into()
    }
    pub(crate) fn remove() -> Hsla {
        rgb(0xfd4949).into()
    }
    pub(crate) fn control_border() -> Hsla {
        rgb(0x707070).into()
    }
    pub(crate) fn backdrop() -> Hsla {
        rgba(0x0000004d).into()
    }
}

/// Current keyboard ACTUATION `.btn-sync` and `.actuation-warning` palette.
pub(crate) struct KeyboardActuationColors;
impl KeyboardActuationColors {
    pub(crate) fn warning() -> Hsla {
        rgb(0xfd8611).into()
    }
    pub(crate) fn sync_background() -> Hsla {
        rgb(0x222222).into()
    }
    pub(crate) fn sync_border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub(crate) fn sync_hover() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub(crate) fn sync_text() -> Hsla {
        rgb(0xffffff).into()
    }
}

/// systrayv2 554.7cdbd936: its popup uses the renderer's fixed dark palette.
pub(crate) struct TrayColors;
impl TrayColors {
    pub(crate) fn tooltip_border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub(crate) fn selected_text() -> Hsla {
        rgb(0xffffff).into()
    }
    pub(crate) fn accent() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub(crate) fn surface() -> Hsla {
        rgb(0x222222).into()
    }
    pub(crate) fn border() -> Hsla {
        rgb(0x000000).into()
    }
    pub(crate) fn launcher() -> Hsla {
        rgb(0x111111).into()
    }
    pub(crate) fn text() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub(crate) fn muted() -> Hsla {
        rgb(0x999999).into()
    }
    pub(crate) fn hover_text() -> Hsla {
        rgb(0xeeeeee).into()
    }
}

/// Current device and Profiles CSS `.nav-tabs .nav:active`.
pub(crate) struct NavigationColors;
impl NavigationColors {
    pub(crate) fn pressed() -> Hsla {
        rgb(0x3cbf27).into()
    }
}

/// Current 3334/3337 `.warning-alert` and `.backdrop`; independent of host theme.
pub(crate) struct StreamMixerColors;
impl StreamMixerColors {
    pub(crate) fn warning() -> Hsla {
        rgb(0xfd8611).into()
    }
    pub(crate) fn panel() -> Hsla {
        rgb(0x111111).into()
    }
    pub(crate) fn text() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub(crate) fn accent() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub(crate) fn secondary() -> Hsla {
        rgb(0x707070).into()
    }
    pub(crate) fn button_text() -> Hsla {
        rgb(0x212121).into()
    }
    pub(crate) fn backdrop() -> Hsla {
        rgba(0x00000080).into()
    }
}

/// Current 515 `.key-record-item`, `.snap-tap-*` and warning dialog palette.
pub(crate) struct SnapTapColors;
impl SnapTapColors {
    pub(crate) fn border() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub(crate) fn text() -> Hsla {
        rgb(0xffffff).into()
    }
    pub(crate) fn muted() -> Hsla {
        rgb(0x999999).into()
    }
    pub(crate) fn warning() -> Hsla {
        rgb(0xfd8611).into()
    }
    pub(crate) fn accent() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub(crate) fn success() -> Hsla {
        rgb(0x00ff00).into()
    }
    pub(crate) fn panel() -> Hsla {
        rgb(0x111111).into()
    }
    pub(crate) fn tooltip_border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub(crate) fn backdrop() -> Hsla {
        rgba(0x00000080).into()
    }
}

/// 226 current `.swtm-*` controls and pointer tooltip.
pub(crate) struct ScrollWheelColors;
impl ScrollWheelColors {
    pub(crate) fn text() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub(crate) fn muted() -> Hsla {
        rgb(0x999999).into()
    }
    pub(crate) fn border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub(crate) fn accent() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub(crate) fn pills() -> Hsla {
        rgba(0x00000066).into()
    }
    pub(crate) fn pill_text() -> Hsla {
        rgba(0xffffffb3).into()
    }
    pub(crate) fn selected_text() -> Hsla {
        rgba(0x000000e6).into()
    }
    pub(crate) fn tooltip() -> Hsla {
        rgb(0x000000).into()
    }
}
pub(crate) struct DpiColors;
impl DpiColors {
    pub(crate) fn tick() -> Hsla {
        rgb(0x204c19).into()
    }
    pub(crate) fn text() -> Hsla {
        rgb(0xcccccc).into()
    }
}
/// Current Chroma Settings 75.e743878a.css palette; independent of OS theme.

pub(crate) struct ChromaSettingsColors;
impl ChromaSettingsColors {
    pub(crate) fn background() -> Hsla {
        rgb(0x222222).into()
    }
    pub(crate) fn panel() -> Hsla {
        rgb(0x111111).into()
    }
    pub(crate) fn text() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub(crate) fn secondary() -> Hsla {
        rgb(0x999999).into()
    }
    pub(crate) fn note() -> Hsla {
        rgb(0x707070).into()
    }
    pub(crate) fn selected() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub(crate) fn checkbox_border() -> Hsla {
        rgb(0x737373).into()
    }
    pub(crate) fn border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub(crate) fn hover() -> Hsla {
        rgb(0x2d2d2d).into()
    }
    pub(crate) fn help() -> Hsla {
        rgb(0x4a4a4a).into()
    }
    pub(crate) fn help_hover() -> Hsla {
        rgba(0xffffff4d).into()
    }
    pub(crate) fn tooltip() -> Hsla {
        rgb(0x000000).into()
    }
}
