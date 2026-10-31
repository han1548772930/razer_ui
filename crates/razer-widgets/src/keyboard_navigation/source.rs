//! Keyboard runtime installation belongs to the mounted source document.

/// The product URL identity, before a device's physical-ID resolution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductId(pub u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostPage {
    SynapseDashboard,
    SynapseSettings,
    ChromaDashboard,
    ChromaSettings,
    ChromaSensa,
    ChromaStudio,
    SynapseTour,
    ChromaTour,
    Macro,
    Profiles,
    Pairing,
    ProfileMigration,
    HostSettings,
    AppMenu,
    FirmwareUpdate,
    Armory,
    AudioVisualizer,
    ChromaConnect,
    ReleaseNotes,
    Alexa,
    BackgroundManager,
    Installer,
    Natalie,
    NotificationManager,
    GmsProxy,
    LightingEngine,
    Systray,
    SynapseRoot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourcePage {
    Host(HostPage),
    Product(ProductId),
}

/// Absence and unavailable source remain distinct; neither installs the runtime.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DocumentNavigation {
    Shared,
    SharedWithoutInitialGuide,
    Native,
    Unverified,
}

impl DocumentNavigation {
    pub(super) fn installed(self) -> bool {
        matches!(self, Self::Shared | Self::SharedWithoutInitialGuide)
    }

    pub(super) fn initial_guide(self) -> bool {
        self == Self::Shared
    }
}

impl SourcePage {
    pub fn navigation(self) -> DocumentNavigation {
        match self {
            Self::Product(ProductId(product)) => {
                if SHARED_PRODUCTS.contains(&product) {
                    DocumentNavigation::Shared
                } else if NATIVE_PRODUCTS.contains(&product) {
                    DocumentNavigation::Native
                } else {
                    DocumentNavigation::Unverified
                }
            }
            Self::Host(page) => match page {
                HostPage::SynapseDashboard
                | HostPage::SynapseSettings
                | HostPage::ChromaDashboard
                | HostPage::ChromaSettings
                | HostPage::ChromaSensa
                | HostPage::ChromaStudio
                | HostPage::SynapseTour
                | HostPage::ChromaTour
                | HostPage::Macro
                | HostPage::Profiles
                | HostPage::Pairing
                | HostPage::ProfileMigration
                | HostPage::HostSettings
                | HostPage::Armory
                | HostPage::AudioVisualizer
                | HostPage::ChromaConnect => DocumentNavigation::Shared,
                // The mounted lazy Root initializes with showGuideOnFirstTab:false.
                HostPage::AppMenu => DocumentNavigation::SharedWithoutInitialGuide,
                HostPage::FirmwareUpdate
                | HostPage::ReleaseNotes
                | HostPage::Alexa
                | HostPage::BackgroundManager
                | HostPage::Installer
                | HostPage::Natalie
                | HostPage::NotificationManager
                | HostPage::GmsProxy
                | HostPage::LightingEngine
                | HostPage::Systray => DocumentNavigation::Native,
                HostPage::SynapseRoot => DocumentNavigation::Unverified,
            },
        }
    }
}

// Current mounted main loaders, including their invoked root/effect owners.
const SHARED_PRODUCTS: &[u32] = &[
    70, 80, 83, 89, 92, 96, 98, 99, 101, 103, 104, 105, 106, 107, 108, 110, 112, 113, 115, 116,
    117, 120, 122, 124, 126, 128, 131, 132, 133, 134, 138, 140, 141, 143, 145, 148, 150, 152, 153,
    154, 156, 158, 161, 163, 164, 165, 170, 175, 178, 184, 190, 192, 196, 199, 203, 204, 208, 211,
    214, 218, 222, 224, 226, 229, 235, 239, 515, 521, 529, 534, 542, 545, 550, 551, 552, 554, 555,
    556, 563, 564, 565, 567, 569, 570, 571, 574, 575, 576, 577, 579, 580, 581, 582, 585, 586, 587,
    588, 589, 590, 591, 592, 593, 594, 595, 597, 598, 599, 600, 601, 602, 605, 606, 614, 616, 617,
    618, 619, 620, 621, 622, 623, 624, 630, 631, 633, 634, 642, 650, 651, 652, 653, 654, 655, 658,
    661, 664, 669, 670, 671, 672, 673, 677, 678, 679, 688, 689, 691, 694, 695, 696, 697, 708, 709,
    710, 711, 716, 717, 719, 720, 721, 724, 728, 736, 737, 739, 740, 741, 742, 746, 747, 777, 778,
    780, 781, 782, 783, 784, 790, 791, 1303, 1304, 1306, 1308, 1310, 1313, 1318, 1319, 1325, 1328,
    1332, 1335, 1342, 1346, 1352, 1365, 1376, 1378, 1382, 1383, 1387, 1389, 1390, 1392, 1396, 1398,
    1401, 1406, 1407, 1415, 1420, 1427, 1439, 1442, 1462, 2594, 2595, 2596, 2629, 2636, 2638, 2641,
    2647, 2650, 2657, 2660, 2664, 2668, 2689, 3072, 3073, 3074, 3076, 3077, 3080, 3331, 3334, 3337,
    3587, 3589, 3590, 3592, 3594, 3595, 3596, 3849, 3853, 3858, 3863, 3866, 3867, 3869, 3871, 3872,
    3873, 3878, 3879, 3880, 3883, 3884, 3894, 3900, 3921, 3922, 3929, 3932, 3940, 3946, 3949, 4115,
    4124, 4126, 4133, 4144, 45066,
];

// These documents' current HTML and captured JavaScript closure install no runtime.
const NATIVE_PRODUCTS: &[u32] = &[
    100, 147, 162, 167, 179, 180, 182, 185, 194, 207, 221, 231, 241, 647, 659, 660, 674, 675, 727,
    752, 756, 769, 1312, 1320, 1321, 1330, 1331, 1337, 1339, 1347, 1353, 1354, 1364, 1370, 1372,
    1374, 1386, 1391, 1404, 1411, 1412, 1422, 1443, 1446, 1451, 1453, 1465, 2644, 2676, 2684, 3078,
    3848, 3859, 3893, 3907, 3909, 3942, 4130, 45061, 45063,
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_installation_keeps_absence_and_unavailable_source_distinct() {
        assert_eq!(SHARED_PRODUCTS.len(), 274);
        assert_eq!(NATIVE_PRODUCTS.len(), 60);
        for products in [SHARED_PRODUCTS, NATIVE_PRODUCTS] {
            assert!(products.windows(2).all(|pair| pair[0] < pair[1]));
        }
        for product in SHARED_PRODUCTS {
            assert!(!NATIVE_PRODUCTS.contains(product));
            assert_eq!(
                SourcePage::Product(ProductId(*product)).navigation(),
                DocumentNavigation::Shared
            );
        }
        for product in [680, 3886, u32::MAX] {
            assert_eq!(
                SourcePage::Product(ProductId(product)).navigation(),
                DocumentNavigation::Unverified
            );
        }
        assert!(
            !SourcePage::Host(HostPage::AppMenu)
                .navigation()
                .initial_guide()
        );
        assert!(SourcePage::Host(HostPage::AppMenu).navigation().installed());
    }
}
