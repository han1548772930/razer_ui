//! Application pages, independent of host windows and AppShell.
#[path = "app_picker/app_picker.rs"]
pub mod app_picker;
#[path = "chroma/chroma_page.rs"]
pub mod chroma_page;
#[path = "chroma/chroma_settings.rs"]
pub mod chroma_settings;
mod data;
#[path = "firmware/firmware_update.rs"]
pub mod firmware_update;
#[path = "introduction/introduction_tour.rs"]
pub mod introduction_tour;
#[path = "gamer_room/iot_popup.rs"]
pub mod iot_popup;
#[path = "macros/macro_page.rs"]
pub mod macro_page;
#[path = "pairing/pairing_page.rs"]
pub mod pairing_page;
#[path = "profiles/profile_migration.rs"]
pub mod profile_migration;
#[path = "profiles/profiles_page.rs"]
pub mod profiles_page;
#[path = "release_notes/release_notes.rs"]
pub mod release_notes;
#[path = "services/service_pages.rs"]
pub mod service_pages;
