//! Current mapping-engine input selection and retained make/break ownership.
//!
//! Native RVAs: 0xe3f1e merges defaults and the active profile; 0x66d10
//! selects digital inputs; 0x67ff2 binds the release at press time; 0x677ba
//! tracks held identities; 0x7961a releases them during shutdown.
use anyhow::{Context, Result, bail, ensure};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[path = "mapping_native_lookup/input.rs"]
mod input;
#[path = "mapping_native_lookup/serialization.rs"]
mod serialization;
pub use input::{Input, InputIdentity, InputKind, Phase, parse_input};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Family {
    Gaming,
    Otfs,
    DynamicKey,
    SnapTap,
    #[default]
    Normal,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selectors {
    pub hypershift: bool,
    pub gaming: bool,
    pub otfs: bool,
    pub dynamic_key: Option<u32>,
    pub snap_tap: Option<u32>,
    pub snap_tap_prioritize: bool,
}

impl Selectors {
    pub fn family(&self) -> Family {
        if self.gaming {
            Family::Gaming
        } else if self.otfs {
            Family::Otfs
        } else if self.dynamic_key.is_some() {
            Family::DynamicKey
        } else if self.snap_tap.is_some() {
            Family::SnapTap
        } else {
            Family::Normal
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompiledMapping<O> {
    pub input: Input,
    pub selectors: Selectors,
    pub output: O,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct Key {
    input: InputIdentity,
    hypershift: bool,
    family: Family,
    modifiers: Option<u32>,
}

impl Key {
    fn new(input: &Input, hypershift: bool, family: Family, global: bool) -> Self {
        Self {
            input: input.lookup_identity(),
            hypershift,
            family,
            modifiers: global.then_some(input.lookup_modifiers()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Tables<O> {
    defaults: BTreeMap<Key, CompiledMapping<O>>,
    merged: BTreeMap<Key, CompiledMapping<O>>,
    globals: BTreeMap<Key, CompiledMapping<O>>,
}

impl<O: Clone> Tables<O> {
    pub fn new(
        defaults: Vec<CompiledMapping<O>>,
        active: Vec<CompiledMapping<O>>,
        globals: Vec<CompiledMapping<O>>,
    ) -> Result<Self> {
        let defaults = index(defaults, false)?;
        let mut merged = defaults.clone();
        merged.extend(index(active, false)?);
        let globals = index(globals, true)?;
        // 0xe3f1e rejects incomplete or repeated dynamic-key pairs.
        let mut pairs: BTreeMap<InputIdentity, (usize, usize)> = BTreeMap::new();
        for mapping in merged
            .values()
            .filter(|m| m.selectors.family() == Family::DynamicKey)
        {
            let pair = pairs.entry(mapping.input.identity()).or_default();
            match mapping.input.phase() {
                Phase::Press => pair.0 += 1,
                Phase::Release => pair.1 += 1,
                Phase::Instant => bail!("Dynamic-key input has no press/release state"),
            }
        }
        ensure!(
            pairs.values().all(|pair| *pair == (1, 1)),
            "Missing or repeated dynamic-key input pair"
        );
        Ok(Self {
            defaults,
            merged,
            globals,
        })
    }

    /// Returns the actual compiled inputs, including global and disabled-mode
    /// defaults. The transport translates these into its verified hook ABI.
    pub fn hook_inputs(&self) -> Vec<Input> {
        self.defaults
            .values()
            .chain(self.merged.values())
            .chain(self.globals.values())
            .flat_map(|m| std::iter::once(&m.input).chain(m.input.interrupts.iter()))
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    fn find(
        &self,
        input: &Input,
        modes: Modes,
        family: Family,
        global: bool,
    ) -> Option<&CompiledMapping<O>> {
        let table = if global {
            &self.globals
        } else if modes.mapping_enabled {
            &self.merged
        } else {
            &self.defaults
        };
        table.get(&Key::new(input, modes.hypershift, family, global))
    }

    fn select(&self, input: &Input, modes: Modes, break_of: Option<&Resolved<O>>) -> Resolved<O> {
        let global_only = break_of.is_some_and(|m| m.global_shortcut);
        let gaming = break_of.map_or(modes.gaming, |m| m.selectors.gaming);
        let otfs = break_of.map_or(modes.otfs, |m| m.selectors.otfs);
        let mut found = None;
        let mut global = global_only;
        if global_only {
            found = self.find(input, modes, Family::Normal, true);
        } else {
            if gaming {
                found = self.find(input, modes, Family::Gaming, false);
            }
            if modes.mapping_enabled {
                if found.is_none() && otfs {
                    found = self.find(input, modes, Family::Otfs, false);
                }
                for family in [Family::DynamicKey, Family::SnapTap] {
                    if found.is_none() {
                        found = self.find(input, modes, family, false);
                    }
                }
                if found.is_none() && break_of.is_none() && modes.global_shortcuts_enabled {
                    found = self.find(input, modes, Family::Normal, true);
                    global = found.is_some();
                }
            }
            if found.is_none() {
                found = self.find(input, modes, Family::Normal, false);
            }
        }
        match found {
            Some(mapping) => Resolved {
                input: mapping.input.clone(),
                output: ResolvedOutput::Mapped(mapping.output.clone()),
                selectors: mapping.selectors.clone(),
                mapping_enabled: modes.mapping_enabled,
                global_shortcut: global,
            },
            None => Resolved {
                input: input.clone(),
                output: ResolvedOutput::Passthrough(input.clone()),
                selectors: Selectors {
                    hypershift: modes.hypershift,
                    ..Default::default()
                },
                mapping_enabled: modes.mapping_enabled,
                global_shortcut: global,
            },
        }
    }
}

fn index<O>(
    mappings: Vec<CompiledMapping<O>>,
    global: bool,
) -> Result<BTreeMap<Key, CompiledMapping<O>>> {
    let mut indexed = BTreeMap::new();
    for mapping in mappings {
        let family = if global {
            Family::Normal
        } else {
            mapping.selectors.family()
        };
        let key = Key::new(&mapping.input, mapping.selectors.hypershift, family, global);
        ensure!(
            !indexed.contains_key(&key),
            "Duplicate compiled input identity"
        );
        indexed.insert(key, mapping);
    }
    Ok(indexed)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Modes {
    pub mapping_enabled: bool,
    pub hypershift: bool,
    pub gaming: bool,
    pub otfs: bool,
    pub global_shortcuts_enabled: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResolvedOutput<O> {
    Mapped(O),
    Passthrough(Input),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved<O> {
    pub input: Input,
    pub output: ResolvedOutput<O>,
    pub selectors: Selectors,
    pub mapping_enabled: bool,
    pub global_shortcut: bool,
}

impl<O> Resolved<O> {
    pub fn is_passthrough(&self) -> bool {
        matches!(self.output, ResolvedOutput::Passthrough(_))
    }
}

#[derive(Clone, Debug)]
struct Held<O> {
    make: Resolved<O>,
    release: Resolved<O>,
}

#[derive(Clone, Debug)]
pub struct HeldMapping<O> {
    pub make: Resolved<O>,
    pub release: Resolved<O>,
}

/// One instance belongs to one connected device actor. Connection identity and
/// task cancellation are enforced by that actor before calling this state.
pub struct Lookup<O> {
    tables: Tables<O>,
    held: BTreeMap<InputIdentity, Held<O>>,
    suppressed: BTreeSet<InputIdentity>,
}

impl<O: Clone> Lookup<O> {
    pub fn new(tables: Tables<O>) -> Self {
        Self {
            tables,
            held: BTreeMap::new(),
            suppressed: BTreeSet::new(),
        }
    }
    /// Native QueryRunningMacrosAndKeysPressed copies the low byte of held size.
    pub fn held_count(&self) -> u8 { self.held.len() as u8 }
    pub fn tables(&self) -> &Tables<O> {
        &self.tables
    }
    /// A snapshot of the press-time pair used by composite mappings (79522).
    /// The lookup remains the sole owner of physical held-input state.
    pub fn held_by_stateless(&self, identity: &str) -> Option<HeldMapping<O>> {
        self.held
            .values()
            .find(|held| held.make.input.stateless_id() == identity)
            .map(|held| HeldMapping {
                make: held.make.clone(),
                release: held.release.clone(),
            })
    }
    pub fn replace_tables(&mut self, tables: Tables<O>) {
        self.tables = tables;
    }

    pub fn resolve(&mut self, input: Input, modes: Modes) -> Option<Resolved<O>> {
        let identity = input.identity();
        match input.phase() {
            Phase::Release => {
                let held = self.held.remove(&identity);
                if self.suppressed.remove(&identity) {
                    return None;
                }
                held.map(|held| held.release)
            }
            Phase::Press => {
                if self.suppressed.contains(&identity) || self.held.contains_key(&identity) {
                    return None;
                }
                let make = self.tables.select(&input, modes, None);
                let release_input = make.input.released();
                let release_modes = Modes {
                    hypershift: make.selectors.hypershift,
                    ..modes
                };
                let release = self
                    .tables
                    .select(&release_input, release_modes, Some(&make));
                self.held.insert(
                    identity,
                    Held {
                        make: make.clone(),
                        release,
                    },
                );
                Some(make)
            }
            Phase::Instant => Some(self.tables.select(&input, modes, None)),
        }
    }

    /// 0x6a5e0 releases held mappings whose execution mode changes.
    pub fn mapping_changed(&mut self, enabled: bool) -> Vec<Resolved<O>> {
        self.release_matching(|make| make.mapping_enabled != enabled, false)
    }

    /// 0x6ae30 releases mapped held keys and suppresses them until physical
    /// release; it does not treat passthrough as a configured mapping.
    pub fn hypershift_disabled(&mut self) -> Vec<Resolved<O>> {
        self.release_matching(|make| !make.is_passthrough(), true)
    }

    fn release_matching(
        &mut self,
        predicate: impl Fn(&Resolved<O>) -> bool,
        suppress: bool,
    ) -> Vec<Resolved<O>> {
        let keys = self
            .held
            .iter()
            .filter(|(_, held)| predicate(&held.make))
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();
        keys.into_iter()
            .map(|key| {
                if suppress {
                    self.suppressed.insert(key.clone());
                } else {
                    self.suppressed.remove(&key);
                }
                self.held.remove(&key).unwrap().release
            })
            .collect()
    }

    /// The actor must execute these releases and await transport completion
    /// before closing its driver. Returning them does not prove delivery.
    pub fn drain_releases(&mut self) -> Vec<Resolved<O>> {
        self.suppressed.clear();
        std::mem::take(&mut self.held)
            .into_values()
            .map(|held| held.release)
            .collect()
    }
}

pub fn parse_mappings<O>(
    engine: &Value,
    mut parse_output: impl FnMut(&Value) -> Result<O>,
) -> Result<Vec<CompiledMapping<O>>> {
    engine
        .get("mappings")
        .and_then(Value::as_array)
        .context("Missing compiled mappings array")?
        .iter()
        .map(|mapping| {
            let raw = mapping
                .get("input")
                .context("Missing compiled mapping input")?;
            let input = parse_input(raw)?;
            let dynamic_key = input::optional_int(raw, "dkpId")?.map(|v| v as u32);
            let selectors = Selectors {
                hypershift: input::optional_bool(raw, "hypershift")?,
                gaming: input::optional_bool(raw, "gamemode")?,
                otfs: input::optional_bool(raw, "otfs")?,
                dynamic_key,
                snap_tap: if dynamic_key.is_none() {
                    input::optional_int(raw, "snaptapId")?.map(|v| v as u32)
                } else {
                    None
                },
                snap_tap_prioritize: input::optional_bool(raw, "snaptapPrioritize")?,
            };
            let output = parse_output(
                mapping
                    .get("output")
                    .context("Missing compiled mapping output")?,
            )?;
            Ok(CompiledMapping {
                input,
                selectors,
                output,
            })
        })
        .collect()
}

pub fn from_device_document<O: Clone>(
    document: &Value,
    global_engine: Option<&Value>,
    parse_output: impl FnMut(&Value) -> Result<O>,
) -> Result<Tables<O>> {
    from_device_document_filtered(document, global_engine, parse_output, |_| Ok(true))
}

pub(crate) fn from_device_document_filtered<O: Clone>(
    document: &Value,
    global_engine: Option<&Value>,
    mut parse_output: impl FnMut(&Value) -> Result<O>,
    keep_mapping: impl Fn(&CompiledMapping<O>) -> Result<bool>,
) -> Result<Tables<O>> {
    let active = document
        .get("activeProfile")
        .and_then(Value::as_str)
        .context("Missing active mapping profile")?;
    let profiles = document
        .get("profiles")
        .and_then(Value::as_array)
        .context("Missing mapping profiles")?;
    let matching = profiles
        .iter()
        .filter(|p| p.get("guid").and_then(Value::as_str) == Some(active))
        .collect::<Vec<_>>();
    ensure!(
        matching.len() == 1,
        "Active mapping profile is missing or ambiguous"
    );
    let defaults = document
        .get("defaultMappings")
        .and_then(|v| v.get("appEngine"))
        .context("Missing default compiled engine")?;
    let profile = matching[0]
        .get("appEngine")
        .context("Missing active compiled engine")?;
    let filter = |rows: Vec<CompiledMapping<O>>| -> Result<Vec<CompiledMapping<O>>> {
        // Validate the publication before dropping source identity mappings.
        let rows = index(rows, false)?;
        rows.into_values()
            .filter_map(|row| match keep_mapping(&row) {
                Ok(true) => Some(Ok(row)),
                Ok(false) => None,
                Err(error) => Some(Err(error)),
            })
            .collect()
    };
    let defaults = filter(parse_mappings(defaults, &mut parse_output)?)?;
    let profile = filter(parse_mappings(profile, &mut parse_output)?)?;
    let global = global_engine
        .map(|v| parse_mappings(v, &mut parse_output))
        .transpose()?
        .unwrap_or_default();
    Tables::new(defaults, profile, global)
}

#[cfg(test)]
#[path = "mapping_native_lookup/tests.rs"]
mod tests;
