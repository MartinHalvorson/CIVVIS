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
    MapSpec {
        id: "Continents.lua",
        name: "Continents",
        civvis: Some("continents"),
    },
    MapSpec {
        id: "Fractal.lua",
        name: "Fractal",
        civvis: None,
    },
    MapSpec {
        id: "InlandSea.lua",
        name: "Inland Sea",
        civvis: Some("inland_sea"),
    },
    MapSpec {
        id: "Island_Plates.lua",
        name: "Island Plates",
        civvis: Some("islands"),
    },
    MapSpec {
        id: "Lakes.lua",
        name: "Lakes",
        civvis: Some("lakes"),
    },
    MapSpec {
        id: "Pangaea.lua",
        name: "Pangaea",
        civvis: Some("pangaea"),
    },
    MapSpec {
        id: "Seven_Seas.lua",
        name: "Seven Seas",
        civvis: None,
    },
    MapSpec {
        id: "Shuffle.lua",
        name: "Shuffle",
        civvis: None,
    },
    MapSpec {
        id: "Small_Continents.lua",
        name: "Small Continents",
        civvis: Some("small_continents"),
    },
    MapSpec {
        id: "Terra.lua",
        name: "Terra",
        civvis: None,
    },
    MapSpec {
        id: "Archipelago.lua",
        name: "Archipelago",
        civvis: Some("water_world"),
    },
    MapSpec {
        id: "Continents_Islands.lua",
        name: "Continents & Islands",
        civvis: None,
    },
    MapSpec {
        id: "Primordial.lua",
        name: "Primordial",
        civvis: None,
    },
    MapSpec {
        id: "Splintered_Fractal.lua",
        name: "Splintered Fractal",
        civvis: None,
    },
    MapSpec {
        id: "Tilted_Axis.lua",
        name: "Tilted Axis",
        civvis: None,
    },
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
    DifficultySpec {
        id: "DIFFICULTY_SETTLER",
        name: "Settler",
        civvis: "settler",
    },
    DifficultySpec {
        id: "DIFFICULTY_CHIEFTAIN",
        name: "Chieftain",
        civvis: "chieftain",
    },
    DifficultySpec {
        id: "DIFFICULTY_WARLORD",
        name: "Warlord",
        civvis: "warlord",
    },
    DifficultySpec {
        id: "DIFFICULTY_PRINCE",
        name: "Prince",
        civvis: "prince",
    },
    DifficultySpec {
        id: "DIFFICULTY_KING",
        name: "King",
        civvis: "king",
    },
    DifficultySpec {
        id: "DIFFICULTY_EMPEROR",
        name: "Emperor",
        civvis: "emperor",
    },
    DifficultySpec {
        id: "DIFFICULTY_IMMORTAL",
        name: "Immortal",
        civvis: "immortal",
    },
    DifficultySpec {
        id: "DIFFICULTY_DEITY",
        name: "Deity",
        civvis: "deity",
    },
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
    SizeSpec {
        id: "MAPSIZE_DUEL",
        name: "Duel",
        civvis: "duel",
        players: 2,
    },
    SizeSpec {
        id: "MAPSIZE_TINY",
        name: "Tiny",
        civvis: "tiny",
        players: 4,
    },
    SizeSpec {
        id: "MAPSIZE_SMALL",
        name: "Small",
        civvis: "small",
        players: 6,
    },
    SizeSpec {
        id: "MAPSIZE_STANDARD",
        name: "Standard",
        civvis: "standard",
        players: 8,
    },
    SizeSpec {
        id: "MAPSIZE_LARGE",
        name: "Large",
        civvis: "large",
        players: 10,
    },
    SizeSpec {
        id: "MAPSIZE_HUGE",
        name: "Huge",
        civvis: "huge",
        players: 12,
    },
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
    SpeedSpec {
        id: "GAMESPEED_ONLINE",
        name: "Online",
        civvis: "online",
        turns: 250,
    },
    SpeedSpec {
        id: "GAMESPEED_QUICK",
        name: "Quick",
        civvis: "quick",
        turns: 330,
    },
    SpeedSpec {
        id: "GAMESPEED_STANDARD",
        name: "Standard",
        civvis: "standard",
        turns: 500,
    },
    SpeedSpec {
        id: "GAMESPEED_EPIC",
        name: "Epic",
        civvis: "epic",
        turns: 750,
    },
    SpeedSpec {
        id: "GAMESPEED_MARATHON",
        name: "Marathon",
        civvis: "marathon",
        turns: 1500,
    },
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
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
}

/// The installation root — the directory holding `Civ6.app` — or `None`.
pub fn install_dir() -> Option<PathBuf> {
    if let Some(explicit) = std::env::var_os("CIV6_INSTALL") {
        let path = PathBuf::from(explicit);
        return path.is_dir().then_some(path);
    }
    install_candidates(&home_dir())
        .into_iter()
        .find(|path| path.is_dir())
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
    let mut run = Run {
        tag,
        live,
        ..Run::default()
    };
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
    fs::create_dir_all(&run_dir).map_err(|error| {
        format!(
            "cannot make the run directory {}: {error}",
            run_dir.display()
        )
    })?;
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
    format!("civvis-{}", compact_utc(now))
}

/// The same UTC second as an ISO-8601 instant, for artifacts that record
/// when they were generated (the arena report). Kept beside `new_tag` so the
/// crate's one date conversion stays in one place.
pub fn utc_stamp(now: std::time::SystemTime) -> String {
    let secs = now
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or(0);
    let (year, month, day, hour, minute, second) = civil_from_unix(secs as i64);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

fn compact_utc(now: std::time::SystemTime) -> String {
    let secs = now
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or(0);
    let (year, month, day, hour, minute, second) = civil_from_unix(secs as i64);
    format!("{year:04}{month:02}{day:02}T{hour:02}{minute:02}{second:02}Z")
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
    (
        year,
        m,
        d,
        (rem / 3_600) as u32,
        ((rem % 3_600) / 60) as u32,
        (rem % 60) as u32,
    )
}

// --- the operator's pause -------------------------------------------------

/// Whether this server shows the live verification game, and so may hold it.
///
/// Only the mirror `tools/follow.py` keeps on screen beside Civilization VI
/// passes `--live-pause`. A spectator table, the lobby or the follower's
/// throwaway rebuild server never offers a button that stops a game it is not
/// showing.
static LIVE_PAUSE_CONTROL: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub fn enable_live_pause_control() {
    LIVE_PAUSE_CONTROL.store(true, std::sync::atomic::Ordering::Relaxed);
}

pub fn live_pause_control() -> bool {
    LIVE_PAUSE_CONTROL.load(std::sync::atomic::Ordering::Relaxed)
}

/// The request: present means "hold the live game". `tools/civ6_control/
/// live_pause.py` owns the protocol; these paths and their reading are kept in
/// step with it.
fn live_pause_request_path() -> PathBuf {
    std::env::var_os("CIVVIS_LIVE_PAUSE_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".civvis-live-pause.json"))
}

/// The report `civ6_play.py` writes from the control mod's own events.
fn live_pause_state_path() -> PathBuf {
    std::env::var_os("CIVVIS_LIVE_PAUSE_STATE_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".civvis-live-pause-state.json"))
}

/// How long a hold report is believed on its own. The harness rewrites it
/// every 30 s while the mod holds (`operator_pause_held`); past this, it is
/// believed only while its run still holds the game lock, so a harness that
/// died holding cannot leave the page saying "Paused" over nothing.
const LIVE_PAUSE_REPORT_FRESH: std::time::Duration = std::time::Duration::from_secs(120);

fn read_json_object(path: &Path) -> Option<serde_json::Map<String, Value>> {
    let raw = fs::read_to_string(path).ok()?;
    match serde_json::from_str::<Value>(&raw).ok()? {
        Value::Object(map) => Some(map),
        _ => None,
    }
}

/// The file's existence is the request; only an explicit `"paused": false`
/// inside it reads as "play" — the same reading as `live_pause.requested()`.
fn live_pause_requested_at(path: &Path) -> bool {
    if !path.exists() {
        return false;
    }
    read_json_object(path).is_none_or(|map| map.get("paused") != Some(&Value::Bool(false)))
}

fn live_pause_status_from(
    request_path: &Path,
    report_path: &Path,
    run_holds_game: &dyn Fn(&str) -> bool,
) -> Value {
    let request = read_json_object(request_path).unwrap_or_default();
    let report = read_json_object(report_path).unwrap_or_default();
    let tag = report.get("tag").and_then(Value::as_str);
    let fresh = fs::metadata(report_path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(|age| age <= LIVE_PAUSE_REPORT_FRESH);
    let held = report.get("held") == Some(&Value::Bool(true))
        && (fresh || tag.is_some_and(run_holds_game));
    serde_json::json!({
        "available": live_pause_control(),
        "requested": live_pause_requested_at(request_path),
        "requested_utc": request.get("requested_utc").cloned().unwrap_or(Value::Null),
        "held": held,
        "tag": tag,
        "turn": report.get("turn").cloned().unwrap_or(Value::Null),
        "since_utc": if held {
            report.get("since_utc").cloned().unwrap_or(Value::Null)
        } else {
            Value::Null
        },
    })
}

/// What the page shows: whether a pause is asked for, and whether the game is
/// actually standing still yet (the mod holds on its next tick of our turn).
pub fn live_pause_status() -> Value {
    let mut status = live_pause_status_from(
        &live_pause_request_path(),
        &live_pause_state_path(),
        &|tag| holder().is_some_and(|holder| holder.alive && holder.tag == tag),
    );
    // The New game beside the Pause reads the same live run.
    status["new_game"] = live_new_game_status_at(live_run_dir().as_deref());
    status
}

fn set_live_pause_at(
    path: &Path,
    paused: bool,
    source: &str,
    now: std::time::SystemTime,
) -> Result<(), String> {
    if !paused {
        return match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(format!("could not lift the pause: {error}")),
        };
    }
    let secs = now
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or(0);
    let (year, month, day, hour, minute, second) = civil_from_unix(secs as i64);
    let body = serde_json::json!({
        "paused": true,
        "requested_utc": format!(
            "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"
        ),
        "source": source,
    });
    // Written beside the target and renamed over it: the watchdogs read this
    // file every poll and must never see half of it.
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "civvis-live-pause.json".to_string());
    let staging = path.with_file_name(format!(".{name}.{}.tmp", std::process::id()));
    fs::write(&staging, format!("{body}\n"))
        .and_then(|()| fs::rename(&staging, path))
        .map_err(|error| {
            let _ = fs::remove_file(&staging);
            format!("could not ask for the pause: {error}")
        })
}

/// Ask for, or lift, the hold on the live game. Resuming removes the request.
pub fn set_live_pause(paused: bool, source: &str) -> Result<(), String> {
    set_live_pause_at(
        &live_pause_request_path(),
        paused,
        source,
        std::time::SystemTime::now(),
    )
}

// --- the operator's watch pace --------------------------------------------
//
// On the live mirror the page's Watch pace is the Civilization VI game's own
// per-turn budget (`tools/civ6_control/live_pause.py` `requested_pace_ms`):
// `civ6_play.py` relays it to the control mod, which holds each turn's
// opening until the previous opening is that long ago. Lightning (0) is no
// wait at all, the lane's own speed, and what an unset pace means.

fn live_pace_path() -> PathBuf {
    std::env::var_os("CIVVIS_LIVE_PACE_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".civvis-live-pace.json"))
}

/// The live game's pace, as `live_pause.requested_pace_ms` reads it.
fn live_pace_ms_at(path: &Path) -> u64 {
    read_json_object(path)
        .and_then(|map| map.get("ms").and_then(Value::as_u64))
        .map_or(0, |ms| ms.min(60_000))
}

pub fn live_pace_ms() -> u64 {
    live_pace_ms_at(&live_pace_path())
}

fn set_live_pace_at(path: &Path, ms: u64) -> Result<(), String> {
    let body = serde_json::json!({ "ms": ms.min(60_000) });
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "civvis-live-pace.json".to_string());
    let staging = path.with_file_name(format!(".{name}.{}.tmp", std::process::id()));
    fs::write(&staging, format!("{body}\n"))
        .and_then(|()| fs::rename(&staging, path))
        .map_err(|error| {
            let _ = fs::remove_file(&staging);
            format!("could not set the live game's pace: {error}")
        })
}

/// Hold the live game to a Watch pace; 0 lets it play at full speed.
pub fn set_live_pace(ms: u64) -> Result<(), String> {
    set_live_pace_at(&live_pace_path(), ms)
}

// --- the operator's New game ----------------------------------------------
//
// The page's New game is `civvis-games retire`: the live run is asked to end
// with Civilization VI's own Retire (`tools/civ6_control/operator_retire.py`
// owns that protocol), the ladder row says `operator_retired`, and the lane's
// supervisor starts the next game as it does after any ending. Nothing is
// killed and nothing is launched from here.

/// The reason the run and its ladder row carry, so a census can tell an
/// operator's restart from a lost game.
const LIVE_NEW_GAME_REASON: &str = "operator: new game from the CIVVIS page";

/// The live run's directory: the game lock's holder, while its process lives.
fn live_run_dir() -> Option<PathBuf> {
    let holder = holder().filter(|holder| holder.alive)?;
    let safe = !holder.tag.is_empty()
        && holder
            .tag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        && !holder.tag.starts_with('.');
    safe.then(|| run_root().join(&holder.tag))
}

/// Whether the live run has been asked to retire, and whether the control mod
/// has done it. Read from the sidecars `operator_retire.py` keeps in the run.
fn live_new_game_status_at(run_dir: Option<&Path>) -> Value {
    // `live: false` is the gap between games: the retired run's harness has
    // exited and the next one has not taken the lock yet.
    let Some(run_dir) = run_dir else {
        return serde_json::json!({
            "live": false, "requested": false, "retired": false, "detail": null,
        });
    };
    let request = run_dir.join("operator-retire-request.json").is_file();
    let retired = run_dir.join("operator-retire.json").is_file();
    let detail = read_json_object(&run_dir.join("operator-retire-status.json"))
        .and_then(|note| note.get("detail").cloned())
        .unwrap_or(Value::Null);
    serde_json::json!({
        "live": true,
        "requested": request && !retired,
        "retired": retired,
        "detail": if retired { Value::Null } else { detail },
    })
}

/// What `civvis-games retire` said, as the page's answer. A game already on its
/// way out is what was asked for, not a failure.
fn live_new_game_outcome(succeeded: bool, stdout: &str, stderr: &str) -> Result<(), String> {
    let said = format!("{stdout}\n{stderr}");
    if succeeded
        || said.contains("already has a pending retirement request")
        || said.contains("already has a recorded retirement")
    {
        return Ok(());
    }
    // The retire tool explains itself on stderr; the switch's own refusals
    // (lanes off) print a `==` heading and then the reason on stdout.
    let last_said = |text: &str| {
        text.lines()
            .map(str::trim)
            .rfind(|line| !line.is_empty() && !line.starts_with("=="))
            .map(str::to_string)
    };
    let why = last_said(stderr)
        .or_else(|| last_said(stdout))
        .unwrap_or_else(|| "the lane's switch refused without saying why".to_string());
    let why = why
        .trim_start_matches("retire request refused: ")
        .trim_start_matches("refused: ");
    Err(if why.contains("has no recorded turn") {
        "the game is still being set up; try again once its first turn is on screen".to_string()
    } else if why.contains("no tagged civ6_play harness is active") {
        "no live game is running to replace".to_string()
    } else if why.contains("is already complete") {
        "this game has already ended; the next one is on its way".to_string()
    } else {
        why.to_string()
    })
}

/// Ask Civilization VI to retire the live game so the lane starts a fresh one,
/// and lift any pause: the pause request is host-wide, so standing it would
/// hold the new game at its first turn, and New game means a game that plays.
pub fn request_live_new_game() -> Result<(), String> {
    let tools = tools_dir()
        .ok_or_else(|| "the CIVVIS tools directory is not beside this server".to_string())?;
    // The operator's own switch, not a copy of it: it refuses when the lanes
    // are off (a retired game would get no successor), and its retire tool
    // proves exactly one live game and refuses to touch one still in setup.
    let output = Command::new("/bin/zsh")
        .arg(tools.join("ops").join("civvis-games.sh"))
        .args(["retire", LIVE_NEW_GAME_REASON])
        .env("CIVVIS_RUNS_ROOT", run_root())
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("could not ask for a new game: {error}"))?;
    live_new_game_outcome(
        output.status.success(),
        &String::from_utf8_lossy(&output.stdout),
        &String::from_utf8_lossy(&output.stderr),
    )?;
    set_live_pause(false, "civvis-page")
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
            assert!(
                spec.id.ends_with(".lua"),
                "{} is not a script file",
                spec.id
            );
            assert!(!spec.name.is_empty());
            if let Some(civvis) = spec.civvis {
                assert!(
                    crate::setup::CIV6_MAP_SCRIPTS
                        .iter()
                        .any(|ours| ours.id == civvis),
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
            assert_eq!(
                difficulty_for_civvis(id).map(|found| found.id),
                Some(rung.id)
            );
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
            assert_eq!(
                size_for_players(size.players).map(|found| found.id),
                Some(size.id)
            );
        }
        // Sizes past Huge are ours alone, so they have to fall back.
        assert!(crate::setup::CIV6_MAP_SIZES.len() > SIZES.len());
        assert_eq!(size_for_players(16), None);
        for speed in SPEEDS {
            let ours = crate::setup::GameSpeed::from_id(speed.civvis)
                .unwrap_or_else(|| panic!("{} is not one of our speeds", speed.civvis));
            assert_eq!(speed.turns, ours.turn_limit());
            assert_eq!(
                speed_for_civvis(speed.civvis).map(|found| found.id),
                Some(speed.id)
            );
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
                "--tag",
                "civvis-20260731T154034Z",
                "--difficulty",
                "DIFFICULTY_IMMORTAL",
                "--ruleset",
                "RULESET_EXPANSION_2",
                "--map",
                "Shuffle.lua",
                "--map-size",
                "MAPSIZE_SMALL",
                "--speed",
                "GAMESPEED_QUICK",
                "--max-turns",
                "330",
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

    fn live_pause_scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "civvis-live-pause-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The page's Watch pace is the live game's per-turn budget, read the way
    /// `live_pause.requested_pace_ms` reads it: unset or unreadable is no
    /// wait, and past a minute it is clamped.
    #[test]
    fn the_page_sets_the_live_games_watch_pace() {
        let dir = live_pause_scratch("pace");
        let pace = dir.join("pace.json");
        assert_eq!(live_pace_ms_at(&pace), 0);
        set_live_pace_at(&pace, 2_000).unwrap();
        assert_eq!(live_pace_ms_at(&pace), 2_000);
        let written: Value = serde_json::from_str(&fs::read_to_string(&pace).unwrap()).unwrap();
        assert_eq!(written["ms"], 2_000);
        set_live_pace_at(&pace, 0).unwrap();
        assert_eq!(live_pace_ms_at(&pace), 0);
        set_live_pace_at(&pace, 3_600_000).unwrap();
        assert_eq!(live_pace_ms_at(&pace), 60_000);
        fs::write(&pace, "{torn").unwrap();
        assert_eq!(live_pace_ms_at(&pace), 0);
        // No staging file is left behind beside it.
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
    }

    /// Pause writes the request the harness and every watchdog read; Resume
    /// removes it, and lifting a pause nobody asked for is not an error.
    #[test]
    fn the_page_asks_for_and_lifts_the_live_pause() {
        let dir = live_pause_scratch("request");
        let request = dir.join("pause.json");
        let report = dir.join("state.json");
        let nobody = |_: &str| false;
        assert!(!live_pause_requested_at(&request));
        let at = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_791_864_000);
        set_live_pause_at(&request, true, "civvis-page", at).unwrap();
        assert!(live_pause_requested_at(&request));
        let written: Value = serde_json::from_str(&fs::read_to_string(&request).unwrap()).unwrap();
        assert_eq!(written["paused"], true);
        assert_eq!(written["source"], "civvis-page");
        assert_eq!(written["requested_utc"], "2026-10-13T04:00:00Z");
        let status = live_pause_status_from(&request, &report, &nobody);
        assert_eq!(status["requested"], true);
        assert_eq!(status["requested_utc"], "2026-10-13T04:00:00Z");
        // Asked for is not the same as standing still: no report yet.
        assert_eq!(status["held"], false);
        // No staging file is left behind beside the request.
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        set_live_pause_at(&request, false, "civvis-page", at).unwrap();
        assert!(!request.exists());
        set_live_pause_at(&request, false, "civvis-page", at).unwrap();
        // The same reading as `live_pause.requested()`: an empty hand-written
        // file pauses; only an explicit `false` inside it plays.
        fs::write(&request, "").unwrap();
        assert!(live_pause_requested_at(&request));
        fs::write(&request, r#"{"paused": false}"#).unwrap();
        assert!(!live_pause_requested_at(&request));
        let _ = fs::remove_dir_all(&dir);
    }

    /// "Paused" is said only on the harness's word that the game stopped, and
    /// that word expires with the run that gave it.
    #[test]
    fn the_page_believes_a_hold_only_from_a_live_run() {
        let dir = live_pause_scratch("report");
        let request = dir.join("pause.json");
        let report = dir.join("state.json");
        fs::write(
            &report,
            r#"{"tag": "civvis-20261013T041158Z", "held": true, "turn": 151,
                "since_utc": "2026-10-13T04:20:00Z"}"#,
        )
        .unwrap();
        let nobody = |_: &str| false;
        let status = live_pause_status_from(&request, &report, &nobody);
        assert_eq!(status["held"], true, "a fresh report is believed");
        assert_eq!(status["turn"], 151);
        assert_eq!(status["tag"], "civvis-20261013T041158Z");
        assert_eq!(status["since_utc"], "2026-10-13T04:20:00Z");
        assert_eq!(status["requested"], false, "a resume already asked for");
        // An old report stands only while its run still holds the game.
        let old = std::time::SystemTime::now() - std::time::Duration::from_secs(600);
        fs::File::options()
            .write(true)
            .open(&report)
            .unwrap()
            .set_modified(old)
            .unwrap();
        assert_eq!(
            live_pause_status_from(&request, &report, &nobody)["held"],
            false
        );
        let holds = |tag: &str| tag == "civvis-20261013T041158Z";
        assert_eq!(
            live_pause_status_from(&request, &report, &holds)["held"],
            true
        );
        let other = |tag: &str| tag == "civvis-20261013T050000Z";
        assert_eq!(
            live_pause_status_from(&request, &report, &other)["held"],
            false
        );
        // A released report is not a hold, however fresh.
        fs::write(
            &report,
            r#"{"tag": "civvis-20261013T041158Z", "held": false}"#,
        )
        .unwrap();
        let status = live_pause_status_from(&request, &report, &holds);
        assert_eq!(status["held"], false);
        assert_eq!(status["since_utc"], Value::Null);
        let _ = fs::remove_dir_all(&dir);
    }

    /// New game reads the live run's retire sidecars: asked for until the
    /// control mod has retired the game, and no live run means nothing asked.
    #[test]
    fn the_page_sees_a_new_game_asked_for_and_done() {
        let none = live_new_game_status_at(None);
        assert_eq!(none["live"], false);
        assert_eq!(none["requested"], false);
        assert_eq!(none["retired"], false);
        let dir = live_pause_scratch("new-game");
        let playing = live_new_game_status_at(Some(&dir));
        assert_eq!(playing["live"], true);
        assert_eq!(playing["requested"], false);
        fs::write(
            dir.join("operator-retire-request.json"),
            r#"{"tag": "civvis-20261013T054246Z", "state": "requested"}"#,
        )
        .unwrap();
        fs::write(
            dir.join("operator-retire-status.json"),
            r#"{"state": "pending", "detail": "wrote native retire order; awaiting control-mod acknowledgement"}"#,
        )
        .unwrap();
        let asked = live_new_game_status_at(Some(&dir));
        assert_eq!(asked["requested"], true);
        assert_eq!(asked["retired"], false);
        assert_eq!(
            asked["detail"],
            "wrote native retire order; awaiting control-mod acknowledgement"
        );
        fs::write(dir.join("operator-retire.json"), r#"{"state": "retired"}"#).unwrap();
        let done = live_new_game_status_at(Some(&dir));
        assert_eq!(
            done["requested"], false,
            "a retired game is no longer asked for"
        );
        assert_eq!(done["retired"], true);
        assert_eq!(done["detail"], Value::Null);
        let _ = fs::remove_dir_all(&dir);
    }

    /// What `civvis-games retire` says becomes the page's answer: a game
    /// already on its way out is success, and a refusal says why in words the
    /// operator can act on.
    #[test]
    fn the_page_reads_the_switch_for_a_new_game() {
        assert_eq!(
            live_new_game_outcome(true, "== retiring the active game ==\n", ""),
            Ok(())
        );
        assert_eq!(
            live_new_game_outcome(
                false,
                "== retiring the active game ==\n",
                "retire request refused: run 'civvis-x' already has a pending retirement request\n",
            ),
            Ok(())
        );
        assert_eq!(
            live_new_game_outcome(
                false,
                "== retiring the active game ==\n",
                "retire request refused: run 'civvis-x' has no recorded turn; refusing to touch setup\n",
            ),
            Err("the game is still being set up; try again once its first turn is on screen".into())
        );
        assert_eq!(
            live_new_game_outcome(
                false,
                "== retiring the active game ==\n",
                "retire request refused: no tagged civ6_play harness is active\n",
            ),
            Err("no live game is running to replace".into())
        );
        // The switch's own refusal is on stdout, under its heading.
        assert_eq!(
            live_new_game_outcome(
                false,
                "== retiring the active game ==\n  refused: the game lanes are OFF; use civvis-games on before requesting a replacement\n",
                "",
            ),
            Err("the game lanes are OFF; use civvis-games on before requesting a replacement".into())
        );
        assert_eq!(
            live_new_game_outcome(false, "", ""),
            Err("the lane's switch refused without saying why".into())
        );
    }
}
