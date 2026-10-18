// Fixed brightness command and Feature transport capabilities.
const AUDITED_KEYBOARDS: &[KeyboardSettingsDescriptor] = &[
    KeyboardSettingsDescriptor {
        product_id: 600,
        claim_interface: 3,
        claim_interfaces_by_pid: &[],
        hardware_profile_slots: 4,
    },
    KeyboardSettingsDescriptor {
        product_id: 602,
        claim_interface: 2,
        claim_interfaces_by_pid: &[],
        hardware_profile_slots: 4,
    },
    KeyboardSettingsDescriptor {
        product_id: 664,
        claim_interface: 3,
        claim_interfaces_by_pid: &[(662, 2), (663, 3), (664, 3)],
        hardware_profile_slots: 4,
    },
    KeyboardSettingsDescriptor {
        product_id: 678,
        claim_interface: 3,
        claim_interfaces_by_pid: &[],
        hardware_profile_slots: 5,
    },
    KeyboardSettingsDescriptor {
        product_id: 679,
        claim_interface: 3,
        claim_interfaces_by_pid: &[],
        hardware_profile_slots: 5,
    },
    KeyboardSettingsDescriptor {
        product_id: 688,
        claim_interface: 3,
        claim_interfaces_by_pid: &[],
        hardware_profile_slots: 5,
    },
    KeyboardSettingsDescriptor {
        product_id: 697,
        claim_interface: 3,
        claim_interfaces_by_pid: &[],
        hardware_profile_slots: 4,
    },
    KeyboardSettingsDescriptor {
        product_id: 708,
        claim_interface: 3,
        claim_interfaces_by_pid: &[(706, 2), (707, 3), (708, 3)],
        hardware_profile_slots: 1,
    },
    KeyboardSettingsDescriptor {
        product_id: 716,
        claim_interface: 3,
        claim_interfaces_by_pid: &[(713, 2), (714, 3), (716, 3)],
        hardware_profile_slots: 4,
    },
    KeyboardSettingsDescriptor {
        product_id: 717,
        claim_interface: 3,
        claim_interfaces_by_pid: &[(717, 3), (718, 3), (730, 2)],
        hardware_profile_slots: 4,
    },
    KeyboardSettingsDescriptor {
        product_id: 724,
        claim_interface: 3,
        claim_interfaces_by_pid: &[(722, 2), (723, 3), (724, 3)],
        hardware_profile_slots: 4,
    },
    KeyboardSettingsDescriptor {
        product_id: 727,
        claim_interface: 3,
        claim_interfaces_by_pid: &[(725, 2), (726, 3), (727, 3)],
        hardware_profile_slots: 4,
    },
];
