//! Per-device flag-3 cursors (89968/8f0e4/8f5e0). Each trigger advances to
//! one source-defined output boundary. I/O remains with the device owner.
use super::{Action, Definition, Event, Library};
use anyhow::{Context, Result, ensure};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Poll {
    Progress,
    Actions(Vec<Action>),
    Complete,
}

struct Loop {
    start: usize,
    remaining: u32,
}
struct Cursor {
    definition: Arc<Definition>,
    index: usize,
    loops: Vec<Loop>,
    children: BTreeMap<String, Cursor>,
    entered: bool,
}
impl Cursor {
    fn new(definition: Arc<Definition>) -> Self {
        Self {
            definition,
            index: 0,
            loops: Vec::new(),
            children: BTreeMap::new(),
            entered: false,
        }
    }
    fn begin(&mut self) {
        self.entered = false;
        for child in self.children.values_mut() {
            child.begin();
        }
    }
}
struct Active {
    guid: String,
    initialized: bool,
    wrapped: bool,
}

/// Owned by one device/connection. Replacing the connection drops every cursor;
/// cancelling a trigger preserves source cursors without claiming any output.
#[derive(Default)]
pub struct Sequences {
    cursors: BTreeMap<String, Cursor>,
    active: Option<Active>,
}

impl Sequences {
    pub fn begin(&mut self, guid: &str) -> Result<()> {
        ensure!(
            self.active.is_none(),
            "A sequence trigger is already active"
        );
        self.active = Some(Active {
            guid: guid.into(),
            initialized: false,
            wrapped: false,
        });
        Ok(())
    }
    pub fn cancel(&mut self) {
        self.active = None;
    }
    pub fn clear(&mut self) {
        self.active = None;
        self.cursors.clear();
    }
    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }
    pub fn poll(&mut self, library: &Library) -> Result<Poll> {
        let result = self.advance(library);
        if result.is_err() || matches!(result, Ok(Poll::Actions(_) | Poll::Complete)) {
            self.active = None;
        }
        result
    }
    fn advance(&mut self, library: &Library) -> Result<Poll> {
        let Some(active) = self.active.as_mut() else {
            return Ok(Poll::Complete);
        };
        if !active.initialized {
            let definition = definition(library, &active.guid)?;
            let cursor = refresh(&mut self.cursors, &active.guid, definition);
            cursor.begin();
            if cursor.index == cursor.definition.events.len() {
                cursor.index = 0;
            }
            active.initialized = true;
        }
        let cursor = self
            .cursors
            .get_mut(&active.guid)
            .expect("Initialized sequence cursor exists");
        match advance_cursor(cursor, library, 0)? {
            Advance::Progress => Ok(Poll::Progress),
            Advance::Actions(actions) => Ok(Poll::Actions(actions)),
            Advance::End { .. } if !active.wrapped => {
                active.wrapped = true;
                cursor.index = 0;
                cursor.begin();
                Ok(Poll::Progress)
            }
            Advance::End { .. } => Ok(Poll::Complete),
        }
    }
}

fn definition(library: &Library, guid: &str) -> Result<Arc<Definition>> {
    let definition = library
        .get(guid)
        .with_context(|| format!("Cannot find sequence macro {guid}"))?;
    ensure!(
        !definition.events.is_empty(),
        "Sequence macro {guid} has no events"
    );
    Ok(definition)
}

fn refresh<'a>(
    cursors: &'a mut BTreeMap<String, Cursor>,
    guid: &str,
    definition: Arc<Definition>,
) -> &'a mut Cursor {
    let cursor = cursors
        .entry(guid.into())
        .or_insert_with(|| Cursor::new(definition.clone()));
    if cursor.definition.hash != definition.hash {
        *cursor = Cursor::new(definition);
    }
    cursor
}

enum Advance {
    Progress,
    Actions(Vec<Action>),
    End { remove: bool },
}

fn advance_cursor(cursor: &mut Cursor, library: &Library, depth: usize) -> Result<Advance> {
    // Bound host stack/resource use independently of source completion. This is
    // an explicit execution failure, never a successful skipped nested macro.
    ensure!(
        depth < 100,
        "Sequence macro recursion exceeds executor capacity"
    );
    if cursor.index == cursor.definition.events.len() {
        return Ok(Advance::End {
            remove: cursor.entered,
        });
    }
    cursor.entered = true;
    let event = cursor.definition.events[cursor.index]
        .clone()
        .map_err(anyhow::Error::msg)?;
    let actions = match event {
        Event::Keyboard(mut key) if key.flag & 1 == 0 => {
            let make = Action::Keyboard(key.clone());
            key.flag |= 1;
            vec![make, Action::Keyboard(key)]
        }
        Event::Mouse(mouse) => {
            let input = mouse.input();
            if input.is_pressed() {
                let released = input.released();
                let release = super::Mouse {
                    id: released.id,
                    data: released.data,
                    ..mouse.clone()
                };
                vec![Action::Mouse(mouse), Action::Mouse(release)]
            } else if input.is_released() {
                Vec::new()
            } else {
                vec![Action::Mouse(mouse)]
            }
        }
        Event::Clipboard(text) => vec![Action::Clipboard(text)],
        Event::Launch(launch) => vec![Action::Launch(launch)],
        Event::LoopStart { repeat } => {
            cursor.loops.push(Loop {
                start: cursor.index,
                remaining: repeat,
            });
            Vec::new()
        }
        Event::LoopEnd => {
            let frame = cursor
                .loops
                .last_mut()
                .context("Sequence loop end has no loop start")?;
            frame.remaining -= 1;
            if frame.remaining == 0 {
                cursor.loops.pop();
            } else {
                cursor.index = frame.start;
            }
            Vec::new()
        }
        Event::Macro { guid, .. } => {
            let child = refresh(&mut cursor.children, &guid, definition(library, &guid)?);
            return match advance_cursor(child, library, depth + 1)? {
                Advance::End { remove } => {
                    if remove {
                        cursor.children.remove(&guid);
                    }
                    cursor.index += 1;
                    Ok(Advance::Progress)
                }
                boundary => Ok(boundary),
            };
        }
        // 8f0e4 does not dispatch these event kinds in sequence mode.
        Event::Keyboard(_)
        | Event::MouseMove { .. }
        | Event::Delay { .. }
        | Event::RandomDelay { .. }
        | Event::Audio(_) => Vec::new(),
    };
    cursor.index += 1;
    if actions.is_empty() {
        Ok(Advance::Progress)
    } else {
        Ok(Advance::Actions(actions))
    }
}

#[cfg(test)]
#[path = "sequence/tests.rs"]
mod tests;
