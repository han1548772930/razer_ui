const ASSETS: &[(&str, &[u8])] = include!("embedded.rs");
const AUDIO_MIXER_ASSETS: &[(&str, &[u8])] = &[
    (
        "synapse/audio-mixer-lighting-direction-left.svg",
        include_bytes!("../shared/source-unverified/5941810302129850751968dbad3973eee8b3f0065c05fc47a24dcf6b968463e6.svg"),
    ),
    (
        "synapse/audio-mixer-lighting-direction-left-active.svg",
        include_bytes!("../shared/source-unverified/0bf39215e3ea3f39e2cf75f03eb93805d418883f8f1201d8aa5873ba35c6e111.svg"),
    ),
    (
        "synapse/audio-mixer-lighting-direction-right.svg",
        include_bytes!("../shared/source-unverified/d3809db286279a6b146711026cf88372d525e3c8b195ac90488f7f4ef24c6926.svg"),
    ),
    (
        "synapse/audio-mixer-lighting-direction-right-active.svg",
        include_bytes!("../shared/source-unverified/a07a5cfb088a0103cb097436e6dc4ca3309c7eb115d33ed7b3d04a7f0ece69e8.svg"),
    ),
    (
        "synapse/audio-mixer-lighting-direction-cw.svg",
        include_bytes!("../products/653/57b6cb9f6f45d016-direction-cw.svg"),
    ),
    (
        "synapse/audio-mixer-lighting-direction-cw-active.svg",
        include_bytes!("../products/653/682abb4cfcbe0b19-direction-cw-active.svg"),
    ),
    (
        "synapse/audio-mixer-lighting-direction-ccw.svg",
        include_bytes!("../products/653/ae872f3333639d2e-direction-ccw.svg"),
    ),
    (
        "synapse/audio-mixer-lighting-direction-ccw-active.svg",
        include_bytes!("../products/653/289e47a4c1b43c7f-direction-ccw-active.svg"),
    ),
    (
        "synapse/audio-mixer-bleep-button.svg",
        include_bytes!("../shared/source-unverified/ab5a30324433a75312c88674964c1372cb82bf87e875049a2e12d127af62ab30.svg"),
    ),
    (
        "synapse/audio-mixer-mic-mute-button.svg",
        include_bytes!("../shared/source-unverified/9b294c3220de3fa767d4cfe54bfeed4d4e34ff07ca66557e367c4ba9a1e43a38.svg"),
    ),
    (
        "synapse/audio-mixer-product-1x.png",
        include_bytes!("../shared/source-unverified/5bc38c4b54844adb76fafb5c867e3c800aee61f9ebe394952e1c908c333ac29d.png"),
    ),
    (
        "synapse/audio-mixer-product-3x.png",
        include_bytes!("../shared/source-unverified/183e8ed83a0acff8235cd4956595a453ac0aa276cac0393fce2f6a96de600476.png"),
    ),
    (
        "synapse/audio-mixer-output-info.svg",
        include_bytes!("../shared/source-unverified/7bb03d6ce30647370745b47f870b67c837222879c1067ef90d83dc556bdbdea0.svg"),
    ),
    (
        "synapse/audio-mixer-stepper-down.svg",
        include_bytes!("../shared/source-unverified/45f867045c1ca91e478d143e26ca6b8dd606f265336c9c5e60dd57cea3cbf7aa.svg"),
    ),
    (
        "synapse/audio-mixer-stepper-up.svg",
        include_bytes!("../shared/source-unverified/f56d5fccf24c370b7f170057e6afa79636e2ac08b403aa629d126ff19d40a93c.svg"),
    ),
    (
        "synapse/audio-mixer-volume-high.svg",
        include_bytes!("../shared/source-unverified/69c9dabce9dafe854e70f609aeda10e1bd063607c2c9698b4bafb3f3b06e0832.svg"),
    ),
    (
        "synapse/audio-mixer-volume-mute.svg",
        include_bytes!("../shared/source-unverified/7bc01e6855ddb9e8ab3b50ffe5a18fd66e2e456243671e8dc8248546d0461b3c.svg"),
    ),
    (
        "synapse/audio-mixer-line-out.svg",
        include_bytes!("../shared/source-unverified/6e90653be531eb520f2b416c64644b811586f296c0ee3a52f8face4e0a89185b.svg"),
    ),
    (
        "synapse/audio-mixer-stream-mix.svg",
        include_bytes!("../shared/source-unverified/6451245e3d2c4ba5db9fb74415a4f8a030f9c8db21163756c7efe8070bd45f1d.svg"),
    ),
    (
        "synapse/audio-mixer-playback-mix.svg",
        include_bytes!("../shared/source-unverified/e775dd347ce3b8eeb2cbf51a8f4fbab55cd58de3e650a1e36d0f7791a6b28f26.svg"),
    ),
    (
        "synapse/audio-mixer-voice-chat.svg",
        include_bytes!("../shared/source-unverified/fda0ac362439a6066515115efe6870cab2830227eb55f3f10467d48faacc878d.svg"),
    ),
    (
        "synapse/audio-mixer-route-high.svg",
        include_bytes!("../shared/source-unverified/f97add9a32baab3c1dfefaf27a60a4966337e058245cde8abfd6474477428c56.svg"),
    ),
    (
        "synapse/audio-mixer-route-mute.svg",
        include_bytes!("../shared/source-unverified/f97add9a32baab3c1dfefaf27a60a4966337e058245cde8abfd6474477428c56.svg"),
    ),
    (
        "synapse/audio-mixer-mic-preview-close.svg",
        include_bytes!("../shared/source-unverified/41d47f12855bbac51d0cedbc816a4f677c96b75d0a880f29de019fbda4e860b6.svg"),
    ),
    (
        "synapse/audio-mixer-mic-jack.svg",
        include_bytes!("../shared/source-unverified/a4869ecd6e8faf99c17253c3760ef0333e34439fb450878cd58f4ebc4e58a435.svg"),
    ),
    (
        "synapse/audio-mixer-mic-xlr.svg",
        include_bytes!("../shared/source-unverified/f5424c67cfdf4f802b91c12bde151683c6b71bf1bc7127a4096e8205ddc59ce7.svg"),
    ),
    (
        "synapse/audio-mixer-mic-preview.svg",
        include_bytes!("../shared/source-unverified/2c15236ce5ac1959c23d763bf99d92900d0a226f612f1bf263bd8c30f64d646e.svg"),
    ),
    (
        "synapse/audio-mixer-eq-reset.svg",
        include_bytes!("../shared/products/6812181e240494d9-audio-sound-6812181e240494d9.svg"),
    ),
    (
        "synapse/audio-mixer-eq-reset-hover.svg",
        include_bytes!("../shared/products/445d4417e9b5f164-audio-sound-445d4417e9b5f164.svg"),
    ),
    (
        "synapse/audio-mixer-eq-reset-active.svg",
        include_bytes!("../shared/products/2eb9a74f22665f9c-audio-sound-2eb9a74f22665f9c.svg"),
    ),
];
const AUDIO_ENHANCEMENT_ASSETS: &[(&str, &[u8])] =
    include!("audio-enhancements-embedded.rs");
const AUDIO_SOUND_ASSETS: &[(&str, &[u8])] =
    include!("audio-sound-embedded.rs");
const MOUSE_LAUNCH_CURRENT_ASSETS: &[(&str, &[u8])] =
    include!("mouse-launch-current-embedded.rs");
const OWNER_SMART_SWITCH_ASSETS: &[(&str, &[u8])] =
    include!("owner-smart-switch-embedded.rs");
const OWNER_WHATS_NEW_ASSETS: &[(&str, &[u8])] =
    include!("owner-whats-new-embedded.rs");
const HELP_ANALOG_ASSETS: &[(&str, &[u8])] =
    include!("help-analog-embedded.rs");
const HELP_OWNER_ASSETS: &[(&str, &[u8])] = &[
    (
        "synapse/help-owner-1398.webp",
        include_bytes!(
            "../products/1398/161262b4c4b34d89-help-owner-1398.webp"
        ),
    ),
    (
        "synapse/help-owner-1453.webp",
        include_bytes!(
            "../products/1453/7fd897db96f346d6-help-owner-1453.webp"
        ),
    ),
    (
        "synapse/help-owner-close.svg",
        include_bytes!(
            "../shared/source-unverified/4e56083704455350d03163789384b1f7386c1007d71df246dbed9a4074626391.svg"
        ),
    ),
    (
        "synapse/help-owner-sound-external.svg",
        include_bytes!(
            "../shared/products/55ee27113c83c64b8c4e26ca1dae9403375a70c9180a06cdf3492aedaeae07d3.svg"
        ),
    ),
];
const HELP_SYSTEM_ASSETS: &[(&str, &[u8])] = &[
    (
        "synapse/help-system-expand.svg",
        include_bytes!("../shared/products/238cd792ac3ceebc-expand.svg"),
    ),
    (
        "synapse/help-system-download.svg",
        include_bytes!("../shared/products/2f9ea56768e57f27-download.svg"),
    ),
];
const HELP_CAMO_ASSETS: &[(&str, &[u8])] = &[
    (
        "synapse/help-camo-copy-hover.svg",
        include_bytes!(
            "../shared/source-unverified/9e3939f4bdc80899960c0828e43d1fb157fb3e89a389b63715406367c207337f.svg"
        ),
    ),
    (
        "synapse/help-camo-external-hover.svg",
        include_bytes!(
            "../shared/source-unverified/ca36b57aff808108c553ebd125ce620478ef4554976f31a7c4be3d90eb93c3da.svg"
        ),
    ),
    (
        "synapse/help-camo-copy.svg",
        include_bytes!(
            "../shared/products/fbec7d1c9d77bee21f2dc52089cf2ae8f0107e2a5c6d6cc1354b064404e7000a.svg"
        ),
    ),
    (
        "synapse/help-camo-external.svg",
        include_bytes!(
            "../shared/products/95d03a6f1a2915a0ab98744f4e507c0f749948545d8444432a7fbfc2d66779cf.svg"
        ),
    ),
];
const MODULE_SERVICE_ASSETS: &[(&str, &[u8])] =
    include!("module-service-embedded.rs");
const AUDIO_OLED_HOME_ASSETS: &[(&str, &[u8])] =
    include!("audio-oled-home-embedded.rs");
const AUDIO_OLED_ARTWORK_ASSETS: &[(&str, &[u8])] =
    include!("audio-oled-artwork-embedded.rs");
const AUDIO_OLED_BANNER_ASSETS: &[(&str, &[u8])] =
    include!("audio-oled-banner-embedded.rs");
const AUDIO_OLED_SYSTEM_ASSETS: &[(&str, &[u8])] =
    include!("audio-oled-system-embedded.rs");
const TRAY_ACCOUNT_ASSETS: &[(&str, &[u8])] =
    include!("tray-account-embedded.rs");
const TRAY_WIDGET_ASSETS: &[(&str, &[u8])] =
    include!("tray-widget-embedded.rs");
const SETTINGS_WINDOW_ASSETS: &[(&str, &[u8])] =
    include!("settings-window-embedded.rs");
const CHROMA_SETTINGS_ASSETS: &[(&str, &[u8])] =
    include!("chroma-settings-embedded.rs");
const CHROMA_STUDIO_ASSETS: &[(&str, &[u8])] =
    include!("chroma-studio-embedded.rs");
const CHROMA_STUDIO_COLOR_ASSETS: &[(&str, &[u8])] =
    include!("chroma-studio-color-embedded.rs");
const KITSUNE_ASSETS: &[(&str, &[u8])] = include!("kitsune-embedded.rs");
const GAMEPAD_DIALOG_ASSETS: &[(&str, &[u8])] =
    include!("gamepad-2636-embedded.rs");
const GAMEPAD_CALIBRATION_ASSETS: &[(&str, &[u8])] =
    include!("gamepad-2636-calibration-embedded.rs");
const GAMEPAD_CALIBRATION_CURRENT_ASSETS: &[(&str, &[u8])] =
    include!("gamepad-calibration-current-embedded.rs");
const GAMEPAD_TRIGGER_SELECTION_ASSETS: &[(&str, &[u8])] =
    include!("gamepad-trigger-selection-embedded.rs");
const MAPPING_AI_ASSETS: &[(&str, &[u8])] = &[
    (
        "synapse/mouse-default-ai-long-press.svg",
        include_bytes!(
            "../shared/source-unverified/a3a521fab8e5225f2ff68917073978848944512f6971213f8ef7b1a7ee29b1cb.svg"
        ),
    ),
    (
        "synapse/mapping-190-requires-synapse.svg",
        include_bytes!("../shared/products/f74f198ee96c8058-synapse.svg"),
    ),
    (
        "synapse/mapping-ai.svg",
        include_bytes!("../shared/products/9e902836c3c18c9d08a3a1020436f294b1a8c674a99ea6d913b307316438c3fb.svg"),
    ),
    (
        "synapse/mapping-ai-active.svg",
        include_bytes!(
            "../shared/products/09b35421b751aeabad5a74a247ace6d765782b44a555d818d42df09a6d4844ee.svg"
        ),
    ),
];
const KEYBOARD_MAPPING_AI_ASSETS: &[(&str, &[u8])] =
    include!("keyboard-mapping-ai-embedded.rs");
const KEYBOARD_KEYPAD_ARROWS_ASSETS: &[(&str, &[u8])] =
    include!("keyboard-keypad-arrows-embedded.rs");
const KEYBOARD_KEYPAD_SECONDARY_ASSETS: &[(&str, &[u8])] =
    include!("keyboard-keypad-secondary-embedded.rs");
const KEYBOARD_PANEL_DROPDOWN_ASSETS: &[(&str, &[u8])] =
    include!("keyboard-panel-dropdown-embedded.rs");
const KEYBOARD_KEYPAD_ARTWORK_ASSETS: &[(&str, &[u8])] =
    include!("keyboard-keypad-artwork-embedded.rs");
const KEYBOARD_KEYPAD_DEVICE_IMAGE_ASSETS: &[(&str, &[u8])] =
    include!("keyboard-keypad-device-image-embedded.rs");
const KEYBOARD_MAPPING_LOGIN_WARNING_ASSETS: &[(&str, &[u8])] =
    include!("keyboard-mapping-login-warning-embedded.rs");
const RIPSAW_SETTINGS_ASSETS: &[(&str, &[u8])] =
    include!("ripsaw-settings-embedded.rs");
const HYPERFLUX_ASSETS: &[(&str, &[u8])] =
    include!("hyperflux-207-embedded.rs");
const MULTIPAIRING_CURRENT_ASSETS: &[(&str, &[u8])] =
    include!("multipairing-current-embedded.rs");
const MULTIPAIRING_PRODUCT_ASSETS: &[(&str, &[u8])] =
    include!("multipairing-products-embedded.rs");
const KEYBOARD_MAPPING_BLADE_ASSETS: &[(&str, &[u8])] =
    include!("keyboard-mapping-blade-embedded.rs");
const KEYBOARD_MAPPING_NOTICES_ASSETS: &[(&str, &[u8])] =
    include!("keyboard-mapping-notices-embedded.rs");
const KEYBOARD_MAPPING_RECORDING_ASSETS: &[(&str, &[u8])] =
    include!("keyboard-mapping-recording-embedded.rs");
const KEYBOARD_MAPPING_DYNAMIC_ASSETS: &[(&str, &[u8])] =
    include!("keyboard-mapping-dynamic-embedded.rs");
const KEYBOARD_MAPPING_SENSITIVITY_WARNING_ASSETS: &[(&str, &[u8])] =
    include!("keyboard-mapping-sensitivity-warning-embedded.rs");
const KEYBOARD_KEYMAP_BAR_ASSETS: &[(&str, &[u8])] =
    include!("keyboard-keymap-bar-embedded.rs");
const PROFILES_TRANSFER_ASSETS: &[(&str, &[u8])] =
    include!("profiles-transfer-embedded.rs");
const MONITOR_INPUT_ASSETS: &[(&str, &[u8])] =
    include!("monitor-input-embedded.rs");
const STREAM_MIXER_ASSETS: &[(&str, &[u8])] =
    include!("stream-mixer-embedded.rs");
const SNAP_TAP_ASSETS: &[(&str, &[u8])] = include!("snap-tap-embedded.rs");
const KEYBOARD_PROPERTIES_ASSETS: &[(&str, &[u8])] =
    include!("keyboard-properties-embedded.rs");
const KEYBOARD_ANALOG_ASSETS: &[(&str, &[u8])] =
    include!("keyboard-analog-embedded.rs");
const KEYBOARD_FLOATING_CONTROLLER_ASSETS: &[(&str, &[u8])] =
    include!("keyboard-floating-controller-embedded.rs");
const KEYBOARD_SOURCE_ARTWORK_ASSETS: &[(&str, &[u8])] =
    include!("keyboard679-current-embedded.rs");
const KEYBOARD_MOD_TAP_ASSETS: &[(&str, &[u8])] = &[(
    "synapse/keyboard-688-mod-tap.svg",
    include_bytes!(
        "../products/688/4782f618530a4c2b-keyboard-688-mod-tap.svg"
    ),
)];
const KEYBOARD_GAME_CONTROLLER_ASSETS: &[(&str, &[u8])] = &[(
    "synapse/keyboard-game-controller.svg",
    include_bytes!(
        "../shared/source-unverified/e697df0aa22554cf36a1f4c710be61d5835b2fa7bf2405ad07ad432caf74effd.svg"
    ),
)];
const KEYBOARD_ACTUATION_ASSETS: &[(&str, &[u8])] =
    include!("keyboard-actuation-embedded.rs");
const KEYBOARD_INDICATOR_ASSETS: &[(&str, &[u8])] = &[(
    "synapse/keyboard-indicator-connection-body.png",
    include_bytes!(
        "../products/691/0b3bfdbf13136c2d-keyboard-indicator-battery-body.png"
    ),
)];
const KEYBOARD_POWER_POLLING_ASSETS: &[(&str, &[u8])] =
    include!("keyboard-power-polling-embedded.rs");
const KEYBOARD_756_ASSETS: &[(&str, &[u8])] = &[(
    "synapse/keyboard-product-756.svg",
    include_bytes!(
        "../products/756/25b9fd6f8bd9ea75b293aae873baec0cc23b59cb4eeab903743a70bcbf80640f.svg"
    ),
)];
const KEYBOARD_LIGHTING_ASSETS: &[(&str, &[u8])] = &[
    (
        "synapse/keyboard-logo-visualizer.svg",
        include_bytes!(
            "../shared/products/9a03a852d178f619a0fb87c80ebd650eaaa408448550c4be6b02f449fb2c18a3.svg"
        ),
    ),
    (
        "synapse/keyboard-chroma-settings.svg",
        include_bytes!("../shared/products/7bf93d2d7e3c9c40-settings.svg"),
    ),
    (
        "synapse/keyboard-lighting-warning.svg",
        include_bytes!("../shared/products/689d3c61b20bce2ccaabf1c301046b8e7b664c8f7b627ac91204f99efaa6e6c1.svg"),
    ),
    (
        "synapse/keyboard-ble-warning.svg",
        include_bytes!(
            "../products/756/1a3219bad20cf1aeeda2b9f58b697c18a5c4c270e55c9944f0b33b6d02d771bb.svg"
        ),
    ),
    (
        "synapse/keyboard-logo-chromastudio.svg",
        include_bytes!(
            "../shared/products/2c32261f123494c4a00331f5cbbad1eaf2d3cc632ef90c9b279486d366e2d57c.svg"
        ),
    ),
    (
        "synapse/keyboard-chroma-install-spinner.svg",
        include_bytes!(
            "../shared/source-unverified/1611fad4ce4ebaa56138dcfe4c9ec4f34d9da6241ca7c00c936f7bad3a88bf01.svg"
        ),
    ),
];
const KEYBOARD_CHROMA_SYNC_ASSETS: &[(&str, &[u8])] =
    include!("keyboard-chroma-sync-embedded.rs");
const MOUSE_CALIBRATION_ASSETS: &[(&str, &[u8])] =
    include!("mouse-calibration-70-embedded.rs");
const MOUSE_SIDEBAR_CURRENT_ASSETS: &[(&str, &[u8])] =
    include!("mouse-sidebar-current-embedded.rs");
const MOUSE_MAPPING_MENU_CURRENT_ASSETS: &[(&str, &[u8])] =
    include!("mouse-mapping-menu-current-embedded.rs");
const MOUSE_CALIBRATION_162_ASSETS: &[(&str, &[u8])] =
    include!("mouse-calibration-162-embedded.rs");
const MOUSE_LIGHTING_ASSETS: &[(&str, &[u8])] =
    include!("mouse-lighting-embedded.rs");
const MOUSE_CUSTOMIZE_ASSETS: &[(&str, &[u8])] =
    include!("mouse-customize-embedded.rs");
const CHROMA_STUDIO_HOST_ASSETS: &[(&str, &[u8])] = &[(
    "synapse/host-chroma-studio-favicon.svg",
    include_bytes!(
        "../shared/source-unverified/f27627ab4e5eac03d2459ab6ea867d1ef9deb59b0350546f70bf7ab983960dea.svg"
    ),
)];
const AUDIO_OLED_RUNTIME_ASSETS: &[(&str, &[u8])] = &[(
    "synapse/audio-oled-runtime-warning.svg",
    include_bytes!(
        "../shared/source-unverified/125095bad55c3fd5fd6aecea910624d7e2e9987c7beaddb37bbc8f7d6177381b.svg"
    ),
)];
const AUDIO_LEVIATHAN_ASSETS: &[(&str, &[u8])] =
    include!("audio-leviathan-embedded.rs");
const AUDIO_DEMO_MEDIA_ASSETS: &[(&str, &[u8])] = &[
    ("synapse/audio-mode.rzav", include_bytes!("../products/1352/audio-mode.rzav")),
];
const MOUSE_POLLING_ASSETS: &[(&str, &[u8])] = &[
    (
        "synapse/polling-info.svg",
        include_bytes!("../shared/products/0e8f5d9d322ac12a-polling-info.svg"),
    ),
    (
        "synapse/mouse-rotation.svg",
        include_bytes!("../products/190/b1ee704bd4c09e21-mouse-rotation.svg"),
    ),
    (
        "synapse/mouse-scroll-link.svg",
        include_bytes!(
            "../products/167/0da93125c94e7642-mouse-scroll-link.svg"
        ),
    ),
    (
        "synapse/mouse-scroll-unlink.svg",
        include_bytes!(
            "../products/167/fe7816207da78d79-mouse-scroll-unlink.svg"
        ),
    ),
    (
        "synapse/mouse-scroll-close.svg",
        include_bytes!("../shared/products/53b67d2c7d30aa84-audio-sound-mapping-close.svg"),
    ),
    (
        "synapse/mouse-scroll-glow.svg",
        include_bytes!("../shared/products/dc9d485887b4b73450a5dc4f48b3275e62cda3d0beda636333af1444b236de2e.svg"),
    ),
];
const MOUSE_MATCHER_ASSETS: &[(&str, &[u8])] =
    include!("mouse-matcher-190-embedded.rs");
const RECEIVER_PARENT_ASSETS: &[(&str, &[u8])] = &[
    (
        "synapse/receiver/connection-skeleton-base.svg",
        include_bytes!(
            "../products/179/602a8b7b67b6dc05-connection-skeleton-base.svg"
        ),
    ),
    (
        "synapse/receiver/connection-skeleton-sweep.svg",
        include_bytes!(
            "../products/179/0524f61e6a22a0a4-connection-skeleton-sweep.svg"
        ),
    ),
];
// Keep source-table precedence stable: load uses the first matching alias;
// list keeps every entry, including duplicate aliases, in this order.
// A slice iterator avoids a concrete Chain type whose depth grows per table.
static ASSET_TABLES: &[&[(&str, &[u8])]] = &[
    ASSETS,
    MODULE_SERVICE_ASSETS,
    HELP_CAMO_ASSETS,
    HELP_SYSTEM_ASSETS,
    HELP_OWNER_ASSETS,
    HELP_ANALOG_ASSETS,
    OWNER_WHATS_NEW_ASSETS,
    OWNER_SMART_SWITCH_ASSETS,
    AUDIO_ENHANCEMENT_ASSETS,
    AUDIO_SOUND_ASSETS,
    AUDIO_MIXER_ASSETS,
    MOUSE_LAUNCH_CURRENT_ASSETS,
    AUDIO_OLED_HOME_ASSETS,
    AUDIO_OLED_ARTWORK_ASSETS,
    AUDIO_OLED_BANNER_ASSETS,
    AUDIO_OLED_SYSTEM_ASSETS,
    TRAY_ACCOUNT_ASSETS,
    TRAY_WIDGET_ASSETS,
    SETTINGS_WINDOW_ASSETS,
    CHROMA_SETTINGS_ASSETS,
    CHROMA_STUDIO_ASSETS,
    CHROMA_STUDIO_COLOR_ASSETS,
    KITSUNE_ASSETS,
    GAMEPAD_DIALOG_ASSETS,
    GAMEPAD_CALIBRATION_ASSETS,
    GAMEPAD_CALIBRATION_CURRENT_ASSETS,
    GAMEPAD_TRIGGER_SELECTION_ASSETS,
    MAPPING_AI_ASSETS,
    KEYBOARD_MAPPING_AI_ASSETS,
    KEYBOARD_KEYPAD_ARROWS_ASSETS,
    KEYBOARD_KEYPAD_SECONDARY_ASSETS,
    KEYBOARD_PANEL_DROPDOWN_ASSETS,
    KEYBOARD_KEYPAD_ARTWORK_ASSETS,
    KEYBOARD_KEYPAD_DEVICE_IMAGE_ASSETS,
    KEYBOARD_MAPPING_LOGIN_WARNING_ASSETS,
    RIPSAW_SETTINGS_ASSETS,
    HYPERFLUX_ASSETS,
    KEYBOARD_MAPPING_BLADE_ASSETS,
    KEYBOARD_MAPPING_NOTICES_ASSETS,
    KEYBOARD_MAPPING_RECORDING_ASSETS,
    KEYBOARD_MAPPING_DYNAMIC_ASSETS,
    MULTIPAIRING_CURRENT_ASSETS,
    MULTIPAIRING_PRODUCT_ASSETS,
    KEYBOARD_MAPPING_SENSITIVITY_WARNING_ASSETS,
    KEYBOARD_KEYMAP_BAR_ASSETS,
    PROFILES_TRANSFER_ASSETS,
    MONITOR_INPUT_ASSETS,
    STREAM_MIXER_ASSETS,
    SNAP_TAP_ASSETS,
    KEYBOARD_PROPERTIES_ASSETS,
    KEYBOARD_ANALOG_ASSETS,
    KEYBOARD_FLOATING_CONTROLLER_ASSETS,
    KEYBOARD_SOURCE_ARTWORK_ASSETS,
    KEYBOARD_MOD_TAP_ASSETS,
    KEYBOARD_GAME_CONTROLLER_ASSETS,
    KEYBOARD_ACTUATION_ASSETS,
    KEYBOARD_INDICATOR_ASSETS,
    KEYBOARD_POWER_POLLING_ASSETS,
    KEYBOARD_756_ASSETS,
    KEYBOARD_LIGHTING_ASSETS,
    KEYBOARD_CHROMA_SYNC_ASSETS,
    MOUSE_CALIBRATION_ASSETS,
    MOUSE_CALIBRATION_162_ASSETS,
    MOUSE_LIGHTING_ASSETS,
    MOUSE_CUSTOMIZE_ASSETS,
    MOUSE_SIDEBAR_CURRENT_ASSETS,
    MOUSE_MAPPING_MENU_CURRENT_ASSETS,
    CHROMA_STUDIO_HOST_ASSETS,
    AUDIO_OLED_RUNTIME_ASSETS,
    AUDIO_LEVIATHAN_ASSETS,
    AUDIO_DEMO_MEDIA_ASSETS,
    MOUSE_POLLING_ASSETS,
    MOUSE_MATCHER_ASSETS,
    RECEIVER_PARENT_ASSETS,
];
