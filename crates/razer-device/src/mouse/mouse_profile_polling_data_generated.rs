// Fixed product gates for onboard profile polling commands.
pub const PROFILE_POLLING_PRODUCT_IDS: &[u32] = &[170, 204, 226, 45066];
pub const PROFILE_POLLING_RATES: &[(u16, u8)] = &[
    (1000, 1),
    (500, 2),
    (333, 3),
    (250, 4),
    (200, 5),
    (125, 8),
    (100, 10),
];
