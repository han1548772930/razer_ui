//! Capability-oriented feature modules.
//!
//! Each module owns one product capability's page composition and commands.
//! The domain data model remains in `crate::domain` and is re-exported here
//! so existing feature code has one stable capability-facing import path.

pub use crate::domain::*;

pub mod audio;
pub mod calibration;
pub mod customize;
pub mod dashboard;
pub mod display;
pub mod engines;
pub mod enhancement;
pub mod eq;
pub mod haptics;
pub mod keyboard;
pub mod lighting;
pub mod macros;
pub mod mic;
pub mod mixer;
pub mod oled;
pub mod pairing;
pub mod performance;
pub mod power;
pub mod scrolling;
pub mod setting;
pub mod sound;
