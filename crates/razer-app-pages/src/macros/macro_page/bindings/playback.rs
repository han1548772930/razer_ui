//! Literal current 182 module 9267 lists consumed by MapMacroKey 4369.
//! Wheel inputs exclude held and toggle playback. Sequence/phased macros are
//! not synthesized by the local editor and have no selectable local entries.
use razer_i18n as i18n;
use razer_pages::features::Choice;
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

pub fn choices(input: &str) -> Vec<Choice> {
    static LISTS: OnceLock<PlaybackLists> = OnceLock::new();
    let lists = LISTS.get_or_init(|| read_playback_182_catalog());
    let list = if matches!(input, "ScrollUp" | "ScrollDown") {
        &lists.wheel
    } else {
        &lists.normal
    };
    list.iter()
        .map(|choice| Choice::new(&choice.id, i18n::t(&choice.content)))
        .collect()
}

// Generated catalog reader: razer-app-pages/playback_182.
fn read_playback_182_catalog() -> PlaybackLists {
    fn decode_0(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<PlaybackLists> {
        let value = match r.tag()? {
            0 => PlaybackLists {
                normal: r.child(decode_1)?,
                wheel: r.child(decode_1)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_1(
        r: &mut razer_resource_db::NodeReader<'_>,
    ) -> anyhow::Result<std::vec::Vec<PlaybackChoice>> {
        let value = match r.tag()? {
            0 => r.sequence(decode_2)?,
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_2(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<PlaybackChoice> {
        let value = match r.tag()? {
            0 => PlaybackChoice {
                id: r.child(decode_3)?,
                content: r.child(decode_3)?,
            },
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    fn decode_3(r: &mut razer_resource_db::NodeReader<'_>) -> anyhow::Result<String> {
        let value = match r.tag()? {
            0 => r.string()?.into(),
            _ => anyhow::bail!("Invalid typed catalog alternative"),
        };
        r.finish()?;
        Ok(value)
    }
    razer_resource_db::read_document(
        razer_resource_db::catalogs::RAZER_APP_PAGES_PLAYBACK_182,
        0,
        "d228dce5d0b91f8b34ecc4958923f13edbce013a7a2d338c479f2a2be01cd1c4",
        decode_0,
    )
    .expect("Packaged typed catalog must match its domain schema")
}

#[cfg(test)]
mod playback_182_catalog_tests {
    #[test]
    fn packaged_catalog_playback_182_decodes() {
        let _ = super::read_playback_182_catalog();
    }
}
// End generated catalog reader: razer-app-pages/playback_182.
