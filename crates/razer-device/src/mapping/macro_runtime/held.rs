use super::{Action, Mouse};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Identity {
    Keyboard {
        scancode: u16,
        flag: u16,
        unit: Option<u16>,
    },
    Mouse {
        buttons: u32,
        unit: Option<u16>,
    },
}

#[derive(Default)]
pub(super) struct HeldOutputs {
    outputs: BTreeMap<Identity, Action>,
}

impl HeldOutputs {
    pub(super) fn pop_release(&mut self) -> Option<Action> {
        self.outputs.pop_first().map(|(_, action)| action)
    }
    pub(super) fn has_middle_make(&self, unit: Option<u16>) -> bool {
        self.outputs
            .contains_key(&Identity::Mouse { buttons: 16, unit })
    }
    pub(super) fn track(&mut self, action: &Action) {
        let (identity, released, release) = match action {
            Action::Keyboard(key) => {
                let identity = Identity::Keyboard {
                    scancode: key.scancode,
                    flag: key.flag & !1,
                    unit: key.unit,
                };
                let mut release = key.clone();
                release.flag |= 1;
                (identity, key.flag & 1 != 0, Action::Keyboard(release))
            }
            Action::Mouse(mouse) => {
                let input = mouse.input();
                if !input.is_pressed() && !input.is_released() {
                    return;
                }
                let identity = Identity::Mouse {
                    buttons: input.pressed().packed_buttons(),
                    unit: mouse.unit,
                };
                let released = input.released();
                let release = Action::Mouse(Mouse {
                    id: released.id,
                    data: released.data,
                    modifiers: mouse.modifiers,
                    unit: mouse.unit,
                });
                (identity, input.is_released(), release)
            }
            _ => return,
        };
        if released {
            self.outputs.remove(&identity);
        } else {
            self.outputs.entry(identity).or_insert(release);
        }
    }
}
