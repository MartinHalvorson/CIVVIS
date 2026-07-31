//! The lobby's third game mode: play Firaxis's Civilization VI, under control.
//!
//! The other two modes build a world in `src/game.rs` and play it. This one
//! builds nothing: it starts a real game of Sid Meier's Civilization VI on the
//! computer serving the page, hands the human seat to `tools/civ6_play.py`, and
//! reports what that run is doing. `docs/CIV6_GAME_MODE.md` is the design
//! contract; this module is the vocabulary the two games are configured in and
//! the boundary between them.
//!
//! Three facts about the shape of it, each of which decided a design question:
//!
//! - **A difficulty means something here and nowhere else.** Civilization VI
//!   gives its handicap bonuses to *human* seats, so the autoplay measurements
//!   in `docs/GROUNDING.md` cannot say anything about difficulty at all. Taking
//!   the seat is what makes the ladder in `docs/CIV6_LADDER.md` climbable, and
//!   it is why this mode is worth a row in the mode select.
//! - **The settings that carry are the ones the setup mod sets.**
//!   `CivvisControlSetup.lua` writes ruleset, map script, map size, difficulty
//!   and speed into `MapConfiguration`/`GameConfiguration` and nothing else. A
//!   lobby row for anything else — a leader, a team, a victory condition —
//!   would be a promise about the run that the run does not keep.
//! - **The mode can be refused, and the refusal is the feature.** It needs the
//!   game installed on the machine serving the page and needs nobody else
//!   driving it. Both of those have gone wrong expensively: a dead Steam client
//!   burned eleven of twenty-four ladder attempts, each recorded as a *loss*
//!   rather than as an attempt that never happened, and two harnesses sharing
//!   one installation produced weeks of what read as a flaky game. So every
//!   path here answers with a sentence rather than with nothing.

use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// A map script, as Civilization VI's own configuration database registers it.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapSpec {
    /// The script file. `MapConfiguration.SetScript` takes exactly this string
    /// and `tools/civ6_play.py --map` passes it through unchanged, so it is the
    /// identifier rather than a name to be looked up in a table.
    pub id: &'static str,
    pub name: &'static str,
    /// The CIVVIS world this is the nearest thing to, where there is one, so a
    /// map chosen in one mode survives switching to the other. Most of the
    /// roster has no counterpart in either direction; see the module docs.
    pub civvis: Option<&'static str>,
}

/// The map scripts a Gathering Storm game offers, in the game's own order.
///
/// Transcribed from the `Maps` rows of
/// `Base/Assets/Configuration/Data/StandardMaps.xml` and
/// `DLC/Expansion1|2/Config/*_StandardMaps.xml`, ordered by their `SortIndex`.
/// `WorldBuilderMap.lua` is omitted: it is flagged `WorldBuilderOnly` and is an
/// empty map, not a generator.
///
/// This is a transcription rather than a read of the installation on purpose.
/// The lobby's contents must not depend on a game being installed on whichever
/// machine happens to serve the page — the mode is offered everywhere and
/// *refused* with a reason where it cannot run. `tools/civ6_setup.py` is where
/// the list is checked against an install that does exist.
pub const MAPS: [MapSpec; 15] = [
    MapSpec { id: "Continents.lua", name: "Continents", civvis: Some("continents") },
    MapSpec { id: "Fractal.lua", name: "Fractal", civvis: None },
    MapSpec { id: "InlandSea.lua", name: "Inland Sea", civvis: Some("inland_sea") },
    MapSpec { id: "Island_Plates.lua", name: "Island Plates", civvis: Some("islands") },
    MapSpec { id: "Lakes.lua", name: "Lakes", civvis: Some("lakes") },
    MapSpec { id: "Pangaea.lua", name: "Pangaea", civvis: Some("pangaea") },
    MapSpec { id: "Seven_Seas.lua", name: "Seven Seas", civvis: None },
    MapSpec { id: "Shuffle.lua", name: "Shuffle", civvis: None },
    MapSpec { id: "Small_Continents.lua", name: "Small Continents", civvis: Some("small_continents") },
    MapSpec { id: "Terra.lua", name: "Terra", civvis: None },
    MapSpec { id: "Archipelago.lua", name: "Archipelago", civvis: Some("water_world") },
    MapSpec { id: "Continents_Islands.lua", name: "Continents & Islands", civvis: None },
    MapSpec { id: "Primordial.lua", name: "Primordial", civvis: None },
    MapSpec { id: "Splintered_Fractal.lua", name: "Splintered Fractal", civvis: None },
    MapSpec { id: "Tilted_Axis.lua", name: "Tilted Axis", civvis: None },
];

/// The script a game gets when nothing else applies: Civilization VI's own
/// default, and the first row of its list.
pub const DEFAULT_MAP: &str = "Continents.lua";

/// One rung of the handicap ladder.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DifficultySpec {
    /// The game's `Handicaps` type, which is what `GameConfiguration
    /// .SetHandicapType` and `--difficulty` take.
    pub id: &'static str,
    pub name: &'static str,
    /// Our difficulty of the same name. The two ladders are the same eight
    /// rungs because ours was built from theirs, so this is always present —
    /// but it is written down rather than derived, because a ladder that grew
    /// a rung on one side and silently mapped it to the wrong one on the other
    /// would be a handicap nobody chose.
    pub civvis: &'static str,
}

/// The eight rungs, weakest first. `tools/civ6_play.py` takes these ids
/// directly and rejects anything else.
pub const DIFFICULTIES: [DifficultySpec; 8] = [
    DifficultySpec { id: "DIFFICULTY_SETTLER", name: "Settler", civvis: "settler" },
    DifficultySpec { id: "DIFFICULTY_CHIEFTAIN", name: "Chieftain", civvis: "chieftain" },
    DifficultySpec { id: "DIFFICULTY_WARLORD", name: "Warlord", civvis: "warlord" },
    DifficultySpec { id: "DIFFICULTY_PRINCE", name: "Prince", civvis: "prince" },
    DifficultySpec { id: "DIFFICULTY_KING", name: "King", civvis: "king" },
    DifficultySpec { id: "DIFFICULTY_EMPEROR", name: "Emperor", civvis: "emperor" },
    DifficultySpec { id: "DIFFICULTY_IMMORTAL", name: "Immortal", civvis: "immortal" },
    DifficultySpec { id: "DIFFICULTY_DEITY", name: "Deity", civvis: "deity" },
];

/// A world size, keyed by the seat count our lobby chooses it with.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SizeSpec {
    pub id: &'static str,
    pub name: &'static str,
    /// The CIVVIS map size of the same name, and the number of major seats our
    /// world-size control carries for it.
    pub civvis: &'static str,
    pub players: usize,
}

/// The six stock sizes. Ours are the same six rows read out of the same table
/// (`src/setup::CIV6_MAP_SIZES`, whose first six entries these are), plus four
/// larger worlds Civilization VI does not ship — which is why picking one of
/// those and switching to this mode has to fall back rather than translate.
pub const SIZES: [SizeSpec; 6] = [
    SizeSpec { id: "MAPSIZE_DUEL", name: "Duel", civvis: "duel", players: 2 },
    SizeSpec { id: "MAPSIZE_TINY", name: "Tiny", civvis: "tiny", players: 4 },
    SizeSpec { id: "MAPSIZE_SMALL", name: "Small", civvis: "small", players: 6 },
    SizeSpec { id: "MAPSIZE_STANDARD", name: "Standard", civvis: "standard", players: 8 },
    SizeSpec { id: "MAPSIZE_LARGE", name: "Large", civvis: "large", players: 10 },
    SizeSpec { id: "MAPSIZE_HUGE", name: "Huge", civvis: "huge", players: 12 },
];

/// A game speed and the turn limit it comes with.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpeedSpec {
    pub id: &'static str,
    pub name: &'static str,
    pub civvis: &'static str,
    /// The limit `--max-turns` is given. Both games use the same five numbers;
    /// `src/setup::GameSpeed::turn_limit` is the other copy.
    pub turns: u32,
}

pub const SPEEDS: [SpeedSpec; 5] = [
    SpeedSpec { id: "GAMESPEED_ONLINE", name: "Online", civvis: "online", turns: 250 },
    SpeedSpec { id: "GAMESPEED_QUICK", name: "Quick", civvis: "quick", turns: 330 },
    SpeedSpec { id: "GAMESPEED_STANDARD", name: "Standard", civvis: "standard", turns: 500 },
    SpeedSpec { id: "GAMESPEED_EPIC", name: "Epic", civvis: "epic", turns: 750 },
    SpeedSpec { id: "GAMESPEED_MARATHON", name: "Marathon", civvis: "marathon", turns: 1500 },
];

/// The ruleset a run is configured with. Gathering Storm is the only one the
/// grounding work has ever measured against, and the only one CIVVIS's own
/// rules claim to be.
pub const RULESET: &str = "RULESET_EXPANSION_2";

/// Everything the lobby needs to fill the mode's controls. It never changes
/// while a server runs, so it rides on `/rules` with the rest of the
/// vocabulary rather than on the per-request host report.
pub fn vocabulary() -> Value {
    serde_json::json!({
        "maps": MAPS,
        "difficulties": DIFFICULTIES,
        "sizes": SIZES,
        "speeds": SPEEDS,
        "default_map": DEFAULT_MAP,
        "ruleset": RULESET,
    })
}

/// The map script for one of our world types, or `None` where the other game
/// has no counterpart for it (Grand Canals, Land Only, True Start Earth).
pub fn map_for_civvis(script: &str) -> Option<&'static MapSpec> {
    MAPS.iter().find(|spec| spec.civvis == Some(script))
}

/// The size Civilization VI is asked for when our lobby is set to `players`
/// major seats. Sizes above Huge do not exist in the other game.
pub fn size_for_players(players: usize) -> Option<&'static SizeSpec> {
    SIZES.iter().find(|spec| spec.players == players)
}

pub fn difficulty_for_civvis(id: &str) -> Option<&'static DifficultySpec> {
    DIFFICULTIES.iter().find(|spec| spec.civvis == id)
}

pub fn speed_for_civvis(id: &str) -> Option<&'static SpeedSpec> {
    SPEEDS.iter().find(|spec| spec.civvis == id)
}

// --- the host -------------------------------------------------------------

/// Where Steam and the standalone installer put the game on macOS. Kept in
/// step with `tools/civ6_env.py`'s `INSTALL_CANDIDATES`, which is the copy the
/// controller itself reads; `$CIV6_INSTALL` overrides both.
fn install_candidates(home: &Path) -> Vec<PathBuf> {
    vec![
        home.join("Library/Application Support/Steam/steamapps/common/Sid Meier's Civilization VI"),
        PathBuf::from("/Applications/Sid Meier's Civilization VI"),
    ]
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

/// The installation root — the directory holding `Civ6.app` — or `None`.
pub fn install_dir() -> Option<PathBuf> {
    if let Some(explicit) = std::env::var_os("CIV6_INSTALL") {
        let path = PathBuf::from(explicit);
        return path.is_dir().then_some(path);
    }
    install_candidates(&home_dir()).into_iter().find(|path| path.is_dir())
}

/// This repository's `tools/`, which holds the controller.
///
/// A server binary can be started from anywhere, so this is answered by
/// looking rather than by assuming: `$CIVVIS_TOOLS` first, then beside the
/// working directory, then up from the executable — `target/release/civvis`
/// and `target/ci/civvis` both sit two levels below the repository root.
pub fn tools_dir() -> Option<PathBuf> {
    let holds_controller = |dir: &Path| dir.join("civ6_play.py").is_file();
    if let Some(explicit) = std::env::var_os("CIVVIS_TOOLS") {
        let path = PathBuf::from(explicit);
        return holds_controller(&path).then_some(path);
    }
    let mut roots = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        roots.extend(exe.ancestors().skip(1).map(Path::to_path_buf));
    }
    roots
        .into_iter()
        .flat_map(|root| root.ancestors().map(Path::to_path_buf).collect::<Vec<_>>())
        .map(|root| root.join("tools"))
        .find(|dir| holds_controller(dir))
}

/// Whoever is driving the installation right now.
///
/// `tools/civ6_control/gamelock.py` holds this while a run is up. There is one
/// installation, one mod directory inside it, one log file and one process, and
/// two harnesses driving that do not conflict loudly — they conflict silently,
/// and the result reads as a flaky game. So the lobby reads the same lock the
/// controller does, and names the run in the way rather than failing to start.
#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
pub struct Holder {
    pub pid: i64,
    pub tag: String,
    pub since: String,
    /// Whether the process named is still alive. A holder whose process is gone
    /// is stale — the failure the lock guards against is concurrency, not
    /// crashes, so a killed run must not block the next one forever.
    pub alive: bool,
}

fn lock_dir() -> PathBuf {
    home_dir().join(".civvis-civ6-game.lock")
}

/// True if a process with this id exists. `kill -0` is the portable test and
/// costs a syscall; without `libc` the readable equivalent is to ask `ps`.
fn process_alive(pid: i64) -> bool {
    if pid <= 0 {
        return false;
    }
    Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "pid="])
        .output()
        .map(|out| out.status.success() && !out.stdout.is_empty())
        .unwrap_or(false)
}

pub fn holder() -> Option<Holder> {
    let raw = fs::read_to_string(lock_dir().join("holder.json")).ok()?;
    let parsed: Value = serde_json::from_str(&raw).ok()?;
    let pid = parsed["pid"].as_i64().unwrap_or(0);
    Some(Holder {
        pid,
        tag: parsed["tag"].as_str().unwrap_or("a run").to_string(),
        since: parsed["since"].as_str().unwrap_or_default().to_string(),
        alive: process_alive(pid),
    })
}

/// Where `tools/civ6_play.py` keeps its runs. One directory per tag.
pub fn run_root() -> PathBuf {
    home_dir().join("civvis-civ6-runs").join("control")
}

/// How far a run has got, read from the record the controller keeps.
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct Run {
    pub tag: String,
    /// The last turn the seat played, and what the empire looked like on it.
    pub turn: u64,
    pub cities: u64,
    pub units: u64,
    pub score: i64,
    /// Who the game dealt us. The controller cannot choose this — the setup mod
    /// sets ruleset, map, size, difficulty and speed, and the civilization is
    /// whatever the game hands the seat.
    pub civ: String,
    pub leader: String,
    pub difficulty: String,
    pub map: String,
    pub size: String,
    pub speed: String,
    pub max_turns: u64,
    /// Present once the run has stopped: `summary.json`'s reason, and the
    /// outcome event if the game reached one.
    pub reason: Option<String>,
    pub won: Option<bool>,
    /// Whether this run still holds the game.
    pub live: bool,
}

/// Read a run directory. `summary.json` is written once, at the end; the live
/// picture is the last `turn` line of `events.jsonl`, so both are consulted and
/// the events win on the fields they both carry.
fn read_run(dir: &Path, live: bool) -> Option<Run> {
    let tag = dir.file_name()?.to_string_lossy().to_string();
    let mut run = Run { tag, live, ..Run::default() };
    let mut seen_anything = false;

    if let Ok(raw) = fs::read_to_string(dir.join("summary.json")) {
        if let Ok(summary) = serde_json::from_str::<Value>(&raw) {
            seen_anything = true;
            run.reason = summary["reason"].as_str().map(str::to_string);
            run.won = summary["outcome"]["won"].as_bool();
            run.turn = summary["last_turn"].as_u64().unwrap_or(0);
            run.score = summary["last_score"].as_i64().unwrap_or(0);
            run.difficulty = summary["difficulty"].as_str().unwrap_or_default().into();
            run.size = summary["map_size"].as_str().unwrap_or_default().into();
            run.speed = summary["speed"].as_str().unwrap_or_default().into();
            run.max_turns = summary["max_turns"].as_u64().unwrap_or(0);
        }
    }
    for event in last_events(&dir.join("events.jsonl")) {
        seen_anything = true;
        match event["kind"].as_str() {
            Some("seat") => {
                run.civ = event["civ"].as_str().unwrap_or_default().into();
                run.leader = event["leader"].as_str().unwrap_or_default().into();
                run.map = event["map"].as_str().unwrap_or_default().into();
                for (field, key) in [
                    (&mut run.difficulty, "difficulty"),
                    (&mut run.size, "size"),
                    (&mut run.speed, "speed"),
                ] {
                    if let Some(value) = event[key].as_str() {
                        *field = value.to_string();
                    }
                }
            }
            Some("turn") => {
                run.turn = event["turn"].as_u64().unwrap_or(run.turn);
                run.cities = event["cities"].as_u64().unwrap_or(run.cities);
                run.units = event["units"].as_u64().unwrap_or(run.units);
                // The mod reports -1 for a score it could not read, which is
                // not a score of minus one.
                if let Some(score) = event["score"].as_i64().filter(|score| *score >= 0) {
                    run.score = score;
                }
            }
            _ => {}
        }
    }
    seen_anything.then_some(run)
}

/// The `seat` and `turn` lines of a run's log, oldest first.
///
/// A finished run's `events.jsonl` is megabytes and almost all of it is
/// per-tick chatter, so only the tail is read — far enough back to hold the
/// last turn comfortably. `seat` is emitted once, at the start, so it is
/// searched for from the whole file's head instead; that line is short and the
/// first few hundred bytes always contain it.
fn last_events(path: &Path) -> Vec<Value> {
    const TAIL: u64 = 96 * 1024;
    let Ok(text) = read_tail(path, TAIL) else {
        return Vec::new();
    };
    let mut found: Vec<Value> = Vec::new();
    let mut seat = None;
    let mut turn = None;
    for line in text.lines() {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            continue; // a truncated first line, or a log the mod half-wrote
        };
        match event["kind"].as_str() {
            Some("seat") => seat = Some(event),
            Some("turn") => turn = Some(event),
            _ => {}
        }
    }
    if seat.is_none() {
        seat = read_head(path, 8 * 1024).ok().and_then(|head| {
            head.lines()
                .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                .find(|event| event["kind"].as_str() == Some("seat"))
        });
    }
    found.extend(seat);
    found.extend(turn);
    found
}

fn read_tail(path: &Path, bytes: u64) -> std::io::Result<String> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = fs::File::open(path)?;
    let len = file.metadata()?.len();
    file.seek(SeekFrom::Start(len.saturating_sub(bytes)))?;
    let mut raw = Vec::new();
    file.read_to_end(&mut raw)?;
    Ok(String::from_utf8_lossy(&raw).into_owned())
}

fn read_head(path: &Path, bytes: u64) -> std::io::Result<String> {
    use std::io::Read;
    let mut raw = Vec::new();
    fs::File::open(path)?.take(bytes).read_to_end(&mut raw)?;
    Ok(String::from_utf8_lossy(&raw).into_owned())
}

/// The newest run, whether or not this server started it.
///
/// Tracking a run somebody started from a terminal is the same job as tracking
/// one started from the lobby, and it is what happens the first time somebody
/// starts one out of habit. The lock says which run is live; the newest
/// directory says which run to report.
pub fn latest_run() -> Option<Run> {
    let live_tag = holder().filter(|held| held.alive).map(|held| held.tag);
    let mut newest: Option<(std::time::SystemTime, PathBuf)> = None;
    for entry in fs::read_dir(run_root()).ok()?.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        // A directory the controller has only just made has no events yet;
        // ordering on the log rather than the directory keeps a run that
        // failed to start from displacing the one that is playing.
        let stamped = fs::metadata(path.join("events.jsonl"))
            .or_else(|_| entry.metadata())
            .and_then(|meta| meta.modified())
            .ok()?;
        if newest.as_ref().is_none_or(|(best, _)| stamped > *best) {
            newest = Some((stamped, path));
        }
    }
    let (_, dir) = newest?;
    let live = live_tag.as_deref() == dir.file_name().and_then(|name| name.to_str());
    read_run(&dir, live)
}

/// What this computer can do about the mode, and what is in the way.
#[derive(Serialize, Clone, Debug)]
pub struct Host {
    pub install: Option<String>,
    pub controller: Option<String>,
    /// `None` when a run can be started right now; otherwise the one sentence
    /// that says why not.
    pub blocked: Option<String>,
    pub holder: Option<Holder>,
    pub run: Option<Run>,
}

impl Host {
    pub fn probe() -> Self {
        let install = install_dir();
        let tools = tools_dir();
        let held = holder();
        let blocked = if install.is_none() {
            Some("Civilization VI is not installed on this computer".to_string())
        } else if tools.is_none() {
            Some("the CIVVIS tools directory is not beside this server".to_string())
        } else if !python_available() {
            Some("python3 is not on this server's PATH".to_string())
        } else {
            held.as_ref().filter(|held| held.alive).map(|held| {
                format!(
                    "{} already holds the game (pid {}{})",
                    held.tag,
                    held.pid,
                    if held.since.is_empty() {
                        String::new()
                    } else {
                        format!(", since {}", held.since)
                    }
                )
            })
        };
        Self {
            install: install.map(|path| path.display().to_string()),
            controller: tools.map(|dir| dir.join("civ6_play.py").display().to_string()),
            blocked,
            holder: held,
            run: latest_run(),
        }
    }

    pub fn ready(&self) -> bool {
        self.blocked.is_none()
    }
}

fn python_available() -> bool {
    Command::new("python3")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

// --- starting a run -------------------------------------------------------

/// A game to start, in Civilization VI's vocabulary.
///
/// Built from the lobby's settings by [`Request::from_settings`], which is
/// where our ids become theirs; every field here is passed to the controller
/// unchanged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub difficulty: &'static str,
    pub map: String,
    pub size: &'static str,
    pub speed: &'static str,
    pub max_turns: u32,
}

impl Request {
    /// Translate the lobby's selection.
    ///
    /// Every field falls back rather than failing, because each one can be a
    /// choice the other game does not have — a Grand Canals world, a Massive
    /// map — and a refusal to start over a map name is a worse answer than the
    /// game's own default. The one exception is the difficulty, which is the
    /// point of the mode: an unrecognised rung is an error rather than a
    /// silent Settler.
    pub fn from_settings(settings: &Value) -> Result<Self, String> {
        let difficulty = settings["difficulty"].as_str().unwrap_or("prince");
        let difficulty = difficulty_for_civvis(difficulty)
            .ok_or_else(|| format!("no Civilization VI difficulty is called {difficulty:?}"))?;
        // `civ6_map` lets the client name a script directly; the lobby does,
        // because in this mode its map control is Civilization VI's own list.
        // A CIVVIS world type is still accepted and translated, so a settings
        // payload built for either mode starts the same game.
        let map = settings["civ6_map"]
            .as_str()
            .filter(|name| MAPS.iter().any(|spec| spec.id == *name))
            .map(str::to_string)
            .or_else(|| {
                settings["map_script"]
                    .as_str()
                    .and_then(map_for_civvis)
                    .map(|spec| spec.id.to_string())
            })
            .unwrap_or_else(|| DEFAULT_MAP.to_string());
        let players = settings["num_players"].as_u64().unwrap_or(4) as usize;
        let size = size_for_players(players).unwrap_or(&SIZES[1]);
        let speed = settings["game_speed"]
            .as_str()
            .and_then(speed_for_civvis)
            .unwrap_or(&SPEEDS[0]);
        Ok(Self {
            difficulty: difficulty.id,
            map,
            size: size.id,
            speed: speed.id,
            max_turns: speed.turns,
        })
    }

    fn argv(&self, tag: &str) -> Vec<String> {
        [
            "--tag",
            tag,
            "--difficulty",
            self.difficulty,
            "--ruleset",
            RULESET,
            "--map",
            &self.map,
            "--map-size",
            self.size,
            "--speed",
            self.speed,
            "--max-turns",
            &self.max_turns.to_string(),
        ]
        .iter()
        .map(|arg| arg.to_string())
        .collect()
    }
}

/// A run that has been asked for. The controller takes about three minutes to
/// reach the first turn, so this is the receipt rather than the game.
#[derive(Serialize, Clone, Debug)]
pub struct Started {
    pub tag: String,
    pub pid: u32,
    pub run_dir: String,
    pub command: Vec<String>,
}

/// Put the run in a session of its own, so it outlives the page that started
/// it.
///
/// It holds the game lock and drives a window for hours; tying that to the
/// lifetime of a browser tab is how a Deity attempt is lost at turn 180. There
/// is no `setsid` binary on macOS, so the interpreter makes the call itself
/// immediately before becoming the controller.
const DETACH: &str =
    "import os, sys; os.setsid(); os.execv(sys.executable, [sys.executable] + sys.argv[1:])";

/// Start a game. Refuses, with a sentence, rather than failing quietly.
pub fn start(request: &Request, tag: &str) -> Result<Started, String> {
    let host = Host::probe();
    if let Some(blocked) = host.blocked {
        return Err(blocked);
    }
    let controller = host
        .controller
        .ok_or_else(|| "the CIVVIS tools directory is not beside this server".to_string())?;
    let run_dir = run_root().join(tag);
    fs::create_dir_all(&run_dir)
        .map_err(|error| format!("cannot make the run directory {}: {error}", run_dir.display()))?;
    let log = fs::File::create(run_dir.join("controller.log"))
        .map_err(|error| format!("cannot open the controller's log: {error}"))?;
    let errors = log
        .try_clone()
        .map_err(|error| format!("cannot open the controller's log: {error}"))?;
    let argv = request.argv(tag);
    let child = Command::new("python3")
        .arg("-c")
        .arg(DETACH)
        .arg(&controller)
        .args(&argv)
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(errors))
        .spawn()
        .map_err(|error| format!("cannot start {controller}: {error}"))?;
    let mut command = vec!["python3".to_string(), controller];
    command.extend(argv);
    Ok(Started {
        tag: tag.to_string(),
        pid: child.id(),
        run_dir: run_dir.display().to_string(),
        command,
    })
}

/// A run tag: the UTC second it was asked for, which is what
/// `tools/civ6_play.py` names its directory after and what the game lock
/// reports as its holder.
pub fn new_tag(now: std::time::SystemTime) -> String {
    let secs = now
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or(0);
    let (year, month, day, hour, minute, second) = civil_from_unix(secs as i64);
    format!("civvis-{year:04}{month:02}{day:02}T{hour:02}{minute:02}{second:02}Z")
}

/// Days-to-civil-date, Howard Hinnant's algorithm. The crate has no date
/// dependency and this is the only place one would be wanted.
fn civil_from_unix(secs: i64) -> (i64, u32, u32, u32, u32, u32) {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let year = if m <= 2 { y + 1 } else { y };
    (year, m, d, (rem / 3_600) as u32, ((rem % 3_600) / 60) as u32, (rem % 60) as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The lobby's map control is Civilization VI's own list in this mode, so
    /// every row has to name a script the game will accept. The two rosters
    /// overlap without either containing the other, which is the whole reason
    /// the list is replaced rather than filtered.
    #[test]
    fn every_map_names_a_script_and_the_shared_ones_round_trip() {
        for spec in MAPS {
            assert!(spec.id.ends_with(".lua"), "{} is not a script file", spec.id);
            assert!(!spec.name.is_empty());
            if let Some(civvis) = spec.civvis {
                assert!(
                    crate::setup::CIV6_MAP_SCRIPTS.iter().any(|ours| ours.id == civvis),
                    "{} claims a CIVVIS world {civvis} that does not exist",
                    spec.id
                );
                assert_eq!(map_for_civvis(civvis).map(|found| found.id), Some(spec.id));
            }
        }
        // Both directions have worlds the other lacks. If that ever stops
        // being true the mode should filter rather than replace.
        assert!(MAPS.iter().any(|spec| spec.civvis.is_none()));
        assert!(crate::setup::CIV6_MAP_SCRIPTS
            .iter()
            .any(|ours| map_for_civvis(ours.id).is_none()));
        // A world with no counterpart falls back to the game's own default
        // rather than refusing to start.
        assert_eq!(map_for_civvis("grand_canals"), None);
    }

    /// Every script in the roster is a file this installation actually has.
    ///
    /// The roster is a transcription, so the failure it is exposed to is a
    /// typo — `Small_Continents.lua` is spelled with an underscore where the
    /// game's own name for it has a space, and `InlandSea.lua` has neither.
    /// A wrong name is not refused at the door: the run configures, launches,
    /// and comes up on whatever map the game falls back to, three minutes
    /// later and several screens away from the cause.
    ///
    /// This can only run where the game is, so it is a check rather than an
    /// assertion about the world: on a machine with no installation (CI is
    /// Linux) it passes without looking, and `Host::probe` is what tells a
    /// person the mode is unavailable there.
    #[test]
    fn every_map_in_the_roster_exists_on_an_install_that_is_here() {
        let Some(install) = install_dir() else {
            return; // no game on this computer; the mode is refused, not broken
        };
        // Base ships the stock scripts, the expansions add and override.
        let assets = install.join("Civ6.app/Contents/Assets");
        let dirs = [
            assets.join("Base/Assets/Maps"),
            assets.join("DLC/Expansion1/Maps"),
            assets.join("DLC/Expansion2/Maps"),
        ];
        if !dirs.iter().any(|dir| dir.is_dir()) {
            return; // an install laid out some other way, or a partial one
        }
        for spec in MAPS {
            assert!(
                dirs.iter().any(|dir| dir.join(spec.id).is_file()),
                "{} is in the lobby's roster but no such script is installed",
                spec.id
            );
        }
    }

    /// The two ladders are the same eight rungs in the same order, because ours
    /// was built from theirs. A rung added to one and not the other would hand
    /// out a handicap nobody chose.
    #[test]
    fn the_difficulty_ladders_line_up_rung_for_rung() {
        let ours = crate::rules::Rules::embedded();
        let mut ladder: Vec<_> = ours.difficulties.iter().collect();
        ladder.sort_by_key(|(_, spec)| spec.order);
        assert_eq!(ladder.len(), DIFFICULTIES.len());
        for (rung, (id, _)) in DIFFICULTIES.iter().zip(ladder) {
            assert_eq!(rung.civvis, id.as_str());
            assert_eq!(rung.id, format!("DIFFICULTY_{}", id.to_uppercase()));
            assert_eq!(difficulty_for_civvis(id).map(|found| found.id), Some(rung.id));
        }
    }

    /// Sizes and speeds are the same rows read out of the same tables, so the
    /// seat counts and the turn limits must agree with `src/setup.rs` — the
    /// lobby chooses a world by its seat count and the run is given a turn
    /// limit derived from its speed.
    #[test]
    fn sizes_and_speeds_agree_with_our_own_tables() {
        for (index, size) in SIZES.iter().enumerate() {
            let ours = crate::setup::CIV6_MAP_SIZES[index];
            assert_eq!(size.civvis, ours.id);
            assert_eq!(size.players, ours.default_players);
            assert_eq!(size.id, format!("MAPSIZE_{}", ours.id.to_uppercase()));
            assert_eq!(size_for_players(size.players).map(|found| found.id), Some(size.id));
        }
        // Sizes past Huge are ours alone, so they have to fall back.
        assert!(crate::setup::CIV6_MAP_SIZES.len() > SIZES.len());
        assert_eq!(size_for_players(16), None);
        for speed in SPEEDS {
            let ours = crate::setup::GameSpeed::from_id(speed.civvis)
                .unwrap_or_else(|| panic!("{} is not one of our speeds", speed.civvis));
            assert_eq!(speed.turns, ours.turn_limit());
            assert_eq!(speed_for_civvis(speed.civvis).map(|found| found.id), Some(speed.id));
        }
    }

    /// A settings payload built by the lobby in either mode starts the same
    /// game, and every field that cannot be translated falls back to what
    /// Civilization VI would have chosen.
    #[test]
    fn lobby_settings_translate_and_fall_back() {
        let request = Request::from_settings(&serde_json::json!({
            "difficulty": "emperor", "map_script": "pangaea",
            "num_players": 8, "game_speed": "epic",
        }))
        .expect("emperor is a rung");
        assert_eq!(
            request,
            Request {
                difficulty: "DIFFICULTY_EMPEROR",
                map: "Pangaea.lua".into(),
                size: "MAPSIZE_STANDARD",
                speed: "GAMESPEED_EPIC",
                max_turns: 750,
            }
        );
        // A named script wins over a translated one: in this mode the lobby's
        // map control is the other game's list.
        let named = Request::from_settings(&serde_json::json!({
            "difficulty": "deity", "civ6_map": "Tilted_Axis.lua", "map_script": "pangaea",
        }))
        .expect("deity is a rung");
        assert_eq!(named.map, "Tilted_Axis.lua");
        assert_eq!(named.difficulty, "DIFFICULTY_DEITY");
        // Ours-only choices fall back rather than refuse.
        let ours = Request::from_settings(&serde_json::json!({
            "difficulty": "king", "map_script": "grand_canals",
            "num_players": 16, "game_speed": "online",
        }))
        .expect("king is a rung");
        assert_eq!(ours.map, DEFAULT_MAP);
        assert_eq!(ours.size, "MAPSIZE_TINY");
        // A script this game does not register is not passed through.
        let unknown = Request::from_settings(&serde_json::json!({
            "difficulty": "prince", "civ6_map": "Highlands.lua",
        }))
        .expect("prince is a rung");
        assert_eq!(unknown.map, DEFAULT_MAP);
        // The difficulty is the point of the mode, so it is the one field that
        // errors rather than falling back.
        assert!(Request::from_settings(&serde_json::json!({"difficulty": "sandbox"})).is_err());
    }

    /// Every argument the controller is given is one it declares, spelled the
    /// way it declares it. This has to be a test rather than a review: the two
    /// sides are in different languages, so a renamed flag is a runtime failure
    /// three minutes into a launch.
    #[test]
    fn the_command_line_is_one_the_controller_accepts() {
        let request = Request::from_settings(&serde_json::json!({
            "difficulty": "immortal", "civ6_map": "Shuffle.lua",
            "num_players": 6, "game_speed": "quick",
        }))
        .expect("immortal is a rung");
        let argv = request.argv("civvis-20260731T154034Z");
        assert_eq!(
            argv,
            [
                "--tag", "civvis-20260731T154034Z",
                "--difficulty", "DIFFICULTY_IMMORTAL",
                "--ruleset", "RULESET_EXPANSION_2",
                "--map", "Shuffle.lua",
                "--map-size", "MAPSIZE_SMALL",
                "--speed", "GAMESPEED_QUICK",
                "--max-turns", "330",
            ]
        );
    }

    /// The mode is offered on every computer and refused on the ones that
    /// cannot run it, so a probe always answers and never panics — including
    /// on a machine with no game, no tools and no lock.
    #[test]
    fn a_host_always_answers() {
        let host = Host::probe();
        assert_eq!(host.ready(), host.blocked.is_none());
        if host.install.is_none() || host.controller.is_none() {
            assert!(!host.ready(), "a host missing the game cannot be ready");
        }
        // Whatever it says, it is one sentence rather than a stack trace.
        if let Some(blocked) = host.blocked {
            assert!(!blocked.is_empty() && !blocked.contains('\n'));
        }
    }

    /// A tag names the second the run was asked for, which is what the
    /// controller's directory and the game lock's holder are keyed on.
    #[test]
    fn a_tag_is_the_utc_second_it_was_asked_for() {
        let epoch = std::time::UNIX_EPOCH;
        assert_eq!(new_tag(epoch), "civvis-19700101T000000Z");
        let stamp = epoch + std::time::Duration::from_secs(1_785_512_434);
        assert_eq!(new_tag(stamp), "civvis-20260731T154034Z");
        // Leap years and month ends are where a hand-rolled calendar breaks.
        let leap = epoch + std::time::Duration::from_secs(1_709_164_800);
        assert_eq!(new_tag(leap), "civvis-20240229T000000Z");
    }
}
