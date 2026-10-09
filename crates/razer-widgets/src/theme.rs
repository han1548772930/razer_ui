//! Product palette roles that are distinct from GPUI Component's shared surfaces.
use gpui_kit::{Hsla, rgb, rgba};

/// Current Profiles 43/Ia CSS; see profiles-transfer-current-evidence.json.
pub struct ProfilesTransferColors;
impl ProfilesTransferColors {
    pub fn panel() -> Hsla {
        rgb(0x111111).into()
    }
    pub fn chrome() -> Hsla {
        rgb(0x222222).into()
    }
    pub fn border() -> Hsla {
        rgb(0x515151).into()
    }
    pub fn input_border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub fn text() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub fn title() -> Hsla {
        rgb(0x999999).into()
    }
    pub fn selected() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub fn browse_hover() -> Hsla {
        rgb(0x44c62d).into()
    }
    pub fn selected_text() -> Hsla {
        rgb(0x212121).into()
    }
    pub fn button_text() -> Hsla {
        rgb(0x000000).into()
    }
    pub fn secondary() -> Hsla {
        rgb(0x707070).into()
    }
    pub fn secondary_text() -> Hsla {
        rgb(0xffffff).into()
    }
    pub fn error() -> Hsla {
        rgb(0xfd4949).into()
    }
    pub fn warning() -> Hsla {
        rgb(0xfd8611).into()
    }
    pub fn backdrop() -> Hsla {
        rgba(0x000000b3).into()
    }
    pub fn button_border() -> Hsla {
        rgba(0x0000004d).into()
    }
}

/// 1382 MapAudio inline declarations and shared radio CSS.
pub struct ControlPodAudioColors;
impl ControlPodAudioColors {
    pub fn warning_panel() -> Hsla {
        rgb(0x111111).into()
    }
    pub fn warning_backdrop() -> Hsla {
        rgba(0x00000080).into()
    }
    pub fn warning_button_border() -> Hsla {
        rgb(0x000000).into()
    }
    pub fn warning_secondary() -> Hsla {
        rgb(0x707070).into()
    }
    pub fn warning_secondary_text() -> Hsla {
        rgb(0xffffff).into()
    }
    pub fn text() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub fn secondary() -> Hsla {
        rgb(0x999999).into()
    }
    pub fn error() -> Hsla {
        rgb(0xfd4949).into()
    }
    pub fn radio_border() -> Hsla {
        rgb(0x737373).into()
    }
    pub fn selected() -> Hsla {
        rgb(0x44d62c).into()
    }
}

/// Current 2636 `.alert-backdrop`, `.alert-container` and deadzone disclosure.
pub struct GamepadDialogColors;
impl GamepadDialogColors {
    pub fn backdrop() -> Hsla {
        rgba(0x111111b3).into()
    }
    pub fn panel() -> Hsla {
        rgb(0x111111).into()
    }
    pub fn text() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub fn warning() -> Hsla {
        rgb(0xfd8611).into()
    }
    pub fn detail() -> Hsla {
        rgb(0x222222).into()
    }
    pub fn secondary() -> Hsla {
        rgb(0x707070).into()
    }
    pub fn secondary_text() -> Hsla {
        rgb(0xffffff).into()
    }
    pub fn primary() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub fn primary_text() -> Hsla {
        rgb(0x000000).into()
    }
    pub fn primary_border() -> Hsla {
        rgba(0x0000004d).into()
    }
    pub fn shadow() -> Hsla {
        rgba(0x00000033).into()
    }
}

/// Current independent Settings `.installed-software .action`.
pub struct SettingsWindowColors;
impl SettingsWindowColors {
    pub fn navigation_border() -> Hsla {
        rgb(0x000000).into()
    }
    pub fn installed_action() -> Hsla {
        rgb(0x707070).into()
    }
}

/// Current Feedback application's mounted form and privacy-overlay CSS.
pub struct FeedbackColors;
impl FeedbackColors {
    pub fn background() -> Hsla {
        rgb(0x222222).into()
    }
    pub fn input() -> Hsla {
        rgb(0x000000).into()
    }
    pub fn foreground() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub fn border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub fn secondary() -> Hsla {
        rgb(0x999999).into()
    }
    pub fn placeholder() -> Hsla {
        rgb(0x707070).into()
    }
    pub fn error() -> Hsla {
        rgb(0xfd4949).into()
    }
    pub fn warning() -> Hsla {
        rgb(0xfd8611).into()
    }
    pub fn privacy_text() -> Hsla {
        rgb(0xeeeeee).into()
    }
    pub fn primary() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub fn primary_foreground() -> Hsla {
        rgb(0x000000).into()
    }
}

/// Current Dock Pro HyperPolling and Dock V2 Pro Duallink CSS modules.
pub struct DockPairingColors;
impl DockPairingColors {
    pub fn single_border() -> Hsla {
        rgb(0x515151).into()
    }
    pub fn unpair_hover() -> Hsla {
        rgb(0x9b9b9b).into()
    }
    pub fn unpair_pressed() -> Hsla {
        rgb(0x4e4e4e).into()
    }
    pub fn panel() -> Hsla {
        rgb(0x222222).into()
    }
    pub fn card() -> Hsla {
        rgb(0x111111).into()
    }
    pub fn category() -> Hsla {
        rgb(0x707070).into()
    }
    pub fn secondary() -> Hsla {
        rgb(0x707070).into()
    }
    pub fn primary_text() -> Hsla {
        rgb(0x212121).into()
    }
    pub fn secondary_text() -> Hsla {
        rgb(0xffffff).into()
    }
    pub fn warning() -> Hsla {
        rgb(0xfd8611).into()
    }
    /// `.HyperPollingWirelessMouseDock_dongleWarningText` /
    /// `_bothDevicesPollingCappedText`: 12px `#999` on the pairing page.
    pub fn warning_text() -> Hsla {
        rgb(0x999999).into()
    }
    /// `.Duallink_bothDevicesConnectedWarningText`: 14px/17px `#ccc` inside the
    /// pairing utility dialog, which is lighter than the page warnings.
    pub fn dialog_warning_text() -> Hsla {
        rgb(0xcccccc).into()
    }
    /// Current 179 `.HyperPollingWirelessUma_pairInfoBox .unpairbutton`.
    pub fn receiver_unpair_border() -> Hsla {
        rgb(0xcfcfcf).into()
    }
    pub fn border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub fn backdrop() -> Hsla {
        rgba(0x000000b3).into()
    }
}

/// Current `OTA` slider declarations (`.slider-container`, `.slider`,
/// `.slider-tip`) shared by the accessory, audio and OLED pages.
pub struct SliderColors;
impl SliderColors {
    /// `.slider-container{opacity:.3}` inverts with `.on{opacity:1}`.
    pub fn track() -> Hsla {
        rgba(0x44d62c4d).into()
    }
    /// `.slider-container .left` and `.slider::-webkit-slider-thumb`.
    pub fn fill() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub fn thumb() -> Hsla {
        rgb(0x44d62c).into()
    }
    /// `.on .slider::-webkit-slider-thumb:hover{background:#5d5d5d}`.
    pub fn thumb_hover() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    /// `.on .slider::-webkit-slider-thumb:active{background:#383838}`.
    pub fn thumb_active() -> Hsla {
        rgb(0x383838).into()
    }
    pub fn thumb_border() -> Hsla {
        rgb(0x44d62c).into()
    }
    /// `.slider-tip{color:#212121}`.
    pub fn tip_text() -> Hsla {
        rgb(0x212121).into()
    }
}

/// Current product 769's Home, Bridge and Brightness CSS modules.
pub struct HueColors;
impl HueColors {
    pub fn error() -> Hsla {
        rgb(0xc8323c).into()
    }
    pub fn secondary() -> Hsla {
        rgb(0x707070).into()
    }
    pub fn secondary_text() -> Hsla {
        rgb(0xffffff).into()
    }
    pub fn primary_text() -> Hsla {
        rgb(0x212121).into()
    }
    pub fn button_border() -> Hsla {
        rgba(0x0000004d).into()
    }
    pub fn ip_border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub fn icon_border() -> Hsla {
        rgba(0xffffff80).into()
    }
    pub fn warning() -> Hsla {
        rgb(0xfd8611).into()
    }
    pub fn remove() -> Hsla {
        rgb(0xfd4949).into()
    }
    pub fn light_hover() -> Hsla {
        rgb(0x222222).into()
    }
}

/// Current audio demo .demo-preview > .box > .icon, shared by 1392/1442/3942.
pub struct AudioDemoColors;
impl AudioDemoColors {
    pub fn unchecked_background() -> Hsla {
        Hsla::transparent_black()
    }
    /// Current audio demo `.custom-control-bar`: translucent black #0003.
    pub fn control_background() -> Hsla {
        rgba(0x00000033).into()
    }
    pub fn play_foreground() -> Hsla {
        rgb(0xffffff).into()
    }
    pub fn play_background() -> Hsla {
        rgba(0x000000cc).into()
    }
}

/// Current 740/746 KeyboardSwitchCalibrationModal and introduction source CSS.
pub struct KeyboardCalibrationColors;
impl KeyboardCalibrationColors {
    pub fn introduction() -> Hsla {
        rgb(0x2d2d2d).into()
    }
    pub fn panel() -> Hsla {
        rgb(0x2b2b2b).into()
    }
    pub fn key() -> Hsla {
        rgb(0x444444).into()
    }
    pub fn step() -> Hsla {
        rgba(0xffffff40).into()
    }
    pub fn step_text() -> Hsla {
        rgb(0x111111).into()
    }
    pub fn instruction() -> Hsla {
        rgba(0xffffffbf).into()
    }
    pub fn button() -> Hsla {
        rgb(0x707070).into()
    }
    pub fn button_text() -> Hsla {
        rgb(0xffffff).into()
    }
    pub fn button_border() -> Hsla {
        rgba(0x0000004d).into()
    }
    pub fn button_hover() -> Hsla {
        rgb(0x6ade57).into()
    }
}

/// Alexa's account button and sample response bubbles, from main.bbca16c4.css.
pub struct AlexaColors;
impl AlexaColors {
    pub fn amazon_button() -> Hsla {
        rgb(0x31c4f3).into()
    }
    pub fn amazon_button_hover() -> Hsla {
        rgb(0x6fd6f7).into()
    }
    pub fn amazon_button_active() -> Hsla {
        rgb(0x3589aa).into()
    }
    pub fn amazon_button_text() -> Hsla {
        rgb(0x232f3e).into()
    }
    pub fn bubble_blue() -> Hsla {
        rgb(0x2445f7).into()
    }
    pub fn skill_text() -> Hsla {
        rgb(0xdadada).into()
    }
    pub fn checkbox_border() -> Hsla {
        rgb(0x737373).into()
    }
    pub fn checkbox_hover() -> Hsla {
        rgb(0x7ce26b).into()
    }
    pub fn checkbox_active() -> Hsla {
        rgb(0x2f951e).into()
    }
    pub fn progress_track() -> Hsla {
        rgb(0x2c5824).into()
    }
    pub fn improvement() -> Hsla {
        rgb(0x8b7add).into()
    }
    pub fn fixed() -> Hsla {
        rgb(0x28aadc).into()
    }
    pub fn dropdown_hover() -> Hsla {
        rgb(0x1a1a1a).into()
    }
    pub fn dropdown_disabled() -> Hsla {
        rgb(0x454545).into()
    }
    pub fn dropdown_arrow() -> Hsla {
        rgb(0x999999).into()
    }
    pub fn tooltip_background() -> Hsla {
        rgb(0x5c5c5c).into()
    }
    pub fn tooltip_text() -> Hsla {
        rgb(0xf4f4f4).into()
    }
    pub fn bubble_text() -> Hsla {
        rgb(0x212121).into()
    }
}

/// Independent `/rz-app-menu/` group surfaces and the source New badge.
pub struct AppPickerColors;
impl AppPickerColors {
    pub fn surface() -> Hsla {
        rgb(0x000000).into()
    }
    pub fn border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub fn hover() -> Hsla {
        rgb(0x2d2d2d).into()
    }
    pub fn title() -> Hsla {
        rgb(0x999999).into()
    }
    pub fn text() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub fn badge_background() -> Hsla {
        rgb(0xffffff).into()
    }
    pub fn badge_text() -> Hsla {
        rgb(0x000000).into()
    }
}

/// Settings `.thx-btn.test` has its own gray fill and translucent black border.
pub struct SettingsButtonColors;
impl SettingsButtonColors {
    pub fn background() -> Hsla {
        rgb(0x707070).into()
    }
    pub fn foreground() -> Hsla {
        rgb(0xffffff).into()
    }
    pub fn border() -> Hsla {
        rgba(0x0000004d).into()
    }
}

/// Electron host tab strip, distinct from the product frontend buttons.
pub struct HostColors;
impl HostColors {
    pub fn background() -> Hsla {
        rgb(0x000000).into()
    }
    pub fn surface() -> Hsla {
        rgb(0x222222).into()
    }
    pub fn hover() -> Hsla {
        rgb(0x444444).into()
    }
    pub fn accent() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub fn active_text() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub fn inactive_text() -> Hsla {
        rgb(0x999999).into()
    }
}

/// Source `[tooltip]` and `.tip` surfaces, independent of the application title bar.
pub struct TooltipColors;
impl TooltipColors {
    pub fn help_background() -> Hsla {
        rgb(0x4a4a4a).into()
    }
    pub fn help_hover() -> Hsla {
        rgb(0x6c6c6c).into()
    }
    pub fn background() -> Hsla {
        rgb(0x000000).into()
    }
    pub fn border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub fn foreground() -> Hsla {
        rgb(0xcccccc).into()
    }
}

/// `.keymap-head .close` in the product frontend.
pub struct KeymapCloseColors;
impl KeymapCloseColors {
    pub fn idle() -> Hsla {
        rgba(0x00000000).into()
    }
    pub fn hover() -> Hsla {
        rgba(0xffffff1a).into()
    }
    pub fn pressed() -> Hsla {
        rgba(0x0000001a).into()
    }
}

/// Introduction Tour's `.rz-btn`, `.rz-dot` and black photographic surface.
pub struct TourColors;
impl TourColors {
    pub fn background() -> Hsla {
        rgb(0x000000).into()
    }
    pub fn secondary() -> Hsla {
        rgb(0x707070).into()
    }
    pub fn border() -> Hsla {
        rgba(0x0000004d).into()
    }
    pub fn button_text() -> Hsla {
        rgb(0xffffff).into()
    }
    pub fn primary_text() -> Hsla {
        rgb(0x222222).into()
    }
    pub fn paragraph() -> Hsla {
        rgb(0xcccccc).into()
    }
}

/// Main frontend's photographic banner, module details, and tutorial emphasis.
pub struct MainPageColors;
impl MainPageColors {
    /// Current 55 CSS .progress-bar and its .progress child.
    pub fn service_progress_track(&self) -> Hsla {
        rgb(0x2c5824).into()
    }
    pub fn service_progress_fill(&self) -> Hsla {
        rgb(0x44d62c).into()
    }
    pub fn module_action_gray(&self) -> Hsla {
        rgb(0x555555).into()
    }
    pub fn card_caption(&self) -> Hsla {
        rgb(0x707070).into()
    }
    pub fn detail_surface(&self) -> Hsla {
        rgb(0x2d2d2d).into()
    }
    pub fn tutorial_accent(&self) -> Hsla {
        rgb(0xfd8611).into()
    }
    pub fn banner_shade(&self) -> Hsla {
        rgb(0x000000).into()
    }
    pub fn banner_heading(&self) -> Hsla {
        rgb(0xffffff).into()
    }
    pub fn empty_group_text(&self) -> Hsla {
        rgb(0x999999).into()
    }
    pub fn mobile_action(&self) -> Hsla {
        rgb(0x2b2b2b).into()
    }
    pub fn tutorial_hover(&self) -> Hsla {
        rgb(0xfda044).into()
    }
}

/// Independent pairing workflow's original modal palette.
pub struct PairingColors;
impl PairingColors {
    pub fn dialog_surface(&self) -> Hsla {
        rgb(0x1a1a1a).into()
    }
    pub fn button_hover(&self) -> Hsla {
        rgb(0xff9530).into()
    }
    pub fn device_name(&self) -> Hsla {
        rgb(0xe3e3e3).into()
    }
    pub fn empty_border(&self) -> Hsla {
        rgb(0x666666).into()
    }
}

/// Source numeric stepper outline; separate from the ordinary dropdown border.
pub fn stepper_border() -> Hsla {
    rgb(0x5d5d5d).into()
}

/// Original `.s3-options` colors. Other popovers retain the application's #111 surface.
pub struct DropdownColors {
    surface: Hsla,
    hover: Hsla,
}

impl DropdownColors {
    pub fn new() -> Self {
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
    pub fn surface(&self) -> Hsla {
        self.surface
    }

    pub fn hover(&self) -> Hsla {
        self.hover
    }
}

/// Original `.profile-del` border, title and confirmation button color.
pub struct ProfileAlertColors {
    danger: Hsla,
}

impl ProfileAlertColors {
    pub fn new() -> Self {
        Self {
            danger: rgb(0xfd4949).into(),
        }
    }

    pub fn danger(&self) -> Hsla {
        self.danger
    }
    /// 777 overrides the confirmation border/button, while retaining the red title.
    pub fn headphone_danger(&self) -> Hsla {
        rgb(0xc8323c).into()
    }
}

/// Final `.color-options` / `.picker-container` colors in the product stylesheets.
#[derive(Clone, Copy)]
pub struct PaletteColors;
impl PaletteColors {
    pub fn surface(&self) -> Hsla {
        rgb(0x111111).into()
    }
    pub fn border(&self) -> Hsla {
        rgb(0x515151).into()
    }
    pub fn picker_border(&self) -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub fn swatch_border(&self) -> Hsla {
        rgba(0x0000004d).into()
    }
    pub fn white(&self) -> Hsla {
        rgb(0xffffff).into()
    }
    pub fn selected_dot(&self) -> Hsla {
        rgba(0x000000b3).into()
    }
    pub fn secondary(&self) -> Hsla {
        rgb(0x707070).into()
    }
}

/// 653 Kt.slotColors: the hardware slot identifiers are 2 through 5.
pub struct OnboardMemoryColors;
impl OnboardMemoryColors {
    pub fn trigger_slot(slot_id: u8) -> Hsla {
        if slot_id == 3 {
            rgb(0x008000).into()
        } else {
            Self::slot(slot_id)
        }
    }
    pub fn secondary() -> Hsla {
        rgb(0x707070).into()
    }
    pub fn white() -> Hsla {
        rgb(0xffffff).into()
    }
    pub fn slot(slot_id: u8) -> Hsla {
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

pub struct CommandDialColors;
impl CommandDialColors {
    pub fn hover_border(&self) -> Hsla {
        rgb(0x1b5811).into()
    }
    pub fn detail_border(&self) -> Hsla {
        rgb(0x3a3a3a).into()
    }
}

pub struct IotColors;
impl IotColors {
    pub fn help_link() -> Hsla {
        rgb(0x30d5ff).into()
    }
}

pub struct HeaderStatusColors;
impl HeaderStatusColors {
    pub fn status_surface() -> Hsla {
        rgb(0x212121).into()
    }
    pub fn offline_hover() -> Hsla {
        rgb(0x3cbf27).into()
    }
    pub fn update_tooltip_border() -> Hsla {
        rgb(0x383838).into()
    }
}

/// The independent firmware updater's current main.3e3077d6.css palette.
pub struct FirmwareColors;
impl FirmwareColors {
    pub fn warning_cancel_hover() -> Hsla {
        rgb(0x555555).into()
    }
    pub fn warning_continue_hover() -> Hsla {
        rgb(0x00e600).into()
    }
    pub fn background() -> Hsla {
        rgb(0x222222).into()
    }
    pub fn foreground() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub fn highlight() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub fn track() -> Hsla {
        rgb(0x2c5824).into()
    }
    pub fn tile() -> Hsla {
        rgb(0x111111).into()
    }
    pub fn warning() -> Hsla {
        rgb(0xfd8611).into()
    }
    pub fn border() -> Hsla {
        rgb(0x000000).into()
    }
    pub fn button_gray() -> Hsla {
        rgb(0x707070).into()
    }
}

/// rz-user-profile-menu's .dropdown-razer and the host avatar trigger.
pub struct AccountMenuColors;
impl AccountMenuColors {
    pub fn surface() -> Hsla {
        rgb(0x000000).into()
    }
    pub fn border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub fn hover() -> Hsla {
        rgb(0x1f1f1f).into()
    }
    pub fn trigger_active() -> Hsla {
        rgb(0x2d2d2d).into()
    }
}

pub struct MigrationColors;
impl MigrationColors {
    pub fn checkbox_border() -> Hsla {
        rgb(0x737373).into()
    }
}

pub struct SettingsColors;
impl SettingsColors {
    /// Current Settings 720: `.widget .help:hover`.
    pub fn help_hover() -> Hsla {
        rgba(0xffffff4d).into()
    }
    pub fn tree_note() -> Hsla {
        rgb(0x707070).into()
    }
    pub fn tooltip_surface() -> Hsla {
        rgb(0x000000).into()
    }
}

/// Original Customize input drawer, including the orange Hypershift assignments.
pub struct DrawerColors;
impl DrawerColors {
    pub fn new() -> Self {
        Self
    }
    pub fn surface(&self) -> Hsla {
        rgb(0x2d2d2d).into()
    }
    pub fn keyboard_surface(&self) -> Hsla {
        rgb(0x2b2b2b).into()
    }
    pub fn hover(&self) -> Hsla {
        rgb(0x383838).into()
    }
    pub fn muted(&self) -> Hsla {
        rgb(0x707070).into()
    }
    pub fn hypershift(&self) -> Hsla {
        rgb(0xfd8611).into()
    }
    /// `.keyboard-svg .disabled` keeps disabled assignments red in either layer.
    pub fn disabled_mapping(&self) -> Hsla {
        rgba(0xc8323c80).into()
    }
}
/// Current product 691 OLED screensaver CSS, independent of app theme.
pub struct OledColors;
impl OledColors {
    pub fn crop_face() -> Hsla {
        rgb(0xffffff).into()
    }
    pub fn screen() -> Hsla {
        rgb(0x000000).into()
    }
    pub fn muted() -> Hsla {
        rgb(0x999999).into()
    }
    pub fn border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub fn hover_border() -> Hsla {
        rgb(0x166809).into()
    }
}

/// Current camera CSS `.advanced-camera-container .default-button`.
pub struct CameraProductColors;
impl CameraProductColors {
    pub fn focus() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub fn spinner_hover() -> Hsla {
        rgba(0xffffff1a).into()
    }
    pub fn spinner_pressed() -> Hsla {
        rgba(0x0000001a).into()
    }
    pub fn border() -> gpui_kit::Hsla {
        gpui_kit::rgb(0x5d5d5d).into()
    }
    pub fn text() -> gpui_kit::Hsla {
        gpui_kit::rgb(0xcccccc).into()
    }
    pub fn selected() -> gpui_kit::Hsla {
        gpui_kit::rgb(0x292929).into()
    }
    pub fn hover() -> gpui_kit::Hsla {
        gpui_kit::rgb(0x111111).into()
    }
    pub fn pressed() -> gpui_kit::Hsla {
        gpui_kit::rgb(0xffffff).into()
    }
    /// `.advanced-camera-container .camera-container` background and the
    /// `.keyboard_listen` shortcut field share `#111`.
    pub fn background() -> gpui_kit::Hsla {
        gpui_kit::rgb(0x111111).into()
    }
    /// `.advanced-camera-container .camera-divider` border.
    pub fn divider() -> gpui_kit::Hsla {
        gpui_kit::rgb(0x222222).into()
    }
    /// The shortcut field's inline `color:"#707070"` placeholder.
    pub fn placeholder() -> gpui_kit::Hsla {
        gpui_kit::rgb(0x707070).into()
    }
    /// `.direction-container .direction-item` background `#222`.
    pub fn swatch() -> gpui_kit::Hsla {
        gpui_kit::rgb(0x222222).into()
    }
}

/// Current product 784 layout controls and its offline message.
pub struct AetherStripColors;
impl AetherStripColors {
    pub fn border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub fn selected() -> Hsla {
        rgb(0x292929).into()
    }
    pub fn warning() -> Hsla {
        rgb(0xfd8611).into()
    }
    pub fn background() -> Hsla {
        rgb(0x111111).into()
    }
    pub fn dialog() -> Hsla {
        rgb(0x000000).into()
    }
    pub fn remove() -> Hsla {
        rgb(0xfd4949).into()
    }
    pub fn control_border() -> Hsla {
        rgb(0x707070).into()
    }
    pub fn backdrop() -> Hsla {
        rgba(0x0000004d).into()
    }
}

/// Current keyboard ACTUATION `.btn-sync` and `.actuation-warning` palette.
pub struct KeyboardActuationColors;
impl KeyboardActuationColors {
    pub fn warning() -> Hsla {
        rgb(0xfd8611).into()
    }
    pub fn sync_background() -> Hsla {
        rgb(0x222222).into()
    }
    pub fn sync_border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub fn sync_hover() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub fn sync_text() -> Hsla {
        rgb(0xffffff).into()
    }
}

/// systrayv2 554.7cdbd936: its popup uses the renderer's fixed dark palette.
pub struct TrayColors;
impl TrayColors {
    pub fn tooltip_border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub fn selected_text() -> Hsla {
        rgb(0xffffff).into()
    }
    pub fn accent() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub fn surface() -> Hsla {
        rgb(0x222222).into()
    }
    pub fn border() -> Hsla {
        rgb(0x000000).into()
    }
    pub fn launcher() -> Hsla {
        rgb(0x111111).into()
    }
    pub fn text() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub fn muted() -> Hsla {
        rgb(0x999999).into()
    }
    pub fn hover_text() -> Hsla {
        rgb(0xeeeeee).into()
    }
}

/// Current device and Profiles CSS `.nav-tabs .nav:active`.
pub struct NavigationColors;
impl NavigationColors {
    pub fn pressed() -> Hsla {
        rgb(0x3cbf27).into()
    }
}

/// Current 3334/3337 `.warning-alert` and `.backdrop`; independent of host theme.
pub struct StreamMixerColors;
impl StreamMixerColors {
    pub fn warning() -> Hsla {
        rgb(0xfd8611).into()
    }
    pub fn panel() -> Hsla {
        rgb(0x111111).into()
    }
    pub fn text() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub fn accent() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub fn secondary() -> Hsla {
        rgb(0x707070).into()
    }
    pub fn button_text() -> Hsla {
        rgb(0x212121).into()
    }
    pub fn backdrop() -> Hsla {
        rgba(0x00000080).into()
    }
}

/// Current 515 `.key-record-item`, `.snap-tap-*` and warning dialog palette.
pub struct SnapTapColors;
impl SnapTapColors {
    pub fn border() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub fn text() -> Hsla {
        rgb(0xffffff).into()
    }
    pub fn muted() -> Hsla {
        rgb(0x999999).into()
    }
    pub fn warning() -> Hsla {
        rgb(0xfd8611).into()
    }
    pub fn accent() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub fn success() -> Hsla {
        rgb(0x00ff00).into()
    }
    pub fn panel() -> Hsla {
        rgb(0x111111).into()
    }
    pub fn tooltip_border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub fn backdrop() -> Hsla {
        rgba(0x00000080).into()
    }
}

/// 226 current `.swtm-*` controls and pointer tooltip.
pub struct ScrollWheelColors;
impl ScrollWheelColors {
    pub fn text() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub fn muted() -> Hsla {
        rgb(0x999999).into()
    }
    pub fn border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub fn accent() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub fn pills() -> Hsla {
        rgba(0x00000066).into()
    }
    pub fn pill_text() -> Hsla {
        rgba(0xffffffb3).into()
    }
    pub fn selected_text() -> Hsla {
        rgba(0x000000e6).into()
    }
    pub fn tooltip() -> Hsla {
        rgb(0x000000).into()
    }
}
pub struct DpiColors;
impl DpiColors {
    pub fn tick() -> Hsla {
        rgb(0x204c19).into()
    }
    pub fn text() -> Hsla {
        rgb(0xcccccc).into()
    }
}
/// Current Chroma Settings 75.e743878a.css palette; independent of OS theme.

pub struct ChromaSettingsColors;
impl ChromaSettingsColors {
    pub fn background() -> Hsla {
        rgb(0x222222).into()
    }
    pub fn panel() -> Hsla {
        rgb(0x111111).into()
    }
    pub fn text() -> Hsla {
        rgb(0xcccccc).into()
    }
    pub fn secondary() -> Hsla {
        rgb(0x999999).into()
    }
    pub fn note() -> Hsla {
        rgb(0x707070).into()
    }
    pub fn selected() -> Hsla {
        rgb(0x44d62c).into()
    }
    pub fn checkbox_border() -> Hsla {
        rgb(0x737373).into()
    }
    pub fn border() -> Hsla {
        rgb(0x5d5d5d).into()
    }
    pub fn hover() -> Hsla {
        rgb(0x2d2d2d).into()
    }
    pub fn help() -> Hsla {
        rgb(0x4a4a4a).into()
    }
    pub fn help_hover() -> Hsla {
        rgba(0xffffff4d).into()
    }
    pub fn tooltip() -> Hsla {
        rgb(0x000000).into()
    }
}
