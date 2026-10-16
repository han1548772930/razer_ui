//! Source pairing storage projection; runtime uses no JSON catalog.
//! Maintenance input SHA-256: 9f879410a15a51faaf04706e188e87efb9ba3ff7aedf15d2cb5efd986f0f74e2
use crate::receiver_defaults::PairingMaster;

#[rustfmt::skip]
pub(crate) static MASTERS: &[PairingMaster] = &[
    PairingMaster {
        product_id: 164, dongle_id: 164, ble_id: 0, claim_interface: 0,
        supports_eight_khz: None,
    },
    PairingMaster {
        product_id: 241, dongle_id: 241, ble_id: 0, claim_interface: 3,
        supports_eight_khz: Some(true),
    },
    PairingMaster {
        product_id: 756, dongle_id: 757, ble_id: 758, claim_interface: 3,
        supports_eight_khz: None,
    },
    PairingMaster {
        product_id: 179, dongle_id: 179, ble_id: 0, claim_interface: 0,
        supports_eight_khz: None,
    },
];

// Source order is retained; this set only gates the source756 callback.
#[rustfmt::skip]
pub(crate) static RADIO_IDS: &[u16] = &[
    604, 625, 698, 144, 168, 171, 212, 205, 227, 234,
    215, 45067, 125, 148, 232, 166, 183, 156, 154, 176,
    184, 635, 656, 662, 722, 725, 713, 730, 164, 241,
    179, 185, 180, 197, 191, 193, 207, 219, 223, 757,
    45061, 45063,
];
