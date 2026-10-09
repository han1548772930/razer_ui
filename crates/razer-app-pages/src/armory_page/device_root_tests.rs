//! Receipts for the product-side `displayMode=armory` panel geometry.
//!
//! The heights come from the current Armory bundle's `ie` switch; they are the
//! frame height the host gives the embedded product root.
use super::device_frame_height;
use razer_model::model::DeviceCategory;

#[test]
fn frame_height_follows_the_device_type_switch() {
    assert_eq!(device_frame_height(DeviceCategory::Keypad), 460.);
    assert_eq!(device_frame_height(DeviceCategory::Headset), 340.);
    assert_eq!(device_frame_height(DeviceCategory::Audio), 340.);
    // `default: return "420px"` covers mice, keyboards, mats and accessories.
    for category in [
        DeviceCategory::Mouse,
        DeviceCategory::Keyboard,
        DeviceCategory::Mousepad,
        DeviceCategory::Accessory,
        DeviceCategory::Controller,
        DeviceCategory::Other,
    ] {
        assert_eq!(device_frame_height(category), 420., "{category:?}");
    }
}
