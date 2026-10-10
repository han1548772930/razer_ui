//! DLL-free endpoint volume entry points backed by current simple_service bytes.
use razer_device::simple_audio_volume::{AudioVolumeReadResult, AudioVolumeWriteResult};

#[cfg(windows)]
#[path = "runtime/windows/audio_volume.rs"]
mod windows;

pub fn read(device_id: &str) -> anyhow::Result<AudioVolumeReadResult> {
    #[cfg(windows)]
    return windows::read(device_id);
    #[cfg(not(windows))]
    {
        let _ = device_id;
        anyhow::bail!("simple_service Windows audio volume is unsupported on this platform")
    }
}

pub fn write(device_id: &str, mute: bool, volume: u8) -> anyhow::Result<AudioVolumeWriteResult> {
    #[cfg(windows)]
    return windows::write(device_id, mute, volume);
    #[cfg(not(windows))]
    {
        let _ = (device_id, mute, volume);
        anyhow::bail!("simple_service Windows audio volume is unsupported on this platform")
    }
}
