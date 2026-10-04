//! Literal current 182 module 9267 lists consumed by MapMacroKey 4369.
//! Wheel inputs exclude held and toggle playback. Sequence/phased macros are
//! not synthesized by the local editor and have no selectable local entries.
use crate::{features::Choice, i18n};
use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Deserialize)]
struct PlaybackChoice {
    id: String,
    content: String,
}
#[derive(Deserialize)]
struct PlaybackLists {
    normal: Vec<PlaybackChoice>,
    wheel: Vec<PlaybackChoice>,
}

pub(super) fn choices(input: &str) -> Vec<Choice> {
    static LISTS: OnceLock<PlaybackLists> = OnceLock::new();
    let lists = LISTS.get_or_init(|| {
        serde_json::from_str(include_str!("playback_182.json"))
            .expect("audited Macro playback lists")
    });
    let list = if matches!(input, "ScrollUp" | "ScrollDown") {
        &lists.wheel
    } else {
        &lists.normal
    };
    list.iter()
        .map(|choice| Choice::new(&choice.id, i18n::t(&choice.content)))
        .collect()
}
