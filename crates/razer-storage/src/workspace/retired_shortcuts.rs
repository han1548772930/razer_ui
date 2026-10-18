//! Retained legacy online actions never enter page or runtime shortcut lists.
use razer_model::shortcuts::{Shortcut, ShortcutOutput};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RetiredShortcuts(Vec<Shortcut>);

impl RetiredShortcuts {
    pub(super) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn excluded(item: &Shortcut) -> bool {
        matches!(&item.output, ShortcutOutput::Windows { action } if action == "MSCopilot")
    }

    pub(super) fn migrate(&mut self, active: &mut Vec<Shortcut>) -> anyhow::Result<()> {
        let mut local = Vec::with_capacity(active.len());
        for item in active.drain(..) {
            if Self::excluded(&item) {
                self.0.push(item);
            } else {
                local.push(item);
            }
        }
        *active = local;
        self.validate(active)
    }

    pub(super) fn validate(&self, active: &[Shortcut]) -> anyhow::Result<()> {
        for (index, item) in self.0.iter().enumerate() {
            anyhow::ensure!(Self::excluded(item), "旧联网快捷键归档包含本地操作。");
            anyhow::ensure!(
                !item.id.is_empty()
                    && !self.0[..index].iter().any(|other| other.id == item.id)
                    && !active.iter().any(|other| other.id == item.id),
                "旧联网快捷键身份无效或重复。"
            );
            if item.input.is_empty() {
                anyhow::ensure!(
                    item.modifiers.is_empty(),
                    "尚未录制按键的快捷键不能包含辅助键。"
                );
            } else {
                item.validate_input().map_err(anyhow::Error::msg)?;
            }
        }
        Ok(())
    }
}
