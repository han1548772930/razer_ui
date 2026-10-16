// Prepared from all four current video-react EOT font outlines.
pub(super) fn audio_demo_controls_load(path: &str) -> Option<&'static [u8]> {
    match path {
        "synapse/audio-demo-control-play.svg" => Some(include_bytes!("../products/1392/audio-demo-control-play.svg")),
        "synapse/audio-demo-control-pause.svg" => Some(include_bytes!("../products/1392/audio-demo-control-pause.svg")),
        "synapse/audio-demo-control-fullscreen.svg" => Some(include_bytes!("../products/1392/audio-demo-control-fullscreen.svg")),
        "synapse/audio-demo-control-fullscreen-exit.svg" => Some(include_bytes!("../products/1392/audio-demo-control-fullscreen-exit.svg")),
        _ => None,
    }
}
pub(super) fn audio_demo_controls_list(path: &str) -> Vec<gpui_kit::SharedString> {
    if path == "synapse" || path == "synapse/" || path.is_empty() {
        vec!["synapse/audio-demo-control-play.svg".into(), "synapse/audio-demo-control-pause.svg".into(), "synapse/audio-demo-control-fullscreen.svg".into(), "synapse/audio-demo-control-fullscreen-exit.svg".into()]
    } else { Vec::new() }
}
