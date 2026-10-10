// Exact current PID 1352 icon_external_link_sprite.0b292d50.svg bytes.
// GPUI has no browser SVG-fragment view selection. This adapter only adds
// the original selected <view> viewport to the root; symbols/uses stay intact.
const AUDIO_VOLUME_SPRITE: &str = include_str!("audio-volume-external-sprite.svg");
const AUDIO_VOLUME_SPRITE_PATH: &str = "synapse/audio-volume-external-sprite.svg";

pub(super) fn audio_volume_sprite_load(path: &str) -> Option<std::borrow::Cow<'static, [u8]>> {
    if path == AUDIO_VOLUME_SPRITE_PATH {
        return Some(std::borrow::Cow::Borrowed(AUDIO_VOLUME_SPRITE.as_bytes()));
    }
    let view = path
        .strip_prefix(AUDIO_VOLUME_SPRITE_PATH)?
        .strip_prefix('#')?;
    if !matches!(view, "link" | "hover") {
        return None;
    }
    let marker = format!("<view id=\"{view}\" viewBox=\"");
    let start = AUDIO_VOLUME_SPRITE.find(&marker)? + marker.len();
    let tail = &AUDIO_VOLUME_SPRITE[start..];
    let viewport = &tail[..tail.find('"')?];
    let root_end = AUDIO_VOLUME_SPRITE.find('>')?;
    let mut selected = String::with_capacity(AUDIO_VOLUME_SPRITE.len() + 70);
    selected.push_str(&AUDIO_VOLUME_SPRITE[..root_end]);
    selected.push_str(&format!(
        " width=\"20\" height=\"20\" viewBox=\"{viewport}\""
    ));
    selected.push_str(&AUDIO_VOLUME_SPRITE[root_end..]);
    Some(std::borrow::Cow::Owned(selected.into_bytes()))
}

pub(super) fn audio_volume_sprite_list(path: &str) -> Vec<gpui_kit::SharedString> {
    [
        AUDIO_VOLUME_SPRITE_PATH,
        "synapse/audio-volume-external-sprite.svg#link",
        "synapse/audio-volume-external-sprite.svg#hover",
    ]
    .into_iter()
    .filter(|key| key.starts_with(path))
    .map(gpui_kit::SharedString::from)
    .collect()
}
