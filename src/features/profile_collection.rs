//! Profiles 43/Ua collection intents, applied only to the local workspace.
//! New identities reserve the saved baseline; duplication preserves opaque
//! product settings while associations remain in their separate GUID domain.
use crate::model::{Device, Profile};

pub(crate) enum Action {
    Add,
    Duplicate(String),
    Delete(String),
}

pub(crate) fn edit(device: &mut Device, saved: &Device, action: Action) -> Result<String, String> {
    if let Action::Delete(id) = &action {
        if device.profiles.len() <= 1 {
            return Err("至少保留一个本地配置文件".into());
        }
        if !device.profiles.iter().any(|profile| &profile.id == id) {
            return Err("待删除的配置文件已不存在".into());
        }
        device.profiles.retain(|profile| &profile.id != id);
        // Ua supplies the last profile as its successor. The local successor
        // must be a surviving identity, including deletion of that last row.
        let target = device.profiles.last().unwrap().id.clone();
        if &device.active_profile == id {
            device.active_profile = target.clone();
        }
        return Ok(target);
    }
    let id = (1..)
        .map(|index| format!("local-profile-{index}"))
        .find(|id| {
            !device
                .profiles
                .iter()
                .chain(&saved.profiles)
                .any(|profile| profile.id == *id || profile.guid == *id)
        })
        .ok_or("无法分配本地配置标识")?;
    let mut profile = match action {
        Action::Duplicate(source) => {
            let mut profile = device
                .profiles
                .iter()
                .find(|profile| profile.id == source)
                .cloned()
                .ok_or("待复制的配置文件已不存在")?;
            let base = profile
                .name
                .strip_suffix(')')
                .and_then(|name| name.rsplit_once(" ("))
                .filter(|(_, suffix)| {
                    !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
                })
                .map(|(base, _)| base.trim_end())
                .unwrap_or(&profile.name);
            profile.name = (0..)
                .map(|index| {
                    if index == 0 {
                        base.to_owned()
                    } else {
                        format!("{base} ({index})")
                    }
                })
                .find(|name| !device.profiles.iter().any(|profile| profile.name == *name))
                .ok_or("无法生成不重复的配置名称")?;
            if let Some(settings) = &mut profile.settings {
                settings.linked_games.clear();
            }
            profile
        }
        Action::Add => {
            let computer = std::env::var("COMPUTERNAME").unwrap_or_default();
            let base = if computer.is_empty() {
                "Default".to_owned()
            } else {
                format!("{computer}-Default")
            };
            let name = (0..)
                .map(|index| {
                    if index == 0 {
                        base.clone()
                    } else {
                        format!("{base} {index}")
                    }
                })
                .find(|name| !device.profiles.iter().any(|profile| profile.name == *name))
                .ok_or("无法生成不重复的配置名称")?;
            Profile {
                id: String::new(),
                guid: String::new(),
                name,
                dpi_stages: None,
                settings: Some(super::super::settings::ProfileSettings::for_product(
                    device.product_id,
                )),
                source_settings: None,
            }
        }
        Action::Delete(_) => unreachable!(),
    };
    profile.id = id.clone();
    profile.guid = id.clone();
    device.profiles.push(profile);
    // The Profiles dropdown is an assignment target, not hardware activation.
    Ok(id)
}
