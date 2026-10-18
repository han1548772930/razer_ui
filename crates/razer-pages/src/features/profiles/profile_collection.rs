//! Profiles 43/Ua collection intents, applied only to the local workspace.
//! New identities reserve the saved baseline; duplication preserves opaque
//! product settings while associations remain in their separate GUID domain.
use razer_model::model::Device;
use razer_model::model::Profile;

pub enum Action {
    Add,
    Duplicate(String),
    Delete {
        id: String,
        successor: Option<String>,
    },
}

pub fn edit(device: &mut Device, saved: &Device, action: Action) -> Result<String, String> {
    if let Action::Delete { id, successor } = &action {
        if device.profiles.len() <= 1 {
            return Err("至少保留一个本地配置文件".into());
        }
        if !device.profiles.iter().any(|profile| &profile.id == id) {
            return Err("待删除的配置文件已不存在".into());
        }
        if successor.as_ref().is_some_and(|successor| {
            successor == id
                || !device
                    .profiles
                    .iter()
                    .any(|profile| &profile.id == successor)
        }) {
            return Err("删除后的目标配置文件已不存在".into());
        }
        let removed_guid = device
            .profiles
            .iter()
            .find(|profile| &profile.id == id)
            .map(|profile| profile.guid.clone())
            .unwrap();
        device.profiles.retain(|profile| &profile.id != id);
        if device.product_id == 756 {
            // Current50574 qf uses the requested survivor when valid, otherwise
            // the first stored profile. UI deletion supplies its first survivor.
            for profile in &mut device.profiles {
                if let Some(mappings) = profile
                    .source_settings
                    .as_mut()
                    .and_then(|source| source.get_mut("mappings"))
                    .and_then(serde_json::Value::as_array_mut)
                {
                    mappings.retain(|mapping| {
                        let target = if mapping["mapping"].is_array() {
                            &mapping["mapping"][0]
                        } else {
                            mapping
                        };
                        !(target["outputType"] == "profileNavigationGroup"
                            && target["profileNavigationGroup"]["guid"] == removed_guid
                            && target["profileNavigationGroup"]["profileNavigationAssignment"]
                                == "Specific")
                    });
                }
            }
            let target = successor
                .clone()
                .unwrap_or_else(|| device.profiles[0].id.clone());
            device.active_profile = target.clone();
            return Ok(target);
        }
        // Ua selects the last survivor. Product menus can supply the successor
        // from their current-source ON_DELETE_PROFILE payload instead.
        let target = successor
            .clone()
            .unwrap_or_else(|| device.profiles.last().unwrap().id.clone());
        if successor.is_some() || &device.active_profile == id {
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
        Action::Delete { .. } => unreachable!(),
    };
    profile.id = id.clone();
    profile.guid = id.clone();
    device.profiles.push(profile);
    // The Profiles dropdown is an assignment target, not hardware activation.
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn device() -> Device {
        let mut device = crate::test_devices::keyboard();
        device.product_id = razer_device::hanbo_cooler::PRODUCT_ID;
        let template = device.profiles[0].clone();
        device.profiles = ["first", "selected", "last"]
            .into_iter()
            .map(|id| Profile {
                id: id.into(),
                guid: format!("guid-{id}"),
                name: id.into(),
                ..template.clone()
            })
            .collect();
        device.active_profile = "selected".into();
        device
    }

    #[::core::prelude::v1::test]
    fn product_delete_selects_the_source_payload_successor() {
        let mut device = device();
        let saved = device.clone();
        let target = edit(
            &mut device,
            &saved,
            Action::Delete {
                id: "selected".into(),
                successor: Some("first".into()),
            },
        )
        .unwrap();
        assert_eq!(target, "first");
        assert_eq!(device.active_profile, "first");
        assert_eq!(device.profiles.len(), 2);
        assert_eq!(saved.active_profile, "selected");
        assert_eq!(saved.profiles.len(), 3);
    }

    #[::core::prelude::v1::test]
    fn profiles_delete_keeps_its_existing_selection_semantics() {
        let mut device = device();
        let saved = device.clone();
        let target = edit(
            &mut device,
            &saved,
            Action::Delete {
                id: "first".into(),
                successor: None,
            },
        )
        .unwrap();
        assert_eq!(target, "last");
        assert_eq!(device.active_profile, "selected");
        edit(
            &mut device,
            &saved,
            Action::Delete {
                id: "selected".into(),
                successor: None,
            },
        )
        .unwrap();
        assert_eq!(device.active_profile, "last");
    }

    #[::core::prelude::v1::test]
    fn invalid_successor_is_rejected_before_mutating_the_collection() {
        for successor in ["missing", "selected"] {
            let mut device = device();
            let saved = device.clone();
            assert!(
                edit(
                    &mut device,
                    &saved,
                    Action::Delete {
                        id: "selected".into(),
                        successor: Some(successor.into()),
                    },
                )
                .is_err()
            );
            assert_eq!(device.profiles.len(), 3);
            assert_eq!(device.active_profile, "selected");
        }
    }
}
