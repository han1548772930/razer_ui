//! Current keyboard middleware profile-stack semantics, without cloud identity.
//! The local owner is explicit. A stack commit records a local selection; its
//! runtime switch still needs the product's normal persistence/device path.
use anyhow::{Context as _, ensure};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::{collections::BTreeSet, path::Path};

pub const LOCAL_OWNER: &str = "local";
pub const FOREGROUND_DELAY_MS: u64 = 500;
pub const STOP_RESTORE_DELAY_MS: u64 = 3_000;

pub fn storage_key(product_id: u32) -> String {
    format!("synapse_{product_id}_profiles_stack")
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RunningGame {
    profile_id: String,
    game_id: String,
    timer_tick: u64,
    #[serde(flatten)]
    additional: Map<String, Value>,
}
impl RunningGame {
    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }
    pub fn game_id(&self) -> &str {
        &self.game_id
    }
    pub fn timer_tick(&self) -> u64 {
        self.timer_tick
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileStack {
    active_profile: String,
    #[serde(default)]
    running_games: Vec<RunningGame>,
    #[serde(flatten)]
    additional: Map<String, Value>,
}
impl ProfileStack {
    pub fn new(base_profile: impl Into<String>) -> Self {
        Self {
            active_profile: base_profile.into(),
            running_games: vec![],
            additional: Map::new(),
        }
    }
    pub fn active_profile(&self) -> &str {
        &self.active_profile
    }
    pub fn running_games(&self) -> &[RunningGame] {
        &self.running_games
    }
    /// Only an ordinary UI profile selection changes the base. Runtime switches
    /// leave both the base and the existing running-game entries intact.
    pub fn select_base(&mut self, profile: impl Into<String>) {
        self.active_profile = profile.into();
    }
}

pub fn read(path: &Path, product_id: u32) -> anyhow::Result<Option<ProfileStack>> {
    let Some(root) = crate::source_local::get(path, &storage_key(product_id))? else {
        return Ok(None);
    };
    let root = root
        .as_object()
        .context("Keyboard profile-stack store is not an object")?;
    root.get(LOCAL_OWNER)
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(Into::into)
}

/// Preserve other owners/unknown fields; never import an account identity or
/// silently replace a malformed local document with a successful empty read.
pub fn save(path: &Path, product_id: u32, stack: &ProfileStack) -> anyhow::Result<()> {
    ensure!(
        !stack.active_profile.is_empty(),
        "Keyboard profile-stack base GUID is empty"
    );
    let stack = serde_json::to_value(stack)?;
    crate::source_local::update(path, &storage_key(product_id), |previous| {
        let mut root = previous.unwrap_or_else(|| Value::Object(Map::new()));
        root.as_object_mut()
            .context("Keyboard profile-stack store is not an object")?
            .insert(LOCAL_OWNER.into(), stack.clone());
        Ok(Some(root))
    })?;
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Association {
    game_id: String,
    profile_id: String,
}
impl Association {
    pub fn new(game: impl Into<String>, profile: impl Into<String>) -> Self {
        Self {
            game_id: game.into(),
            profile_id: profile.into(),
        }
    }
    pub fn game_id(&self) -> &str {
        &self.game_id
    }
    pub fn profile_id(&self) -> &str {
        &self.profile_id
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunningGameDetail {
    game_id: String,
    executables: Vec<String>,
}
impl RunningGameDetail {
    pub fn new(game: impl Into<String>, executables: Vec<String>) -> Self {
        Self {
            game_id: game.into(),
            executables,
        }
    }
}

/// Each effect preserves the source ordering: optimistic runtime UI selection,
/// local stack persistence, then the ordinary system profile-switch task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    RuntimeProfile(String),
    SaveStack(ProfileStack),
    SwitchProfile(String),
    GameActive(bool),
}

#[derive(Clone, Debug)]
struct StopRestore {
    due: u64,
    stack: ProfileStack,
    captured_profile: Option<String>,
    associations: Vec<Association>,
}

/// One mounted middleware owner's timers and active game. This reducer uses a
/// supplied clock so ordering, stale captures and debounce are pure-testable.
/// Normal stops do not cancel earlier 3-second callbacks or the start debounce:
/// no such cancellation exists in the current 9640/97161 source owners.
pub struct Runtime {
    stack: ProfileStack,
    active_game: Option<String>,
    associations: Vec<Association>,
    available_games: BTreeSet<String>,
    running_games: Vec<String>,
    pending_start: Option<(u64, String)>,
    pending_stops: Vec<StopRestore>,
}
impl Runtime {
    pub fn new(stack: ProfileStack) -> Self {
        Self {
            stack,
            active_game: None,
            associations: vec![],
            available_games: BTreeSet::new(),
            running_games: vec![],
            pending_start: None,
            pending_stops: vec![],
        }
    }
    pub fn stack(&self) -> &ProfileStack {
        &self.stack
    }
    pub fn active_game(&self) -> Option<&str> {
        self.active_game.as_deref()
    }
    pub fn configure(&mut self, associations: Vec<Association>, available_games: BTreeSet<String>) {
        self.associations = associations;
        self.available_games = available_games;
    }
    pub fn observe_running(&mut self, games: Vec<String>) {
        self.running_games = games;
    }
    pub fn select_base(&mut self, profile: impl Into<String>) -> Vec<Effect> {
        self.stack.select_base(profile);
        vec![Effect::SaveStack(self.stack.clone())]
    }
    pub fn initialize(&mut self, tick: u64) -> Vec<Effect> {
        let mut effects = vec![Effect::RuntimeProfile(self.stack.active_profile.clone())];
        let (stack, last) = rebuild(&self.stack, &self.running_games, &self.associations, tick);
        self.stack = stack;
        if let Some(profile) = last {
            effects.push(Effect::RuntimeProfile(profile.clone()));
            effects.push(Effect::SwitchProfile(profile));
        }
        effects.push(Effect::SaveStack(self.stack.clone()));
        effects
    }
    pub fn game_started(
        &mut self,
        game: &str,
        current_profile: Option<&str>,
        tick: u64,
    ) -> Vec<Effect> {
        // Source sets the active ID before its injected-localgames membership
        // gate, including a game which has no profile association.
        self.active_game = Some(game.into());
        if !self.available_games.contains(game) {
            return vec![];
        }
        let mut effects = vec![Effect::GameActive(true)];
        let Some(link) = self
            .associations
            .iter()
            .find(|link| link.game_id == game)
            .filter(|_| current_profile.is_some_and(|profile| !profile.is_empty()))
        else {
            return effects;
        };
        if let Some(entry) = self
            .stack
            .running_games
            .iter_mut()
            .find(|entry| entry.game_id == game)
        {
            // Relinking a running game does not rewrite the already captured
            // profileId. The newly requested switch still uses the current link.
            entry.timer_tick = tick;
        } else {
            self.stack.running_games.push(RunningGame {
                game_id: game.into(),
                profile_id: link.profile_id.clone(),
                timer_tick: tick,
                additional: Map::new(),
            });
        }
        effects.push(Effect::RuntimeProfile(link.profile_id.clone()));
        effects.push(Effect::SaveStack(self.stack.clone()));
        effects.push(Effect::SwitchProfile(link.profile_id.clone()));
        effects
    }
    pub fn game_stopped(
        &mut self,
        game: &str,
        window_change: bool,
        current_profile: Option<&str>,
        tick: u64,
    ) -> Vec<Effect> {
        self.active_game = None;
        let mut effects = if window_change {
            vec![]
        } else {
            vec![Effect::GameActive(false)]
        };
        if !self.associations.iter().any(|link| link.game_id == game) {
            return effects;
        }
        effects.push(Effect::SwitchProfile(self.stack.active_profile.clone()));
        if !window_change {
            self.pending_stops.push(StopRestore {
                due: tick.saturating_add(STOP_RESTORE_DELAY_MS),
                stack: self.stack.clone(),
                captured_profile: current_profile.map(str::to_owned),
                associations: self.associations.clone(),
            });
        }
        effects
    }
    pub fn foreground(
        &mut self,
        executable: &str,
        details: &[RunningGameDetail],
        current_profile: Option<&str>,
        tick: u64,
    ) -> Vec<Effect> {
        if details.is_empty() {
            return vec![];
        }
        let mut matched = false;
        for detail in details {
            // The first item, and exact case, are intentional current-source
            // behavior. Matching path/any alias would change the UI contract.
            if detail
                .executables
                .first()
                .is_some_and(|first| first == executable)
            {
                matched = true;
                self.pending_start = Some((
                    tick.saturating_add(FOREGROUND_DELAY_MS),
                    detail.game_id.clone(),
                ));
            }
        }
        if !matched && let Some(active) = self.active_game.clone() {
            return self.game_stopped(&active, true, current_profile, tick);
        }
        vec![]
    }
    pub fn tick(&mut self, tick: u64, current_profile: Option<&str>) -> Vec<Effect> {
        let mut effects = vec![];
        if self
            .pending_start
            .as_ref()
            .is_some_and(|(due, _)| *due <= tick)
        {
            let (_, game) = self.pending_start.take().expect("due start");
            effects.extend(self.game_started(&game, current_profile, tick));
        }
        let mut pending = vec![];
        for restore in std::mem::take(&mut self.pending_stops) {
            if restore.due > tick {
                pending.push(restore);
                continue;
            }
            let (stack, last) = if restore
                .captured_profile
                .as_deref()
                .is_some_and(|profile| profile != restore.stack.active_profile)
            {
                rebuild(
                    &restore.stack,
                    &self.running_games,
                    &restore.associations,
                    tick,
                )
            } else {
                let mut stack = restore.stack;
                stack.running_games.clear();
                (stack, None)
            };
            self.stack = stack;
            if let Some(profile) = last {
                effects.push(Effect::RuntimeProfile(profile.clone()));
                effects.push(Effect::SwitchProfile(profile));
            }
            effects.push(Effect::SaveStack(self.stack.clone()));
        }
        self.pending_stops = pending;
        effects
    }
}

fn rebuild(
    base: &ProfileStack,
    running: &[String],
    links: &[Association],
    tick: u64,
) -> (ProfileStack, Option<String>) {
    let mut stack = base.clone();
    stack.running_games.clear();
    let mut last = None;
    for game in running {
        if let Some(link) = links.iter().find(|link| link.game_id == *game) {
            stack.running_games.push(RunningGame {
                game_id: game.clone(),
                profile_id: link.profile_id.clone(),
                timer_tick: tick,
                additional: Map::new(),
            });
            last = Some(link.profile_id.clone());
        }
    }
    (stack, last)
}

#[cfg(test)]
#[path = "linked_profiles_tests.rs"]
mod tests;
