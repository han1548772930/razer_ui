// Prepared from current 1392/1442/3942 video-react font outlines.
pub(super) fn audio_demo_controls_load(path: &str) -> Option<&'static [u8]> {
    match path {
        "synapse/audio-demo-control-play.svg" => Some(include_bytes!("audio-demo-control-play.svg")),
        "synapse/audio-demo-control-fullscreen.svg" => Some(include_bytes!("audio-demo-control-fullscreen.svg")),
        _ => None,
    }
}
pub(super) fn audio_demo_controls_list(path: &str) -> Vec<gpui_kit::SharedString> {
    if path == "synapse" || path == "synapse/" || path.is_empty() {
        vec!["synapse/audio-demo-control-play.svg".into(), "synapse/audio-demo-control-fullscreen.svg".into()]
    } else {
        Vec::new()
    }
}
