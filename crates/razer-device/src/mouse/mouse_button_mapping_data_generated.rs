// Fixed product capabilities for firmware button assignments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum ConsumerAction {
    MediaEject = 184,
    MuteVolume = 226,
    VolumeDown = 234,
    VolumeUp = 233,
    PreviousTrack = 182,
    MediaStop = 183,
    MediaPlayOrPause = 205,
    NextTrack = 181,
    CycleApps = 671,
    DisplayBrightnessUp = 111,
    DisplayBrightnessDown = 112,
    SpotlightSearch = 545,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum RazerKey {
    FnKey = 1,
    RazerKey = 2,
    GameModeKey = 3,
    MacroRecordKey = 4,
    DisplayInternalExternal = 5,
    DisplayBrightnessUp = 6,
    DisplayBrightnessDown = 7,
    KeyboardBacklightUp = 8,
    KeyboardBacklightDown = 9,
    ToggleLowPowerMode = 11,
    MouseDPIUpButton = 32,
    MouseDPIDownButton = 33,
    MouseRightBackwardButton = 34,
    MouseRightForwardButton = 35,
    MacroKeys = 36,
    SystemSleep = 76,
    MacroKeysEnd = 79,
    ProfileSwitchButton = 80,
    MouseSniperButton = 81,
    MouseDPICycleUpButton = 82,
    MouseDPICycleDownButton = 83,
    MouseScrollWheelFreeNotchSwitch = 84,
    HapticScrollStageUp = 85,
    HapticScrollStageDown = 86,
    HapticScrollStageCycleUp = 87,
    HapticScrollStageCycleDown = 88,
    HyperShiftButton = 89,
    ActuationAdjustmentMode = 112,
    RapidTriggerAdjustmentMode = 113,
    SnapTapToggle = 117,
    MicMute = 216,
    PrintScreen = 70,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum PowerAction {
    SystemPower = 129,
    SystemSleep = 130,
    SystemWake = 131,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum WindowsShortcut {
    Screenshot = 99,
    XboxGameBar = 100,
    CycleApps = 101,
}

pub const AUDITED_MAPPING_DESCRIPTORS: &[MouseMappingDescriptor] = &[
    MouseMappingDescriptor {
        product_id: 170,
        function_ids: &[0, 1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 12, 13, 14, 15, 17, 18, 21, 22],
        consumer_codes: &[184, 226, 234, 233, 182, 183, 205, 181, 671, 111, 112, 545],
        razer_codes: &[1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 32, 33, 34, 35, 36, 76, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 112, 113, 117, 216, 70],
        windows_codes: &[99, 100, 101],
        power_codes: &[129, 130, 131],
    },
    MouseMappingDescriptor {
        product_id: 204,
        function_ids: &[0, 1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 12, 13, 14, 15, 17, 18, 21, 22],
        consumer_codes: &[184, 226, 234, 233, 182, 183, 205, 181, 671, 111, 112, 545],
        razer_codes: &[1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 32, 33, 34, 35, 36, 76, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 112, 113, 117, 216, 70],
        windows_codes: &[99, 100, 101],
        power_codes: &[129, 130, 131],
    },
    MouseMappingDescriptor {
        product_id: 226,
        function_ids: &[0, 1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 12, 13, 14, 15, 17, 18, 21, 22],
        consumer_codes: &[184, 226, 234, 233, 182, 183, 205, 181, 671, 111, 112, 545],
        razer_codes: &[1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 32, 33, 34, 35, 36, 76, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 112, 113, 117, 216, 70],
        windows_codes: &[99, 100, 101],
        power_codes: &[129, 130, 131],
    },
    MouseMappingDescriptor {
        product_id: 45066,
        function_ids: &[0, 1, 2, 3, 4, 5, 6, 7, 9, 10, 11, 12, 13, 14, 15, 17, 18, 21],
        consumer_codes: &[184, 226, 234, 233, 182, 183, 205, 181, 671, 111, 112, 545],
        razer_codes: &[1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 32, 33, 34, 35, 36, 76, 79, 80, 81, 82, 83, 84, 85, 86, 87, 88, 89, 112, 113, 117, 216, 70],
        windows_codes: &[99, 100, 101],
        power_codes: &[129, 130, 131],
    },
];
