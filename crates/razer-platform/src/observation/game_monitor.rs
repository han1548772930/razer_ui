//! Local replacement for the current signed-client-only GMS3 observation.
//! Catalog import, Windows process observation and source JSON reduction are
//! separate. No service authentication, process launch, account or download is
//! involved. Shared matching uses the current GameSignatureHelper rules.
use anyhow::{Context as _, Result, ensure};
use razer_storage::game_catalog::{GameSignature, read_game_catalog};
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, SystemTime},
};

pub const STORAGE_KEY: &str = "gms-running-games";

#[derive(Clone, Debug, Default)]
pub struct GameLibrary {
    local: Vec<GameSignature>,
    user: Vec<GameSignature>,
    master: Vec<GameSignature>,
    linked: Vec<GameSignature>,
}

impl GameLibrary {
    pub fn new(
        local: Vec<GameSignature>,
        user: Vec<GameSignature>,
        master: Vec<GameSignature>,
        linked: Vec<GameSignature>,
    ) -> Self {
        Self {
            local,
            user,
            master,
            linked,
        }
    }

    pub fn candidate_names(&self) -> BTreeSet<String> {
        let mut names = BTreeSet::from(["JAVAW".into()]);
        for signature in self
            .local
            .iter()
            .chain(&self.user)
            .chain(&self.master)
            .chain(&self.linked)
        {
            for path in signature
                .default_executables()
                .into_iter()
                .chain(components(signature.alternative_exe()))
                .chain(components(signature.default_folder_exe()))
                .chain(components(signature.default_folder_exe32()))
                .chain(components(signature.running_exe()))
                .chain(components(signature.launch_helper()).filter(|path| {
                    !components(signature.launch_helper_excludables())
                        .any(|excluded| excluded == *path)
                }))
            {
                let name = executable_stem(path).to_uppercase();
                if !name.is_empty() {
                    names.insert(name);
                }
            }
        }
        names
    }

    /// GsnDataManager searches LGL then UGL. App-owned links are exact full
    /// paths; MGL fallback also considers default executable basenames.
    pub fn match_process(&self, executable: &str, is_64_bit_os: bool) -> Option<&GameSignature> {
        self.local
            .iter()
            .chain(&self.user)
            .find(|game| matches_local(game, executable, is_64_bit_os))
            .or_else(|| {
                self.linked
                    .iter()
                    .find(|game| equal_path(game.launch_exe(), executable))
            })
            .or_else(|| {
                // GetRunningGames first finds a possible MGL record, then
                // AddDetectedGame/FindLaunchHelperMatch resolve the installed
                // folder. A shared basename alone is not an installed game.
                let possible = self.master.iter().find(|game| {
                    contains_alternative(game, executable)
                        || matches_default(game, executable)
                        || matches_other(game, executable, is_64_bit_os)
                })?;
                let path = normalized(executable);
                let mut best = None;
                let mut longest = 0;
                for game in &self.master {
                    if !(equal_path(game.launch_exe(), executable)
                        || matches_default(game, executable)
                        || matches_other(game, executable, is_64_bit_os)
                        || contains_alternative(game, executable))
                    {
                        continue;
                    }
                    let folder = if !is_64_bit_os && !game.default_folder_exe32().is_empty() {
                        game.default_folder_exe32()
                    } else {
                        game.default_folder_exe()
                    };
                    for component in components(folder) {
                        for component in [component, game.default_folder_exe32()] {
                            if !component.is_empty()
                                && path.ends_with(&normalized(component))
                                && component.len() > longest
                            {
                                best = Some(game);
                                longest = component.len();
                            }
                        }
                    }
                }
                let resolved = best.or_else(|| {
                    self.master.iter().find(|game| {
                        let folder = if !is_64_bit_os && !game.default_folder32().is_empty() {
                            game.default_folder32()
                        } else {
                            game.default_folder()
                        };
                        let exe = game.default_executables()[0];
                        if folder.is_empty() || exe.is_empty() {
                            return false;
                        }
                        let folder = folder.strip_prefix("%ProgramFiles%").unwrap_or(folder);
                        let separator = if folder.ends_with('\\') { "" } else { "\\" };
                        path.contains(&normalized(&format!("{folder}{separator}{exe}")))
                    })
                })?;
                (resolved.entry_guid() == possible.entry_guid()).then_some(possible)
            })
    }
}

fn components(value: &str) -> impl Iterator<Item = &str> {
    value.split('|').filter(|value| !value.is_empty())
}
fn filename(value: &str) -> &str {
    value.rsplit(['\\', '/']).next().unwrap_or_default()
}
pub(crate) fn executable_stem(value: &str) -> &str {
    let name = filename(value);
    name.rsplit_once('.').map_or(name, |(stem, _)| stem)
}
fn normalized(value: &str) -> String {
    value.replace('/', "\\").to_uppercase()
}
fn equal_path(first: &str, second: &str) -> bool {
    !first.is_empty() && !second.is_empty() && normalized(first) == normalized(second)
}
fn parent(value: &str) -> Option<&str> {
    value.rfind(['\\', '/']).map(|index| &value[..=index])
}
fn combine(directory: &str, component: &str) -> String {
    // Windows Path.Combine returns an absolute second component unchanged.
    if component.starts_with(['\\', '/']) || component.as_bytes().get(1) == Some(&b':') {
        component.into()
    } else {
        format!("{directory}{component}")
    }
}
fn contains_alternative(game: &GameSignature, path: &str) -> bool {
    let Some(directory) = parent(path) else {
        return false;
    };
    let upper = normalized(path);
    components(game.alternative_exe())
        .any(|exe| upper.contains(&normalized(&combine(directory, exe))))
}
fn matches_default(game: &GameSignature, path: &str) -> bool {
    let Some(directory) = parent(path) else {
        return false;
    };
    game.default_executables()
        .iter()
        .any(|exe| !exe.is_empty() && equal_path(&combine(directory, exe), path))
}
fn path_contains(path: &str, component: &str) -> bool {
    let path = normalized(path);
    let component = normalized(component);
    if component.contains('*') {
        // Current DoesPathContainComponents checks all pieces, without imposing
        // order or a filename equality rule on the wildcard branch.
        component.split('*').all(|piece| path.contains(piece))
    } else {
        path.contains(&component) && filename(&path) == filename(&component)
    }
}
fn matches_other(game: &GameSignature, path: &str, is_64_bit_os: bool) -> bool {
    let folder = if !is_64_bit_os && !game.default_folder_exe32().is_empty() {
        game.default_folder_exe32()
    } else {
        game.default_folder_exe()
    };
    components(folder).any(|component| path_contains(path, component))
        || (game.source_ex() != "MIHOYO"
            && components(game.running_exe()).any(|component| path_contains(path, component)))
}
fn matches_local(game: &GameSignature, path: &str, is_64_bit_os: bool) -> bool {
    equal_path(game.launch_exe(), path)
        || contains_alternative(game, path)
        || matches_other(game, path, is_64_bit_os)
        || game
            .overlay_tracking()
            .split(';')
            .any(|component| equal_path(component.trim_matches(['\\', ';', ' ']), path))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunningProcess {
    pid: u32,
    executable: String,
}
impl RunningProcess {
    pub fn new(pid: u32, executable: impl Into<String>) -> Self {
        Self {
            pid,
            executable: executable.into(),
        }
    }
    pub fn pid(&self) -> u32 {
        self.pid
    }
    pub fn executable(&self) -> &str {
        &self.executable
    }
}

#[derive(Clone, Debug, Default)]
pub struct ProcessSnapshot {
    processes: Vec<RunningProcess>,
    diagnostics: Vec<String>,
}
impl ProcessSnapshot {
    pub fn new(processes: Vec<RunningProcess>, diagnostics: Vec<String>) -> Self {
        Self {
            processes,
            diagnostics,
        }
    }
    pub fn processes(&self) -> &[RunningProcess] {
        &self.processes
    }
    pub fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedGameIdentity {
    executable: String,
    game_id: String,
}
impl LinkedGameIdentity {
    pub fn executable(&self) -> &str {
        &self.executable
    }
    pub fn game_id(&self) -> &str {
        &self.game_id
    }
}

#[derive(Clone, Debug)]
pub struct GameUpdate {
    messages: Vec<Value>,
    count: usize,
    diagnostics: Vec<String>,
    running_games: Vec<GameSignature>,
    available_game_ids: BTreeSet<String>,
    linked_identities: Vec<LinkedGameIdentity>,
}
impl GameUpdate {
    pub fn messages(&self) -> &[Value] {
        &self.messages
    }
    pub fn count(&self) -> usize {
        self.count
    }
    pub fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }
    pub fn running_games(&self) -> &[GameSignature] {
        &self.running_games
    }
    pub fn available_game_ids(&self) -> &BTreeSet<String> {
        &self.available_game_ids
    }
    /// Resolve an app-owned executable association through the same LGL/UGL
    /// priority as process matching, instead of confusing its app-local hash
    /// identity with the installed catalog's real entry_guid.
    pub fn linked_identities(&self) -> &[LinkedGameIdentity] {
        &self.linked_identities
    }
}

/// Source proxy deduplicates by entry_guid; PID is observation metadata only.
#[derive(Default)]
pub struct RunningGames {
    running: Vec<GameSignature>,
}
impl RunningGames {
    pub fn observe(
        &mut self,
        library: &GameLibrary,
        processes: &[RunningProcess],
        is_64_bit_os: bool,
    ) -> GameUpdate {
        let mut next: Vec<GameSignature> = Vec::new();
        for process in processes
            .iter()
            .filter(|process| !matches!(process.pid, 0 | 4))
        {
            if let Some(game) = library
                .match_process(&process.executable, is_64_bit_os)
                .filter(|game| game.has_identity())
                .filter(|game| {
                    !next
                        .iter()
                        .any(|running| running.entry_guid() == game.entry_guid())
                })
            {
                next.push(game.clone());
            }
        }
        let stopped = self
            .running
            .iter()
            .filter(|old| {
                !next
                    .iter()
                    .any(|game| old.entry_guid() == game.entry_guid())
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut messages = Vec::new();
        for game in stopped {
            self.running
                .retain(|running| running.entry_guid() != game.entry_guid());
            messages.push(self.message("GMS_onGameStoppedEvent", &game));
        }
        for game in next {
            if !self
                .running
                .iter()
                .any(|running| running.entry_guid() == game.entry_guid())
            {
                self.running.push(game.clone());
                messages.push(self.message("GMS_onGameStartedEvent", &game));
            }
        }
        let linked_identities = library
            .linked
            .iter()
            .filter_map(|link| {
                library
                    .match_process(link.launch_exe(), is_64_bit_os)
                    .filter(|game| game.has_identity())
                    .map(|game| LinkedGameIdentity {
                        executable: link.launch_exe().into(),
                        game_id: game.entry_guid().into(),
                    })
            })
            .collect::<Vec<_>>();
        let available_game_ids = library
            .local
            .iter()
            .chain(&library.user)
            .filter(|game| game.has_identity())
            .map(|game| game.entry_guid().to_owned())
            .chain(linked_identities.iter().map(|link| link.game_id.clone()))
            .collect();
        GameUpdate {
            messages,
            count: self.running.len(),
            diagnostics: Vec::new(),
            running_games: self.running.clone(),
            available_game_ids,
            linked_identities,
        }
    }

    /// Losing observation does not mean any game stopped. A successful next
    /// snapshot is republished so stale storage cannot survive recovery.
    pub fn invalidate(&mut self) {
        self.running.clear();
    }

    fn message(&self, action: &str, game: &GameSignature) -> Value {
        json!({"action":action,"payload":{
            "entry_guid":game.entry_guid(), "display_name":game.display_name(),
            "runningGames":self.running.iter().map(|game| game.entry_guid()).collect::<Vec<_>>(),
            "runningGamesDetail":self.running.iter().map(|game| json!({
                "entry_guid":game.entry_guid(),
                "exe_name":game.default_executables().into_iter().filter(|exe| !exe.is_empty()).collect::<Vec<_>>()
            })).collect::<Vec<_>>()
        }})
    }
}

/// Original keyboard consumer: host reply is JSON rows, each value is another
/// JSON string. Missing/malformed observation follows its unavailable fallback.
pub fn running_count_from_memory(reply: &Value) -> Option<usize> {
    let rows: Value = serde_json::from_str(reply.as_str()?).ok()?;
    let value = rows.as_array()?.first()?.get("value")?.as_str()?;
    let message: Value = serde_json::from_str(value).ok()?;
    Some(
        message
            .get("payload")?
            .get("runningGames")?
            .as_array()?
            .len(),
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CatalogKind {
    Local,
    User,
    Master,
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct CatalogStamp {
    kind: CatalogKind,
    path: PathBuf,
    bytes: u64,
    modified: Option<SystemTime>,
}

/// Local files are independent of any cloud account. Accessible persisted
/// user subfolders are imported into one local library, never synchronized.
pub struct CatalogLocations {
    root: PathBuf,
    fallback_master: Option<PathBuf>,
    machine_id: String,
}
impl CatalogLocations {
    pub fn new(root: PathBuf, fallback_master: Option<PathBuf>, machine_name: &str) -> Self {
        Self {
            root,
            fallback_master,
            machine_id: razer_storage::game_catalog::machine_catalog_id(machine_name),
        }
    }

    fn stamps(&self) -> Result<Vec<CatalogStamp>> {
        let mut stamps = Vec::new();
        let preferred = self.root.join("MasterGameList.xml");
        if !add_stamp(&mut stamps, CatalogKind::Master, &preferred)? {
            if let Some(fallback) = &self.fallback_master {
                add_stamp(&mut stamps, CatalogKind::Master, fallback)?;
            }
        }
        let data = self.root.join("Data");
        let mut directories = vec![data.clone()];
        match std::fs::read_dir(&data) {
            Ok(entries) => {
                for entry in entries {
                    let entry = entry?;
                    if entry.file_type()?.is_dir() {
                        ensure!(
                            directories.len() < 1024,
                            "Too many local GMS catalog folders"
                        );
                        directories.push(entry.path());
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("List local GMS catalog folders"),
        }
        directories.sort();
        for directory in directories {
            for (kind, prefix) in [
                (CatalogKind::Local, "LocalGameListEnc"),
                (CatalogKind::User, "UserGameListEnc"),
            ] {
                add_stamp(
                    &mut stamps,
                    kind,
                    &directory.join(format!("{prefix}_{}.xml", self.machine_id)),
                )?;
            }
        }
        Ok(stamps)
    }
}

fn add_stamp(stamps: &mut Vec<CatalogStamp>, kind: CatalogKind, path: &Path) -> Result<bool> {
    match std::fs::metadata(path) {
        Ok(metadata) => {
            ensure!(
                metadata.is_file(),
                "Local GMS catalog is not a file: {}",
                path.display()
            );
            stamps.push(CatalogStamp {
                kind,
                path: path.into(),
                bytes: metadata.len(),
                modified: Some(
                    metadata
                        .modified()
                        .context("Read local catalog modification time")?,
                ),
            });
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => {
            Err(error).with_context(|| format!("Inspect local GMS catalog {}", path.display()))
        }
    }
}

pub struct CatalogCache {
    locations: CatalogLocations,
    stamps: Option<Vec<CatalogStamp>>,
    library: GameLibrary,
}
impl CatalogCache {
    pub fn new(locations: CatalogLocations) -> Self {
        Self {
            locations,
            stamps: None,
            library: GameLibrary::default(),
        }
    }
    pub fn set_linked(&mut self, linked: Vec<GameSignature>) {
        self.library.linked = linked;
    }

    pub fn refresh(&mut self) -> Result<&GameLibrary> {
        let stamps = self.locations.stamps()?;
        if self.stamps.as_ref() != Some(&stamps) {
            let mut local = Vec::new();
            let mut user = Vec::new();
            let mut master = Vec::new();
            for stamp in &stamps {
                let catalog = read_game_catalog(&stamp.path)?
                    .context("Local game catalog disappeared while reading")?;
                match stamp.kind {
                    CatalogKind::Local => local.extend(catalog.into_signatures()),
                    CatalogKind::User => user.extend(catalog.into_signatures()),
                    CatalogKind::Master => master.extend(catalog.into_signatures()),
                }
            }
            let mut master_ids = BTreeSet::new();
            master.retain(|game| master_ids.insert(game.entry_guid().to_owned()));
            // MglDataManager.InitMglDictionary appends this current embedded
            // offline signature after deduplicating the installed master list.
            let internal = razer_storage::game_catalog::internal_scan_list()?;
            for signature in internal.into_signatures() {
                if !master
                    .iter()
                    .any(|game| game.entry_guid() == signature.entry_guid())
                {
                    master.push(signature);
                }
            }
            self.library = GameLibrary::new(local, user, master, self.library.linked.clone());
            self.stamps = Some(stamps);
        }
        Ok(&self.library)
    }
}

pub enum GameObservation {
    Updated(GameUpdate),
    Failed(String),
}
enum Command {
    Linked(Vec<GameSignature>),
    Stop,
}

pub struct GameMonitor {
    commands: mpsc::Sender<Command>,
    worker: Option<JoinHandle<()>>,
    canceled: Arc<AtomicBool>,
}
impl GameMonitor {
    pub fn start() -> Result<(Self, async_channel::Receiver<GameObservation>)> {
        #[cfg(not(windows))]
        anyhow::bail!("Local GMS process observation requires Windows");
        #[cfg(windows)]
        {
            let (locations, is_64_bit_os) =
                crate::platform::windows::game_monitor::catalog_locations()?;
            let (commands, receiver) = mpsc::channel();
            let (events, observations) = async_channel::unbounded();
            let canceled = Arc::new(AtomicBool::new(false));
            let signal = canceled.clone();
            let worker = thread::Builder::new()
                .name("local-game-observation".into())
                .spawn(move || {
                    let mut cache = CatalogCache::new(locations);
                    let mut running = RunningGames::default();
                    let mut previous: Option<(
                        usize,
                        Vec<String>,
                        BTreeSet<String>,
                        Vec<LinkedGameIdentity>,
                    )> = None;
                    let mut failed = false;
                    loop {
                        if signal.load(Ordering::Acquire) {
                            break;
                        }
                        let result = (|| -> Result<GameUpdate> {
                            let library = cache.refresh()?;
                            let snapshot =
                                crate::platform::windows::game_monitor::process_snapshot(
                                    &library.candidate_names(),
                                    &|| {
                                        ensure!(
                                            !signal.load(Ordering::Acquire),
                                            "Local game observation canceled"
                                        );
                                        Ok(())
                                    },
                                )?;
                            let mut update =
                                running.observe(library, snapshot.processes(), is_64_bit_os);
                            update.diagnostics = snapshot.diagnostics;
                            Ok(update)
                        })();
                        if signal.load(Ordering::Acquire) {
                            break;
                        }
                        let observation = match result {
                            Ok(update) => {
                                let signature = (
                                    update.count,
                                    update.diagnostics.clone(),
                                    update.available_game_ids.clone(),
                                    update.linked_identities.clone(),
                                );
                                let emit = failed
                                    || previous.as_ref() != Some(&signature)
                                    || !update.messages.is_empty();
                                previous = Some(signature);
                                failed = false;
                                emit.then_some(GameObservation::Updated(update))
                            }
                            Err(error) => {
                                running.invalidate();
                                previous = None;
                                failed = true;
                                Some(GameObservation::Failed(format!("{error:#}")))
                            }
                        };
                        if observation
                            .is_some_and(|observation| events.try_send(observation).is_err())
                        {
                            break;
                        }
                        match receiver.recv_timeout(Duration::from_millis(500)) {
                            Ok(Command::Linked(linked)) => cache.set_linked(linked),
                            Ok(Command::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                            Err(mpsc::RecvTimeoutError::Timeout) => {}
                        }
                    }
                })
                .context("Start local game observation worker")?;
            Ok((
                Self {
                    commands,
                    worker: Some(worker),
                    canceled,
                },
                observations,
            ))
        }
    }

    pub fn set_linked(&self, linked: Vec<GameSignature>) -> Result<()> {
        self.commands
            .send(Command::Linked(linked))
            .context("Local game observer has stopped")
    }
    pub fn retire(mut self) -> Option<JoinHandle<()>> {
        self.canceled.store(true, Ordering::Release);
        let _ = self.commands.send(Command::Stop);
        self.worker.take()
    }
}
impl Drop for GameMonitor {
    fn drop(&mut self) {
        self.canceled.store(true, Ordering::Release);
        let _ = self.commands.send(Command::Stop);
    }
}

#[cfg(test)]
#[path = "game_monitor_tests.rs"]
mod tests;
