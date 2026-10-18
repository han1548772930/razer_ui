//! Local device protocols, service adapters and owned background sessions.
//! Remaining vendor implementations and analysis evidence are tracked separately
//! in the unified workflow.
#[path = "audio/audio_notification.rs"]
pub mod audio_notification;
#[path = "audio/audio_router.rs"]
pub mod audio_router;
#[path = "audio/audio_util.rs"]
pub mod audio_util;
#[path = "camera.rs"]
pub mod camera;
#[path = "lighting/chroma_connect.rs"]
pub mod chroma_connect;
#[path = "mouse/dpi_matcher.rs"]
pub mod dpi_matcher;
#[path = "native/help_system_info.rs"]
pub mod help_system_info;
#[path = "mouse/mouse_dynamic.rs"]
pub mod mouse_dynamic;
#[path = "mouse/mouse_rotation.rs"]
pub mod mouse_rotation;
#[path = "mouse/worker.rs"]
mod mouse_worker;
#[path = "runtime/mod.rs"]
pub mod runtime;
#[path = "audio/seiren/mod.rs"]
pub mod seiren_audio;
#[path = "audio/simple_audio.rs"]
pub mod simple_audio;
#[path = "audio/simple_audio_volume.rs"]
pub mod simple_audio_volume;
#[path = "audio/thx.rs"]
pub mod thx;
#[path = "audio/thx_direct.rs"]
pub mod thx_direct;
#[path = "audio/thx_rpc.rs"]
pub mod thx_rpc;


#[path = "accessory/hanbo_cooler.rs"]
pub mod hanbo_cooler;
pub mod mapping;
