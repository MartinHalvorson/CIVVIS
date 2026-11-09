//! Translate a Civilization VI seat's own view of the world into CIVVIS terms.
//!
//! The control mod (`tools/civ6_control/mod/CivvisControlAgent.lua`) exports what
//! one seat can see: its cities and units, the rivals it has met, the cities of
//! those rivals it has actually laid eyes on, and every plot it has revealed.
//! This is the reading half of that bridge — it turns the game's vocabulary into
//! CIVVIS's so a `Game` can be rebuilt and CIVVIS asked what to do.
//!
//! # Only what the seat knows
//!
//! The mod sends an **unrevealed plot as a hole**, not as its true terrain, and
//! a rival city only once `IsRevealed()` is true. That is the whole point: a
//! mirror built from the full map would let the simulator plan with knowledge no
//! human player at that seat could have, and every decision it justified would be
//! worthless as a measurement. [`Snapshot::revealed`] keeps the holes visible
//! rather than filling them in, so a caller cannot accidentally treat unknown
//! ground as ordinary ground.
//!
//! # Names, not indices
//!
//! `plot:GetTerrainType()` returns a row index into the game's own tables, and
//! this vocabulary is keyed by `TERRAIN_`/`FEATURE_`/`RESOURCE_` names. Mapping
//! index to name here would mean guessing that table's ordering, so the mod
//! resolves the name through `GameInfo` before sending it. An unresolvable type
//! arrives as `null` and is reported, never guessed.
//!
//! # Why the vocabulary is embedded
//!
//! `VOCABULARY` is `include_str!`, not a file read. Every cwd-relative asset load
//! in this project has eventually resolved to `None` somewhere that mattered —
//! the champion genome (#469/#471, worth +49 Elo once fixed), the league roster
//! (#490), and the value net, which has *never* loaded in any game because
//! `ValueNet::load` is a single cwd-relative read. A translation table that
//! silently comes back empty would put the simulator on ground that does not
//! exist, which is a worse failure than any of those.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde::Deserialize;

mod air_fog_targets;
mod board;
pub(crate) use board::*;
mod capital_identity;
mod cities;
use cities::*;
mod dedication_history;
mod great_people;
use great_people::*;
mod host_deaths;
mod policy_windows;
pub use policy_windows::StatePolicyChange;
pub mod input_capture;
mod religion_state;
mod research_tree;
pub use research_tree::StateResearchTreeRow;
mod rivals;
pub(crate) use rivals::*;
mod seat;
pub(crate) use seat::*;
mod settlement_loyalty;
mod spy_posting;
mod state;
pub use state::*;
mod strategic_income;
pub use host_deaths::HostUnitDeath;
pub use spy_posting::{StateSpyCity, StateSpyTarget};
mod world;
use world::*;

use crate::{
    name::Name,
    setup::{GameSpeed, MapScript},
};

/// Built by `python3 tools/civ6_control/vocab.py --json` from two authorities:
/// Civilization VI's own `DebugGameplay.sqlite` and CIVVIS's `data/*.json`.
const VOCABULARY: &str = include_str!("../tools/civ6_control/vocab.json");

#[derive(Debug, Clone, Deserialize)]
struct TerrainEntry {
    terrain: String,
    hills: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct VocabularyFile {
    terrains: BTreeMap<String, TerrainEntry>,
    features: BTreeMap<String, String>,
    resources: BTreeMap<String, String>,
    /// Types deliberately not modelled, kept so a caller can tell "we chose not
    /// to map this" from "we failed to". Currently only `RESOURCE_LEY_LINE`, a
    /// Secret Societies mode marker with no yields.
    #[serde(default)]
    excluded: BTreeMap<String, String>,
}

/// Civilization VI's type names, resolved to CIVVIS's.
pub struct Vocabulary {
    file: VocabularyFile,
}

/// What a name resolved to, including the two "no" answers stated explicitly.
#[derive(Debug, Clone, PartialEq)]
pub enum Resolved<T> {
    /// A CIVVIS equivalent.
    Known(T),
    /// Deliberately not modelled, with the reason.
    Excluded(String),
    /// Not in the vocabulary at all. A caller must decide what to do; it must
    /// not quietly become a default terrain.
    Unknown(String),
}

impl Vocabulary {
    /// The embedded table. Panics only if the committed JSON stopped parsing,
    /// which is a build mistake rather than a runtime condition.
    pub fn embedded() -> &'static Vocabulary {
        use std::sync::OnceLock;
        static PARSED: OnceLock<Vocabulary> = OnceLock::new();
        PARSED.get_or_init(|| Vocabulary {
            file: serde_json::from_str(VOCABULARY)
                .expect("tools/civ6_control/vocab.json is committed and must parse"),
        })
    }

    /// `TERRAIN_GRASS_HILLS` -> (`grassland`, hills = true).
    ///
    /// Civilization VI encodes elevation *in* the terrain and CIVVIS carries it
    /// separately, so this returns both halves.
    pub fn terrain(&self, civ6: &str) -> Resolved<(Name, bool)> {
        match self.file.terrains.get(civ6) {
            Some(entry) => Resolved::Known((Name::new(&entry.terrain), entry.hills)),
            None => self.miss(civ6),
        }
    }

    pub fn feature(&self, civ6: &str) -> Resolved<Name> {
        match self.file.features.get(civ6) {
            Some(name) => Resolved::Known(Name::new(name)),
            None => self.miss(civ6),
        }
    }

    pub fn resource(&self, civ6: &str) -> Resolved<Name> {
        match self.file.resources.get(civ6) {
            Some(name) => Resolved::Known(Name::new(name)),
            None => self.miss(civ6),
        }
    }

    fn miss<T>(&self, civ6: &str) -> Resolved<T> {
        match self.file.excluded.get(civ6) {
            Some(why) => Resolved::Excluded(why.clone()),
            None => Resolved::Unknown(civ6.to_string()),
        }
    }

    pub fn terrain_count(&self) -> usize {
        self.file.terrains.len()
    }
    pub fn feature_count(&self) -> usize {
        self.file.features.len()
    }
    pub fn resource_count(&self) -> usize {
        self.file.resources.len()
    }
}

/// One revealed plot, exactly as the mod emits it.
#[derive(Debug, Clone, Deserialize)]
pub struct Plot {
    pub x: i32,
    pub y: i32,
    /// Terrain type name. Absent only if the game could not resolve it.
    #[serde(default)]
    pub t: Option<String>,
    #[serde(default)]
    pub f: Option<String>,
    #[serde(default)]
    pub r: Option<String>,
    /// Owning player, or -1 for nobody.
    #[serde(default = "minus_one")]
    pub o: i32,
    /// Native purchase city for our plots or observed parent of a foreign district.
    /// Absent in legacy exports, ordinary foreign ground, or unavailable observations.
    #[serde(default)]
    pub oc: Option<i64>,
    #[serde(default)]
    pub w: bool,
    /// Native lake identity (`Plot:IsLake`); terrain alone names lakes as coast.
    /// Absent on legacy exports or failed reads.
    #[serde(default)]
    pub lk: Option<bool>,
    #[serde(default)]
    pub i: bool,
    #[serde(default)]
    pub fw: Option<bool>,
    /// Improvement type name already built here, e.g. `IMPROVEMENT_FARM`.
    #[serde(default)]
    pub im: Option<String>,
    /// Native improvement owner, independent of territorial owner `o`.
    /// Missing or negative means no ownership observation, including legacy
    /// exports. Translate through the host-player map before assigning a seat.
    #[serde(default)]
    pub io: Option<i32>,
    /// Whether the improvement or observed district is pillaged. Improvements
    /// use `Plot:IsImprovementPillaged`; districts use `District:IsPillaged`.
    /// Absent reads as not pillaged, preserving older exports.
    /// A pillaged improvement pays nothing until
    /// repaired, and without this bit the model paid it in full — a pastured
    /// Horses tile read at the bare-terrain figure for ninety turns on run
    /// civvis-20260816T040537Z.
    #[serde(default)]
    pub p: bool,
    /// District type standing here (`Plot:GetDistrictType`), any owner, e.g.
    /// `DISTRICT_CAMPUS`; `DISTRICT_CITY_CENTER` on a centre, `DISTRICT_WONDER`
    /// under a wonder. Read for rival and city-state cities, whose records carry
    /// no districts; our own come from the city record. Absent on older exports
    /// and on empty ground.
    #[serde(default)]
    pub d: Option<String>,
    /// Whether that district is COMPLETE (`CityManager.GetDistrictAt(x, y):IsComplete()`).
    /// `GetDistrictType` names a district from the turn it is placed, and a placed
    /// district is not adjacent to anything until it is built (Puteoli's Commercial
    /// Hub read +2 beside a placed Campus for eleven turns and +3 the turn it
    /// completed, run civvis-20260816T223457Z t108-119). `None` on an older export
    /// or where the district object could not be read — treated as complete, which
    /// is what every earlier export meant.
    #[serde(default)]
    pub dc: Option<bool>,
    /// Last observed defense pools of a completed defending district. The mod
    /// refreshes these only in sight and retains them in fog. Missing is unknown,
    /// distinct from a measured zero; city-center health uses `StateCity`.
    #[serde(default)]
    pub dh: Option<PlotDistrictHealth>,
    /// Wonder type standing here (`Plot:GetWonderType`), any owner, e.g.
    /// `BUILDING_PYRAMIDS`.
    #[serde(default)]
    pub wo: Option<String>,
    /// This plot's own river edges as a bitmask: 1 = W, 2 = NW, 4 = NE.
    ///
    /// Civilization VI records a river on three of a plot's six edges; the other
    /// three are the same segments held by the neighbouring plots. See
    /// [`apply_rivers`].
    #[serde(default)]
    pub rv: u8,
    /// Whether any of the six edges carries a river.
    ///
    /// Not derivable from `rv`: a river along only this plot's E, SE or SW edge
    /// lives on the neighbour's flags, so `rv` is 0 while the plot is riverside.
    #[serde(default)]
    pub ri: bool,
    /// Continent type name, e.g. `CONTINENT_AFRICA`. Absent on water and on any
    /// plot whose continent does not resolve.
    #[serde(default)]
    pub ct: Option<String>,
    /// Gathering Storm coastal-lowland band (1–3 metres); -1 or 0 for ground that
    /// sea-level rise cannot reach.
    #[serde(default = "minus_one")]
    pub cl: i32,
    /// Exact Gathering Storm tile state (`TerrainManager.IsFlooded` /
    /// `IsSubmerged`). Unknown on legacy exports or failed native reads.
    /// Observations override phase-derived flooding after climate is applied.
    #[serde(default)]
    pub flooded: Option<bool>,
    #[serde(default)]
    pub submerged: Option<bool>,
    /// Appeal as the host counts it (`Plot:GetAppeal`, the shipped
    /// PlotToolTip's read). The board derives appeal from its own six
    /// neighbours and cannot see a wonder's +2 in fog, a Governor's promotion
    /// or a rival's district; Neighborhood housing, Seaside and Ski Resorts
    /// and National Parks are all priced on it. `None` on an older export or
    /// a failed read, where [`crate::game::Game::tile_appeal`] keeps its own
    /// derivation. Carried onto the board by [`apply_landmass`].
    #[serde(default)]
    pub ap: Option<i32>,
    /// Whether this plot is inside a National Park (`Plot:IsNationalPark`).
    /// A park is not an improvement in Civilization VI and IS one on this
    /// board, so the flag lands as the `national_park` improvement.
    #[serde(default)]
    pub np: bool,
    /// Whether this plot is in the seat's sight NOW
    /// (`PlayersVisibility[pid]:IsVisible`), not merely revealed once. Joins
    /// [`crate::game::Game::host_observed`], which the mirrored seat's vision
    /// frame unions in; absent reads as fog, which is what every earlier
    /// export meant.
    #[serde(default)]
    pub vis: bool,
    /// Route standing here (`Plot:GetRouteType`), e.g. `ROUTE_ANCIENT_ROAD`.
    /// Roads were never exported and the mirror wrote `tile.road = 0`
    /// everywhere, so every march was priced across roadless ground. Absent
    /// on an older export and where no route stands.
    #[serde(default)]
    pub rt: Option<String>,
    /// Whether that route is pillaged (`Plot:IsRoutePillaged`); a pillaged
    /// road pays no movement.
    #[serde(default)]
    pub rp: bool,
    /// ★★★★★ THE HOST'S OWN SIX YIELDS FOR THIS PLOT, in the order Food,
    /// Production, Gold, Science, Culture, Faith (`Plot:GetYield`).
    ///
    /// Everything else on this record names a ROW OF THE RULESET, and CIVVIS
    /// re-derives what the plot pays from its own catalogue. That derivation is
    /// short by every term the ground holds and no row names, and the largest
    /// of them is the permanent fertility Gathering Storm's disasters leave
    /// behind: `RandomEvent_Yields` grants an affected plot +1 Food and +1
    /// Production (and Science on the two worst eruptions) with per-severity
    /// odds up to 75%, stored on the plot and reachable through no other
    /// accessor. Volcanic Soil has no `Feature_YieldChanges` row at all, so a
    /// mirror reading the feature name alone sees bare Grassland where the game
    /// shows 3 Food 3 Production — which is exactly what the operator reported
    /// on the live board.
    ///
    /// Land plots only, absent where every yield is zero or the read failed,
    /// and absent on every export older than this field. See
    /// [`apply_observed_plot_yields`] for how it reaches the board, and
    /// `plotYieldTuple` in `CivvisControlAgent.lua` for how it is gathered.
    #[serde(default)]
    pub yl: Option<Vec<f64>>,
}

/// Raw district damage and capacities from the shipped city-banner accessors.
#[derive(Debug, Clone, Deserialize)]
pub struct PlotDistrictHealth {
    #[serde(default = "unknown_damage")]
    pub damage: i32,
    #[serde(default = "unknown_damage")]
    pub max_damage: i32,
    #[serde(default = "unknown_damage")]
    pub wall_damage: i32,
    #[serde(default = "unknown_damage")]
    pub max_wall_damage: i32,
}

impl Plot {
    /// The exported yields as the engine's own record, or `None` when the plot
    /// carries no reading. A tuple of the wrong length is no reading: an export
    /// that cannot spell all six is not one to correct a tile against.
    pub fn host_yields(&self) -> Option<crate::rules::Yields> {
        let values = self.yl.as_ref()?;
        if values.len() != 6 || !values.iter().all(|value| value.is_finite()) {
            return None;
        }
        Some(crate::rules::Yields {
            food: values[0],
            production: values[1],
            gold: values[2],
            science: values[3],
            culture: values[4],
            faith: values[5],
        })
    }
}

fn minus_one() -> i32 {
    -1
}

/// The engine's route ladder for a host route name: 0 none, 1 Ancient,
/// 2 Medieval, 3 Industrial, 4 Modern, 5 Railroad (`world.rs`). A route the
/// host names and this ladder does not know reads as the Ancient road — the
/// honest floor for "there is a road here".
pub fn route_level(name: Option<&str>, pillaged: bool) -> u8 {
    let Some(name) = name.map(str::trim).filter(|name| !name.is_empty()) else {
        return 0;
    };
    if pillaged {
        return 0;
    }
    match name {
        "ROUTE_ANCIENT_ROAD" => 1,
        "ROUTE_MEDIEVAL_ROAD" => 2,
        "ROUTE_INDUSTRIAL_ROAD" => 3,
        "ROUTE_MODERN_ROAD" => 4,
        "ROUTE_RAILROAD" => 5,
        _ => 1,
    }
}

/// A `tiles` event: one chunk of the revealed map.
#[derive(Debug, Clone, Deserialize)]
pub struct TilesChunk {
    pub turn: u32,
    pub width: i32,
    pub height: i32,
    #[serde(default)]
    pub chunk: u32,
    #[serde(default)]
    pub plots: Vec<Plot>,
}

/// The one stamp a between-sweeps `tiles` delta carries (`CivvisTiles.sweep`:
/// only the plots revealed or changed hands since the last board went out).
/// Read beside [`TilesChunk`] rather than on it so the chunk's many literal
/// constructions stay as they are.
#[derive(Deserialize)]
struct TilesDeltaStamp {
    #[serde(default)]
    delta: bool,
}

/// Which board a `tiles` line belongs to, read beside [`TilesChunk`] for the
/// same reason as [`TilesDeltaStamp`]: the opening sweep carries no `frame`
/// (it is frame 0), a mid-turn delta carries the frame it was swept on.
#[derive(Deserialize)]
struct TilesBoardStamp {
    turn: u32,
    #[serde(default)]
    frame: u32,
}

/// The seat's view of the world at one turn, assembled from its `tiles` chunks.
///
/// Deliberately not a `Game`: the holes have to survive into whatever consumes
/// this, because a `Game` has no place to put "I do not know".
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub turn: u32,
    pub width: i32,
    pub height: i32,
    revealed: BTreeMap<(i32, i32), Plot>,
    /// The turn each plot's record was written. `revealed` accumulates and
    /// never forgets, so a plot last exported forty turns ago still sits in it
    /// looking exactly like one exported this turn — fine for terrain, which
    /// does not change, and wrong for anything read as the host's CURRENT
    /// answer. [`apply_observed_plot_yields`] uses this to refuse a stale
    /// reading rather than pay a tile what it paid before the sweep that
    /// dropped it.
    stamped: BTreeMap<(i32, i32), u32>,
    settlement_loyalty: Option<settlement_loyalty::Observation>,
}

impl Snapshot {
    /// Assemble chunks into one view. Later chunks win, so a re-export of the
    /// same turn refreshes rather than duplicates.
    pub fn from_chunks(chunks: &[TilesChunk]) -> Snapshot {
        let mut snapshot = Snapshot::default();
        for chunk in chunks {
            snapshot.merge_sweep(chunk);
        }
        snapshot
    }

    /// Merge one chunk of a full sweep: its plots land and its turn advances
    /// the snapshot's sweep turn.
    pub fn merge_sweep(&mut self, chunk: &TilesChunk) {
        self.turn = self.turn.max(chunk.turn);
        self.merge_delta(chunk);
    }

    /// Whether this seat has revealed a plot. Everything outside this is unknown
    /// ground and must never be treated as ordinary ground.
    pub fn is_revealed(&self, pos: (i32, i32)) -> bool {
        self.revealed.contains_key(&pos)
    }

    /// Merge a between-sweeps delta: its plots land like any chunk's, but
    /// the snapshot's sweep turn stays where the last FULL sweep put it. The
    /// `improved` fold (`apply_finished_improvements`) keeps events at or
    /// after the newest sweep, and a delta carries none of the older plots'
    /// improvements — letting it stand for a sweep would drop every
    /// improvement finished since the real one.
    pub fn merge_delta(&mut self, chunk: &TilesChunk) {
        self.width = self.width.max(chunk.width);
        self.height = self.height.max(chunk.height);
        for plot in &chunk.plots {
            self.revealed.insert((plot.x, plot.y), plot.clone());
            self.stamped.insert((plot.x, plot.y), chunk.turn);
        }
    }

    /// Whether this plot's record is the CURRENT one: written by the newest
    /// full sweep, or by a delta since. A plot whose stamp predates the newest
    /// sweep was missed by it — a truncated chunk, a dropped line — and what it
    /// says about the host right now is a guess.
    pub fn is_current(&self, pos: (i32, i32)) -> bool {
        self.stamped
            .get(&pos)
            .is_some_and(|stamp| *stamp >= self.turn)
    }

    pub fn plot(&self, pos: (i32, i32)) -> Option<&Plot> {
        self.revealed.get(&pos)
    }

    /// Record a finished improvement (an `improved` event) on an already revealed
    /// plot. Returns false when the plot is not revealed; the event says nothing
    /// about terrain, so it cannot reveal one.
    pub fn set_improvement(&mut self, pos: (i32, i32), im: &str) -> bool {
        match self.revealed.get_mut(&pos) {
            Some(plot) => {
                if plot.im.as_deref() != Some(im) {
                    plot.io = None;
                }
                plot.im = Some(im.to_string());
                // ⚠ AND THE PLOT'S YIELD READING GOES WITH IT. The tuple was
                // measured before this improvement existed; keeping it would
                // make the host-to-model correction below cancel the very Farm
                // that was just finished, because the model has it and the
                // reading does not. The next sweep brings a matching pair.
                plot.yl = None;
                true
            }
            None => false,
        }
    }

    /// Every plot this seat has revealed, in offset coordinates.
    ///
    /// ⚠ Offset, like everything the mod emits — the caller converts. Handing out
    /// axial here would put a coordinate conversion inside a getter, which is the
    /// shape of the bug that cost an hour once already.
    pub fn revealed_positions(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        self.revealed.keys().copied()
    }

    pub fn revealed_count(&self) -> usize {
        self.revealed.len()
    }

    /// How much of the world this seat has seen. A settle ranking computed at 4%
    /// revealed is a different kind of claim from one at 90%, and a caller that
    /// cannot see the difference will overtrust the first.
    pub fn revealed_fraction(&self) -> f64 {
        let total = (self.width as i64) * (self.height as i64);
        if total <= 0 {
            return 0.0;
        }
        self.revealed.len() as f64 / total as f64
    }

    /// Every type name in this snapshot that the vocabulary cannot place.
    ///
    /// Returned rather than logged: a caller deciding whether to trust a ranking
    /// needs to know the map contained ground the translator did not understand.
    pub fn untranslatable(&self, vocab: &Vocabulary) -> Vec<String> {
        let mut misses = std::collections::BTreeSet::new();
        for plot in self.revealed.values() {
            if let Some(t) = &plot.t {
                if let Resolved::Unknown(name) = vocab.terrain(t) {
                    misses.insert(name);
                }
            }
            if let Some(f) = &plot.f {
                if let Resolved::Unknown(name) = vocab.feature(f) {
                    misses.insert(name);
                }
            }
            if let Some(r) = &plot.r {
                if let Resolved::Unknown(name) = vocab.resource(r) {
                    misses.insert(name);
                }
            }
        }
        misses.into_iter().collect()
    }
}

/// Read every `tiles` chunk out of a run's `events.jsonl`.
///
/// When a run has state events, the snapshot is cut at the stream position of
/// the selected state. A state and a later mid-turn tile delta carry the same
/// turn number but describe different boards; pairing the state with that
/// later delta makes a replay judge one frame against another. Runs without
/// state events retain the old whole-stream behaviour.
pub fn snapshot_from_events(path: &std::path::Path) -> std::io::Result<Snapshot> {
    snapshot_from_events_at(path, None)
}

/// Read the explored map as it existed at the selected state, never from its
/// future. If the run has no matching state event, this falls back to the
/// historical turn-only boundary.
pub fn snapshot_from_events_at(
    path: &std::path::Path,
    turn: Option<u32>,
) -> std::io::Result<Snapshot> {
    let raw = read_events(path)?;
    let selected = latest_state_range(&raw, turn);
    let state_start = selected.map(|((start, _), _)| start);
    // Historical native logs publish state before its terrain. Current opening
    // exports publish terrain first so the Python worker cannot wake on a
    // half-published board. Keep support for both layouts: pre-state chunks
    // belong to the selected prefix; post-state chunks belong only when they
    // match its turn and do not exceed its frame.
    //
    // The post-state rule fixed old logs whose turn-1 terrain otherwise vanished
    // (six native runs on 2026-09-01 opened with no map and zero orders). It is
    // still needed for historical replay, while a later frame's delta stays out
    // (`snapshot_stops_at_the_selected_state_before_a_later_mid_turn_delta`).
    let board = selected.map(|(_, board)| board);
    // In stream order, so a later chunk's plot wins whichever kind it is;
    // a delta (`CivvisTiles.sweep`) merges without standing for a sweep —
    // see `Snapshot::merge_delta`.
    let ranges = line_ranges_containing(&raw, "\"tiles\"");
    // Up to the selected state record every chunk counts (subject to the turn
    // limit); after it, only the same board's. The first part is the same
    // prefix for every later request of a growing log, so it is merged once
    // and kept (`SnapshotPrefix`), and each request clones it and merges only
    // what is new. Chunks merge in stream order either way.
    let split =
        ranges.partition_point(|(start, _)| state_start.is_none_or(|limit| *start <= limit));
    let (head, tail) = ranges.split_at(split);
    let generation = events_generation(path, &raw);
    let mut snapshot = Snapshot::default();
    let mut done = 0;
    let mut max_turn = 0;
    if let Some(generation) = generation {
        SNAPSHOT_PREFIX.with(|cell| {
            if let Some(kept) = cell.borrow().as_ref() {
                let last_kept_start = kept.covered.checked_sub(1);
                if kept.path == path
                    && kept.generation == generation
                    && turn.is_none_or(|limit| kept.max_turn <= limit)
                    && last_kept_start
                        .is_none_or(|last| state_start.is_none_or(|limit| last <= limit))
                {
                    done = head.partition_point(|(start, _)| *start < kept.covered);
                    snapshot = kept.snapshot.clone();
                    max_turn = kept.max_turn;
                }
            }
        });
    }
    let mut keep = generation.is_some();
    for &(start, end) in &head[done..] {
        if let Ok(chunk) = serde_json::from_str::<TilesChunk>(&raw[start..end]) {
            if !chunk.plots.is_empty() {
                if turn.is_none_or(|limit| chunk.turn <= limit) {
                    merge_chunk(&mut snapshot, &raw[start..end], &chunk);
                    max_turn = max_turn.max(chunk.turn);
                } else {
                    // A later request with a higher limit would merge this
                    // chunk here; never keep a prefix that skipped it.
                    keep = false;
                }
            }
        }
    }
    if keep {
        if let (Some(generation), Some(&(last, _))) = (generation, head.last()) {
            SNAPSHOT_PREFIX.with(|cell| {
                *cell.borrow_mut() = Some(SnapshotPrefix {
                    path: path.to_path_buf(),
                    generation,
                    covered: last + 1,
                    max_turn,
                    snapshot: snapshot.clone(),
                })
            });
        }
    }
    for &(start, end) in tail {
        let line = &raw[start..end];
        let same_board = board.is_some_and(|(board_turn, board_frame)| {
            serde_json::from_str::<TilesBoardStamp>(line)
                .is_ok_and(|stamp| stamp.turn == board_turn && stamp.frame <= board_frame)
        });
        if !same_board {
            continue;
        }
        if let Ok(chunk) = serde_json::from_str::<TilesChunk>(line) {
            if !chunk.plots.is_empty() && turn.is_none_or(|limit| chunk.turn <= limit) {
                merge_chunk(&mut snapshot, line, &chunk);
            }
        }
    }
    apply_finished_improvements(&raw, turn, state_start, &mut snapshot);
    snapshot.settlement_loyalty = settlement_loyalty::read(&raw, state_start, board);
    Ok(snapshot)
}

/// The merged tiles up to some `tiles` line of the log `read_events` holds:
/// every chunk starting before byte `covered` has been merged, in order.
struct SnapshotPrefix {
    path: std::path::PathBuf,
    generation: u64,
    covered: usize,
    /// The highest chunk turn merged; a request limited below it cannot reuse it.
    max_turn: u32,
    snapshot: Snapshot,
}

/// Merge one parsed `tiles` chunk as a sweep, or as a delta when its line says so.
fn merge_chunk(snapshot: &mut Snapshot, line: &str, chunk: &TilesChunk) {
    let is_delta = serde_json::from_str::<TilesDeltaStamp>(line).is_ok_and(|stamp| stamp.delta);
    if is_delta {
        snapshot.merge_delta(chunk);
    } else {
        snapshot.merge_sweep(chunk);
    }
}

// The live reader requests one turn, but late-game logs contain hundreds of
// large state records. Inspect only the root turn before constructing their
// units, cities, schema-gap values, and cloned JSON trees. Ambiguous headers
// still reach the full parser: in particular it accepts duplicate keys via
// Value's last-key-wins semantics, while this small typed header rejects them.
fn state_line_can_match_turn(line: &str, turn: Option<u32>) -> bool {
    let Some(want) = turn else {
        return true;
    };
    // ★★★★ SKIP THE SERDE WALK FOR EVERY OTHER TURN'S BOARD.
    //
    // The typed header still has to step over the whole record to reach
    // `turn` — keys are written sorted, so it sits behind `units`, `tiles`,
    // `players` and the rest — and a live seat asks for one turn out of
    // hundreds of 100-300 KB state records. Profiled on a t220 frame of
    // civvis-20261003T164758Z (96 MB events.jsonl): this header walk was 18% of
    // the decider's CPU, run twice per frame over all ~690 state records.
    // A record that spells an integer `"turn"` member and never `"turn": <want>`
    // is another turn's board and is rejected without the walk. Anything the
    // scan cannot settle — no integer `turn` at all, as in the ambiguous
    // headers below — still goes through the exact parse, as before.
    if !turn_may_match(line, want) {
        return false;
    }
    #[derive(Deserialize)]
    struct TurnHeader {
        turn: Option<u32>,
    }
    serde_json::from_str::<TurnHeader>(line)
        .ok()
        .and_then(|header| header.turn)
        .is_none_or(|found| found == want)
}

/// Reject an old state's lexical turn hints before the unchanged exact
/// classifier/header parser. Hints belong only to the raw log currently held
/// by EventsRead; a retained older buffer uses the original path.
fn state_line_can_match_turn_in_log(raw: &str, range: (usize, usize), turn: Option<u32>) -> bool {
    let Some(want) = turn else {
        return true;
    };
    let wanted = want.to_string();
    let possible = EVENTS_READ.with(|cell| {
        let mut slot = cell.borrow_mut();
        let read = slot.as_mut()?;
        if !std::ptr::eq(read.text.as_ptr(), raw.as_ptr()) || read.text.len() != raw.len() {
            return None;
        }
        let hints = read
            .turn_hints
            .entry(range)
            .or_insert_with(|| integer_turn_ranges(&raw[range.0..range.1], range.0));
        Some(
            hints.is_empty()
                || hints
                    .iter()
                    .any(|&(start, end)| &raw.as_bytes()[start..end] == wanted.as_bytes()),
        )
    });
    if possible == Some(false) {
        return false;
    }
    state_line_can_match_turn(&raw[range.0..range.1], turn)
}

/// The digit ranges the existing turn_may_be_any scan treats as integer
/// members. Keep its lexical rules exactly: nesting/duplicate keys are not
/// interpreted, and ambiguous input still reaches the original parser.
fn integer_turn_ranges(line: &str, base: usize) -> Vec<(usize, usize)> {
    let bytes = line.as_bytes();
    let key = "\"turn\"";
    let finder = memchr::memmem::Finder::new(key.as_bytes());
    let mut from = 0;
    let mut out = Vec::new();
    while let Some(offset) = finder.find(&bytes[from..]) {
        let found = from + offset;
        from = found + 1;
        let mut at = found + key.len();
        while at < bytes.len() && matches!(bytes[at], b' ' | b'\t' | b'\n' | b'\r') {
            at += 1;
        }
        if at >= bytes.len() || bytes[at] != b':' {
            continue;
        }
        at += 1;
        while at < bytes.len() && matches!(bytes[at], b' ' | b'\t' | b'\n' | b'\r') {
            at += 1;
        }
        let digits = bytes[at..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count();
        if digits == 0 {
            continue;
        }
        let after = bytes.get(at + digits).copied();
        if after.is_some_and(|b| matches!(b, b'.' | b'e' | b'E')) {
            continue;
        }
        out.push((base + at, base + at + digits));
    }
    out
}

/// False only when `line` spells at least one integer `"turn"` member and none
/// of them is `want`: a record that cannot be turn `want`'s board. True when
/// the scan cannot tell (no integer `turn` anywhere), so the exact parse still
/// owns missing, null, string, escaped and duplicate keys. Nesting is ignored.
fn turn_may_match(line: &str, want: u32) -> bool {
    turn_may_be_any(line, &[want])
}

/// [`turn_may_match`] for a set of turns: false only when `line` spells at
/// least one integer `"turn"` member and none of them is in `turns`. A record
/// whose top-level `turn` reads as one of `turns` (`as_u64`, or a `u32` field)
/// must spell it, so a reader that keeps only such records may skip the parse
/// of everything this rejects.
pub fn turn_may_be_any(line: &str, turns: &[u32]) -> bool {
    let bytes = line.as_bytes();
    let key = "\"turn\"";
    let wanted: Vec<String> = turns.iter().map(|turn| turn.to_string()).collect();
    let mut saw_integer = false;
    // Every occurrence, overlapping included, as the `str::find` walk it
    // replaces found them; memmem is the SIMD searcher (std's is scalar here).
    let finder = memchr::memmem::Finder::new(key.as_bytes());
    let mut from = 0;
    while let Some(offset) = finder.find(&bytes[from..]) {
        let found = from + offset;
        from = found + 1;
        let mut at = found + key.len();
        while at < bytes.len() && matches!(bytes[at], b' ' | b'\t' | b'\n' | b'\r') {
            at += 1;
        }
        if at >= bytes.len() || bytes[at] != b':' {
            continue;
        }
        at += 1;
        while at < bytes.len() && matches!(bytes[at], b' ' | b'\t' | b'\n' | b'\r') {
            at += 1;
        }
        let digits = bytes[at..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count();
        if digits == 0 {
            continue;
        }
        let after = bytes.get(at + digits).copied();
        if after.is_some_and(|b| matches!(b, b'.' | b'e' | b'E')) {
            continue;
        }
        if wanted
            .iter()
            .any(|want| &bytes[at..at + digits] == want.as_bytes())
        {
            return true;
        }
        saw_integer = true;
    }
    !saw_integer
}

// ★★★★ ONE READ PER CHANGE OF THE LOG, AND NO LINE-BY-LINE WALK FOR A RARE EVENT.
//
// A live decider frame ran a dozen refusal readers, and each re-read the whole
// `events.jsonl` (100 MB by turn 200) and split it into lines to test each one
// for its event name. After the prefilters above that was still ~35% of a
// t220 frame: 80% of it the line walk, 20% the read. `read_events` keeps the
// log per thread and, while the file only grows, reads just the appended
// bytes; `lines_containing` hands each reader only the lines that mention its
// event, found by testing 64 KB blocks at once. Both answer exactly what the
// direct read and `raw.lines().filter(|l| l.contains(needle))` answered.
thread_local! {
    static EVENTS_READ: std::cell::RefCell<Option<EventsRead>> = const { std::cell::RefCell::new(None) };
    static EVENTS_GENERATION: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static SNAPSHOT_PREFIX: std::cell::RefCell<Option<SnapshotPrefix>> = const { std::cell::RefCell::new(None) };
}

struct EventsRead {
    path: std::path::PathBuf,
    text: std::rc::Rc<String>,
    /// Bumped on every full read; the same while the log only grows, so
    /// anything derived from a prefix of the text stays valid under it.
    generation: u64,
    /// Per needle: the byte ranges of the lines that contain it, and how far
    /// into `text` they were looked for. Kept while the log only grows.
    hits: std::collections::HashMap<String, NeedleHits>,
    /// Lexical digit offsets, keyed by the enclosing state-line range.
    /// Prefix offsets survive append-only growth; a full read discards them.
    turn_hints: std::collections::HashMap<(usize, usize), Vec<(usize, usize)>>,
    /// Exact combat-kind predicate per line; follows the same retained prefix.
    combat_hints: std::collections::HashMap<(usize, usize), bool>,
}

struct NeedleHits {
    scanned: usize,
    lines: Vec<(usize, usize)>,
}

/// The run log at `path`, read whole — from a per-thread copy when the file has
/// only grown since (its old tail is re-read and must match byte for byte), so
/// a persistent reader pays for the appended bytes alone. A shrunk, replaced or
/// unreadable file is read afresh. Bytes after the last newline are a line still
/// being written; they are left for the next read on the incremental path.
pub fn read_events(path: &std::path::Path) -> std::io::Result<std::rc::Rc<String>> {
    let result = read_events_uncaptured(path);
    input_capture::observe(path, &result);
    result
}

fn read_events_uncaptured(path: &std::path::Path) -> std::io::Result<std::rc::Rc<String>> {
    use std::io::{Read, Seek, SeekFrom};
    const TAIL: u64 = 4096;
    let len = std::fs::metadata(path)?.len();
    let cached = EVENTS_READ.with(|cell| {
        cell.borrow()
            .as_ref()
            .filter(|read| read.path == path)
            .map(|read| read.text.clone())
    });
    if let Some(text) = cached {
        let have = text.len() as u64;
        if len == have {
            // Same size: unchanged only if the tail still matches.
            let mut file = std::fs::File::open(path)?;
            let from = have.saturating_sub(TAIL);
            file.seek(SeekFrom::Start(from))?;
            let mut tail = Vec::with_capacity((have - from) as usize);
            file.by_ref().take(have - from).read_to_end(&mut tail)?;
            if tail == text.as_bytes()[from as usize..] {
                return Ok(text);
            }
        } else if len > have && text.ends_with('\n') {
            let mut file = std::fs::File::open(path)?;
            let from = have.saturating_sub(TAIL);
            file.seek(SeekFrom::Start(from))?;
            let mut bytes = Vec::with_capacity((len - from) as usize);
            file.by_ref().take(len - from).read_to_end(&mut bytes)?;
            let split = (have - from) as usize;
            if bytes.len() >= split && bytes[..split] == text.as_bytes()[from as usize..] {
                let added = &bytes[split..];
                let keep = added.iter().rposition(|b| *b == b'\n').map_or(0, |i| i + 1);
                if let Ok(more) = std::str::from_utf8(&added[..keep]) {
                    let mut grown = String::with_capacity(text.len() + more.len());
                    grown.push_str(&text);
                    grown.push_str(more);
                    let grown = std::rc::Rc::new(grown);
                    EVENTS_READ.with(|cell| {
                        let mut slot = cell.borrow_mut();
                        // The old text is a prefix of the new one, so every
                        // line found in it is still where it was.
                        let (hits, turn_hints, combat_hints, generation) = slot
                            .take()
                            .map(|read| {
                                (
                                    read.hits,
                                    read.turn_hints,
                                    read.combat_hints,
                                    read.generation,
                                )
                            })
                            .unwrap_or_default();
                        *slot = Some(EventsRead {
                            path: path.to_path_buf(),
                            text: grown.clone(),
                            generation,
                            hits,
                            turn_hints,
                            combat_hints,
                        })
                    });
                    return Ok(grown);
                }
            }
        }
    }
    let text = std::rc::Rc::new(std::fs::read_to_string(path)?);
    let generation = EVENTS_GENERATION.with(|counter| {
        counter.set(counter.get() + 1);
        counter.get()
    });
    EVENTS_READ.with(|cell| {
        *cell.borrow_mut() = Some(EventsRead {
            path: path.to_path_buf(),
            text: text.clone(),
            generation,
            hits: Default::default(),
            turn_hints: Default::default(),
            combat_hints: Default::default(),
        })
    });
    Ok(text)
}

/// The generation of the log `read_events` holds for `path`, when `raw` is
/// that very text; None otherwise.
fn events_generation(path: &std::path::Path, raw: &str) -> Option<u64> {
    EVENTS_READ.with(|cell| {
        cell.borrow().as_ref().and_then(|read| {
            (read.path == path
                && std::ptr::eq(read.text.as_ptr(), raw.as_ptr())
                && read.text.len() == raw.len())
            .then_some(read.generation)
        })
    })
}

/// `raw.lines().filter(|line| line.contains(needle))`, in order, each line
/// once. Found with `memchr::memmem` over the buffer, cutting the enclosing
/// line around each hit; when `raw` is the log `read_events` is holding, the
/// lines found are kept and only bytes appended since are searched again, so a
/// persistent decider pays for each line once in the life of the run rather
/// than once per reader per frame. `str::lines` strips a `\r` only before a
/// `\n`; so does this.
pub(crate) fn lines_containing<'a>(raw: &'a str, needle: &str) -> std::vec::IntoIter<&'a str> {
    line_ranges_containing(raw, needle)
        .into_iter()
        .map(|(start, end)| &raw[start..end])
        .collect::<Vec<_>>()
        .into_iter()
}

/// [`lines_containing`] as byte ranges into `raw`, for readers that also need
/// where a line sits (its order against a selected state record).
pub fn line_ranges_containing(raw: &str, needle: &str) -> Vec<(usize, usize)> {
    EVENTS_READ
        .with(|cell| {
            let mut slot = cell.borrow_mut();
            let read = slot.as_mut()?;
            if !std::ptr::eq(read.text.as_ptr(), raw.as_ptr()) || read.text.len() != raw.len() {
                return None;
            }
            let found = read
                .hits
                .entry(needle.to_string())
                .or_insert_with(|| NeedleHits {
                    scanned: 0,
                    lines: Vec::new(),
                });
            if found.scanned < raw.len() {
                found.scanned = line_hits(raw, found.scanned, needle, &mut found.lines);
            }
            Some(found.lines.clone())
        })
        .unwrap_or_else(|| {
            let mut lines = Vec::new();
            line_hits(raw, 0, needle, &mut lines);
            lines
        })
}

/// Append to `out` the byte range of every line of `raw[from..]` containing
/// `needle` (`from` is the start of a line); returns how far it looked.
fn line_hits(raw: &str, from: usize, needle: &str, out: &mut Vec<(usize, usize)>) -> usize {
    let bytes = raw.as_bytes();
    if needle.is_empty() {
        // Every line; `str::lines` yields no empty line after a final newline.
        let mut start = from;
        for line in raw[from..].split_inclusive('\n') {
            let mut end = start + line.len();
            if line.ends_with('\n') {
                end -= 1;
                if bytes[start..end].ends_with(b"\r") {
                    end -= 1;
                }
            }
            out.push((start, end));
            start += line.len();
        }
        return bytes.len();
    }
    let finder = memchr::memmem::Finder::new(needle.as_bytes());
    let mut at = from;
    while at <= bytes.len() {
        let Some(offset) = finder.find(&bytes[at..]) else {
            break;
        };
        let hit = at + offset;
        let start = memchr::memrchr(b'\n', &bytes[..hit]).map_or(0, |i| i + 1);
        let newline = memchr::memchr(b'\n', &bytes[hit..]).map(|i| hit + i);
        let mut end = newline.unwrap_or(bytes.len());
        at = end + 1;
        if newline.is_some() && bytes[start..end].ends_with(b"\r") {
            end -= 1;
            if !raw[start..end].contains(needle) {
                continue;
            }
        }
        out.push((start, end));
    }
    bytes.len()
}

/// Whether `line` could hold `<key>: <literal>` as one JSON member.
///
/// A cheap NECESSARY condition for a serde check that follows it, never a
/// replacement: every occurrence of `key` (quoted, e.g. `"\"kind\""`) is
/// tested for optional JSON whitespace, a colon, optional whitespace and then
/// `literal` (`"\"combat\""`, or a number's digits), and a numeric literal
/// must not run on into more of a number. Nesting is ignored, so a nested
/// member also passes — the caller's parse decides. What it buys: a substring
/// such as `"combat"` also occurs in every state record (unit strength keys),
/// and a 300 KB record is not parsed to learn that it is not a combat event.
pub(crate) fn json_may_have_value(line: &str, key: &str, literal: &str) -> bool {
    let bytes = line.as_bytes();
    let numeric = literal
        .as_bytes()
        .last()
        .is_some_and(|b| b.is_ascii_digit());
    // Every occurrence, overlapping included, as the `str::find` walk it
    // replaces found them; memmem is the SIMD searcher (std's is scalar here).
    let finder = memchr::memmem::Finder::new(key.as_bytes());
    let mut from = 0;
    while let Some(offset) = finder.find(&bytes[from..]) {
        let found = from + offset;
        from = found + 1;
        let mut at = found + key.len();
        while at < bytes.len() && matches!(bytes[at], b' ' | b'\t' | b'\n' | b'\r') {
            at += 1;
        }
        if at >= bytes.len() || bytes[at] != b':' {
            continue;
        }
        at += 1;
        while at < bytes.len() && matches!(bytes[at], b' ' | b'\t' | b'\n' | b'\r') {
            at += 1;
        }
        if !bytes[at..].starts_with(literal.as_bytes()) {
            continue;
        }
        let after = bytes.get(at + literal.len()).copied();
        if numeric && after.is_some_and(|b| b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E')) {
            continue;
        }
        return true;
    }
    false
}

#[cfg(test)]
mod log_scan_prefilter_tests {
    use super::{json_may_have_value, state_line_can_match_turn, turn_may_match};

    #[test]
    fn a_kind_matches_with_or_without_spaces() {
        assert!(json_may_have_value(
            r#"{"kind": "combat", "turn": 3}"#,
            "\"kind\"",
            "\"combat\""
        ));
        assert!(json_may_have_value(
            r#"{"kind":"combat"}"#,
            "\"kind\"",
            "\"combat\""
        ));
        assert!(json_may_have_value(
            "{\"kind\" :\t\"combat\"}",
            "\"kind\"",
            "\"combat\""
        ));
    }

    #[test]
    fn the_word_alone_is_not_the_kind() {
        // Every exported unit carries a `combat` strength; a state record must
        // not look like a combat event to the prefilter.
        let state =
            r#"{"kind": "state", "turn": 9, "units": [{"combat": 20, "kind": "UNIT_WARRIOR"}]}"#;
        assert!(!json_may_have_value(state, "\"kind\"", "\"combat\""));
        assert!(!json_may_have_value(
            r#"{"label": "kind", "x": "combat"}"#,
            "\"kind\"",
            "\"combat\""
        ));
    }

    #[test]
    fn a_turn_number_must_end_where_it_ends() {
        let line = r#"{"kind": "state", "turn": 220, "utc": "x"}"#;
        assert!(json_may_have_value(line, "\"turn\"", "220"));
        assert!(!json_may_have_value(line, "\"turn\"", "22"));
        assert!(!json_may_have_value(line, "\"turn\"", "2"));
        assert!(!json_may_have_value(r#"{"turn": 2201}"#, "\"turn\"", "220"));
        assert!(!json_may_have_value(
            r#"{"turn": 220.5}"#,
            "\"turn\"",
            "220"
        ));
        assert!(json_may_have_value(r#"{"turn":220}"#, "\"turn\"", "220"));
        // A nested member passes: the exact parse behind the prefilter decides.
        assert!(json_may_have_value(
            r#"{"turn": 5, "deal": {"turn": 220}}"#,
            "\"turn\"",
            "220"
        ));
    }

    #[test]
    fn the_turn_prefilter_only_rejects_what_the_header_would() {
        let other = r#"{"kind": "state", "turn": 219, "units": []}"#;
        let ours = r#"{"kind": "state", "turn": 220, "units": []}"#;
        let nested = r#"{"kind": "state", "deals": [{"turn": 220}], "turn": 219}"#;
        assert!(!state_line_can_match_turn(other, Some(220)));
        assert!(state_line_can_match_turn(ours, Some(220)));
        // Passes the prefilter, then the exact header rejects it.
        assert!(!state_line_can_match_turn(nested, Some(220)));
        assert!(state_line_can_match_turn(other, None));
    }

    #[test]
    fn block_search_yields_exactly_the_filtered_lines() {
        use super::lines_containing;
        // Several 64 KB blocks, a needle at a block edge, CRLF endings, a long
        // line spanning blocks, a repeated needle on one line, no final newline.
        let mut raw = String::new();
        for i in 0..6000 {
            match i % 997 {
                0 => raw.push_str(&format!("{{\"kind\": \"build_no_plot\", \"i\": {i}}}\r\n")),
                1 => raw.push_str(&format!(
                    "{}build_no_plot build_no_plot\n",
                    "x".repeat(70_000)
                )),
                _ => raw.push_str(&format!("{{\"kind\": \"await\", \"polls\": {i}}}\n")),
            }
        }
        raw.push_str("tail build_no_plot");
        for needle in ["build_no_plot", "await", "never there", "\"i\": 1994}"] {
            let want: Vec<&str> = raw.lines().filter(|line| line.contains(needle)).collect();
            let got: Vec<&str> = lines_containing(&raw, needle).collect();
            assert_eq!(got, want, "{needle}");
        }
        assert_eq!(lines_containing("", "x").count(), 0);
    }

    #[test]
    fn the_kept_lines_of_a_growing_log_match_a_fresh_filter() {
        use super::{lines_containing, read_events};
        let dir = std::env::temp_dir().join(format!("civvis-line-hits-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("events.jsonl");
        let check = |needle: &str| {
            let text = read_events(&path).unwrap();
            let want: Vec<&str> = text.lines().filter(|l| l.contains(needle)).collect();
            let got: Vec<&str> = lines_containing(&text, needle).collect();
            assert_eq!(got, want, "{needle}");
        };
        std::fs::write(&path, "a build_no_plot\nb\nc build_no_plot\r\n").unwrap();
        check("build_no_plot");
        check("b");
        // Appended lines are searched; kept ones are not searched again.
        std::fs::write(
            &path,
            "a build_no_plot\nb\nc build_no_plot\r\nd\ne build_no_plot\n",
        )
        .unwrap();
        check("build_no_plot");
        check("b");
        check("");
        // A replaced file drops what was kept.
        std::fs::write(&path, "zz build_no_plot\n").unwrap();
        check("build_no_plot");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_kept_tile_prefix_answers_like_a_cold_read() {
        use super::{snapshot_from_events_at, EVENTS_READ, SNAPSHOT_PREFIX};
        let forget = || {
            EVENTS_READ.with(|cell| *cell.borrow_mut() = None);
            SNAPSHOT_PREFIX.with(|cell| *cell.borrow_mut() = None);
        };
        let dir = std::env::temp_dir().join(format!("civvis-tile-prefix-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("events.jsonl");
        let tiles = |turn: u32, frame: u32, delta: bool, x: i32| {
            format!(
                r#"{{"kind": "tiles", "turn": {turn}, "frame": {frame}, "delta": {delta}, "width": 8, "height": 8, "plots": [{{"x": {x}, "y": 1, "t": "TERRAIN_GRASS"}}]}}"#
            )
        };
        let state = |turn: u32, frame: u32| {
            format!(r#"{{"kind": "state", "turn": {turn}, "frame": {frame}}}"#)
        };
        let mut log = vec![
            state(1, 0),
            tiles(1, 0, false, 1),
            state(2, 0),
            tiles(2, 0, true, 2),
        ];
        let check = |log: &Vec<String>, turn: u32| {
            std::fs::write(&path, log.join("\n") + "\n").unwrap();
            let warm = format!("{:?}", snapshot_from_events_at(&path, Some(turn)).unwrap());
            forget();
            let cold = format!("{:?}", snapshot_from_events_at(&path, Some(turn)).unwrap());
            assert_eq!(warm, cold, "turn {turn} after {} lines", log.len());
            // Warm the caches again the way a persistent decider holds them.
            let _ = snapshot_from_events_at(&path, Some(turn));
        };
        check(&log, 2);
        log.extend([state(2, 1), tiles(2, 1, true, 3)]);
        check(&log, 2);
        log.extend([state(3, 0), tiles(3, 0, false, 4), tiles(3, 0, true, 5)]);
        check(&log, 3);
        // A lower limit after a higher one must not reuse what it cannot see.
        check(&log, 2);
        check(&log, 3);
        log.extend([
            state(4, 0),
            tiles(4, 0, true, 6),
            state(4, 1),
            tiles(4, 1, true, 7),
        ]);
        check(&log, 4);
        forget();
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_cached_log_reads_like_a_fresh_one() {
        use super::read_events;
        let dir = std::env::temp_dir().join(format!("civvis-read-events-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("events.jsonl");
        let fresh = |p: &std::path::Path| std::fs::read_to_string(p).unwrap();
        std::fs::write(&path, "a\n").unwrap();
        assert_eq!(*read_events(&path).unwrap(), fresh(&path));
        // Appended whole lines: only the new bytes are read, same answer.
        std::fs::write(&path, "a\nb\nc\n").unwrap();
        assert_eq!(*read_events(&path).unwrap(), fresh(&path));
        // A line still being written stays out until its newline lands.
        std::fs::write(&path, "a\nb\nc\nd").unwrap();
        assert_eq!(*read_events(&path).unwrap(), "a\nb\nc\n");
        std::fs::write(&path, "a\nb\nc\nd\n").unwrap();
        assert_eq!(*read_events(&path).unwrap(), fresh(&path));
        // Replaced at the same length, shrunk, or replaced longer: read afresh.
        std::fs::write(&path, "A\nB\nC\nD\n").unwrap();
        assert_eq!(*read_events(&path).unwrap(), fresh(&path));
        std::fs::write(&path, "z\n").unwrap();
        assert_eq!(*read_events(&path).unwrap(), fresh(&path));
        std::fs::write(&path, "y\nlonger than before\n").unwrap();
        assert_eq!(*read_events(&path).unwrap(), fresh(&path));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn what_the_scan_cannot_settle_still_reaches_the_parser() {
        for line in [
            r#"{"kind":"state"}"#,
            r#"{"kind":"state","turn":null}"#,
            r#"{"kind":"state","turn":"7"}"#,
            r#"{"kind":"state","turn":7.0}"#,
            r#"{"kind":"state","tu\u0072n":7}"#,
        ] {
            assert!(turn_may_match(line, 7), "{line}");
        }
        assert!(!turn_may_match(r#"{"kind":"state","turn":8}"#, 7));
        assert!(!turn_may_match(r#"{"kind":"state","turn":70}"#, 7));
        assert!(turn_may_match(r#"{"kind":"state","turn":8,"turn":7}"#, 7));
        use super::turn_may_be_any;
        assert!(turn_may_be_any(r#"{"kind":"combat","turn":12}"#, &[11, 12]));
        assert!(!turn_may_be_any(
            r#"{"kind":"combat","turn":13}"#,
            &[11, 12]
        ));
        assert!(turn_may_be_any(r#"{"kind":"combat","turn":-3}"#, &[11, 12]));
        assert!(!turn_may_be_any(
            r#"{"kind":"combat","turn":18446744073709551616}"#,
            &[11]
        ));
    }
}

/// The line at which [`state_from_events`] selects its state.
///
/// State selection is newest-wins for a turn, and highest-turn-wins when no
/// turn was requested. Keeping the same rule here makes a snapshot and a state
/// share one exact point in the append-only event stream.
#[cfg(test)]
fn latest_state_line(raw: &str, turn: Option<u32>) -> Option<usize> {
    latest_state_range(raw, turn).map(|((start, _), _)| raw[..start].matches('\n').count())
}

/// The byte range of the state record [`state_from_events`] selects, and its
/// `(turn, frame)`; found among the lines naming `"state"` only.
fn latest_state_range(raw: &str, turn: Option<u32>) -> Option<((usize, usize), (u32, u32))> {
    if turn.is_none() {
        return latest_state_range_bounded(raw);
    }
    latest_state_range_scan(raw, turn)
}

/// The largest integer `"turn"` member `line` spells that fits a `u32`, read
/// with [`integer_turn_ranges`]' lexical rules, and whether any spelled member
/// was larger than that.
///
/// A record's top-level `turn`, whenever a parse reads it as an integer, is one
/// of these members -- the same premise [`state_line_can_match_turn`] already
/// rejects other turns' boards on (keys are written by `json.dumps`, never
/// escaped). So the first value is an upper bound on the turn a parse of the
/// line can return.
fn turn_upper_bound(line: &str) -> (Option<u32>, bool) {
    let mut bound: Option<u32> = None;
    let mut overflow = false;
    for (start, end) in integer_turn_ranges(line, 0) {
        match line[start..end].parse::<u32>() {
            Ok(value) => bound = Some(bound.map_or(value, |had| had.max(value))),
            // All digits, so the only failure is a value past `u32`.
            Err(_) => overflow = true,
        }
    }
    (bound, overflow)
}

/// Candidate state lines, most promising first: the highest turn bound, and on
/// a tie the later line (the newest board wins a tie).
fn by_turn_bound_newest_first(order: &mut [(u32, (usize, usize))]) {
    order.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(b.1 .0.cmp(&a.1 .0)));
}

/// ★★★ THE NEWEST BOARD WITHOUT PARSING EVERY OLDER ONE.
///
/// With no turn requested, [`latest_state_range_scan`] ran the full typed
/// parse ([`state_from_json`]: a `Value`, a clone, a struct) of EVERY state
/// record in the log to keep the last one with the highest turn. The live
/// mirror's throwaway rebuild (`civvis play --mirror`, started by
/// `tools/follow.py` for every published turn) has no warm cache, so that walk
/// grew with the game: 62 MB and 718 records by turn 263 of a Standard game
/// (civvis-20261104T010447Z), and the board reached the page 0.27 s after Civ
/// VI exported it at turn 50 but 0.86 s at turn 250. The end-turn gate
/// (#4246) waits for the page, so late turns waited on it.
///
/// The selection is unchanged: the last record of the highest turn. Records
/// are tried in [`by_turn_bound_newest_first`] order and the walk stops once
/// no remaining record can beat the best -- a bound below its turn, or the
/// same bound on an earlier line. A typed state's `turn` is a `u32` the line
/// spells, so a line that spells none cannot be one.
fn latest_state_range_bounded(raw: &str) -> Option<((usize, usize), (u32, u32))> {
    let mut order = Vec::new();
    for (start, end) in line_ranges_containing(raw, "\"state\"") {
        if let (Some(bound), _) = turn_upper_bound(&raw[start..end]) {
            order.push((bound, (start, end)));
        }
    }
    by_turn_bound_newest_first(&mut order);
    let mut best: Option<(u32, (usize, usize), u32)> = None;
    for (bound, range) in order {
        if let Some((turn, at, _)) = best {
            if bound < turn || (bound == turn && range.0 < at.0) {
                break;
            }
        }
        let Ok(state) = state_from_json(&raw[range.0..range.1]) else {
            continue;
        };
        if state.turn > bound {
            // The premise above failed (an escaped key?); trust the walk.
            return latest_state_range_scan(raw, None);
        }
        let newer = best.is_none_or(|(turn, at, _)| {
            state.turn > turn || (state.turn == turn && range.0 > at.0)
        });
        if newer {
            best = Some((state.turn, range, state.frame));
        }
    }
    best.map(|(turn, range, frame)| (range, (turn, frame)))
}

/// Every state record in turn, in log order: the reference selection, and the
/// path for a requested turn (whose lexical hints already skip other boards).
fn latest_state_range_scan(raw: &str, turn: Option<u32>) -> Option<((usize, usize), (u32, u32))> {
    let mut best: Option<(u32, (usize, usize), u32)> = None;
    for (start, end) in line_ranges_containing(raw, "\"state\"") {
        let line = &raw[start..end];
        if !state_line_can_match_turn_in_log(raw, (start, end), turn) {
            continue;
        }
        let Ok(state) = state_from_json(line) else {
            continue;
        };
        if turn.is_some_and(|want| state.turn != want) {
            continue;
        }
        if best
            .as_ref()
            .map(|(best_turn, _, _)| state.turn >= *best_turn)
            .unwrap_or(true)
        {
            best = Some((state.turn, (start, end), state.frame));
        }
    }
    best.map(|(turn, range, frame)| (range, (turn, frame)))
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct NativeBaseKey {
    players: usize,
    width: i32,
    height: i32,
    seed: u64,
    city_states: usize,
}

thread_local! {
    // Only the untouched constructor result is retained. Every observed board
    // starts from its own clone, and a different setup replaces this one slot.
    static NATIVE_BASE_GAME: std::cell::RefCell<Option<(NativeBaseKey, crate::game::Game)>> =
        const { std::cell::RefCell::new(None) };
}

fn native_base_game(
    players: usize,
    width: i32,
    height: i32,
    seed: u64,
    city_states: usize,
) -> crate::game::Game {
    let key = NativeBaseKey {
        players: players.max(2),
        width: width.max(1),
        height: height.max(1),
        seed,
        city_states,
    };
    NATIVE_BASE_GAME.with(|cached| {
        let mut cached = cached.borrow_mut();
        if let Some((previous, game)) = cached.as_ref() {
            if *previous == key {
                return game.clone_for_native_rebuild();
            }
        }
        let mut game = crate::game::Game::new(
            key.players,
            key.width,
            key.height,
            key.seed,
            500,
            key.city_states,
        );
        Arc::make_mut(&mut game.rules).enable_unknown_terrain();
        let result = game.clone_for_native_rebuild();
        *cached = Some((key, game));
        result
    })
}

/// Rebuild a CIVVIS `Game` whose map is the ground this seat has actually seen.
///
/// ⚠⚠ CIVILIZATION VI EXPORTS **OFFSET** COORDINATES; CIVVIS STORES **AXIAL**.
///
/// They are not the same space, and the numbers look interchangeable, which is why
/// this went unnoticed. A 60x38 CIVVIS map holds 2280 tiles keyed from `(-18, 36)`
/// to `(59, 0)` — negative columns — because `q = col - (row - (row & 1)) / 2`.
/// Writing a Civ 6 plot straight in therefore lands on a different hex or on no hex
/// at all: the capital of a real run at offset (56, 28) had NO TILE in the
/// reconstruction, so nothing was written, no city could be placed, and
/// `civvis-advise` reported "no legal revealed site" while blaming the map.
///
/// `hex::offset_to_axial` is CIVVIS's own conversion and is used rather than a
/// reimplementation. The [`Snapshot`] keeps Civ 6's offset coordinates because that
/// is what the export says and what the operator sees on screen; conversion happens
/// here, at the boundary, once.
///
/// A generated map arrives full of terrain the seat has never laid eyes on. Left
/// alone, that terrain is a map generator's invention presented as knowledge, and
/// anything reading tile yields — `settle_value` reads them directly, with no
/// visibility filter, because CIVVIS stores no per-player revealed map — would be
/// planning on ground that does not exist. Every undisclosed coordinate is therefore
/// rewritten to the zero-yield, impassable `unknown` terrain. [`grow_frontier`] may
/// separately mark a bounded ring as provisionally traversable; that planning prior
/// never turns the underlying tile into invented land or water.
pub fn rebuild_game(snapshot: &Snapshot, players: usize, seed: u64) -> crate::game::Game {
    rebuild_game_with_city_states(snapshot, players, seed, 0)
}

fn rebuild_game_with_city_states(
    snapshot: &Snapshot,
    players: usize,
    seed: u64,
    city_states: usize,
) -> crate::game::Game {
    let mut game = native_base_game(players, snapshot.width, snapshot.height, seed, city_states);

    apply_terrain(&mut game, snapshot);
    game
}

/// Re-take the seat's city memory from the board as it stands now.
///
/// ⚠ The rebuild takes the memory (`city_memory`, Board phase) BEFORE the
/// host's city facts land (`capital_identity::apply`, inside
/// `observed_host_metrics`). Every revealed rival city is planted through
/// `Game::found_city_for`, which crowns a player's first planted city its
/// capital and makes its current owner its founder; the host's
/// `original_capital` and `original_owner` correct the board afterwards, but
/// the memory keeps the planting's guess. The planning board
/// (`Game::player_decision_view`) draws every fogged rival city from that
/// memory, and `find-the-capital` counts a remembered capital as seen. The
/// live brain rebuilds the board every frame (`--fresh-board`), so the guess
/// was there on every decision. Called by the decider under
/// `unseen-capital-is-unseen`; the rebuild itself is unchanged.
pub fn refresh_city_memory(game: &mut crate::game::Game) {
    apply_city_memory(game);
}

// ============================================================ the live empire
//
// ★★★★★ WHY THIS EXISTS. `rebuild_game` gives CIVVIS terrain, but no UNITS and
// no RIVAL — so CIVVIS could not answer the two questions that
// decide a game on Settler: whom to fight, and where to send the army. Those were
// answered instead by hand-written Lua, which is how a veto comparing SCORE ratios
// forbade every war for 190 turns while nineteen units stood on ALERT.
//
// ⚠ Civilization VI speaks OFFSET, CIVVIS stores AXIAL. Every crossing here goes
// through `hex::offset_to_axial`, because mixing them is silent: a capital at
// offset (56,28) landed on NO TILE and the ranker then blamed the map.

/// Whether a `hostiles[]` entry is a Free Cities unit: flagged `free` by a current
/// mod, or carrying the `player` of the exported Free Cities minor on an older one.
/// Everything else on that list is a barbarian.
fn hostile_is_free_cities(state: &StateSnapshot, unit: &StateUnit) -> bool {
    unit.free
        || (unit.player >= 0
            && state
                .minors
                .iter()
                .any(|minor| minor.is_free_cities() && minor.player as i64 == unit.player))
}

/// Whether the Free Cities actor's own `units[]` already carries this hostile: the
/// mod exports a met Free Cities player's visible units under `minors[]` as well as
/// under `hostiles[]`, and one unit is planted once, from the actor's record.
fn hostile_exported_as_minor_unit(state: &StateSnapshot, unit: &StateUnit) -> bool {
    unit.id > 0
        && state
            .minors
            .iter()
            .filter(|minor| minor.is_free_cities())
            .any(|minor| minor.units.iter().any(|exported| exported.id == unit.id))
}

/// Pair exported non-major actors with the matching CIVVIS seats.
///
/// Firaxis includes the aggregate Free Cities player in `GetAliveMinors()` even
/// on turn 1, when it owns nothing. Treating that placeholder as the first real
/// city-state turned CIVVIS's generated Kabul seat into an enemy and put the
/// planner into conquest before the capital was founded. A present Free Cities
/// actor uses the dedicated dormant seat; only actual city-states consume the
/// generated city-state roster.
/// Which exported minor stands on which mirrored seat: met city-states take
/// the board's city-state seats in export order, the present Free Cities actor
/// takes the Free Cities seat. Public so the order bridge can resolve a
/// city-state seat back to Firaxis's player id by the SAME rule the board was
/// built with (see `civvis_orders::host_minor_target`), instead of by a city
/// plot the fog may not have revealed yet.
pub fn minor_actor_assignments<'a>(
    game: &crate::game::Game,
    state: &'a StateSnapshot,
) -> Vec<(&'a StateMinor, usize)> {
    let mut city_state_seats = game
        .players
        .iter()
        .filter(|player| player.is_minor && !player.is_barbarian && !player.is_free_city)
        .map(|player| player.id);
    let free_city_seat = game
        .players
        .iter()
        .find(|player| player.is_free_city)
        .map(|player| player.id);
    let mut out = Vec::new();
    for minor in &state.minors {
        if minor.is_city_state() {
            if let Some(seat) = city_state_seats.next() {
                out.push((minor, seat));
            }
        } else if minor.is_present_free_cities(state) {
            if let Some(seat) = free_city_seat {
                out.push((minor, seat));
            }
        }
    }
    out
}

/// Count the major seats at the front of a mirrored game's player roster.
///
/// `Game::new` appends city-states, Free Cities and Barbarians after the major
/// seats, so this remains stable even when the number of minor actors changes.
pub fn modeled_major_player_count(game: &crate::game::Game) -> usize {
    game.players
        .iter()
        .take_while(|player| !player.is_minor)
        .count()
}

/// Map Civilization VI's major-player ids to CIVVIS's model seats.
///
/// The host's `rivals` array is met-gated: a civilization the seat has not met
/// is absent, so its position in that array is not a stable player id. When the
/// `seat.players` roster size is available, CIVVIS keeps the host ids in their
/// original order and only rotates the local host id into model seat zero.
/// That makes a later sighting of player 3 land on the same seat even if the
/// earlier export contained players 1, 2, 4 and 5 only.
///
/// `model_major_seats` is the number of major slots actually present on the
/// board. It is zero for callers that do not have a `Game`; in that case the
/// host's roster size is used. Older direct snapshots may have neither roster
/// size nor a merged `seat` event, so those retain the historical compact
/// export-order fallback rather than guessing at unseen seats.
pub fn host_major_seat_map(
    state: &StateSnapshot,
    model_major_seats: usize,
) -> BTreeMap<usize, usize> {
    let configured = state.seat.players > 0;
    if configured {
        let major_seats = if model_major_seats > 0 {
            model_major_seats
        } else {
            state.seat.players
        }
        .max(2);
        let local_host = usize::try_from(state.seat.local_player)
            .ok()
            .filter(|host| *host < major_seats)
            .unwrap_or(0);
        let mut seats = BTreeMap::new();
        seats.insert(local_host, 0);
        let mut model_seat = 1;
        for host in 0..major_seats {
            if host == local_host {
                continue;
            }
            seats.insert(host, model_seat);
            model_seat += 1;
        }
        return seats;
    }

    let local_host = usize::try_from(state.seat.local_player).unwrap_or(0);
    let mut seats = BTreeMap::new();
    seats.insert(local_host, 0);
    for (index, rival) in state.rivals.iter().enumerate() {
        let model_seat = index + 1;
        if model_major_seats > 0 && model_seat >= model_major_seats {
            break;
        }
        seats.insert(rival.player, model_seat);
    }
    seats
}

/// Find a met major by its CIVVIS seat, using the same mapping as the mirror.
///
/// This is public because the order bridge must name the original Firaxis
/// player id when an action targets a model seat; looking up `rivals[seat - 1]`
/// would send an order to the wrong empire as soon as a met-gated entry is
/// missing.
pub fn host_rival_for_seat(
    state: &StateSnapshot,
    seat: usize,
    model_major_seats: usize,
) -> Option<&StateRival> {
    let seats = host_major_seat_map(state, model_major_seats);
    state
        .rivals
        .iter()
        .find(|rival| seats.get(&rival.player).copied() == Some(seat))
}

/// Resolve a city-state through its exported capital name before its type id.
///
/// Firaxis keeps legacy type ids after renaming actors: the final-patch row for
/// `CIVILIZATION_JAKARTA`, for example, is displayed and founded as Bandar
/// Brunei. The city name is already in the state export and is the identity the
/// player sees, so it is a stronger key than surgery on the implementation id.
fn mirrored_city_state_name(game: &crate::game::Game, minor: &StateMinor) -> Option<String> {
    let visible_name = minor
        .cities
        .iter()
        .find(|city| city.capital)
        .or_else(|| minor.cities.first())
        .map(|city| city.name.trim())
        .filter(|name| !name.is_empty());
    let type_name = minor
        .civ
        .trim()
        .strip_prefix("CIVILIZATION_")
        .unwrap_or(minor.civ.trim())
        .replace('_', " ");
    game.rules
        .city_states
        .roster
        .iter()
        .find(|spec| {
            visible_name.is_some_and(|name| spec.name.eq_ignore_ascii_case(name))
                || spec.name.eq_ignore_ascii_case(&type_name)
        })
        .map(|spec| spec.name.clone())
}

/// Traders that Firaxis says are already servicing a route.
///
/// This remains separate from `Game::routes`: an international destination can
/// be outside the currently retained city memory, but that still never makes the
/// Trader idle or available for a second route.
pub fn active_trade_route_traders(state: &StateSnapshot) -> std::collections::BTreeSet<i64> {
    state
        .trade_routes
        .iter()
        .filter_map(|route| (route.trader >= 0).then_some(route.trader))
        .collect()
}

/// Put the host's active routes into the CIVVIS economic model.
///
/// CIVVIS normally creates this state by consuming a Trader.  The host keeps the
/// physical unit visible while it travels, so callers keep that unit on the map
/// and use [`active_trade_route_traders`] to remove it only from a speculative
/// planning clone.  Route expiry is deliberately held beyond this planning turn:
/// every real state export replaces this list, and inventing an end turn would
/// make a real active route disappear from CIVVIS early.
fn restore_active_trade_routes(
    game: &mut crate::game::Game,
    routes: &[StateTradeRoute],
    city_of_civ6: &std::collections::BTreeMap<i64, u32>,
) -> Vec<String> {
    game.routes.clear();
    Arc::make_mut(&mut game.observed_route_posts).clear();
    Arc::make_mut(&mut game.observed_route_yields).clear();
    let ends = game.turn.saturating_add(game.max_turns.max(1));
    let mut unresolved = Vec::new();

    for route in routes {
        // City ids are only unique within a Firaxis player. Every first city is
        // commonly 65536, so an id-only map can resolve Krakow's route endpoint as
        // Zanzibar. The export carries coordinates precisely to disambiguate them.
        let origin = if route.origin_x >= 0 && route.origin_y >= 0 {
            game.city_at(crate::hex::offset_to_axial(route.origin_x, route.origin_y))
        } else {
            None
        }
        .or_else(|| city_of_civ6.get(&route.origin).copied());
        let Some(origin) = origin else {
            unresolved.push(format!("trade_route:{}:origin", route.trader));
            continue;
        };
        let destination = if route.destination_x >= 0 && route.destination_y >= 0 {
            game.city_at(crate::hex::offset_to_axial(
                route.destination_x,
                route.destination_y,
            ))
        } else {
            None
        }
        .or_else(|| city_of_civ6.get(&route.destination).copied());
        let Some(destination) = destination else {
            unresolved.push(format!("trade_route:{}:destination", route.trader));
            continue;
        };
        if origin == destination {
            unresolved.push(format!("trade_route:{}:same_city", route.trader));
            continue;
        }
        let Some(origin_city) = game.cities.get(&origin) else {
            unresolved.push(format!("trade_route:{}:missing_origin", route.trader));
            continue;
        };
        if origin_city.owner != 0 || !game.cities.contains_key(&destination) {
            unresolved.push(format!("trade_route:{}:unavailable_city", route.trader));
            continue;
        }
        // The host's own path is the one that pays: its Trading Posts, by
        // owner, override the model's straight-line walk (Ostia -> Aquileia
        // ran through Cumae's post, run civvis-20260816T200454Z t144-154).
        if let (Some(own), Some(foreign)) = (route.posts_own, route.posts_foreign) {
            if own >= 0 && foreign >= 0 {
                Arc::make_mut(&mut game.observed_route_posts)
                    .insert((origin, destination), (own, foreign));
            }
        }
        // And what the host says the route pays its origin, which covers a
        // destination whose districts the seat has never seen.
        if let Some(yields) = route.yields {
            let finite = [
                yields.food,
                yields.production,
                yields.gold,
                yields.science,
                yields.culture,
                yields.faith,
            ]
            .iter()
            .all(|value| value.is_finite() && *value >= 0.0);
            if finite {
                Arc::make_mut(&mut game.observed_route_yields)
                    .insert((origin, destination), yields);
            }
        }
        game.routes.push(crate::game::TradeRoute {
            origin,
            dest: destination,
            owner: 0,
            ends,
        });
    }
    unresolved
}

/// Seat the routes OTHER players run into this seat's cities.
///
/// `restore_active_trade_routes` carries only our own routes, so every rule the
/// destination earns from an incoming foreign route — Zhang Qian's "+2 Gold from
/// incoming foreign routes", alliance yields, the World Congress Trade Policy's
/// +4 Gold per incoming international route — paid nothing on a mirrored board:
/// Cumae's host ledger read "+4 from Incoming Trade Routes" for t87-101 of run
/// civvis-20260816T200454Z against a model that had no route to count. The
/// mod's `incoming_routes.origins[]` names each route's origin city and owner;
/// the origin's owner ON THE BOARD is the route's seat (rival cities are planted
/// before this runs), so no host-id map is needed. A route whose origin city is
/// not on the board (an unrevealed city of an unmet civ) is reported, not
/// guessed. Routes seated here run to the end of the game the way restored own
/// routes do — the host says when they end by dropping them from the export.
fn restore_incoming_foreign_routes(
    game: &mut crate::game::Game,
    cities: &[StateCity],
) -> Vec<String> {
    let ends = game.turn.saturating_add(game.max_turns.max(1));
    let mut unresolved = Vec::new();
    // The route origins are visibility-limited, but the host's destination
    // counts are not. The count delta is reconciled after all known routes,
    // including rival outgoing routes, have been seated below.
    Arc::make_mut(&mut game.observed_incoming_route_deltas).clear();
    for city in cities {
        let Some(incoming) = city.incoming_routes.as_ref() else {
            continue;
        };
        let Some(dest) = game.city_at(crate::hex::offset_to_axial(city.x, city.y)) else {
            unresolved.push(format!("incoming_route:{}:destination", city.name));
            continue;
        };
        if incoming.origins.is_empty() {
            continue;
        }
        let dest_owner = game.cities[&dest].owner;
        for origin in &incoming.origins {
            if origin.x < 0 || origin.y < 0 {
                unresolved.push(format!("incoming_route:{}:origin", city.name));
                continue;
            }
            let Some(origin_city) = game.city_at(crate::hex::offset_to_axial(origin.x, origin.y))
            else {
                unresolved.push(format!("incoming_route:{}:origin_city", city.name));
                continue;
            };
            let owner = game.cities[&origin_city].owner;
            if owner == dest_owner || origin_city == dest {
                // A domestic route of ours is already in `game.routes` from the
                // seat's own export; anything else here is a host/board mismatch.
                continue;
            }
            let route = crate::game::TradeRoute {
                origin: origin_city,
                dest,
                owner,
                ends,
            };
            if !game.routes.contains(&route) {
                game.routes.push(route);
            }
        }
    }
    unresolved
}

/// Seat the routes a rival runs OUT of its cities, when both ends are on the
/// board.
///
/// `restore_incoming_foreign_routes` carries only the ones that end in our
/// cities, so a rival's domestic and third-party routes paid it nothing on
/// the mirrored board and its route income was CIVVIS's guess from a bare
/// city. The mod exports a route only when both ends stand on revealed
/// ground; here the owner is the origin city's seat ON THE BOARD, as for
/// incoming routes, a route whose end is not planted is skipped rather than
/// guessed, and our own routes (carried by the seat's own export, which
/// `restore_active_trade_routes` rebuilds from scratch) are left alone.
/// Runs after both of those, since the former clears `game.routes`.
fn restore_rival_outgoing_routes(game: &mut crate::game::Game, rivals: &[StateRival]) {
    let ends = game.turn.saturating_add(game.max_turns.max(1));
    for rival in rivals {
        let Some(routes) = rival.trade_routes.as_ref() else {
            continue;
        };
        for route in routes {
            if route.origin_x < 0
                || route.origin_y < 0
                || route.destination_x < 0
                || route.destination_y < 0
            {
                continue;
            }
            let Some(origin) =
                game.city_at(crate::hex::offset_to_axial(route.origin_x, route.origin_y))
            else {
                continue;
            };
            let Some(dest) = game.city_at(crate::hex::offset_to_axial(
                route.destination_x,
                route.destination_y,
            )) else {
                continue;
            };
            let owner = game.cities[&origin].owner;
            if origin == dest || owner == 0 {
                continue;
            }
            let route = crate::game::TradeRoute {
                origin,
                dest,
                owner,
                ends,
            };
            if !game.routes.contains(&route) {
                game.routes.push(route);
            }
        }
    }
}

/// Reconcile the host's incoming-route totals with the route entities that
/// could be materialized on the mirrored board. Positive deltas are normally
/// routes whose origin is fogged; negative deltas keep a stale or partial
/// route export from making the model over-count. Keeping a delta instead of
/// replacing the derived count means a counterfactual route action still moves
/// the projected destination-side yields by exactly one route.
fn reconcile_incoming_route_deltas(game: &mut crate::game::Game, cities: &[StateCity]) {
    Arc::make_mut(&mut game.observed_incoming_route_deltas).clear();
    for city in cities {
        let Some(incoming) = city.incoming_routes.as_ref() else {
            continue;
        };
        let Some(dest) = game.city_at(crate::hex::offset_to_axial(city.x, city.y)) else {
            continue;
        };
        let dest_owner = game.cities[&dest].owner;
        let known_foreign = game
            .routes
            .iter()
            .filter(|route| route.dest == dest && route.owner != dest_owner)
            .count() as i64;
        let known_domestic = game
            .routes
            .iter()
            .filter(|route| route.dest == dest && route.owner == dest_owner)
            .count() as i64;
        Arc::make_mut(&mut game.observed_incoming_route_deltas).insert(
            dest,
            (
                incoming.foreign.saturating_sub(known_foreign),
                incoming.domestic.saturating_sub(known_domestic),
            ),
        );
    }
}

/// The engine id and target vocabulary of one host World Congress resolution,
/// or `None` when the model has no such resolution (Arms Control, the
/// Diplomatic Victory resolution) or the target does not translate. The
/// Diplomatic Victory row is intentionally filtered from the unmapped report
/// by [`is_known_congress_noop`], because its standing is imported separately
/// through `congress_dvp`.
///
/// Targets follow the engine's own `congress_resolution` rosters: a player is
/// its SEAT as a decimal string, a resource/district/building/feature/project
/// its CIVVIS node name, and the class-like targets (Great Person class,
/// promotion class, great-work object, spy operation, yield) the Firaxis
/// suffix in lower case, which is what the engine keys them by. The popup
/// exports localized display keys (`LOC_*_NAME` / `LOC_*_DESCRIPTION`) for many non-player
/// targets, so those wrappers are removed before the rule-node translation.
fn civvis_city_state_type(target: &str) -> Option<String> {
    let target = target.strip_prefix("MINOR_CIV_")?;
    let target = target.strip_prefix("BONUS_").unwrap_or(target);
    let target = target.strip_suffix("_TRAIT").unwrap_or(target);
    let target = target.strip_suffix("_BONUS").unwrap_or(target);
    match target {
        "SCIENTIFIC" | "RELIGIOUS" | "TRADE" | "CULTURAL" | "MILITARISTIC" | "INDUSTRIAL" => {
            Some(target.to_ascii_lowercase())
        }
        _ => None,
    }
}

fn civvis_congress_effect(
    rules: &crate::rules::Rules,
    resolution: &StateResolution,
    seat_of_host: &std::collections::BTreeMap<usize, usize>,
    expires: u32,
) -> Option<crate::game::CongressEffect> {
    let outcome = match resolution.option {
        1 => "A",
        2 => "B",
        _ => return None,
    };
    let raw_target = resolution.target.trim();
    let localized_target = raw_target.strip_prefix("LOC_").unwrap_or(raw_target);
    let target = localized_target
        .strip_suffix("_NAME")
        .or_else(|| localized_target.strip_suffix("_DESCRIPTION"))
        .unwrap_or(localized_target);
    // `ChosenThing` is not always the engine row name. The Heritage
    // Organization popup has emitted both `GREATWORKOBJECT_WRITING` and the
    // localized-key spelling `LOC_GREAT_WORK_OBJECT_WRITING_NAME`; normalize
    // the latter before the class-like target match below.
    let normalized_target = target
        .strip_prefix("GREAT_WORK_OBJECT_")
        .map(|rest| format!("GREATWORKOBJECT_{rest}"))
        .unwrap_or_else(|| target.to_string());
    let target = normalized_target.as_str();
    let seat = || {
        target
            .parse::<usize>()
            .ok()
            .and_then(|host| seat_of_host.get(&host).copied())
            .map(|seat| seat.to_string())
    };
    let suffix = |prefix: &str| {
        target
            .strip_prefix(prefix)
            .map(|rest| rest.to_ascii_lowercase())
            .filter(|rest| !rest.is_empty())
    };
    let (id, target): (&str, String) = match resolution.kind.as_str() {
        "WC_RES_TRADE_TREATY" => ("trade_policy", seat()?),
        "WC_RES_BORDER_CONTROL" => ("border_control_treaty", seat()?),
        "WC_RES_MIGRATION_TREATY" => ("migration_treaty", seat()?),
        "WC_RES_PUBLIC_RELATIONS" => ("public_relations", seat()?),
        "WC_RES_SOVEREIGNTY" => ("sovereignty", civvis_city_state_type(target)?),
        "WC_RES_LUXURY" => (
            "luxury_policy",
            civvis_node_name(&rules.resources, target, "RESOURCE_")?,
        ),
        "WC_RES_MERCENARY_COMPANIES" => ("mercenary_companies", suffix("YIELD_")?),
        "WC_RES_WORLD_RELIGION" => ("world_religion", civvis_religion_name(target)?),
        "WC_RES_URBAN_DEVELOPMENT" => (
            "urban_development_treaty",
            if target == "DISTRICT_CITY_CENTER" {
                "city_center".to_string()
            } else {
                civvis_node_name(&rules.districts, target, "DISTRICT_")?
            },
        ),
        "WC_RES_PATRONAGE" => ("patronage", suffix("GREAT_PERSON_CLASS_")?),
        "WC_RES_MILITARY_ADVISORY" => (
            "military_advisory",
            match target {
                "PROMOTION_CLASS_APOSTLE" => "religious_apostle".to_string(),
                "PROMOTION_CLASS_MONK" => "warrior_monk".to_string(),
                "PROMOTION_CLASS_SPY" => "espionage".to_string(),
                _ => suffix("PROMOTION_CLASS_")?,
            },
        ),
        "WC_RES_ESPIONAGE_PACT" => ("espionage_pact", suffix("UNITOPERATION_SPY_")?),
        // The engine keys the visual-art objects — sculpture, portrait,
        // landscape, religious art — as one "art" class.
        "WC_RES_HERITAGE_ORG" => (
            "heritage_organization",
            match target {
                "GREATWORKOBJECT_SCULPTURE"
                | "GREATWORKOBJECT_PORTRAIT"
                | "GREATWORKOBJECT_LANDSCAPE"
                | "GREATWORKOBJECT_RELIGIOUS" => "art".to_string(),
                _ => suffix("GREATWORKOBJECT_")?,
            },
        ),
        "WC_RES_WORLD_IDEOLOGY" => (
            "world_ideology",
            civvis_node_name(&rules.governments, target, "GOVERNMENT_")?,
        ),
        "WC_RES_GLOBAL_ENERGY_TREATY" => (
            "global_energy_treaty",
            civvis_node_name(&rules.buildings, target, "BUILDING_")?,
        ),
        "WC_RES_PUBLIC_WORKS" => (
            "public_works_program",
            civvis_node_name(&rules.projects, target, "PROJECT_")?,
        ),
        "WC_RES_DEFORESTATION_TREATY" => (
            "deforestation_treaty",
            civvis_node_name(&rules.features, target, "FEATURE_")?,
        ),
        _ => return None,
    };
    Some(crate::game::CongressEffect {
        resolution: id.to_string(),
        outcome: outcome.to_string(),
        target,
        expires,
    })
}

/// A host resolution whose effect is already represented by another
/// authoritative export and therefore has no `CongressEffect` on the model.
///
/// Diplomatic Victory changes the standings reported in `congress_dvp`; it
/// does not change a yield, policy, or unlock on the mirrored board. Keeping
/// it out of `unmapped` prevents a known, correctly handled resolution from
/// masking actual bridge gaps while leaving unknown resolution kinds visible.
fn is_known_congress_noop(resolution: &StateResolution) -> bool {
    resolution.kind == "WC_RES_DIPLOVICTORY"
}

/// Extend the ordinary host-player map with the anonymous major seats that a
/// Congress table exposes even when the seat has not met those players.
///
/// `seat_of_host` may contain only players whose identity is known to the
/// mirror (plus mapped city-states) when an older export has no roster size.
/// Use the same stable major-seat assignment as [`apply_congress_dvp`] for any
/// numeric Congress target whose standing identifies an otherwise empty major
/// slot, without changing the global map and accidentally assigning an unseen
/// major to a city-state slot in unrelated systems.
fn congress_seat_of_host(
    state: &StateSnapshot,
    seat_of_host: &std::collections::BTreeMap<usize, usize>,
    seat_count: usize,
) -> std::collections::BTreeMap<usize, usize> {
    let mut congress_seats = seat_of_host.clone();
    let Some(congress) = &state.congress_dvp else {
        return congress_seats;
    };
    let ours = state.seat.local_player.max(0) as usize;
    let major_seats = match state.seat.players {
        0 => seat_count,
        players => players.min(seat_count),
    };
    let mut unmet: Vec<&StateCongressDvpEntry> = congress
        .points
        .iter()
        .filter(|entry| entry.player != ours && !congress_seats.contains_key(&entry.player))
        .collect();
    unmet.sort_by_key(|entry| entry.player);
    let occupied: BTreeSet<usize> = congress_seats.values().copied().collect();
    let free_seats = (1..major_seats).filter(|seat| !occupied.contains(seat));
    for (seat, entry) in free_seats.zip(unmet) {
        congress_seats.insert(entry.player, seat);
    }
    congress_seats
}

/// Carry the host's current Great Person classes and hard named prerequisites
/// into the simulator.
///
/// The class label is not enough: Firaxis offers Hildegard of Bingen as a
/// Great Scientist but requires a Holy Site, and Mary Leakey as a Great
/// Scientist but requires a Theater. A Campus-only science empire can recruit
/// both and leave their physical units stranded indefinitely. Do not guess at
/// all of Firaxis's positional conditions here; these three exported columns
/// are authoritative *necessary* conditions, so their absence is a safe reason
/// not to spend another point, project, or patronage purchase on the live offer.
///
/// The live bridge has one additional fidelity constraint for Artifact offers:
/// the compact ruleset represents Palace/Apadana's flexible host slots as
/// `any`, but Firaxis's `GREATWORKOBJECT_ARTIFACT` is accepted only by an
/// Archaeological Museum (or another explicitly artifact-typed slot). Keep
/// that correction at the bridge boundary so the native simulator's existing
/// universal-slot semantics remain unchanged for headless AI tests and saves.
fn live_great_work_offer_has_capacity(game: &crate::game::Game, pid: usize, kind: &str) -> bool {
    if kind == "artifact" {
        let has_typed_artifact_slot =
            game.cities
                .values()
                .filter(|city| city.owner == pid)
                .any(|city| {
                    city.buildings.iter().any(|building| {
                        game.rules
                            .buildings
                            .get(building)
                            .and_then(|spec| spec.great_work_slots.get(kind))
                            .is_some_and(|count| *count > 0)
                    }) || city.wonders.keys().any(|wonder| {
                        game.rules
                            .wonders
                            .get(wonder)
                            .and_then(|spec| spec.great_work_slots.get(kind))
                            .is_some_and(|count| *count > 0)
                    })
                });
        if !has_typed_artifact_slot {
            return false;
        }
    }
    game.can_house_great_works(pid, kind, 1)
}

/// Native works remain in their reported building: the bridge does not issue
/// relocation orders. Global modeled housing can free a Palace by moving its
/// writing elsewhere, which is not an immediately usable slot in the host.
fn live_open_great_work_slots(
    rules: &crate::rules::Rules,
    state: &StateSnapshot,
) -> Option<BTreeSet<String>> {
    let mut open = BTreeSet::new();
    for city in &state.cities {
        let works = city.great_works.as_ref()?;
        if works.iter().any(|work| work.building.is_empty()) {
            return None;
        }
        let buildings: BTreeSet<_> = city
            .buildings
            .iter()
            .chain(city.wonders.iter().map(|wonder| &wonder.kind))
            .collect();
        for building in buildings {
            let slots = civvis_node_name(&rules.buildings, building, "BUILDING_")
                .map(|name| rules.buildings[&name].great_work_slots.clone())
                .or_else(|| {
                    civvis_node_name(&rules.wonders, building, "BUILDING_")
                        .map(|name| rules.wonders[&name].great_work_slots.clone())
                });
            let Some(mut slots) = slots else {
                continue;
            };
            let mut seen = BTreeSet::new();
            for work in works.iter().filter(|work| &work.building == building) {
                if !seen.insert(work.slot) {
                    continue;
                }
                let Some(kind) = great_work_kind(&work.object) else {
                    slots.clear();
                    break;
                };
                let compatible = slots
                    .keys()
                    .find(|slot| {
                        slot.as_str() == kind
                            || (matches!(kind, "art" | "religious_art")
                                && matches!(slot.as_str(), "art" | "religious_art"))
                    })
                    .cloned()
                    .or_else(|| slots.contains_key("any").then(|| "any".to_string()));
                if let Some(slot) = compatible {
                    *slots.get_mut(&slot).unwrap() -= 1;
                } else {
                    // Unmapped native capacity cannot establish a free slot.
                    slots.clear();
                    break;
                }
            }
            open.extend(
                slots
                    .into_iter()
                    .filter(|(_, count)| *count > 0)
                    .map(|(kind, _)| kind),
            );
        }
    }
    Some(open)
}

/// The identity Civilization VI gave this game: who we play, and under what rules.
///
/// ★★★★ THE SIXTH FACT THE BRIDGE DROPPED. The mod has always exported all of this
/// in its `seat` event; nothing read it, so `Game::new` assigned CIVVIS's default
/// roster and the operator watched Trajan of Rome next to a Civilization VI game
/// playing Persia. Identity is not cosmetic here — the whole point of running the
/// two side by side is checking that they match, and a mismatched name defeats that
/// before any deeper comparison starts.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct Seat {
    #[serde(default = "minus_one")]
    pub local_player: i32,
    #[serde(default)]
    pub players: usize,
    /// Configured city-state seats, including ones this player has not met yet.
    #[serde(default)]
    pub city_states: usize,
    #[serde(default)]
    pub max_turns: i64,
    #[serde(default)]
    pub civ: String,
    #[serde(default)]
    pub leader: String,
    #[serde(default)]
    pub difficulty: String,
    #[serde(default)]
    pub speed: String,
    #[serde(default)]
    pub map: String,
    #[serde(default)]
    pub size: String,
    /// The six victory checkboxes as the host reports them
    /// (`Game.IsVictoryEnabled`, the shipped WorldRankings check). The mod has
    /// exported these all along and the mirror dropped them, so a live board's
    /// `victory_conditions` were always the all-six default and
    /// `victory_strategy_enabled` could authorise a lane the lobby had
    /// switched off. `None` (older mod) keeps the default.
    #[serde(default)]
    pub victories: Option<SeatVictories>,
    /// The mod sequences a unit's orders (`CivvisQueue`): a strike after a
    /// walk is issued once the walk has arrived, a found after a walk once
    /// the settler stands on its site. `civvis_orders` sends a unit's whole
    /// planned sequence only when this is true; against an older mod it
    /// defers the follow-ups to the next frame exactly as before. Absent
    /// (older mod) reads `false`, which is the conservative behaviour.
    #[serde(default)]
    pub order_queue: bool,
    /// The mod caps every MOVE_TO to the turn's reach and cancels combat
    /// units' queued paths at turn start, so `StateUnit::moves` is read at the
    /// start of the seat's turn and means "movement available this turn".
    /// Only then does the mirror trust it (see `mirror_unit_moves_for`);
    /// absent (older mod), every unit keeps its full allowance exactly as
    /// before, because the export's `moves` has misled twice in the past.
    #[serde(default)]
    pub moves_at_turn_start: bool,
    /// The mod opens mid-turn replan frames (`CivvisFrames`, `ReplanFrames`
    /// ≥ 1): once the opening orders settle on a board with newly revealed
    /// ground and movement left to spend on it, the board is exported again
    /// and the same turn re-planned. Absent (older mod) reads `false`.
    #[serde(default)]
    pub replan_frames: bool,
    /// Exact cap reported by the host. A boolean cannot promise that a
    /// spot, volley and cavalry follow-up all fit into this turn.
    #[serde(default)]
    pub replan_frame_limit: Option<u32>,
    /// Newly revealed plots cross every turn and frame as `tiles` deltas
    /// (`CivvisTiles`), not only with the periodic sweep. Informational: the
    /// snapshot merges chunks cumulatively either way.
    #[serde(default)]
    pub tile_delta: bool,
}

/// See [`Seat::victories`]. Each checkbox is independently optional so one
/// refused `IsVictoryEnabled` read cannot switch the other five off.
#[derive(Clone, Copy, Debug, Default, serde::Deserialize)]
pub struct SeatVictories {
    #[serde(default)]
    pub conquest: Option<bool>,
    #[serde(default)]
    pub score: Option<bool>,
    #[serde(default)]
    pub technology: Option<bool>,
    #[serde(default)]
    pub culture: Option<bool>,
    #[serde(default)]
    pub religious: Option<bool>,
    #[serde(default)]
    pub diplomatic: Option<bool>,
}

/// Civilization VI emits `GAMESPEED_ONLINE`; CIVVIS's setup names it `online`.
///
/// Keep the trim/prefix boundary here instead of letting callers carry a string
/// normalisation convention: both `game.speed` and the typed `game.game_speed`
/// drive rules, and leaving either at the generated Standard default makes a live
/// Online game look plausible while costing twice as much to research and build.
fn civvis_game_speed(civ6: &str) -> Option<GameSpeed> {
    let id = civ6
        .trim()
        .strip_prefix("GAMESPEED_")
        .unwrap_or(civ6.trim())
        .to_ascii_lowercase();
    GameSpeed::from_id(&id)
}

/// The turn the game Civilization VI is playing ends on, as the seat reports it.
///
/// The engine's own readback wins (`seat.max_turns`, `Game.GetMaxGameTurns()`):
/// the harness may have created the game with a custom limit, and a page that
/// printed the speed's stock limit over it would be quietly wrong. Without a
/// readback, the speed's own limit (`GameSpeed::turn_limit`, Civilization VI's
/// `GameSpeed_Turns` sums: Online 250, Quick 330, Standard 500, Epic 750,
/// Marathon 1500). `None` when the seat names neither, so the caller says what
/// it is guessing.
///
/// ★ `civvis play --mirror` used to rebuild every board with a literal 250, which
/// is Online's limit alone, so a Standard game's page read TURN n/250.
pub fn host_turn_limit(state: &StateSnapshot) -> Option<u32> {
    u32::try_from(state.seat.max_turns)
        .ok()
        .filter(|turns| *turns > 0)
        .or_else(|| civvis_game_speed(&state.seat.speed).map(GameSpeed::turn_limit))
}

/// Civilization VI emits `DIFFICULTY_SETTLER`; CIVVIS uses the rules key `settler`.
fn civvis_difficulty(civ6: &str) -> Option<String> {
    let id = civ6
        .trim()
        .strip_prefix("DIFFICULTY_")
        .unwrap_or(civ6.trim())
        .to_ascii_lowercase();
    (!id.is_empty()).then_some(id)
}

/// Civilization VI exports its selected map file, for example `Continents.lua`.
fn civvis_map_script(civ6: &str) -> Option<MapScript> {
    let id = civ6.trim().to_ascii_lowercase();
    let id = id.strip_suffix(".lua").unwrap_or(&id);
    MapScript::from_id(id)
}

/// `CIVILIZATION_ROME` -> `Rome`, using CIVVIS's own roster as the authority.
///
/// Returns `None` when Civilization VI names a civilization CIVVIS does not have,
/// which is deliberate: a wrong-but-plausible name is worse than an obvious gap,
/// because it silently reintroduces exactly the mismatch this function exists to
/// remove. Firaxis's Babylon pack retains an internal `_STK` suffix, and the
/// Ottomans differ only by CIVVIS's plural spelling; both exact normalizations
/// are handled below before the roster is consulted.
pub fn civvis_civ_name(civ6: &str) -> Option<&'static str> {
    let id = civ6
        .trim()
        .strip_prefix("CIVILIZATION_")
        .unwrap_or(civ6.trim());
    // `CIVILIZATION_BABYLON_STK` is the shipping Babylon civilization id;
    // STK is Firaxis's pack/implementation suffix, not part of its identity.
    let bare = id.strip_suffix("_STK").unwrap_or(id).replace('_', " ");
    if bare.is_empty() || bare == "?" {
        return None;
    }
    let matches = |candidate: &str| {
        crate::game::CIV_NAMES
            .iter()
            .find(|known| known.eq_ignore_ascii_case(candidate))
            .copied()
    };
    // Plural retry: Civ 6 says OTTOMAN where CIVVIS says Ottomans, and the same
    // shape covers any future singular/plural disagreement.
    matches(&bare)
        .or_else(|| matches(&format!("{bare}s")))
        .or_else(|| matches(bare.trim_end_matches('s')))
}

const GOVERNOR_TYPES: &[(&str, &str)] = &[
    ("amani", "GOVERNOR_THE_AMBASSADOR"),
    ("liang", "GOVERNOR_THE_BUILDER"),
    ("moksha", "GOVERNOR_THE_CARDINAL"),
    ("victor", "GOVERNOR_THE_DEFENDER"),
    ("pingala", "GOVERNOR_THE_EDUCATOR"),
    ("reyna", "GOVERNOR_THE_MERCHANT"),
    ("magnus", "GOVERNOR_THE_RESOURCE_MANAGER"),
];

const GOVERNOR_PROMOTION_TYPES: &[(&str, &str)] = &[
    ("emissary", "GOVERNOR_PROMOTION_AMBASSADOR_EMISSARY"),
    ("affluence", "GOVERNOR_PROMOTION_AMBASSADOR_AFFLUENCE"),
    ("local_informants", "GOVERNOR_PROMOTION_LOCAL_INFORMANTS"),
    (
        "foreign_investor",
        "GOVERNOR_PROMOTION_AMBASSADOR_FOREIGN_INVESTOR",
    ),
    ("puppeteer", "GOVERNOR_PROMOTION_AMBASSADOR_PUPPETEER"),
    (
        "zoning_commissioner",
        "GOVERNOR_PROMOTION_ZONING_COMMISSIONER",
    ),
    ("aquaculture", "GOVERNOR_PROMOTION_AQUACULTURE"),
    (
        "reinforced_materials",
        "GOVERNOR_PROMOTION_REINFORCED_INFRASTRUCTURE",
    ),
    ("water_works", "GOVERNOR_PROMOTION_WATER_WORKS"),
    (
        "parks_and_recreation",
        "GOVERNOR_PROMOTION_PARKS_RECREATION",
    ),
    (
        "grand_inquisitor",
        "GOVERNOR_PROMOTION_CARDINAL_GRAND_INQUISITOR",
    ),
    (
        "laying_on_of_hands",
        "GOVERNOR_PROMOTION_CARDINAL_LAYING_ON_OF_HANDS",
    ),
    (
        "citadel_of_god",
        "GOVERNOR_PROMOTION_CARDINAL_CITADEL_OF_GOD",
    ),
    ("patron_saint", "GOVERNOR_PROMOTION_CARDINAL_PATRON_SAINT"),
    (
        "divine_architect",
        "GOVERNOR_PROMOTION_CARDINAL_DIVINE_ARCHITECT",
    ),
    (
        "garrison_commander",
        "GOVERNOR_PROMOTION_GARRISON_COMMANDER",
    ),
    ("defense_logistics", "GOVERNOR_PROMOTION_DEFENSE_LOGISTICS"),
    ("embrasure", "GOVERNOR_PROMOTION_EMBRASURE"),
    (
        "air_defense_initiative",
        "GOVERNOR_PROMOTION_AIR_DEFENSE_INITIATIVE",
    ),
    (
        "arms_race_proponent",
        "GOVERNOR_PROMOTION_EDUCATOR_ARMS_RACE_PROPONENT",
    ),
    ("connoisseur", "GOVERNOR_PROMOTION_EDUCATOR_CONNOISSEUR"),
    ("researcher", "GOVERNOR_PROMOTION_EDUCATOR_RESEARCHER"),
    ("grants", "GOVERNOR_PROMOTION_EDUCATOR_GRANTS"),
    (
        "space_initiative",
        "GOVERNOR_PROMOTION_EDUCATOR_SPACE_INITIATIVE",
    ),
    ("curator", "GOVERNOR_PROMOTION_MERCHANT_CURATOR"),
    ("harbormaster", "GOVERNOR_PROMOTION_MERCHANT_HARBORMASTER"),
    (
        "forestry_management",
        "GOVERNOR_PROMOTION_MERCHANT_FORESTRY_MANAGEMENT",
    ),
    ("tax_collector", "GOVERNOR_PROMOTION_MERCHANT_TAX_COLLECTOR"),
    ("contractor", "GOVERNOR_PROMOTION_MERCHANT_CONTRACTOR"),
    (
        "renewable_subsidizer",
        "GOVERNOR_PROMOTION_MERCHANT_RENEWABLE_ENERGY",
    ),
    (
        "surplus_logistics",
        "GOVERNOR_PROMOTION_RESOURCE_MANAGER_SURPLUS_LOGISTICS",
    ),
    (
        "provision",
        "GOVERNOR_PROMOTION_RESOURCE_MANAGER_EXPEDITION",
    ),
    (
        "industrialist",
        "GOVERNOR_PROMOTION_RESOURCE_MANAGER_INDUSTRIALIST",
    ),
    (
        "black_marketeer",
        "GOVERNOR_PROMOTION_RESOURCE_MANAGER_BLACK_MARKETEER",
    ),
    (
        "vertical_integration",
        "GOVERNOR_PROMOTION_RESOURCE_MANAGER_VERTICAL_INTEGRATION",
    ),
];

const GOVERNOR_BASE_PROMOTION_TYPES: &[(&str, &str)] = &[
    ("amani", "GOVERNOR_PROMOTION_AMBASSADOR_MESSENGER"),
    ("liang", "GOVERNOR_PROMOTION_BUILDER_GUILDMASTER"),
    ("moksha", "GOVERNOR_PROMOTION_CARDINAL_BISHOP"),
    ("victor", "GOVERNOR_PROMOTION_REDOUBT"),
    ("pingala", "GOVERNOR_PROMOTION_EDUCATOR_LIBRARIAN"),
    ("reyna", "GOVERNOR_PROMOTION_MERCHANT_LAND_ACQUISITION"),
    (
        "magnus",
        "GOVERNOR_PROMOTION_RESOURCE_MANAGER_GROUNDBREAKER",
    ),
];

/// Translate Firaxis's stable Governor type id into CIVVIS's rules key.
pub fn civvis_governor_name(civ6: &str) -> Option<&'static str> {
    let civ6 = civ6.trim();
    GOVERNOR_TYPES
        .iter()
        .find_map(|(ours, host)| (*host == civ6).then_some(*ours))
}

/// Translate a CIVVIS Governor key into the exact Firaxis database type.
pub fn civ6_governor_name(civvis: &str) -> Option<&'static str> {
    GOVERNOR_TYPES
        .iter()
        .find_map(|(ours, host)| (*ours == civvis).then_some(*host))
}

/// Translate Firaxis's Governor promotion ids into CIVVIS's promotion keys.
pub fn civvis_governor_promotion(civ6: &str) -> Option<&'static str> {
    let civ6 = civ6.trim();
    GOVERNOR_PROMOTION_TYPES
        .iter()
        .find_map(|(ours, host)| (*host == civ6).then_some(*ours))
}

/// Translate a CIVVIS promotion key into the exact Firaxis database type.
pub fn civ6_governor_promotion(civvis: &str) -> Option<&'static str> {
    GOVERNOR_PROMOTION_TYPES
        .iter()
        .find_map(|(ours, host)| (*ours == civvis).then_some(*host))
}

/// Firaxis exposes each Governor's intrinsic appointment ability through the
/// promotion API even though it costs no separate title. CIVVIS keeps the same
/// effects directly on the Governor specification rather than in its promotion set.
pub fn civ6_governor_base_promotion(civvis: &str) -> Option<&'static str> {
    GOVERNOR_BASE_PROMOTION_TYPES
        .iter()
        .find_map(|(ours, host)| (*ours == civvis).then_some(*host))
}

/// The newest `state` event, or the one for `turn` when asked for a specific one.
///
/// ⚠ Newest-wins rather than first-match: the mod re-emits state as a turn is
/// replayed, and an early partial export would otherwise win forever.
/// Keys `state_schema_gaps` accepts on an exported city and unit.
///
/// ⚠⚠ HOISTED SO A TEST CAN SEE THEM. These are a second list of field names beside
/// `StateCity`/`StateUnit`, and nothing kept the two in step — so #877 added
/// `production`, `production_cost` and `production_turns` to the struct, the mirror
/// read them correctly, and the decider went on reporting
/// `unmapped: schema:city.production,...` on every turn. `class` had been doing the
/// same thing for longer.
///
/// An instrument that reports a gap where there is none is worse than no instrument:
/// this project relies on `unmapped` to find exactly this class of defect, and a list
/// with known-false entries is one nobody reads. `the_schema_allowlists_cover_every_
/// declared_field` now fails if they drift apart again.
///
/// ⚠ A superset is correct, not an error. Serde aliases mean one field answers to two
/// names — `kind` also accepts `type` — and only the export side needs both.
const CITY_KEYS: &[&str] = &[
    "id",
    "name",
    "captured_from",
    "original_owner",
    "buildings",
    "pillaged_buildings",
    "religion",
    "religion_observed",
    "religion_next",
    "religion_turns",
    "pantheon_active",
    "districts",
    "district_capacity",
    "wonders",
    "worked",
    "specialists",
    "great_works",
    "yields",
    "producing",
    "producing_hash",
    "production_progress",
    "production",
    "production_cost",
    "production_turns",
    // The host's menus and the queue behind the head, read by
    // `host_menus_from` and `host_queue_tail`.
    "buildable",
    "purchasable",
    "plot_purchases",
    "queue",
    "food",
    "loyalty_per_turn",
    "falls_to",
    "x",
    "y",
    "pop",
    "capital",
    "original_capital",
    "defense",
    "ranged_strength",
    "damage",
    "max_damage",
    "wall_damage",
    "max_wall_damage",
    "loyalty",
    "housing",
    "housing_from_improvements",
    // The host's own amenity ledger and the multiplier it puts on every non-food
    // yield. `the_schema_allowlists_cover_every_declared_field` caught these missing
    // on the first run, which is the whole reason that test exists.
    "amenities",
    "amenities_needed",
    "happiness",
    "happiness_yield_mult",
    "amenities_luxuries",
    "amenities_entertainment",
    "amenities_civics",
    "amenities_city_states",
    "amenities_war_weariness",
    "amenities_bankruptcy",
    // The complete amenity and housing ledgers, the host's growth arithmetic and
    // the per-yield source tooltips: the fields the yield-fidelity instrument
    // reads. `the_schema_allowlists_cover_every_declared_field` fails if a
    // StateCity field is missing here.
    "amenities_great_people",
    "amenities_religion",
    "amenities_national_parks",
    "amenities_starting_era",
    "amenities_improvements",
    "amenities_districts",
    "amenities_natural_wonders",
    "housing_from_water",
    "housing_from_buildings",
    "housing_from_districts",
    "housing_from_civics",
    "housing_from_great_people",
    "housing_from_starting_era",
    "housing_from_great_works",
    "food_surplus",
    "growth_threshold",
    "growth_turns",
    "housing_growth_mult",
    "happiness_growth_mult",
    "overall_growth_mult",
    "food_favored",
    "food_disfavored",
    "food_focus_managed",
    "yield_sources",
    "center_yields",
    "incoming_routes",
];

const UNIT_KEYS: &[&str] = &[
    "id",
    "kind",
    "type",
    "base",
    "class",
    "x",
    "y",
    "hp",
    "combat",
    "ranged",
    "player",
    "moves",
    "xp",
    "level",
    "rock_band_level",
    "album_sales",
    "promotions",
    "offered_promotions",
    "build_charges",
    "spread_charges",
    "religion",
    "fortified",
    "fortify_turns",
    "formation",
    "formation_count",
    "great_person",
    "queued_dest",
    "embarked",
    "attacks_remaining",
    "range",
    "free",
    // The host's per-unit affordances (docs/FIDELITY.md item 9).
    "upgrade_to",
    "upgrade_cost",
    "upgrade_resource_cost",
    "upgrade_resource",
    "upgrade_blocked_reason",
    "maintenance",
    "religious_strength",
    "max_moves",
    "activity",
    "spy_operation",
    "spy_operation_end_turn",
    "spy_missions_available",
    "spy_mission_targets",
    "spy_city",
    "concert_plots",
];

const PUBLIC_STATS_KEYS: &[&str] = &[
    "city_count",
    "population",
    "food",
    "production",
    "wonder_count",
    "suzerain_count",
    "nuclear_devices",
    "thermonuclear_devices",
];

/// Every key a met rival's export may carry. `state_schema_gaps` reports
/// anything else as `schema:rival.<key>`, and
/// `the_schema_allowlists_cover_every_declared_field` fails if a
/// `StateRival` field is missing here.
const RIVAL_KEYS: &[&str] = &[
    "player",
    "civ",
    "leader",
    "government",
    "dark_age",
    "golden_age",
    "heroic_golden_age",
    "can_declare",
    "war_declarations",
    // The host's own relationship, ledger, alliance, missions, promises
    // and visibility for this rival — see `StateRival::diplomatic_state`.
    "diplomatic_state",
    "our_grievances_against_them",
    "grievances_against_us",
    "grievance_change_per_turn",
    "alliance_type",
    "alliance_level",
    "alliance_turns_left",
    "our_denounce_turn",
    "their_denounce_turn",
    "friendship_turn",
    "denounce_time_limit",
    "visibility",
    "their_visibility_on_us",
    "open_borders_granted",
    "delegation_at",
    "embassy_at",
    "their_delegation",
    "their_embassy",
    "promises_made",
    "promises_received",
    "score",
    "dvp",
    "military",
    "at_war",
    // Its wars with other majors (#4452), read as `Game::observed_rival_wars`.
    "at_war_with",
    "techs",
    "civics",
    "cities",
    "units",
    "science",
    "culture",
    "tourism",
    "gold",
    "gold_per_turn",
    "faith",
    "faith_per_turn",
    "public_stats",
    // The rival's tree by name, the World Rankings lane numbers, its
    // majority religion, the per-rival tourists, Era Score and routes.
    "tech_names",
    "civic_names",
    "techs_researched",
    "military_no_treasury",
    "cities_following_religion",
    "religion",
    "tourists_visiting_us",
    "era_score",
    "trade_routes",
    "tradeable_luxuries",
    "tradeable_strategics",
    "captives_of_ours",
    "opinion_modifiers",
    // Rival victory progress as the shipped World Rankings screen shows it.
    // `the_schema_allowlists_cover_every_declared_field` fails if a new
    // StateRival field is missing here.
    "science_projects",
    "science_victory_points",
    "science_victory_points_per_turn",
    "science_victory_points_needed",
    "foreign_tourists",
    "domestic_tourists",
    "culture_turns_to_victory",
    // Both border fields: `open_borders` had crossed since the buy lane
    // shipped and filed `schema:rival.open_borders` on every live turn.
    "open_borders",
    "enforces_borders",
];

/// The field names `state_schema_gaps` will accept for one struct, extracted from
/// the source of this file and of `mirror/state.rs`, where the snapshot types
/// live.
///
/// ⚠ Only used by the drift test below. It is deliberately crude — `pub name:` lines
/// inside the named `pub struct` block — because the alternative was another
/// hand-maintained list, which is the failure it exists to catch.
#[cfg(test)]
fn declared_fields(struct_name: &str) -> Vec<String> {
    let source = concat!(include_str!("mirror.rs"), include_str!("mirror/state.rs"));
    let head = format!("pub struct {struct_name} {{");
    let start = source
        .find(&head)
        .expect("the struct is declared in mirror.rs or mirror/state.rs");
    let body = &source[start + head.len()..];
    let end = body.find("\n}").expect("the struct block terminates");
    body[..end]
        .lines()
        .filter_map(|line| line.trim().strip_prefix("pub "))
        .filter_map(|rest| rest.split(':').next())
        .map(|name| name.trim().to_string())
        .collect()
}

fn state_schema_gaps(value: &serde_json::Value) -> Vec<String> {
    fn keys(
        value: &serde_json::Value,
        allowed: &[&str],
        path: &str,
        gaps: &mut std::collections::BTreeSet<String>,
    ) {
        let Some(object) = value.as_object() else {
            return;
        };
        for key in object.keys() {
            if !allowed.contains(&key.as_str()) {
                gaps.insert(format!("schema:{path}.{key}"));
            }
        }
    }

    #[rustfmt::skip]
    const STATE: &[&str] = &[
        // Wall-clock stamps are telemetry metadata, not mirrored game facts,
        // but they are intentional top-level keys on every live state export.
        // Keep them out of the gap stream so a real state-schema addition is
        // not buried under `schema:state.t` and `schema:state.utc` noise.
        "kind", "event", "run", "ctx", "turn", "frame", "t", "utc", "techs", "civics", "research",
        "science_projects", "science_victory_points", "science_victory_points_per_turn",
        "science_victory_points_needed", "boosted_techs", "boosted_civics",
        "research_progress", "research_quotes", "research_tree", "civic", "civic_progress", "government", "used_governments",
        "pantheon",
        "founded_religion", "founded_religions", "religion_beliefs",
        "holy_city", "inquisition_launched",
        "taken_religion_beliefs", "religions", "prophet_pending",
        "policies", "available_policies", "policy_slots", "policy_change", "gold", "gold_per_turn",
        "unit_maintenance_total", "building_maintenance_total", "district_maintenance_total",
        "faith", "faith_per_turn",
        "faith_sources", "science",
        "culture", "public_stats", "score", "dvp", "favor", "congress_dvp",
        "spy_capacity",
        // Consumed since the stockpile import (`seat.strategic_resources`) and
        // absent here until 2026-08-26, so every live state record filed a
        // `schema:state.strategic_resources` gap that was not one.
        "strategic_resources",
        "luxury_counts",
        "strategic_resource_income",
        "foreign_tourists", "domestic_tourists",
        "culture_turns_to_victory",
        "tourism_per_turn",
        "cities_following_religion",
        "military",
        "trade_capacity",
        "great_person_points",
        "great_person_points_per_turn",
        "great_person_exhausted",
        "great_person_costs",
        "great_person_offers",
        "governor_points",
        "governor_points_spent",
        // The age. `the_schema_allowlists_cover_every_declared_field` fails if a
        // StateSnapshot field is missing here.
        "era_score", "era_score_baseline", "normal_age_threshold",
        "golden_age_threshold", "world_era", "era_countdown", "dark_age", "golden_age",
        "heroic_golden_age", "dedications", "dedication_choices", "resolutions",
        "congress_turns_left",
        // The host's climate and its trade-route projections (2026-08-26).
        "climate", "route_options",
        "emergencies",
        "governors", "cities", "units", "trade_routes", "rivals", "minors", "hostiles",
        // Unspent envoys. `the_schema_allowlists_cover_every_declared_field` fails
        // if a new StateSnapshot field is missing here — this list is a second
        // copy of the struct's names and nothing keeps them in step automatically.
        "envoys_free",
    ];
    const CITY: &[&str] = CITY_KEYS;
    const DISTRICT: &[&str] = &[
        "type",
        "x",
        "y",
        "pillaged",
        "complete",
        // Hit points. `the_schema_allowlists_cover_every_declared_field` fails if
        // a StateDistrict field is missing here.
        "damage",
        "max_damage",
        "wall_damage",
        "max_wall_damage",
    ];
    const WONDER: &[&str] = &["type", "x", "y"];
    const WORKED: &[&str] = &["x", "y", "yields"];
    const GREAT_WORK: &[&str] = &["type", "object", "era", "creator", "building", "slot"];
    const YIELDS: &[&str] = &["food", "production", "gold", "science", "culture", "faith"];
    const EMERGENCY: &[&str] = &[
        "type",
        "name",
        "target",
        "target_city",
        "turns_left",
        "begun",
        "success",
        "members",
        "scores",
        "ours",
        "goals",
        "score_sources",
    ];
    const EMERGENCY_SCORE: &[&str] = &["player", "score", "tier"];
    const EMERGENCY_OURS: &[&str] = &["member", "target", "score", "tier"];
    const UNIT: &[&str] = UNIT_KEYS;
    const ROUTE: &[&str] = &[
        "trader",
        "origin",
        "destination",
        "destination_player",
        "origin_x",
        "origin_y",
        "destination_x",
        "destination_y",
        "posts_own",
        "posts_foreign",
        "yields",
    ];
    const GOVERNOR: &[&str] = &[
        "type",
        "city",
        "city_player",
        "x",
        "y",
        "established",
        "turns_on_site",
        "turns_to_establish",
        "neutralized_turns",
        "promotions",
    ];
    const RIVAL_ROUTE: &[&str] = &[
        "origin_x",
        "origin_y",
        "destination_x",
        "destination_y",
        "destination_player",
    ];
    const MINOR: &[&str] = &[
        "player",
        "civ",
        "score",
        "military",
        "at_war",
        "can_send_envoy",
        "suzerain",
        "envoys",
        "most_envoys",
        "cities",
        "units",
        "enforces_borders",
        // The city-state's request and every major's delegation (2026-08-26).
        "quests",
        "envoys_by_player",
    ];
    const QUEST: &[&str] = &["type", "target", "name"];
    const ENVOY_COUNT: &[&str] = &["player", "envoys"];
    const CLIMATE: &[&str] = &[
        "level",
        "temperature",
        "co2_total",
        "co2_ours",
        "sea_level_turns",
        "tiles_flooded",
        "storm_pct",
        "flood_pct",
        "drought_pct",
    ];
    const ROUTE_OPTION: &[&str] = &[
        "origin",
        "origin_x",
        "origin_y",
        "dest",
        "dest_player",
        "dest_x",
        "dest_y",
        "yields",
    ];
    const RELIGION: &[&str] = &["type", "founder", "beliefs"];

    fn cities(value: Option<&serde_json::Value>, gaps: &mut std::collections::BTreeSet<String>) {
        for city in value.and_then(|v| v.as_array()).into_iter().flatten() {
            keys(city, CITY, "city", gaps);
            for district in city
                .get("districts")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
            {
                keys(district, DISTRICT, "district", gaps);
            }
            for wonder in city
                .get("wonders")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
            {
                keys(wonder, WONDER, "wonder", gaps);
            }
            for plot in city
                .get("worked")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
            {
                keys(plot, WORKED, "worked", gaps);
            }
            for work in city
                .get("great_works")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
            {
                keys(work, GREAT_WORK, "great_work", gaps);
            }
            if let Some(yields) = city.get("yields") {
                keys(yields, YIELDS, "yields", gaps);
            }
        }
    }
    fn units(value: Option<&serde_json::Value>, gaps: &mut std::collections::BTreeSet<String>) {
        for unit in value.and_then(|v| v.as_array()).into_iter().flatten() {
            keys(unit, UNIT, "unit", gaps);
        }
    }
    fn public_stats(
        value: Option<&serde_json::Value>,
        path: &str,
        gaps: &mut std::collections::BTreeSet<String>,
    ) {
        if let Some(value) = value {
            keys(value, PUBLIC_STATS_KEYS, path, gaps);
        }
    }

    let mut gaps = std::collections::BTreeSet::new();
    keys(value, STATE, "state", &mut gaps);
    public_stats(value.get("public_stats"), "public_stats", &mut gaps);
    cities(value.get("cities"), &mut gaps);
    units(value.get("units"), &mut gaps);
    units(value.get("hostiles"), &mut gaps);
    for governor in value
        .get("governors")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
    {
        keys(governor, GOVERNOR, "governor", &mut gaps);
    }
    for route in value
        .get("trade_routes")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
    {
        keys(route, ROUTE, "trade_route", &mut gaps);
    }
    for rival in value
        .get("rivals")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
    {
        keys(rival, RIVAL_KEYS, "rival", &mut gaps);
        for declaration in rival
            .get("war_declarations")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
        {
            keys(
                declaration,
                &["statement", "allowed"],
                "war_declaration",
                &mut gaps,
            );
        }
        public_stats(rival.get("public_stats"), "rival.public_stats", &mut gaps);
        for reason in rival
            .get("opinion_modifiers")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
        {
            keys(
                reason,
                &["text", "score"],
                "rival.opinion_modifier",
                &mut gaps,
            );
        }
        for route in rival
            .get("trade_routes")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
        {
            keys(route, RIVAL_ROUTE, "rival.trade_route", &mut gaps);
        }
        cities(rival.get("cities"), &mut gaps);
        units(rival.get("units"), &mut gaps);
    }
    for minor in value
        .get("minors")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
    {
        keys(minor, MINOR, "minor", &mut gaps);
        cities(minor.get("cities"), &mut gaps);
        units(minor.get("units"), &mut gaps);
        for quest in minor
            .get("quests")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
        {
            keys(quest, QUEST, "minor.quest", &mut gaps);
        }
        for count in minor
            .get("envoys_by_player")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
        {
            keys(count, ENVOY_COUNT, "minor.envoys_by_player", &mut gaps);
        }
    }
    if let Some(climate) = value.get("climate") {
        keys(climate, CLIMATE, "climate", &mut gaps);
    }
    for option in value
        .get("route_options")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
    {
        keys(option, ROUTE_OPTION, "route_option", &mut gaps);
        if let Some(yields) = option.get("yields") {
            keys(yields, YIELDS, "route_option.yields", &mut gaps);
        }
    }
    for religion in value
        .get("religions")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
    {
        keys(religion, RELIGION, "religion", &mut gaps);
    }
    for emergency in value
        .get("emergencies")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
    {
        keys(emergency, EMERGENCY, "emergency", &mut gaps);
        for score in emergency
            .get("scores")
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
        {
            keys(score, EMERGENCY_SCORE, "emergency.score", &mut gaps);
        }
        if let Some(ours) = emergency.get("ours") {
            keys(ours, EMERGENCY_OURS, "emergency.ours", &mut gaps);
        }
    }
    gaps.into_iter().collect()
}

pub fn state_from_json(line: &str) -> serde_json::Result<StateSnapshot> {
    let value: serde_json::Value = serde_json::from_str(line)?;
    let mut state: StateSnapshot = serde_json::from_value(value.clone())?;
    state.schema_gaps = state_schema_gaps(&value);
    Ok(state)
}

pub fn state_from_events(path: &std::path::Path, turn: Option<u32>) -> Option<StateSnapshot> {
    let raw = read_events(path).ok()?;
    let mut best: Option<StateSnapshot> = None;
    let mut deaths = host_deaths::HostDeaths::default();
    // Identity rides in the `seat` event, which is emitted once at startup rather
    // than every turn, so it is collected separately and merged into whichever
    // state wins. Newest-wins here too: a run that reloads re-emits it.
    let mut seat: Option<Seat> = None;
    // Only a line naming `"combat"`, `"seat"` or `"state"` can do anything in
    // this loop (each branch below starts with that test), so walk just those
    // lines, in file order, instead of every line of a 100 MB log per frame.
    const COMBAT: u8 = 1;
    const SEAT: u8 = 2;
    const STATE: u8 = 4;
    let mut marked: std::collections::BTreeMap<(usize, usize), u8> = Default::default();
    for (needle, flag) in [
        ("\"combat\"", COMBAT),
        ("\"seat\"", SEAT),
        ("\"state\"", STATE),
    ] {
        for range in line_ranges_containing(&raw, needle) {
            *marked.entry(range).or_default() |= flag;
        }
    }
    for ((start, end), flags) in marked {
        let line = &raw[start..end];
        if flags & COMBAT != 0 && combat_line_may_match_in_log(&raw, (start, end)) {
            deaths.observe(line);
        }
        if flags & SEAT != 0 {
            if let Ok(found) = serde_json::from_str::<Seat>(line) {
                if !found.civ.is_empty() {
                    seat = Some(found);
                }
            }
        }
        if flags & STATE == 0 || !state_line_can_match_turn_in_log(&raw, (start, end), turn) {
            continue;
        }
        let Ok(mut state) = state_from_json(line) else {
            continue;
        };
        match turn {
            Some(want) if state.turn != want => continue,
            _ => {}
        }
        if best.as_ref().map(|b| state.turn >= b.turn).unwrap_or(true) {
            state.confirmed_unit_deaths = deaths.through(state.turn);
            best = Some(state);
        }
    }
    if let (Some(state), Some(seat)) = (best.as_mut(), seat) {
        state.seat = seat;
    }
    if let Some(state) = best.as_mut() {
        if state.dedication_choices.is_some_and(|choices| choices > 0) {
            state.observed_dedication_activity = dedication_history::through(&raw, state.turn);
        }
        state.refused_sites = refused_sites_of_kind_through(path, "found_refused", turn);
        (state.refused_improves, state.refused_improvement_names) =
            refused_improvements_through(path, turn);
        state.refused_trade_routes = refused_trade_routes_through(path, turn);
        state.refused_policy_names = refused_policies_through(path, turn);
        state.refused_pantheons = refused_pantheons_through(path, turn);
        state.refused_districts = refused_districts_through(path, turn);
        state.host_district_sites = host_district_sites_through(path, state.turn);
        state.host_wonder_sites = host_wonder_sites_through(path, state.turn);
        state.refused_wonders = refused_wonders_through(path, turn);
        state.host_unavailable_wonders = host_unavailable_wonders_through(path, Some(state.turn));
        state.refused_production = refused_production(path, state.turn);
        state.refused_purchases = refused_purchases(path, state.turn);
        state.refused_promotions = refused_promotions_through(path, turn);
        state.refused_strikes = refused_strikes_on(path, state.turn);
        state.host_previews = host_previews_on(path, state.turn);
    }
    best
}

/// The host's retired cards as CIVVIS spells them, dropping any it does not model.
///
/// Split out because both the rebuild and every `sync` need it and neither may guess
/// at a name: an unmatched entry in `blocked_policies` filters nothing while making
/// the set look populated.
fn blocked_policies_from(
    names: &std::collections::BTreeSet<String>,
    rules: &crate::rules::Rules,
) -> std::collections::BTreeSet<Name> {
    names
        .iter()
        .filter_map(|civ6| civvis_node_name(&rules.policies, civ6, "POLICY_"))
        .map(|name| Name::new(&name))
        .collect()
}

/// Translate the completed one-time projects from Civilization VI's project
/// table into the milestones CIVVIS models.
///
/// Civilization VI's base game represents the Mars launch as three independent
/// parts, whereas Gathering Storm replaces those with `PROJECT_LAUNCH_MARS_BASE`.
/// CIVVIS deliberately models one Mars-colony milestone, so base-game parts only
/// count once *all* three are complete. Treating one part as the whole colony
/// would make a nearly finished science victory look terminal; ignoring the three
/// parts makes the base game perpetually retry a finished program.
///
/// `None` preserves a persistent mirror built by an older control mod. An empty
/// vector is instead an authoritative statement that no milestones are complete.
fn completed_strategic_projects(
    civ6_projects: Option<&[String]>,
    unmapped: &mut Vec<String>,
) -> Option<BTreeSet<String>> {
    let civ6_projects = civ6_projects?;
    let reported: BTreeSet<&str> = civ6_projects.iter().map(String::as_str).collect();
    let mut completed = BTreeSet::new();

    let known = |project: &str| {
        matches!(
            project,
            "PROJECT_MANHATTAN_PROJECT"
                | "manhattan_project"
                | "PROJECT_OPERATION_IVY"
                | "operation_ivy"
                | "PROJECT_LAUNCH_EARTH_SATELLITE"
                | "launch_earth_satellite"
                | "PROJECT_LAUNCH_MOON_LANDING"
                | "launch_moon_landing"
                | "PROJECT_LAUNCH_MARS_BASE"
                | "PROJECT_LAUNCH_MARS_REACTOR"
                | "PROJECT_LAUNCH_MARS_HABITATION"
                | "PROJECT_LAUNCH_MARS_HYDROPONICS"
                | "launch_mars_colony"
                | "PROJECT_LAUNCH_EXOPLANET_EXPEDITION"
                | "exoplanet_expedition"
        )
    };
    for project in civ6_projects {
        if !known(project) {
            let issue = format!("science_project:{project}");
            if !unmapped.contains(&issue) {
                unmapped.push(issue);
            }
        }
    }

    for (civ6, civvis) in [
        ("PROJECT_MANHATTAN_PROJECT", "manhattan_project"),
        ("manhattan_project", "manhattan_project"),
        ("PROJECT_OPERATION_IVY", "operation_ivy"),
        ("operation_ivy", "operation_ivy"),
        ("PROJECT_LAUNCH_EARTH_SATELLITE", "launch_earth_satellite"),
        ("launch_earth_satellite", "launch_earth_satellite"),
        ("PROJECT_LAUNCH_MOON_LANDING", "launch_moon_landing"),
        ("launch_moon_landing", "launch_moon_landing"),
        (
            "PROJECT_LAUNCH_EXOPLANET_EXPEDITION",
            "exoplanet_expedition",
        ),
        ("exoplanet_expedition", "exoplanet_expedition"),
    ] {
        if reported.contains(civ6) {
            completed.insert(civvis.to_string());
        }
    }

    let base_game_mars = [
        "PROJECT_LAUNCH_MARS_REACTOR",
        "PROJECT_LAUNCH_MARS_HABITATION",
        "PROJECT_LAUNCH_MARS_HYDROPONICS",
    ];
    if reported.contains("PROJECT_LAUNCH_MARS_BASE")
        || reported.contains("launch_mars_colony")
        || base_game_mars
            .iter()
            .all(|project| reported.contains(project))
    {
        completed.insert("launch_mars_colony".to_string());
    }

    Some(completed)
}

/// Civilization VI's node name as CIVVIS spells it, or None if CIVVIS has no such node.
///
/// ⚠ THE TWO RULESETS DISAGREE ON ARTICLES. Civ 6's `TECH_THE_WHEEL` is CIVVIS's
/// `wheel`, so a straight prefix-strip produced `the_wheel`, which does not exist, and
/// **a completed technology silently failed to cross** — CIVVIS planned as though the
/// seat had never researched it. The mod hits the mirror image of this going the other
/// way and solves it by trimming; this trims the leading article.
///
/// Only ever removes, so it cannot invent a node: whatever it returns was already in
/// CIVVIS's own ruleset.
fn civvis_node_name<T>(
    table: &crate::specmap::SpecMap<T>,
    civ6: &str,
    prefix: &str,
) -> Option<String> {
    let base = civ6
        .strip_prefix(prefix)
        .unwrap_or(civ6)
        .to_ascii_lowercase();
    // Firaxis uses internal building-era or implementation names for several
    // entries whose visible names match CIVVIS. Keep this explicit: fuzzy matching
    // would silently turn future host content into the wrong rule node.
    if prefix == "BUILDING_" {
        let alias = match base.as_str() {
            "castle" => Some("medieval_walls"),
            "star_fort" => Some("renaissance_walls"),
            "museum_art" => Some("art_museum"),
            "museum_artifact" => Some("archaeological_museum"),
            "fossil_fuel_power_plant" => Some("oil_power_plant"),
            "power_plant" => Some("nuclear_power_plant"),
            // Expansion2's Congress popup uses the localized
            // `LOC_BUILDING_POWER_PLANT_EXPANSION2_NAME` key, while the
            // building row itself remains `BUILDING_POWER_PLANT`.
            "power_plant_expansion2" => Some("nuclear_power_plant"),
            "halicarnassus_mausoleum" => Some("mausoleum_at_halicarnassus"),
            "statue_liberty" => Some("statue_of_liberty"),
            "university_sankore" => Some("university_of_sankore"),
            "gov_tall" => Some("audience_chamber"),
            "gov_wide" => Some("ancestral_hall"),
            "gov_conquest" => Some("warlords_throne"),
            "gov_citystates" => Some("foreign_ministry"),
            "gov_faith" => Some("grand_masters_chapel"),
            "gov_spies" => Some("intelligence_agency"),
            "gov_culture" => Some("national_history_museum"),
            "gov_science" => Some("royal_society"),
            "gov_military" => Some("war_department"),
            _ => None,
        };
        if let Some(alias) = alias.filter(|alias| table.contains_key(alias)) {
            return Some(alias.to_string());
        }
    }
    // ★★★★ THE UNIQUE UNITS AND THE TWO WATER DISTRICTS, INBOUND. Civilization
    // VI files a civilization's unique unit under the civilization
    // (`UNIT_ROMAN_LEGION`) and the water districts by function
    // (`DISTRICT_WATER_ENTERTAINMENT_COMPLEX`); `civvis_orders::civ6_unit_type`
    // and `civ6_district_type` have carried the outbound spellings since #959,
    // and nothing carried them back. So a city building a Legion read as idle,
    // a refused Legion never reached `blocked_production`, and — now that
    // `Game::can_produce` gates on the host's exported menu — every one of
    // these would have been gated off every live board forever.
    // `civvis_orders` pins the round trip for every orderable name.
    if prefix == "UNIT_" {
        let alias = match base.as_str() {
            "phoenicia_bireme" => Some("bireme"),
            "byzantine_tagma" => Some("tagma"),
            "roman_legion" => Some("legion"),
            "portuguese_nau" => Some("nau"),
            "greek_hoplite" => Some("hoplite"),
            "aztec_eagle_warrior" => Some("eagle_warrior"),
            "sumerian_war_cart" => Some("war_cart"),
            "nubian_pitati" => Some("pitati_archer"),
            "egyptian_chariot_archer" => Some("maryannu_chariot_archer"),
            "scythian_horse_archer" => Some("saka_horse_archer"),
            "mongolian_keshig" => Some("keshig"),
            "polish_hussar" => Some("winged_hussar"),
            "ethiopian_oromo_cavalry" => Some("oromo_cavalry"),
            "colombian_llanero" => Some("llanero"),
            "maori_toa" => Some("toa"),
            "chinese_crouching_tiger" => Some("crouching_tiger"),
            "gaul_gaesatae" => Some("gaesatae"),
            "japanese_samurai" => Some("samurai"),
            "macedonian_hypaspist" => Some("hypaspist"),
            "indian_varu" => Some("varu"),
            "mali_mandekalu_cavalry" => Some("mandekalu_cavalry"),
            "russian_cossack" => Some("cossack"),
            "american_rough_rider" => Some("rough_rider"),
            "vietnamese_voi_chien" => Some("voi_chien"),
            "lahore_nihang" => Some("nihang"),
            "arabian_mamluk" => Some("mamluk"),
            "babylonian_sabum_kibittum" => Some("sabum_kibittum"),
            "brazilian_minas_geraes" => Some("minas_geraes"),
            "byzantine_dromon" => Some("dromon"),
            "canada_mountie" => Some("mountie"),
            "cree_okihtcitaw" => Some("okihtcitaw"),
            "norwegian_longship" => Some("viking_longship"),
            "english_seadog" => Some("sea_dog"),
            "english_redcoat" => Some("redcoat"),
            "french_garde_imperiale" => Some("garde_imperiale"),
            "georgian_khevsureti" => Some("khevsureti"),
            "german_uboat" => Some("u_boat"),
            "hungary_huszar" => Some("huszar"),
            "hungary_black_army" => Some("black_army"),
            "inca_warakaq" => Some("warakaq"),
            "indonesian_jong" => Some("jong"),
            "khmer_domrey" => Some("domrey"),
            "korean_hwacha" => Some("hwacha"),
            "macedonian_hetairoi" => Some("hetairoi"),
            "mapuche_malon_raider" => Some("malon_raider"),
            "mayan_hulche" => Some("hulche"),
            "norwegian_berserker" => Some("berserker"),
            "ottoman_barbary_corsair" => Some("barbary_corsair"),
            "suleiman_janissary" => Some("janissary"),
            "persian_immortal" => Some("immortal"),
            "scottish_highlander" => Some("highlander"),
            "spanish_conquistador" => Some("conquistador"),
            "sweden_carolean" => Some("carolean"),
            "zulu_impi" => Some("impi"),
            "american_p51" => Some("p51_mustang"),
            "antiair_gun" => Some("anti_air_gun"),
            _ => None,
        };
        if let Some(alias) = alias.filter(|alias| table.contains_key(alias)) {
            return Some(alias.to_string());
        }
    }
    if prefix == "DISTRICT_" {
        let alias = match base.as_str() {
            "water_entertainment_complex" => Some("water_park"),
            "water_street_carnival" => Some("copacabana"),
            _ => None,
        };
        if let Some(alias) = alias.filter(|alias| table.contains_key(alias)) {
            return Some(alias.to_string());
        }
    }
    if table.contains_key(&base) {
        return Some(base);
    }
    if let Some(without_article) = base.strip_prefix("the_") {
        if table.contains_key(without_article) {
            return Some(without_article.to_string());
        }
    }
    // ★★★★★ CIVILIZATION VI TRUNCATES WHERE CIVVIS SPELLS IT OUT, AND THAT COST TWO
    // SEPARATE BUGS.
    //
    // `DISTRICT_GOVERNMENT` is CIVVIS's `government_plaza`. Prefix-stripping gives
    // `government`, which is in no table, so this returned None — and both callers
    // then did the honest thing with a wrong answer:
    //
    // - `civvis_production_item` returned None, so a city BUILDING a Government Plaza
    //   read as idle and CIVVIS re-ordered it. Its own comment records the cost: **60
    //   `DISTRICT_GOVERNMENT` orders between t46 and t128**, sixty of that run's ~91
    //   build orders.
    // - the blocked-districts reader (#729) dropped the name, so the block it exists
    //   to apply never engaged for the one district that needed it. Measured after
    //   #729 shipped: `no_params_DISTRICT_GOVERNMENT` still **9**, when the whole
    //   claim was that it would be zero.
    //
    // So a Civilization VI name that is a proper prefix of exactly one CIVVIS name
    // resolves to it.
    //
    // ⚠ EXACTLY ONE, and only at a word boundary. `government` matches
    // `government_plaza` and nothing else; if two entries shared the stem this refuses
    // rather than picking one, because a confident wrong translation is what this
    // whole file exists to prevent. The boundary check stops `dam` matching `damascus`.
    let mut matches = table
        .keys()
        .filter(|known| {
            known
                .as_str()
                .strip_prefix(base.as_str())
                .is_some_and(|rest| rest.starts_with('_'))
        })
        .map(|known| known.as_str().to_string());
    let only = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(only)
}

/// What a reconstruction produced, including what it could NOT translate.
///
/// ⚠ `unmapped` is not decoration. A Civilization VI unit type with no CIVVIS
/// counterpart is a unit CIVVIS cannot see, and an army that is half-invisible
/// produces confident orders about the wrong battle. The caller reports it.
pub struct Reconstruction {
    pub game: crate::game::Game,
    /// CIVVIS unit id -> Civilization VI unit id, for translating orders back.
    pub unit_ids: std::collections::BTreeMap<u32, i64>,
    /// Civilization VI id -> board id for every FOREIGN unit planted (rival,
    /// city-state, barbarian, Free Cities). Kept apart from `unit_ids`, whose
    /// inverse the sync path prunes against `state.units` — our units only.
    pub foreign_unit_ids: std::collections::BTreeMap<i64, u32>,
    /// CIVVIS city id -> Civilization VI city id.
    pub city_ids: std::collections::BTreeMap<u32, i64>,
    /// Civilization VI city id -> CIVVIS city id for every city retained in the
    /// board memory.  Unlike `city_ids`, this includes visible rival cities so an
    /// outgoing international route can retain its destination.
    ///
    /// ⚠ Native city ids are unique only per player (our first city and every
    /// rival's first city are all 65536), so a colliding id keeps whichever
    /// owner was inserted last. Resolve our own cities by position or through
    /// `city_ids`, never through this map alone.
    pub known_city_ids: std::collections::BTreeMap<i64, u32>,
    pub placed_cities: usize,
    pub placed_units: usize,
    pub placed_rival_cities: usize,
    pub placed_rival_units: usize,
    /// City-state cities on the board. Counted apart from the rivals' so a
    /// missing minor reads as a mirror gap rather than vanishing into a total.
    pub placed_minor_cities: usize,
    pub unmapped: Vec<String>,
    /// Every unit the export named that did NOT end up on the board, with the reason.
    ///
    /// ⚠⚠ A unit the mirror drops is a unit CIVVIS never orders, and it then stands
    /// where it was built for the rest of the game. That is the operator's "units
    /// stacking up in the capital, unused", arriving by a route nobody had looked at,
    /// and `unmapped` could not show it: these are not translation failures.
    pub dropped_units: Vec<String>,
}

/// `UNIT_BATTERING_RAM` -> `battering_ram`. Mechanical, then CHECKED against the
/// ruleset — `spawn_unit` indexes `rules.units` and panics on a name it does not
/// have, so an unchecked guess would take the brain down mid-game.
fn civvis_unit_name(civ6: &str) -> String {
    let base = civ6
        .strip_prefix("UNIT_")
        .unwrap_or(civ6)
        .to_ascii_lowercase();
    // ★★★ BARBARIAN CAVALRY ARE NOT THE ORDINARY UNITS WITH A PREFIX.
    //
    // The installed game's Units.xml gives `UNIT_BARBARIAN_HORSEMAN` BaseMoves=3 /
    // Combat=20 and `UNIT_BARBARIAN_HORSE_ARCHER` BaseMoves=3 / Combat=10 /
    // RangedCombat=15. Ordinary Horsemen are BaseMoves=4 / Combat=36, while the
    // modeled Saka Horse Archer is BaseMoves=4 / Combat=20 / RangedCombat=25.
    // Collapsing the names therefore made the threat flood both too fast and too
    // strong. Keep exact CIVVIS specs for these two host-only variants; the other
    // barbarian-prefixed stock units still use their ordinary CIVVIS counterpart.
    let base = if matches!(
        base.as_str(),
        "barbarian_horseman" | "barbarian_horse_archer"
    ) {
        base
    } else {
        base.strip_prefix("barbarian_")
            .map(str::to_string)
            .unwrap_or(base)
    };
    match base.as_str() {
        // Firaxis's Scythian type name includes the civilization, whereas
        // CIVVIS stores the unit by its actual Saka name.
        "horse_archer" | "scythian_horse_archer" => "saka_horse_archer".to_string(),
        // Keep this in lockstep with `civ6_unit_type`: the host calls Pitati
        // Archers `NUBIAN_PITATI`, not `NUBIAN_PITATI_ARCHER`.
        "nubian_pitati" => "pitati_archer".to_string(),
        // Nihang is a Lahore suzerain unit rather than a civilization-unique
        // unit, so its CIVVIS row cannot be discovered through `unique_to`.
        "lahore_nihang" => "nihang".to_string(),
        // Firaxis retained Poland's implementation id after the unit's display
        // name became Winged Hussar.
        "polish_hussar" => "winged_hussar".to_string(),
        // Modeled since the unique-unit roster was completed from the shipped
        // database (docs/DECISIVE_WINDOWS.md §6). These three used to stand in
        // as their stock role; the Khevsureti's live export still carries no
        // `base` or `class`, so its explicit spelling stays here.
        "scottish_highlander" => "highlander".to_string(),
        "korean_hwacha" => "hwacha".to_string(),
        "georgian_khevsureti" => "khevsureti".to_string(),
        // Firaxis runs these nouns together, CIVVIS spells them out, and the
        // P-51 keeps its name: no qualifier-stripping can find them.
        "english_seadog" => "sea_dog".to_string(),
        "german_uboat" => "u_boat".to_string(),
        "american_p51" => "p51_mustang".to_string(),
        // Firaxis joins "antiair" in UNIT_ANTIAIR_GUN; the supported
        // rules row uses anti_air_gun, as production lookup already does.
        "antiair_gun" => "anti_air_gun".to_string(),
        _ => base,
    }
}

fn civvis_improvement_name(civ6: &str) -> String {
    let base = civ6
        .strip_prefix("IMPROVEMENT_")
        .unwrap_or(civ6)
        .to_ascii_lowercase();
    match base.as_str() {
        // The shipped type ids predate the final Civilopedia names.
        "beach_resort" => "seaside_resort".to_string(),
        "mountain_road" => "qhapaq_nan".to_string(),
        _ => base,
    }
}

/// The CIVVIS unit that stands in for a Civilization VI promotion class, for a
/// unique whose own name is unmodelled and that REPLACES nothing — `UnitReplaces`
/// has no row for a Malón Raider or a Nihang, so the `base` fallback one rung up
/// never fires for them. Varu is now modeled exactly and therefore no longer
/// reaches this class fallback.
///
/// Run `civvis-20260801T175955Z` was LOST at turn 140 with two
/// `UNIT_MAPUCHE_MALON_RAIDER` sitting two tiles from the final city, dropped as
/// untranslatable — the army that took the empire was invisible on the board.
/// An approximation understates a unique's strength and says so in
/// `dropped_units`; absence said nothing at all.
///
/// Candidates are ordered and the first one the LOADED ruleset has wins, so a
/// trimmed ruleset cannot make this invent a unit kind it does not model.
fn class_representative(class: &str, rules: &crate::rules::Rules) -> Option<&'static str> {
    let candidates: &[&str] = match class {
        "PROMOTION_CLASS_MELEE" => &["swordsman", "warrior"],
        "PROMOTION_CLASS_ANTI_CAVALRY" => &["spearman", "pikeman"],
        "PROMOTION_CLASS_LIGHT_CAVALRY" => &["horseman", "courser", "cavalry"],
        // Found the hard way: batch-4 attempt 1 dealt MONGOLIA, whose signature
        // Keshig is a standalone RANGED_CAVALRY unique — a class this table did
        // not carry, so it would have dropped exactly like the Malón Raider.
        "PROMOTION_CLASS_RANGED_CAVALRY" => &["saka_horse_archer", "courser", "horseman"],
        "PROMOTION_CLASS_HEAVY_CAVALRY" => &["knight", "heavy_chariot", "cuirassier"],
        "PROMOTION_CLASS_RANGED" => &["archer", "crossbowman", "slinger"],
        "PROMOTION_CLASS_SIEGE" => &["catapult", "trebuchet", "bombard"],
        "PROMOTION_CLASS_RECON" => &["scout", "skirmisher", "ranger"],
        "PROMOTION_CLASS_SKIRMISHER" => &["skirmisher", "ranger", "scout"],
        "PROMOTION_CLASS_NAVAL_MELEE" => &["galley", "caravel", "ironclad"],
        "PROMOTION_CLASS_NAVAL_RANGED" => &["quadrireme", "frigate"],
        "PROMOTION_CLASS_NAVAL_RAIDER" => &["privateer", "submarine"],
        "PROMOTION_CLASS_NAVAL_CARRIER" => &["aircraft_carrier"],
        "PROMOTION_CLASS_MONK" => &["warrior_monk"],
        "PROMOTION_CLASS_AIR_FIGHTER" => &["fighter", "biplane"],
        "PROMOTION_CLASS_AIR_BOMBER" => &["bomber"],
        "PROMOTION_CLASS_SUPPORT" => &["battering_ram", "siege_tower", "medic"],
        _ => &[],
    };
    candidates
        .iter()
        .copied()
        .find(|c| rules.units.contains_key(c))
}

/// A Civilization VI unit name with its owner qualifier removed, when that is what
/// stands between it and a name CIVVIS models.
///
/// ★★★ CIVILIZATION-UNIQUE UNITS CARRY THE CIV AS A PREFIX and CIVVIS stores the bare
/// name. `UNIT_AZTEC_EAGLE_WARRIOR` is `eagle_warrior`, which CIVVIS has — and without
/// this it was dropped from the board on **162 of the 240 turns** of run
/// `civvis-20260731T114437Z`, the dominant reason units went missing once the earlier
/// three routes were closed. A rival's unique unit is most of what a rival's army IS.
///
/// ⚠ A rename, not a substitution: the qualifier is the only difference. The caller
/// checks the result against the ruleset, so a name that still does not resolve is
/// reported in `dropped_units` rather than guessed at.
///
/// ⚠⚠ **THE PREFIX IS ASSUMED TO BE A CIVILIZATION AND NOTHING CHECKED IT.** Stripping
/// the first token unconditionally is only correct when that token really is an owner
/// qualifier. Eight units CIVVIS models have a tail that is a *different real unit* —
/// `jet_bomber`→`bomber`, `nuclear_submarine`→`submarine`, `line_infantry`→`infantry`,
/// `rocket_artillery`→`artillery`, `mechanized_infantry`, `jet_fighter`,
/// `pitati_archer`, `eagle_warrior` — so a Civilization VI name this bridge does not
/// model under its full form but whose tail collides would be planted as the WRONG
/// unit, silently, with **no entry in `dropped_units`**. That is strictly worse than
/// dropping it, because the drop detector is the only thing that would have caught it.
///
/// This is a syntax-only candidate. `resolved_civvis_unit_name` applies the
/// ruleset guard: the stripped destination must be a unit marked `unique_to`.
/// That is stronger than trusting the first token to be a civilization adjective
/// and prevents a trimmed or modded ruleset from turning `UNIT_JET_FIGHTER` into
/// the ordinary `fighter`, while keeping the known unique-unit spellings alive.
fn civvis_unit_name_unqualified(civ6: &str) -> Option<String> {
    let base = civ6.strip_prefix("UNIT_")?.to_ascii_lowercase();
    let (qualifier, rest) = base.split_once('_')?;
    if qualifier == GREAT_PERSON_PREFIX {
        return None;
    }
    (!rest.is_empty()).then(|| rest.to_string())
}

/// Resolve an exported unit through both its ordinary and civilization-qualified
/// spellings.  Construction and persistent sync must share this exact lookup;
/// otherwise a unique unit visible at startup vanishes on the next state update.
fn resolved_civvis_unit_name(rules: &crate::rules::Rules, civ6: &str) -> Option<String> {
    let direct = civvis_unit_name(civ6);
    if rules.units.contains_key(&direct) {
        return Some(direct);
    }
    let bare = civvis_unit_name_unqualified(civ6);
    if let Some(bare) = bare.as_deref().filter(|bare| {
        rules
            .units
            .get(bare)
            .is_some_and(|unit| unit.unique_to.is_some())
    }) {
        return Some(bare.to_string());
    }
    // ⚠ A UNIQUE UNIT WHOSE CIVVIS NAME CARRIES AN EPITHET.
    //
    // Civilization VI names uniques by CIVILIZATION — `UNIT_EGYPTIAN_CHARIOT_ARCHER`
    // — and stripping that qualifier gives `chariot_archer`, which is not what
    // CIVVIS calls it: `data/units.json` has **maryannu_chariot_archer**. Neither
    // spelling matches, so the unit resolved to nothing and vanished from the
    // board. Caught live by `civ6_mirror_check` on run `civvis-20260804T233745Z`:
    //
    //     UNITDATA ⚠ UNIT_EGYPTIAN_CHARIOT_ARCHER@(39, 24) count Civ6=1 CIVVIS=0
    //
    // An ENEMY unit CIVVIS cannot see is worse than a cosmetic gap: threat
    // assessment, settler safety and every tactical decision read a board with a
    // chariot archer missing from it.
    //
    // Rather than a hand-written table of host names — which would mean GUESSING
    // spellings for civilizations never yet observed — resolve by the noun: accept
    // the modelled unit whose name ENDS WITH the unqualified name. Exactly two
    // units in `data/units.json` need it (`maryannu_chariot_archer` and
    // `winged_hussar`), and only the Egyptian one has actually been seen.
    //
    // ⚠ Required to be UNAMBIGUOUS. If two modelled units share a suffix the
    // answer is refused, because a wrong unit on the board is worse than a
    // missing one — it would carry the wrong strength, movement and abilities.
    let bare = bare?;
    let suffix = format!("_{bare}");
    let mut matches = rules
        .units
        .keys()
        .filter(|name| name.as_str().ends_with(suffix.as_str()))
        .filter(|name| {
            rules
                .units
                .get(name.as_str())
                .is_some_and(|unit| unit.unique_to.is_some())
        });
    let only = matches.next()?;
    matches.next().is_none().then(|| only.to_string())
}

/// The one qualifier measured being mistaken for a civilization.
const GREAT_PERSON_PREFIX: &str = "great";

/// Whether Civilization VI's name is a Great Person.
///
/// ★★★★ **CIVVIS MODELS GREAT PEOPLE AS NAMED INDIVIDUALS, NOT AS UNITS.**
/// `data/great_people.json` holds 29 of them (`hypatia`, `isaac_newton`, …) while
/// `data/units.json` has 83 entries and none of `general`, `prophet`, `merchant`,
/// `scientist`, `engineer`, `admiral`, `artist`, `writer` or `musician`. So every
/// `UNIT_GREAT_*` standing on the board fails the ruleset lookup — 13 sightings of
/// `UNIT_GREAT_GENERAL` in run `civvis-20260731T114437Z`, beside 162 genuinely
/// untranslatable `UNIT_AZTEC_EAGLE_WARRIOR`.
///
/// ⚠ It is reported under its own reason rather than as `untranslatable` because the
/// two call for opposite work. An untranslatable name is a **bridge defect** — a unit
/// CIVVIS models under a name this code failed to produce. A Great Person is a
/// **modelling gap**: there is no name to produce, and no edit to this file will
/// create one. Counting them together is what let 13 real drops a run hide inside a
/// number that was being read as a translation score.
///
/// ⚠ The `UNIT_GREAT_*` prefix does not catch all of them. A civilization's unique
/// replacement for a Great Person keeps its own name: Gran Colombia's
/// `UNIT_COMANDANTE_GENERAL` is a Great General, granted free every era, and it was
/// being counted as a bridge defect on the live run `civvis-20260731T172058Z` —
/// `unmapped: UNIT_COMANDANTE_GENERAL`, which reads as "add a vocabulary entry" when
/// there is no entry to add. See [`GREAT_PERSON_UNIQUES`].
fn is_great_person(civ6: &str) -> bool {
    if GREAT_PERSON_UNIQUES.contains(&civ6) {
        return true;
    }
    civ6.strip_prefix("UNIT_")
        .map(|base| base.to_ascii_lowercase())
        .and_then(|base| {
            base.split_once('_')
                .map(|(qualifier, _)| qualifier == GREAT_PERSON_PREFIX)
        })
        .unwrap_or(false)
}

/// Great People whose Civilization VI name does not start with `UNIT_GREAT_`.
///
/// Only civilization-unique replacements land here, so the list is short and grows
/// one entry at a time as a run meets a new civilization. Anything on it is a
/// modelling gap, never a translation failure.
const GREAT_PERSON_UNIQUES: &[&str] = &["UNIT_COMANDANTE_GENERAL"];

/// Let pathfinding probe `depth` rings beyond the land the seat has seen.
///
/// ⚠⚠ THIS IS AN EXPLICIT PRIOR, not terrain. `apply_terrain` leaves the
/// undisclosed map UNKNOWN, which is honest for scoring but, while impassable, would
/// be catastrophic for deciding: a
/// seat that has revealed 51 plots sees a 51-tile island, so it has nowhere to settle,
/// nowhere to explore, and nothing worth building but soldiers. Measured: revealed
/// plots crawled 25 -> 150 over 104 turns, `met` stopped at 2, and ZERO rival cities
/// were ever seen.
///
/// The bounded flag means only "it is reasonable to try going there." The tile remains
/// `unknown`, carries no yields, and makes no land/water claim; both land and naval
/// explorers may test it. Each tile becomes real terrain only when revealed.
///
/// ★★★★★ THE SEA HAD NO FRONTIER, AND THE FLEET NEVER SAILED. The prior above is grown
/// from revealed LAND only, and it stops at every revealed tile — so on a coast the
/// seat has looked out over (a city sees its water three tiles out), the fog beyond
/// that water is reached from no land tile at all and stays `assumed_traversable =
/// false`. `Game::class_can_traverse` then answers "no" for a ship, `exploration_goal`
/// finds it nothing to sail toward, `naval_recon_can_chart_from` finds no fog at the
/// water's edge, and `BasicAi::naval_recon_is_the_missing_arm` reads the world as
/// charted. Measured across every live run of 2026-08-18 (25 runs, up to 251 turns
/// each): the sea-scout reservation fired ZERO times, ships built for the navy floor
/// "stand down; going nowhere" within a few turns, and run `civvis-20260818T225716Z`
/// reached t169 with 559 of 3404 plots revealed, two of five rivals met, and not one
/// hull ever laid down — Cartography and Square Rigging in hand.
///
/// So the sea gets its own prior: `assumed_navigable`, grown the same bounded way
/// from revealed WATER, read by ships alone (`come_ashore` keeps the land army out of
/// the water and could not do so for fog that has no domain yet if it shared the land
/// flag). Both flags may sit on one tile.
pub(crate) fn grow_frontier(game: &mut crate::game::Game, _snapshot: &Snapshot, depth: u32) {
    game.grow_player_frontier(depth);
}

/// Every site Civilization VI has refused to found a city on, in AXIAL coordinates.
///
/// ★★★★ THE CEILING WAS A FEEDBACK GAP. Peak city count was 2 in every run of the
/// ladder from t88 to t233. Run `230605Z` refused 18 `FOUND_CITY` orders while the
/// picker re-chose tile (18,29) at turns 20, 33 and 79; run `203028Z` refused 141.
/// Nothing carried a refusal back into the next decision, so each turn re-derived the
/// same site from the same board and re-sent the same rejected order — indefinitely.
///
/// ⚠ Reads the settler's ACTUAL position from the event, not the site CIVVIS aimed
/// at. Those differ whenever the settler has not arrived, and blocking the tile the
/// order named would blacklist good ground the settler simply had not reached.
fn refused_sites_of_kind_through(
    path: &std::path::Path,
    kind: &str,
    turn: Option<u32>,
) -> std::collections::BTreeSet<crate::Pos> {
    refusals_of_kind_through(path, kind, turn)
        .into_iter()
        .map(|(pos, _)| pos)
        .collect()
}

/// `improve_refused`, split: a refusal naming its improvement (`want`)
/// blocks that improvement on the tile; one without a name blocks the tile.
/// See `Game::blocked_improvements`.
fn refused_improvements_through(
    path: &std::path::Path,
    turn: Option<u32>,
) -> (
    std::collections::BTreeSet<crate::Pos>,
    std::collections::BTreeMap<crate::Pos, std::collections::BTreeSet<crate::name::Name>>,
) {
    let mut tiles = std::collections::BTreeSet::new();
    let mut named: std::collections::BTreeMap<
        crate::Pos,
        std::collections::BTreeSet<crate::name::Name>,
    > = Default::default();
    for (pos, want) in refusals_of_kind_through(path, "improve_refused", turn) {
        match want {
            Some(want) => {
                named
                    .entry(pos)
                    .or_default()
                    .insert(crate::name::Name::new(&civvis_improvement_name(&want)));
            }
            None => {
                tiles.insert(pos);
            }
        }
    }
    (tiles, named)
}

/// Every refusal of `kind` through `turn`, AXIAL, with the `want` it names.
fn refusals_of_kind_through(
    path: &std::path::Path,
    kind: &str,
    turn: Option<u32>,
) -> Vec<(crate::Pos, Option<String>)> {
    let mut refused = Vec::new();
    let Ok(raw) = read_events(path) else {
        return refused;
    };
    for line in lines_containing(&raw, kind) {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if event.get("kind").and_then(|k| k.as_str()) != Some(kind) {
            continue;
        }
        if turn.is_some_and(|limit| {
            event
                .get("turn")
                .and_then(|value| value.as_u64())
                .unwrap_or(0)
                > limit as u64
        }) {
            continue;
        }
        // ★★★★★ A TRANSIENT REFUSAL IS NOT A DEAD TILE.
        //
        // These sets feed `blocked_improvement_sites`, which is extended and
        // NEVER cleared, and `Game` skips any blocked position for the rest of
        // the game. So whatever lands here is a permanent verdict on that
        // ground.
        //
        // ⚠⚠⚠ THE MEASUREMENT THAT MOTIVATED THIS GUARD WAS WRONG, AND THE GUARD
        // IS THEREFORE INERT. Kept, corrected, and documented rather than
        // silently deleted, because the mistake is the useful part.
        //
        // The claim was: across run civvis-20260811T103914Z, builders had
        // `movesRemaining == 0` on 25 of 26 refusals, so the tiles were being
        // condemned for a condition that clears next turn. That number came from
        // matching each refusal against the STATE EXPORT by turn — and the state
        // snapshot is written at a different point in the turn than the refusal.
        //
        // Once #1548 put the reading in the event itself, taken by
        // `GetMovesRemaining()` at the instant of the attempt, the two disagree
        // flatly. Same turn, same unit, run civvis-20260811T134008Z:
        //
        //     turn 19  unit 327683   event moves 2   state moves 0
        //     turn 42  unit 851975   event moves 2   state moves 0
        //     turn 46  unit 983049   event moves 2   state moves 0
        //     ...  all 25 refusals: event moves 2, 3 or 4. NEVER zero.
        //
        // The event is the authoritative reading. So builders were NOT out of
        // moves, the refusals are genuine, and blocking those tiles was right all
        // along. This branch has never fired and on current evidence never will.
        //
        // ⭐ THE TRAP, so nobody repeats it: a per-turn state snapshot and an
        // event emitted during that turn are NOT the same instant. Matching them
        // by turn number reads as a measurement and is not one. Ask for the
        // reading at the point of the decision, or do not claim it.
        //
        // The guard stays because it is correct in principle — a unit with no
        // movement genuinely cannot act, and that genuinely would be transient —
        // and it costs one comparison. It is a safety net, not a repair.
        //
        // ⚠ An absent reading is still not evidence: events written before #1548
        // carry no `moves` and keep the old behaviour, so replaying an older run
        // is unchanged.
        if event.get("moves").and_then(|v| v.as_i64()) == Some(0) {
            continue;
        }
        let (Some(x), Some(y)) = (
            event.get("x").and_then(|v| v.as_i64()),
            event.get("y").and_then(|v| v.as_i64()),
        ) else {
            continue;
        };
        let want = event
            .get("want")
            .and_then(|v| v.as_str())
            .filter(|want| !want.is_empty())
            .map(str::to_string);
        refused.push((crate::hex::offset_to_axial(x as i32, y as i32), want));
    }
    refused
}

/// Promotions a host engine refused, keyed by Civilization VI unit id.
///
/// ★★★★★ Read from `promotion_refused`, which the mod emits when `CanPromote` says
/// no. Without this the ask returned every turn for the rest of the game: **411
/// refusals from 19 distinct (unit, promotion) pairs on 2026-08-03, median 13
/// retries, max 71**, 318 of them Apostles — which take one promotion at creation
/// and can never take another.
fn refused_promotions_through(
    path: &std::path::Path,
    turn: Option<u32>,
) -> std::collections::BTreeMap<i64, std::collections::BTreeSet<String>> {
    let mut refused: std::collections::BTreeMap<i64, std::collections::BTreeSet<String>> =
        Default::default();
    let Ok(raw) = read_events(path) else {
        return refused;
    };
    for line in lines_containing(&raw, "promotion_refused") {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if event.get("kind").and_then(|k| k.as_str()) != Some("promotion_refused") {
            continue;
        }
        if turn.is_some_and(|limit| {
            event
                .get("turn")
                .and_then(|value| value.as_u64())
                .unwrap_or(0)
                > limit as u64
        }) {
            continue;
        }
        let (Some(unit), Some(promotion)) = (
            event.get("unit").and_then(|v| v.as_i64()),
            event.get("promotion").and_then(|v| v.as_str()),
        ) else {
            continue;
        };
        refused
            .entry(unit)
            .or_default()
            .insert(promotion.to_string());
    }
    refused
}

/// Non-air strikes the host refused on exactly `turn`, keyed by Civilization VI unit id
/// and the offset plot they were aimed at.
///
/// ★★★ READ FROM TWO EVENTS THE MOD HAS EMITTED FOR MONTHS AND NOTHING IN RUST
/// CONSUMED. `range_attack_refused` (unit, x, y, moves, attacks, activity, why)
/// is the host declining a shot the simulator previewed — line of sight, range,
/// a spent attack; `war_refused` (unit, verb, x, y, players, target_owner) is
/// `refuseWarStarter` holding back an order the engine would answer with a war
/// the agent never declared. Both name the pair exactly, and until now the
/// same pair was re-proposed on the next frame of the same turn and refused
/// again. The `war_refused` a DeclareWar order raises carries `target` and no
/// `unit`/`x`/`y`, so it is not a strike and is skipped here.
fn refused_strikes_on(
    path: &std::path::Path,
    turn: u32,
) -> std::collections::BTreeSet<(i64, i32, i32)> {
    let mut refused = std::collections::BTreeSet::new();
    let Ok(raw) = read_events(path) else {
        return refused;
    };
    // Both streams feed a set, so a line naming both needles is harmless.
    // Reuse the log's indexes instead of scanning every past board again.
    for line in
        lines_containing(&raw, "range_attack_refused").chain(lines_containing(&raw, "war_refused"))
    {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if !matches!(
            event.get("kind").and_then(|k| k.as_str()),
            Some("range_attack_refused" | "war_refused")
        ) {
            continue;
        }
        // A first AIR_ATTACK refusal can succeed later in the SAME turn:
        // native 060034 t196 Bomber9306136 at33,16 was refused then hit.
        // The orders bridge supplies exact repeated-refusal air cooldowns;
        // do not turn this raw telemetry into a single-refusal blacklist.
        if event.get("verb").and_then(|value| value.as_str()) == Some("AIR_ATTACK") {
            continue;
        }
        if event.get("turn").and_then(|value| value.as_u64()) != Some(u64::from(turn)) {
            continue;
        }
        let (Some(unit), Some(x), Some(y)) = (
            event.get("unit").and_then(|v| v.as_i64()),
            event.get("x").and_then(|v| v.as_i64()),
            event.get("y").and_then(|v| v.as_i64()),
        ) else {
            continue;
        };
        refused.insert((unit, x as i32, y as i32));
    }
    refused
}

/// The host's `preview` answers on exactly `turn`, keyed by Civilization VI
/// unit id, the offset plot and the verb asked. A later answer for the same
/// key replaces an earlier one: the board may have moved between frames and
/// the newer simulation read the newer board.
///
/// The event is the mod's answer to a `preview` order: `{turn, frame, unit,
/// verb, x, y, preview{attacker_strength, defender_strength,
/// damage_to_attacker, damage_to_defender, defender_wall_damage}}`, the
/// `preview` table being `CivvisLedger.preview`'s reading of
/// `CombatManager.SimulateAttackInto`. A field the host could not read is
/// absent and lands as zero; an event with no `preview` table is skipped.
fn host_previews_on(
    path: &std::path::Path,
    turn: u32,
) -> std::collections::BTreeMap<(i64, i32, i32, String), crate::game::HostStrikePreview> {
    let mut previews = std::collections::BTreeMap::new();
    let Ok(raw) = read_events(path) else {
        return previews;
    };
    for line in lines_containing(&raw, "\"preview\"") {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if event.get("kind").and_then(|k| k.as_str()) != Some("preview") {
            continue;
        }
        if event.get("turn").and_then(|value| value.as_u64()) != Some(u64::from(turn)) {
            continue;
        }
        let (Some(unit), Some(x), Some(y), Some(verb), Some(preview)) = (
            event.get("unit").and_then(|v| v.as_i64()),
            event.get("x").and_then(|v| v.as_i64()),
            event.get("y").and_then(|v| v.as_i64()),
            event.get("verb").and_then(|v| v.as_str()),
            event.get("preview").filter(|v| v.is_object()),
        ) else {
            continue;
        };
        let float = |key: &str| preview.get(key).and_then(|v| v.as_f64()).unwrap_or(0.0);
        let whole = |key: &str| {
            preview
                .get(key)
                .and_then(|v| v.as_f64())
                .map_or(0, |v| v.round() as i32)
        };
        previews.insert(
            (unit, x as i32, y as i32, verb.to_string()),
            crate::game::HostStrikePreview {
                attacker_strength: float("attacker_strength"),
                defender_strength: float("defender_strength"),
                damage_to_attacker: whole("damage_to_attacker"),
                damage_to_defender: whole("damage_to_defender"),
                defender_wall_damage: whole("defender_wall_damage"),
            },
        );
    }
    previews
}

/// How many times the host must refuse the same origin/destination pair
/// before the mirror condemns it.
///
/// ★★★★★ ONE REFUSAL IS A REPORT; A VERDICT NEEDS TWO. `blocked_trade_routes`
/// carries the same contract as `blocked_improvement_sites` — extended and
/// NEVER cleared — so a single entry retires that pairing for the rest of the
/// game.
///
/// Live run `civvis-20260822T020434Z` finished with **three Traders parked in
/// Rome and a fourth elsewhere against a trade capacity of 20 with only 16
/// routes running**. Its refusal ledger is 23 distinct pairs, **every one of
/// them refused exactly once**, all between turns 183 and 223 — the window
/// where capacity climbed from 9 to 21 and the chooser was reaching for new
/// pairings. And **8 of the 15 condemned destinations are our OWN cities**, so
/// this is not a foreign-borders story: domestic pairings were retired on one
/// reading each and never tried again. The three parked Traders each received
/// their last order at turns 205, 221 and 222 and then nothing at all.
///
/// A refusal is a snapshot of one instant — a closed border, a war not yet
/// ended, a unit out of movement, a route slot filled that turn — and the
/// mod's own comment for this event names only the permanent case ("geometric
/// range is not a route"). Requiring corroboration keeps that permanent case,
/// which refuses again the moment it is retried, and costs exactly one extra
/// order for a transient one. The builder path met this same shape and got a
/// transient guard for it (`moves == 0`); the trade path never did, and cannot
/// use that one anyway because the mod sends no `moves` on this event.
const TRADE_ROUTE_REFUSALS_BEFORE_BLOCK: usize = 2;

fn refused_trade_routes_through(
    path: &std::path::Path,
    turn: Option<u32>,
) -> std::collections::BTreeSet<(crate::Pos, crate::Pos)> {
    let mut seen: std::collections::BTreeMap<(crate::Pos, crate::Pos), usize> = Default::default();
    let Ok(raw) = read_events(path) else {
        return Default::default();
    };
    for line in lines_containing(&raw, "trade_route_refused") {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if event.get("kind").and_then(|value| value.as_str()) != Some("trade_route_refused")
            || turn.is_some_and(|limit| {
                event
                    .get("turn")
                    .and_then(|value| value.as_u64())
                    .unwrap_or(0)
                    > limit as u64
            })
        {
            continue;
        }
        let values = ["from_x", "from_y", "x", "y"].map(|key| {
            event
                .get(key)
                .and_then(|value| value.as_i64())
                .map(|v| v as i32)
        });
        if let [Some(from_x), Some(from_y), Some(x), Some(y)] = values {
            *seen
                .entry((
                    crate::hex::offset_to_axial(from_x, from_y),
                    crate::hex::offset_to_axial(x, y),
                ))
                .or_default() += 1;
        }
    }
    // See `TRADE_ROUTE_REFUSALS_BEFORE_BLOCK`: a pairing is retired only once
    // the host has refused it more than once, because retiring it is forever.
    seen.into_iter()
        .filter(|(_, count)| *count >= TRADE_ROUTE_REFUSALS_BEFORE_BLOCK)
        .map(|(pair, _)| pair)
        .collect()
}

/// Translate host refusals onto CIVVIS's own city and district names.
///
/// Split out because the rebuild and every `sync` both need it, and neither may guess:
/// an entry under a name no district answers to filters nothing while making the set
/// look populated.
fn blocked_districts_from(
    refused: &std::collections::BTreeMap<i64, std::collections::BTreeSet<String>>,
    city_ids: &std::collections::BTreeMap<u32, i64>,
    rules: &crate::rules::Rules,
) -> BTreeMap<u32, std::collections::BTreeSet<Name>> {
    let mut out: BTreeMap<u32, std::collections::BTreeSet<Name>> = Default::default();
    for (cid, civ6_id) in city_ids {
        let Some(names) = refused.get(civ6_id) else {
            continue;
        };
        let translated: std::collections::BTreeSet<Name> = names
            .iter()
            .filter_map(|civ6| civvis_node_name(&rules.districts, civ6, "DISTRICT_"))
            .map(|name| Name::new(&name))
            .collect();
        if !translated.is_empty() {
            out.insert(*cid, translated);
        }
    }
    out
}

/// Translate current native limits; missing or invalid values leave no override.
fn host_district_capacity_from(
    cities: &[StateCity],
    city_ids: &BTreeMap<u32, i64>,
) -> BTreeMap<u32, usize> {
    let capacities: BTreeMap<_, _> = cities
        .iter()
        .filter_map(|city| {
            city.district_capacity
                .and_then(|value| usize::try_from(value).ok())
                .map(|value| (city.id, value))
        })
        .collect();
    city_ids
        .iter()
        .filter_map(|(cid, host_id)| capacities.get(host_id).map(|value| (*cid, *value)))
        .collect()
}

/// Translate fresh, host-approved district plots onto the reconstructed city ids.
///
/// A `build_no_plot` with `offered > 0` says the district is valid in this city,
/// but the direct CIVVIS order named a different coordinate. The companion
/// `offered_plots` list is Firaxis's authoritative replacement candidate set.
fn host_district_sites_from(
    offered: &BTreeMap<i64, BTreeMap<String, BTreeSet<crate::Pos>>>,
    city_ids: &BTreeMap<u32, i64>,
    rules: &crate::rules::Rules,
) -> BTreeMap<u32, BTreeMap<Name, BTreeSet<crate::Pos>>> {
    let mut out = BTreeMap::new();
    for (cid, civ6_id) in city_ids {
        let Some(by_district) = offered.get(civ6_id) else {
            continue;
        };
        let translated: BTreeMap<Name, BTreeSet<crate::Pos>> = by_district
            .iter()
            .filter_map(|(civ6, sites)| {
                civvis_node_name(&rules.districts, civ6, "DISTRICT_")
                    .map(|district| (Name::new(&district), sites.clone()))
            })
            .filter(|(_, sites)| !sites.is_empty())
            .collect();
        if !translated.is_empty() {
            out.insert(*cid, translated);
        }
    }
    out
}

/// Translate fresh, host-approved wonder plots onto the reconstructed city ids.
///
/// The event is structurally identical to a district placement disagreement, but
/// Firaxis names a wonder under `building`. The candidate set is intentionally
/// separate so a district name can never be interpreted against the wonder table.
fn host_wonder_sites_from(
    offered: &BTreeMap<i64, BTreeMap<String, BTreeSet<crate::Pos>>>,
    city_ids: &BTreeMap<u32, i64>,
    rules: &crate::rules::Rules,
) -> BTreeMap<u32, BTreeMap<Name, BTreeSet<crate::Pos>>> {
    let mut out = BTreeMap::new();
    for (cid, civ6_id) in city_ids {
        let Some(by_wonder) = offered.get(civ6_id) else {
            continue;
        };
        let translated: BTreeMap<Name, BTreeSet<crate::Pos>> = by_wonder
            .iter()
            .filter_map(|(civ6, sites)| {
                civvis_node_name(&rules.wonders, civ6, "BUILDING_")
                    .map(|wonder| (Name::new(&wonder), sites.clone()))
            })
            .filter(|(_, sites)| !sites.is_empty())
            .collect();
        if !translated.is_empty() {
            out.insert(*cid, translated);
        }
    }
    out
}

/// The wonder counterpart of `blocked_districts_from`, translated against the
/// wonder ruleset: a refused `BUILDING_` name that answers to no wonder here would
/// filter nothing while making the set look populated.
fn blocked_wonders_from(
    refused: &std::collections::BTreeMap<i64, std::collections::BTreeSet<String>>,
    city_ids: &std::collections::BTreeMap<u32, i64>,
    rules: &crate::rules::Rules,
) -> BTreeMap<u32, std::collections::BTreeSet<Name>> {
    let mut out: BTreeMap<u32, std::collections::BTreeSet<Name>> = Default::default();
    for (cid, civ6_id) in city_ids {
        let Some(names) = refused.get(civ6_id) else {
            continue;
        };
        let translated: std::collections::BTreeSet<Name> = names
            .iter()
            .filter_map(|civ6| civvis_node_name(&rules.wonders, civ6, "BUILDING_"))
            .map(|name| Name::new(&name))
            .collect();
        if !translated.is_empty() {
            out.insert(*cid, translated);
        }
    }
    out
}

/// Translate permanent host facts about world-unique wonders. Unlike
/// [`blocked_wonders_from`], no city id participates: an explicit zero-target
/// answer says the wonder cannot start in any city.
fn host_unavailable_wonders_from(
    unavailable: &std::collections::BTreeSet<String>,
    rules: &crate::rules::Rules,
) -> std::collections::BTreeSet<Name> {
    unavailable
        .iter()
        .filter_map(|civ6| civvis_node_name(&rules.wonders, civ6, "BUILDING_"))
        .map(|name| Name::new(&name))
        .collect()
}

/// Translate current native Rock Band and Apostle choices onto mirrored unit
/// ids. Both draw a limited menu the simulator's own draw cannot predict.
fn host_band_promotions_from(
    state: &StateSnapshot,
    unit_ids: &BTreeMap<u32, i64>,
    rules: &crate::rules::Rules,
) -> BTreeMap<u32, crate::game::HostBandPromotions> {
    unit_ids
        .iter()
        .filter_map(|(uid, native)| {
            let unit = state.units.iter().find(|unit| unit.id == *native)?;
            let class = match unit.kind.as_str() {
                "UNIT_ROCK_BAND" => "rock_band",
                "UNIT_APOSTLE" => "religious_apostle",
                _ => return None,
            };
            let offered = unit.offered_promotions.as_ref()?;
            let names = offered
                .iter()
                .map(|name| civvis_unit_promotion_name(name))
                .filter(|name| {
                    rules
                        .promotions
                        .get(name)
                        .is_some_and(|spec| spec.class == class)
                })
                .map(|name| Name::new(&name))
                .collect();
            let held = unit
                .promotions
                .as_ref()?
                .iter()
                .map(|name| civvis_unit_promotion_name(name))
                .filter(|name| rules.promotions.contains_key(name))
                .map(|name| Name::new(&name))
                .collect();
            Some((
                *uid,
                crate::game::HostBandPromotions {
                    held,
                    offered: names,
                },
            ))
        })
        .collect()
}

/// Translate refusals per unit: another band may have a different legal menu.
fn blocked_promotions_from(
    refused: &std::collections::BTreeMap<i64, std::collections::BTreeSet<String>>,
    unit_ids: &std::collections::BTreeMap<u32, i64>,
    rules: &crate::rules::Rules,
) -> BTreeMap<u32, std::collections::BTreeSet<crate::name::Name>> {
    let mut out: BTreeMap<u32, std::collections::BTreeSet<crate::name::Name>> = Default::default();
    for (uid, civ6_id) in unit_ids {
        let Some(names) = refused.get(civ6_id) else {
            continue;
        };
        // ⚠ TRANSLATE, do not intern the host name. Civilization VI says
        // `PROMOTION_TRANSLATOR`; CIVVIS's rules call it `translator`, and
        // `available_promotions` compares CIVVIS names. Interning the raw host name
        // produced a block set that was correctly populated and matched nothing —
        // the gate measured 153 promotion orders before and 153 after.
        // A Spy's carries a second prefix, `PROMOTION_SPY_TECHNOLOGIST`, which
        // the plain strip leaves as `spy_technologist`: no rule of that name,
        // so a refused Spy promotion was never blocked. The unit-promotion
        // mapper strips it, as it does for the observed promotions.
        let translated: std::collections::BTreeSet<crate::name::Name> = names
            .iter()
            .filter_map(|name| {
                civvis_node_name(&rules.promotions, name, "PROMOTION_").or_else(|| {
                    let mapped = civvis_unit_promotion_name(name);
                    rules.promotions.contains_key(&mapped).then_some(mapped)
                })
            })
            .map(|name| crate::name::Name::new(&name))
            .collect();
        if !translated.is_empty() {
            out.insert(*uid, translated);
        }
    }
    out
}

/// Translate this turn's host strike refusals onto CIVVIS unit ids and axial
/// tiles. A refusal for a unit the board does not carry is dropped: there is
/// no attacker to gate.
fn blocked_strikes_from(
    refused: &std::collections::BTreeSet<(i64, i32, i32)>,
    unit_ids: &std::collections::BTreeMap<u32, i64>,
) -> BTreeSet<(u32, crate::Pos)> {
    if refused.is_empty() {
        return BTreeSet::new();
    }
    let by_host: BTreeMap<i64, u32> = unit_ids.iter().map(|(uid, civ6)| (*civ6, *uid)).collect();
    refused
        .iter()
        .filter_map(|(civ6, x, y)| {
            let uid = *by_host.get(civ6)?;
            Some((uid, crate::hex::offset_to_axial(*x, *y)))
        })
        .collect()
}

/// Translate this turn's host strike previews onto CIVVIS unit ids and axial
/// tiles, `ranged` standing for the RANGE_ATTACK verb. A preview for a unit
/// the board does not carry is dropped; a verb that is neither strike is not
/// a preview the board can file.
fn host_previews_from(
    previews: &std::collections::BTreeMap<(i64, i32, i32, String), crate::game::HostStrikePreview>,
    unit_ids: &std::collections::BTreeMap<u32, i64>,
) -> BTreeMap<(u32, crate::Pos, bool), crate::game::HostStrikePreview> {
    if previews.is_empty() {
        return BTreeMap::new();
    }
    let by_host: BTreeMap<i64, u32> = unit_ids.iter().map(|(uid, civ6)| (*civ6, *uid)).collect();
    previews
        .iter()
        .filter_map(|((civ6, x, y, verb), preview)| {
            let ranged = match verb.as_str() {
                "RANGE_ATTACK" => true,
                "ATTACK" => false,
                _ => return None,
            };
            let uid = *by_host.get(civ6)?;
            Some(((uid, crate::hex::offset_to_axial(*x, *y), ranged), *preview))
        })
        .collect()
}

/// The typed key the board files a host production item under — the
/// `Game::production_block_key` vocabulary the refusal sets already use
/// (`unit:warrior`, `formation:warrior:1`, `building:library`,
/// `wonder:pyramids`, `district:campus`, `project:campus_research_grants`).
/// A district needs no plot here; a Corps/Army row carries its tier. `None`
/// for a host name CIVVIS does not model.
///
/// ⚠ This is the one translation the positive gate in `Game::can_produce`
/// rests on: an item the board can ORDER whose host spelling does not come
/// back to the same key would be gated off every live board forever.
/// `civvis_orders` pins the round trip for every orderable item.
pub fn host_production_key(
    rules: &crate::rules::Rules,
    civ6: &str,
    formation: Option<u8>,
) -> Option<String> {
    if let Some(item) = civvis_production_item(rules, Some(civ6), &[], None) {
        return Some(match (&item, formation) {
            (crate::game::Item::Unit { unit }, Some(tier @ 1..=2)) => {
                format!("formation:{unit}:{tier}")
            }
            _ => crate::game::Game::production_block_key(&item),
        });
    }
    civvis_node_name(&rules.districts, civ6, "DISTRICT_")
        .map(|district| format!("district:{district}"))
}

/// The host's menus translated onto the board, per CIVVIS city id. See
/// [`crate::game::Game::host_buildable`].
#[derive(Default)]
pub(crate) struct HostMenus {
    pub buildable: BTreeMap<u32, BTreeMap<String, crate::game::HostMenuEntry>>,
    pub unit_resource_prices: BTreeMap<u32, BTreeMap<String, f64>>,
    pub purchasable: BTreeMap<u32, BTreeMap<String, crate::game::HostPurchaseEntry>>,
    pub plot_purchases: BTreeMap<u32, Vec<crate::game::HostPlotPurchaseEntry>>,
    pub district_plots: BTreeMap<u32, BTreeMap<crate::name::Name, BTreeSet<crate::Pos>>>,
    /// See [`crate::game::Game::host_saved_work`].
    pub saved_work: BTreeMap<u32, BTreeMap<String, f64>>,
}

fn host_menus_from(
    cities: &[StateCity],
    city_ids: &BTreeMap<u32, i64>,
    rules: &crate::rules::Rules,
) -> HostMenus {
    let reading = |value: f64| (value.is_finite() && value >= 0.0).then_some(value);
    let mut out = HostMenus::default();
    for (cid, civ6_id) in city_ids {
        let Some(city) = cities.iter().find(|city| city.id == *civ6_id) else {
            continue;
        };
        if let Some(menu) = city.buildable.as_deref() {
            let mut translated = BTreeMap::new();
            let mut resource_prices = BTreeMap::new();
            let mut plots: BTreeMap<crate::name::Name, BTreeSet<crate::Pos>> = BTreeMap::new();
            let mut saved = BTreeMap::new();
            for row in menu {
                let Some(key) = host_production_key(rules, &row.t, row.f) else {
                    continue;
                };
                // Saved work on a paused unit, building or project. A district's
                // belongs to its foundation (`apply_host_district_progress`); a
                // Wonder's and a formation's key names no plot or tier.
                if key.starts_with("unit:")
                    || key.starts_with("building:")
                    || key.starts_with("project:")
                {
                    if let Some(progress) = row.pr.and_then(reading).filter(|p| *p > 0.0) {
                        saved.insert(key.clone(), progress);
                    }
                }
                if key.starts_with("unit:") || key.starts_with("formation:") {
                    if let Some(price) = row.r.and_then(reading) {
                        resource_prices.insert(key.clone(), price);
                    }
                }
                if let Some(district) = key.strip_prefix("district:") {
                    // Only a COMPLETE offer can say a plot is not legal; a
                    // capped list says only where some of them are.
                    if let (Some(sites), Some(offered)) = (row.s.as_ref(), row.n) {
                        if offered >= 0 && offered as usize == sites.len() {
                            plots.insert(
                                crate::name::Name::new(district),
                                sites
                                    .iter()
                                    .map(|plot| crate::hex::offset_to_axial(plot.x, plot.y))
                                    .collect(),
                            );
                        }
                    }
                }
                translated.insert(
                    key,
                    crate::game::HostMenuEntry {
                        cost: reading(row.c),
                        turns: reading(row.p),
                    },
                );
            }
            // ⚠ An empty translated menu is a read that failed or a ruleset the
            // board cannot name, not a city that can build nothing: a
            // Civilization VI city can always train something. No gate then.
            if !translated.is_empty() {
                out.buildable.insert(*cid, translated);
            }
            if !resource_prices.is_empty() {
                out.unit_resource_prices.insert(*cid, resource_prices);
            }
            if !saved.is_empty() {
                out.saved_work.insert(*cid, saved);
            }
            if !plots.is_empty() {
                out.district_plots.insert(*cid, plots);
            }
        }
        if let Some(menu) = city.plot_purchases.as_deref() {
            let translated = menu
                .iter()
                .filter_map(|row| {
                    Some(crate::game::HostPlotPurchaseEntry {
                        pos: crate::hex::offset_to_axial(row.x?, row.y?),
                        gold: reading(row.g?)?,
                        can_have_acropolis: row.can_have_acropolis,
                    })
                })
                .collect();
            out.plot_purchases.insert(*cid, translated);
        }
        if let Some(menu) = city.purchasable.as_deref() {
            let translated: BTreeMap<String, crate::game::HostPurchaseEntry> = menu
                .iter()
                .filter_map(|row| {
                    host_production_key(rules, &row.t, None).map(|key| {
                        (
                            key,
                            crate::game::HostPurchaseEntry {
                                gold: row.g.and_then(reading),
                                faith: row.f.and_then(reading),
                            },
                        )
                    })
                })
                .collect();
            out.purchasable.insert(*cid, translated);
        }
    }
    out
}

/// The queue behind the head, translated. A district row has no plot until it
/// is the head (`ProductionHelper.lua`: "invalid coordinates for everything
/// but the head node"), so it needs one of the city's placed districts to name
/// it and is otherwise dropped rather than planted on invented ground.
fn host_queue_tail(rules: &crate::rules::Rules, city: &StateCity) -> Vec<crate::game::Item> {
    let centre = Some(crate::hex::offset_to_axial(city.x, city.y));
    city.queue
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .filter_map(|row| {
            let item = civvis_production_item(rules, Some(&row.t), &city.districts, centre)?;
            Some(match (item, row.f) {
                (crate::game::Item::Unit { unit }, Some(tier @ 1..=2)) => {
                    crate::game::Item::Formation {
                        unit,
                        formation: tier,
                    }
                }
                (item, _) => item,
            })
        })
        .collect()
}

fn blocked_production_from(
    refused: &std::collections::BTreeMap<i64, std::collections::BTreeSet<String>>,
    city_ids: &std::collections::BTreeMap<u32, i64>,
    rules: &crate::rules::Rules,
) -> BTreeMap<u32, std::collections::BTreeSet<String>> {
    let mut out: BTreeMap<u32, std::collections::BTreeSet<String>> = Default::default();
    for (cid, civ6_id) in city_ids {
        let Some(names) = refused.get(civ6_id) else {
            continue;
        };
        let translated: std::collections::BTreeSet<String> = names
            .iter()
            .filter_map(|name| host_production_key(rules, name, None))
            .collect();
        if !translated.is_empty() {
            out.insert(*cid, translated);
        }
    }
    out
}

/// Civilization VI grants Spies through civics and governments; cities cannot train them.
///
/// CIVVIS models Spies as ordinary units for standalone simulations, so keep that model
/// intact and block the host-only mismatch only on reconstructed live boards.
fn block_live_spy_production(game: &mut crate::game::Game, capacity: Option<i64>) {
    // ★★★★ THE BLANKET BLOCK IS WHY THE SEAT HAS NEVER HELD A SPY, AND WITH
    // IT THE WHOLE ESPIONAGE LAYER WAS DEAD.
    //
    // The block was a fair response to a real number — `UNIT_SPY` was the
    // second most-requested production item in the fleet, 550 of 5,618 orders
    // (9.8%), 84% refused as unplayable — but it treats "cannot right now" as
    // "cannot ever". Civilization VI trains Spies in cities like any other
    // unit (`Units.xml` gives `UNIT_SPY` a Cost of 225 and no purchase-only
    // flag); what gates them is CAPACITY, and the refusals are what ordering
    // past a full or unearned capacity looks like. Measured over twelve
    // completed live games the seat finished holding the Diplomatic Service
    // civic in **12 of 12** and fielded **zero** Spies, because this function
    // refused every order before the host ever saw it.
    //
    // With `spy_capacity` on the wire the block becomes what it should always
    // have been: refuse only when the empire is already at its limit. An older
    // mod that cannot report capacity keeps the old unconditional behaviour,
    // so this cannot loosen a bridge that has not been taught to measure it.
    let held = game
        .spies
        .values()
        .filter(|spy| spy.owner == 0 && spy.captured_by.is_none())
        .count() as i64;
    let room = capacity.is_some_and(|capacity| held < capacity);
    let spy = crate::game::Item::Unit {
        unit: crate::name!("spy"),
    };
    let key = crate::game::Game::production_block_key(&spy);
    let mut blocked = Arc::unwrap_or_clone(std::mem::take(&mut game.blocked_production));
    for city in game.player_city_ids(0) {
        let entry = blocked.entry(city).or_default();
        if room {
            entry.remove(&key);
        } else {
            entry.insert(key.clone());
        }
    }
    // Replacing rather than mutating directly also invalidates a previously cached menu.
    game.replace_blocked_production(blocked);
}

/// Seat the empire's live Spies in `Game::spies`, which is otherwise empty for
/// the whole of a live game.
///
/// ⚠⚠ TWO REPRESENTATIONS, AND ONLY ONE OF THEM DRIVES THE AI. A native CIVVIS
/// Spy is a `Game::spies` entry and nothing on the map; Civilization VI's is an
/// ordinary `UNIT_SPY` this mirror imports as a unit. `AdvancedAi::advanced_spies`
/// and `BasicAi::spies` both iterate `Game::spies`, so until it is filled the
/// entire espionage layer — twelve missions, promotion priorities per grand
/// strategy, and a +90 weight on the denial target — cannot see a single agent
/// and is a guaranteed no-op.
///
/// The unit id is reused as the spy id so an order can be translated straight
/// back onto the unit it came from. `city` is the city the agent is standing
/// in, which is what the host's own missions are aimed from.
///
/// ★★★★ A LIVE SPY'S PROMOTION ENTITLEMENT IS `level − 1`, NOT `level`. The
/// native rule (`spy_needs_promotion`) owes a Spy one promotion per level, so
/// a freshly trained native Spy — level 1, none chosen — picks immediately.
/// Civilization VI grants the FIRST promotion at level 2: a new Spy has
/// nothing to pick until a mission levels it. Seating the host's level
/// unshifted made every fresh live Spy permanently "promotable", and
/// `legal_spy_actions` returns promotions as the ONLY legal actions while one
/// is owed — so the whole mission layer was unreachable, not merely
/// deprioritised. Measured on run civvis-20260818T095712Z: spy 2621443 was
/// sent the same promotion for 73 consecutive turns (t116–t188), the seat's
/// four spies drew 129 promotion orders between them, and not one
/// `SPY_TRAVEL_NEW_CITY` or mission order crossed the bridge in any run on
/// record. Seat the Spy at `host level − 1` so the mirror owes it exactly
/// what the host would let it choose; a genuinely levelled Spy still gets
/// its offer, named per #2012.
///
/// ★★ AND THE CITY IT STANDS IN MUST MATCH RIVAL CITIES TOO. Offensive
/// missions are generated only for `spy.city` (`spy_operation_actions`), and
/// `advanced_spies` treats a Spy as offensive only when that city has a rival
/// owner — so matching `unit.pos` against OUR cities alone meant a Spy that
/// completed its travel imported with `city: None` and could never be handed
/// the mission it travelled for. Match whatever city stands on the tile; the
/// counterspy branch already guards on ownership itself.
/// A host spy operation type as the CIVVIS mission kind:
/// `UNITOPERATION_SPY_GAIN_SOURCES` is `gain_sources`, the exact inverse of
/// the bridge's `SPY_{mission.to_uppercase()}` verb. The shipped
/// `UnitOperations` table carries fourteen `UNITOPERATION_SPY_*` rows; thirteen
/// strip to a kind `Game::spy_operation_actions` names and the fourteenth,
/// `travel_new_city`, is the travel itself, which blocks re-tasking the same
/// way a mission does.
fn civvis_spy_mission_kind(operation: &str) -> String {
    operation
        .strip_prefix("UNITOPERATION_SPY_")
        .or_else(|| operation.strip_prefix("UNITOPERATION_"))
        .unwrap_or(operation)
        .to_ascii_lowercase()
}

/// Record what the host said about one visible unit under the board's unit id —
/// see `Game::host_unit_facts` for who reads which. The stable Civ 6 id is
/// retained even when the older mod carries none of the optional fact keys:
/// live decision memory must survive the fresh-board rebuild and cannot key a
/// last-seen hostile by the transient CIVVIS id. Optional absent members remain
/// real absences — no successor, no operation. Foreign units use this primarily
/// for the host's movement allowance in threat floods.
fn record_host_unit_facts(game: &mut crate::game::Game, uid: u32, unit: &StateUnit) {
    let finite = |value: Option<f64>| value.filter(|value| value.is_finite());
    let exported = unit.concert_plots.is_some()
        || (unit.kind == "UNIT_ROCK_BAND" && unit.rock_band_level.is_some())
        || unit.upgrade_to.is_some()
        || unit.upgrade_cost.is_some()
        || unit.upgrade_resource_cost.is_some()
        || unit.upgrade_blocked_reason.is_some()
        || unit.maintenance.is_some()
        || unit.religious_strength.is_some()
        || unit.max_moves.is_some()
        || unit.activity.is_some()
        || unit.spy_operation.is_some()
        || unit.spy_operation_end_turn.is_some()
        || unit.spy_missions_available.is_some()
        || unit.spy_mission_targets.is_some()
        || (unit.kind == "UNIT_SPY" && unit.offered_promotions.is_some());
    if !exported {
        Arc::make_mut(&mut game.host_unit_facts).insert(
            uid,
            crate::game::HostUnitFacts {
                civ6_id: Some(unit.id),
                ..Default::default()
            },
        );
        return;
    }
    let upgrade = (unit.upgrade_to.is_some() || unit.upgrade_blocked_reason.is_some()).then(|| {
        crate::game::HostUnitUpgrade {
            // The successor in CIVVIS's vocabulary, through the same resolver
            // the unit itself crossed by; a successor CIVVIS does not model
            // leaves `None` and the board prices its own.
            to: unit
                .upgrade_to
                .as_deref()
                .and_then(|to| resolved_civvis_unit_name(&game.rules, to))
                .map(|name| Name::new(&name)),
            cost: finite(unit.upgrade_cost).filter(|cost| *cost >= 0.0),
            resources: finite(unit.upgrade_resource_cost)
                .filter(|cost| *cost >= 0.0)
                .and_then(|cost| {
                    let resource = match unit.upgrade_resource.as_deref() {
                        Some(raw) => match Vocabulary::embedded().resource(raw) {
                            Resolved::Known(resource) => Some(resource),
                            _ => return None,
                        },
                        None if cost == 0.0 => None,
                        None => return None,
                    };
                    Some(crate::game::HostUnitUpgradeResource { resource, cost })
                }),
            blocked: unit
                .upgrade_blocked_reason
                .clone()
                .filter(|reason| !reason.is_empty()),
        }
    });
    let spy_missions = unit
        .spy_missions_available
        .as_ref()
        .map(|menu| {
            menu.iter()
                .map(|operation| civvis_spy_mission_kind(operation))
                .collect::<BTreeSet<String>>()
        })
        .filter(|menu| !menu.is_empty());
    // Each offered mission's target plot and odds, keyed by CIVVIS mission
    // kind; odds outside [0, 1] are no reading.
    let odds = |value: Option<f64>| finite(value).filter(|odds| (0.0..=1.0).contains(odds));
    let spy_targets = unit
        .spy_mission_targets
        .as_ref()
        .map(|rows| {
            rows.iter()
                .map(|row| {
                    (
                        civvis_spy_mission_kind(&row.op),
                        crate::game::HostSpyTarget {
                            pos: crate::hex::offset_to_axial(row.x, row.y),
                            success: odds(row.success),
                            fail: odds(row.fail),
                        },
                    )
                })
                .collect::<BTreeMap<String, crate::game::HostSpyTarget>>()
        })
        .filter(|targets| !targets.is_empty());
    // A Spy's promotion menu, as the host offers it, in CIVVIS names; a name
    // outside the Spy tree is no choice of its.
    let spy_promotions = (unit.kind == "UNIT_SPY")
        .then_some(unit.offered_promotions.as_ref())
        .flatten()
        .map(|menu| {
            menu.iter()
                .map(|name| civvis_unit_promotion_name(name))
                .filter(|name| crate::game::Game::SPY_PROMOTIONS.contains(&name.as_str()))
                .map(|name| Name::new(&name))
                .collect::<BTreeSet<Name>>()
        });
    Arc::make_mut(&mut game.host_unit_facts).insert(
        uid,
        crate::game::HostUnitFacts {
            civ6_id: Some(unit.id),
            concert_plots: unit.concert_plots.as_ref().map(|plots| {
                plots
                    .iter()
                    .map(|plot| crate::hex::offset_to_axial(plot.x, plot.y))
                    .collect()
            }),
            rock_band_level: (unit.kind == "UNIT_ROCK_BAND")
                .then_some(unit.rock_band_level)
                .flatten()
                .filter(|level| *level >= 0),
            upgrade,
            maintenance: finite(unit.maintenance).filter(|bill| *bill >= 0.0),
            religious_strength: finite(unit.religious_strength),
            max_moves: finite(unit.max_moves),
            activity: unit.activity.clone(),
            spy_operation: unit.spy_operation.as_deref().map(civvis_spy_mission_kind),
            spy_operation_ends: unit
                .spy_operation_end_turn
                .filter(|turn| *turn >= 0)
                .map(|turn| turn as u32),
            spy_missions,
            spy_targets,
            spy_promotions,
        },
    );
}

/// A rival Spy is not revealed merely because its city tile is visible.
///
/// The live exporter reads a rival's engine roster, where hidden spies are
/// present so their operations can run. Keep this boundary guard as well so a
/// stale or malformed state file cannot turn tile sight into agent detection.
fn foreign_spy_is_hidden(owner: usize, name: &str) -> bool {
    owner != 0 && name == "spy"
}

/// One seat 0 Spy as `seat_live_spies` reads it off the mirrored unit.
struct LiveSpySeat {
    id: u32,
    level: i64,
    promotions: std::collections::BTreeSet<String>,
    city: Option<u32>,
    mission: Option<crate::game::SpyMission>,
    ready_turn: u32,
}

fn seat_live_spies(game: &mut crate::game::Game, state: &StateSnapshot) {
    game.spies.retain(|_, spy| spy.owner != 0);
    let turn = game.turn;
    let live: Vec<LiveSpySeat> = game
        .units
        .values()
        .filter(|unit| unit.owner == 0 && unit.kind == "spy")
        .map(|unit| {
            let city = spy_posting::city_of(game, state, unit.id, unit.pos);
            // ★★★ THE HOST'S OWN OPERATION, WHEN IT CROSSED. Every seat 0 Spy
            // is re-seated fresh on every sync, so until the host's
            // `GetSpyOperation` reached this board a Spy on a mission read as
            // idle and was handed the same mission again: `SPY_GAIN_SOURCES`
            // was refused 195 of 862 times on one run. A running operation
            // seats as the mission itself when the city it stands in is on
            // the board, so `legal_spy_actions` offers nothing and
            // `spy_mission_already_running` sees it; when that city is not
            // mirrored the Spy is held busy (`ready_turn`) until the host's
            // end turn instead. No facts at all — an older mod — leaves the
            // Spy idle exactly as before.
            let facts = game.host_unit_facts.get(&unit.id);
            let ends = facts
                .and_then(|facts| facts.spy_operation_ends)
                .map_or(turn + 1, |ends| ends.max(turn + 1));
            let operation = facts.and_then(|facts| facts.spy_operation.clone());
            let (mission, ready_turn) = match (operation, city) {
                (Some(kind), Some(city)) => (
                    Some(crate::game::SpyMission {
                        kind,
                        city,
                        target: unit.pos,
                        started: turn,
                        ends,
                    }),
                    0,
                ),
                (Some(_), None) => (None, ends),
                (None, _) => (None, 0),
            };
            // ★★★ AND THE HOST'S PROMOTION MENU, WHEN IT CROSSED. `level − 1`
            // (below) owes a fresh Spy nothing, but a Spy that levels on the
            // host keeps reading level 1 until its promotion is chosen, so
            // that rule never owed one either: 99 of 99 live Spies on
            // 2026-11-05 ended level 1 with no promotion, 81 of them holding
            // the 15 XP of a pending choice (the XP stops there until it is
            // made). A Spy whose menu still offers a choice it holds not and
            // the host has not refused owes exactly one, chosen from that
            // menu (`available_spy_promotions`); an empty menu, or none at
            // all, leaves `level − 1`.
            let held = unit.promotions.len() as i64;
            let blocked = game.blocked_promotions.get(&unit.id);
            let owed = facts
                .and_then(|facts| facts.spy_promotions.as_ref())
                .is_some_and(|menu| {
                    menu.iter().any(|name| {
                        !unit.promotions.contains(name)
                            && !blocked.is_some_and(|refused| refused.contains(name))
                    })
                });
            LiveSpySeat {
                id: unit.id,
                level: if owed {
                    held + 1
                } else {
                    (unit.level - 1).max(0) as i64
                },
                promotions: unit
                    .promotions
                    .iter()
                    .map(|name| name.to_string())
                    .collect(),
                city,
                mission,
                ready_turn,
            }
        })
        .collect();
    for LiveSpySeat {
        id,
        level,
        promotions,
        city,
        mission,
        ready_turn,
    } in live
    {
        let spy = game.spies.entry(id).or_insert_with(|| crate::game::Spy {
            id,
            owner: 0,
            level,
            promotions: Default::default(),
            city: None,
            ready_turn: 0,
            mission: None,
            sources_city: None,
            sources_until: 0,
            captured_by: None,
        });
        spy.level = level;
        spy.promotions = promotions;
        spy.city = city;
        spy.mission = mission;
        spy.ready_turn = ready_turn;
    }
}

/// Districts the host refused to place, per Civilization VI city id.
///
/// ★★★★ Read from `build_no_plot`, which the mod emits when
/// `CityManager.GetOperationTargets` offers no plot for a district. Measured on live
/// run `civvis-20260801T024428Z`: **39** of them by turn 115, every one the Government
/// Plaza — one per civilization, so once it exists there is no plot anywhere, and
/// CIVVIS re-chose it from the same board turn after turn.
///
/// ⚠ Returns Civilization VI's own city ids and district names. The caller maps ids to
/// CIVVIS cities and names through the shipped district table, and drops what it
/// cannot name rather than inventing a key — a blocked set full of unmatched names
/// would look populated and filter nothing.
///
/// ⚠ Older exports sent a bare hash with no city. Those carry no usable name and are
/// skipped, so an old stream reads as "nothing blocked" rather than blocking something
/// arbitrary.
pub fn refused_districts(
    path: &std::path::Path,
) -> std::collections::BTreeMap<i64, std::collections::BTreeSet<String>> {
    refused_districts_through(path, None)
}

fn refused_districts_through(
    path: &std::path::Path,
    turn: Option<u32>,
) -> std::collections::BTreeMap<i64, std::collections::BTreeSet<String>> {
    refused_no_plot_through(path, turn, "district", "DISTRICT_")
}

/// Wonders the host had no legal plot for, from the same event under its own key.
fn refused_wonders_through(
    path: &std::path::Path,
    turn: Option<u32>,
) -> std::collections::BTreeMap<i64, std::collections::BTreeSet<String>> {
    refused_no_plot_through(path, turn, "building", "BUILDING_")
}

/// World-unique wonders for which the host found no location at all.
///
/// A direct order carries a model-legal wonder site. When Firaxis responds with an
/// explicit `offered: 0`, its operation-target query found no site in the world, not
/// merely a different site in this city. The common cause is a rival completing the
/// world unique outside the partial mirror. Keep the fact forever; a later city does
/// not make a claimed world wonder available again. An absent `offered` belongs to
/// older telemetry and deliberately keeps the previous city-scoped behaviour.
fn host_unavailable_wonders_through(
    path: &std::path::Path,
    turn: Option<u32>,
) -> std::collections::BTreeSet<String> {
    let mut unavailable = std::collections::BTreeSet::new();
    let Ok(raw) = read_events(path) else {
        return unavailable;
    };
    for line in lines_containing(&raw, "build_no_plot") {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if event.get("kind").and_then(|value| value.as_str()) != Some("build_no_plot")
            || event.get("offered").and_then(|value| value.as_i64()) != Some(0)
        {
            continue;
        }
        if let Some(limit) = turn {
            let Some(at) = event.get("turn").and_then(|value| value.as_u64()) else {
                continue;
            };
            if at > u64::from(limit) {
                continue;
            }
        }
        let Some(wonder) = event.get("building").and_then(|value| value.as_str()) else {
            continue;
        };
        if wonder.starts_with("BUILDING_") {
            unavailable.insert(wonder.to_string());
        }
    }
    unavailable
}

/// The newest fresh positive placement result for every `(city, district)` pair.
///
/// Firaxis writes `offered_plots` only after it rejected CIVVIS's requested
/// coordinate.  Reusing the list on the next board gives the planner a small,
/// authoritative candidate set instead of making the bridge silently substitute a
/// tile in the current order.  A later `offered: 0` supersedes an earlier positive
/// result, so a site that ceased to be legal never survives merely because its old
/// event is still inside the cooldown window.
fn host_district_sites_through(
    path: &std::path::Path,
    current_turn: u32,
) -> BTreeMap<i64, BTreeMap<String, BTreeSet<crate::Pos>>> {
    host_sites_through(path, current_turn, "district", "DISTRICT_")
}

/// The wonder counterpart of [`host_district_sites_through`].
fn host_wonder_sites_through(
    path: &std::path::Path,
    current_turn: u32,
) -> BTreeMap<i64, BTreeMap<String, BTreeSet<crate::Pos>>> {
    host_sites_through(path, current_turn, "building", "BUILDING_")
}

/// Read the newest fresh, host-approved coordinates for one `build_no_plot` field.
///
/// A later zero-site response cancels a prior positive response, regardless of
/// whether that response named a district or a wonder.
fn host_sites_through(
    path: &std::path::Path,
    current_turn: u32,
    field: &str,
    prefix: &str,
) -> BTreeMap<i64, BTreeMap<String, BTreeSet<crate::Pos>>> {
    let Ok(raw) = read_events(path) else {
        return BTreeMap::new();
    };
    let oldest = current_turn.saturating_sub(PRODUCTION_REFUSAL_TTL);
    // Keyed by (player, item); the newest refusal turn for that pair and the
    // plots it named, if it named any.
    type Newest = BTreeMap<(i64, String), (u64, Option<BTreeSet<crate::Pos>>)>;
    let mut newest: Newest = BTreeMap::new();
    for line in lines_containing(&raw, "build_no_plot") {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if event.get("kind").and_then(|value| value.as_str()) != Some("build_no_plot") {
            continue;
        }
        let (Some(turn), Some(city), Some(item), Some(offered)) = (
            event.get("turn").and_then(|value| value.as_u64()),
            event.get("city").and_then(|value| value.as_i64()),
            event.get(field).and_then(|value| value.as_str()),
            event.get("offered").and_then(|value| value.as_i64()),
        ) else {
            continue;
        };
        if turn > u64::from(current_turn) || turn < u64::from(oldest) || !item.starts_with(prefix) {
            continue;
        }
        let key = (city, item.to_string());
        if newest
            .get(&key)
            .is_some_and(|(known_turn, _)| *known_turn > turn)
        {
            continue;
        }
        let sites = (offered > 0).then(|| {
            event
                .get("offered_plots")
                .and_then(|value| value.as_array())
                .into_iter()
                .flatten()
                .filter_map(|plot| {
                    let (x, y) = (
                        plot.get("x").and_then(|value| value.as_i64()),
                        plot.get("y").and_then(|value| value.as_i64()),
                    );
                    let (Some(x), Some(y)) = (x, y) else {
                        return None;
                    };
                    let (Ok(x), Ok(y)) = (i32::try_from(x), i32::try_from(y)) else {
                        return None;
                    };
                    Some(crate::hex::offset_to_axial(x, y))
                })
                .collect::<BTreeSet<_>>()
        });
        newest.insert(key, (turn, sites));
    }

    let mut out: BTreeMap<i64, BTreeMap<String, BTreeSet<crate::Pos>>> = BTreeMap::new();
    for ((city, item), (_, sites)) in newest {
        let Some(sites) = sites else {
            continue;
        };
        if !sites.is_empty() {
            out.entry(city).or_default().insert(item, sites);
        }
    }
    out
}

/// ⚠ One event, two keys. `build_no_plot` names a refused district under `district`
/// and a refused wonder under `building`; a parser that reads only the first drops
/// the second entirely, which is exactly what happened to 370 of 425 refusals.
fn refused_no_plot_through(
    path: &std::path::Path,
    turn: Option<u32>,
    field: &str,
    prefix: &str,
) -> std::collections::BTreeMap<i64, std::collections::BTreeSet<String>> {
    let mut refused: std::collections::BTreeMap<i64, std::collections::BTreeSet<String>> =
        Default::default();
    let Ok(raw) = read_events(path) else {
        return refused;
    };
    for line in lines_containing(&raw, "build_no_plot") {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if event.get("kind").and_then(|k| k.as_str()) != Some("build_no_plot") {
            continue;
        }
        if turn.is_some_and(|limit| {
            event
                .get("turn")
                .and_then(|value| value.as_u64())
                .unwrap_or(0)
                > limit as u64
        }) {
            continue;
        }
        // ★★★★★ THE EVENT ALREADY DRAWS THIS DISTINCTION AND THE BLOCK IGNORED IT.
        //
        // `offered` is on `build_no_plot` precisely to separate two opposite
        // refusals, and `Game::blocked_districts` says so in its own doc: zero
        // means the engine has no target ANYWHERE — a Government Plaza that
        // already exists, which is one per civilization and belongs blocked;
        // above zero means "the engine has ground, just not ours", which that
        // doc calls *a placement disagreement in one city that must not stop the
        // empire*.
        //
        // Above zero the district IS placeable in this city. `productionPlot`
        // returns nothing only because a direct CIVVIS order names a plot and
        // that plot was not among the offered ones, and it deliberately refuses
        // to substitute another — "substituting another legal plot would actuate
        // a different decision". So the item is fine, the city is fine, and only
        // the plot choice was wrong; blocking the district there forecloses
        // ground CIVVIS could have used by simply asking for a different tile.
        //
        // Measured across every live run of 2026-08-11, 47 `build_no_plot`
        // events: **41 of them had `offered > 0`** — 10 Theater, 7 Campus, 6
        // Industrial Zone, 4 Commercial Hub, 4 Diplomatic Quarter — the science,
        // production and culture backbone, struck out of those cities for the
        // rest of the game over a tile choice.
        //
        // ⚠ An ABSENT `offered` is not a reading. Older exports sent none, and
        // those keep the old behaviour exactly, so replaying an older run is
        // unchanged — the same rule `moves` follows in
        // `refused_sites_of_kind_through`.
        //
        // ⚠⚠ "NEVER BLOCK IT" IS THE WRONG HALF OF THE ANSWER — but the evidence
        // this comment first cited for that was overstated, and the correction
        // matters more than the claim.
        //
        // #1555 dropped these refusals entirely, so nothing remembers a
        // disagreement and the same district can be re-proposed every turn. Two
        // runs show that happening at scale: civvis-20260811T202458Z with 28
        // `build_no_plot` events in 250 turns, and …T212652Z with 57 in 250, and
        // in BOTH cases essentially one pair — city 131073,
        // `DISTRICT_COMMERCIAL_HUB`, `offered > 0` every time.
        //
        // ⚠⚠⚠ WHAT I WROTE HERE FIRST WAS "the very next full run", AND THAT IS
        // FALSE. Across 21 runs of 2026-08-10/11, four consecutive runs carrying
        // #1555 — …T150840Z (pinned to #1555 itself), …T163652Z, …T174134Z,
        // …T191919Z — recorded ZERO. Eight of the twenty-one recorded zero and
        // most of the rest recorded one to seven. The two spikes are outliers
        // that arrived five runs later, and #1555 alone does not explain them.
        //
        // So this guard is justified by the SHAPE of the failure — an unbounded
        // re-ask is a loop whenever it starts — and not by a causal story about
        // #1555 that the run distribution does not support. The cause of the two
        // spikes is unestablished; both also carried the improvement fold
        // (#1565/#1567), which is a correlation across two runs and nothing more.
        //
        // The remedy stands on the convention this codebase already has for a
        // refusal that is true now and not forever: block briefly, which bounds
        // any loop, and expire, which keeps the district from being foreclosed in
        // a city that may make room for it.
        //
        // ⚠ `turn: None` means "replay the whole file", and there is no "now" to
        // measure staleness against — a turn-5 disagreement is certainly stale by
        // turn 250. Those keep #1555's behaviour and do not block, which is the
        // honest reconstruction for a whole-game replay.
        if event
            .get("offered")
            .and_then(|v| v.as_i64())
            .is_some_and(|offered| offered > 0)
        {
            let Some(now) = turn else {
                continue;
            };
            let at = event.get("turn").and_then(|v| v.as_u64()).unwrap_or(0);
            if at < u64::from(now.saturating_sub(PRODUCTION_REFUSAL_TTL)) {
                continue;
            }
        }
        let (Some(city), Some(named)) = (
            event.get("city").and_then(|v| v.as_i64()),
            event.get(field).and_then(|v| v.as_str()),
        ) else {
            continue;
        };
        // A bare hash is an old export; it names nothing this side can use.
        if !named.starts_with(prefix) {
            continue;
        }
        refused.entry(city).or_default().insert(named.to_string());
    }
    refused
}

/// Host production choices rejected recently enough to still describe this board.
///
/// A host `CanProduce` failure is exact for one city and one moment. Remembering it
/// forever would turn a temporary resource or prerequisite shortage into a permanent
/// rules change; forgetting it immediately recreates the live loop where Library was
/// selected and rejected every turn. Eight turns is long enough for another building
/// to be selected and enter the queue, while guaranteeing a changed city is retried.
const PRODUCTION_REFUSAL_TTL: u32 = 8;

fn recent_host_item_refusals(
    path: &std::path::Path,
    current_turn: u32,
    event_kind: &str,
    accepted_prefixes: &[&str],
) -> std::collections::BTreeMap<i64, std::collections::BTreeSet<String>> {
    let mut refused: std::collections::BTreeMap<i64, std::collections::BTreeSet<String>> =
        Default::default();
    let Ok(raw) = read_events(path) else {
        return refused;
    };
    let oldest = current_turn.saturating_sub(PRODUCTION_REFUSAL_TTL);
    for line in lines_containing(&raw, event_kind) {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if event.get("kind").and_then(|value| value.as_str()) != Some(event_kind) {
            continue;
        }
        let (Some(turn), Some(city), Some(item)) = (
            event.get("turn").and_then(|value| value.as_u64()),
            event.get("city").and_then(|value| value.as_i64()),
            event.get("item").and_then(|value| value.as_str()),
        ) else {
            continue;
        };
        if city < 0 || turn < u64::from(oldest) || turn > u64::from(current_turn) {
            continue;
        }
        if accepted_prefixes
            .iter()
            .any(|prefix| item.starts_with(prefix))
        {
            refused.entry(city).or_default().insert(item.to_string());
        }
    }
    refused
}

pub fn refused_production(
    path: &std::path::Path,
    current_turn: u32,
) -> std::collections::BTreeMap<i64, std::collections::BTreeSet<String>> {
    // ⚠ `DISTRICT_` belongs here, and its absence made a whole branch of
    // `blocked_production_from` dead code: that function already falls back to
    // `civvis_node_name(&rules.districts, name, "DISTRICT_")`, and
    // `production_block_key` already emits `district:{name}`, but no district name
    // ever reached either because this filter dropped it first.
    //
    // ★ The prefix list predicts the cooldown EXACTLY. Over 20 live runs, gaps
    // between successive refusals of the same (run, city, item):
    //
    //   UNIT_       49 combos, 164 events — 0 gaps of 1, 115 of >=8   TTL holding
    //   BUILDING_   29 combos, 126 events — 0 gaps of 1,  97 of >=8   TTL holding
    //   PROJECT_    12 combos,  16 events — 0 gaps of 1,   4 of >=8   TTL holding
    //   DISTRICT_    4 combos,  18 events — 13 gaps of 1,  0 of >=8   NOT holding
    //
    // `DISTRICT_HOLY_SITE` was re-proposed in one city on turns 45 through 58, every
    // consecutive turn, against a TTL of eight.
    //
    // This is NOT a duplicate of `blocked_districts`, which is permanent and answers
    // "the host has no PLOT here" from `build_no_plot`. This one is the eight-turn
    // cooldown for "the host will not let this city produce that right now" — a
    // missing prerequisite or a district cap, which changes.
    recent_host_item_refusals(
        path,
        current_turn,
        "civvis_build_unplayable",
        &["UNIT_", "BUILDING_", "PROJECT_", "DISTRICT_"],
    )
}

/// Host purchase refusals use the same short cooldown as production refusals,
/// but feed a separate legal-action gate. The distinction is load-bearing: when
/// buying a Settler fails, producing one must become *more* likely, not illegal.
pub fn refused_purchases(
    path: &std::path::Path,
    current_turn: u32,
) -> std::collections::BTreeMap<i64, std::collections::BTreeSet<String>> {
    recent_host_item_refusals(
        path,
        current_turn,
        "purchase_refused",
        &["UNIT_", "BUILDING_", "DISTRICT_"],
    )
}

/// How far CIVVIS's own economy has drifted from the one Civilization VI reports.
///
/// ★★★★ The reconstruction is openly partial, and nothing has ever said BY HOW MUCH.
/// Measured on live run `civvis-20260801T024428Z` at turn 60: science 5.80 in the game
/// against 8.6 on the board (**+48%**), culture 7.08 against 8.9 (**+26%**).
///
/// That matters because research valuations are spent in these units — CIVVIS rates a
/// tech "worth 42 to the expansion plan" and times the plan against a rate half again
/// too fast, so a plan that looks affordable is not.
///
/// ⚠ REPORTED, NOT INJECTED. `StateSnapshot::science` says why: these are derived
/// yields and overriding them while the board that produces them disagrees would fight
/// the simulation instead of informing it. This turns an unmeasured axis into a number
/// on every turn; closing the gap is a separate decision that now has evidence behind
/// it rather than a guess.
///
/// `None` when the export carried no yields — an older mod must read as unknown, never
/// as agreement.
pub fn economy_drift(game: &crate::game::Game, state: &StateSnapshot) -> Option<String> {
    let science_host = state
        .science
        .filter(|value| value.is_finite() && *value >= 0.0);
    let culture_host = state
        .culture
        .filter(|value| value.is_finite() && *value >= 0.0);
    let (Some(science_host), Some(culture_host)) = (science_host, culture_host) else {
        return None;
    };
    let mut science = 0.0f64;
    let mut culture = 0.0f64;
    let mut production = 0.0f64;
    for city in game.cities.values().filter(|city| city.owner == 0) {
        let yields = game.city_yields_model(city.id);
        science += yields.science;
        culture += yields.culture;
        production += yields.production;
    }
    // ★★★★ PRODUCTION BELONGS ON THIS LINE NOW, AND COULD NOT BEFORE.
    //
    // Use the exact empire total first. `StateCity.production` is the
    // BuildQueue's whole-number `GetProductionYield()` reading, not the city's
    // exact `City:GetYield(YieldTypes.PRODUCTION)` value; summing it manufactured
    // a recurring +3% drift on the live Science run even while the exact city
    // yields and `public_stats.production` agreed. Fall back to exact per-city
    // yields for exports that predate `public_stats`, but only when every city is
    // present and has a finite reading so a partial export cannot look complete.
    let host_production = state
        .public_stats
        .production
        .filter(|value| value.is_finite() && *value >= 0.0)
        .or_else(|| {
            let values: Vec<f64> = state
                .cities
                .iter()
                .filter_map(|city| city.yields.map(|yields| yields.production))
                .collect();
            (!values.is_empty()
                && values.len() == state.cities.len()
                && values
                    .iter()
                    .all(|value| value.is_finite() && *value >= 0.0))
            .then(|| values.into_iter().sum())
        });
    let pct = |ours: f64, theirs: f64| {
        if theirs.abs() < 1e-6 {
            return "n/a".to_string();
        }
        format!("{:+.0}%", 100.0 * (ours - theirs) / theirs)
    };
    // ★★★★★ A DRIFT THAT CANNOT NAME ITS OWN CAUSE GETS RE-INVESTIGATED FOREVER.
    //
    // The single largest term is known, expected, and has nothing to do with the
    // reconstruction being wrong: **CIVVIS's civilization abilities are not
    // Civilization VI's**. `data/civs.json` gives eleven civs a flat
    // `city_science` and many more a flat `city_culture`/`city_gold` — Arabia's
    // entry reads "House of Wisdom: +1 science and +1 faith in every city", where
    // the real ability is a Madrasa/religion effect granting no flat per-city
    // yield. So a mirrored seat plays a different civilization from the one the
    // game dealt, by exactly `effect x cities`.
    //
    // Measured on run civvis-20260802T064240Z (Arabia, 4-5 cities): science ran
    // **+18% median** while culture sat at **-0%** — the palace double-pay having
    // been fixed — and the absolute gap was +3.2 to +3.6 against `city_science: 1`
    // across four cities. That is the whole residual.
    //
    // ⚠ Reported, NOT silently subtracted. The gap is real: CIVVIS is planning on
    // yields the game will not pay it, and hiding that would be the same class of
    // error as the instruments this file exists to repair. What changes is that a
    // reader can now tell the KNOWN offset from a NEW defect at a glance, which is
    // the difference between a number and a shrug.
    //
    // ⚠ Fixing the underlying data moves `Rules::source_fingerprint`, so the Elo
    // ledger rejects new games at bind time — the #703 trap. That is an operator
    // decision, and until it is taken this line is how the cost stays visible.
    let cities = game.cities.values().filter(|city| city.owner == 0).count() as f64;
    let civ_science = game.civ_effect(0, "city_science") * cities;
    let civ_culture = game.civ_effect(0, "city_culture") * cities;
    let attributed = if civ_science > 0.0 || civ_culture > 0.0 {
        format!(
            "; of which civ ability {} accounts for science {:+.1} culture {:+.1} over {} cities",
            game.players
                .get(0)
                .map(|player| player.civ.as_str())
                .unwrap_or("?"),
            civ_science,
            civ_culture,
            cities as u32,
        )
    } else {
        String::new()
    };
    // Omitted rather than printed as 0.0/x when no city reported a figure, so an
    // older mod is silent here instead of claiming a 100% drift.
    let production_part = match host_production.filter(|value| *value > 0.0) {
        Some(host_production) => format!(
            " production {:.1}/{:.1} {}",
            host_production,
            production,
            pct(production, host_production)
        ),
        None => String::new(),
    };
    Some(format!(
        "economy civ6/civvis science {:.1}/{:.1} {} culture {:.1}/{:.1} {}{}{}{}{}",
        science_host,
        science,
        pct(science, science_host),
        culture_host,
        culture,
        pct(culture, culture_host),
        production_part,
        attributed,
        host_amenity_report(state),
        host_envoy_report(state),
    ))
}

/// Envoys Civilization VI says we are holding and have not placed.
///
/// ★★★★★ Reported here because the loss was invisible everywhere else. `SendEnvoy`
/// is gated behind `Player::envoys_free`, which the mirror did not carry until
/// [`apply_mirrored_envoys_free`], so the action was never enumerated on a live
/// board and never even reached the skipped-action tally. The empire simply
/// never noticed it was holding them. Kept as the per-turn instrument: with the
/// board spending, `unspent` should fall to 0 within a few turns of any income.
///
/// Measured over 36 live runs past turn 150: median envoys PLACED **1**, median
/// suzerainties **0**, and 16 of 36 runs finished having placed none at all.
///
/// Prints nothing when the host did not answer, so a mirror built before the
/// export does not read as an empire that is correctly holding zero.
fn host_envoy_report(state: &StateSnapshot) -> String {
    let Some(free) = state.envoys_free.filter(|n| *n >= 0) else {
        return String::new();
    };
    let placed: i64 = state.minors.iter().map(|m| m.envoys.max(0)).sum();
    // ⚠ `suzerain` is a SEAT ID defaulting to `minus_one`, not a flag. Seat 0 is
    // ours, and an unclaimed city-state reads -1 — so testing `== 0` counts only
    // the ones we actually hold. A truthiness test here would report every
    // masterless city-state as ours, which is the instrument lying in the one
    // direction that would make this whole finding look already-solved.
    let suzerain = state.minors.iter().filter(|m| m.suzerain == 0).count();
    format!(
        "; envoys unspent {free} placed {placed} suzerain {suzerain}/{}",
        state.minors.len()
    )
}

/// What Civilization VI itself says the empire's happiness is costing it.
///
/// ★★★★★ This line has compared science, culture and production for the whole
/// project and never once carried the term that multiplies all three. CIVVIS bands
/// its own derived amenity surplus into a factor on every non-food yield
/// ([`Game::amenity_yield_mult_for`]: `-4` → 0.80, `-6` → 0.70) and its model puts
/// the live empires at `-4/-5`. If that is real it is the largest single multiplier
/// on the board — a **25-30% standing tax on culture and science alike**. If it is
/// not, CIVVIS has been planning against an invented penalty.
///
/// ⚠ **The rest of this line cannot answer that**, which is exactly why the term
/// belongs here: totals-vs-totals hides a spurious 0.75 behind any offsetting
/// overestimate. `mult` is the host's own `GetHappinessNonFoodYieldModifier`.
///
/// Prints nothing when the host said nothing — the fields default to the
/// `unknown_metric` sentinel, and a mirror built before the export must not read as
/// a perfectly happy empire.
fn host_amenity_report(state: &StateSnapshot) -> String {
    let known: Vec<&StateCity> = state
        .cities
        .iter()
        .filter(|c| c.amenities >= 0.0 && c.amenities_needed >= 0.0)
        .collect();
    if known.is_empty() {
        return String::new();
    }
    let surplus: f64 = known.iter().map(|c| c.amenities - c.amenities_needed).sum();
    let short = known
        .iter()
        .filter(|c| c.amenities < c.amenities_needed)
        .count();
    // ⚠⚠ `GetHappinessNonFoodYieldModifier` RETURNS A PERCENTAGE, NOT A MULTIPLIER,
    // and it is NEGATIVE when the empire is unhappy. First live reading, run
    // `civvis-20260803T082856Z` t111: Kraków -10, Wrocław -20, Radom/Warsaw/Gdańsk
    // -10 — i.e. -10% and -20% on every non-food yield, not 0.90 and 0.80.
    //
    // The original code printed it as `host_yield_mult {:.2}` and filtered on
    // `>= 0.0`, which was wrong twice over: the label reads like a multiplier, so
    // `-12.00` would be misread as a factor, and the filter discarded every real
    // reading because a taxed empire always reports a negative. An instrument that
    // silently drops exactly the case it was built to measure is the failure this
    // file exists to prevent.
    //
    // ⚠ The sentinel is now `is_finite` plus the mod's `-1`… which is itself a legal
    // percentage. So the -1 sentinel is UNUSABLE for this field and the absent case
    // is the only one that can be detected: `unknown_metric` is NaN, which
    // `is_finite` rejects. A host that answered -1% and a host that failed to answer
    // are indistinguishable here, and -1% is close enough to zero that treating it
    // as unknown costs nothing.
    let mults: Vec<f64> = known
        .iter()
        .map(|c| c.happiness_yield_mult)
        .filter(|m| m.is_finite() && *m != -1.0)
        .collect();
    let mult = if mults.is_empty() {
        String::new()
    } else {
        format!(
            " host_yield_pct {:+.0}%",
            mults.iter().sum::<f64>() / mults.len() as f64
        )
    };
    // Name the two sources CIVVIS can actually act on: improve or trade for a
    // luxury, or build an Entertainment Complex.
    let from = |pick: fn(&StateCity) -> f64| -> f64 {
        known.iter().map(|c| pick(c)).filter(|v| *v >= 0.0).sum()
    };
    format!(
        "; amenities net {:+.0} over {} cities ({} short){} from luxuries {:.0} entertainment {:.0}",
        surplus,
        known.len(),
        short,
        mult,
        from(|c| c.amenities_luxuries),
        from(|c| c.amenities_entertainment),
    )
}

/// Firaxis serializes the integer Amenity ledger through Lua/JSON as floats.
/// Keep a value only when both parts of the ledger are known, finite, and still
/// representable by the integer rules engine; a partial query is not evidence
/// that a city is content.
fn host_city_amenity_surplus(city: &StateCity) -> Option<i64> {
    if !city.amenities.is_finite()
        || !city.amenities_needed.is_finite()
        || city.amenities < 0.0
        || city.amenities_needed < 0.0
    {
        return None;
    }
    let amenities = city.amenities.round();
    let needed = city.amenities_needed.round();
    if (city.amenities - amenities).abs() > 1e-6
        || (city.amenities_needed - needed).abs() > 1e-6
        || amenities < i64::MIN as f64
        || amenities > i64::MAX as f64
        || needed < i64::MIN as f64
        || needed > i64::MAX as f64
    {
        return None;
    }
    Some(amenities as i64 - needed as i64)
}

/// Policy cards the host ruleset has retired, learned from its own refusals.
///
/// ★★★★ **NO NEW MOD EVENT WAS NEEDED — THE ANSWER WAS ALREADY IN THE STREAM.**
/// Every `orders` event carries a `refusals` map keyed by reason, and the agent
/// already writes `obsolete_<POLICY>` there after asking
/// `culture:IsPolicyObsolete`. Measured on live run `civvis-20260801T012454Z`:
/// `obsolete_POLICY_ILKUM` **105**, `DISCIPLINE` 6, `AGOGE` 1 — 112 of 813
/// refusals, third behind movement and `no_params`. The game said so every time and
/// nothing read it, so CIVVIS re-derived the same card on almost every turn.
///
/// ⚠ Keyed by reason, so the count is per turn and the same card appears in event
/// after event; a set is the right shape and re-reading the whole file is idempotent.
///
/// ⚠ Returns the RAW Civilization VI names. Translation happens where the ruleset is
/// in hand, through the shipped policy table rather than by string surgery, and a card
/// CIVVIS does not model is dropped rather than inserted under a name that matches
/// nothing — a blocked set full of names no filter can match would look populated and
/// do nothing, which is the failure mode this bridge specialises in.
pub fn refused_policies(path: &std::path::Path) -> std::collections::BTreeSet<String> {
    refused_policies_through(path, None)
}

fn refused_policies_through(
    path: &std::path::Path,
    turn: Option<u32>,
) -> std::collections::BTreeSet<String> {
    let mut refused: std::collections::BTreeSet<String> = Default::default();
    let Ok(raw) = read_events(path) else {
        return refused;
    };
    for line in lines_containing(&raw, "obsolete_") {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if turn.is_some_and(|limit| {
            event
                .get("turn")
                .and_then(|value| value.as_u64())
                .unwrap_or(0)
                > limit as u64
        }) {
            continue;
        }
        let Some(reasons) = event.get("refusals").and_then(|r| r.as_object()) else {
            continue;
        };
        for reason in reasons.keys() {
            let Some(civ6) = reason.strip_prefix("obsolete_") else {
                continue;
            };
            refused.insert(civ6.to_string());
        }
    }
    refused
}

/// The pantheon beliefs Civilization VI refused as already taken by another
/// player, as its own `BELIEF_*` names, harvested from the `taken_<BELIEF>`
/// refusal reasons in the `orders` events. Same shape as [`refused_policies`]:
/// the mod's `pantheon` handler answers `taken_BELIEF_<X>` when
/// `IsInSomePantheon` says a rival holds it, and until this nothing read it back
/// — the mirror seats no rival pantheons, so the same belief was re-derived from
/// the same board next turn. See `Game::blocked_pantheons`.
pub fn refused_pantheons(path: &std::path::Path) -> std::collections::BTreeSet<String> {
    refused_pantheons_through(path, None)
}

fn refused_pantheons_through(
    path: &std::path::Path,
    turn: Option<u32>,
) -> std::collections::BTreeSet<String> {
    let mut refused: std::collections::BTreeSet<String> = Default::default();
    let Ok(raw) = read_events(path) else {
        return refused;
    };
    for line in lines_containing(&raw, "taken_BELIEF_") {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if turn.is_some_and(|limit| {
            event
                .get("turn")
                .and_then(|value| value.as_u64())
                .unwrap_or(0)
                > limit as u64
        }) {
            continue;
        }
        let Some(reasons) = event.get("refusals").and_then(|r| r.as_object()) else {
            continue;
        };
        for reason in reasons.keys() {
            let Some(civ6) = reason.strip_prefix("taken_") else {
                continue;
            };
            if civ6.starts_with("BELIEF_") {
                refused.insert(civ6.to_string());
            }
        }
    }
    refused
}

/// The host's taken pantheons as CIVVIS spells them, dropping any it does not
/// model — an unmatched entry would filter nothing while making the set look
/// populated, the same care as [`blocked_policies_from`].
fn blocked_pantheons_from(
    names: &std::collections::BTreeSet<String>,
    rules: &crate::rules::Rules,
) -> std::collections::BTreeSet<Name> {
    names
        .iter()
        .filter_map(|civ6| civvis_belief_name(rules, civ6))
        .filter(|name| rules.beliefs.pantheon.contains_key(name.as_str()))
        .map(|name| Name::new(&name))
        .collect()
}

/// A Civilization VI production type name as a CIVVIS queue [`Item`].
///
/// ⚠ Returns None rather than guessing. A wrong item would tell CIVVIS a city is
/// busy with something it is not, which is worse than the idle city this fixes: it
/// would suppress a real production decision instead of merely repeating one.
///
/// Districts are reconstructed only where the export names the plot; a district the
/// export did not place is refused rather than invented on arbitrary ground.
///
/// ★★★★ A WONDER IN PROGRESS IS A BUSY CITY, AND UNTIL NOW IT READ AS AN IDLE ONE.
/// Civilization VI names a wonder under construction `BUILDING_<WONDER>`; that is
/// not a `rules.buildings` row, so this returned `None`, the queue was seeded
/// empty, and the planner chose production from scratch every turn — the first
/// live wonder the seat ever started (Hagia Sophia, Rome, run
/// civvis-20260815T202611Z t124, 14 turns) was replaced by a University the very
/// next turn, `0 already banked` both times. `release_foreign_production` could
/// not help: the host WAS building what we asked for. The export carries the plots
/// of COMPLETED wonders only, so the queue item takes the city centre as its
/// placeholder plot when `centre` is given — a committed queue is skipped by the
/// planner and never re-emitted as an order, so the plot is a marker of busyness
/// and nothing reads it as ground. Callers translating a bare name for a block key
/// pass `None` and get the same wonder item keyed by name.
fn civvis_production_item(
    rules: &crate::rules::Rules,
    civ6: Option<&str>,
    districts: &[StateDistrict],
    centre: Option<crate::Pos>,
) -> Option<crate::game::Item> {
    let civ6 = civ6?.trim();
    if civ6.is_empty() {
        return None;
    }
    if let Some(name) = civvis_node_name(&rules.units, civ6, "UNIT_") {
        return Some(crate::game::Item::Unit {
            unit: crate::name::Name::new(&name),
        });
    }
    if let Some(name) = civvis_node_name(&rules.buildings, civ6, "BUILDING_") {
        return Some(crate::game::Item::Building {
            building: crate::name::Name::new(&name),
        });
    }
    if let Some(name) = civvis_node_name(&rules.wonders, civ6, "BUILDING_") {
        return Some(crate::game::Item::Wonder {
            wonder: crate::name::Name::new(&name),
            pos: centre.unwrap_or((0, 0)),
        });
    }
    // Firaxis's repeatable district grants use implementation names while
    // CIVVIS keeps their player-facing names. The outbound bridge already
    // translates these seven aliases; mirror the same vocabulary inbound so a
    // city actively running one never appears idle and gets its queue replaced.
    let project_alias = match civ6.to_ascii_uppercase().as_str() {
        "PROJECT_ENHANCE_DISTRICT_CAMPUS" => Some("campus_research_grants"),
        "PROJECT_ENHANCE_DISTRICT_HOLY_SITE" => Some("holy_site_prayers"),
        "PROJECT_ENHANCE_DISTRICT_COMMERCIAL_HUB" => Some("commercial_hub_investment"),
        "PROJECT_ENHANCE_DISTRICT_HARBOR" => Some("harbor_shipping"),
        "PROJECT_ENHANCE_DISTRICT_ENCAMPMENT" => Some("encampment_training"),
        "PROJECT_ENHANCE_DISTRICT_INDUSTRIAL_ZONE" => Some("industrial_zone_logistics"),
        "PROJECT_ENHANCE_DISTRICT_THEATER" => Some("theater_square_festival"),
        // The three space-race rows `civvis_orders` spells the other way; the
        // fourth, `PROJECT_TERRESTRIAL_LASER`, reaches its CIVVIS name through
        // the unique-prefix rule in `civvis_node_name`.
        "PROJECT_LAUNCH_EXOPLANET_EXPEDITION" => Some("exoplanet_expedition"),
        "PROJECT_LAUNCH_MARS_BASE" => Some("launch_mars_colony"),
        "PROJECT_ORBITAL_LASER" => Some("lagrange_laser_station"),
        _ => None,
    };
    if let Some(name) = project_alias
        .filter(|name| rules.projects.contains_key(name))
        .map(str::to_string)
        .or_else(|| civvis_node_name(&rules.projects, civ6, "PROJECT_"))
    {
        return Some(crate::game::Item::Project {
            project: crate::name::Name::new(&name),
        });
    }
    // ★★★★★ A DISTRICT, once the export says WHERE.
    //
    // This used to return None for every district on the honest grounds that
    // `Item::District` needs a `pos` and inventing one would place a district on
    // arbitrary ground. The consequence was worse than the guess it avoided: a city
    // building a district read as IDLE, so CIVVIS re-decided its production every
    // turn and ordered the same district again. Measured on run
    // civvis-20260731T163924Z — 60 `DISTRICT_GOVERNMENT` orders between t46 and
    // t128, every one answered `applied: true`, on a capital still showing three
    // buildings at t130. Sixty of that run's ~91 build orders.
    //
    // Nothing is invented here: Civilization VI assigns the plot when the district
    // is placed, and the export now carries it. Still None when the plot is absent,
    // because refusing to guess was never the wrong half.
    if let Some(name) = civvis_node_name(&rules.districts, civ6, "DISTRICT_") {
        let plot = districts
            .iter()
            .find(|d| d.kind.eq_ignore_ascii_case(civ6))
            .map(|d| crate::hex::offset_to_axial(d.x, d.y))?;
        return Some(crate::game::Item::District {
            district: crate::name::Name::new(&name),
            pos: plot,
        });
    }
    None
}

/// The newest `state` event as raw JSON, with the run's `seat` identity merged in.
///
/// The typed [`state_from_events`] is what the decision path wants. This is for the
/// mock path, where a field has to survive the round trip even if CIVVIS has no
/// struct member for it — dumping through `StateSnapshot` would silently drop
/// anything unmodelled, and the operator would edit a file that cannot describe
/// what they are looking at.
pub fn state_value_from_events(
    path: &std::path::Path,
    turn: Option<u32>,
) -> Option<serde_json::Value> {
    let raw = read_events(path).ok()?;
    if turn.is_none() {
        if let Some(found) = latest_state_value_bounded(&raw) {
            return found;
        }
    }
    state_value_scan(&raw, turn)
}

/// [`state_value_scan`]'s answer for no requested turn, from the newest
/// records only (see [`latest_state_range_bounded`] -- the mirror rebuild runs
/// this walk too, as a full `Value` parse of every line). `None` when the log
/// holds a record the walk's own rule reads unusually (a turn past `u32`, or a
/// state with no integer turn, which it reads as 0 and lets anything replace),
/// or the newest turn is 0; those logs keep the walk.
fn latest_state_value_bounded(raw: &str) -> Option<Option<serde_json::Value>> {
    let kind_is = |value: &serde_json::Value, kind: &str| {
        value.get("kind").and_then(|k| k.as_str()) == Some(kind)
    };
    let mut order = Vec::new();
    for (start, end) in line_ranges_containing(raw, "\"state\"") {
        let line = &raw[start..end];
        match turn_upper_bound(line) {
            (_, true) => return None,
            (Some(bound), false) => order.push((bound, (start, end))),
            (None, false) => {
                if serde_json::from_str::<serde_json::Value>(line)
                    .is_ok_and(|value| kind_is(&value, "state"))
                {
                    return None;
                }
            }
        }
    }
    by_turn_bound_newest_first(&mut order);
    let mut best: Option<(u32, (usize, usize), serde_json::Value)> = None;
    for (bound, range) in order {
        if let Some((turn, at, _)) = &best {
            if bound < *turn || (bound == *turn && range.0 < at.0) {
                break;
            }
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw[range.0..range.1]) else {
            continue;
        };
        if !kind_is(&value, "state") {
            continue;
        }
        let turn = value.get("turn").and_then(|t| t.as_u64())?;
        if turn > u64::from(bound) {
            return None;
        }
        let turn = turn as u32;
        let newer = best
            .as_ref()
            .is_none_or(|(had, at, _)| turn > *had || (turn == *had && range.0 > at.0));
        if newer {
            best = Some((turn, range, value));
        }
    }
    let Some((turn, _, mut state)) = best else {
        return Some(None);
    };
    if turn == 0 {
        return None;
    }
    // The newest seat, wherever it sits; the walk keeps the last one too.
    let seat = line_ranges_containing(raw, "\"seat\"")
        .into_iter()
        .rev()
        .filter_map(|(start, end)| serde_json::from_str::<serde_json::Value>(&raw[start..end]).ok())
        .find(|value| kind_is(value, "seat"));
    if let (Some(object), Some(seat)) = (state.as_object_mut(), seat) {
        object.insert("seat".into(), seat);
    }
    Some(Some(state))
}

/// Every line in turn, in log order: the reference for [`state_value_from_events`].
fn state_value_scan(raw: &str, turn: Option<u32>) -> Option<serde_json::Value> {
    let mut best: Option<serde_json::Value> = None;
    let mut seat: Option<serde_json::Value> = None;
    for line in raw.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        match value.get("kind").and_then(|k| k.as_str()) {
            Some("seat") => seat = Some(value),
            Some("state") => {
                let at = value.get("turn").and_then(|t| t.as_u64()).unwrap_or(0) as u32;
                if matches!(turn, Some(want) if at != want) {
                    continue;
                }
                let newer = best
                    .as_ref()
                    .and_then(|b| b.get("turn").and_then(|t| t.as_u64()))
                    .map(|had| at as u64 >= had)
                    .unwrap_or(true);
                if newer {
                    best = Some(value);
                }
            }
            _ => {}
        }
    }
    if let (Some(state), Some(seat)) = (best.as_mut(), seat) {
        if let Some(object) = state.as_object_mut() {
            object.insert("seat".into(), seat);
        }
    }
    best
}

/// Merge `patch` over `base`: objects recurse key by key, anything else replaces.
///
/// ⚠ A LIST REPLACES WHOLE. Merging arrays element-wise would make it impossible to
/// DELETE a city or unit from a mocked board — you could only ever add or edit — and
/// removing things is most of what setting up a position is.
pub fn merge_state(base: &mut serde_json::Value, patch: &serde_json::Value) {
    match (base.as_object_mut(), patch.as_object()) {
        (Some(base_map), Some(patch_map)) => {
            for (key, value) in patch_map {
                if value.is_null() {
                    base_map.remove(key);
                    continue;
                }
                merge_state(
                    base_map
                        .entry(key.clone())
                        .or_insert(serde_json::Value::Null),
                    value,
                );
            }
        }
        _ => *base = patch.clone(),
    }
}

/// The name to hang on a reconstructed city: Civilization VI's, when it sent one.
///
/// Falling back to `None` keeps CIVVIS's own naming for runs recorded before the
/// mod exported names, rather than leaving those cities blank.
fn banner(city: &StateCity) -> Option<String> {
    let name = city.name.trim();
    (!name.is_empty()).then(|| name.to_string())
}

fn civvis_religion_name(civ6: &str) -> Option<String> {
    let bare = civ6.trim().strip_prefix("RELIGION_").unwrap_or(civ6.trim());
    if bare.is_empty() {
        return None;
    }
    Some(
        bare.split('_')
            .map(|word| {
                let word = word.to_ascii_lowercase();
                let mut chars = word.chars();
                chars
                    .next()
                    .map(|first| first.to_ascii_uppercase().to_string() + chars.as_str())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join(" "),
    )
}

fn civvis_belief_name(rules: &crate::rules::Rules, civ6: &str) -> Option<String> {
    let name = civ6
        .trim()
        .strip_prefix("BELIEF_")
        .unwrap_or(civ6.trim())
        .to_ascii_lowercase();
    // Gathering Storm's XML calls this `BELIEF_DEFENDER_OF_FAITH`, while the
    // model keeps the unambiguous `defender_of_the_faith` node. The fidelity
    // audit has the same shipped-data alias; the live mirror must use it too or
    // the already-implemented combat bonus is silently absent in-game.
    let name = match name.as_str() {
        "defender_of_faith" => "defender_of_the_faith".to_string(),
        _ => name,
    };
    [
        &rules.beliefs.pantheon,
        &rules.beliefs.founder,
        &rules.beliefs.follower,
        &rules.beliefs.enhancer,
        &rules.beliefs.worship,
    ]
    .iter()
    .any(|family| family.contains_key(name.as_str()))
    .then_some(name)
}

/// A Firaxis `COMMEMORATION_*` type as CIVVIS's dedication id — the same pairing
/// `Game`'s era-window table (`free_inquiry` / SCIENTIFIC, `heartbeat_of_steam` /
/// INDUSTRIAL, ...) is pinned to.
pub fn civvis_dedication_name(commemoration: &str) -> Option<&'static str> {
    Some(match commemoration.trim() {
        "COMMEMORATION_SCIENTIFIC" => "free_inquiry",
        "COMMEMORATION_CULTURAL" => "pen_brush_and_voice",
        "COMMEMORATION_INFRASTRUCTURE" => "monumentality",
        "COMMEMORATION_RELIGIOUS" => "exodus_of_the_evangelists",
        "COMMEMORATION_EXPLORATION" => "hic_sunt_dracones",
        "COMMEMORATION_ECONOMIC" => "reform_the_coinage",
        "COMMEMORATION_INDUSTRIAL" => "heartbeat_of_steam",
        "COMMEMORATION_MILITARY" => "to_arms",
        "COMMEMORATION_TOURISM" => "wish_you_were_here",
        "COMMEMORATION_ESPIONAGE" => "bodyguard_of_lies",
        "COMMEMORATION_AERONAUTICAL" => "sky_and_stars",
        "COMMEMORATION_AUTOMATON" => "automaton_warfare",
        _ => return None,
    })
}

/// Firaxis player id -> mirrored seat, for the two per-city player fields the
/// capture decision carries (`StateCity::captured_from`, `::original_owner`).
/// The same stable major-seat rule the war bond and `apply_territory` use;
/// city-states and the Free Cities actor take the seats `minor_actor_assignments`
/// gives them, and the local player is seat 0.
fn host_capture_seats(
    game: &crate::game::Game,
    state: &StateSnapshot,
) -> std::collections::BTreeMap<i64, usize> {
    let mut seat_of: std::collections::BTreeMap<i64, usize> =
        host_major_seat_map(state, modeled_major_player_count(game))
            .into_iter()
            .filter_map(|(host, seat)| i64::try_from(host).ok().map(|host| (host, seat)))
            .collect();
    for (minor, seat) in minor_actor_assignments(game, state) {
        seat_of.insert(minor.player as i64, seat);
    }
    seat_of
}

fn district_garrison_hp(damage: i32, max_damage: i32) -> i32 {
    if max_damage > 0 && damage >= 0 {
        let remaining = (i64::from(max_damage) - i64::from(damage)).max(0);
        ((100 * remaining) / i64::from(max_damage)).clamp(0, 100) as i32
    } else {
        100
    }
}

fn district_outer_hp(damage: i32, max_damage: i32, fallback: i32) -> i32 {
    if max_damage >= 0 && damage >= 0 {
        (i64::from(max_damage) - i64::from(damage)).clamp(0, i64::from(i32::MAX)) as i32
    } else {
        fallback
    }
}

fn retain_observed_defending_districts(game: &mut crate::game::Game, cid: u32) {
    let city = game.cities.get_mut(&cid).unwrap();
    city.defending_districts.retain(|state| {
        city.districts
            .iter()
            .any(|(kind, pos)| *kind == state.kind && *pos == state.pos)
    });
}

#[derive(Default)]
struct ObservedUnitProgress {
    promotions: Option<BTreeSet<Name>>,
    religion: Option<String>,
}

fn civvis_unit_promotion_name(civ6: &str) -> String {
    let bare = civ6.strip_prefix("PROMOTION_").unwrap_or(civ6);
    let lower = bare.to_ascii_lowercase();
    if let Some(monk) = lower.strip_prefix("monk_") {
        return monk.to_string();
    }
    // The Spy's tree carries a SPY_ prefix on the host, the same way the
    // Warrior Monk's carries MONK_. `civ6_unit_promotion_name` has written
    // that prefix outbound since the espionage promotions were the seat's
    // largest refusal category; without the matching strip inbound, every
    // promotion on an observed Spy lands in `unmapped` instead.
    if let Some(spy) = lower.strip_prefix("spy_") {
        // Firaxis spells this one with a single `r`.
        return if spy == "guerilla_leader" {
            "guerrilla_leader".to_string()
        } else {
            spy.to_string()
        };
    }
    match lower.as_str() {
        "super_carrier" => "supercarrier".to_string(),
        "goes_to" => "goes_to_11".to_string(),
        "pop" => "pop_star".to_string(),
        "surf_rock" => "surf_band".to_string(),
        _ => lower,
    }
}

fn observed_unit_progress(
    rules: &crate::rules::Rules,
    state: &StateUnit,
    unmapped: &mut Vec<String>,
) -> ObservedUnitProgress {
    let promotions = state.promotions.as_ref().map(|host| {
        host.iter()
            .filter_map(|civ6| {
                let name = civvis_unit_promotion_name(civ6);
                if rules.promotions.contains_key(&name) {
                    Some(Name::new(&name))
                } else {
                    let issue = format!("{civ6}:unit_promotion");
                    if !unmapped.contains(&issue) {
                        unmapped.push(issue);
                    }
                    None
                }
            })
            .collect()
    });
    let religion = match state.religion.as_deref() {
        Some(civ6) => match civvis_religion_name(civ6) {
            Some(name) => Some(name),
            None => {
                let issue = format!("{civ6}:unit_religion");
                if !unmapped.contains(&issue) {
                    unmapped.push(issue);
                }
                None
            }
        },
        None => None,
    };
    ObservedUnitProgress {
        promotions,
        religion,
    }
}

/// What an established Amani adds to seat 0's delegation at `minor`, read from the
/// HOST's governor record rather than the board's roster, as `(messenger,
/// multiplier)` in the shape of `Game::amani_envoy_terms`.
///
/// ★★★★ THE HOST'S ENVOY COUNT ALREADY INCLUDES HER. `minors[].envoys` is
/// `GetTokensReceived`, and on run civvis-20260826T184456Z La Venta read 5 → 7 at
/// t145 frame 1, the export in which `GOVERNOR_THE_AMBASSADOR` first reported
/// `established: true` there, with no envoy order to that player. The mirror
/// stored 7 and `Game::envoys_at` added `city_state_envoys` (2) again: board 9,
/// host 7, board Suzerain where the host reported a tie (`suzerain -1`), and the
/// planner stopped sending envoys to a suzerainty it did not hold. So the stored
/// count is the host's number NET of these terms (`raw_envoys_for`), and the
/// board's `envoys_at` reproduces the host's. Read from the export so the seed
/// does not depend on whether `apply_governor_state` has run yet; the board's
/// own predicate is checked again afterwards in `reconcile_host_envoys`.
fn host_amani_envoy_terms(
    rules: &crate::rules::Rules,
    state: &StateSnapshot,
    minor: &StateMinor,
) -> (f64, f64) {
    let none = (0.0, 1.0);
    let Some(governors) = state.governors.as_deref() else {
        return none;
    };
    let Some(spec) = rules.governors.get("amani") else {
        return none;
    };
    let Some(amani) = governors
        .iter()
        .find(|governor| civvis_governor_name(&governor.kind) == Some("amani"))
    else {
        return none;
    };
    if !amani.established
        || amani.neutralized_turns > 0
        || amani.city_player < 0
        || amani.city_player as usize != minor.player
    {
        return none;
    }
    let effect = |key: &str| -> f64 {
        spec.effects.get(key).copied().unwrap_or(0.0)
            + amani
                .promotions
                .iter()
                .filter(|promotion| {
                    civ6_governor_base_promotion("amani") != Some(promotion.as_str())
                })
                .filter_map(|promotion| civvis_governor_promotion(promotion))
                .filter_map(|promotion| spec.promotions.get(promotion))
                .filter_map(|promotion| promotion.effects.get(key))
                .sum::<f64>()
    };
    (
        effect("city_state_envoys"),
        effect("city_state_envoys_multiplier").max(1.0),
    )
}

/// The delegation to STORE for seat 0 so that `Game::envoys_at` answers `host`.
fn raw_envoys_for(host: i64, (messenger, multiplier): (f64, f64)) -> i64 {
    ((host.max(0) as f64 / multiplier).round() - messenger)
        .round()
        .max(0.0) as i64
}

/// What `Game::envoys_at` answers for a stored delegation under these terms.
fn effective_envoys(raw: i64, (messenger, multiplier): (f64, f64)) -> i64 {
    ((raw as f64 + messenger) * multiplier).round() as i64
}

/// After the governors are on the board: every city-state where the board's
/// `envoys_at` does not answer the host's number is re-seeded against the BOARD's
/// own Amani predicate. This is what makes the seed right whichever side of
/// `apply_governor_state` it was first written on — the host record and the
/// board can disagree (her city unlocated, an establishment the board times
/// differently), and the host's count is the fact either way.
fn reconcile_host_envoys(
    game: &mut crate::game::Game,
    minor_assignments: &[(&StateMinor, usize)],
    seat_of_host: &std::collections::BTreeMap<usize, usize>,
) {
    for &(minor, owner) in minor_assignments {
        if !minor.is_city_state() || game.envoys_at(0, owner) == minor.envoys.max(0) {
            continue;
        }
        let amani = game.amani_envoy_terms(0, owner);
        set_mirrored_envoys(
            &mut game.players[0],
            owner,
            raw_envoys_for(minor.envoys, amani),
        );
        seed_mirrored_suzerainty(game, minor, owner, seat_of_host, amani);
    }
}

fn set_mirrored_envoys(player: &mut crate::game::Player, minor: usize, count: i64) {
    player.envoys.retain(|(seat, _)| *seat != minor);
    if count > 0 {
        player.envoys.push((minor, count));
    }
}

fn mirrored_envoys(player: &crate::game::Player, minor: usize) -> i64 {
    player
        .envoys
        .iter()
        .filter(|(seat, _)| *seat == minor)
        .map(|(_, count)| *count)
        .sum()
}

/// Seed one mirrored city-state's public suzerainty. `minor.suzerain` is a
/// host SEAT ID with `-1` meaning no suzerain; the board instead *derives*
/// suzerainty from envoy counts, so both answers must be constructed. A named
/// holder gets the minimum winning delegation. The `-1` sentinel needs its own
/// construction, not just a skip: our factual delegation (`minor.envoys`) is
/// already seeded and rival delegations are not, so three unopposed envoys of
/// ours elect seat 0 by walkover — measured live on `civvis-20260808T003040Z`:
/// `taruga suzerain Civ6=-1 CIVVIS=0`. Civ 6 reporting none while we hold
/// three or more means some rival at least ties us, so seed that tie on one
/// alive major; fabricated rival delegations from an earlier suzerainty are
/// cleared first so a former holder cannot stay elected either. Seat 0's count
/// is the export's fact and is never touched here.
fn seed_mirrored_suzerainty(
    game: &mut crate::game::Game,
    minor: &StateMinor,
    owner: usize,
    seat_of_host: &std::collections::BTreeMap<usize, usize>,
    amani: (f64, f64),
) {
    if !minor.is_city_state() {
        return;
    }
    if minor.suzerain >= 0 {
        if let Some(&holder) = seat_of_host.get(&(minor.suzerain as usize)) {
            let current = mirrored_envoys(&game.players[holder], owner);
            let winning = if holder == 0 {
                // Seat 0's stored count is net of Amani's terms (see
                // `host_amani_envoy_terms`): the floor is set on the EFFECTIVE
                // delegation and stored net again.
                let target = effective_envoys(current, amani).max(3).max(minor.envoys);
                raw_envoys_for(target, amani).max(current)
            } else {
                current
                    .max(3)
                    .max(minor.most_envoys.max(minor.envoys.max(0) + 1).max(3))
            };
            set_mirrored_envoys(&mut game.players[holder], owner, winning);
        }
        return;
    }
    // The tie is against what the host counts for us — Amani included — and a
    // rival's stored delegation is its effective one.
    let ours = effective_envoys(mirrored_envoys(&game.players[0], owner), amani);
    // Where the export carries every major's count (`envoys_by_player`), the
    // rival delegations on the board are the host's facts and stay; only an
    // older export's fabricated ones are cleared before the tie is seeded.
    let facts = minor.envoys_by_player.is_some();
    if !facts {
        for pid in 1..game.players.len() {
            if !game.players[pid].is_minor {
                set_mirrored_envoys(&mut game.players[pid], owner, 0);
            }
        }
    }
    if ours >= 3 {
        // The rival that ties us: the one the host counts highest where the
        // counts crossed, else the first alive major — and never LOWERED,
        // so a rival the host already counts at or above us keeps its number.
        let blocker = game
            .players
            .iter()
            .filter(|player| player.id != 0 && player.alive && !player.is_minor)
            .max_by_key(|player| (mirrored_envoys(player, owner), std::cmp::Reverse(player.id)))
            .map(|player| player.id);
        if let Some(blocker) = blocker {
            if mirrored_envoys(&game.players[blocker], owner) < ours {
                set_mirrored_envoys(&mut game.players[blocker], owner, ours);
            }
        }
    }
}

/// Apply public host measurements after the reconstructed economy and city
/// roster are complete. Yield differences are stored as corrections so an AI
/// clone can still measure the effect of a candidate policy or building.
fn great_work_kind(object: &str) -> Option<&'static str> {
    match object {
        "GREATWORKOBJECT_WRITING" => Some("writing"),
        "GREATWORKOBJECT_LANDSCAPE" | "GREATWORKOBJECT_PORTRAIT" | "GREATWORKOBJECT_SCULPTURE" => {
            Some("art")
        }
        "GREATWORKOBJECT_RELIGIOUS" => Some("religious_art"),
        "GREATWORKOBJECT_ARTIFACT" => Some("artifact"),
        "GREATWORKOBJECT_MUSIC" => Some("music"),
        "GREATWORKOBJECT_RELIC" => Some("relic"),
        _ => None,
    }
}

/// Normalize Firaxis's per-individual Great Work prerequisite to the work
/// kind used by CIVVIS's housing and production rules.
///
/// The live control mod sends the database spelling, but accepting the bare
/// lower-case spelling keeps the mirror compatible with test fixtures and an
/// older bridge that may already have stripped the prefix. `ART` is included
/// even though the stock database uses the concrete sculpture/portrait/
/// landscape object types; it is a harmless forward-compatible alias.
fn great_person_required_work_kind(object: &str) -> Option<&'static str> {
    let object = object.trim().to_ascii_uppercase();
    match object.as_str() {
        "GREATWORKOBJECT_WRITING" | "WRITING" => Some("writing"),
        "GREATWORKOBJECT_LANDSCAPE"
        | "GREATWORKOBJECT_PORTRAIT"
        | "GREATWORKOBJECT_SCULPTURE"
        | "GREATWORKOBJECT_ART"
        | "LANDSCAPE"
        | "PORTRAIT"
        | "SCULPTURE"
        | "ART" => Some("art"),
        "GREATWORKOBJECT_RELIGIOUS" | "RELIGIOUS" => Some("religious_art"),
        "GREATWORKOBJECT_ARTIFACT" | "ARTIFACT" => Some("artifact"),
        "GREATWORKOBJECT_MUSIC" | "MUSIC" => Some("music"),
        "GREATWORKOBJECT_RELIC" | "RELIC" => Some("relic"),
        _ => None,
    }
}

fn great_work_era(era: Option<&str>) -> usize {
    let bare = era
        .unwrap_or("ERA_ANCIENT")
        .strip_prefix("ERA_")
        .unwrap_or(era.unwrap_or("ANCIENT"));
    crate::rules::ERA_NAMES
        .iter()
        .position(|name| name.eq_ignore_ascii_case(bare))
        .unwrap_or(0)
}

/// The city fire each foreign seat last showed, carried across the boards a
/// `--fresh-board` bridge rebuilds every turn.
///
/// The export reads a rival city's `ranged_strength` only while the seat can
/// see the city (`cityRangedStrength` in CivvisControlAgent.lua: "Never read a
/// fogged rival's strength"), and a city with no reading falls back to the
/// strongest ranged unit of its owner's the board can see — a Builder's 3 when
/// none is in sight. Live King civvis-20261005T024614Z (game 94) besieged
/// Porto from turn 127, where the export read 60 (a Field Cannon civ) on every
/// turn a unit of ours had the city in sight and nothing on the others. On
/// turns 132 and 135 it read nothing, the siege's endurance against the
/// city's strike jumped from 3.1 to 37.2 turns (and from 2.4 to 22.8), the
/// train invested, and a Catapult, an Archer and a Trebuchet walked into
/// Porto's reach one turn apiece and died for 1 to 12 wall points each.
///
/// A human remembers the number on the banner. Each city's last reading is
/// kept by position, under the owner that showed it, so a city that changes
/// hands drops the old owner's fire. Every city of one empire showed the same
/// fire in that game (Porto and Lisbon 40, then 60, then 107 together), so the
/// seat's latest reading across all its cities stands for a city of it never
/// read. Neither reads below what the seat's research lets it train
/// ([`researched_city_fire`]), which still holds for a seat never seen. An
/// observed reading always wins.
///
/// `CIVVIS_NO_CITY_FIRE_MEMORY=1` (or `civvis_orders --no-city-fire-memory`)
/// turns it off; see [`CityFireMemory::disabled_by_env`].
#[derive(Clone, Debug, Default)]
pub struct CityFireMemory {
    turn: Option<u32>,
    /// The last reading at each city position, and the seat that showed it.
    by_city: BTreeMap<crate::Pos, (usize, f64)>,
    /// Each seat's latest reading across all of its cities.
    by_seat: BTreeMap<usize, f64>,
}

impl CityFireMemory {
    /// The live kill switch: a non-empty `CIVVIS_NO_CITY_FIRE_MEMORY` other
    /// than `0` disables the memory without a rebuild.
    pub fn disabled_by_env() -> bool {
        std::env::var("CIVVIS_NO_CITY_FIRE_MEMORY")
            .is_ok_and(|value| !value.is_empty() && value != "0")
    }

    /// Record the fire this board observed, then give every foreign city the
    /// export left unread the strongest of its own remembered fire (same
    /// owner only), its seat's remembered fire and its seat's researched fire.
    pub fn apply(&mut self, game: &mut crate::game::Game) {
        if self.turn.is_some_and(|turn| game.turn < turn) {
            // A replay starts a new timeline.
            self.by_city.clear();
            self.by_seat.clear();
        }
        self.turn = Some(game.turn);
        let mut latest: BTreeMap<usize, f64> = BTreeMap::new();
        for (cid, strength) in game.observed_city_ranged_strength.iter() {
            if let Some(city) = game.cities.get(cid).filter(|city| city.owner != 0) {
                self.by_city.insert(city.pos, (city.owner, *strength));
                let best = latest.entry(city.owner).or_insert(0.0);
                *best = best.max(*strength);
            }
        }
        self.by_seat.extend(latest);
        let fills: Vec<(u32, f64)> = game
            .cities
            .values()
            .filter(|city| {
                city.owner != 0 && !game.observed_city_ranged_strength.contains_key(&city.id)
            })
            .filter_map(|city| {
                let own = self
                    .by_city
                    .get(&city.pos)
                    .filter(|(owner, _)| *owner == city.owner)
                    .map(|(_, strength)| *strength);
                own.into_iter()
                    .chain(self.by_seat.get(&city.owner).copied())
                    .chain(researched_city_fire(game, city.owner))
                    .reduce(f64::max)
                    .map(|strength| (city.id, strength))
            })
            .collect();
        if fills.is_empty() {
            return;
        }
        let observed = Arc::make_mut(&mut game.observed_city_ranged_strength);
        for (cid, strength) in fills {
            observed.insert(cid, strength);
        }
    }
}

/// The ranged strength of the strongest land unit this seat's research lets
/// it train — the engine's own `strongest_ranged_built` once it trains one.
/// Units that need a strategic resource are left out, since the seat may hold
/// none; `None` for a seat with no such unit.
fn researched_city_fire(game: &crate::game::Game, owner: usize) -> Option<f64> {
    let player = game.players.get(owner)?;
    game.rules
        .units
        .values()
        .filter(|spec| {
            spec.class == "military"
                && spec.buildable
                && spec.ranged_strength > 0.0
                && spec.domain.as_deref().is_none_or(|domain| domain == "land")
                && spec.requires_resource.is_none()
                && spec
                    .tech
                    .as_ref()
                    .is_none_or(|tech| player.techs.contains(tech))
                && spec
                    .civic
                    .as_ref()
                    .is_none_or(|civic| player.civics.contains(civic))
                && spec
                    .unique_to
                    .as_deref()
                    .is_none_or(|civ| game.owns_civ_unique(owner, civ))
        })
        .map(|spec| spec.ranged_strength)
        .reduce(f64::max)
}

/// Which of the two host-state passes is running.
///
/// `rebuild_from_state` builds a board from nothing; [`Mirror::sync`] brings an
/// existing one up to date. They apply the SAME readings in the same order, so
/// the shared ones are written once as [`HOST_STATE_STEPS`] and told apart by
/// this.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum MirrorMode {
    Rebuild,
    Sync,
}

/// A step runs on the rebuild pass only.
const REBUILD: u8 = 1;
/// A step runs on the sync pass only.
const SYNC: u8 = 2;
/// A step runs on both passes — what almost every step is.
const BOTH: u8 = REBUILD | SYNC;

impl MirrorMode {
    const fn bit(self) -> u8 {
        match self {
            MirrorMode::Rebuild => REBUILD,
            MirrorMode::Sync => SYNC,
        }
    }
}

/// Where in the pass a step belongs.
///
/// The two passes call the phases in the same order; what each one does BETWEEN
/// the phases is its own business (the rebuild plants cities and units where the
/// sync reconciles them against their Civilization VI ids), and that middle is
/// what still differs between the two functions.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum HostPhase {
    /// Seat identity, before anything is on the board.
    Empire,
    /// The seat's own economy readings, still before the board.
    Economy,
    /// The sync-only mid-pass over ground that was revealed this turn. The
    /// rebuild's board does not exist yet at this point, so it runs nothing —
    /// the call is there so both passes walk the same list of phases.
    Refresh,
    /// The whole-board passes, once every city and unit is planted.
    Board,
    /// Readings that must survive whatever the board passes scored on their way.
    Finish,
}

/// Everything a host-state step is allowed to touch.
///
/// The rebuild path owns its `game`/`unmapped` as locals and the sync path owns
/// them as fields of [`Mirror`]; borrowing them through this is what lets one
/// step body serve both.
pub(crate) struct HostStepCtx<'a> {
    mode: MirrorMode,
    game: &'a mut crate::game::Game,
    snapshot: &'a Snapshot,
    state: &'a StateSnapshot,
    unmapped: &'a mut Vec<String>,
    /// Civ 6 city id → board city id for every RETAINED city, ours and theirs.
    /// Empty until the board phase; no earlier step reads it.
    known_city_ids: &'a std::collections::BTreeMap<i64, u32>,
    /// Empty until the board phase, as `known_city_ids`.
    minor_assignments: &'a [(&'a StateMinor, usize)],
    /// Civ 6 player id → board seat. Empty until the board phase.
    seat_of_host: &'a std::collections::BTreeMap<usize, usize>,
    /// The horizon the rebuild was asked for. Unused on the sync path, which
    /// never rewrites it.
    max_turns: u32,
    frontier_depth: u32,
    /// `CIVVIS_SYNC_NO_TERRAIN`, the sync path's bisect switch. Never set on the
    /// rebuild path.
    skip_terrain: bool,
    /// [`Mirror::last_treasury`], differenced by the sync-only derived income
    /// fallback. A throwaway on the rebuild path, which has no predecessor.
    last_treasury: &'a mut Option<(u32, f64)>,
}

/// Empty stand-ins for the board-phase lookups, which do not exist yet when the
/// empire, economy and refresh phases run.
static NO_CITY_IDS: std::collections::BTreeMap<i64, u32> = std::collections::BTreeMap::new();
static NO_HOST_SEATS: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();

/// The constants of one host-state pass, fixed before the first step runs.
#[derive(Clone, Copy)]
pub(crate) struct HostPass {
    mode: MirrorMode,
    /// The horizon the rebuild was asked for. Unused on the sync path, which
    /// never rewrites it.
    max_turns: u32,
    frontier_depth: u32,
    /// `CIVVIS_SYNC_NO_TERRAIN`, the sync path's bisect switch. Never set on the
    /// rebuild path.
    skip_terrain: bool,
}

impl<'a> HostStepCtx<'a> {
    /// Everything that exists before the board does. The board-phase lookups
    /// start empty; no step before [`HostPhase::Board`] reads one.
    fn new(
        game: &'a mut crate::game::Game,
        snapshot: &'a Snapshot,
        state: &'a StateSnapshot,
        unmapped: &'a mut Vec<String>,
        last_treasury: &'a mut Option<(u32, f64)>,
        pass: HostPass,
    ) -> Self {
        HostStepCtx {
            mode: pass.mode,
            game,
            snapshot,
            state,
            unmapped,
            known_city_ids: &NO_CITY_IDS,
            minor_assignments: &[],
            seat_of_host: &NO_HOST_SEATS,
            max_turns: pass.max_turns,
            frontier_depth: pass.frontier_depth,
            skip_terrain: pass.skip_terrain,
            last_treasury,
        }
    }

    /// The lookups the board phase needs, which are only complete once every
    /// city, rival and minor is on the board.
    fn with_board(
        mut self,
        known_city_ids: &'a std::collections::BTreeMap<i64, u32>,
        minor_assignments: &'a [(&'a StateMinor, usize)],
        seat_of_host: &'a std::collections::BTreeMap<usize, usize>,
    ) -> Self {
        self.known_city_ids = known_city_ids;
        self.minor_assignments = minor_assignments;
        self.seat_of_host = seat_of_host;
        self
    }
}

/// One entry of [`HOST_STATE_STEPS`]: its NAME, the passes that take it, and
/// what it does.
///
/// The name is what the tests record, so a step that appears on one pass only
/// is a failure with a name on it rather than a live seat that quietly drifts.
/// A tuple rather than a struct so the table below stays a readable list.
type HostStep = (&'static str, u8, fn(&mut HostStepCtx<'_>));

/// ⚠⚠ THE ORDERED HOST-STATE STEP LIST — THE ONE COPY.
///
/// `rebuild_from_state` and [`Mirror::sync`] used to carry a private copy of
/// this each, ~1,000 lines apiece calling 26 and 25 `apply_*` helpers. Every
/// one-to-one mapping of a new Civilization VI reading had to be written twice
/// and a missed second edit desynced a live seat with nothing to see. The order
/// and the per-pass membership below are exactly what those two bodies did.
///
/// Reading it: `ON_BOTH` is the normal case. `ON_REBUILD` / `ON_SYNC` mark the
/// places the two passes genuinely differ, and each one has a reason on it.
/// The single `apply_*` helper only one pass calls is `apply_seat_victories`
/// (rebuild only: the host's enabled victory conditions cannot change mid-game,
/// so the sync never re-reads them).
///
/// NOT steps, deliberately: the refusal/menu wiring — `blocked_districts`,
/// `host_*_sites`, `blocked_wonders`, `replace_blocked_production`,
/// `replace_host_menus`, `replace_blocked_purchases`, `blocked_promotions` and
/// the live-spy seating. The rebuild REPLACES those from locals that only exist
/// once every city is planted (so it runs them between `Board` and `Finish`);
/// the sync UNIONS some of them into what the caller already added and runs
/// them before `Empire`. Different position, different bodies — folding them in
/// would hide the difference rather than state it.
const HOST_STATE_STEPS: &[(HostPhase, &[HostStep])] = &[
    (
        HostPhase::Empire,
        &[
            ("game_speed", REBUILD, step_game_speed),
            ("seat_victories", REBUILD, step_seat_victories),
            ("difficulty", REBUILD, step_difficulty),
            ("human_seat", REBUILD, step_human_seat),
            ("map_script", REBUILD, step_map_script),
            ("refused_site_blocks", REBUILD, step_refused_site_blocks),
            ("policy_choices", BOTH, step_policy_choices),
            ("identity", BOTH, step_identity),
        ],
    ),
    (
        HostPhase::Economy,
        &[
            ("turn_and_score", BOTH, step_turn_and_score),
            ("max_turns", REBUILD, step_max_turns),
            // ⚠ The treasury is read BEFORE the maintenance bill on the
            // rebuild and AFTER it on the sync. Both orders are what shipped;
            // neither helper reads what the other writes (`host_maintenance`
            // is its own map, the gold lands on `players[0]`), so the two
            // entries stay where they were rather than being moved onto one
            // line and quietly changing a live board.
            ("host_gold", REBUILD, step_host_gold),
            ("host_maintenance", BOTH, step_host_maintenance),
            ("host_gold", SYNC, step_host_gold),
            ("faith_and_dvp", BOTH, step_faith_and_dvp),
            ("congress_dvp", BOTH, step_congress_dvp),
            ("host_competitions", BOTH, step_host_competitions),
            ("diplomatic_favor", BOTH, step_diplomatic_favor),
            ("mirrored_envoys_free", BOTH, step_mirrored_envoys_free),
            ("player_religion", BOTH, step_player_religion),
        ],
    ),
    (
        HostPhase::Refresh,
        &[
            // Newly revealed ground, and the traversability prior redrawn
            // beyond it. The rebuild has nothing to refresh: it has not planted
            // a city yet, and its own terrain pass is the board phase below.
            ("terrain", SYNC, step_terrain),
            ("territory", SYNC, step_territory),
            ("city_memory", SYNC, step_city_memory),
        ],
    ),
    (
        HostPhase::Board,
        &[
            // ⚠ The trade routes are restored BEFORE the terrain passes on the
            // sync and after `city_memory` on the rebuild. Kept where each pass
            // had them.
            ("trade_routes", SYNC, step_trade_routes),
            ("terrain", BOTH, step_terrain),
            ("territory", BOTH, step_territory),
            ("tile_memory", BOTH, step_tile_memory),
            ("city_memory", BOTH, step_city_memory),
            ("trade_routes", REBUILD, step_trade_routes),
            ("religion_identity", BOTH, religion_state::apply),
            ("governor_state", BOTH, step_governor_state),
            ("host_envoys", BOTH, step_host_envoys),
            ("host_war_types", BOTH, step_host_war_types),
            ("rival_wars", BOTH, step_rival_wars),
            ("great_person_points", BOTH, step_great_person_points),
            ("strategic_stockpiles", BOTH, step_strategic_stockpiles),
            ("player_ages", BOTH, step_player_ages),
            ("host_congress", BOTH, step_host_congress),
            // Climate changes the yields of flooded plots. It must be applied
            // before host-to-model city and empire calibration below, or a
            // later flood invalidates the correction measured on the old map.
            ("host_climate", BOTH, step_host_climate),
            ("observed_host_metrics", BOTH, step_observed_host_metrics),
            ("loyalty_doomed_sites", BOTH, step_loyalty_doomed_sites),
        ],
    ),
    (
        HostPhase::Finish,
        &[
            ("player_ages", BOTH, step_player_ages),
            ("district_progress", BOTH, step_district_progress),
            ("record_host_observed", BOTH, step_record_host_observed),
        ],
    ),
];

/// The steps of one phase, in the order [`HOST_STATE_STEPS`] lists them.
fn steps_of(phase: HostPhase) -> &'static [HostStep] {
    HOST_STATE_STEPS
        .iter()
        .find(|(listed, _)| *listed == phase)
        .map(|(_, steps)| *steps)
        // A phase with no entry would silently apply nothing, which is the very
        // failure this table exists to catch — so the recorded-order test names
        // every phase and every step it must hold.
        .unwrap_or_default()
}

/// Run every step of `phase` that `ctx.mode` takes, in table order.
fn run_host_steps(ctx: &mut HostStepCtx<'_>, phase: HostPhase) {
    // The list, named, on a live seat: `CIVVIS_MIRROR_STEP_TRACE=1`. Same switch
    // culture as the `CIVVIS_SYNC_NO_*` bisects — which step a board picked up a
    // wrong reading from is otherwise invisible from outside the process.
    let trace = std::env::var("CIVVIS_MIRROR_STEP_TRACE").is_ok();
    for &(name, modes, run) in steps_of(phase) {
        if modes & ctx.mode.bit() != 0 {
            if trace {
                eprintln!("[host-step] {:?} {:?} {name}", ctx.mode, phase);
            }
            run(ctx);
        }
    }
}

/// The ordered step names `mode` runs in `phase` — the driver's own walk, named.
#[cfg(test)]
pub(crate) fn host_step_names(mode: MirrorMode, phase: HostPhase) -> Vec<&'static str> {
    steps_of(phase)
        .iter()
        .filter(|(_, modes, _)| modes & mode.bit() != 0)
        .map(|(name, _, _)| *name)
        .collect()
}

// --- the steps ----------------------------------------------------------
// One body each, whatever the pass. A `ctx.mode` test inside a step is a real
// difference between the two passes and carries its reason.

fn step_district_progress(ctx: &mut HostStepCtx<'_>) {
    let city_ids = ctx
        .state
        .cities
        .iter()
        .filter_map(|city| {
            let ours = |cid: &u32| ctx.game.cities.get(cid).is_some_and(|city| city.owner == 0);
            // Our own centre names the city planted for this export in the same
            // pass. Civilization VI numbers cities per player, so the native id
            // alone is ambiguous: Bogotá's 65536 is every other player's first
            // city too, and `known_city_ids` keeps whichever owner was planted
            // last (live: 58 of 75 of our cities shared an id with a visible
            // rival or city-state). The id is only the fallback, and only when
            // it still names a city of ours.
            let cid = ctx
                .game
                .city_at(crate::hex::offset_to_axial(city.x, city.y))
                .filter(ours)
                .or_else(|| {
                    (city.id > 0)
                        .then(|| ctx.known_city_ids.get(&city.id).copied())
                        .flatten()
                        .filter(ours)
                })?;
            Some((cid, city.id))
        })
        .collect();
    apply_host_district_progress(ctx.game, &ctx.state.cities, &city_ids);
}

fn step_game_speed(ctx: &mut HostStepCtx<'_>) {
    if let Some(speed) = civvis_game_speed(&ctx.state.seat.speed) {
        // These are deliberately redundant in `Game` for save compatibility.
        // The viewer renders `game_speed`; a number of rules still use `speed`
        // to find the speed spec. A half-update is therefore a visual lie or a
        // mathematical one depending on which path reads it first.
        ctx.game.speed = speed.id().to_string();
        ctx.game.game_speed = speed;
    }
}

fn step_seat_victories(ctx: &mut HostStepCtx<'_>) {
    apply_seat_victories(ctx.game, &ctx.state.seat);
}

fn step_difficulty(ctx: &mut HostStepCtx<'_>) {
    if let Some(difficulty) = civvis_difficulty(&ctx.state.seat.difficulty)
        .filter(|difficulty| ctx.game.rules.difficulties.contains_key(difficulty))
    {
        ctx.game.difficulty = difficulty;
    }
}

fn step_human_seat(ctx: &mut HostStepCtx<'_>) {
    // ★★★★ THE MIRRORED SEAT IS THE HUMAN. The difficulty ladder pays its
    // yield, combat, experience and era-boost handicaps to the AI seats and
    // withholds them from the human, and on the host the seat this board
    // plans for IS the human. Nothing here ever said so, so every mirrored
    // board paid seat 0 King's own AI bonus: measured over the 150 turns of
    // run civvis-20260826T184456Z with `tools/civ6_yield_drift.py`, the
    // model's city production, gold read exactly 1.20× the host's and science,
    // culture and faith 1.08× — `ai_yield_pct` to the digit, food untouched
    // because food is never handicapped — in 288 of 299 persistent episodes.
    // The board's corrected totals hid it; every plan the seat priced on its
    // own model was priced in a currency 8–20 % richer than the host pays.
    ctx.game.human_seats.insert(0);
}

fn step_map_script(ctx: &mut HostStepCtx<'_>) {
    if let Some(map_script) = civvis_map_script(&ctx.state.seat.map) {
        ctx.game.map_script = map_script;
    }
}

fn step_policy_choices(ctx: &mut HostStepCtx<'_>) {
    apply_host_policy_choices(ctx.game, ctx.state);
}

fn step_refused_site_blocks(ctx: &mut HostStepCtx<'_>) {
    // Sites the host engine has already rejected, so the planner stops re-deriving
    // them. See `refused_sites_of_kind_through`.
    ctx.game.blocked_city_sites = Arc::new(ctx.state.refused_sites.clone());
    ctx.game.blocked_improvement_sites = Arc::new(ctx.state.refused_improves.clone());
    ctx.game.blocked_improvements = Arc::new(ctx.state.refused_improvement_names.clone());
    ctx.game.blocked_trade_routes = Arc::new(ctx.state.refused_trade_routes.clone());
    let policies = blocked_policies_from(&ctx.state.refused_policy_names, &ctx.game.rules);
    ctx.game.blocked_policies = Arc::new(policies);
    let pantheons = blocked_pantheons_from(&ctx.state.refused_pantheons, &ctx.game.rules);
    ctx.game.blocked_pantheons = Arc::new(pantheons);
    // ⚠ The district/wonder/production half needs `city_ids`, which is only
    // complete once every city is planted, so it is wired after the board phase
    // rather than here. See the note on `HOST_STATE_STEPS`.
}

fn step_identity(ctx: &mut HostStepCtx<'_>) {
    // Identity first: city naming reads it, so this cannot wait until after the
    // cities are placed. See `apply_identity`.
    //
    // Rivals are met as the game goes on, so it is not a one-time job at
    // reconstruction either: a civilization first seen on turn 90 arrives here.
    let unresolved = apply_identity(ctx.game, ctx.state);
    if ctx.mode == MirrorMode::Rebuild {
        // Only the rebuild reports and files them; the sync path's ledger was
        // already seeded from the reconstruction and would repeat the line every
        // turn for the rest of the game.
        if !unresolved.is_empty() {
            eprintln!(
                "mirror: no CIVVIS civilization for {unresolved:?} — those seats keep their \
                 default roster name and will NOT match the Civilization VI screen"
            );
        }
        ctx.unmapped.extend(unresolved);
    }
}

fn step_turn_and_score(ctx: &mut HostStepCtx<'_>) {
    // ★★★★ TELL CIVVIS WHAT TURN IT IS. `Game::new` starts at the beginning, and the
    // board is rebuilt from scratch every turn, so without this CIVVIS was answering
    // TURN 1 for the whole game — every time. Measured consequence on run
    // civvis-20260730T111953Z: 15 production orders, ALL of them Warrior, no settler
    // and no district, while its own plan asked for 3 cities. An agent whose strategy
    // is keyed to era and timing cannot plan from a clock stuck at zero.
    ctx.game.turn = ctx.state.turn.max(1);
    Arc::make_mut(&mut ctx.game.observed_score).clear();
    Arc::make_mut(&mut ctx.game.observed_military_power).clear();
    if ctx.state.score >= 0 {
        Arc::make_mut(&mut ctx.game.observed_score).insert(0, ctx.state.score);
    }
    if ctx.state.military.is_finite() && ctx.state.military >= 0.0 {
        Arc::make_mut(&mut ctx.game.observed_military_power).insert(0, ctx.state.military);
    }
}

fn step_max_turns(ctx: &mut HostStepCtx<'_>) {
    // ★★★ AND HOW MANY TURNS ARE LEFT. `rebuild_game` hardcodes 500; the game's real
    // limit is the seat's (`host_turn_limit`: the engine readback, else the speed's
    // own), and the HUD prints it as TURN n/<limit>. Callers pass that in.
    // CIVVIS keys several windows on the remaining turns — `expansion_pays_back_for`
    // asks whether a settler can still pay for itself before the game ends, and
    // `expansion_window_open` reserves the endgame — so a horizon that is twice too
    // long makes late expansion look affordable when it is not, and distorts every
    // build-versus-fight trade in the other direction too.
    //
    // The sync never rewrites it: the horizon is the reconstruction's answer.
    ctx.game.max_turns = ctx.max_turns;
}

fn step_host_gold(ctx: &mut HostStepCtx<'_>) {
    // The treasury and each city's population are read by CIVVIS's buy and build
    // decisions. Defaults made a 20-population empire with 600 gold look like a
    // founding settlement.
    if ctx.state.gold < 0 {
        // ⚠ The rebuild wrote the rate even without a treasury reading. Keeping
        // that: `gold_per_turn` is an independent export field.
        if ctx.mode == MirrorMode::Rebuild {
            if let Some(net) = ctx.state.gold_per_turn.filter(|net| net.is_finite()) {
                ctx.game.players[0].gold_per_turn = net;
            }
        }
        return;
    }
    ctx.game.players[0].gold = ctx.state.gold as f64;
    // ⚠ THE FRESH-BOARD PATH IS THE ONE THAT MATTERS. `civvis_orders --serve
    // --fresh-board` comes through here every turn, and this rebuild has no
    // predecessor to difference against, so before this line `gold_per_turn` was
    // whatever `Player::default` said — 0 — in every live decision.
    //
    // The host's own figure first: it needs no history and so survives
    // `--fresh-board`, which is what kills the derived rate. The sync falls back
    // to differencing consecutive treasuries only when Firaxis did not answer;
    // the rebuild has no predecessor and so has no fallback.
    if let Some(net) = ctx.state.gold_per_turn.filter(|net| net.is_finite()) {
        ctx.game.players[0].gold_per_turn = net;
    } else if ctx.mode == MirrorMode::Sync {
        let turn = ctx.game.turn;
        let gold = ctx.state.gold as f64;
        if let Some(net) = mirror_net_income_from(ctx.last_treasury, turn, gold) {
            ctx.game.players[0].gold_per_turn = net;
        }
    }
}

fn step_host_maintenance(ctx: &mut HostStepCtx<'_>) {
    apply_host_maintenance(ctx.game, ctx.state);
}

fn step_faith_and_dvp(ctx: &mut HostStepCtx<'_>) {
    if ctx.state.faith >= 0 {
        ctx.game.players[0].faith = ctx.state.faith as f64;
    }
    if let Some(dvp) = ctx.state.dvp {
        ctx.game.players[0].dvp = dvp;
    }
}

fn step_congress_dvp(ctx: &mut HostStepCtx<'_>) {
    apply_congress_dvp(ctx.game, ctx.state);
}

fn step_host_competitions(ctx: &mut HostStepCtx<'_>) {
    apply_host_competitions(ctx.game, ctx.state);
}

fn step_diplomatic_favor(ctx: &mut HostStepCtx<'_>) {
    if let Some(favor) = ctx.state.favor.filter(|favor| favor.is_finite()) {
        ctx.game.players[0].diplomatic_favor = favor;
    }
}

fn step_mirrored_envoys_free(ctx: &mut HostStepCtx<'_>) {
    apply_mirrored_envoys_free(ctx.game, ctx.state);
}

fn step_player_religion(ctx: &mut HostStepCtx<'_>) {
    apply_player_religion(ctx.game, ctx.state, ctx.unmapped);
}

fn step_terrain(ctx: &mut HostStepCtx<'_>) {
    // `place_city` applies native founding rules and clears removable features
    // from the centre. Firaxis is authoritative here: real city centres can
    // retain Floodplains, so restore every exported plot after all cities exist.
    if ctx.skip_terrain {
        return;
    }
    apply_terrain(ctx.game, ctx.snapshot);
    if ctx.mode == MirrorMode::Sync {
        // Terrain that was already known does not change, but the frontier has
        // to be recomputed because its edge just moved. The rebuild grows its
        // frontier once, before the board is planted.
        grow_frontier(ctx.game, ctx.snapshot, ctx.frontier_depth);
    }
}

fn step_territory(ctx: &mut HostStepCtx<'_>) {
    // ★★★★ BORDERS MOVE, AND THIS USED TO LEARN THEM ONCE AND NEVER AGAIN.
    //
    // `apply_territory` ran only in `rebuild_from_state`, which a persistent mirror
    // calls exactly once — at construction. Every border that grew afterwards, and
    // every owned plot revealed afterwards, stayed unowned on CIVVIS's board for
    // the rest of the game.
    //
    // Measured on live run civvis-20260801T012454Z at turn 43, over the 243 plots
    // paired between the export and the board:
    //
    //     Civ 6 says OWNED, CIVVIS says unowned : 28
    //     CIVVIS says OWNED, Civ 6 says unowned :  0
    //     agreement                              : 88.5%
    //
    // ⚠ The error has a direction, and it is the expensive one.
    // `Game::valid_improvements` returns an empty list for a tile whose
    // `owner_city` is None, so a builder standing on ground the seat really owns is
    // offered NOTHING to build there — the empire silently stops developing the
    // land it just took.
    apply_territory(ctx.game, ctx.snapshot, ctx.state);
}

fn step_tile_memory(ctx: &mut HostStepCtx<'_>) {
    // ⚠ AFTER territory, not before. `apply_terrain` already recorded the seat's memory
    // of every revealed plot, but ownership is written there — so a memory taken earlier
    // would say every fogged tile is unowned, and `obs.rs` reads `memory.owner` for
    // exactly those tiles. Re-recording is idempotent and costs one pass over the
    // revealed set.
    apply_tile_memory(ctx.game, ctx.snapshot);
}

fn step_city_memory(ctx: &mut HostStepCtx<'_>) {
    // ⚠ AFTER every city is planted, ours and the rivals', or the seat remembers only
    // the ones that happened to exist earlier in the pass. Every sync too, not just
    // the rebuild: rival cities are placed as they are revealed.
    apply_city_memory(ctx.game);
}

fn step_trade_routes(ctx: &mut HostStepCtx<'_>) {
    // Firaxis leaves an active Trader on the map, while CIVVIS normally removes
    // it into `game.routes`.  Reconstruct the economic state here and retain the
    // physical unit; the planner removes only active-route traders from its
    // temporary clone.
    let active = restore_active_trade_routes(ctx.game, &ctx.state.trade_routes, ctx.known_city_ids);
    let incoming = restore_incoming_foreign_routes(ctx.game, &ctx.state.cities);
    match ctx.mode {
        MirrorMode::Rebuild => {
            ctx.unmapped.extend(active);
            ctx.unmapped.extend(incoming);
            restore_rival_outgoing_routes(ctx.game, &ctx.state.rivals);
            reconcile_incoming_route_deltas(ctx.game, &ctx.state.cities);
            let options = restore_route_options(
                ctx.game,
                ctx.state.route_options.as_deref(),
                ctx.known_city_ids,
            );
            ctx.unmapped.extend(options);
        }
        MirrorMode::Sync => {
            // ⚠ The sync's ledger persists across turns, so an issue it already
            // holds must not be filed again — and the rival routes are restored
            // last here because they contribute nothing to file.
            let options = restore_route_options(
                ctx.game,
                ctx.state.route_options.as_deref(),
                ctx.known_city_ids,
            );
            for issue in active.into_iter().chain(incoming).chain(options) {
                if !ctx.unmapped.contains(&issue) {
                    ctx.unmapped.push(issue);
                }
            }
            restore_rival_outgoing_routes(ctx.game, &ctx.state.rivals);
            reconcile_incoming_route_deltas(ctx.game, &ctx.state.cities);
        }
    }
}

fn step_governor_state(ctx: &mut HostStepCtx<'_>) {
    apply_governor_state(ctx.game, ctx.state, ctx.unmapped);
}

fn step_host_envoys(ctx: &mut HostStepCtx<'_>) {
    reconcile_host_envoys(ctx.game, ctx.minor_assignments, ctx.seat_of_host);
    // Refresh on BOTH reconstruction paths, including same-turn frames.
    // An omitted city-state or unknown permission must not retain an older
    // grant/refusal. Keys use mapped board seats, never raw host ids.
    let permissions = Arc::make_mut(&mut ctx.game.host_envoy_permissions);
    permissions.retain(|(actor, _), _| *actor != 0);
    for &(minor, owner) in ctx.minor_assignments {
        if minor.is_city_state() {
            if let Some(allowed) = minor.can_send_envoy {
                permissions.insert((0, owner), (ctx.game.turn, allowed));
            }
        }
    }
}

/// Who each met rival is fighting besides us. See [`apply_rival_wars`].
fn step_rival_wars(ctx: &mut HostStepCtx<'_>) {
    apply_rival_wars(ctx.game, ctx.state, ctx.seat_of_host);
}

fn step_host_war_types(ctx: &mut HostStepCtx<'_>) {
    let permissions = Arc::make_mut(&mut ctx.game.host_war_type_permissions);
    permissions.retain(|(actor, _), _| *actor != 0);
    for rival in &ctx.state.rivals {
        let (Some(facts), Some(&owner)) = (
            rival.war_declarations.as_ref(),
            ctx.seat_of_host.get(&rival.player),
        ) else {
            continue;
        };
        permissions.insert(
            (0, owner),
            crate::game::ObservedWarTypes {
                turn: ctx.game.turn,
                permissions: facts
                    .iter()
                    .filter_map(|fact| {
                        fact.allowed
                            .map(|allowed| (fact.statement.clone(), allowed))
                    })
                    .collect(),
            },
        );
    }
}

fn step_great_person_points(ctx: &mut HostStepCtx<'_>) {
    apply_great_person_points(ctx.game, ctx.state, ctx.unmapped);
}

fn step_strategic_stockpiles(ctx: &mut HostStepCtx<'_>) {
    apply_strategic_stockpiles(ctx.game, ctx.state, ctx.unmapped);
}

fn step_player_ages(ctx: &mut HostStepCtx<'_>) {
    // The age and its Dedications change what the model pays (Heartbeat of
    // Steam's Campus Production, Free Inquiry's Science), so they must be on
    // the seat BEFORE the host-to-model corrections are measured, or the
    // correction is taken against a Normal-Age model and paid on top of a
    // Golden-Age one — Ravenna read 14.5 Science against the host's 9.5 on
    // run civvis-20260816T175306Z.
    //
    // ⚠ The `Finish` phase repeats it, and deliberately so: founding a city
    // AWARDS ERA SCORE — a four-city Rome arrived at Firaxis's 31 plus five of
    // CIVVIS's own. Firaxis's number is the reading; anything the pass scored
    // along the way is an artefact of how the board was assembled, so the
    // host's answer is written after it rather than before. The two calls are
    // idempotent with each other.
    apply_player_ages(ctx.game, ctx.state);
}

fn step_host_congress(ctx: &mut HostStepCtx<'_>) {
    // The host's World Congress, likewise before the corrections: Trade Policy
    // and Luxury Policy change what the model pays and supplies.
    apply_host_congress(ctx.game, ctx.state, ctx.seat_of_host, ctx.unmapped);
}

fn step_observed_host_metrics(ctx: &mut HostStepCtx<'_>) {
    apply_observed_host_metrics(ctx.game, ctx.state, Some(ctx.snapshot), ctx.unmapped);
    // Foreign infrastructure lands before city-banner facts. Resolve missing
    // district pools again after their fallback wall maximum is imported, on
    // both rebuild and sync. Measured district pools still take precedence.
    let forts: Vec<_> = ctx
        .game
        .cities
        .values()
        .filter(|city| city.owner != 0)
        .flat_map(|city| {
            city.districts
                .iter()
                .filter(|(kind, _)| ctx.game.district_has_defenses(**kind))
                .map(|(_, position)| (city.id, *position))
        })
        .collect();
    for (cid, position) in forts {
        if let Some(plot) = ctx
            .snapshot
            .plot(crate::hex::axial_to_offset(position.0, position.1))
        {
            apply_foreign_encampment_health(ctx.game, cid, plot);
        }
    }
}

fn step_loyalty_doomed_sites(ctx: &mut HostStepCtx<'_>) {
    settlement_loyalty::apply(ctx.game, ctx.snapshot, ctx.state);
    block_loyalty_doomed_settler_sites(ctx.game);
}

fn step_host_climate(ctx: &mut HostStepCtx<'_>) {
    // The host's climate needs the finished map (the lowland bands) and the
    // finished city roster (a Flood Barrier keeps its ground).
    apply_host_climate(ctx.game, ctx.state);
    // Phase alone cannot describe protected tiles or the host's exact timing.
    // Apply observations LAST, on both rebuild and sync, before yield calibration.
    // Missing fields retain the legacy phase-derived fallback.
    for plot in ctx.snapshot.revealed.values() {
        if let Some(tile) = ctx
            .game
            .map
            .tiles
            .get_mut(&crate::hex::offset_to_axial(plot.x, plot.y))
        {
            if let Some(flooded) = plot.flooded {
                tile.flooded = flooded;
            }
            if let Some(submerged) = plot.submerged {
                tile.submerged = submerged;
            }
        }
    }
}

fn step_record_host_observed(ctx: &mut HostStepCtx<'_>) {
    // Last, because it reads the finished board: every rival, minor and
    // barbarian for this turn is on it by now, and the previous turn's
    // sightings were removed with them.
    strategic_income::apply(ctx.game, ctx.state, ctx.unmapped);
    apply_research_quotes(ctx.game, ctx.state, ctx.unmapped);
    research_tree::apply(ctx.game, ctx.state, ctx.unmapped);
    record_host_observed(ctx.game, ctx.snapshot);
    air_fog_targets::apply(ctx.game, ctx.snapshot);
}

/// The derived net income, and the treasury reading the next call differences
/// against. Split out of [`Mirror::mirror_net_income`] so the shared
/// `host_gold` step can reach it with only `last_treasury` borrowed.
fn mirror_net_income_from(
    last_treasury: &mut Option<(u32, f64)>,
    turn: u32,
    gold: f64,
) -> Option<f64> {
    let previous = last_treasury.replace((turn, gold));
    let (last_turn, last_gold) = previous?;
    if turn != last_turn + 1 {
        return None;
    }
    let delta = gold - last_gold;
    match gold <= 0.0 {
        true => Some(delta.min(-1.0)),
        false => Some(delta),
    }
}

/// Rebuild terrain, both empires, and everything visible of the rivals.
pub fn rebuild_from_state(
    snapshot: &Snapshot,
    state: &StateSnapshot,
    players: usize,
    seed: u64,
    max_turns: u32,
    frontier_depth: u32,
) -> Reconstruction {
    let mut game = rebuild_game_with_city_states(
        snapshot,
        players.max(2),
        seed,
        state.seat.city_states.max(
            state
                .minors
                .iter()
                .filter(|minor| minor.is_city_state())
                .count(),
        ),
    );
    // Keep the generated minor player slots, but none of their generated cities
    // or territory describes the Firaxis world being mirrored.
    game.clear_mirror_cities();

    // ⚠ ONE ORDERED STEP LIST, WALKED BY BOTH PASSES. Everything this function
    // and `Mirror::sync` apply from the host state lives in `HOST_STATE_STEPS`,
    // written once and told apart by `MirrorMode`. What the two passes do
    // BETWEEN the phases is still their own — this one plants a board from
    // nothing, the other reconciles one against its Civilization VI ids.
    let pass = HostPass {
        mode: MirrorMode::Rebuild,
        max_turns,
        frontier_depth,
        // The bisect switch belongs to the sync path; a rebuild always applies
        // terrain.
        skip_terrain: false,
    };
    // A rebuild has no predecessor to difference a treasury against, so the
    // derived-income fallback has nothing to keep and this is never read back.
    let mut no_treasury: Option<(u32, f64)> = None;
    let mut unmapped: Vec<String> = Vec::new();
    run_host_steps(
        &mut HostStepCtx::new(
            &mut game,
            snapshot,
            state,
            &mut unmapped,
            &mut no_treasury,
            pass,
        ),
        HostPhase::Empire,
    );

    // ★★★★★ CLEAR THE STARTING UNITS `Game::new` HANDS OUT. This is the root of the
    // economy failure, and it is invisible from the outside.
    //
    // A new CIVVIS game gives every player a settler and a warrior. This
    // reconstruction then adds the REAL units on top, so CIVVIS's board permanently
    // contains a settler that does not exist in Civilization VI — measured directly:
    // `phantom=[1:settler,2:warrior]`.
    //
    // Two consequences, and the second is the expensive one:
    //
    //   * CIVVIS marches the phantom settler toward a site every turn ("Settler
    //     marching to (-1, 21), 5 tiles away, the site is worth 70.8") and those orders
    //     name a unit this bridge cannot map, so they are dropped as `unit_not_mapped`
    //     and nothing moves in the real game.
    //   * `advanced_units` computes `decline_settlers = counts.settlers > 0`, so with a
    //     phantom settler always present CIVVIS **never builds a real one**. One city
    //     for a whole game against a plan asking for five, and 26 turns out of 182 with
    //     any settler alive at all.
    //
    // Every unit on this board must come from the export, or CIVVIS is planning with an
    // army that does not exist.
    let starting: Vec<u32> = game.units.keys().copied().collect();
    for uid in starting {
        game.remove_unit(uid);
    }

    let mut unit_ids = std::collections::BTreeMap::new();
    let mut foreign_unit_ids: std::collections::BTreeMap<i64, u32> = Default::default();
    let mut city_ids = std::collections::BTreeMap::new();
    // Every retained city, including visible rivals.  `city_ids` stays our-city
    // only because order translation must never point a purchase at a rival;
    // active international routes need the broader lookup.
    let mut known_city_ids = std::collections::BTreeMap::new();
    unmapped.extend(state.schema_gaps.iter().cloned());
    let mut placed_cities = 0;
    let mut placed_units = 0;
    let mut placed_rival_cities = 0;
    let mut placed_rival_units = 0;
    let mut placed_minor_cities = 0;

    // Land only, and revealed only. `place_city` on water or on an unseen tile
    // would put CIVVIS's empire somewhere the seat cannot act.
    let plant_city = |game: &mut crate::game::Game, owner: usize, c: &StateCity| -> Option<u32> {
        if !snapshot.is_revealed((c.x, c.y)) {
            return None;
        }
        let pos = crate::hex::offset_to_axial(c.x, c.y);
        let water = game
            .map
            .get(pos)
            .map(|tile| game.rules.is_water(tile))
            .unwrap_or(true);
        if water || game.city_at(pos).is_some() {
            return None;
        }
        Some(game.place_city(owner, pos, banner(c)))
    };

    // ★★★★★ THE FRONTIER RING — without it CIVVIS believes it lives on a tiny island.
    //
    // `rebuild_game` makes every unrevealed plot explicit UNKNOWN. That is honest but
    // intentionally impassable until this separate prior marks the bounded frontier
    // as worth probing. Without the prior, now that CIVVIS is the DECIDER,
    // a seat that has revealed 51 plots sees a 51-tile island: nowhere to expand,
    // nowhere to explore, nothing to build but soldiers.
    //
    // Measured on run bisect1-114111Z: `desired_cities = 3`, and at turn 34 the empire
    // was still ONE city with 33 production orders, every one of them a Warrior.
    //
    // A bounded traversability assumption lets expansion and exploration aim OUTWARD
    // without assigning land, water, yields, or a continent underneath. An order the
    // real terrain cannot support is refused by Civilization VI and counted.
    grow_frontier(&mut game, snapshot, frontier_depth);

    // The seat's own economy readings, still before the board. See
    // `HOST_STATE_STEPS`.
    run_host_steps(
        &mut HostStepCtx::new(
            &mut game,
            snapshot,
            state,
            &mut unmapped,
            &mut no_treasury,
            pass,
        ),
        HostPhase::Economy,
    );
    // Cheap: `rules` is an Arc. Cloned so the city loop below can consult it while
    // holding a mutable borrow of `game`.
    let game_rules = std::sync::Arc::clone(&game.rules);

    // ★ Research first: what a seat KNOWS bounds what it can sensibly do, and a
    // CIVVIS player with an empty tree recommends Slingers in the Medieval era.
    // ⚠ Names that do not exist in the CIVVIS ruleset are counted, not ignored —
    // a silently dropped tech is a capability CIVVIS will not use and nobody sees.
    for civ6 in &state.techs {
        match civvis_node_name(&game.rules.techs, civ6, "TECH_") {
            Some(name) => {
                game.players[0].techs.insert(crate::name::Name::new(&name));
            }
            None => {
                if !unmapped.contains(civ6) {
                    unmapped.push(civ6.clone());
                }
            }
        }
    }
    if let Some(projects) =
        completed_strategic_projects(state.science_projects.as_deref(), &mut unmapped)
    {
        game.players[0].science_projects = projects;
    }
    for civ6 in &state.policies {
        match civvis_node_name(&game.rules.policies, civ6, "POLICY_") {
            Some(name) => {
                game.players[0]
                    .policies
                    .insert(crate::name::Name::new(&name));
            }
            None => {
                if !unmapped.contains(civ6) {
                    unmapped.push(civ6.clone());
                }
            }
        }
    }
    if let Some(civ6) = &state.pantheon {
        let name = civ6
            .strip_prefix("BELIEF_")
            .unwrap_or(civ6)
            .to_ascii_lowercase();
        // ⚠⚠ `player.pantheon` IS THE FIELD THAT GATES THE DECISION, and the first
        // version of this set `religion_beliefs` instead — which carries the belief
        // for effects but leaves `do_choose_pantheon`'s `players[pid].pantheon.is_some()`
        // check false, so CIVVIS went on asking for a pantheon every turn. Measured
        // after that first fix: `pantheon_already_founded` 32 times in 41 turns — the
        // mod refusing correctly while the mirror kept producing the order.
        //
        // Both are set: the gate so the decision stops being re-made, and the belief
        // list so its effects are counted.
        if game.players[0].pantheon.is_none() {
            game.players[0].pantheon = Some(name.clone());
        }
        if !game.players[0].religion_beliefs.contains(&name) {
            game.players[0].religion_beliefs.push(name);
        }
    }
    if let Some(civ6) = &state.government {
        match civvis_node_name(&game.rules.governments, civ6, "GOVERNMENT_") {
            Some(name) => {
                // ⚠⚠ AND DROP CARDS THE NEW CONSTITUTION CANNOT HOLD. A
                // government defines the slot SHAPE, and `policies_fit` is
                // enforced only when a card is SLOTTED — so a deck legal under
                // the old government survived the change and CIVVIS re-sent it
                // every turn. See `prune_policies_to_government`.
                let changed = game.players[0].government.as_ref() != Some(&name);
                game.players[0].government = Some(name);
                if changed {
                    game.prune_policies_to_government(0);
                }
            }
            None => {
                if !unmapped.contains(civ6) {
                    unmapped.push(civ6.clone());
                }
            }
        }
    }
    // The seat's government HISTORY, not just its present. Returning to a
    // used government costs Anarchy and `do_government` charges it — but only
    // via `past_governments`, which a fresh rebuild never carries, so the
    // planner priced return switches as free and proposed them (deck and all)
    // every turn against the bridge guard's permanent veto. Seeding history
    // makes the planner feel the real cost; `guard_government_orders` in
    // tools/civ6_brain.py stays as the backstop.
    for civ6 in &state.used_governments {
        match civvis_node_name(&game.rules.governments, civ6, "GOVERNMENT_") {
            Some(name) => {
                game.players[0].past_governments.insert(name);
            }
            None => {
                if !unmapped.contains(civ6) {
                    unmapped.push(civ6.clone());
                }
            }
        }
    }
    for civ6 in &state.civics {
        match civvis_node_name(&game.rules.civics, civ6, "CIVIC_") {
            Some(name) => {
                game.players[0].civics.insert(crate::name::Name::new(&name));
            }
            None => {
                if !unmapped.contains(civ6) {
                    unmapped.push(civ6.clone());
                }
            }
        }
    }
    if let Some(civ6) = &state.research {
        match civvis_node_name(&game.rules.techs, civ6, "TECH_") {
            Some(name) => {
                game.players[0].research = Some(name);
                if state.research_progress.is_finite() && state.research_progress >= 0.0 {
                    game.players[0].research_progress = state.research_progress;
                }
            }
            None => {
                if !unmapped.contains(civ6) {
                    unmapped.push(civ6.clone());
                }
            }
        }
    }
    if let Some(civ6) = &state.civic {
        match civvis_node_name(&game.rules.civics, civ6, "CIVIC_") {
            Some(name) => {
                game.players[0].civic = Some(name);
                if state.civic_progress.is_finite() && state.civic_progress >= 0.0 {
                    game.players[0].civic_progress = state.civic_progress;
                }
            }
            None => {
                if !unmapped.contains(civ6) {
                    unmapped.push(civ6.clone());
                }
            }
        }
    }

    // Nothing on this path: the rebuild's board does not exist yet, and its own
    // terrain pass is the board phase below. The call is here so both passes walk
    // the same list of phases. See `HOST_STATE_STEPS`.
    run_host_steps(
        &mut HostStepCtx::new(
            &mut game,
            snapshot,
            state,
            &mut unmapped,
            &mut no_treasury,
            pass,
        ),
        HostPhase::Refresh,
    );
    let capture_seats = host_capture_seats(&game, state);
    for city in &state.cities {
        if let Some(cid) = plant_city(&mut game, 0, city) {
            city_ids.insert(cid, city.id);
            if city.id > 0 {
                known_city_ids.insert(city.id, cid);
            }
            placed_cities += 1;
            if let Some(built) = game.cities.get_mut(&cid) {
                if city.pop > 0 {
                    built.pop = city.pop;
                }
                // ★★★ WITHOUT THIS CIVVIS CANNOT SEE A CITY ABOUT TO REVOLT. Run
                // civvis-20260730T170738Z was ELIMINATED at turn 80 with its capital at
                // loyalty 5.07 and falling — and the mirror was reporting a healthy
                // city, because loyalty never crossed. Disabling governors (they
                // segfault the Game Core) removed the +8 that would have held it, so
                // the seat needs to weigh loyalty itself and could not even read it.
                if city.loyalty >= 0.0 {
                    built.loyalty = city.loyalty;
                }
                if city.food >= 0.0 {
                    built.food = city.food;
                }
                apply_city_religion(built, city);
                Arc::make_mut(&mut game.observed_city_religion_markers).insert(cid);
                apply_city_capture(built, city, &capture_seats);
                // ★★★★ SEED THE QUEUE WITH WHAT CIVILIZATION VI IS ALREADY BUILDING.
                //
                // Without it every city reads as idle, so CIVVIS chooses production
                // from scratch each turn with no knowledge of work in progress —
                // which is what a run alternating Builder / Monument / Campus every
                // second turn looks like from the inside.
                if let Some(item) = civvis_production_item(
                    &game_rules,
                    city.producing.as_deref(),
                    &city.districts,
                    Some(crate::hex::offset_to_axial(city.x, city.y)),
                ) {
                    if built.queue.is_empty() {
                        built.queue.push(item);
                    }
                }
                // The queue behind the head, so a Settler two places back reads
                // as coming rather than as absent.
                for queued in host_queue_tail(&game.rules, city) {
                    if !built.queue.contains(&queued) {
                        built.queue.push(queued);
                    }
                }
                if city.production_progress.is_finite() && city.production_progress >= 0.0 {
                    built.production = city.production_progress;
                }
                // ★★★★ WHAT THE CITY ALREADY HAS. Without this a city reads as empty
                // forever and CIVVIS re-orders the same development every turn:
                // measured 19 Builders and 17 Granaries for ONE city, against one
                // Warrior — the mirror image of the old all-army failure.
                // ⚠⚠ TRANSLATED, NOT LOWERCASED. This used to strip the prefix and
                // push whatever came out, so a building CIVVIS does not model entered
                // the city's list under a name no ruleset entry answers to — and
                // `rules.buildings[..]` is a direct index.
                //
                // `BUILDING_CASTLE` PANICKED THE WHOLE DECIDER. Reproduced on live run
                // civvis-20260801T012454Z at turn 238:
                //
                //     panicked at src/specmap.rs: no ruleset entry named "castle"
                //     Game::building_district_is_active -> Game::spawn_unit
                //       -> mirror::rebuild_from_state -> LiveMirror::new
                //
                // The brain then reported `0 orders in 0.04s` on every turn, the mod
                // sat on `await` past 98 polls, and the run fell back to the heuristic
                // ladder (`orders_source: "fallback"`). One Castle ends a run
                // permanently, because every rebuild hits it again.
                //
                // ⚠ STRICTLY THE EXPORT'S LIST, so clear the founding seed first.
                // `place_city` grants native founding bonuses — Rome's Trajan's
                // Column pushes a free monument — and Civilization VI applies that
                // bonus at FOUNDING only, so a city this seat CAPTURED never earned
                // one the export does not list. Measured on run
                // `civvis-20260807T172510Z` at turn ~160 (#1366): both cities Rome
                // captured mirrored with `extra=['monument']` against an EMPTY
                // export list, +2 ghost culture each, exactly in the captured
                // cities the recovery planner was re-valuing. Founded Roman cities
                // hid the seed because their real monument is exported and the push
                // below deduplicates. The persistent sync already clears before
                // translating; this is the same discipline.
                built.buildings.clear();
                for civ6 in &city.buildings {
                    match civvis_node_name(&game.rules.buildings, civ6, "BUILDING_") {
                        // ★★★★★ THE PALACE IS NOT A LISTED BUILDING IN CIVVIS, AND
                        // PUTTING IT IN THE LIST PAYS FOR IT TWICE.
                        //
                        // CIVVIS models the palace positionally: `city_has_palace`
                        // derives it from capital status, and FOUR separate sites add
                        // its yields, housing, amenity and great-work slots off that
                        // predicate alone. Nothing in the engine ever pushes "palace"
                        // into a `buildings` list — a native capital's list does not
                        // contain it. Civilization VI does export `BUILDING_PALACE`,
                        // so the translation above put it there, and every one of
                        // those four sites then paid a capital twice.
                        //
                        // Measured on run civvis-20260802T014139Z, turn 3: one city,
                        // pop 1, palace only. Civ 6 reported 2.5 science; the
                        // reconstruction reported 5.0 — palace 2 twice plus 0.5 for
                        // the citizen. That is the largest single term in the economy
                        // drift the `economy civ6/civvis` line has been reporting all
                        // along (median +50% science over 121 turns, +142% at t3,
                        // worst exactly in the opening where settling is decided).
                        //
                        // ⚠ Dropping it from the LIST loses nothing: `city_has_palace`
                        // is what every consumer reads, and it is true for precisely
                        // the city Civ 6 exported the palace in.
                        Some(name) if name == "palace" => {}
                        Some(name) => {
                            let named = crate::name::Name::new(&name);
                            if !built.buildings.contains(&named) {
                                built.buildings.push(named);
                            }
                        }
                        None => {
                            // Firaxis stores wonders in GameInfo.Buildings too. The
                            // exact type and plot arrive separately below; do not call
                            // a known wonder an unsupported ordinary building.
                            if civvis_node_name(&game.rules.wonders, civ6, "BUILDING_").is_none() {
                                unmapped.push(format!("{civ6}:building"));
                            }
                        }
                    }
                }
                apply_pillaged_buildings(&game.rules, built, city);
            }
            apply_observed_city_infrastructure(&mut game, cid, city, &mut unmapped);
        }
    }

    let mut dropped: Vec<String> = Vec::new();
    let plant_unit = |game: &mut crate::game::Game,
                      owner: usize,
                      u: &StateUnit,
                      unmapped: &mut Vec<String>,
                      dropped: &mut Vec<String>|
     -> Option<u32> {
        // ★★★★★ NAME EVERY UNIT THAT DOES NOT MAKE IT ONTO THE BOARD.
        //
        // ⚠⚠ A UNIT THE MIRROR DROPS IS A UNIT CIVVIS NEVER ORDERS, and it then stands
        // exactly where it was built for the rest of the game — which is the operator's
        // "units stacking up in the capital, unused", arriving by a route nobody had
        // looked at. Measured on run `civvis-20260731T070956Z` at turn 147: the export
        // carries 21 units and the reconstruction reported `units=15`. Two of the SIX
        // missing were settlers that had been motionless for fourteen turns, and every
        // report in the project read green — `unmapped` was EMPTY, because these were
        // not translation failures.
        //
        // Four distinct reasons, counted apart, because they need different repairs and
        // "6 units missing" is not a diagnosis.
        // ⚠⚠ THE FOG GATE BELONGS AT THE EXPORT, AND IT IS ALREADY THERE. This used
        // to refuse any unit whose plot the SNAPSHOT had not marked revealed, which
        // sounds like fog honesty and is not: the mod only ever exports units the seat
        // can see. Our own units by definition, and hostiles behind an explicit
        // `PlayersVisibility[pid]:IsVisible(ux, uy)` gate in `exportState`. So a unit
        // arriving here has ALREADY passed a visibility test made by the game itself.
        //
        // What this check actually measured is the TILE export being on a slower
        // cadence than the unit export (`--tile-export-every 4`), so anything standing
        // on ground the map has not caught up with vanishes from CIVVIS's board for a
        // few turns. Measured after exempting our own units only:
        // `civvis-20260731T094902Z` still dropped 23 across 14 turns, four at once.
        //
        // A unit CIVVIS cannot see gets no order — and for a hostile it is worse than
        // that, because the settler danger rule reads exactly this list. The tile is
        // still checked for EXISTENCE below; that is the honest gate.
        let _ = &snapshot;
        let mut name = resolved_civvis_unit_name(&game.rules, &u.kind)
            .unwrap_or_else(|| civvis_unit_name(&u.kind));
        if foreign_spy_is_hidden(owner, &name) {
            return None;
        }
        if !game.rules.units.contains_key(&name) {
            // ★★★★ WHAT IT REPLACES, rather than nothing at all.
            //
            // A rival's unique unit is untranslatable and was therefore DISCARDED.
            // Live run `civvis-20260801T145302Z` dropped `UNIT_NORWEGIAN_LONGSHIP`
            // on every turn it was visible — CIVVIS models no Norwegian uniques at
            // all — so an enemy WARSHIP was simply not on the board. That is not one
            // unit; it is every civilization's uniques, for every civilization met.
            //
            // A Longship replaces a Galley and CIVVIS models `galley`, so the base
            // type is a true statement about the unit where the exact name is not.
            //
            // ⚠ Recorded as `approximated`, never silently. Collapsing a distinction
            // without saying so is the failure this project's mapping rule names
            // explicitly, and a reader must be able to see that the board holds a
            // Galley where Civilization VI has a Longship.
            if let Some(base) = u.base.as_ref().filter(|b| !b.is_empty()) {
                let from_base = civvis_unit_name(base);
                if game.rules.units.contains_key(&from_base) {
                    dropped.push(format!(
                        "{}@{},{}:approximated_as_{from_base}",
                        u.kind, u.x, u.y
                    ));
                    name = from_base;
                }
            }
            // A STANDALONE unique replaces nothing, so `base` never fires for it.
            // Its promotion class is the last honest rung before the board loses
            // the unit: a Malón Raider lands as a horseman, visibly approximated,
            // instead of leaving an invisible army at the gates.
            if !game.rules.units.contains_key(&name) {
                if let Some(class) = u.class.as_ref().filter(|c| !c.is_empty()) {
                    if let Some(rep) = class_representative(class, &game.rules) {
                        let label = class
                            .strip_prefix("PROMOTION_CLASS_")
                            .unwrap_or(class)
                            .to_ascii_lowercase();
                        dropped.push(format!(
                            "{}@{},{}:approximated_as_{rep}_from_{label}",
                            u.kind, u.x, u.y
                        ));
                        name = rep.to_string();
                    }
                }
            }
        }
        if !game.rules.units.contains_key(&name) {
            // A Great Person is not a unit CIVVIS failed to name, it is a unit CIVVIS
            // does not model — see `is_great_person`. Reported apart so the
            // translation count stays a translation count.
            if is_great_person(&u.kind) {
                dropped.push(format!("{}@{},{}:great_person", u.kind, u.x, u.y));
                // ★★★★ NOT ON THE BOARD, BUT ON THE GROUND. The plot it stands on
                // is occupied in the host's civilian layer, and a board that shows
                // it empty sends builders at it forever: run
                // civvis-20260816T003229Z ordered one Builder `MOVE_TO` the
                // founded Prophet's tile on 25 consecutive turns. See
                // `Game::great_person_plots` for what honours this.
                let pos = crate::hex::offset_to_axial(u.x, u.y);
                if game.map.get(pos).is_some() {
                    game.great_person_plots.insert(pos, owner);
                }
                return None;
            }
            if !unmapped.contains(&u.kind) {
                unmapped.push(u.kind.clone());
            }
            dropped.push(format!("{}@{},{}:untranslatable", u.kind, u.x, u.y));
            return None;
        };
        let pos = crate::hex::offset_to_axial(u.x, u.y);
        // ★★★★★ A LAND UNIT ON A COAST TILE IS EMBARKED, NOT ABSENT.
        //
        // This used to refuse every unit standing on water, and the cost was total:
        // a settler that puts to sea disappears from CIVVIS's board and is therefore
        // never ordered again. Measured on run `civvis-20260731T070956Z` at turn 147 —
        // two settlers on TERRAIN_COAST at (42,14) and (49,12), motionless for
        // fourteen turns and counting, with a third still walking. The empire had
        // three settlers alive and none of them existed as far as CIVVIS knew.
        //
        // CIVVIS already models this correctly: `is_embarked` is emergent, a land unit
        // whose tile is water IS embarked, so nothing has to be flagged. Reality wins
        // — Civilization VI says the unit is on that plot, so the mirror puts it there.
        //
        // ⚠ The tile is still checked for EXISTENCE. A plot outside the map has no
        // tile at all and spawning there would be inventing ground.
        if game.map.get(pos).is_none() {
            dropped.push(format!("{}@{},{}:off_map", u.kind, u.x, u.y));
            return None;
        }
        let before = game.units.len();
        let uid = game.spawn_unit(&name, owner, pos);
        // ⚠ `spawn_unit` returns an id whether or not the unit ended up on the board.
        // Civilization VI stacks civilians with military freely and CIVVIS does not,
        // so a tile that holds three units in the real game can hold fewer here — and
        // the loser is silently absent rather than refused.
        if game.units.len() == before || !game.units.contains_key(&uid) {
            dropped.push(format!("{}@{},{}:tile_taken", u.kind, u.x, u.y));
            return None;
        }
        // Carry damage across: a unit at 30 hp is a unit CIVVIS should pull out,
        // and defaulting it to full health is how an army gets thrown away.
        let progress = observed_unit_progress(&game.rules, u, unmapped);
        if let Some(unit) = game.units.get_mut(&uid) {
            apply_unit_observation(unit, u, progress);
        }
        Some(uid)
    };

    // Every plant below records the Great People it cannot place; start from
    // nothing so a carried board never keeps a plot a Great Person has left.
    game.great_person_plots.clear();
    // What the host said about each unit, keyed by the board id it just got.
    // Written here, before `LiveMirror::new` takes the movement allowance and
    // before `seat_live_spies` reads a Spy's operation off it.
    Arc::make_mut(&mut game.host_unit_facts).clear();
    for unit in &state.units {
        if let Some(uid) = plant_unit(&mut game, 0, unit, &mut unmapped, &mut dropped) {
            unit_ids.insert(uid, unit.id);
            placed_units += 1;
            record_host_unit_facts(&mut game, uid, unit);
        }
    }

    // Firaxis exposes formation membership as a count rather than a partner id. Its
    // stacking rules make the partner unambiguous for the two-member formations
    // CIVVIS creates: both members are on the same plot and report count > 1. Carry
    // that observed state into the fresh mirror so the agent moves the escort next
    // turn instead of issuing LinkUnits forever.
    let uid_of_host = unit_ids
        .iter()
        .map(|(uid, host)| (*host, *uid))
        .collect::<std::collections::BTreeMap<_, _>>();
    for (index, first) in state.units.iter().enumerate() {
        if first.formation_count <= 1 {
            continue;
        }
        let Some(&first_uid) = uid_of_host.get(&first.id) else {
            continue;
        };
        if game.units[&first_uid].linked_to.is_some() {
            continue;
        }
        let partner = state.units[index + 1..].iter().find_map(|second| {
            if second.formation_count <= 1 || (second.x, second.y) != (first.x, first.y) {
                return None;
            }
            let &second_uid = uid_of_host.get(&second.id)?;
            game.units[&second_uid]
                .linked_to
                .is_none()
                .then_some(second_uid)
        });
        if let Some(second_uid) = partner {
            game.units.get_mut(&first_uid).unwrap().linked_to = Some(second_uid);
            game.units.get_mut(&second_uid).unwrap().linked_to = Some(first_uid);
        }
    }

    // `state.rivals` is met-gated, so use each rival's original Firaxis id to
    // reach its stable model seat. A missing player must leave its own slot
    // untouched rather than shifting every later empire left by one.
    let mut seat_of_host = host_major_seat_map(state, modeled_major_player_count(&game));
    for rival in &state.rivals {
        let Some(&owner) = seat_of_host.get(&rival.player) else {
            continue;
        };
        if owner >= game.players.len() {
            continue;
        }
        if rival.military.is_finite() && rival.military >= 0.0 {
            Arc::make_mut(&mut game.observed_military_power).insert(owner, rival.military);
        }
        if rival.score >= 0 {
            Arc::make_mut(&mut game.observed_score).insert(owner, rival.score);
        }
        if let Some(dvp) = rival.dvp {
            game.players[owner].dvp = dvp;
        }
        // Same as `LiveMirror::sync`: Civilization VI's own `CanDeclareWarOn` is the
        // permission, and CIVVIS's Formal War wait cannot mature here.
        // See `LiveMirror::sync`: without this, war is not a legal action at all.
        game.players[0].met.insert(owner);
        if owner < game.players.len() {
            game.players[owner].met.insert(0);
        }
        // See `LiveMirror::sync`: a war CIVVIS cannot see is a war it will not fight.
        let bond = if 0 < owner { (0, owner) } else { (owner, 0) };
        if rival.at_war {
            game.at_war.insert(bond);
        } else {
            game.at_war.remove(&bond);
        }
        // The host's relationship, ledger, alliance, missions, promises and
        // visibility for this rival — and, inside it, the `can_declare`
        // permission fake an older export is left with. See
        // `apply_host_diplomacy`.
        apply_host_diplomacy(&mut game, owner, rival);
        // The host's own "received Open Borders" answer, assigned rather than
        // extended: the flag is re-read from every export, so an agreement
        // that lapses on the host closes the mirrored grant on the next
        // import. Two turns of validity is enough to outlive this board —
        // the next export writes it again or removes it.
        if rival.open_borders == Some(true) {
            game.players[owner]
                .open_borders_until
                .insert(0, game.turn + 2);
        } else {
            game.players[owner].open_borders_until.remove(&0);
        }
        // Whether their border exists at all. `has_open_borders` used to read
        // the rival's Early Empire off a civic tree this board never fills,
        // and so answered "free passage" for every rival whose city was in
        // view — the fogged seal below never applied to attributed ground.
        // Assigned, not extended, like the grant: an older export without the
        // key reads as enforced, the conservative answer and the measured one.
        // The rival's tree by name lands on the seat in the same call, and a
        // tree without the bit lets Early Empire decide (`apply_rival_tree`).
        apply_rival_tree(&mut game, owner, rival, &mut unmapped);
        for city in &rival.cities {
            if let Some(cid) = plant_city(&mut game, owner, city) {
                if city.id > 0 {
                    known_city_ids.insert(city.id, cid);
                }
                apply_observed_city_infrastructure(&mut game, cid, city, &mut unmapped);
                placed_rival_cities += 1;
            }
        }
        for unit in &rival.units {
            if let Some(uid) = plant_unit(&mut game, owner, unit, &mut unmapped, &mut dropped) {
                foreign_unit_ids.insert(unit.id, uid);
                placed_rival_units += 1;
                record_host_unit_facts(&mut game, uid, unit);
                apply_foreign_unit_strikes(&mut game, uid, unit);
            }
        }
    }

    // Met city-states are public actors, not anonymous blocked territory. Keep
    // their real cities, visible units, war state, Envoys and Suzerain so settling,
    // diplomacy and military planning see the same board as the Firaxis seat.
    for player in game.players.iter_mut().filter(|player| player.is_free_city) {
        player.alive = false;
    }
    let minor_assignments = minor_actor_assignments(&game, state);
    for &(minor, owner) in &minor_assignments {
        seat_of_host.insert(minor.player, owner);
        if game.players[owner].is_free_city {
            game.players[owner].alive = true;
        }
        game.players[0].met.insert(owner);
        game.players[owner].met.insert(0);
        // Net of an established Amani's terms: see `host_amani_envoy_terms`.
        let amani = host_amani_envoy_terms(&game.rules, state, minor);
        set_mirrored_envoys(
            &mut game.players[0],
            owner,
            raw_envoys_for(minor.envoys, amani),
        );
        apply_host_rival_envoys(&mut game, minor, owner, &seat_of_host);
        apply_host_quest(&mut game, minor, owner, &mut unmapped);
        if minor.score >= 0 {
            Arc::make_mut(&mut game.observed_score).insert(owner, minor.score);
        }
        if minor.military.is_finite() && minor.military >= 0.0 {
            Arc::make_mut(&mut game.observed_military_power).insert(owner, minor.military);
        }
        let bond = (0, owner);
        if minor.at_war {
            game.at_war.insert(bond);
        } else {
            game.at_war.remove(&bond);
        }
        // A city-state's land is shut to everyone but its Suzerain once it
        // holds Early Empire. `has_open_borders` asks suzerainty only AFTER
        // it has asked whether the border exists, and on this board the
        // civic that creates it was never modelled — so a met city-state
        // whose city we could see read as open ground, and the planner sent
        // scouts, warriors and trebuchets through it turn after turn.
        game.players[owner].borders_enforced = Some(minor.enforces_borders.unwrap_or(true));
        for city in &minor.cities {
            if let Some(cid) = plant_city(&mut game, owner, city) {
                if city.id > 0 {
                    known_city_ids.insert(city.id, cid);
                }
                apply_observed_city_infrastructure(&mut game, cid, city, &mut unmapped);
                placed_minor_cities += 1;
            }
        }
        for unit in &minor.units {
            if let Some(uid) = plant_unit(&mut game, owner, unit, &mut unmapped, &mut dropped) {
                foreign_unit_ids.insert(unit.id, uid);
                placed_rival_units += 1;
                record_host_unit_facts(&mut game, uid, unit);
                apply_foreign_unit_strikes(&mut game, uid, unit);
            }
        }
    }
    // The suzerain is public even when it is another major — and so is its
    // absence. Seed the delegations after every host id has a compact seat
    // mapping.
    for &(minor, owner) in &minor_assignments {
        let amani = host_amani_envoy_terms(&game.rules, state, minor);
        seed_mirrored_suzerainty(&mut game, minor, owner, &seat_of_host, amani);
    }

    // Barbarians go on CIVVIS's own barbarian seat rather than a rival's, so the
    // threat is visible to the planner without inventing a civilization at war with
    // us.
    //
    // ★★★★★ `barb_pid`, NOT THE FIRST `is_barbarian` SEAT. Those are two different
    // players. `ensure_free_city_player` builds the dormant Free Cities seat with
    // `is_barbarian = true` and pushes it BEFORE the real Barbarians seat, so a scan
    // for the first barbarian finds Free Cities — which the engine holds at
    // `alive = false` until a loyalty revolt wakes it.
    //
    // Measured on the live run `civvis-20260731T172058Z` at turn 43, reconstructing
    // the same export `civvis-orders` decides from:
    //
    //   4 Free Cities  barbarian=true free_city=true  alive=FALSE
    //   5 Barbarians   barbarian=true free_city=false alive=true   <- barb_pid
    //   units on the board by owner: {0: 5, 4: 9}
    //
    // All nine barbarians — a warrior adjacent to the capital, an archer two tiles
    // off, and a barbarian settler — were owned by a dead player. Every count read
    // green: they are placed, so they never reach `dropped_units`, and the seat they
    // land on is barbarian by flag, so a spot check of the flag agrees too.
    let barbarian_seat = game.barb_pid.or_else(|| {
        game.players
            .iter()
            .position(|player| player.is_barbarian && !player.is_free_city)
    });
    // ★★★★ A FREE CITIES UNIT GOES ON THE FREE CITIES SEAT. `hostiles[]` carries
    // two players' units — `GetAliveBarbarianIDs()` and every `IsFreeCities()`
    // player — and every entry used to be handed to `barb` whatever its `player`
    // said, so the army that took four cities on run civvis-20260802T064240Z was
    // mirrored as barbarians (docs/FIDELITY.md, "The one-to-one map", item 3). A
    // unit the Free Cities actor's own `units[]` already planted is not planted
    // twice; a seat that holds one is alive, whether or not the aggregate actor
    // was exported under `minors[]`.
    let free_cities_seat = game
        .players
        .iter()
        .find(|player| player.is_free_city)
        .map(|player| player.id);
    for unit in &state.hostiles {
        let free = hostile_is_free_cities(state, unit);
        if free && hostile_exported_as_minor_unit(state, unit) {
            continue;
        }
        let seat = if free {
            free_cities_seat.or(barbarian_seat)
        } else {
            barbarian_seat
        };
        // ⚠ NEVER SKIP SILENTLY. A roster with no barbarian seat is a reconstruction
        // that cannot hold the threat list, and the planner has to be told rather
        // than left to read an empty board as a safe one.
        let Some(owner) = seat else {
            dropped.push(format!(
                "{}@{},{}:no_barbarian_seat",
                unit.kind, unit.x, unit.y
            ));
            continue;
        };
        if let Some(uid) = plant_unit(&mut game, owner, unit, &mut unmapped, &mut dropped) {
            foreign_unit_ids.insert(unit.id, uid);
            placed_rival_units += 1;
            record_host_unit_facts(&mut game, uid, unit);
            apply_foreign_unit_strikes(&mut game, uid, unit);
            if game.players[owner].is_free_city {
                game.players[owner].alive = true;
            }
        }
    }

    // Every whole-board pass, now that every city and unit — ours, the rivals'
    // and the minors' — is planted. See `HOST_STATE_STEPS`.
    run_host_steps(
        &mut HostStepCtx::new(
            &mut game,
            snapshot,
            state,
            &mut unmapped,
            &mut no_treasury,
            pass,
        )
        .with_board(&known_city_ids, &minor_assignments, &seat_of_host),
        HostPhase::Board,
    );

    // Districts the host has refused to place, mapped onto CIVVIS's cities. Done here
    // because it needs `city_ids`, which is only complete once every city is planted.
    game.blocked_districts = Arc::new(blocked_districts_from(
        &state.refused_districts,
        &city_ids,
        &game.rules,
    ));
    game.host_district_capacity = Arc::new(host_district_capacity_from(&state.cities, &city_ids));
    game.host_district_sites = Arc::new(host_district_sites_from(
        &state.host_district_sites,
        &city_ids,
        &game.rules,
    ));
    game.host_wonder_sites = Arc::new(host_wonder_sites_from(
        &state.host_wonder_sites,
        &city_ids,
        &game.rules,
    ));
    game.blocked_wonders = Arc::new(blocked_wonders_from(
        &state.refused_wonders,
        &city_ids,
        &game.rules,
    ));
    game.host_unavailable_wonders = Arc::new(host_unavailable_wonders_from(
        &state.host_unavailable_wonders,
        &game.rules,
    ));
    let blocked_production =
        blocked_production_from(&state.refused_production, &city_ids, &game.rules);
    game.replace_blocked_production(blocked_production);
    // ★ The host's menus — the positive gate beside the refusal cooldown
    // above, and the price list the purchase lanes read. See
    // `Game::host_buildable`; wired on BOTH paths for the reason the
    // promotion blocks are.
    let menus = host_menus_from(&state.cities, &city_ids, &game.rules);
    game.replace_host_unit_resource_prices(menus.unit_resource_prices);
    game.replace_host_saved_work(menus.saved_work);
    game.host_plot_purchases = std::sync::Arc::new(menus.plot_purchases);
    game.replace_host_menus(menus.buildable, menus.purchasable, menus.district_plots);
    seat_live_spies(&mut game, state);
    block_live_spy_production(&mut game, state.spy_capacity);
    let blocked_purchases =
        blocked_production_from(&state.refused_purchases, &city_ids, &game.rules);
    if std::env::var("CIVVIS_DEBUG_PURCHASE_BLOCK").is_ok() {
        eprintln!(
            "[purchase-block] rebuild: refused_purchases={:?} city_ids={:?} -> blocked={:?}",
            state.refused_purchases, city_ids, blocked_purchases
        );
    }
    game.replace_blocked_purchases(blocked_purchases);
    // ⚠ Wired on BOTH the rebuild (here) and the refresh path. `--fresh-board`
    // reconstructs the board every turn and never runs the refresh, so wiring only
    // there left the block set permanently empty and the gate measured no change.
    game.blocked_promotions = Arc::new(blocked_promotions_from(
        &state.refused_promotions,
        &unit_ids,
        &game.rules,
    ));
    game.host_band_promotions = Arc::new(host_band_promotions_from(state, &unit_ids, &game.rules));
    game.blocked_strikes = Arc::new(blocked_strikes_from(&state.refused_strikes, &unit_ids));
    game.host_previews = Arc::new(host_previews_from(&state.host_previews, &unit_ids));

    // The readings that must survive whatever the board passes scored on their
    // way there. See `HOST_STATE_STEPS`. Finish still needs the completed
    // city-ID map: an own native city's positive ID must resolve on a fresh
    // board just as it does in the persistent context used by sync.
    run_host_steps(
        &mut HostStepCtx::new(
            &mut game,
            snapshot,
            state,
            &mut unmapped,
            &mut no_treasury,
            pass,
        )
        .with_board(&known_city_ids, &minor_assignments, &seat_of_host),
        HostPhase::Finish,
    );
    Reconstruction {
        game,
        unit_ids,
        foreign_unit_ids,
        city_ids,
        known_city_ids,
        placed_cities,
        placed_units,
        placed_rival_cities,
        placed_rival_units,
        placed_minor_cities,
        unmapped,
        dropped_units: dropped,
    }
}

/// Seat the host's projection for every route a Trader could start
/// (`route_options`) on `Game::observed_route_options`, keyed by the
/// (origin, destination) pair, where the trader-destination chooser
/// (`AdvancedAi::trade_route_destination_value_from`) prices the pair from
/// it instead of from the model. Cleared on every apply: the host sends the
/// list only while a route slot is open, and a projection for a slot that
/// closed is not one. Endpoints resolve by coordinates first, the way active
/// routes do, because Firaxis city ids are per player; a destination the
/// board does not hold is skipped and filed once.
fn restore_route_options(
    game: &mut crate::game::Game,
    options: Option<&[StateRouteOption]>,
    city_of_civ6: &std::collections::BTreeMap<i64, u32>,
) -> Vec<String> {
    Arc::make_mut(&mut game.observed_route_options).clear();
    let Some(options) = options else {
        return Vec::new();
    };
    let mut unresolved: Vec<String> = Vec::new();
    let mut file = |issue: &str| {
        if !unresolved.iter().any(|filed| filed == issue) {
            unresolved.push(issue.to_string());
        }
    };
    for option in options {
        let origin = if option.origin_x >= 0 && option.origin_y >= 0 {
            game.city_at(crate::hex::offset_to_axial(
                option.origin_x,
                option.origin_y,
            ))
        } else {
            None
        }
        .or_else(|| city_of_civ6.get(&option.origin).copied());
        let Some(origin) = origin else {
            file("route_option:origin");
            continue;
        };
        if game.cities.get(&origin).map(|city| city.owner) != Some(0) {
            file("route_option:origin_not_ours");
            continue;
        }
        let destination = if option.dest_x >= 0 && option.dest_y >= 0 {
            game.city_at(crate::hex::offset_to_axial(option.dest_x, option.dest_y))
        } else {
            None
        };
        let Some(destination) = destination else {
            file("route_option:destination");
            continue;
        };
        if destination == origin {
            continue;
        }
        let Some(yields) = option.yields else {
            continue;
        };
        let finite = [
            yields.food,
            yields.production,
            yields.gold,
            yields.science,
            yields.culture,
            yields.faith,
        ]
        .iter()
        .all(|value| value.is_finite() && *value >= 0.0);
        if finite {
            Arc::make_mut(&mut game.observed_route_options).insert((origin, destination), yields);
        }
    }
    unresolved
}

/// Record the ground Civilization VI has just proved this seat can see.
///
/// Every foreign unit standing on this board arrived from the export, and the
/// export carries a rival's or a minor's units **only under current visibility**
/// — `StateMinor`'s own doc says so, and `state.hostiles` is the same channel for
/// barbarians. So each of their tiles is a tile Civilization VI is showing us
/// right now, whatever this engine's sight model would derive on a reconstructed
/// map. See [`crate::game::Game::host_observed`] for what the disagreement cost.
///
/// Taken from the board rather than from the three planting loops so it cannot
/// fall out of step with them: rivals, city-states and barbarians all land here,
/// and a fourth channel added later is covered without being remembered.
///
/// And, since 2026-08-26, every plot the sweep marked `vis`
/// (`PlayersVisibility[pid]:IsVisible`), which is the same fact for EMPTY
/// ground: the export carried only "revealed once", so fog and sight were one
/// state to the board. The signature re-sends a plot when its sight flips, so
/// the accumulated record is current as of the last delta.
fn record_host_observed(game: &mut crate::game::Game, snapshot: &Snapshot) {
    let mut observed: BTreeSet<crate::Pos> = game
        .units
        .values()
        .filter(|unit| unit.owner != crate::game::MIRRORED_SEAT)
        .map(|unit| unit.pos)
        .collect();
    for (x, y) in snapshot.revealed_positions() {
        if !snapshot.plot((x, y)).is_some_and(|plot| plot.vis) {
            continue;
        }
        let pos = crate::hex::offset_to_axial(x, y);
        if game.map.get(pos).is_some() {
            observed.insert(pos);
        }
    }
    game.host_observed = Arc::new(observed);
}

/// Give every revealed plot the owner Civilization VI says it has.
///
/// ★★★★ FOUND BY `tools/civ6_watchdogs.py`, WHICH IS THE POINT OF IT. Diffing the
/// mirror against Civ 6's own export tile for tile: terrain, hills, water, features
/// and resources agreed on every plot, no exported plot was missing — and **20 of 509
/// plots that Civilization VI says WE OWN were unowned in CIVVIS's board** (44 of 375
/// on another run, 27 of 266 on a third). Nothing else in any report could have shown
/// this: the seat looked correct, the map looked correct, and the borders were wrong.
///
/// The cause is structural rather than a bug. `place_city` claims a city centre and
/// its first ring — seven tiles — and that is all a mirrored city ever gets, because
/// the board is rebuilt from scratch every turn so no border ever grows. A real
/// Civ 6 city at population six owns three rings. So CIVVIS was planning an empire
/// roughly a third the size of the one on screen.
///
/// ⚠ Both directions matter, and they fail differently. Ground of OURS that reads
/// unowned understates our yields and our workable tiles. Ground of a RIVAL'S that
/// reads unowned invites CIVVIS to settle inside their borders, which Civilization VI
/// then refuses — one of the live explanations for the `found` refusal loop.
///
/// ⚠ Our own Civ 6 player id is read off our own city centres rather than assumed.
/// The alternative — "any owner that is not a known rival is us" — would hand a plot
/// belonging to a civilization we have not met to our own empire, and a seat that
/// believes it owns a rival's capital ring is worse off than one that knows nothing.
/// The furthest a Civilization VI city ever owns a plot: culture bombs and
/// border growth stop at five tiles from the centre (`GlobalParameters`
/// `CITY_MAX_BUY_PLOT_RANGE`, and the plot-acquisition ring). A plot a met
/// major owns further than this from every city of theirs we can see is owned
/// by a city we cannot. A plot exactly this far away is not proved safe either:
/// the host exports only its player owner, so it can equally belong to a nearer
/// city still in fog. Mark that outer ring as unresolved for the live Loyalty
/// guard. See [`crate::game::Game::unseen_major_borders`].
const CIV6_CITY_OWNERSHIP_REACH: i32 = 5;

/// Refuse a founding plot whose modeled population pressure will erase the city
/// before it can repay its Settler.
///
/// This is deliberately limited to a live Settler's current plot. Forecasting every
/// revealed tile would clone the complete game hundreds of times per Firaxis turn;
/// checking the one or two plots on which founding is immediately possible is cheap
/// and still stops the irreversible action. Once blocked, AdvancedAI drops that site
/// through the same `blocked_city_sites` channel used for host refusals and chooses a
/// different destination.
///
/// The threshold describes an emergency rather than a merely imperfect frontier:
/// -8 per turn exhausts a full Loyalty bar in at most thirteen turns. Live run
/// `live-head-rome-20260802T164220Z` planted Cumae beside Stirling at -22 per turn on
/// turn 45; it revolted on turn 53, erasing the third city and the Settler investment.
/// Use the engine's complete Loyalty calculation on a speculative city instead of
/// maintaining a second approximation of population, age, capital, policy, religion,
/// amenity, starvation, and governor effects here.
fn block_loyalty_doomed_settler_sites(game: &mut crate::game::Game) {
    const DOOMED_LOYALTY_PER_TURN: f64 = -8.0;

    let sites: Vec<crate::Pos> = game
        .player_unit_ids(0)
        .into_iter()
        .filter(|unit| game.units[unit].kind == "settler" && game.can_found_city(*unit))
        .map(|unit| game.units[&unit].pos)
        .collect();
    for site in sites {
        let mut forecast = game.clone();
        let city = forecast.found_city_for(0, site, None);
        let loyalty = forecast.settlement_loyalty_per_turn(&forecast.cities[&city]);
        if loyalty <= DOOMED_LOYALTY_PER_TURN {
            Arc::make_mut(&mut game.blocked_city_sites).insert(site);
        }
    }
}

// ===================================================== the persistent live mirror
//
// ★★★★★ WHY REBUILDING EVERY TURN IS NOT ENOUGH.
//
// `rebuild_from_state` makes a fresh `Game` from each board, and a fresh `AdvancedAi`
// is asked to decide on it. That agent's whole medium-term apparatus — its strategic
// plan, its force groups, the site each settler is walking to — is therefore thrown
// away and re-derived every single turn. What survives is only what is locally optimal
// on this turn's board, and holding still is almost always locally optimal.
//
// Two measurements of the same defect:
//
//   * A settler issued `MOVE_TO 12 19` then `MOVE_TO 13 19` on alternating turns for
//     twenty turns, oscillating between two tiles it kept re-choosing.
//   * On run civvis-20260730T120107Z at turn 108, with 28 units alive, the FURTHEST
//     unit from the capital was 7 tiles and the mean was 3.2 — plateaued since turn
//     74. Nothing ever went looking for the enemy, so `met` stayed at 2, ZERO rival
//     cities were ever seen, and an army of 23 had nothing it could attack.
//     Domination was unreachable, and no heuristic tweak would have changed it.
//
// So the game and the agent must persist, and reality must be synced INTO them. The
// unit id mapping persisting is what makes CIVVIS's per-unit memory stay valid: a
// `settler_target` keyed to a unit id is worthless if that id is reassigned each turn.
pub struct LiveMirror {
    pub game: crate::game::Game,
    /// Civilization VI unit id -> the CIVVIS unit standing in for it. Stable for the
    /// life of the unit, which is the point.
    pub uid_of: std::collections::BTreeMap<i64, u32>,
    /// Civilization VI id -> board id for the foreign units on the board,
    /// rebuilt with them on every sync. `live_divergence::combat_pairs` needs
    /// the foreign side of a `combat` event to resolve; before this every
    /// `combat_damage` row read "no pairs" because only `uid_of` — our units —
    /// was ever consulted (184 combats in run civvis-20260826T184456Z, 110 of
    /// them unit-vs-unit, none paired).
    pub foreign_uid_of: std::collections::BTreeMap<i64, u32>,
    pub civ6_of: std::collections::BTreeMap<u32, i64>,
    /// Civilization VI city id -> CIVVIS city id.
    pub cid_of: std::collections::BTreeMap<i64, u32>,
    /// Civilization VI city id -> CIVVIS city id for every city currently
    /// retained by the mirror, including remembered rival cities used as trade
    /// destinations.
    pub known_city_ids: std::collections::BTreeMap<i64, u32>,
    /// Firaxis route records name the trader that is busy.  Keep this even when a
    /// destination cannot yet be reconstructed, so the same trader is never sent
    /// a second `TRADE_ROUTE` order.
    pub active_trade_route_traders: std::collections::BTreeSet<i64>,
    /// Rival stand-ins, rebuilt each sync: they need no continuity of their own and
    /// what we can see of them changes with the fog.
    rival_units: Vec<u32>,
    /// Barbarians planted on the board this sync, so the next one can clear them.
    /// See the barbarian block in [`LiveMirror::sync`] for why they are rebuilt
    /// wholesale rather than tracked.
    hostile_units: Vec<u32>,
    rival_cities: std::collections::BTreeSet<(i32, i32)>,
    pub unmapped: Vec<String>,
    /// See [`Reconstruction::dropped_units`]. Carried onto the live mirror so the
    /// decider can report it every turn: a unit that is missing from the board is a
    /// unit that will stand still, and nothing else in the telemetry can say so.
    pub dropped_units: Vec<String>,
    pub turns_synced: u32,
    /// Previous treasury observation, used only when the host omits income.
    last_treasury: Option<(u32, f64)>,
    /// A finite native income reading outranks balance differences, which also
    /// include purchases, pillaging, deals, and other one-time transactions.
    observed_gold_income: Option<f64>,
}

/// A unit's full movement allowance for a fresh mirrored turn.
fn mirror_unit_moves(game: &crate::game::Game, uid: u32) -> f64 {
    let kind = match game.units.get(&uid) {
        Some(unit) => unit.kind,
        None => return 2.0,
    };
    // ★★★★ A TRADER CANNOT BE WALKED IN CIVILIZATION VI, AND CIVVIS KEPT TRYING.
    //
    // CIVVIS's ruleset gives `trader` **2 moves**; Civilization VI gives it
    // `AiType="UNITTYPE_TRADE"` and reports `moves: 0` on every export — a trade unit
    // travels its route, it does not walk. Granting it full ruleset movement here
    // made CIVVIS plan steps the host refuses every single time.
    //
    // Measured on run civvis-20260801T065721Z with the `move_refused` instrument:
    // ONE trader, unit 786439, produced **22 of 33** move refusals by turn 70 —
    // ordered to (9,27) seven times, (6,25) seven times, and three more
    // destinations besides, shuffling between four tiles for 38 turns with
    // `moves: 0` in every sighting.
    //
    // ⚠ This does NOT touch the ruleset. Changing `trader.moves` in data/units.json
    // would move `Rules::source_fingerprint` and the Elo ledger would reject new
    // games at bind time — the same wall PR #703 is held behind. This is per-game
    // reconstruction state, so an ordinary CIVVIS game is unaffected and its traders
    // keep the 2 moves the ruleset gives them.
    //
    // ⚠ And it does not silence the trader: `TradeRoute` is a separate action and
    // stays available. What stops is the walking it was never able to do.
    if kind == "trader" {
        return 0.0;
    }
    // ★★★★★ AND A SPY CANNOT BE WALKED EITHER — IT IS THE BIGGEST REFUSAL CLASS LEFT.
    //
    // Measured over every run recorded on 2026-08-03, after the self-tile repair
    // (#836) had already removed its own class: **893 of 1,197 refused adjacent
    // moves — 75% — were `UNIT_SPY`**, and every one of them was on OUR OWN
    // territory (`owner: 0`) onto ordinary passable ground. Individual spies were
    // ordered and refused for the length of a game: unit 5439532 stuck at (16,22)
    // for 81 turns, 5046311 at (16,26) for 73, 6291477 at (27,17) for 65.
    //
    // ⚠ THE SHAPE DIFFERS FROM THE TRADER'S AND THE CONCLUSION IS THE SAME.
    // Civilization VI reports a trader with `moves: 0`, which is a plain signal.
    // It reports a SPY with `moves` of 1, 2 or 3 — so the export says it can move
    // and the host still refuses every `MOVE_TO`, because a spy travels by being
    // given a destination city through the espionage system, not by walking a
    // tile at a time. The movement points are real; tile movement is not the
    // operation that spends them.
    //
    // ⚠ HISTORY: this comment once claimed the spy still acts, then (correctly,
    // at the time) that it could not — `civvis_orders`' `translate` had no arm
    // for `AssignSpy`, `SpyMission` or `PromoteSpy`, and `Game::spies` stayed
    // empty for the whole of a live game. #1929 added the translate arms, the
    // mod's `UNITOPERATION_SPY_*` verbs, and `seat_live_spies`; #2012 named
    // the promotions the host's way; and the promotion-entitlement shift in
    // `seat_live_spies` is what finally let a live Spy be offered travel and
    // missions at all (`legal_spy_actions` returns promotions as the ONLY
    // legal actions while one is owed, and an unshifted fresh Spy owed one
    // forever). What stops HERE is only the tile-walking a Civilization VI
    // Spy never does — it travels by `SPY_TRAVEL_NEW_CITY`, not by steps.
    //
    // ⚠ Mirror only. `data/units.json` still gives `spy` its 1 move, so
    // `Rules::source_fingerprint` does not shift and an ordinary CIVVIS game is
    // unaffected.
    if kind == "spy" {
        return 0.0;
    }
    // A static unit definition is not enough here. Embarked units, roads,
    // technology, policy, wonder, formation, and support effects all change
    // the allowance, and the first disembark step can cost four movement points
    // while the static Settler definition still says two. Use the reconstructed
    // board's same allowance calculation that `can_move` and route planning use;
    // otherwise a live Settler can be given a valid inland target but never have
    // enough mirrored movement to leave the coast.
    game.unit_max_moves(uid)
}

/// The movement a mirrored unit starts its turn with.
///
/// The full allowance ([`mirror_unit_moves`]) unless the run's `seat` event
/// says the mod reads `moves` at the start of the seat's turn and keeps the
/// host from spending it beforehand (`moves_at_turn_start`: every MOVE_TO
/// capped to the turn's reach, combat units' queued paths cancelled). Then the
/// export's `moves` is the truth and the board takes `min(allowance, moves)`
/// — a unit the host already walked this turn is planned as it stands, not
/// as if it had a fresh turn. Measured on the recorded runs before this:
/// 12.5 % of MOVE_TOs did not move at all, and the plan built on the moves
/// they did not have.
fn mirror_unit_moves_for(
    game: &crate::game::Game,
    uid: u32,
    observed: Option<&StateUnit>,
    trust_moves: bool,
) -> f64 {
    let allowance = mirror_unit_moves(game, uid);
    if !trust_moves {
        return allowance;
    }
    match observed {
        Some(unit) if unit.moves >= 0.0 => allowance.min(unit.moves),
        _ => allowance,
    }
}

impl LiveMirror {
    /// Our units that start this frame with less than their full allowance —
    /// the host walked them (a queued path, an automation) before the brain
    /// could act. Only meaningful when the seat trusts `moves`
    /// (`Seat::moves_at_turn_start`); zero is the healthy reading once every
    /// MOVE_TO is capped to the turn's reach.
    pub fn units_short_of_movement(&self) -> usize {
        self.game
            .player_unit_ids(0)
            .into_iter()
            .filter(|uid| {
                let unit = &self.game.units[uid];
                unit.moves_left + 1e-9 < self.game.unit_max_moves(*uid)
            })
            .count()
    }

    pub fn new(
        snapshot: &Snapshot,
        state: &StateSnapshot,
        players: usize,
        seed: u64,
        max_turns: u32,
        frontier_depth: u32,
    ) -> LiveMirror {
        let rebuilt = rebuild_from_state(snapshot, state, players, seed, max_turns, frontier_depth);
        let mut uid_of = std::collections::BTreeMap::new();
        for (uid, civ6) in &rebuilt.unit_ids {
            uid_of.insert(*civ6, *uid);
        }
        let mut game = rebuilt.game;
        // `civvis_orders --serve --fresh-board` constructs this mirror for every
        // decision, so movement normalization cannot live only in `sync`. In
        // particular, Civ VI traders travel only through TradeRoute, while the
        // standalone CIVVIS ruleset grants them two walking moves.
        let observed: std::collections::BTreeMap<i64, &StateUnit> =
            state.units.iter().map(|unit| (unit.id, unit)).collect();
        for (uid, civ6) in rebuilt
            .unit_ids
            .iter()
            .map(|(uid, civ6)| (*uid, *civ6))
            .collect::<Vec<_>>()
        {
            let allowance = mirror_unit_moves_for(
                &game,
                uid,
                observed.get(&civ6).copied(),
                state.seat.moves_at_turn_start,
            );
            let attacks = observed
                .get(&civ6)
                .and_then(|unit| unit.attacks_remaining)
                .filter(|_| state.seat.moves_at_turn_start);
            if let Some(live) = game.units.get_mut(&uid) {
                live.moves_left = allowance;
                if let Some(attacks) = attacks {
                    live.attacks_left = attacks.max(0);
                }
            }
        }
        let mut cid_of = std::collections::BTreeMap::new();
        for (cid, civ6) in &rebuilt.city_ids {
            cid_of.insert(*civ6, *cid);
        }
        // ⚠ Every foreign unit the rebuild planted, so the FIRST sync can clear it
        // the way every later sync clears its own. These lists started empty, so a
        // rival, city-state or hostile standing on the board at construction was
        // never removed: the next export's copy of it was dropped as `hostile_tile`
        // (same plot) or planted beside it (moved), and the construction reading
        // of its hp stood for the rest of the game.
        let foreign_units: Vec<u32> = game
            .units
            .values()
            .filter(|unit| unit.owner != 0)
            .map(|unit| unit.id)
            .collect();
        LiveMirror {
            game,
            civ6_of: rebuilt.unit_ids,
            uid_of,
            foreign_uid_of: rebuilt.foreign_unit_ids,
            cid_of,
            known_city_ids: rebuilt.known_city_ids,
            active_trade_route_traders: active_trade_route_traders(state),
            rival_units: foreign_units,
            hostile_units: Vec::new(),
            rival_cities: std::collections::BTreeSet::new(),
            unmapped: rebuilt.unmapped,
            dropped_units: rebuilt.dropped_units,
            turns_synced: 1,
            observed_gold_income: state.gold_per_turn.filter(|net| net.is_finite()),
            // Seed the baseline here so the very first sync can be differenced.
            // A resync rebuilds through this constructor too, which is exactly
            // when the previous baseline has become meaningless.
            last_treasury: match state.gold >= 0 {
                true => Some((state.turn.max(1), state.gold as f64)),
                false => None,
            },
        }
    }

    /// Carry the fallback treasury history across a fresh-board rebuild.
    /// Keep the host's reported net income when available, including a real
    /// zero. A balance delta includes spending and cannot replace that rate.
    pub fn carry_treasury_baseline(&mut self, previous: Option<(u32, f64)>) {
        let Some(previous) = previous else { return };
        self.last_treasury = Some(previous);
        let gold = self.game.players[0].gold;
        let derived = self.mirror_net_income(self.game.turn, gold);
        if let Some(net) = self.observed_gold_income.or(derived) {
            self.game.players[0].gold_per_turn = net;
        }
    }

    /// The baseline to hand to the next board built over this one.
    pub fn treasury_baseline(&self) -> Option<(u32, f64)> {
        self.last_treasury
    }

    /// Net gold per turn, derived from the treasury balance because the export
    /// carries no rate.
    ///
    /// ⚠⚠ `gold_per_turn` gates CIVVIS's whole bankruptcy response — the
    /// `economic_recovery` branch of `BasicAi::product_for` requires
    /// `gold_per_turn < -0.5` — and **nothing in the bridge has ever written
    /// it**. Outside a mirrored game it is computed by CIVVIS's own economy;
    /// inside one it holds whatever that simulation last produced over a board
    /// whose maintenance rules are not Civilization VI's. So the recovery has
    /// been unreachable in every real game.
    ///
    /// Measured on run `civvis-20260802T044726Z`: the treasury reached zero on
    /// t85 and stayed there for 56 turns while the seat produced seven heavy
    /// chariots, three battering rams, two archers and a trebuchet. Cities went
    /// 3 → 2 → 1 and the score finished 143 against a best rival's 613.
    ///
    /// ⚠ **A treasury pinned at zero is insolvency, not thrift, and the first
    /// difference reads ZERO exactly then** — Civilization VI clamps the balance
    /// at zero and disbands units to pay the bill, so the delta cannot go
    /// negative once the empire is already broke. An empty treasury that is not
    /// refilling is therefore reported as negative outright.
    ///
    /// Only consecutive turns are differenced. A resync, a replayed turn or a
    /// gap leaves the previous value alone rather than inventing a rate from a
    /// span of unknown length.
    fn mirror_net_income(&mut self, turn: u32, gold: f64) -> Option<f64> {
        mirror_net_income_from(&mut self.last_treasury, turn, gold)
    }

    /// Remove only active-route visual stand-ins from a speculative planning game.
    ///
    /// Firaxis keeps these Traders on the map, while CIVVIS consumes them into a
    /// `TradeRoute`.  The authoritative mirror therefore keeps both facts for a
    /// faithful display, but a planning clone must not treat the same physical
    /// trader as idle just because another capacity slot exists.
    pub fn prune_active_trade_route_traders(&self, planned_game: &mut crate::game::Game) {
        let active: Vec<u32> = self
            .active_trade_route_traders
            .iter()
            .filter_map(|civ6| self.uid_of.get(civ6).copied())
            .collect();
        for uid in active {
            planned_game.remove_unit(uid);
        }
    }

    /// Bring the persistent game up to date with what Civilization VI now reports.
    ///
    /// ⚠ Units are matched by their CIV 6 id, never by position or index. Position
    /// changes every turn and index changes whenever anything dies, so either would
    /// silently re-point CIVVIS's memory at a different unit — the failure mode being
    /// a plan that looks continuous and is not.
    pub fn sync(&mut self, snapshot: &Snapshot, state: &StateSnapshot, frontier_depth: u32) {
        self.observed_gold_income = state.gold_per_turn.filter(|net| net.is_finite());
        // ⚠ Bisect switches. Persistent sync silences CIVVIS completely — 0 actions on
        // every turn after the first, with a FRESH agent, a correct roster and full
        // movement — so the cause is somewhere in the mutations below. Each part can be
        // switched off to find which one, because five rounds of hypothesis were wrong.
        let skip_terrain = std::env::var("CIVVIS_SYNC_NO_TERRAIN").is_ok();
        let skip_rivals = std::env::var("CIVVIS_SYNC_NO_RIVALS").is_ok();
        let skip_units = std::env::var("CIVVIS_SYNC_NO_UNITS").is_ok();
        // The constants of this pass; the steps of `HOST_STATE_STEPS` read them.
        let pass = HostPass {
            mode: MirrorMode::Sync,
            // The horizon is the reconstruction's answer — a sync never rewrites
            // it, so the step that would is not on this path.
            max_turns: 0,
            frontier_depth,
            skip_terrain,
        };
        self.turns_synced += 1;
        for gap in &state.schema_gaps {
            if !self.unmapped.contains(gap) {
                self.unmapped.push(gap.clone());
            }
        }

        // `take_turn` simulates production so its own economy can evaluate the
        // resulting army. In a live mirror that unit is only QUEUED in Civilization
        // VI, not present in the next state export, and therefore has no Civ VI id.
        // Keeping it lets the persistent AI issue orders to an archer that does not
        // exist and count it as a real defender. Delete every such locally-created
        // seat-0 unit before reality is applied; when Firaxis actually finishes it,
        // the export below gives it a new mapped unit instead.
        let simulated: Vec<u32> = self
            .game
            .units
            .values()
            .filter(|unit| unit.owner == 0 && !self.civ6_of.contains_key(&unit.id))
            .map(|unit| unit.id)
            .collect();
        for uid in simulated {
            self.game.remove_unit(uid);
        }

        // ⚠ UNION, NEVER REPLACE. Refusals accumulate over a game and the set is
        // rebuilt from the whole event log each time, but a sync that assigned
        // instead of merging would silently drop anything the caller had added
        // directly — and a forgotten refusal is a settler back in the same loop.
        Arc::make_mut(&mut self.game.blocked_city_sites)
            .extend(state.refused_sites.iter().copied());
        Arc::make_mut(&mut self.game.blocked_improvement_sites)
            .extend(state.refused_improves.iter().copied());
        let blocked = Arc::make_mut(&mut self.game.blocked_improvements);
        for (pos, names) in &state.refused_improvement_names {
            blocked
                .entry(*pos)
                .or_default()
                .extend(names.iter().copied());
        }
        Arc::make_mut(&mut self.game.blocked_trade_routes)
            .extend(state.refused_trade_routes.iter().copied());
        // Union for the same reason as the two above: a card the host retired stays
        // retired, and the set is rebuilt from the whole event log each time.
        let retired = blocked_policies_from(&state.refused_policy_names, &self.game.rules);
        Arc::make_mut(&mut self.game.blocked_policies).extend(retired);
        // And a pantheon a rival holds stays held.
        let taken = blocked_pantheons_from(&state.refused_pantheons, &self.game.rules);
        Arc::make_mut(&mut self.game.blocked_pantheons).extend(taken);
        let refused = blocked_districts_from(
            &state.refused_districts,
            &self
                .cid_of
                .iter()
                .map(|(civ6, cid)| (*cid, *civ6))
                .collect(),
            &self.game.rules,
        );
        for (cid, names) in refused {
            Arc::make_mut(&mut self.game.blocked_districts)
                .entry(cid)
                .or_default()
                .extend(names);
        }
        self.game.host_district_capacity = Arc::new(host_district_capacity_from(
            &state.cities,
            &self
                .cid_of
                .iter()
                .map(|(host, cid)| (*cid, *host))
                .collect(),
        ));
        self.game.host_district_sites = Arc::new(host_district_sites_from(
            &state.host_district_sites,
            &self
                .cid_of
                .iter()
                .map(|(civ6, cid)| (*cid, *civ6))
                .collect(),
            &self.game.rules,
        ));
        self.game.host_wonder_sites = Arc::new(host_wonder_sites_from(
            &state.host_wonder_sites,
            &self
                .cid_of
                .iter()
                .map(|(civ6, cid)| (*cid, *civ6))
                .collect(),
            &self.game.rules,
        ));
        // The wonder half of the same event, unioned for the same reason.
        let refused_wonders = blocked_wonders_from(
            &state.refused_wonders,
            &self
                .cid_of
                .iter()
                .map(|(civ6, cid)| (*cid, *civ6))
                .collect(),
            &self.game.rules,
        );
        for (cid, names) in refused_wonders {
            Arc::make_mut(&mut self.game.blocked_wonders)
                .entry(cid)
                .or_default()
                .extend(names);
        }
        let unavailable_wonders =
            host_unavailable_wonders_from(&state.host_unavailable_wonders, &self.game.rules);
        Arc::make_mut(&mut self.game.host_unavailable_wonders).extend(unavailable_wonders);
        // Unlike impossible district plots, a production refusal can be temporary.
        // Replace this cooldown snapshot so entries disappear after their TTL.
        let blocked_production = blocked_production_from(
            &state.refused_production,
            &self
                .cid_of
                .iter()
                .map(|(civ6, cid)| (*cid, *civ6))
                .collect(),
            &self.game.rules,
        );
        self.game.replace_blocked_production(blocked_production);
        // The host's menus, replaced like the cooldown snapshot above so a
        // city whose export lost the key stops being gated.
        let menus = host_menus_from(
            &state.cities,
            &self
                .cid_of
                .iter()
                .map(|(civ6, cid)| (*cid, *civ6))
                .collect(),
            &self.game.rules,
        );
        self.game
            .replace_host_unit_resource_prices(menus.unit_resource_prices);
        self.game.replace_host_saved_work(menus.saved_work);
        self.game.host_plot_purchases = std::sync::Arc::new(menus.plot_purchases);
        self.game
            .replace_host_menus(menus.buildable, menus.purchasable, menus.district_plots);
        let blocked_purchases = blocked_production_from(
            &state.refused_purchases,
            &self
                .cid_of
                .iter()
                .map(|(civ6, cid)| (*cid, *civ6))
                .collect(),
            &self.game.rules,
        );
        self.game.replace_blocked_purchases(blocked_purchases);
        // Unit ids are only in hand here, so the promotion blocks are wired late for
        // the same reason the production blocks are.
        self.game.blocked_promotions = Arc::new(blocked_promotions_from(
            &state.refused_promotions,
            &self.civ6_of,
            &self.game.rules,
        ));
        self.game.host_band_promotions = Arc::new(host_band_promotions_from(
            state,
            &self.civ6_of,
            &self.game.rules,
        ));
        self.game.blocked_strikes =
            Arc::new(blocked_strikes_from(&state.refused_strikes, &self.civ6_of));
        self.game.host_previews = Arc::new(host_previews_from(&state.host_previews, &self.civ6_of));
        // ⚠ ONE ORDERED STEP LIST, WALKED BY BOTH PASSES. See
        // `HOST_STATE_STEPS`: everything this method and `rebuild_from_state`
        // apply from the host state is written there once and told apart by
        // `MirrorMode`. The refusal and menu wiring above is the deliberate
        // exception — it UNIONS into what the caller already holds where the
        // rebuild replaces, and so it runs here rather than after the board.
        let mut ctx = HostStepCtx::new(
            &mut self.game,
            snapshot,
            state,
            &mut self.unmapped,
            &mut self.last_treasury,
            pass,
        );
        run_host_steps(&mut ctx, HostPhase::Empire);
        run_host_steps(&mut ctx, HostPhase::Economy);
        if let Some(civ6) = &state.government {
            if let Some(name) = civvis_node_name(&self.game.rules.governments, civ6, "GOVERNMENT_")
            {
                // Same rule on the sync path: see the rebuild path above.
                let changed = self.game.players[0].government.as_ref() != Some(&name);
                self.game.players[0].government = Some(name);
                if changed {
                    self.game.prune_policies_to_government(0);
                }
            } else if !self.unmapped.contains(civ6) {
                self.unmapped.push(civ6.clone());
            }
        }
        // History too, same as the rebuild path: without it the planner prices
        // a return switch as Anarchy-free and re-proposes it forever.
        for civ6 in &state.used_governments {
            if let Some(name) = civvis_node_name(&self.game.rules.governments, civ6, "GOVERNMENT_")
            {
                self.game.players[0].past_governments.insert(name);
            } else if !self.unmapped.contains(civ6) {
                self.unmapped.push(civ6.clone());
            }
        }
        if let Some(civ6) = &state.pantheon {
            let name = civ6
                .strip_prefix("BELIEF_")
                .unwrap_or(civ6)
                .to_ascii_lowercase();
            self.game.players[0].pantheon = Some(name.clone());
            if !self.game.players[0].religion_beliefs.contains(&name) {
                self.game.players[0].religion_beliefs.push(name);
            }
        }
        let mut policies = std::collections::BTreeSet::new();
        for civ6 in &state.policies {
            if let Some(name) = civvis_node_name(&self.game.rules.policies, civ6, "POLICY_") {
                policies.insert(crate::name::Name::new(&name));
            } else if !self.unmapped.contains(civ6) {
                self.unmapped.push(civ6.clone());
            }
        }
        self.game.players[0].policies = policies;
        for civ6 in &state.techs {
            if let Some(name) = civvis_node_name(&self.game.rules.techs, civ6, "TECH_") {
                self.game.players[0]
                    .techs
                    .insert(crate::name::Name::new(&name));
            }
        }
        if let Some(projects) =
            completed_strategic_projects(state.science_projects.as_deref(), &mut self.unmapped)
        {
            self.game.players[0].science_projects = projects;
        }
        for civ6 in &state.civics {
            if let Some(name) = civvis_node_name(&self.game.rules.civics, civ6, "CIVIC_") {
                self.game.players[0]
                    .civics
                    .insert(crate::name::Name::new(&name));
            }
        }
        // ⚠ REPLACED, not merged. A boost is spent the moment its technology is
        // researched, and the host reports only the ones still outstanding — so
        // carrying last turn's set forward would keep paying `tech_value`'s +28
        // for discounts that no longer exist.
        self.game.players[0].boosted_techs = state
            .boosted_techs
            .iter()
            .filter_map(|civ6| civvis_node_name(&self.game.rules.techs, civ6, "TECH_"))
            .map(|name| crate::name::Name::new(&name))
            .collect();
        self.game.players[0].boosted_civics = state
            .boosted_civics
            .iter()
            .filter_map(|civ6| civvis_node_name(&self.game.rules.civics, civ6, "CIVIC_"))
            .map(|name| crate::name::Name::new(&name))
            .collect();
        self.game.players[0].research = match &state.research {
            Some(civ6) => match civvis_node_name(&self.game.rules.techs, civ6, "TECH_") {
                Some(name) => Some(name),
                None => {
                    if !self.unmapped.contains(civ6) {
                        self.unmapped.push(civ6.clone());
                    }
                    None
                }
            },
            None => None,
        };
        self.game.players[0].research_progress = if self.game.players[0].research.is_some()
            && state.research_progress.is_finite()
            && state.research_progress >= 0.0
        {
            state.research_progress
        } else {
            0.0
        };
        self.game.players[0].research_overflow = 0.0;
        self.game.players[0].civic = match &state.civic {
            Some(civ6) => match civvis_node_name(&self.game.rules.civics, civ6, "CIVIC_") {
                Some(name) => Some(name),
                None => {
                    if !self.unmapped.contains(civ6) {
                        self.unmapped.push(civ6.clone());
                    }
                    None
                }
            },
            None => None,
        };
        self.game.players[0].civic_progress = if self.game.players[0].civic.is_some()
            && state.civic_progress.is_finite()
            && state.civic_progress >= 0.0
        {
            state.civic_progress
        } else {
            0.0
        };
        self.game.players[0].civic_overflow = 0.0;

        // Newly revealed ground, the traversability prior redrawn beyond it, and
        // the borders and city memory that moved with it — the mid-pass only this
        // path takes. See `HOST_STATE_STEPS`.
        run_host_steps(
            &mut HostStepCtx::new(
                &mut self.game,
                snapshot,
                state,
                &mut self.unmapped,
                &mut self.last_treasury,
                pass,
            ),
            HostPhase::Refresh,
        );

        // --- our units -------------------------------------------------------
        if !skip_units {
            // Re-read from this export; a unit that stopped carrying a key
            // falls back to the board's rule rather than keeping a stale fact.
            Arc::make_mut(&mut self.game.host_unit_facts).clear();
        }
        let mut seen: std::collections::BTreeSet<i64> = std::collections::BTreeSet::new();
        for unit in if skip_units {
            &[][..]
        } else {
            &state.units[..]
        } {
            seen.insert(unit.id);
            if !snapshot.is_revealed((unit.x, unit.y)) {
                continue;
            }
            let pos = crate::hex::offset_to_axial(unit.x, unit.y);
            match self.uid_of.get(&unit.id).copied() {
                Some(uid) if self.game.units.contains_key(&uid) => {
                    if self.game.units[&uid].pos != pos {
                        self.game.relocate(uid, pos);
                    }
                    // A modeled action can change the row before its native verdict.
                    // Re-read the supported type before movement or observations;
                    // the host snapshot is authoritative about its capabilities.
                    if let Some(name) = resolved_civvis_unit_name(&self.game.rules, &unit.kind) {
                        if let Some(live) = self.game.units.get_mut(&uid) {
                            live.kind = Name::new(&name);
                        }
                    }
                    let progress =
                        observed_unit_progress(&self.game.rules, unit, &mut self.unmapped);
                    // Before the allowance below is taken: `unit_max_moves`
                    // reads the host's `max_moves` off this.
                    record_host_unit_facts(&mut self.game, uid, unit);
                    if let Some(live) = self.game.units.get_mut(&uid) {
                        apply_unit_observation(live, unit, progress);
                        // ★★★★★ REFRESH THE TURN FROM REALITY, DO NOT SIMULATE IT.
                        //
                        // `take_turn` spends a unit's movement, and on a persistent
                        // game nothing hands it back — so after the first turn every
                        // unit had `moves_left = 0` and CIVVIS returned ZERO orders on
                        // every subsequent turn while its plan kept evolving. Measured:
                        // 10 orders on the first synced turn, then 0, 0, 0, 0.
                        //
                        // `Game::begin_turn` would reset this, but it also runs CIVVIS's
                        // own loyalty, pressure, trade and great-people processing —
                        // simulating a second game beside the real one and drifting from
                        // it. Civilization VI already reports the truth (`moves` comes
                        // from `GetMovesRemaining`), so take it from there. Reality is
                        // cheaper and cannot diverge.
                        // ⚠⚠⚠ FULL MOVEMENT, NOT THE EXPORTED `moves`. I had this
                        // backwards and it silenced CIVVIS completely.
                        //
                        // The reasoning was "take movement from reality, reality cannot
                        // diverge". But the quantity Civilization VI reports is not the
                        // one that was assumed: measured on run civvis-20260730T120107Z,
                        // the export at the START of turn 31 had **7 of 8 units at
                        // `moves: 0`**. `GetMovesRemaining` at the instant `beginTurn`
                        // runs does not mean "movement available this turn", so feeding
                        // it in left `advanced_units` breaking immediately on
                        // `moves_left <= 0.0` for almost every unit — 0 actions logged
                        // on every turn after the first, with the plan still evolving.
                        //
                        // A unit facing a fresh turn has its full allowance, which is
                        // what the working one-shot path gives it via `spawn_unit`. If
                        // CIVVIS then orders a unit that really cannot move,
                        // `canOperate` refuses it in the mod and it is counted — a
                        // wasted order, not a silent paralysis.
                        let allowance = mirror_unit_moves_for(
                            &self.game,
                            uid,
                            Some(unit),
                            state.seat.moves_at_turn_start,
                        );
                        if let Some(live) = self.game.units.get_mut(&uid) {
                            live.moves_left = allowance;
                            live.acted = false;
                            // The host says how many strikes are left; a
                            // frame's re-plan then cannot spend one twice.
                            live.attacks_left = if state.seat.moves_at_turn_start {
                                unit.attacks_remaining.unwrap_or(1).max(0)
                            } else {
                                1
                            };
                            // Cleared by `Game::begin_turn` every turn; on a persistent
                            // game they survive and a unit that "already moved" is
                            // skipped.
                            live.moved = false;
                            live.zoc_stopped = false;
                        }
                    }
                }
                _ => {
                    let Some(name) = resolved_civvis_unit_name(&self.game.rules, &unit.kind) else {
                        if !self.unmapped.contains(&unit.kind) {
                            self.unmapped.push(unit.kind.clone());
                        }
                        continue;
                    };
                    let water = self
                        .game
                        .map
                        .get(pos)
                        .map(|tile| self.game.rules.is_water(tile))
                        .unwrap_or(true);
                    if water {
                        continue;
                    }
                    let uid = self.game.spawn_unit(&name, 0, pos);
                    self.uid_of.insert(unit.id, uid);
                    self.civ6_of.insert(uid, unit.id);
                    record_host_unit_facts(&mut self.game, uid, unit);
                }
            }
        }
        // Anything Civilization VI no longer reports is dead or consumed. Leaving it
        // in would have CIVVIS planning with an army that does not exist.
        let gone: Vec<i64> = self
            .uid_of
            .keys()
            .copied()
            .filter(|civ6| !seen.contains(civ6))
            .collect();
        for civ6 in gone {
            if let Some(uid) = self.uid_of.remove(&civ6) {
                self.civ6_of.remove(&uid);
                Arc::make_mut(&mut self.game.host_unit_facts).remove(&uid);
                if self.game.units.contains_key(&uid) {
                    self.game.remove_unit(uid);
                }
            }
        }

        // --- our cities ------------------------------------------------------
        // Firaxis's own-city roster is authoritative. A city that disappeared
        // from it was captured or razed; retaining it as ours corrupts population,
        // territory, production, score and every military objective at once.
        let own_host_ids: std::collections::BTreeSet<i64> =
            state.cities.iter().map(|city| city.id).collect();
        let foreign_city_positions: std::collections::BTreeSet<(i32, i32)> = state
            .rivals
            .iter()
            .flat_map(|rival| rival.cities.iter())
            .chain(state.minors.iter().flat_map(|minor| minor.cities.iter()))
            .map(|city| (city.x, city.y))
            .collect();
        let gone: Vec<(i64, u32)> = self
            .cid_of
            .iter()
            .filter(|(host, _)| !own_host_ids.contains(host))
            .map(|(host, cid)| (*host, *cid))
            .collect();
        for (host, cid) in gone {
            if let Some(city) = self.game.cities.get(&cid) {
                self.rival_cities
                    .remove(&crate::hex::axial_to_offset(city.pos.0, city.pos.1));
            }
            self.cid_of.remove(&host);
            self.known_city_ids.retain(|_, known| *known != cid);
            let captured = self.game.cities.get(&cid).is_some_and(|city| {
                foreign_city_positions
                    .contains(&crate::hex::axial_to_offset(city.pos.0, city.pos.1))
            });
            if !captured {
                self.game.mirror_remove_city(cid);
            }
        }
        // A rival city captured by us already occupies its plot. Adopt that exact
        // city rather than refusing to place our newly observed one on an occupied
        // tile and leaving its former owner in place forever.
        for city in &state.cities {
            let pos = crate::hex::offset_to_axial(city.x, city.y);
            let existing = self
                .cid_of
                .get(&city.id)
                .copied()
                .filter(|cid| self.game.cities.contains_key(cid))
                .or_else(|| self.game.city_at(pos));
            if let Some(cid) = existing {
                self.cid_of
                    .retain(|host, mapped| *mapped != cid || *host == city.id);
                self.cid_of.insert(city.id, cid);
                if city.id > 0 {
                    self.known_city_ids.insert(city.id, cid);
                }
                self.game.mirror_set_city_owner(cid, 0);
                self.rival_cities.remove(&(city.x, city.y));
            }
        }
        for city in &state.cities {
            if self.cid_of.contains_key(&city.id) || !snapshot.is_revealed((city.x, city.y)) {
                continue;
            }
            let pos = crate::hex::offset_to_axial(city.x, city.y);
            let water = self
                .game
                .map
                .get(pos)
                .map(|tile| self.game.rules.is_water(tile))
                .unwrap_or(true);
            if water || self.game.city_at(pos).is_some() {
                continue;
            }
            let cid = self.game.place_city(0, pos, banner(city));
            self.cid_of.insert(city.id, cid);
            if city.id > 0 {
                self.known_city_ids.insert(city.id, cid);
            }
        }
        let capture_seats = host_capture_seats(&self.game, state);
        for city in &state.cities {
            if let Some(cid) = self.cid_of.get(&city.id) {
                // The authoritative mirror never simulates a queue itself: the
                // decider runs on a clone.  Replacing this with the live item is
                // therefore both safe and necessary.  Leaving the startup item in
                // place made a completed Scout look like an in-progress Scout
                // forever, and a new Settler looked like an idle city.
                let queued = civvis_production_item(
                    &self.game.rules,
                    city.producing.as_deref(),
                    &city.districts,
                    Some(crate::hex::offset_to_axial(city.x, city.y)),
                );
                // This must run before replacing `live.queue`: the old queue is the
                // only exact identity of an abandoned district foundation.
                apply_observed_city_infrastructure(&mut self.game, *cid, city, &mut self.unmapped);
                if let Some(live) = self.game.cities.get_mut(cid) {
                    if city.pop > 0 {
                        live.pop = city.pop;
                    }
                    if city.loyalty >= 0.0 {
                        live.loyalty = city.loyalty;
                    }
                    if city.food >= 0.0 {
                        live.food = city.food;
                    }
                    apply_city_religion(live, city);
                    Arc::make_mut(&mut self.game.observed_city_religion_markers).insert(*cid);
                    apply_city_capture(live, city, &capture_seats);
                    // Firaxis exports the current item, not a speculative
                    // multi-item queue.  Clear even when the item is absent: a
                    // finished build is an empty queue in the real game, not the
                    // last thing CIVVIS happened to see.
                    let had_assigned_production = !live.queue.is_empty();
                    live.queue.clear();
                    if let Some(item) = queued {
                        live.queue.push(item);
                    }
                    for queued in host_queue_tail(&self.game.rules, city) {
                        if !live.queue.contains(&queued) {
                            live.queue.push(queued);
                        }
                    }
                    if city.production_progress.is_finite() && city.production_progress >= 0.0 {
                        live.production = city.production_progress;
                    } else if had_assigned_production && live.queue.is_empty() {
                        // A vanished head's old work is not observed overflow.
                        // Keeping it would credit that completed item again when
                        // an idle city resumes a paused district. This discards
                        // stale assigned work, not an explicitly observed idle
                        // balance; actual native overflow remains unknown.
                        live.production = 0.0;
                    }
                    // Same translation as the rebuild path, and for the same reason:
                    // an untranslated name here panics `rules.buildings[..]` later.
                    live.buildings.clear();
                    for civ6 in &city.buildings {
                        if let Some(name) =
                            civvis_node_name(&self.game.rules.buildings, civ6, "BUILDING_")
                        {
                            let named = crate::name::Name::new(&name);
                            if !live.buildings.contains(&named) {
                                live.buildings.push(named);
                            }
                        } else if civvis_node_name(&self.game.rules.wonders, civ6, "BUILDING_")
                            .is_none()
                        {
                            let issue = format!("{civ6}:building");
                            if !self.unmapped.contains(&issue) {
                                self.unmapped.push(issue);
                            }
                        }
                    }
                    apply_pillaged_buildings(&self.game.rules, live, city);
                }
            }
        }

        // --- rivals ----------------------------------------------------------
        // Rebuilt wholesale: what we can see of them is fog-dependent and they carry
        // no plan of ours worth preserving.
        if skip_rivals {
            seat_live_spies(&mut self.game, state);
            block_live_spy_production(&mut self.game, state.spy_capacity);
            apply_governor_state(&mut self.game, state, &mut self.unmapped);
            apply_great_person_points(&mut self.game, state, &mut self.unmapped);
            apply_strategic_stockpiles(&mut self.game, state, &mut self.unmapped);
            strategic_income::apply(&mut self.game, state, &mut self.unmapped);
            apply_research_quotes(&mut self.game, state, &mut self.unmapped);
            research_tree::apply(&mut self.game, state, &mut self.unmapped);
            return;
        }
        self.foreign_uid_of.clear();
        for uid in std::mem::take(&mut self.rival_units) {
            if self.game.units.contains_key(&uid) {
                self.game.remove_unit(uid);
            }
        }

        // ★★★★★ BARBARIANS WERE PLANTED ONCE AND NEVER LOOKED AT AGAIN.
        //
        // `sync` had **no reference to `state.hostiles` or `barb_pid` at all**, so on
        // the persistent mirror the decider runs, barbarians were whatever the
        // construction rebuild found and nothing after. At turn 1 that is normally
        // NONE — so CIVVIS played entire games with an empty barbarian seat while the
        // export named them every turn.
        //
        // Measured on live run civvis-20260801T040700Z: Montréal founded turn 26, GONE
        // by turn 42, loyalty 100 the whole time and at war with nobody it had met —
        // so neither revolt nor a rival took it. `hostiles` was non-empty in the
        // export throughout. A seat that cannot see barbarians cannot garrison against
        // them, and "expansion that cannot be held is not expansion".
        //
        // Rebuilt wholesale each sync for the same reason as the rivals above: what we
        // can see of them is fog-dependent and they carry no plan of ours.
        for uid in std::mem::take(&mut self.hostile_units) {
            if self.game.units.contains_key(&uid) {
                self.game.remove_unit(uid);
            }
        }
        // See `rebuild_from_state`: a Free Cities unit goes on the Free Cities seat,
        // and one the actor's own `units[]` carries is planted from there instead.
        let free_cities_seat = self
            .game
            .players
            .iter()
            .find(|player| player.is_free_city)
            .map(|player| player.id);
        for unit in &state.hostiles {
            let free = hostile_is_free_cities(state, unit);
            if free && hostile_exported_as_minor_unit(state, unit) {
                continue;
            }
            let seat = if free {
                free_cities_seat.or(self.game.barb_pid)
            } else {
                self.game.barb_pid
            };
            let Some(owner) = seat else {
                self.dropped_units.push(format!(
                    "{}@{},{}:no_barbarian_seat",
                    unit.kind, unit.x, unit.y
                ));
                continue;
            };
            let Some(name) = resolved_civvis_unit_name(&self.game.rules, &unit.kind) else {
                // ⚠ Counted, not swallowed. A barbarian type CIVVIS cannot name is
                // a threat it cannot see, and that is the whole of this defect.
                if !self.unmapped.contains(&unit.kind) {
                    self.unmapped.push(unit.kind.clone());
                }
                continue;
            };
            if foreign_spy_is_hidden(owner, &name) {
                continue;
            }
            let pos = crate::hex::offset_to_axial(unit.x, unit.y);
            if self.game.map.get(pos).is_none() || self.game.units.values().any(|u| u.pos == pos) {
                self.dropped_units
                    .push(format!("{}@{},{}:hostile_tile", unit.kind, unit.x, unit.y));
                continue;
            }
            let uid = self.game.spawn_unit(&name, owner, pos);
            let progress = observed_unit_progress(&self.game.rules, unit, &mut self.unmapped);
            if let Some(live) = self.game.units.get_mut(&uid) {
                apply_unit_observation(live, unit, progress);
                self.hostile_units.push(uid);
                self.foreign_uid_of.insert(unit.id, uid);
            }
            record_host_unit_facts(&mut self.game, uid, unit);
            apply_foreign_unit_strikes(&mut self.game, uid, unit);
        }

        // `state.rivals` is met-gated, so use each rival's original Firaxis id
        // to reach its stable model seat. A missing player must not shift all
        // later empire facts onto the wrong civilization.
        let mut seat_of_host = host_major_seat_map(state, modeled_major_player_count(&self.game));
        for rival in &state.rivals {
            let Some(&owner) = seat_of_host.get(&rival.player) else {
                continue;
            };
            if owner >= self.game.players.len() {
                continue;
            }
            if rival.military.is_finite() && rival.military >= 0.0 {
                Arc::make_mut(&mut self.game.observed_military_power).insert(owner, rival.military);
            }
            if rival.score >= 0 {
                Arc::make_mut(&mut self.game.observed_score).insert(owner, rival.score);
            }
            if let Some(dvp) = rival.dvp {
                self.game.players[owner].dvp = dvp;
            }
            // ★★★★★ MIRROR THE WAR PERMISSION, NOT CIVVIS'S BOOKKEEPING.
            //
            // `preferred_war_opening` wants a casus belli; failing that it denounces a
            // major rival and waits for `denounced_until` to mature into a Formal War.
            // Nothing matures in a reconstruction with no turn processing, so the wait
            // is forever: 81 replayed turns, `strategy = conquest` on 26 of them, and
            // ZERO declarations.
            //
            // Civilization VI has already answered the only question that matters —
            // `CanDeclareWarOn` — so when it says yes, mark the rival as denounced far
            // enough back that the Formal War option is open. This mirrors a real
            // permission the seat holds; it does not invent one. When Civ 6 says no,
            // nothing is set and CIVVIS is correctly unable to declare.
            // ★★★★★ TELL CIVVIS WE HAVE MET THEM. This is the whole reason no war was
            // ever declared.
            //
            // `legal_actions` gates its ENTIRE diplomacy block on `has_met(pid, o.id)`,
            // which reads `players[viewer].met` — a set this reconstruction never
            // populated. So `Action::DeclareWar` was never even a LEGAL action, and
            // CIVVIS could not have declared war however much it wanted to.
            //
            // Measured at turn 184 of run civvis-20260730T142203Z: `can_declare: true`,
            // **our military 163 against their 46**, three of their cities visible —
            // and no declaration. Nothing in CIVVIS's judgement was wrong; the option
            // did not exist.
            //
            // ⚠ Only rivals in `state.rivals` are inserted, and the mod exports a rival
            // ONLY once `HasMet` is true, so this cannot hand CIVVIS contact the seat
            // has not earned.
            self.game.players[0].met.insert(owner);
            if owner < self.game.players.len() {
                self.game.players[owner].met.insert(0);
            }
            // ★★★★★ TELL CIVVIS IT IS AT WAR. This is the last link in the chain, and
            // without it CIVVIS FORGETS THE WAR IT DECLARED, every single turn.
            //
            // Measured on the deepest healthy run: CIVVIS declared war on Civ 6 player 3
            // at turn 85 on its own judgement, our military 451 against their 200, their
            // CAPITAL visible at (45,10) nine tiles from one of our cities — and the
            // army milled around at home for seventy turns. Its journal said exactly
            // why: "Holding off war with Egypt | no city of theirs is within 18 tiles of
            // one of mine". It was planning a FRESH campaign against a different,
            // distant civilization, because the reconstruction reported `at_war = false`
            // for everyone and a war it cannot see is a war it cannot prosecute.
            //
            // `at_war` was exported by the mod from the start and simply never applied —
            // the same shape as the unpopulated `met` set, the invisible buildings and
            // the phantom settler.
            let bond = if 0 < owner { (0, owner) } else { (owner, 0) };
            if rival.at_war {
                self.game.at_war.insert(bond);
            } else {
                self.game.at_war.remove(&bond);
            }
            // The permission above, and — when the mod exports it — the
            // host's whole relationship for this rival: state, grievance
            // ledger, alliance, missions, promises and visibility. See
            // `apply_host_diplomacy`.
            apply_host_diplomacy(&mut self.game, owner, rival);
            // The host's "received Open Borders" answer, applied the same way
            // `at_war` is: assigned from every export, so a lapsed agreement
            // closes the mirrored grant on the next sync. The border-sealing
            // pass reads this grant to stop sealing a rival whose passage the
            // seat just bought.
            if rival.open_borders == Some(true) {
                self.game.players[owner]
                    .open_borders_until
                    .insert(0, self.game.turn + 2);
            } else {
                self.game.players[owner].open_borders_until.remove(&0);
            }
            // See `rebuild_from_state`: the host's own Early Empire answer
            // and the rival's tree by name, re-read from every export.
            apply_rival_tree(&mut self.game, owner, rival, &mut self.unmapped);
            for city in &rival.cities {
                if !snapshot.is_revealed((city.x, city.y)) {
                    continue;
                }
                let pos = crate::hex::offset_to_axial(city.x, city.y);
                let cid = if let Some(cid) = self.game.city_at(pos) {
                    self.cid_of.retain(|_, mapped| *mapped != cid);
                    if city.id > 0 {
                        self.known_city_ids.insert(city.id, cid);
                    }
                    self.game.mirror_set_city_owner(cid, owner);
                    self.rival_cities.insert((city.x, city.y));
                    cid
                } else {
                    let water = self
                        .game
                        .map
                        .get(pos)
                        .map(|tile| self.game.rules.is_water(tile))
                        .unwrap_or(true);
                    if water {
                        continue;
                    }
                    let cid = self.game.place_city(owner, pos, banner(city));
                    if city.id > 0 {
                        self.known_city_ids.insert(city.id, cid);
                    }
                    self.rival_cities.insert((city.x, city.y));
                    cid
                };
                apply_observed_city_infrastructure(&mut self.game, cid, city, &mut self.unmapped);
            }
            for unit in &rival.units {
                let Some(name) = resolved_civvis_unit_name(&self.game.rules, &unit.kind) else {
                    if !self.unmapped.contains(&unit.kind) {
                        self.unmapped.push(unit.kind.clone());
                    }
                    continue;
                };
                if foreign_spy_is_hidden(owner, &name) {
                    continue;
                }
                let pos = crate::hex::offset_to_axial(unit.x, unit.y);
                if self.game.map.get(pos).is_none() {
                    continue;
                }
                let uid = self.game.spawn_unit(&name, owner, pos);
                let progress = observed_unit_progress(&self.game.rules, unit, &mut self.unmapped);
                if let Some(live) = self.game.units.get_mut(&uid) {
                    apply_unit_observation(live, unit, progress);
                    self.rival_units.push(uid);
                    self.foreign_uid_of.insert(unit.id, uid);
                }
                record_host_unit_facts(&mut self.game, uid, unit);
                apply_foreign_unit_strikes(&mut self.game, uid, unit);
            }
        }

        let free_city_seats: Vec<usize> = self
            .game
            .players
            .iter()
            .filter(|player| player.is_free_city)
            .map(|player| player.id)
            .collect();
        // A Free Cities seat holding units planted from `hostiles[]` above is alive
        // whether or not the aggregate actor is exported under `minors[]`.
        let free_cities_armed = state
            .hostiles
            .iter()
            .any(|unit| hostile_is_free_cities(state, unit));
        for owner in free_city_seats {
            self.game.players[owner].alive = free_cities_armed;
            self.game.at_war.remove(&(0, owner));
            Arc::make_mut(&mut self.game.observed_score).remove(&owner);
            Arc::make_mut(&mut self.game.observed_military_power).remove(&owner);
        }
        let minor_assignments = minor_actor_assignments(&self.game, state);
        for &(minor, owner) in &minor_assignments {
            seat_of_host.insert(minor.player, owner);
            if self.game.players[owner].is_free_city {
                self.game.players[owner].alive = true;
            }
            self.game.players[0].met.insert(owner);
            self.game.players[owner].met.insert(0);
            // Net of an established Amani's terms: see `host_amani_envoy_terms`.
            let amani = host_amani_envoy_terms(&self.game.rules, state, minor);
            set_mirrored_envoys(
                &mut self.game.players[0],
                owner,
                raw_envoys_for(minor.envoys, amani),
            );
            apply_host_rival_envoys(&mut self.game, minor, owner, &seat_of_host);
            apply_host_quest(&mut self.game, minor, owner, &mut self.unmapped);
            if minor.score >= 0 {
                Arc::make_mut(&mut self.game.observed_score).insert(owner, minor.score);
            }
            if minor.military.is_finite() && minor.military >= 0.0 {
                Arc::make_mut(&mut self.game.observed_military_power).insert(owner, minor.military);
            }
            if minor.at_war {
                self.game.at_war.insert((0, owner));
            } else {
                self.game.at_war.remove(&(0, owner));
            }
            // See `rebuild_from_state`: the city-state's own border, re-read.
            self.game.players[owner].borders_enforced =
                Some(minor.enforces_borders.unwrap_or(true));
            for city in &minor.cities {
                if !snapshot.is_revealed((city.x, city.y)) {
                    continue;
                }
                let pos = crate::hex::offset_to_axial(city.x, city.y);
                let cid = if let Some(cid) = self.game.city_at(pos) {
                    self.cid_of.retain(|_, mapped| *mapped != cid);
                    if city.id > 0 {
                        self.known_city_ids.insert(city.id, cid);
                    }
                    self.game.mirror_set_city_owner(cid, owner);
                    self.rival_cities.insert((city.x, city.y));
                    Some(cid)
                } else if self.game.map.get(pos).is_some() {
                    let cid = self.game.place_city(owner, pos, banner(city));
                    if city.id > 0 {
                        self.known_city_ids.insert(city.id, cid);
                    }
                    self.rival_cities.insert((city.x, city.y));
                    Some(cid)
                } else {
                    None
                };
                if let Some(cid) = cid {
                    apply_observed_city_infrastructure(
                        &mut self.game,
                        cid,
                        city,
                        &mut self.unmapped,
                    );
                }
            }
            for unit in &minor.units {
                let Some(name) = resolved_civvis_unit_name(&self.game.rules, &unit.kind) else {
                    if !self.unmapped.contains(&unit.kind) {
                        self.unmapped.push(unit.kind.clone());
                    }
                    continue;
                };
                if foreign_spy_is_hidden(owner, &name) {
                    continue;
                }
                let pos = crate::hex::offset_to_axial(unit.x, unit.y);
                if self.game.map.get(pos).is_none() {
                    continue;
                }
                // ★★★★ A CITY-STATE'S UNIT IS PLANTED LIKE A RIVAL'S, ON AN
                // OCCUPIED PLOT TOO.
                //
                // This loop used to carry `&& !self.game.units.values().any(|live|
                // live.pos == pos)`, so a minor's unit sharing a plot with anything
                // already planted was silently DROPPED — not counted in
                // `dropped_units`, not named in `unmapped`, simply absent. The
                // hostiles above are planted first (with their own guard) and the
                // rivals second with NO guard, so the board could see a major's
                // Trader stacked on a barbarian and never the city-state's.
                //
                // A unit the mirror cannot see is a unit no veto can refuse to
                // shoot. Measured 2026-08-29: `civvis-20260827T145140Z` t52 struck
                // the plot Bologna's TRADER stood on and `civvis-20260829T022207Z`
                // t66 the plot Kumasi's did — both invisible on our own board, both
                // a surprise war on the host. `rebuild_from_state` has always
                // planted minors through the same `plant_unit` as rivals, with no
                // such guard; `sync` was the odd one out. See
                // `Game::peaceful_foreign_unit_at`.
                let uid = self.game.spawn_unit(&name, owner, pos);
                let progress = observed_unit_progress(&self.game.rules, unit, &mut self.unmapped);
                if let Some(live) = self.game.units.get_mut(&uid) {
                    apply_unit_observation(live, unit, progress);
                    self.rival_units.push(uid);
                    self.foreign_uid_of.insert(unit.id, uid);
                }
                record_host_unit_facts(&mut self.game, uid, unit);
                apply_foreign_unit_strikes(&mut self.game, uid, unit);
            }
        }
        for &(minor, owner) in &minor_assignments {
            let amani = host_amani_envoy_terms(&self.game.rules, state, minor);
            seed_mirrored_suzerainty(&mut self.game, minor, owner, &seat_of_host, amani);
        }

        self.active_trade_route_traders = active_trade_route_traders(state);
        // Every whole-board pass, now that every city and unit — ours, the
        // rivals' and the minors' — has been reconciled against its Civ 6 id.
        // See `HOST_STATE_STEPS`.
        let mut ctx = HostStepCtx::new(
            &mut self.game,
            snapshot,
            state,
            &mut self.unmapped,
            &mut self.last_treasury,
            pass,
        )
        .with_board(&self.known_city_ids, &minor_assignments, &seat_of_host);
        run_host_steps(&mut ctx, HostPhase::Board);
        run_host_steps(&mut ctx, HostPhase::Finish);
        // The posting can name a newly observed foreign city. Resolve it
        // after the same whole-board reconstruction as the district itself.
        seat_live_spies(&mut self.game, state);
        block_live_spy_production(&mut self.game, state.spy_capacity);
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod transient_refusal_tests;

#[cfg(test)]
mod host_fact_tests;

#[cfg(test)]
mod host_resource_price_tests;

#[cfg(test)]
mod host_upgrade_resource_tests;

#[cfg(test)]
mod enemy_district_pillage_tests;

#[cfg(test)]
#[path = "mirror/city_ranged_strength/tests.rs"]
mod city_ranged_strength_tests;

#[cfg(test)]
mod envoy_permission_tests;

#[cfg(test)]
mod flood_state_tests;

#[cfg(test)]
mod war_type_permission_tests;

#[cfg(test)]
mod research_quote_tests;

#[cfg(test)]
mod air_refusal_tests;

#[cfg(test)]
mod lake_identity_tests;

#[cfg(test)]
mod foreign_encampment_health_tests;

#[cfg(test)]
mod foreign_district_parent_tests;

#[cfg(test)]
mod defending_district_observation_tests;

#[cfg(test)]
mod improvement_owner_tests;

#[cfg(test)]
mod native_base_game_tests;

#[cfg(test)]
mod native_strike_scan_tests;

#[cfg(test)]
mod native_state_turn_hint_tests;

/// Reuse the exact combat-kind prefilter only for an immutable line in the
/// current retained log. The observer still decides and records every death.
fn combat_line_may_match_in_log(raw: &str, range: (usize, usize)) -> bool {
    let cached = EVENTS_READ.with(|cell| {
        let mut slot = cell.borrow_mut();
        let read = slot.as_mut()?;
        if !std::ptr::eq(read.text.as_ptr(), raw.as_ptr()) || read.text.len() != raw.len() {
            return None;
        }
        Some(*read.combat_hints.entry(range).or_insert_with(|| {
            json_may_have_value(&raw[range.0..range.1], "\"kind\"", "\"combat\"")
        }))
    });
    cached.unwrap_or_else(|| json_may_have_value(&raw[range.0..range.1], "\"kind\"", "\"combat\""))
}

#[cfg(test)]
mod native_combat_kind_hint_tests;

#[cfg(test)]
mod latest_state_selection_tests;

#[cfg(test)]
mod mongolia_combat_tests;

#[cfg(test)]
mod ranged_handicap_tests;
