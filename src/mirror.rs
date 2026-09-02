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
    #[serde(default)]
    pub w: bool,
    #[serde(default)]
    pub i: bool,
    #[serde(default)]
    pub fw: Option<bool>,
    /// Improvement type name already built here, e.g. `IMPROVEMENT_FARM`.
    #[serde(default)]
    pub im: Option<String>,
    /// Whether that improvement is pillaged (`Plot:IsImprovementPillaged`). Sent
    /// only where an improvement stands; absent reads as not pillaged, which is
    /// what an older export meant too. A pillaged improvement pays nothing until
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
    let raw = std::fs::read_to_string(path)?;
    let state_line = latest_state_line(&raw, turn);
    // ★★★★ THE MOD WRITES THE TURN'S STATE FIRST AND ITS TILES SECOND.
    //
    // `beginTurn` in the mod is `exportState` then `exportTiles`, so in
    // `events.jsonl` every turn's sweep and delta sit BELOW the state line
    // they describe. Stopping at the state line therefore answered every
    // board on the PREVIOUS turn's map — and turn 1 on no map at all: every
    // live run of 2026-09-01 opened with "no revealed terrain or no state
    // yet", 0 orders, and the first Settler skipped its first turn
    // (`civvis-20260901T212354Z` … `T210954Z`, six of six). A tiles line
    // below the selected state still belongs to it when it is the same turn
    // and no later frame than the state's; a later frame's delta stays out
    // (`snapshot_stops_at_the_selected_state_before_a_later_mid_turn_delta`).
    let board = state_line
        .and_then(|limit| raw.lines().nth(limit))
        .and_then(|line| state_from_json(line).ok())
        .map(|state| (state.turn, state.frame));
    // In stream order, so a later chunk's plot wins whichever kind it is;
    // a delta (`CivvisTiles.sweep`) merges without standing for a sweep —
    // see `Snapshot::merge_delta`.
    let mut snapshot = Snapshot::default();
    for (line_number, line) in raw.lines().enumerate() {
        if !line.contains("\"tiles\"") {
            continue;
        }
        if state_line.is_some_and(|limit| line_number > limit) {
            let same_board = board.is_some_and(|(board_turn, board_frame)| {
                serde_json::from_str::<TilesBoardStamp>(line)
                    .is_ok_and(|stamp| stamp.turn == board_turn && stamp.frame <= board_frame)
            });
            if !same_board {
                continue;
            }
        }
        if let Ok(chunk) = serde_json::from_str::<TilesChunk>(line) {
            if !chunk.plots.is_empty() && turn.is_none_or(|limit| chunk.turn <= limit) {
                let is_delta =
                    serde_json::from_str::<TilesDeltaStamp>(line).is_ok_and(|stamp| stamp.delta);
                if is_delta {
                    snapshot.merge_delta(&chunk);
                } else {
                    snapshot.merge_sweep(&chunk);
                }
            }
        }
    }
    apply_finished_improvements(&raw, turn, state_line, &mut snapshot);
    Ok(snapshot)
}

/// The line at which [`state_from_events`] selects its state.
///
/// State selection is newest-wins for a turn, and highest-turn-wins when no
/// turn was requested. Keeping the same rule here makes a snapshot and a state
/// share one exact point in the append-only event stream.
fn latest_state_line(raw: &str, turn: Option<u32>) -> Option<usize> {
    let mut best: Option<(u32, usize)> = None;
    for (line_number, line) in raw.lines().enumerate() {
        if !line.contains("\"state\"") {
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
            .map(|(best_turn, _)| state.turn >= *best_turn)
            .unwrap_or(true)
        {
            best = Some((state.turn, line_number));
        }
    }
    best.map(|(_, line_number)| line_number)
}

/// Fold `improved` events onto the assembled map, so a finished improvement is
/// on the board before the next sweep repeats it.
///
/// ★★★★★ The sweep runs every few turns and until it does, the mirror shows the
/// ground bare — so CIVVIS re-orders what it has just built. Measured on run
/// `civvis-20260811T163652Z`: 23 duplicate improvement orders, every refusal 1–3
/// turns after a sweep, the ledger reading `IMPROVE:MINE` succeeded at t18 and
/// refused at t19 against `existing=IMPROVEMENT_MINE`.
///
/// Three rules make this safe, and each one is load-bearing:
///
/// 1. **Only the `im` field is touched.** [`Snapshot::from_chunks`] REPLACES a
///    plot (`revealed.insert(pos, plot.clone())`), so folding a partial plot in
///    as a one-plot chunk — the obvious cheap version — would strip that tile's
///    terrain, owner and resource. Mutating the one field cannot.
/// 2. **Only a plot the seat has already revealed.** An improvement on ground
///    never seen is not evidence the ground exists, and inventing a plot here
///    would hand the simulator information the seat does not have.
/// 3. **Only events at or after the newest chunk.** An older event cannot
///    override a fresher sweep, which is what keeps a removed improvement from
///    coming back.
///
/// ⚠ It does open a narrow window in the other direction: a tile improved and
/// then PILLAGED before the next sweep reads as improved until that sweep
/// corrects it. Pillaging is far rarer than building, the window is the same few
/// turns, and the sweep is authoritative either way — so this trades a common
/// error for a rare one rather than removing error altogether.
fn apply_finished_improvements(
    raw: &str,
    turn: Option<u32>,
    state_line: Option<usize>,
    snapshot: &mut Snapshot,
) {
    for (line_number, line) in raw.lines().enumerate() {
        if state_line.is_some_and(|limit| line_number > limit) {
            break;
        }
        if !line.contains("\"improved\"") {
            continue;
        }
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if event.get("kind").and_then(|k| k.as_str()) != Some("improved") {
            continue;
        }
        let at = event.get("turn").and_then(|v| v.as_u64()).unwrap_or(0) as u32;
        if turn.is_some_and(|limit| at > limit) || at < snapshot.turn {
            continue;
        }
        let (Some(x), Some(y), Some(im)) = (
            event.get("x").and_then(|v| v.as_i64()),
            event.get("y").and_then(|v| v.as_i64()),
            event.get("im").and_then(|v| v.as_str()),
        ) else {
            continue;
        };
        if let Some(plot) = snapshot.revealed.get_mut(&(x as i32, y as i32)) {
            plot.im = Some(im.to_string());
            // The plot's exported yields were read before this improvement
            // existed; see `Snapshot::set_improvement` for why they cannot
            // survive it.
            plot.yl = None;
        }
    }
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
    use crate::game::Game;
    let width = snapshot.width.max(1);
    let height = snapshot.height.max(1);
    let mut game = Game::new(players.max(2), width, height, seed, 500, city_states);
    std::sync::Arc::make_mut(&mut game.rules).enable_unknown_terrain();

    apply_terrain(&mut game, snapshot);
    game
}

/// Write every plot the seat has seen onto the map, and explicit unknowns elsewhere.
///
/// Shared by the one-shot rebuild and by [`LiveMirror::sync`], which has to re-apply
/// it as ground is revealed. An unresolved terrain name is unknown too: retaining the
/// generated tile below it would turn a translation failure into plausible fiction.
pub(crate) fn apply_terrain(game: &mut crate::game::Game, snapshot: &Snapshot) {
    let vocab = Vocabulary::embedded();
    let unknown = Name::new("unknown");
    let visible_resources: std::collections::BTreeSet<crate::name::Name> = game
        .rules
        .resources
        .keys()
        .filter(|name| game.resource_visible_to(0, name.as_str()))
        .cloned()
        .collect();
    let width = snapshot.width.max(1);
    let height = snapshot.height.max(1);
    // ⚠ THE REAL IMPROVEMENT SET, taken from the ruleset ONCE before the loop.
    //
    // This used to be a hardcoded list of sixteen names, because checking the ruleset
    // inside the loop needs a second borrow of `game` while `game.map.tiles` is held
    // mutably. The list then drifted: CIVVIS models **36** improvements and the list
    // named 16, so TWENTY modelled improvements read as unimproved ground — including
    // `barbarian_camp`, `goody_hut` and `meteor_goody`, which are precisely the three
    // `AdvancedAi` looks for when deciding whether a move invalidates a plan
    // (`advanced.rs`, `invalidates_followers`).
    //
    // Found on live run `civvis-20260801T141601Z`: the mirror check reported
    // `IMPROVEMENT_BARBARIAN_CAMP` among "improvements CIVVIS does not model", which
    // contradicted the ruleset. Hoisting the lookup costs one allocation per rebuild
    // and cannot drift again.
    let modelled_improvements: std::collections::BTreeSet<String> = game
        .rules
        .improvements
        .keys()
        .map(|name| name.as_str().to_string())
        .collect();
    let mut unknown_positions = std::collections::BTreeSet::new();
    for y in 0..height {
        for x in 0..width {
            let pos = crate::hex::offset_to_axial(x, y);
            let Some(tile) = game.map.tiles.get_mut(&pos) else {
                continue;
            };
            let Some(plot) = snapshot.plot((x, y)) else {
                unknown_positions.insert(pos);
                // Keep `assumed_traversable`: `grow_frontier` owns that planning
                // prior and recalculates it after this pass. Everything else came
                // from the generated placeholder world and must be erased.
                tile.terrain = unknown;
                tile.hills = false;
                tile.feature = None;
                tile.resource = None;
                tile.improvement = None;
                tile.pillaged = false;
                tile.district = None;
                tile.district_foundation = None;
                tile.wonder = None;
                tile.owner_city = None;
                tile.road = 0;
                tile.continent = None;
                tile.coastal_lowland = 0;
                tile.flooded = false;
                tile.submerged = false;
                tile.drought = false;
                tile.storm = None;
                tile.fallout_until = 0;
                tile.disaster_faith = 0.0;
                tile.disaster_food = 0.0;
                tile.disaster_production = 0.0;
                continue;
            };
            tile.assumed_traversable = false;
            tile.assumed_navigable = false;
            let resolved = plot
                .t
                .as_deref()
                .and_then(|name| match vocab.terrain(name) {
                    Resolved::Known(value) => Some(value),
                    Resolved::Excluded(_) | Resolved::Unknown(_) => None,
                });
            let (terrain, hills) = resolved.unwrap_or((unknown, false));
            tile.terrain = terrain;
            tile.hills = hills;
            tile.feature = plot.f.as_ref().and_then(|name| match vocab.feature(name) {
                Resolved::Known(value) => Some(value),
                _ => None,
            });
            tile.resource = plot
                .r
                .as_ref()
                .and_then(|name| match vocab.resource(name) {
                    Resolved::Known(value) => Some(value),
                    _ => None,
                })
                .filter(|resource| visible_resources.contains(resource));
            // ★★★ WHAT IS ALREADY IMPROVED. An unimproved-looking world makes CIVVIS
            // order builders forever: 19 of them for one city in one measured run.
            // ⚠ Mapped by name with no vocabulary, so a Civ 6 improvement CIVVIS does
            // not know becomes None rather than a wrong improvement — the tile then
            // reads unimproved, which is the honest direction for a name we cannot
            // translate.
            tile.improvement = plot.im.as_ref().and_then(|name| {
                let short = civvis_improvement_name(name);
                if modelled_improvements.contains(&short) {
                    Some(Name::new(&short))
                } else {
                    None
                }
            });
            // The host's pillage bit rides only with an improvement; a district's
            // pillage is set from the city record and must not be overwritten
            // here, so a plot without a modelled improvement is left alone.
            if tile.improvement.is_some() {
                tile.pillaged = plot.p;
            }
            // A National Park is not an improvement in Civilization VI — the
            // plot answers `IsNationalPark` and `GetImprovementType` -1 — and
            // it IS one on this board (`national_park` in the improvement
            // rules, read by `established_national_parks`), so the flag lands
            // as the improvement and the Amenities, the Tourism and the
            // unworkable ground follow.
            if plot.np {
                tile.improvement = Some(Name::new("national_park"));
                tile.pillaged = false;
            }
            // The host's road, on the engine's own ladder. An older export
            // carries no `rt` and reads 0, exactly what the mirror wrote before.
            tile.road = route_level(plot.rt.as_deref(), plot.rp);
            // ★★★★ AND NO SIMULATED WEATHER ON MIRRORED GROUND.
            //
            // The same defect as `apply_rivers`, `apply_landmass` and the
            // cliffs, one group of fields over: a field this pass does not
            // write keeps whatever `Game::new`'s generated world put there.
            // These eight are the engine's own disaster bookkeeping, and the
            // export carries none of them — the host has no accessor for the
            // fertility on a plot, and its flood, drought and fallout state
            // reach the board through `Plot:GetYield` instead (a flooded or
            // irradiated plot reads zero there, which is exactly what it pays).
            // So the honest value is nothing at all: a modelled eruption on a
            // mirrored board would be invented weather, and it would now be
            // counted TWICE, once as the model's own fertility and once inside
            // the host correction derived against it. Cleared unconditionally,
            // like `Tile::continent` above, because the point is to lose the
            // generated value rather than to keep it.
            tile.flooded = false;
            tile.submerged = false;
            tile.drought = false;
            tile.storm = None;
            tile.fallout_until = 0;
            tile.disaster_faith = 0.0;
            tile.disaster_food = 0.0;
            tile.disaster_production = 0.0;
        }
    }
    // `place_city` initially claims its complete first ring. When part of that
    // ring is undisclosed, clearing `owner_city` above must clear the city's
    // reverse index too or the two halves of ownership disagree.
    for city in game.cities.values_mut() {
        city.owned_tiles
            .retain(|pos| !unknown_positions.contains(pos));
    }
    // ★★★★ THE HOST'S BARBARIAN CAMPS REACH THE BOARD'S CAMP REGISTER.
    //
    // The tile above carries `barbarian_camp` as an improvement, and that is all
    // it carried: `game.barb_camps` — the register the engine's own barbarian
    // seat fills when it plants a camp — stayed EMPTY on every mirrored board.
    // Four readers depend on it and every one of them read nothing: the home
    // guard's threat list ranks a camp within `HOME_THREAT_RADIUS` "just under
    // a live raider" and sends a unit to clear it (`ai.rs`, the local-threat
    // scan); `barbarian_presence_at_home` counts a camp near home as a reason
    // to treat the barbarian seat as an enemy; `settlement_tile_risk` prices a
    // visible camp within three tiles of a site; `defensibility` discounts a
    // site by its distance to the nearest camp. Run civvis-20260816T155856Z:
    // two camps SEVEN tiles from Rome for the whole game, upgrading warriors
    // into musketmen; 121 attacks on the raiders they sent, not one on either
    // camp; eight of fourteen Settlers captured; five cities at turn 147.
    // Rebuilt from the tiles on every apply — a camp the host cleared is gone
    // on the next export, and the value is the turn it was seen, which is what
    // the engine's own register holds.
    let camps: Vec<crate::Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| tile.improvement.as_deref() == Some("barbarian_camp"))
        .map(|(pos, _)| *pos)
        .collect();
    game.barb_camps.clear();
    for camp in camps {
        game.barb_camps.insert(camp, game.turn);
    }
    // Rivers before the memory below, so what the seat remembers is the mirrored
    // network and not the generated one.
    apply_rivers(game, snapshot);
    apply_landmass(game, snapshot);
    // Called from here, and never separately, because a map and an explored set that
    // disagree is the defect this pair repairs — see `apply_explored`.
    apply_explored(game, snapshot);
    // And what that ground LOOKED like, which is a different set — see
    // `apply_tile_memory`. Explored ground with no memory is not drawn dim, it is
    // not drawn at all.
    apply_tile_memory(game, snapshot);
}

/// Tell the seat which ground it has actually seen.
///
/// ★★★★★ **NOTHING IN THIS BRIDGE EVER WROTE `explored`, SO IT DESCRIBED A PLACE THE
/// SEAT HAS NEVER BEEN.**
///
/// It was not empty, which is why this went unnoticed — and I asserted "empty" before
/// a test corrected me. `rebuild_game` starts from `Game::new`, which generates an
/// ordinary CIVVIS map and reveals a start position on it, leaving **35 explored plots
/// around a capital that has nothing to do with the real Civilization VI seat**. The
/// set was therefore populated, plausible, and pure fiction.
///
/// What that costs: `BasicAi`'s explore step walks outward in rings and takes the
/// nearest tile that is `!explored` and traversable (`ai.rs`, `has_exploration_target`
/// and the ring search beside it). Real ground the seat HAS seen is almost never inside
/// the generated map's 35, so it reads as unexplored — the unit steps onto a tile it has
/// already stood on, and next turn that tile is still "unexplored" because nothing here
/// ever recorded otherwise. That is the three-tile shuffle the livelock detector
/// reports: a scout, three archers and a heavy chariot moving every single turn and
/// uncovering nothing, invisible to both the idle fraction and the frozen count
/// *because the unit does move*.
///
/// The same fiction misdirects every tile purchase: `Game::plot_purchase_cost` requires
/// `explored.contains(&pos)`, so it quotes prices for ground the seat cannot see and
/// refuses the ground it can.
///
/// The snapshot has carried the answer the whole time — `Snapshot::revealed` is exactly
/// the set of plots the mod has exported for this seat, and the mod exports a plot only
/// once the seat can see it.
///
/// ⚠ This **replaces** rather than extends. Extending would keep the generated map's 35
/// invented plots forever, and they are the whole defect. Replacing is still monotonic
/// in practice because `Snapshot::revealed` itself only ever accumulates, so a plot seen
/// on turn 40 is still explored on turn 200 even when the seat cannot currently see it —
/// which is what `explored` means, as distinct from `is_revealed` at this instant.
/// Idempotent.
/// Put the seat's strategic stockpiles on the board.
///
/// ★★★★ THE BOARD HAD NO STRATEGIC RESOURCES AT ALL. Nothing exported them, so
/// `Game::strategic_stockpile` answered 0 for every resource on every live turn:
/// `can_produce` refused every Swordsman, Knight, Musketman and Bombard, the
/// armies were the resource-free units (AT crews, pike-and-shot, chariots — see
/// the production histograms of every live run), and `unit_is_obsolete` never
/// retired a predecessor whose successor costs a resource. Measured on the won
/// game civvis-20260816T054344Z: a Trebuchet ordered on 29 turns across eight
/// cities, each `civvis_build_unplayable` — the host had Niter and a Bombard,
/// the board had neither. Translated from the host's `RESOURCE_X` to CIVVIS's
/// `x`; a resource the ruleset does not know is reported, not dropped.
pub(crate) fn apply_strategic_stockpiles(
    game: &mut crate::game::Game,
    state: &StateSnapshot,
    unmapped: &mut Vec<String>,
) {
    let Some(seat) = game.players.get_mut(0) else {
        return;
    };
    seat.strategic_resources.clear();
    let Some(stock) = state.strategic_resources.as_ref() else {
        return;
    };
    for (host, amount) in stock {
        let name = host
            .strip_prefix("RESOURCE_")
            .unwrap_or(host)
            .to_ascii_lowercase();
        if game.rules.resources.contains_key(&Name::new(&name)) {
            seat.strategic_resources
                .insert(Name::new(&name), amount.max(0.0));
        } else {
            let issue = format!("strategic_resource:{host}");
            if !unmapped.contains(&issue) {
                unmapped.push(issue);
            }
        }
    }
}

pub(crate) fn apply_explored(game: &mut crate::game::Game, snapshot: &Snapshot) {
    // The seat is player 0 throughout this bridge: `plant_unit(&mut game, 0, …)` for
    // our own units, `game.players[0]` for our civics and techs.
    let Some(seat) = game.players.get_mut(0) else {
        return;
    };
    seat.explored = snapshot
        .revealed_positions()
        .map(|(x, y)| crate::hex::offset_to_axial(x, y))
        .collect();
}

/// Put Civilization VI's rivers on the board, and take the invented ones off it.
///
/// ★★★★★ **THE BOARD WAS NEVER RIVER-LESS, WHICH IS WHY THIS SURVIVED.** Nothing in
/// this bridge ever wrote a river — `grep -c river src/mirror.rs` answered **0** — and
/// the honest conclusion from that alone would have been "the mirror has no rivers".
/// It had 33 of them. `rebuild_game` starts from `Game::new`, which generates an
/// ordinary CIVVIS map complete with a river network, and `apply_terrain` overwrites
/// terrain, feature, resource and improvement while leaving `river_edges` untouched.
/// The generated world's rivers therefore showed through on every mirrored game ever
/// played, in places the real game has no river at all.
///
/// The same trap as `explored` (35 invented plots) and `remembered_tiles` (33 tiles of
/// a different world): **a populated field is not a mirrored one**, and the check that
/// would have caught it is agreement against the export, never "is it non-empty".
///
/// **The control that settles it.** A Civilization VI river plot is fresh water *by
/// definition*, and `fw` (`IsFreshWater`) has been exported all along. Measured on the
/// live run `civvis-20260731T235836Z` at turn 112, before this fix:
///
/// | | |
/// |---|---|
/// | revealed plots paired with the board | 513 |
/// | plots Civilization VI calls fresh water | 132 (25.7%) |
/// | CIVVIS tiles carrying a river | 33 |
/// | ...of those, fresh water in the export | **12 (36.4%)** |
///
/// 36.4% against a 25.7% base rate is chance. Real rivers would read ~100%.
///
/// ⚠ **Clears before it writes.** `WorldMap::clear_rivers` runs first for the same
/// reason `apply_explored` replaces rather than extends: leaving the generated network
/// in place and adding to it keeps every invented river forever, and they are the whole
/// defect.
///
/// ## The edge mapping
///
/// Civilization VI holds a river on three of a plot's six edges — W, NW, NE — and the
/// other three are the same segments seen from the neighbours, so exporting three per
/// plot carries the whole network. In this reconstruction `r` **is** Civilization VI's
/// `y`, and Civ 6's `y` grows north, so the axial directions in [`crate::hex::DIRS`]
/// read as E, SE, SW, W, NW, NE and the three flags land on indices 3, 4 and 5.
///
/// ⚠ Written through `set_river_edge`, which sets the reciprocal edge on the neighbour
/// too, so the two tiles cannot disagree about a segment they share. A river whose far
/// side is not revealed is simply not written — that is the part of the network the
/// seat has not seen, and inventing it is what this function exists to stop.
pub(crate) fn apply_rivers(game: &mut crate::game::Game, snapshot: &Snapshot) {
    // ⚠⚠ `IsWOfRiver` MEANS "THIS PLOT IS WEST **OF** THE RIVER" — the river is on the
    // plot's EAST edge. Not "there is a river on the west edge", which is how this was
    // first written, and the two put the segment on opposite sides of the hex.
    //
    // Caught on live run civvis-20260801T011451Z at turn 1, by the one check that can
    // see it: a Civilization VI river plot is fresh water by definition, so every tile
    // CIVVIS calls riverside must be `fw` in the export. Four were not — (18,21)
    // through (18,24), a contiguous column, all reporting `rv = 0`, `ri = false`,
    // `fw = false`. Civilization VI had no river there at all.
    //
    // The export said where it really was. The river runs down the x=19/20 boundary:
    //
    //     (19,22) rv = 1 (W)      and Civ 6 marks (20,22) riverside — its EAST neighbour
    //     (19,25) rv = 4 (NE)     and Civ 6 marks (19,24) riverside — its SOUTH-WEST one
    //     (20,22) rv = 4 (NE)     and Civ 6 marks (19,21) riverside — likewise
    //
    // So each flag names the plot's own position relative to the river, and the edge
    // carrying it is the OPPOSITE one:
    //
    //     IsWOfRiver  -> river on the EAST edge        DIRS[0]
    //     IsNWOfRiver -> river on the SOUTH-EAST edge  DIRS[1]
    //     IsNEOfRiver -> river on the SOUTH-WEST edge  DIRS[2]
    //
    // ⚠ Why the original passed its unit test anyway: `set_river_edge` marks BOTH
    // tiles sharing the segment, so the plot that reported the flag came out riverside
    // either way. Only the NEIGHBOUR differed — the segment sat on the wrong side of
    // the hex, which is invisible to "is this tile riverside" and decides every river
    // crossing and every district's river adjacency. A fixture that exports one plot
    // and checks that plot cannot see it; the test below now pins the neighbour.
    //
    // Directions in this reconstruction's frame, where `r` IS Civilization VI's `y`
    // and Civ 6's `y` grows north: DIRS[0] E, [1] SE, [2] SW, [3] W, [4] NW, [5] NE.
    game.map.clear_rivers();
    for (x, y) in snapshot.revealed_positions() {
        let Some(plot) = snapshot.plot((x, y)) else {
            continue;
        };
        let pos = crate::hex::offset_to_axial(x, y);
        // `ri` is `Plot:IsRiver()`, the host's own answer to "is this plot riverside".
        // `rv` carries the six edges, but a segment whose Firaxis holder is an
        // unrevealed neighbour reads back as 0 here while the plot is riverside — the
        // Lua says so where it writes `ri` — and until 2026-08-26 nothing read the
        // bit, so housing, fresh water and river adjacency on such a plot were all
        // dry. The flag marks the tile riverside without inventing a crossing.
        if plot.ri {
            if let Some(tile) = game.map.tiles.get_mut(&pos) {
                tile.riverside = true;
            }
        }
        if plot.rv == 0 {
            continue;
        }
        // Bits 8/16/32 carry W/NW/NE edges read from the neighbouring Firaxis
        // holders. The exporter includes them even when that neighbour is hidden:
        // the segment on this revealed plot is itself known. North-up staging can
        // also map any of the six directions onto any other one.
        for (bit, direction) in [(1u8, 0usize), (2, 1), (4, 2), (8, 3), (16, 4), (32, 5)] {
            if plot.rv & bit == 0 {
                continue;
            }
            let delta = crate::hex::DIRS[direction];
            let neighbour = (pos.0 + delta.0, pos.1 + delta.1);
            if !game.map.set_river_edge(pos, neighbour, true) {
                // A river edge on a revealed boundary tile is visible even when
                // the tile across it is not. Preserve that one-sided boundary
                // fact instead of turning a known riverside plot into dry land.
                if let Some(tile) = game.map.tiles.get_mut(&pos) {
                    tile.river_edges[direction] = true;
                }
            }
        }
    }
}

/// Which landmass each plot belongs to, how low it lies — and no invented cliffs.
///
/// The third sweep of the same defect as [`apply_rivers`]: a field `apply_terrain` does
/// not write keeps whatever the map `Game::new` generated put there. Measured on the
/// live run `civvis-20260731T235836Z` at turn 207, against a board of 776 tiles:
///
/// | field | populated | by |
/// |---|---|---|
/// | `continent` | 200 | the generator |
/// | `cliff_edges` | 66 | the generator |
/// | `coastal_lowland` | 24 | the generator |
///
/// ⚠ `continent` was not merely wrong, it was **incoherent**: 200 tiles carried a
/// region and 576 carried none, on a board where every land plot has a continent in the
/// real game. "Another continent" is load-bearing in this ruleset, so a seat that
/// cannot tell one landmass from another cannot reason about overseas settling or
/// invasion at all.
///
/// ## Continent indices are assigned, not translated
///
/// `Tile::continent` is a zero-based region id whose only job is to say *same landmass
/// or not*. Civilization VI names them (`CONTINENT_AFRICA`), so the names present in
/// the snapshot are sorted and numbered. Sorting rather than first-seen order keeps the
/// assignment a pure function of the snapshot, so two syncs over the same revealed set
/// agree. ⚠ A newly revealed continent can therefore renumber the others — that is
/// safe here because every consumer compares ids **within one reconstruction**, and the
/// whole map is rebuilt from the snapshot on each sync anyway.
///
/// ## Cliffs are cleared, because they cannot be mirrored
///
/// ⚠ **There is no gameplay Lua accessor for cliffs.** `IsCliff` exists in this
/// install only inside art definitions (`<m_ParamName text="IsCliff"/>`), never as a
/// plot method, so unlike rivers there is nothing to export.
///
/// That leaves a choice between two fictions, and it is the same one `apply_terrain`
/// faced over unseen ground: **the expensive fiction is the one that stops movement.**
/// `Game::crosses_cliff` fires only on a land/water boundary and
/// `unit_can_cross_cliff` gates movement on it, so an invented cliff **blocks
/// embarkation** at a shoreline the real game lets a unit leave from. That is the
/// precise shape of a failure already on the books — a seat whose world ends at the
/// water, with `met` stuck at zero. A missing cliff, by contrast, only means CIVVIS may
/// plan a crossing Civilization VI refuses, and the mod honours refusal, so it costs a
/// turn rather than a permanently walled-in empire.
///
/// So they are cleared, and this comment is the record that it is a known gap rather
/// than an oversight. If a cliff accessor ever appears, mirror them the way rivers are
/// mirrored and delete this paragraph.
pub(crate) fn apply_landmass(game: &mut crate::game::Game, snapshot: &Snapshot) {
    // Sorted so the numbering is a function of the snapshot and nothing else.
    let mut names: Vec<&str> = snapshot
        .revealed_positions()
        .filter_map(|pos| snapshot.plot(pos)?.ct.as_deref())
        .collect();
    names.sort_unstable();
    names.dedup();
    let index_of: BTreeMap<&str, usize> = names
        .iter()
        .enumerate()
        .map(|(index, name)| (*name, index))
        .collect();

    // Rebuilt from the plots on every apply, like the landmass: a plot whose
    // reading lapsed keeps none.
    Arc::make_mut(&mut game.observed_appeal).clear();
    Arc::make_mut(&mut game.observed_fresh_water).clear();
    for (x, y) in snapshot.revealed_positions() {
        let Some(plot) = snapshot.plot((x, y)) else {
            continue;
        };
        let pos = crate::hex::offset_to_axial(x, y);
        let Some(tile) = game.map.tiles.get_mut(&pos) else {
            continue;
        };
        // ⚠ Assigned unconditionally, including the `None` case. A plot the export says
        // has no continent must LOSE the generated one rather than keep it, which is
        // the whole defect.
        tile.continent = plot
            .ct
            .as_deref()
            .and_then(|name| index_of.get(name).copied());
        tile.coastal_lowland = plot.cl.clamp(0, 3) as u8;
        // The host's own appeal, which `Game::tile_appeal` prefers to its
        // derivation from the six neighbours it can see.
        if let Some(appeal) = plot.ap {
            Arc::make_mut(&mut game.observed_appeal).insert(pos, appeal);
        }
        // The host's fresh-water answer, which `Game::city_water` prefers.
        if let Some(fresh) = plot.fw {
            Arc::make_mut(&mut game.observed_fresh_water).insert(pos, fresh);
        }
    }

    // Nothing can mirror these, so nothing may keep them. See above.
    for tile in game.map.tiles.values_mut() {
        tile.cliff_edges = [false; 6];
    }
}

/// Tell the seat which cities it has seen, so fog does not erase them.
///
/// ★★★★★ **THE SAME DEFECT AS `apply_tile_memory`, ONE FIELD OVER, AND I ONLY FIXED
/// THE TILES.** The operator's report was *"civvis sometimes only shows current
/// visibility"*; that was true of ground and it is equally true of cities.
///
/// `obs.rs` includes a city in a seated observation when it is currently visible, or
/// when the seat REMEMBERS it:
///
/// ```ignore
/// for memory in g.players[*viewer].remembered_cities.values() {
///     if explored.contains(&memory.pos) && !vis.contains(&memory.pos) { … }
/// ```
///
/// `grep -c remembered_cities src/mirror.rs` answered **0**, so every enemy city
/// outside the seat's current sight vanished from the board it is shown and reasons
/// over.
///
/// Measured on live run `civvis-20260801T045406Z` at turn 198, at war and losing:
///
/// | | |
/// |---|---|
/// | enemy cities in the export | **7** |
/// | …revealed, on land, unoccupied | **7** |
/// | placed on the reconstruction | **7** (`follow.log`: "7 rival cities") |
/// | **visible in the seated observation** | **1** |
///
/// ⚠ That last row is the one that matters strategically: `findWarTarget` needs a
/// revealed rival city, and this project has already recorded "no enemy city is ever
/// revealed … domination is arithmetically impossible" as a standing blocker. The
/// cities were on the board the whole time; the seat could not remember them.
///
/// Built through `Game::remember_city` rather than assembled here, so the bridge's
/// memory has exactly the shape the engine's own fog bookkeeping produces and the two
/// cannot drift.
///
/// ⚠ Replaces rather than extends, for the same reason [`apply_explored`] does:
/// `Game::new` generates a world with cities of its own, and a memory of THOSE is a
/// memory of somewhere the real seat has never been.
pub(crate) fn apply_city_memory(game: &mut crate::game::Game) {
    let turn = game.turn.max(1);
    let seen: Vec<(u32, crate::game::RememberedCity)> = game
        .cities
        .values()
        .map(|city| {
            let mut memory = game.remember_city(city);
            memory.seen_turn = turn;
            (city.id, memory)
        })
        .collect();
    let Some(seat) = game.players.get_mut(0) else {
        return;
    };
    seat.remembered_cities.clear();
    for (id, memory) in seen {
        seat.remembered_cities.insert(id, memory);
    }
}

/// Tell the seat what the ground it has seen actually LOOKED like.
///
/// ★★★★★ **THE SEAT REMEMBERED NOTHING, SO THE MIRROR DREW ONLY WHAT IT COULD SEE THIS
/// INSTANT** — a live vision cone beside a Civilization VI screen showing a continent.
/// The operator reported it as *"civvis sometimes only shows current visibility… you can
/// see from the map now that area has been explored that isn't in civvis map."*
///
/// `explored` and `remembered_tiles` are two different sets and this bridge only ever
/// wrote the first. `grep -c remembered_tiles src/mirror.rs` answered **0**.
///
/// ⚠⚠ **AND THE MEMORY WAS NOT EMPTY — IT WAS A DIFFERENT WORLD.** Exactly the trap
/// [[civvis-civ6-explored-was-a-fiction]] describes, one field over. Measured by
/// disabling this function on the test below: the seat held **33 remembered tiles**
/// against **6 charted plots**, because `Game::new` generates a CIVVIS map and reveals a
/// start on it. Overlap: **2**. So the board rendered **2 of 6** charted plots, and both
/// survivors were coincidences of the generated map rather than ground the real
/// Civilization VI seat had ever stood on. Every "is the memory populated" check said
/// yes, which is why this outlived the `explored` fix that shipped beside it.
///
/// What that costs: `obs.rs` builds the board a *seated* viewer receives by walking
/// `explored`, and for a tile that is not currently visible it reads the freshest
/// viewer's memory of it —
///
/// ```ignore
/// let tiles: Vec<Value> = explored.iter().filter_map(|pos| {
///     let live = omniscient || vis.contains(pos);
///     let (tile, owner) = if live { … } else {
///         let (memory, _) = viewers.iter().filter_map(…).max_by_key(…)?;  // <-- HERE
///         (&memory.tile, memory.owner)
///     };
/// ```
///
/// That `?` sits inside a `filter_map`, so a tile with no memory is **dropped from the
/// board entirely** — not fogged, not stale, absent. The mirror window attaches with
/// `POST /view {"player": 0}`, so it is exactly the seated, non-omniscient path, and
/// every explored plot outside the seat's current sight radius disappeared from it.
///
/// ⚠ This is a *display and knowledge* defect, not only a cosmetic one: the same
/// observation feeds anything reading the board through `obs`, so ground the seat has
/// charted reads as ground nobody has ever been to.
///
/// ⚠ **Replaces, for the same reason [`apply_explored`] replaces.** `rebuild_game`
/// starts from `Game::new`, which generates an ordinary CIVVIS map and reveals a start
/// on it; any memory of *that* world describes a place the real seat has never been
/// ([[civvis-civ6-explored-was-a-fiction]]). Rebuilding from the snapshot keeps the
/// invariant that matters — **memory is exactly the explored set, never more and never
/// less** — so the two can no longer disagree. It stays monotonic in practice because
/// `Snapshot::revealed` only ever accumulates.
///
/// ⚠ The frontier is deliberately excluded: this keys strictly off
/// `Snapshot::revealed_positions`, so a provisionally traversable unknown is never
/// remembered as though the seat had seen it.
///
/// Idempotent.
pub(crate) fn apply_tile_memory(game: &mut crate::game::Game, snapshot: &Snapshot) {
    let turn = snapshot.turn.max(1);
    // Decided first, applied second: reading the map and the owning city needs `game`
    // immutably while writing the seat's memory needs it mutably.
    let remembered: Vec<(crate::Pos, crate::world::RememberedTile)> = snapshot
        .revealed_positions()
        .filter_map(|(x, y)| {
            let pos = crate::hex::offset_to_axial(x, y);
            let tile = game.map.get(pos)?;
            // The same derivation the live branch of `obs.rs` uses, so a tile does not
            // change owner merely by passing under fog. `apply_territory` has already
            // written `owner_city` from Civilization VI's own `GetOwner` on the rebuild
            // path; where it has not run, this is `None`, which is what the live branch
            // would report for the same tile.
            let owner = tile
                .owner_city
                .and_then(|city| game.cities.get(&city))
                .map(|city| city.owner);
            Some((
                pos,
                crate::world::RememberedTile {
                    tile: tile.clone(),
                    owner,
                    seen_turn: turn,
                },
            ))
        })
        .collect();
    let Some(seat) = game.players.get_mut(0) else {
        return;
    };
    seat.remembered_tiles.forget_all();
    for (pos, memory) in remembered {
        seat.remembered_tiles.remember(pos, memory, turn);
    }
}

/// Rebuild the map AND place the seat's cities on it.
///
/// ⚠ Without the cities, every score that reads spacing or owned territory is
/// evaluated against an empty world. Measured consequence: CIVVIS's settle ranking
/// put its best site two tiles from the real capital, which Civilization VI
/// forbids outright (`CITY_MIN_RANGE` is 3). A comparison drawn from that would
/// have blamed the agent for a gap this reconstruction created.
pub fn rebuild_with_empire(
    snapshot: &Snapshot,
    cities: &[(i32, i32)],
    players: usize,
    seed: u64,
) -> (crate::game::Game, usize) {
    let mut game = rebuild_game(snapshot, players, seed);
    let mut placed = 0;
    for offset in cities {
        // `cities` arrive in Civ 6 offset coordinates, like the plots.
        if !snapshot.is_revealed(*offset) {
            continue;
        }
        let pos = crate::hex::offset_to_axial(offset.0, offset.1);
        let is_water = game
            .map
            .get(pos)
            .map(|tile| game.rules.is_water(tile))
            .unwrap_or(true);
        if is_water {
            continue;
        }
        game.place_city(0, pos, None);
        placed += 1;
    }
    (game, placed)
}

// ============================================================ the live empire
//
// ★★★★★ WHY THIS EXISTS. `rebuild_game` gives CIVVIS terrain and
// `rebuild_with_empire` adds the seat's cities. Neither gives it UNITS, and
// neither gives it a RIVAL — so CIVVIS could not answer the two questions that
// decide a game on Settler: whom to fight, and where to send the army. Those were
// answered instead by hand-written Lua, which is how a veto comparing SCORE ratios
// forbade every war for 190 turns while nineteen units stood on ALERT.
//
// ⚠ Civilization VI speaks OFFSET, CIVVIS stores AXIAL. Every crossing here goes
// through `hex::offset_to_axial`, because mixing them is silent: a capital at
// offset (56,28) landed on NO TILE and the ranker then blamed the map.

/// Accept a production field that is EITHER a type name or Civilization VI's raw hash.
///
/// ⚠⚠ THIS GUARD IS NOT HYPOTHETICAL — its absence was measured. Typing `producing`
/// as `Option<String>` made serde reject every state event carrying a city, because
/// runs recorded before the mod resolved the hash carry `producing: -1743686858`, a
/// NUMBER. `state_from_events` skips a state it cannot parse, silently, so the whole
/// mirror fell back to the newest state that happened to have no cities in it — turn
/// 3 of a 233-turn game, reported as an empty board with no error anywhere.
///
/// Any older run, and any run from a mod build that predates the fix, must keep
/// working: a schema change that silently invalidates recorded history is worse than
/// the missing field it was added for.
fn name_or_nothing<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = serde_json::Value::deserialize(deserializer)?;
    Ok(match value {
        serde_json::Value::String(text) if !text.trim().is_empty() => Some(text),
        // A bare hash is not a name and cannot be turned back into one here; it is
        // the same as knowing nothing, which is what the field meant before.
        _ => None,
    })
}

/// One district a city has placed, in OFFSET coordinates like everything the
/// export sends.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct StateDistrict {
    /// The Civilization VI type name, e.g. `DISTRICT_CAMPUS`.
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    #[serde(default)]
    pub pillaged: bool,
    /// False while Firaxis has placed the district but construction is unfinished.
    /// Historical events lack this field and treated every placement as complete.
    #[serde(default = "district_complete_default")]
    pub complete: bool,
    /// The district's hit points, as Firaxis reports them: DAMAGE taken against a
    /// maximum, for the garrison and for the outer defenses separately.
    ///
    /// ★★★★★ THE FIELD THAT DECIDED 121 TURNS OF PRODUCTION. `pillaged` is a
    /// boolean and a district can be damaged without being pillaged, so it never
    /// stood in for health. Nothing carried hit points, nothing set
    /// [`crate::game::City::encampment_hp`], and it defaults to **0**.
    ///
    /// `Game::can_produce` gates `repair_encampment` on `encampment_hp < 100`, so
    /// on every mirrored board that test passed for any city holding an
    /// Encampment, permanently. The AI queued the repair every turn,
    /// `civvis_orders` correctly refuses to translate a project Civilization VI
    /// does not have, the order was discarded — and nothing else was ordered for
    /// that city, so its queue stayed empty.
    ///
    /// Measured on live run `civvis-20260810T040916Z` (Rome/Trajan, Settler,
    /// Online): Ravenna and Lugdunum, exactly the two cities holding an
    /// Encampment, sat at `producing_hash 0, cost -1, progress -1` from turn 67 to
    /// turn 188 with production yields of 8 and 9 against Rome's 28. 238 discarded
    /// orders — **10.4% of every order CIVVIS issued that game** — and
    /// `ENDTURN_BLOCKING_PRODUCTION` was the run's dominant blocker because two
    /// queues were permanently empty.
    ///
    /// ⚠ AND THE RECORDED FIX WOULD HAVE MADE IT WORSE. The standing plan was to
    /// map `repair_encampment` onto a district BUILD. Both Encampments export as
    /// `pillaged: false, complete: true` — undamaged — so that mapping would have
    /// rebuilt two healthy districts from scratch. The missing translation was
    /// never the bug.
    ///
    /// `-1` means the host did not answer, which must not read as "destroyed".
    #[serde(default = "unknown_damage")]
    pub damage: i32,
    #[serde(default = "unknown_damage")]
    pub max_damage: i32,
    #[serde(default = "unknown_damage")]
    pub wall_damage: i32,
    #[serde(default = "unknown_damage")]
    pub max_wall_damage: i32,
}

/// Firaxis did not answer. Distinct from a real 0, which means "no damage taken".
fn unknown_damage() -> i32 {
    -1
}

fn district_complete_default() -> bool {
    true
}

impl Default for StateDistrict {
    fn default() -> Self {
        Self {
            kind: String::new(),
            x: 0,
            y: 0,
            pillaged: false,
            complete: true,
            damage: unknown_damage(),
            max_damage: unknown_damage(),
            wall_damage: unknown_damage(),
            max_wall_damage: unknown_damage(),
        }
    }
}

/// One completed world wonder and the plot Firaxis built it on.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateWonder {
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
}

/// Routes arriving at one city, by origin: foreign (another player's) and
/// domestic (this seat's own).
#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize)]
pub struct StateIncomingRoutes {
    #[serde(default)]
    pub foreign: i64,
    #[serde(default)]
    pub domestic: i64,
    /// Where each foreign route comes from: the origin city's OFFSET
    /// coordinates and its host player id. With these the mirror seats the
    /// route on the board (`game.routes`, owner = that player's seat) instead
    /// of only counting it, so every destination-side rule that reads the
    /// route list — Zhang Qian's Gold, alliance yields, the World Congress
    /// Trade Policy — sees what the host sees. Empty on an older export.
    #[serde(default)]
    pub origins: Vec<StateRouteOrigin>,
}

/// One foreign route's origin, as `incoming_routes.origins[]` carries it.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Deserialize)]
pub struct StateRouteOrigin {
    #[serde(default = "minus_one")]
    pub x: i32,
    #[serde(default = "minus_one")]
    pub y: i32,
    #[serde(default = "minus_one")]
    pub player: i32,
}

/// One World Congress resolution currently in effect, as the host reports it:
/// the `WC_RES_*` type, which option won (1 = A, 2 = B, 0 = the mod could not
/// tell), and the chosen target verbatim (a host player id for PLAYER-targeted
/// resolutions, a `RESOURCE_*`/`DISTRICT_*`/... type name otherwise).
#[derive(Clone, Debug, Default, PartialEq, serde::Deserialize)]
pub struct StateResolution {
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub option: i64,
    #[serde(default)]
    pub target: String,
}

/// One city-state request, as the host's QuestsManager reports it. `type` is
/// the `Quests.QuestType` row (`QUEST_TRAIN_UNIT_TYPE`, …); `target` is the
/// type the quest names, recovered by the mod from the localized description
/// (the host exposes no target accessor), `None` for the three quests that
/// name nothing and where no name matched.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateQuest {
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub target: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
}

/// One major's Envoy count at a city-state, by host player id.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateEnvoyCount {
    #[serde(default = "minus_one_i64")]
    pub player: i64,
    #[serde(default)]
    pub envoys: i64,
}

/// The host's climate, as `GameClimate` answers the shipped ClimateScreen.
/// `level` is `GetClimateChangeLevel` (0–7), -1 when it could not be read;
/// every other field is `None` where its accessor was missing.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateClimate {
    #[serde(default = "minus_one_i64")]
    pub level: i64,
    #[serde(default)]
    pub temperature: Option<f64>,
    #[serde(default)]
    pub co2_total: Option<f64>,
    #[serde(default)]
    pub co2_ours: Option<f64>,
    #[serde(default)]
    pub sea_level_turns: Option<i64>,
    #[serde(default)]
    pub tiles_flooded: Option<i64>,
    #[serde(default)]
    pub storm_pct: Option<f64>,
    #[serde(default)]
    pub flood_pct: Option<f64>,
    #[serde(default)]
    pub drought_pct: Option<f64>,
}

/// One legal route from one of our cities, priced by the host's own
/// `CanStartRoute` and `CalculateOriginYield…` (TradeRouteChooser.lua:227,
/// :864). Endpoints carry coordinates because Firaxis city ids are only
/// unique per player.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateRouteOption {
    #[serde(default = "minus_one_i64")]
    pub origin: i64,
    #[serde(default = "minus_one")]
    pub origin_x: i32,
    #[serde(default = "minus_one")]
    pub origin_y: i32,
    #[serde(default = "minus_one_i64")]
    pub dest: i64,
    #[serde(default = "minus_one")]
    pub dest_player: i32,
    #[serde(default = "minus_one")]
    pub dest_x: i32,
    #[serde(default = "minus_one")]
    pub dest_y: i32,
    /// What the route would pay its origin per turn, summed as the
    /// active-route export sums it.
    #[serde(default)]
    pub yields: Option<crate::rules::Yields>,
}

/// One score row in Firaxis's World Congress emergency tracker.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateEmergencyScore {
    #[serde(default)]
    pub player: i64,
    #[serde(default)]
    pub score: f64,
    #[serde(default)]
    pub tier: Option<i64>,
}

/// The active seat's membership and score in an exported emergency.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateEmergencyOurs {
    #[serde(default)]
    pub member: bool,
    #[serde(default)]
    pub score: Option<f64>,
    #[serde(default)]
    pub tier: Option<i64>,
}

/// A World Congress emergency or scored competition as Firaxis's tracker
/// reports it. The mirror needs only the active member's score race; native
/// CIVVIS emergencies retain their separate city-capture representation.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateEmergency {
    #[serde(default, rename = "type")]
    pub kind: String,
    /// Firaxis's emergency recipient.  Aid Requests award score when this
    /// civilization receives a normal Gold deal, so the live bridge needs the
    /// host player id rather than merely the competition's leaderboard.
    #[serde(default = "minus_one_i64")]
    pub target: i64,
    /// Firaxis uses a negative value once the tracker entry is complete; zero
    /// remains an active final-turn opportunity.
    #[serde(default = "minus_one_i64")]
    pub turns_left: i64,
    #[serde(default)]
    pub begun: bool,
    #[serde(default)]
    pub scores: Vec<StateEmergencyScore>,
    #[serde(default)]
    pub ours: StateEmergencyOurs,
}

/// One population-worked Firaxis plot, in OFFSET coordinates.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateWorkedPlot {
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    /// The plot's own yields as the host computes them for its owner
    /// (`Plot:GetYield`), so a model-versus-host gap can be located to a tile
    /// rather than only sized per city. `None` on an export older than the
    /// field, or when the host could not read the plot.
    #[serde(default)]
    pub yields: Option<crate::rules::Yields>,
}

/// One Great Work in an exact Firaxis city slot.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateGreatWork {
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub object: String,
    #[serde(default)]
    pub era: Option<String>,
    #[serde(default)]
    pub creator: String,
    #[serde(default)]
    pub building: String,
    #[serde(default)]
    pub slot: i32,
}

/// One appointed Governor exactly as Civilization VI reports it.
///
/// `None` at the snapshot level means the Governor API was unavailable or the
/// event predates this contract. `Some([])` means the player has appointed none.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateGovernor {
    #[serde(default, rename = "type")]
    pub kind: String,
    #[serde(default = "minus_one_i64")]
    pub city: i64,
    #[serde(default = "minus_one")]
    pub city_player: i32,
    #[serde(default = "minus_one")]
    pub x: i32,
    #[serde(default = "minus_one")]
    pub y: i32,
    #[serde(default)]
    pub established: bool,
    #[serde(default)]
    pub turns_on_site: i32,
    #[serde(default)]
    pub turns_to_establish: i32,
    #[serde(default)]
    pub neutralized_turns: i32,
    #[serde(default)]
    pub promotions: Vec<String>,
}

/// One row of the host's production menu for a city (`StateCity::buildable`):
/// the Civilization VI type name, the engine's production cost and turns for
/// it in that city, the formation tier of a Corps/Army row, and for a district
/// the number of plots the engine offers with up to sixteen of them.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateMenuItem {
    #[serde(default)]
    pub t: String,
    #[serde(default = "unknown_metric")]
    pub c: f64,
    #[serde(default = "unknown_metric")]
    pub p: f64,
    #[serde(default)]
    pub f: Option<u8>,
    #[serde(default)]
    pub n: Option<i64>,
    #[serde(default)]
    pub s: Option<Vec<StateMenuPlot>>,
}

/// An offset-coordinate plot in a host menu row.
#[derive(Clone, Copy, Debug, Default, serde::Deserialize)]
pub struct StateMenuPlot {
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
}

/// One row of the host's purchase menu (`StateCity::purchasable`): the type
/// name and the engine's price in Gold and in Faith, each absent where the
/// host will not sell for that currency.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StatePurchaseItem {
    #[serde(default)]
    pub t: String,
    #[serde(default)]
    pub g: Option<f64>,
    #[serde(default)]
    pub f: Option<f64>,
}

/// One entry of the build queue BEHIND the head (`StateCity::queue`): the type
/// name, the formation tier of a Corps/Army, and the production already
/// invested in it.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateQueueItem {
    #[serde(default)]
    pub t: String,
    #[serde(default)]
    pub f: Option<u8>,
    #[serde(default)]
    pub pr: Option<f64>,
}

/// One city as Civilization VI reported it, in OFFSET coordinates.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateCity {
    #[serde(default)]
    pub id: i64,
    /// The name Civilization VI shows on the banner, e.g. `Pasargadae`.
    ///
    /// ⚠ Without this the reconstruction names cities from CIVVIS's own list for
    /// whatever civilization it happened to assign, so a Persian game showed
    /// ROME / OSTIA / ANTIUM and the two screens could not be compared at all.
    #[serde(default)]
    pub name: String,
    /// Civ 6 building type names this city has already finished.
    #[serde(default)]
    pub buildings: Vec<String>,
    /// The subset of `buildings` the host reports pillaged
    /// (`CityBuildings:IsPillaged`). A pillaged building pays nothing until
    /// repaired; without this the mirror paid Antium +6 Science on a raided
    /// Campus for twenty turns. `None` on an older export — an unknown, not
    /// an empty list.
    #[serde(default)]
    pub pillaged_buildings: Option<Vec<String>>,
    /// The religion this city actually follows, by Civ 6 type name, and the one
    /// converting it.
    ///
    /// ★★★★★ Null on all 26,954 city records ever exported before this — the same
    /// shape as `districts`: in the schema, never filled. A city can be converted
    /// away turn by turn and the mirror says nothing, so CIVVIS can neither pursue
    /// a religious victory nor defend against one. Two consecutive completed games
    /// were lost to the same victory type well before the turn limit while CIVVIS
    /// held hundreds of unspent faith.
    #[serde(default)]
    pub religion: Option<String>,
    /// The religion gaining on this city, if one is. `religion` alone is a level:
    /// a city holding steady and a city about to flip read identically — the same
    /// reason `loyalty_per_turn` exists beside `loyalty`.
    #[serde(default)]
    pub religion_next: Option<String>,
    /// Turns until `religion_next` takes the city, or -1 when nothing is.
    #[serde(default)]
    pub religion_turns: i64,
    /// Per-city copy of the active pantheon, retained as an export-contract
    /// diagnostic. CIVVIS applies the same belief through the owning player.
    #[serde(default)]
    pub pantheon_active: Option<String>,
    /// Districts this city has placed, each with the plot it sits on.
    ///
    /// ★★★★★ The plot is why this exists. `Item::District` carries a `pos`, so
    /// without one `civvis_production_item` had to refuse every district — and a
    /// city BUILDING a district then read as idle, so CIVVIS re-decided the same
    /// production every turn. Measured on run `civvis-20260731T163924Z`: 60
    /// `DISTRICT_GOVERNMENT` orders between t46 and t128, all `applied: true`, on a
    /// capital that still showed three buildings at t130.
    ///
    /// Empty when the export could not read the city's plots, which is the same
    /// "could not ask" the Lua side leaves nil — not an assertion that there are
    /// none.
    #[serde(default)]
    pub districts: Vec<StateDistrict>,
    #[serde(default)]
    pub wonders: Vec<StateWonder>,
    /// Exact citizen assignments. `None` means an older export or an unavailable
    /// Firaxis query; `Some([])` means this city works no non-centre plots/jobs.
    #[serde(default)]
    pub worked: Option<Vec<StateWorkedPlot>>,
    #[serde(default)]
    pub specialists: Option<Vec<String>>,
    /// Exact works housed by this city. Great People are physical host units but
    /// immediate CIVVIS effects, so this permanent result must cross separately.
    #[serde(default)]
    pub great_works: Option<Vec<StateGreatWork>>,
    /// Firaxis's final city yield vector after every local and empire modifier.
    /// Stored as an additive correction to preserve modeled counterfactual deltas.
    #[serde(default)]
    pub yields: Option<crate::rules::Yields>,
    /// What Civilization VI is CURRENTLY building here, by type name.
    ///
    /// ★★★★ Exported as a raw hash for the whole project (`producing:
    /// -1743686858`) and therefore unusable, so the mirror had no idea what any
    /// city already had underway and CIVVIS re-decided production every turn blind
    /// to work in progress.
    #[serde(default, deserialize_with = "name_or_nothing")]
    pub producing: Option<String>,
    /// Raw production hash retained so a failed name lookup is observable.
    #[serde(default)]
    pub producing_hash: Option<i64>,
    /// Production already invested in `producing`.
    #[serde(default = "unknown_metric")]
    pub production_progress: f64,
    /// ⚠⚠ THE PRODUCTION AVAILABLE TO THE CITY'S BUILD QUEUE, AND IT WAS BEING
    /// THROWN AWAY.
    ///
    /// PR #845 added `production`, `production_cost` and `production_turns` to the
    /// export precisely because they are a DECISION input and not only a diagnostic,
    /// and `StateCity` deserialized only `production_progress`. The other three
    /// arrived on every state event and were dropped — visible the whole time as
    /// `unmapped: schema:city.production,schema:city.production_cost,
    /// schema:city.production_turns` in the decider's own note, which is the
    /// instrument this file added for exactly this failure.
    ///
    /// Live on run `civvis-20260802T083838Z`: `production: 11`, `production_cost: 60`,
    /// `production_turns: 3` for a Quadrireme at `production_progress: 27`.
    /// This is the queue's whole-number production reading, not the exact
    /// per-city yield used by `City:GetYield(YieldTypes.PRODUCTION)`. The latter
    /// crosses in `yields.production` (and in `public_stats.production` for the
    /// empire total) and is the value an economy comparison must use.
    ///
    /// ⚠ This matters far more since #867, which stopped CIVVIS deferring to the
    /// mod's ladder and made it choose production for every city every turn. Choosing
    /// what to build against a production rate the bridge never supplied is the same
    /// shape as the bankruptcy detector reading a `gold_per_turn` nobody wrote.
    #[serde(default = "unknown_metric")]
    pub production: f64,
    /// What `producing` costs in production. With `production` and
    /// `production_progress`, this is what says whether a city can finish an item
    /// before the game ends.
    #[serde(default = "unknown_metric")]
    pub production_cost: f64,
    /// Civilization VI's own estimate of turns remaining on `producing`.
    #[serde(default = "unknown_metric")]
    pub production_turns: f64,
    /// ★★★★★ THE HOST'S OWN PRODUCTION MENU FOR THIS CITY — every unit,
    /// building, wonder, district and project `BuildQueue:CanProduce(hash,
    /// false, true)` says can be STARTED here now, with the engine's cost and
    /// turns for each: the `ProductionPanel.lua` loops, exported by
    /// `CivvisMenus.buildable`. Until this crossed the board chose production
    /// from its own catalogue and learned legality one refusal at a time
    /// (`refused_production`). `None` on an older mod or a failed read is
    /// unknown, not empty — the mirror gates nothing then.
    #[serde(default)]
    pub buildable: Option<Vec<StateMenuItem>>,
    /// The host's purchase menu — what `CityManager.CanStartCommand(city,
    /// PURCHASE, ...)` says can be BOUGHT here now and what
    /// `CityGold:GetPurchaseCost` charges, in Gold and in Faith. `None` is
    /// unknown; an empty list is the host saying nothing is for sale.
    #[serde(default)]
    pub purchasable: Option<Vec<StatePurchaseItem>>,
    /// The build queue BEHIND `producing` (`BuildQueue:GetAt(i)`, i >= 1).
    /// Only the head ever crossed before.
    #[serde(default)]
    pub queue: Option<Vec<StateQueueItem>>,
    /// Food stockpiled toward the next citizen.
    #[serde(default)]
    pub food: f64,
    /// Firaxis's own Housing for this city, and the part of it that comes from
    /// improvements.
    ///
    /// Population is the term every yield is a linear function of — five
    /// completed live games put science at **1.16 x pop**, with city *count*
    /// predicting nothing — and `Game::housing_growth_mult` gates growth on the
    /// headroom over population: `>= 2` full, `>= 1` **half**, below `-4`
    /// **zero**.
    ///
    /// ⚠ CIVVIS has been deriving this from its own rules on the reconstructed
    /// board with no way to check it — the position Amenities were in before
    /// #967, where a claim made from the model had to be retracted as
    /// unverifiable. This carries the number so the model can be **checked**.
    /// It is not a claim that the model is wrong.
    #[serde(default)]
    pub housing: Option<f64>,
    #[serde(default)]
    pub housing_from_improvements: Option<f64>,
    /// The rest of the host's housing ledger, one term each (`GetHousingFrom*`),
    /// so a modelled total that disagrees can name the term it got wrong.
    /// `None` on an older export; `-1` when the host could not read one term.
    #[serde(default)]
    pub housing_from_water: Option<f64>,
    #[serde(default)]
    pub housing_from_buildings: Option<f64>,
    #[serde(default)]
    pub housing_from_districts: Option<f64>,
    #[serde(default)]
    pub housing_from_civics: Option<f64>,
    #[serde(default)]
    pub housing_from_great_people: Option<f64>,
    #[serde(default)]
    pub housing_from_starting_era: Option<f64>,
    #[serde(default)]
    pub housing_from_great_works: Option<f64>,
    /// Growth as the host computes it: surplus after consumption, the threshold
    /// for the next citizen, the multipliers, and the host's own turn forecast.
    #[serde(default = "unknown_metric")]
    pub food_surplus: f64,
    #[serde(default = "unknown_metric")]
    pub growth_threshold: f64,
    #[serde(default = "unknown_metric")]
    pub growth_turns: f64,
    #[serde(default = "unknown_metric")]
    pub housing_growth_mult: f64,
    #[serde(default = "unknown_metric")]
    pub happiness_growth_mult: f64,
    #[serde(default = "unknown_metric")]
    pub overall_growth_mult: f64,
    /// Where each yield comes from, in the host's own words: the text behind the
    /// city panel's per-yield tooltip (`City:GetYieldToolTip`), icon markup
    /// stripped, one entry per yield name. Diagnostic — nothing in the
    /// reconstruction reads it; `tools/civ6_yield_drift.py` parses the amounts.
    #[serde(default)]
    pub yield_sources: Option<std::collections::BTreeMap<String, String>>,
    /// Routes other cities run INTO this one, foreign and domestic, as the host
    /// counts them (the shipped Trade Overview's walk of every other player's
    /// outgoing routes). The destination earns from them under a few rules —
    /// Zhang Qian's +2 Gold per incoming foreign route was live on Aquileia from
    /// t131 of run civvis-20260816T040537Z — and the mirror's own route list holds
    /// only routes this seat sends. Diagnostic for now; `None` on an older export.
    #[serde(default)]
    pub incoming_routes: Option<StateIncomingRoutes>,
    /// The city centre plot's own yields (`Plot:GetYield` on the centre), which
    /// Firaxis lists among the worked plots and CIVVIS floors to 2 Food /
    /// 1 Production before assigning citizens.
    #[serde(default)]
    pub center_yields: Option<crate::rules::Yields>,

    /// Civilization VI's own amenity ledger for this city, and the multiplier it
    /// puts on every non-food yield.
    ///
    /// ★★★★★ NONE OF THIS WAS EVER ASKED FOR. Neither mod exported an amenity,
    /// happiness or luxury field and this struct imported none, so CIVVIS's whole
    /// happiness picture was derived from its own rules on the reconstructed board
    /// and never once checked against the host.
    ///
    /// That derived number is not decoration: [`crate::game::Game::amenity_yield_mult_for`] bands
    /// it straight onto science, production, gold, **culture** and faith — `+5` →
    /// 1.20, `0` → 1.00, `-4` → 0.80, `-6` → 0.70. CIVVIS's model puts the live
    /// empires at `-4/-5`, i.e. paying a **25-30% tax on every yield**, which would
    /// be the largest single multiplier on the board.
    ///
    /// ⚠ **The economy drift line cannot settle this and must not be read as if it
    /// could.** It compares model totals against host totals; a spurious 0.75 here
    /// and an overestimate anywhere else cancel to a clean-looking number. Only the
    /// host's own figure decides it, which is what these carry.
    ///
    /// `happiness_yield_mult` is `GetHappinessNonFoodYieldModifier` — the host's own
    /// version of the very quantity CIVVIS bands for itself, so the two are directly
    /// comparable.
    ///
    /// ⚠ **UNKNOWN HAS TWO SHAPES HERE AND A CONSUMER MUST REJECT BOTH.** A field the
    /// host never sent defaults to [`unknown_metric`], which is `f64::NAN`; a field
    /// the host tried and failed to read arrives as the mod's `-1`. Neither is zero,
    /// and a city with genuinely zero amenities must not reconstruct like a city that
    /// said nothing — a mirror built before this export would otherwise read as a
    /// perfectly happy empire, which is the instrument inventing good news.
    ///
    /// The `>= 0.0` test used by [`host_amenity_report`] rejects both, because every
    /// comparison against `NAN` is false. Any new reader must do the same; `!= -1.0`
    /// alone would let `NAN` through.
    #[serde(default = "unknown_metric")]
    pub amenities: f64,
    #[serde(default = "unknown_metric")]
    pub amenities_needed: f64,
    /// Civ 6's happiness *state*, an enum index, not a count. 4 is "content" —
    /// the shipped CityPanel special-cases exactly that value.
    #[serde(default = "unknown_metric")]
    pub happiness: f64,
    #[serde(default = "unknown_metric")]
    pub happiness_yield_mult: f64,
    /// Where the amenities come from, so a shortfall names its own repair rather
    /// than only its size. Luxuries and entertainment are the two CIVVIS can act on.
    #[serde(default = "unknown_metric")]
    pub amenities_luxuries: f64,
    #[serde(default = "unknown_metric")]
    pub amenities_entertainment: f64,
    #[serde(default = "unknown_metric")]
    pub amenities_civics: f64,
    #[serde(default = "unknown_metric")]
    pub amenities_city_states: f64,
    #[serde(default = "unknown_metric")]
    pub amenities_war_weariness: f64,
    #[serde(default = "unknown_metric")]
    pub amenities_bankruptcy: f64,
    /// The remaining amenity sources the shipped CitySupport reads, so the host's
    /// count decomposes completely and a modelled total can name its wrong term.
    #[serde(default = "unknown_metric")]
    pub amenities_great_people: f64,
    #[serde(default = "unknown_metric")]
    pub amenities_religion: f64,
    #[serde(default = "unknown_metric")]
    pub amenities_national_parks: f64,
    #[serde(default = "unknown_metric")]
    pub amenities_starting_era: f64,
    #[serde(default = "unknown_metric")]
    pub amenities_improvements: f64,
    #[serde(default = "unknown_metric")]
    pub amenities_districts: f64,
    #[serde(default = "unknown_metric")]
    pub amenities_natural_wonders: f64,
    /// Loyalty CHANGE per turn. `loyalty` alone is a level, and a city at 100
    /// falling fast looks identical to one at 100 holding steady — which is exactly
    /// how a city was lost at t98 with loyalty reading 100.
    #[serde(default = "unknown_metric")]
    pub loyalty_per_turn: f64,
    /// Civilization VI's own verdict on who this city would defect to, by player id.
    #[serde(default)]
    pub falls_to: i64,
    pub x: i32,
    pub y: i32,
    #[serde(default)]
    pub pop: i32,
    #[serde(default)]
    pub capital: bool,
    /// ★★★ THE CAPTURE DECISION THE HOST IS WAITING ON. The Firaxis player id
    /// this city was just taken from (`City:GetJustConqueredFrom()`,
    /// `Popups/RazeCity.lua:86`), exported on exactly the city the shipped
    /// popup would show — `Player:GetCities():GetNextCapturedCity()`
    /// (`RazeCity.lua:71`) — and absent once any keep/raze/liberate directive
    /// has been taken. The mirror maps it onto `City::captured_from` as a
    /// seat, so `pending_city_capture_actions` offers the board the same
    /// three choices the popup offers, and the order bridge carries the one
    /// it takes back as kind `city`. `None` on every export before this: the
    /// controller lists the host's `CONSIDER_RAZE_CITY` blocker as soft, so
    /// the host's default — keep — decided every capture unseen.
    #[serde(default)]
    pub captured_from: Option<i64>,
    /// The founder's Firaxis player id (`City:GetOriginalOwner()`,
    /// `RazeCity.lua:85`), whom LIBERATE returns the city to. Mapped onto
    /// `City::original_owner` when the founder sits on a mirrored seat; an
    /// unmapped founder leaves the board's default (the owner), which offers
    /// neither Raze nor Liberate — the honest degraded answer.
    #[serde(default)]
    pub original_owner: Option<i64>,
    #[serde(default = "unknown_metric")]
    pub defense: f64,
    /// Damage and capacity for the city garrison and outer-defense health pools.
    /// These are the same four values Firaxis's city banner displays.
    #[serde(default = "unknown_metric")]
    pub damage: f64,
    #[serde(default = "unknown_metric")]
    pub max_damage: f64,
    #[serde(default = "unknown_metric")]
    pub wall_damage: f64,
    #[serde(default = "unknown_metric")]
    pub max_wall_damage: f64,
    /// Current loyalty, 0-100. Below ~50 and falling, the city is on its way to
    /// revolting to whoever is pressing on it.
    #[serde(default = "unknown_strength")]
    pub loyalty: f64,
}

/// One unit as Civilization VI reported it, in OFFSET coordinates.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateUnit {
    #[serde(default)]
    pub id: i64,
    /// ★★★★★ `type` IS AN ALIAS AND IT WAS MISSING, SO EVERY BARBARIAN WAS DROPPED.
    ///
    /// Our own units are exported as `kind`; the `hostiles` list — which is the ONLY
    /// way barbarians reach this bridge, because `rivals` is built from
    /// `GetAliveMajorIDs()` and cannot contain them — exports `type`. With only
    /// `kind` deserialized every hostile arrived with an EMPTY name, failed the
    /// ruleset lookup, and was silently discarded.
    ///
    /// So `state.hostiles` has been exported, passed to `plant_unit`, and thrown away
    /// 100% of the time. CIVVIS has never once had a barbarian on its board in this
    /// bridge — which means the settler danger rule built on `captor_within` was
    /// looking at an empty threat list, and every "the settler walked into a
    /// barbarian" diagnosis was about a unit CIVVIS could not see.
    ///
    /// ⚠ Found only because `dropped_units` started naming what did not make it onto
    /// the board: the entries read `@37,14:untranslatable` with NO unit type at all,
    /// and an empty name is not a translation failure, it is a field that was never
    /// read. `unmapped` could not show it either — it records the offending name, and
    /// the name was `""`.
    #[serde(default, alias = "type")]
    pub kind: String,
    /// What this unit REPLACES, when it is a civilization unique — Civ 6's
    /// `UnitReplaces.ReplacesUnitType`. See the fallback in `plant_unit`.
    #[serde(default)]
    pub base: Option<String>,
    /// The unit's `PromotionClass` — the last honest rung for a STANDALONE
    /// unique that replaces nothing (Malón Raider, Varu, Nihang), whose `base`
    /// is therefore absent. See [`class_representative`].
    #[serde(default)]
    pub class: Option<String>,
    pub x: i32,
    pub y: i32,
    #[serde(default)]
    pub hp: f64,
    /// Static host combat values retained for export-contract diagnostics. CIVVIS
    /// gets the same base values from its audited ruleset.
    #[serde(default)]
    pub combat: f64,
    #[serde(default)]
    pub ranged: f64,
    /// Present on the aggregate hostile export; ownership is implicit elsewhere.
    #[serde(default = "minus_one_i64")]
    pub player: i64,
    /// Whether this hostile belongs to the Free Cities player (`IsFreeCities`),
    /// set by the mod on `hostiles[]` so the mirror can seat it without knowing
    /// Firaxis's index for that aggregate. Absent on an older export and on a
    /// barbarian; the exported Free Cities minor's `player` is the other key.
    #[serde(default)]
    pub free: bool,
    /// ⚠⚠⚠ NOT "movement available this turn", whatever it looks like. This is
    /// `GetMovesRemaining` sampled at the instant the export is written, and that
    /// instant is not the start of the seat's turn.
    ///
    /// It has now misled twice, in opposite directions, and both times the reader
    /// believed the old one-line description of this field:
    ///
    /// - Feeding it into `moves_left` **silenced CIVVIS completely** — the export
    ///   at the start of turn 31 of run `civvis-20260730T120107Z` had 7 of 8 units
    ///   at `moves: 0`, so `advanced_units` broke on `moves_left <= 0.0` for
    ///   almost every unit and logged 0 actions per turn. See the ★★★★★ note in
    ///   the persistent-sync path, which takes the full allowance instead.
    /// - **Joining it to refusal events by turn number produced a measurement that
    ///   was simply false** and three merged PRs rested on it (#1548, #1550,
    ///   #1552, corrected in #1557). It read `moves: 0` for builders that the mod
    ///   recorded at 2–4 movement at the instant of the refusal.
    ///
    /// So: for "could this unit act *then*", read the value the mod puts in the
    /// EVENT, taken at the point of the decision. For "what can it do *now*", use
    /// [`mirror_unit_moves`]. This field is the raw export and answers neither
    /// question on its own — keep it for fidelity checks and diagnostics.
    #[serde(default = "unknown_strength")]
    pub moves: f64,
    /// Where a multi-turn host path will carry this unit at the start of its
    /// NEXT turn (`UnitManager.GetQueuedDestination`), in offset coordinates.
    /// A unit with one enters the next turn having already spent movement on
    /// it before the brain can act; the mod cancels combat units' queued
    /// paths at turn start and caps every MOVE_TO to the turn's reach, so on a
    /// current mod this is empty. Absent on an older export.
    #[serde(default)]
    pub queued_dest: Option<(i32, i32)>,
    /// Whether the unit is embarked (`Unit:IsEmbarked`). Absent on an older
    /// export; the mirror then infers it from standing on water.
    #[serde(default)]
    pub embarked: Option<bool>,
    /// Attacks left this turn (`Unit:GetAttacksRemaining`). On a mid-turn
    /// combat frame a unit that already struck reports 0 and the board plans
    /// no second strike for it. Absent on an older export → the fresh-turn
    /// allowance.
    #[serde(default)]
    pub attacks_remaining: Option<i32>,
    /// The unit's live range (`Unit:GetRange()`, `Panels/UnitPanel.lua:2250`).
    /// For an aircraft this is the operational range from the plot it stands
    /// on, which is its base — a Civilization VI aircraft sits on its
    /// airfield, city or carrier between sorties, so `x`/`y` name the base and
    /// this names how far `AIR_ATTACK`, `REBASE` and `PATROL` reach from it.
    /// Exported for the seat's own units; absent on an older export.
    #[serde(default)]
    pub range: Option<i32>,
    /// ★★★ THE HOST'S OWN UPGRADE VERDICT (docs/FIDELITY.md, "The one-to-one
    /// map", item 9). `upgrade_to` is the successor `UnitCommandResults.
    /// UNIT_TYPE` names, `upgrade_cost` is `Unit:GetUpgradeCost()`, and
    /// `upgrade_blocked_reason` is present exactly when a successor exists and
    /// the strict `CanStartCommand(unit, UPGRADE, false, true)` cannot start
    /// it this turn (`Panels/UnitPanel.lua:468-483`). A unit with no successor
    /// exports none of the three; an older mod exports none of them for any
    /// unit, and the board's own rules then decide, unchanged.
    #[serde(default)]
    pub upgrade_to: Option<String>,
    #[serde(default)]
    pub upgrade_cost: Option<f64>,
    #[serde(default)]
    pub upgrade_blocked_reason: Option<String>,
    /// The per-type upkeep the shipped Report screen sums, by formation
    /// (`UnitManager.GetUnitMaintenance` / `GetUnitCorpsMaintenance` /
    /// `GetUnitArmyMaintenance`, `Screens/ReportScreen.lua:314-334`), before
    /// the player's per-unit discount.
    #[serde(default)]
    pub maintenance: Option<f64>,
    /// `Unit:GetReligiousStrength()` and `Unit:GetMaxMoves()`, the unit
    /// panel's own stats (`UnitPanel.lua:2257`, `:2242`).
    #[serde(default)]
    pub religious_strength: Option<f64>,
    #[serde(default)]
    pub max_moves: Option<f64>,
    /// `UnitManager.GetActivityType` named as `WorldTracker.lua:544` does:
    /// `sleep`, `hold`, `operation`, `awake`.
    #[serde(default)]
    pub activity: Option<String>,
    /// A Spy's running operation (`Unit:GetSpyOperation()`, -1 → absent;
    /// `EspionageOverview.lua:659`), the turn it ends, and — for an IDLE Spy
    /// standing in a city — the operations `UnitManager.CanStartOperation`
    /// would let it start there (`Choosers/EspionageChooser.lua:196-213`),
    /// as host operation types. The menu is absent, never empty.
    #[serde(default)]
    pub spy_operation: Option<String>,
    #[serde(default)]
    pub spy_operation_end_turn: Option<i64>,
    #[serde(default)]
    pub spy_missions_available: Option<Vec<String>>,
    /// Exact host experience and promotion state. Option distinguishes an older
    /// archive that never exported the facts from a level-one unit with none.
    #[serde(default)]
    pub xp: Option<i64>,
    #[serde(default)]
    pub level: Option<i32>,
    #[serde(default)]
    pub promotions: Option<Vec<String>>,
    /// Civilization VI separates builder and religious charges. CIVVIS has one
    /// typed charge counter, so the mirror selects the applicable observed pool.
    #[serde(default)]
    pub build_charges: Option<i32>,
    #[serde(default)]
    pub spread_charges: Option<i32>,
    /// The religion physically carried by a religious unit, by Firaxis type name.
    #[serde(default)]
    pub religion: Option<String>,
    /// Already fortified. Civilization VI REFUSES `FORTIFY` on a unit that is, so a
    /// board that did not carry this re-ordered it every turn — 28 refusals in run
    /// 233331Z, exactly one per turn from t196 on.
    #[serde(default)]
    pub fortified: bool,
    #[serde(default)]
    pub fortify_turns: i32,
    /// ★★★★★ THE MERGE TIER, AND WHY `FORM_ARMY` COULD NEVER BE SENT LIVE.
    ///
    /// 0 = standard, 1 = Corps/Fleet, 2 = Army/Armada — Firaxis's own
    /// `Unit:GetMilitaryFormation()`, mapped through the shipped
    /// `MilitaryFormationTypes` enum by the mod's `CivvisMilitaryFormation`.
    ///
    /// #2373 wired `Action::CombineUnits` to `UNITCOMMAND_FORM_CORPS` and
    /// `UNITCOMMAND_FORM_ARMY` and picks between them from the mirror's
    /// [`crate::game::Unit::formation`]. The live seat runs `--fresh-board`, so
    /// the mirror is rebuilt from this export every turn — and the export
    /// carried no tier, so every unit was reconstructed as STANDARD and the seat
    /// could only ever ask for a Corps. Exporting this is what makes the Army
    /// half of the unit-consolidation layer reachable at all.
    ///
    /// ⚠ `None` — an older export that never carried the field — and the mod's
    /// own `-1` ("asked, could not answer") both mean UNKNOWN, and neither may
    /// be read as standard. [`apply_unit_observation`] accepts only 0..=2 and
    /// otherwise leaves the board's own tier alone; a fallback that read as
    /// standard is precisely the `GetDefenseStrength` sentinel trap, which
    /// answered −1 for the project's entire life without anyone being able to
    /// tell it from an answer.
    ///
    /// ⚠ NOT [`StateUnit::formation_count`] below. That is the ESCORT stack size
    /// (`GetFormationUnitCount`), which `LinkUnits` reconstructs; a Corps is one
    /// unit and reports a count of 1. Same word, different mechanism.
    #[serde(default)]
    pub formation: Option<i32>,
    /// Number of members in Firaxis's escort/support formation. The stock Unit
    /// Panel exposes this value, and it distinguishes two units sharing a plot from
    /// two units that move as one formation.
    #[serde(default)]
    pub formation_count: i32,
    /// Firaxis represents a Great Person as a physical unit and exposes the exact
    /// plots on which that named individual can activate. CIVVIS applies Great
    /// Person effects immediately, so the live bridge uses these host-validated
    /// targets to enact the same semantic without inventing placement rules.
    #[serde(default)]
    pub great_person: Option<StateGreatPerson>,
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateGreatPerson {
    #[serde(default)]
    pub individual: Option<String>,
    #[serde(default)]
    pub class: Option<String>,
    /// Firaxis `ActionRequiresCompletedDistrictType` for this exact physical
    /// individual. The current market offer is no longer authoritative after
    /// recruitment, so the unit carries its own copy into the production AI.
    #[serde(default)]
    pub required_district: Option<String>,
    /// Firaxis `ActionRequiresMissingBuildingType` for this exact physical
    /// individual. The building must be absent: some activations, such as
    /// James of St. George's, supply it themselves.
    #[serde(default)]
    pub required_missing_building: Option<String>,
    /// Firaxis `ActionRequiresCityGreatWorkObjectType` for this exact physical
    /// individual, for example `GREATWORKOBJECT_ARTIFACT` for Mary Leakey.
    #[serde(default)]
    pub required_great_work: Option<String>,
    #[serde(default)]
    pub charges: i32,
    #[serde(default)]
    pub can_activate: bool,
    #[serde(default)]
    pub activation_plots: Vec<StateActivationPlot>,
    /// Empty Great Work slots anywhere in the empire that this person's work
    /// fits, counted by the host through its own `GreatWork_ValidSubTypes`
    /// table. `None` for classes that do not consume slots, and for exports
    /// from a mod that could not read the slot tables — never a defaulted 0,
    /// because 0 is a claim ("build capacity") and `None` is an absence.
    #[serde(default)]
    pub empty_slots: Option<u32>,
}

impl StateGreatPerson {
    /// This person has nowhere to put its work: it cannot activate, and no
    /// tile the host offers can take one.
    ///
    /// ★★★★★ `empty_slots == Some(0)` IS NOT THE SAME QUESTION, AND EVERY
    /// ESCAPE HATCH WAS ASKING IT. Live run `civvis-20260822T020434Z` reached
    /// turn 231 with **three Great Artists, three Great Writers, three Great
    /// Musicians and a Great Scientist stacked in Rome** — and
    /// `orders.sqlite` holds **not one order of any kind, ever, for any of
    /// the nine**. `ACTIVATE_GREAT_PERSON` fired 18 times that game, all of
    /// them Scientists, Merchants and one Engineer; no Writer, Artist or
    /// Musician was used once in 231 turns.
    ///
    /// Their export says why. Every one reads `can_activate: false` with
    /// **every single `activation_plot` at `slot_open: false`** — the host
    /// saying, tile by tile, that none of them can take the work — while
    /// `empty_slots` reads **24 for the Writers, 4 for the Musicians, 2 for
    /// the Artists**, because that field counts compatible empty slots
    /// EMPIRE-WIDE by the survey's reckoning, including slots on plots the
    /// engine will not offer this person at all.
    ///
    /// So all three exits were shut at once. The driver would not activate
    /// (`can_activate` false), would not walk (every plot known-full is never
    /// a destination, by design and correctly), the mirror's needs machinery
    /// would not ask for capacity, and the work-sale arm would not free a
    /// slot — the last two because both gate on `empty_slots == Some(0)` and
    /// it was 24. Nine Great People fell clean through every branch and idled
    /// for the whole game.
    ///
    /// The operative question is not how many slots the empire owns; it is
    /// whether this person can REACH one. The host answers that per plot with
    /// `slot_open`, so ask it there. `empty_slots == Some(0)` stays a
    /// sufficient condition — it still is one — and `None` keeps the older
    /// control mod's benefit of the doubt exactly as before, never read as
    /// either claim.
    ///
    /// A person the host offers NO plot at all is deliberately not starved
    /// here: that is a missing district, not a missing slot, and the needs
    /// machinery already has its own branch for it — which is how the same
    /// run's Great Scientist correctly asks for the Spaceport its
    /// `required_district` names.
    pub fn slot_starved(&self) -> bool {
        if self.can_activate {
            return false;
        }
        if self.empty_slots == Some(0) {
            return true;
        }
        !self.activation_plots.is_empty()
            && self
                .activation_plots
                .iter()
                .all(|plot| plot.slot_open == Some(false))
    }
}

/// One currently recruitable entry in Firaxis's Great Person timeline.
///
/// The enclosing map is keyed by `GREAT_PERSON_CLASS_*`; keeping the class
/// outside this value mirrors the live timeline's one-current-offer-per-class
/// shape and keeps the bridge's empty-map `nil` convention intact.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateGreatPersonOffer {
    /// Firaxis's named individual, retained for a readable blocker and for
    /// telemetry. It may be absent on an older control mod.
    #[serde(default)]
    pub individual: Option<String>,
    /// Firaxis `ActionRequiresCompletedDistrictType`, when this individual
    /// cannot activate without an already-completed district of that family.
    #[serde(default)]
    pub required_district: Option<String>,
    /// Firaxis `ActionRequiresMissingBuildingType`. This is a negative gate:
    /// the named building must be absent in at least one eligible city.
    #[serde(default)]
    pub required_missing_building: Option<String>,
    /// Firaxis `ActionRequiresCityGreatWorkObjectType`, retained as the raw
    /// host object name until the mirror translates it to a CIVVIS work kind.
    #[serde(default)]
    pub required_great_work: Option<String>,
}

/// One founded religion as the host's Religion screen lists it: its type, the
/// host player id of its founder, and every belief it holds.
///
/// `taken_religion_beliefs` is the union across religions and says only what
/// is no longer available; it cannot say WHICH religion Divine Inspiration
/// belongs to. A city following Catholicism gets Catholicism's follower
/// beliefs — Rome's three Wonders paid twelve Faith under Divine Inspiration
/// in run civvis-20260816T123936Z while the mirror had that belief parked on
/// whichever seat happened to be zipped with the religion — so each religion's
/// beliefs have to sit on its founder's seat, and only there.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateReligion {
    /// `RELIGION_CATHOLICISM` and the like.
    #[serde(rename = "type", default)]
    pub religion: String,
    /// The founder's host player id; `-1` when the host could not say.
    #[serde(default = "unknown_player")]
    pub founder: i64,
    #[serde(default)]
    pub beliefs: Vec<String>,
}

fn unknown_player() -> i64 {
    -1
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateActivationPlot {
    #[serde(default)]
    pub x: i32,
    #[serde(default)]
    pub y: i32,
    #[serde(default)]
    pub distance: i32,
    /// Whether the host knows a compatible empty Great Work slot stands on
    /// this exact tile. The engine's highlight names a cultural person's
    /// districts whether or not a slot is free, so this is the difference
    /// between a destination and a wedge: `Some(true)` = a matching empty
    /// slot is here, `Some(false)` = one of our districts with no such slot
    /// (eleven people stood on one of these for a whole run), `None` =
    /// unknown — a wonder tile, a non-slot-consuming class, or an export
    /// from an older control mod. `None` must never be read as either claim.
    #[serde(default)]
    pub slot_open: Option<bool>,
}

/// One active outgoing trade route as Civilization VI reports it.
///
/// Firaxis keeps a Trader on the map while it services a route.  CIVVIS instead
/// removes that unit and stores the route, so carrying both the unit id and both
/// city ids is necessary: the former prevents re-ordering the busy Trader and the
/// latter preserves capacity and route yields in the reconstructed economy.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateTradeRoute {
    /// Civilization VI's id for the Trader travelling this route.
    #[serde(default)]
    pub trader: i64,
    /// Civilization VI city ids for the two endpoints.
    #[serde(default)]
    pub origin: i64,
    #[serde(default)]
    pub destination: i64,
    #[serde(default = "minus_one")]
    pub destination_player: i32,
    /// Endpoint positions make an international route recoverable even when its
    /// city id has not yet been retained by the visible-rival mirror.
    #[serde(default = "minus_one")]
    pub origin_x: i32,
    #[serde(default = "minus_one")]
    pub origin_y: i32,
    #[serde(default = "minus_one")]
    pub destination_x: i32,
    #[serde(default = "minus_one")]
    pub destination_y: i32,
    /// Trading Posts on the host's own path for this route (origin excluded,
    /// destination included), filed by owner: our cities and other players'.
    /// `None` on an older export or when the path could not be read; the
    /// model then walks its own straight line.
    #[serde(default)]
    pub posts_own: Option<i64>,
    #[serde(default)]
    pub posts_foreign: Option<i64>,
    /// What the route pays its origin per turn, as the host sums it
    /// (`CalculateOriginYieldsFromPotentialRoute` + `…FromPath` +
    /// `…FromModifiers`, times the international multiplier — the shipped
    /// TradeSupport recipe). `None` on an older export.
    #[serde(default)]
    pub yields: Option<crate::rules::Yields>,
}

/// Empire-wide facts Civilization VI exposes in standings without exposing
/// where the underlying cities, wonders, or weapons are located.
///
/// The values are optional because an older control mod has no `public_stats`
/// object, while `-1` remains the live export's per-field "could not read"
/// sentinel. Keeping the aggregate separate from `StateRival::cities` lets the
/// mirror remain fog-honest while the player HUD stays complete.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StatePublicEmpireStats {
    #[serde(default)]
    pub city_count: Option<i64>,
    #[serde(default)]
    pub population: Option<i64>,
    #[serde(default)]
    pub food: Option<f64>,
    #[serde(default)]
    pub production: Option<f64>,
    #[serde(default)]
    pub wonder_count: Option<i64>,
    #[serde(default)]
    pub suzerain_count: Option<i64>,
    #[serde(default)]
    pub nuclear_devices: Option<i64>,
    #[serde(default)]
    pub thermonuclear_devices: Option<i64>,
}

/// The World Congress diplomatic standing as of the last session, and the turn
/// that session was held. See [`StateSnapshot::congress_dvp`] for why a
/// met-gated rival list cannot answer "who is about to win diplomatically".
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateCongressDvp {
    /// The game turn the seat was shown this standing.
    #[serde(default)]
    pub turn: Option<i64>,
    /// One entry per alive major, the seat included.
    #[serde(default)]
    pub points: Vec<StateCongressDvpEntry>,
}

/// One civilization's diplomatic-victory points as the congress reported them.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateCongressDvpEntry {
    #[serde(default)]
    pub player: usize,
    #[serde(default)]
    pub points: i64,
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateRival {
    #[serde(default)]
    pub player: usize,
    /// Civ 6 type name, e.g. `CIVILIZATION_NUBIA`. Mapped onto CIVVIS's roster by
    /// [`civvis_civ_name`] so the rival list reads the same on both screens.
    #[serde(default)]
    pub civ: String,
    #[serde(default)]
    pub leader: String,
    /// The government the rival's diplomacy panel currently shows. It is a
    /// public empire fact, so a fogged rival must not render as unformed.
    #[serde(default)]
    pub government: Option<String>,
    /// Firaxis's current age state for this met rival. These public flags keep
    /// the standings and loyalty lens honest without exporting private plans.
    #[serde(default)]
    pub dark_age: Option<bool>,
    #[serde(default)]
    pub golden_age: Option<bool>,
    #[serde(default)]
    pub heroic_golden_age: Option<bool>,
    /// Whether Civilization VI says this seat may declare war on them RIGHT NOW.
    #[serde(default)]
    pub can_declare: bool,
    #[serde(default)]
    pub score: i64,
    /// Firaxis's current Diplomatic Victory-point total for this rival.
    ///
    /// This is public standings information rather than a fogged tactical fact:
    /// `AdvancedAi` uses it to identify the diplomatic leader for victory denial
    /// and World Congress targeting. `None` means an older control mod could not
    /// export the value; a real zero must remain distinguishable from that case.
    #[serde(default)]
    pub dvp: Option<i64>,
    #[serde(default = "unknown_strength")]
    pub military: f64,
    #[serde(default)]
    pub at_war: bool,
    /// Whether this rival currently grants OUR seat Open Borders — the shipped
    /// overview's "received" direction (`HasOpenBordersFrom`). The import
    /// records the grant on the mirrored seat and stops sealing that rival's
    /// fogged border while it holds, so a passage the seat just bought is
    /// ground the planner can actually use. `None` on an older export.
    #[serde(default)]
    pub open_borders: Option<bool>,
    /// Whether this rival holds Early Empire, i.e. whether its border is
    /// enforced at all (`CIVIC_ENFORCE_BORDERS`). Without it anyone may walk
    /// through; with it only war, a grant, or an alliance opens the ground.
    /// The mirror writes it onto the seat as `Player::borders_enforced`.
    /// `None` on an older export — read as ENFORCED, the measured rule: run
    /// civvis-20260826T184456Z sent 37 military steps into a rival's closed
    /// border and none arrived.
    #[serde(default)]
    pub enforces_borders: Option<bool>,
    /// The host's own relationship state for this rival toward us —
    /// `GameInfo.DiplomaticStates[GetDiplomaticAI():GetDiplomaticStateIndex(us)]
    /// .StateType`: `DIPLO_STATE_WAR`, `_DENOUNCED`, `_UNFRIENDLY`, `_NEUTRAL`,
    /// `_FRIENDLY`, `_DECLARED_FRIEND`, `_ALLIED` (`DiplomacyActionView.lua:870`).
    /// `None` on an older export, which keeps the `can_declare` fallback below.
    /// See [`apply_host_diplomacy`] for what each state writes on the board.
    #[serde(default)]
    pub diplomatic_state: Option<String>,
    /// Grievances we hold against them and they against us, both `>= 0`. The
    /// host keeps ONE signed balance per pair (`GetGrievancesAgainst`,
    /// `DiplomacyActionView_WorldCongressTab.lua:42`); the mod splits it.
    #[serde(default)]
    pub our_grievances_against_them: Option<f64>,
    #[serde(default)]
    pub grievances_against_us: Option<f64>,
    /// `Game.GetGameDiplomacy():GetGrievanceChangePerTurn(them, us)` — the
    /// host's per-turn drift of that balance. Crosses for the record and
    /// `--dump-mirror`; no decision reads it yet (the board's own decay rule
    /// runs in `Game::process_diplomacy`).
    #[serde(default)]
    pub grievance_change_per_turn: Option<f64>,
    /// `GetAllianceType` as `GameInfo.Alliances[i].AllianceType`
    /// (`ALLIANCE_MILITARY`, …), absent when the host answers `-1`; the level
    /// and the turns until expiry from the shipped Alliance tab.
    #[serde(default)]
    pub alliance_type: Option<String>,
    #[serde(default)]
    pub alliance_level: Option<i32>,
    #[serde(default)]
    pub alliance_turns_left: Option<i64>,
    /// `GetDenounceTurn` from each side and `GetDeclaredFriendshipTurn`
    /// (`DiplomacyActionView.lua:1486-1511`), with the game's
    /// `GetDenounceTimeLimit`, so the Formal War wait on the board is the
    /// host's own clock. A value `<= 0` means no such turn.
    #[serde(default)]
    pub our_denounce_turn: Option<i64>,
    #[serde(default)]
    pub their_denounce_turn: Option<i64>,
    #[serde(default)]
    pub friendship_turn: Option<i64>,
    #[serde(default)]
    pub denounce_time_limit: Option<i64>,
    /// `GetVisibilityOn` both ways — the `GameInfo.Visibilities` index
    /// (0 none … 4 top secret). Written to `Player::observed_visibility`,
    /// which [`crate::game::Game::diplomatic_visibility`] prefers.
    #[serde(default)]
    pub visibility: Option<i64>,
    #[serde(default)]
    pub their_visibility_on_us: Option<i64>,
    /// Open Borders WE grant them (`theirs:HasOpenBordersFrom(us)`).
    #[serde(default)]
    pub open_borders_granted: Option<bool>,
    /// Our Delegation / Resident Embassy at their court and theirs at ours
    /// (`HasDelegationAt` / `HasEmbassyAt`, the accessors the delegation
    /// actuator already calls).
    #[serde(default)]
    pub delegation_at: Option<bool>,
    #[serde(default)]
    pub embassy_at: Option<bool>,
    #[serde(default)]
    pub their_delegation: Option<bool>,
    #[serde(default)]
    pub their_embassy: Option<bool>,
    /// `PromiseTypes` member names (`DONT_SETTLE_NEAR_ME`, …) for promises we
    /// made to them and they made to us (`IsPromiseMade`,
    /// `DiplomacyActionView_AllianceRow.lua:61-70`). `Some(vec![])` is the
    /// host's "none"; `None` an older export.
    #[serde(default)]
    pub promises_made: Option<Vec<String>>,
    #[serde(default)]
    pub promises_received: Option<Vec<String>>,
    /// How many technologies this rival has finished, or `-1` if the host
    /// could not be asked.
    ///
    /// ⚠⚠ THIS IS THE FIELD THAT MAKES THE SCORE GAP READABLE. Over 99
    /// completed runs CIVVIS leads in NONE: our score is a median 267 against
    /// the best rival's 1109, a ratio of 0.26. But on empire size we are at
    /// 0.75-0.80 — 3 cities against 4, population 28 against 35 — and our
    /// cities are individually LARGER (10.3 pop against 9.4). So most of the
    /// gap is in components that are neither cities nor population, and until
    /// now the export could not say which.
    ///
    /// ⚠ `#[serde(default = "unknown_metric")]` is load-bearing. A new
    /// `StateRival` field without a default makes the WHOLE snapshot fail to
    /// deserialize on any older export, which silently loses the board — the
    /// failure documented on `map_or_empty_sequence`.
    #[serde(default = "unknown_metric")]
    pub techs: f64,
    /// Civics finished, or `-1` if unavailable. See `techs`.
    #[serde(default = "unknown_metric")]
    pub civics: f64,
    /// The rival's tree by NAME — every `TechnologyType`/`CivicType` it
    /// holds, from the same `HasTech`/`HasCivic` loops the counts above run.
    /// The names go onto the rival's seat (`Player::techs`/`civics`) exactly
    /// as the local seat's do (`apply_rival_tree`), so its era
    /// (`Game::player_era`), the units it can field and its border
    /// (`Game::enforces_borders` reads Early Empire off the civic tree) are
    /// derived natively; `enforces_borders` above stays the override. `None`
    /// on an older export; an empty list is a real "nothing yet".
    #[serde(default)]
    pub tech_names: Option<Vec<String>>,
    #[serde(default)]
    pub civic_names: Option<Vec<String>>,
    /// The shipped World Rankings overview's own lane numbers for this rival
    /// (`g_victoryData`, WorldRankings.lua:27,44,55): `GetNumTechsResearched`,
    /// `GetMilitaryStrengthWithoutTreasury` and `GetNumCitiesFollowingReligion`.
    /// `None` on an older export or a refused read.
    #[serde(default)]
    pub techs_researched: Option<i64>,
    #[serde(default)]
    pub military_no_treasury: Option<f64>,
    #[serde(default)]
    pub cities_following_religion: Option<i64>,
    /// The religion a majority of this rival's cities follow
    /// (`GetReligionInMajorityOfCities`, WorldRankings.lua:2049) — the test
    /// the shipped religion tab runs to call a civilization converted. `None`
    /// when no religion holds a majority or the host could not be asked.
    #[serde(default)]
    pub religion: Option<String>,
    /// Tourists visiting US from this rival — the culture tab's "Visiting us"
    /// column (`GetTouristsFrom` on the local player's culture,
    /// WorldRankings.lua:1766): the per-rival term of the top-level
    /// `foreign_tourists`.
    #[serde(default)]
    pub tourists_visiting_us: Option<i64>,
    /// The rival's Era Score (`GetPlayerCurrentScore`, the accessor the
    /// top-level `era_score` reads for us).
    #[serde(default)]
    pub era_score: Option<i64>,
    /// The rival's outgoing trade routes whose both ends stand on revealed
    /// ground, by endpoint. Seated on the board as routes the rival's seat
    /// owns when both cities are on it (`restore_rival_outgoing_routes`).
    #[serde(default)]
    pub trade_routes: Option<Vec<StateRivalRoute>>,
    /// Luxury resource types this met rival can currently offer in the
    /// shipped diplomacy deal screen that the seat itself does not own.
    /// `Some([])` is the host's authoritative answer that no useful luxury is
    /// available; `None` means an older control mod could not query the deal
    /// catalogue. The luxury-buy arm uses this to avoid selecting a rich
    /// rival whose deal has nothing the seat lacks, which otherwise wastes the
    /// six-turn purchase window.
    #[serde(default)]
    pub tradeable_luxuries: Option<Vec<String>>,
    /// The rival's economy as the host reports it — per-turn Science and
    /// Culture, Tourism, treasury and Faith balances and their per-turn rates —
    /// every one an accessor the shipped World Rankings and Deal screens call
    /// on other players. Before these crossed, the standings' rival Science
    /// and Culture were CIVVIS's own guess from the rival's visible cities.
    /// `-1` when the host could not be asked; NaN (absent) on an older export.
    #[serde(default = "unknown_metric")]
    pub science: f64,
    #[serde(default = "unknown_metric")]
    pub culture: f64,
    #[serde(default = "unknown_metric")]
    pub tourism: f64,
    /// Space-race milestones the host reports for this rival — the rows the
    /// shipped World Rankings science screen lists for OTHER players
    /// (`GetNumProjectsAdvanced` per launch project). Five of the twelve runs
    /// this seat was leading on 2026-08-16/17 ended early on a rival's
    /// culture, technology or diplomatic victory the tracker read as zero
    /// progress. `None` means an older control mod could not export the list;
    /// an empty list is a real "no milestone yet". `#[serde(default)]` is
    /// load-bearing: without it any older export fails to deserialize whole.
    #[serde(default)]
    pub science_projects: Option<Vec<String>>,
    /// Civilization VI's exact World Rankings science-victory position and
    /// current movement. These are not inferred from repeatable laser project
    /// counts: a Terrestrial Laser Station only helps while it is powered.
    /// `-1` is a refused host read; NaN means an older export omitted the key.
    #[serde(default = "unknown_metric")]
    pub science_victory_points: f64,
    #[serde(default = "unknown_metric")]
    pub science_victory_points_per_turn: f64,
    #[serde(default = "unknown_metric")]
    pub science_victory_points_needed: f64,
    /// The culture victory's own two numbers as the shipped screen shows them
    /// for every major: tourists visiting this rival, and its staycationers
    /// (which set the bar every other civilization must clear). `-1` when the
    /// host could not be asked; NaN (absent) on an older export.
    #[serde(default = "unknown_metric")]
    pub foreign_tourists: f64,
    #[serde(default = "unknown_metric")]
    pub domestic_tourists: f64,
    #[serde(default = "unknown_metric")]
    pub gold: f64,
    #[serde(default = "unknown_metric")]
    pub gold_per_turn: f64,
    #[serde(default = "unknown_metric")]
    pub faith: f64,
    #[serde(default = "unknown_metric")]
    pub faith_per_turn: f64,
    /// Public empire totals for the player HUD. These never contain positions
    /// or identity of unseen cities, units, or wonders.
    #[serde(default)]
    pub public_stats: StatePublicEmpireStats,
    #[serde(default)]
    pub cities: Vec<StateCity>,
    #[serde(default)]
    pub units: Vec<StateUnit>,
}

/// One met city-state. Its cities remain remembered after sight, while units
/// are exported only under current visibility just like a major rival's.
/// One endpoint pair of a rival's outgoing trade route, as
/// `rivals[].trade_routes` carries it. `-1` on a coordinate the host could
/// not read; the mirror skips such a route rather than guess.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateRivalRoute {
    #[serde(default = "minus_one")]
    pub origin_x: i32,
    #[serde(default = "minus_one")]
    pub origin_y: i32,
    #[serde(default = "minus_one")]
    pub destination_x: i32,
    #[serde(default = "minus_one")]
    pub destination_y: i32,
    #[serde(default = "minus_one")]
    pub destination_player: i32,
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateMinor {
    #[serde(default)]
    pub player: usize,
    #[serde(default)]
    pub civ: String,
    #[serde(default)]
    pub score: i64,
    #[serde(default = "unknown_strength")]
    pub military: f64,
    #[serde(default)]
    pub at_war: bool,
    #[serde(default = "minus_one")]
    pub suzerain: i32,
    /// Whether this city-state holds Early Empire and so enforces its border
    /// against everyone but its Suzerain (and whoever is at war with it).
    /// `None` on an older export — read as ENFORCED: run
    /// civvis-20260826T184456Z sent 122 military steps into non-suzerain
    /// city-state land and 4 % arrived, against 51 % where we were Suzerain.
    #[serde(default)]
    pub enforces_borders: Option<bool>,
    #[serde(default)]
    pub envoys: i64,
    #[serde(default)]
    pub most_envoys: i64,
    /// What this city-state is asking US for, per the host's QuestsManager
    /// (`HasActiveQuestFromPlayer` over `GameInfo.Quests()`, the shipped
    /// CityStates panel's read). One quest per pair in the shipped rules, so
    /// the first entry is the request. `None` on an older export leaves the
    /// board's own roll alone; `Some([])` is a city-state asking nothing.
    #[serde(default)]
    pub quests: Option<Vec<StateQuest>>,
    /// Every alive major's Envoy count at this city-state
    /// (`GetTokensReceived(player)`, CityStates.lua:1458), zeros included.
    /// `None` on an older export, where a rival's delegation is seeded as the
    /// minimum that elects the Suzerain the host names.
    #[serde(default)]
    pub envoys_by_player: Option<Vec<StateEnvoyCount>>,
    #[serde(default)]
    pub cities: Vec<StateCity>,
    #[serde(default)]
    pub units: Vec<StateUnit>,
}

impl StateMinor {
    fn is_free_cities(&self) -> bool {
        self.civ == "CIVILIZATION_FREE_CITIES"
    }

    fn is_barbarian(&self) -> bool {
        self.civ == "CIVILIZATION_BARBARIAN"
    }

    /// A real city-state actor: not the Free Cities aggregate, not the
    /// barbarian seat. Public for the order bridge's seat resolution.
    pub fn is_city_state(&self) -> bool {
        !self.civ.is_empty() && !self.is_free_cities() && !self.is_barbarian()
    }

    fn is_present_free_cities(&self, state: &StateSnapshot) -> bool {
        self.is_free_cities()
            && (!self.cities.is_empty()
                || !self.units.is_empty()
                || state
                    .hostiles
                    .iter()
                    .any(|unit| hostile_is_free_cities(state, unit)))
    }
}

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

fn unknown_strength() -> f64 {
    -1.0
}

fn unknown_metric() -> f64 {
    f64::NAN
}

fn minus_one_i64() -> i64 {
    -1
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

/// Accept `{}`, `[]`, `null`, or a populated object for a map-valued host field.
///
/// ⚠ **The mod's JSON encoder emits `[]` for an empty table.** `encode` counts a
/// table's entries and takes the array branch whenever `#v == n`, which an empty
/// table satisfies because both are zero. So a Lua field that is logically an
/// empty map arrives as a JSON *array*, serde refuses to read a sequence into a
/// `BTreeMap`, and the failure is not scoped to the field — **the entire
/// `StateSnapshot` fails to deserialize and the board is silently lost.**
///
/// That happened. `great_person_points` shipped in #983 without this, every
/// player has zero Great Person points on turn 1, and three consecutive live
/// attempts reported "no revealed terrain or no state yet" with 0 orders from
/// turn 1, stalled at turn 6 on an unanswered research prompt, and were killed
/// by the watchdog. The mod now returns `nil` when the table is empty; this is
/// the second half of that repair, so a future map-valued export cannot take the
/// board down the same way before anyone notices the encoder's behaviour.
fn map_or_empty_sequence<'de, D, T>(
    deserializer: D,
) -> Result<Option<BTreeMap<String, T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum MapOrSequence<T> {
        Map(BTreeMap<String, T>),
        /// Only ever empty in practice; a populated array has no key to carry.
        ///
        /// The payload is never read, and rustc offers to replace it with `()`
        /// to say so. **Taking that suggestion puts the outage above back.**
        /// In an untagged enum the field's *type* is the matcher: only
        /// `Vec<_>` accepts a JSON array, and `()` accepts `null` — which the
        /// `Null` variant below already claims. Change it and `[]` matches no
        /// variant, the whole `StateSnapshot` fails, and the board is lost
        /// again. The field earns its place by its type, not its value.
        #[allow(dead_code)]
        Sequence(Vec<serde_json::Value>),
        Null,
    }

    Ok(match MapOrSequence::deserialize(deserializer)? {
        MapOrSequence::Map(map) => Some(map),
        MapOrSequence::Sequence(_) => Some(BTreeMap::new()),
        MapOrSequence::Null => None,
    })
}

/// The whole board as one `state` event described it.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateSnapshot {
    pub turn: u32,
    /// 0 for the turn's opening board; N for the Nth mid-turn combat frame
    /// (`CivvisFrames` in the mod), on which the brain re-plans the same turn
    /// with the units' movement and attacks as they now stand.
    #[serde(default)]
    pub frame: u32,
    /// Civ 6 type names of COMPLETED research, e.g. `TECH_BRONZE_WORKING`.
    #[serde(default)]
    pub techs: Vec<String>,
    #[serde(default)]
    pub civics: Vec<String>,
    /// One-time strategic `PROJECT_*` types Firaxis says this seat has completed.
    ///
    /// This is deliberately not every project the player has ever run. District
    /// conversion projects are repeatable, but these are milestones that change
    /// what the planner should build next: the nuclear prerequisites and the
    /// space-race stages. `None` means an older control mod did not export the
    /// fact, while `Some([])` is an authoritative early-game answer.
    #[serde(default)]
    pub science_projects: Option<Vec<String>>,
    /// Exact science-victory readings from Civilization VI's World Rankings:
    /// current points, their active per-turn increase, and the target for this
    /// game speed. These make the live tracker truthful after the Exoplanet
    /// Expedition, including the actual effects of laser stations.
    /// `-1` is a refused host read; NaN means an older export omitted the key.
    #[serde(default = "unknown_metric")]
    pub science_victory_points: f64,
    #[serde(default = "unknown_metric")]
    pub science_victory_points_per_turn: f64,
    #[serde(default = "unknown_metric")]
    pub science_victory_points_needed: f64,
    /// Civ 6 type names whose **boost is triggered but which are NOT yet
    /// researched** — the eureka discount waiting to be collected.
    ///
    /// ⚠⚠ 62 of 77 technologies carry a boost worth 40-50% of their cost, and
    /// `AdvancedAi::tech_value` already pays +28 for a boosted tech — but until
    /// this field existed nothing ever sent the fact, so the live agent's
    /// `boosted_techs` was whatever its own simulation derived rather than what
    /// Civilization VI granted. Same class as the Amenity export (#967) and the
    /// Housing export (#1007): the valuation is right and the input is absent.
    ///
    /// `#[serde(default)]` so an older mod that sends neither still parses.
    #[serde(default)]
    pub boosted_techs: Vec<String>,
    #[serde(default)]
    pub boosted_civics: Vec<String>,
    /// The active Civilization VI technology and the accumulated beakers on it.
    /// Completed technologies alone do not tell the planner whether changing
    /// course discards a nearly finished choice.
    #[serde(default)]
    pub research: Option<String>,
    #[serde(default)]
    pub research_progress: f64,
    /// The active Civilization VI civic and its accumulated culture.
    #[serde(default)]
    pub civic: Option<String>,
    #[serde(default)]
    pub civic_progress: f64,
    /// Civ 6 type name of the government in force, e.g. `GOVERNMENT_OLIGARCHY`.
    ///
    /// ⚠ Nothing carried this and the consequence was not silent, only unread: 62
    /// `cannot_change_government` refusals in 96 turns of run
    /// `civvis-20260731T052021Z`, one every turn. CIVVIS's mirrored player had no
    /// government, and a player with no government asks for one. Policy slots hang
    /// off the government, so it was also choosing cards for a constitution it did
    /// not know it had.
    #[serde(default)]
    pub government: Option<String>,
    /// Every Firaxis government this seat has held at any point in the game,
    /// current one included. Returning to one of these costs Anarchy, and the
    /// engine charges it — but only through `past_governments`, which a board
    /// rebuilt fresh each turn never carries. Without this field the planner
    /// prices the return switch as free, proposes it plus its deck every turn,
    /// and the bridge guard vetoes the switch while the deck is refused: run
    /// civvis-20260815T012010Z logged 127 guard blocks and 15 deck-refusal
    /// turns this way. A plain list; empty is ordinary early game.
    #[serde(default)]
    pub used_governments: Vec<String>,
    /// Civ 6 belief type of the pantheon this seat has founded, if any.
    ///
    /// ⚠ Its absence was not silent, only unread: 125 `pantheon` orders in 173 turns,
    /// every one counted applied, against one pantheon. A seat that does not know it
    /// has a pantheon keeps choosing one — and is also missing that belief's yields
    /// from every calculation it makes.
    #[serde(default)]
    pub pantheon: Option<String>,
    /// Player-level religion facts, distinct from each city's majority religion.
    #[serde(default)]
    pub founded_religion: Option<String>,
    /// Every non-pantheon religion founded worldwide. Firaxis exposes this in
    /// the Religion screen even when its founder has not otherwise been met.
    #[serde(default)]
    pub founded_religions: Vec<String>,
    #[serde(default)]
    pub religion_beliefs: Vec<String>,
    /// Every belief already claimed worldwide, including religions outside vision.
    #[serde(default)]
    pub taken_religion_beliefs: Vec<String>,
    /// Every founded religion with its founder and its own beliefs. Empty on
    /// an older control mod, when the union above is all the mirror has.
    #[serde(default)]
    pub religions: Vec<StateReligion>,
    #[serde(default)]
    pub prophet_pending: bool,
    /// Civ 6 policy types currently slotted, e.g. `POLICY_DISCIPLINE`.
    ///
    /// ⚠ Same shape as `government` and `pantheon`: a fact the game holds that CIVVIS
    /// was never told, so it re-decided it every turn. 73 `no_slot_for_*` refusals and
    /// 23 `already_*` in 61 turns of run civvis-20260731T070956Z.
    #[serde(default)]
    pub policies: Vec<String>,
    /// How many policy slots this government actually has. Choosing a card for a slot
    /// that does not exist is an uninformed decision, not a bad one.
    #[serde(default)]
    pub policy_slots: i64,
    #[serde(default)]
    pub gold: i64,
    /// Firaxis's own NET income, `GetGoldYield() - GetTotalMaintenance()` — the
    /// figure the shipped TopPanel prints beside the treasury.
    ///
    /// ★★★★★ THE EMPIRE GOES BANKRUPT WITHOUT NOTICING WITHOUT THIS.
    /// [`LiveMirror::mirror_net_income`] derives the rate from the treasury delta
    /// between CONSECUTIVE turns and keeps `last_treasury` on the mirror, but the
    /// bridge runs `civvis_orders --serve --fresh-board`, which rebuilds that
    /// mirror every turn — so the predecessor is never there and the rate never
    /// lands. Measured at **0.00 in 963 of 963 calls**.
    ///
    /// Live run `civvis-20260810T191050Z` (Rome/Trajan, Settler) is what that
    /// costs: the treasury peaked at 319 on turn 60, hit **0 on turn 110 and
    /// stayed there for the remaining 75 turns**. With the bankruptcy guard
    /// blind, the empire kept units it could not pay for, Civilization VI
    /// disbanded them (`army` 12 → 0), and the cities fell at t173, t180 and
    /// t184 — **six cities became two**, final score 403 against Mongolia's 747.
    /// Tech and civics were competitive the whole game (44 vs 46, 35 vs 34), so
    /// this single number is the gap.
    ///
    /// `None` when the host did not answer. A real `0.0` is break-even and a
    /// missing answer is not; conflating them is the failure above.
    #[serde(default)]
    pub gold_per_turn: Option<f64>,
    /// The bill behind that net, by source — `PlayerTreasury:GetUnitMaintenance`,
    /// `GetBuildingMaintenance`, `GetDistrictMaintenance`, the top panel's own
    /// breakdown (`ToolTipHelper_PlayerYields.lua:22-26`). `None` when the
    /// host did not answer, exactly like `gold_per_turn`; they reach
    /// `Game::host_maintenance` and replace the board's own sums there.
    #[serde(default)]
    pub unit_maintenance_total: Option<f64>,
    #[serde(default)]
    pub building_maintenance_total: Option<f64>,
    #[serde(default)]
    pub district_maintenance_total: Option<f64>,
    /// Faith balance. Unlike the rate fields below, this is a stockpile and
    /// crosses directly, exactly like gold.
    #[serde(default)]
    pub faith: i64,
    /// Civilization VI's own Faith PER TURN — `Player:GetReligion():GetFaithYield()`,
    /// the top bar's figure — applied like `science` and `culture` below: a
    /// correction on the reconstructed empire yield, never a replacement.
    ///
    /// Until it was exported the board could not be right by construction:
    /// on run civvis-20260816T123936Z the host banked 100–113 Faith a turn
    /// from t231 while every city together made 49 (the rest is the Faith
    /// paid for unused Great Person points, `unused_great_person_faith`), and
    /// the mirror had no host figure to measure that against. `None` on an
    /// older control mod.
    #[serde(default)]
    pub faith_per_turn: Option<f64>,
    /// The host's own Faith ledger, `GetFaithYieldToolTip` compacted — "+N from
    /// Cities / Beliefs / Envoys / city-states you are Suzerain of / Other" —
    /// so a gap between the two games is named, not guessed at.
    #[serde(default)]
    pub faith_sources: Option<String>,
    /// Civilization VI's own science and culture PER TURN.
    ///
    /// Applied as a correction to the reconstructed empire yield, not as a flat
    /// replacement. That makes the displayed and timing baseline exact while still
    /// letting a counterfactual policy or building contribute its modeled delta.
    /// The raw derived gap remains in the per-turn diagnostic so missing host rules
    /// stay measurable. On `civvis-20260801T024428Z` at turn 60 it was:
    ///
    /// | | Civilization VI | CIVVIS | drift |
    /// |---|---|---|---|
    /// | science | 5.80 | 8.6 | **+48%** |
    /// | culture | 7.08 | 8.9 | **+26%** |
    ///
    /// That matters because research VALUATIONS are spent in these units — CIVVIS
    /// rates a tech "worth 42 to the expansion plan" and times its plan against a rate
    /// half again too fast. An axis nothing reports does not exist; this makes the
    /// approximation's size visible on every turn so it can be tracked, and so any
    /// future decision to close it starts from evidence rather than from a guess.
    /// `None` on an older export; zero is a real yield during the unsettled
    /// opening and must not be mistaken for an unavailable answer.
    #[serde(default)]
    pub science: Option<f64>,
    #[serde(default)]
    pub culture: Option<f64>,
    /// Exact public empire totals for the active seat. Its city records remain
    /// the source of detailed state; this is the fog-safe aggregate shared with
    /// every rival so a standing never becomes zero merely because the mirror
    /// does not retain every city.
    #[serde(default)]
    pub public_stats: StatePublicEmpireStats,
    #[serde(default)]
    pub score: i64,
    /// Civilization VI's current Diplomatic Victory-point tally.
    ///
    /// The board is reconstructed before each live decision, so leaving this at
    /// `Player::default()` makes the strategy engine believe Rome has zero points
    /// even after the host has awarded some. `None` is deliberately distinct from
    /// zero: it preserves compatibility with an older loaded control mod without
    /// erasing a value already held by a persistent mirror.
    #[serde(default)]
    pub dvp: Option<i64>,
    /// Civilization VI's current stock of Diplomatic Favor.
    ///
    /// Favor decides how many World Congress votes the live seat can actually
    /// afford. It is a stock, not a yield, so deriving it from a reconstructed
    /// history loses the fact the host already knows. As with [`Self::dvp`],
    /// `None` means unavailable rather than an authoritative zero.
    #[serde(default)]
    pub favor: Option<f64>,
    /// 🔴🔴🔴 THE DIPLOMATIC STANDING THE SEAT IS SHOWN AND THE TRACKER NEVER SAW.
    ///
    /// [`StateRival`] is met-gated, so `players[*].dvp` only ever carried the
    /// civilizations this seat had contacted. The World Congress seats every
    /// alive major and shows the seat all of their points, and the host's own
    /// `WC_RES_DIPLOVICTORY` ballot names them as targets — which is why
    /// `voteWorldCongress` has always been free to pick its leader from the
    /// full set. That knowledge stopped at the ballot box.
    ///
    /// Measured over the 50 live runs carrying a congress table, **40 (80%)
    /// showed a DVP standing higher than any rival the decider could see**, and
    /// in five the difference crossed the denial alarm. In
    /// `civvis-20260818T103630Z` the eventual winner sat at 22 points while the
    /// best visible rival read 14, so `urgent_victory_threat` never fired once
    /// in 222 turns and the seat lost a game it led by 213.
    ///
    /// This is a congress-time snapshot, not a live read: it is what the last
    /// session showed, stamped with the turn it showed it, and it goes stale
    /// between sessions the same way a human's memory of the last vote does.
    #[serde(default)]
    pub congress_dvp: Option<StateCongressDvp>,
    /// How many Spies Civilization VI will let this empire field, from the
    /// accessor the shipped Espionage Overview prints
    /// (`GetDiplomacy():GetSpyCapacity()`).
    ///
    /// Without it `block_live_spy_production` had to refuse Spy production
    /// unconditionally, and the seat has therefore never held one: measured
    /// over twelve completed live games it finished holding the Diplomatic
    /// Service civic in **12 of 12** and fielded **zero** Spies. `None` means
    /// an older control mod could not say, which keeps the old blanket block.
    #[serde(default)]
    pub spy_capacity: Option<i64>,
    /// Our own culture-victory counters, same accessors as each rival's
    /// (`GetTouristsTo`/`GetStaycationers`): OUR staycationers are the bar
    /// every rival's visiting tourists must clear. `-1` when the host could
    /// not be asked; NaN (absent) on an older export.
    #[serde(default = "unknown_metric")]
    pub foreign_tourists: f64,
    #[serde(default = "unknown_metric")]
    pub domestic_tourists: f64,
    /// Our tourism per turn as the host reports it (`GetStats():GetTourism()`,
    /// the accessor already used for each rival). `None` on an older export.
    /// The mirror writes it into `Game::observed_tourism_per_turn`, and it is
    /// the live side of the `tourism` divergence row, which read "no pairs"
    /// on every run until it crossed.
    #[serde(default)]
    pub tourism_per_turn: Option<f64>,
    /// Cities anywhere following our religion, the religion lane's own
    /// number (`GetNumCitiesFollowingReligion`, WorldRankings.lua:44), the
    /// same accessor as each rival's. `None` on an older export or a refused
    /// read.
    #[serde(default)]
    pub cities_following_religion: Option<i64>,
    #[serde(default = "unknown_strength")]
    pub military: f64,
    /// Era Score and the age it decides, from Firaxis's own `Game.GetEras()`.
    ///
    /// ★★★★★ CIVVIS MODELS THE WHOLE AGE SYSTEM AND THE BRIDGE CARRIED NONE OF IT.
    /// [`crate::game::Player`] has `era_score`, `era_score_baseline`,
    /// `normal_age_threshold`, `golden_age_threshold` and `dedications`, and
    /// `docs/AGES.md` records a row-by-row audit of all 143 scoring Moments
    /// behind them. On a reconstructed live board every one of those was left at
    /// whatever `Player::default` happens to say — era score 0 against a golden
    /// threshold of 26 — so the age CIVVIS reasoned about was not the age Rome
    /// was in.
    ///
    /// Two decisions read exactly these fields, so both ran on fiction live:
    /// `ai::choose_dedications` is gated on `dedication_choices` (which must
    /// cross from the host at an era boundary), and `ai/advanced.rs` filters
    /// `rules.policies[card].dark_age`, so a real Dark Age's wildcard cards were
    /// never slotted — the same shape as the housing and loyalty cards that are
    /// never slotted.
    ///
    /// `normal_age_threshold` is this codebase's name for the score at or above
    /// which the next age is Normal rather than Dark, which is exactly Civ 6's
    /// **Dark Age threshold**: one boundary, named from opposite sides.
    ///
    /// `None`/negative means the host did not answer. A real 0 era score is
    /// ordinary on turn 1 and must not read as "unknown".
    #[serde(default)]
    pub era_score: Option<i64>,
    #[serde(default)]
    pub era_score_baseline: Option<i64>,
    #[serde(default)]
    pub normal_age_threshold: Option<i64>,
    #[serde(default)]
    pub golden_age_threshold: Option<i64>,
    /// Firaxis's world era index, which advances on the field rather than on
    /// this empire alone.
    #[serde(default)]
    pub world_era: Option<i64>,
    #[serde(default)]
    pub dark_age: Option<bool>,
    #[serde(default)]
    pub golden_age: Option<bool>,
    #[serde(default)]
    pub heroic_golden_age: Option<bool>,
    /// The Dedications (Commemorations) the host says this seat has active,
    /// as `COMMEMORATION_*` type names — what a Golden Age PAYS. `None` on an
    /// older export; an empty list is a seat with none.
    #[serde(default)]
    pub dedications: Option<Vec<String>>,
    /// How many dedication choices the host currently permits. `None` means an
    /// older export; a real zero means the seat has no pending era choice.
    #[serde(default)]
    pub dedication_choices: Option<i64>,
    /// The World Congress resolutions binding this turn (`GetResolutions`),
    /// mapped onto the model's own `active_congress_effects`. `None` on an
    /// older export leaves the model's Congress alone; `Some([])` is a world
    /// with nothing in effect.
    #[serde(default)]
    pub resolutions: Option<Vec<StateResolution>>,
    /// The host's active World Congress emergency tracker. `None` means an
    /// older control mod did not export it; `Some([])` is an authoritative
    /// statement that no live competition remains.
    #[serde(default)]
    pub emergencies: Option<Vec<StateEmergency>>,
    /// Turns until the next regular session — how long `resolutions` stay
    /// binding (`GetMeetingStatus().TurnsLeft`).
    #[serde(default)]
    pub congress_turns_left: Option<i64>,
    /// Gathering Storm's climate, off `GameClimate` the way the shipped
    /// ClimateScreen reads it. `None` on an older export or a ruleset without
    /// the object, where the board's own phase stands.
    #[serde(default)]
    pub climate: Option<StateClimate>,
    /// Where a Trader could go from each of our cities and what the host says
    /// each route would pay its origin, exported while a route slot is open
    /// (the 12 richest per origin). `None` when nothing can be started or on
    /// an older export.
    #[serde(default)]
    pub route_options: Option<Vec<StateRouteOption>>,
    /// Firaxis's own outgoing-route capacity. The model can differ because a
    /// mirrored empire does not reproduce every capacity modifier.
    #[serde(default)]
    pub trade_capacity: Option<i64>,
    /// Envoys we are HOLDING and have not placed, per Firaxis's own
    /// `GetTokensToGive`.
    ///
    /// ★★★★★ `minors[].envoys` has always said where our envoys LANDED. Nothing
    /// ever said how many were sitting unspent, and that single omission closed
    /// the whole axis: [`crate::game::Game::legal_actions`] gates
    /// `Action::SendEnvoy` behind `if p.envoys_free > 0`, and while `envoys_free`
    /// was not mirrored every reconstructed live board read 0 and **CIVVIS
    /// never enumerated sending an envoy at all**. That is why `SendEnvoy`
    /// appeared nowhere in the skipped-action tally, while `LevyMilitary` — which
    /// needs a suzerainty we never hold — appeared there 44 times.
    ///
    /// Measured over 36 live runs past turn 150: median envoys placed **1**,
    /// median suzerainties **0**, 16 of 36 runs ending with none placed anywhere.
    /// CIVVIS prices the payoff in full (`envoy_type_yields_for_count` pays a
    /// cultural city-state at the 1/3/6 thresholds), so the sim collects it and
    /// the live game collects nothing.
    ///
    /// **Now MIRRORED onto `Player::envoys_free`** (see
    /// [`apply_mirrored_envoys_free`]), so the deployed `advanced_envoys` pass
    /// enumerates and prices `SendEnvoy` on the live board exactly as it does
    /// natively, and `civvis_orders` translates each one into an `envoy` order
    /// the mod actuates. It was carried-and-reported only, for the crash the
    /// Lua `chooseEnvoy` lane was blamed for; that lane's own record since —
    /// the stale-handle write fixed, the governor-appointment prompt found to
    /// share the crash signature and the t44–47 cluster, and a 250-turn
    /// `EnvoyEnabled` run that placed 113 envoys without a fault — is why the
    /// board is allowed to want them now. The Lua chooser stays off; the
    /// decision is CIVVIS's, the mod only places what it is told to.
    ///
    /// `None`/negative means the host did not answer; a real 0 means we are
    /// genuinely holding none, and the two must not read the same.
    #[serde(default)]
    pub envoys_free: Option<i64>,
    /// Great Person POINTS by Civilization VI class type, e.g.
    /// `GREAT_PERSON_CLASS_SCIENTIST`. The points, not the people: the earned
    /// individuals already arrive as units.
    ///
    /// `district_project_value` prices every district project against the live
    /// Great Person race — how close this empire is to the next one of that
    /// class, and how close the leading rival is. Without this the race is all
    /// zeros in every live game, so the Campus project's entire reason to exist
    /// (Great Scientist points) is invisible to the planner that chooses it.
    #[serde(default, deserialize_with = "map_or_empty_sequence")]
    pub great_person_points: Option<BTreeMap<String, f64>>,
    /// The same classes' points PER TURN (`GetPointsPerTurn`), the host's own
    /// figure for what the districts, buildings, wonders, policies and
    /// Governors add each turn. Nothing plans on it yet; it is exported so the
    /// Faith Firaxis pays for an unrecruitable class can be checked against
    /// the host's rate rather than the model's.
    #[serde(default, deserialize_with = "map_or_empty_sequence")]
    pub great_person_points_per_turn: Option<BTreeMap<String, f64>>,
    /// The classes with nobody left to recruit anywhere on the host's
    /// timeline (`GREAT_PERSON_CLASS_SCIENTIST` once the last Great Scientist
    /// is claimed by anyone). Their points are what Firaxis pays out as
    /// Faith. An empty list is a real answer — everyone still available;
    /// `None` is an older control mod, and the cost map stands in.
    #[serde(default)]
    pub great_person_exhausted: Option<Vec<String>>,
    /// The seat's strategic stockpiles by host resource type
    /// (`RESOURCE_IRON` → amount). Without them `Game::strategic_stockpile`
    /// read 0 for everything on the live seat: no unit that costs a strategic
    /// resource was ever producible, and no unit was ever obsolete for want of
    /// a buildable successor. See `apply_strategic_stockpiles`.
    #[serde(default, deserialize_with = "map_or_empty_sequence")]
    pub strategic_resources: Option<BTreeMap<String, f64>>,
    /// The live RECRUIT COST of each class's current unclaimed Great Person,
    /// by the same class-type key. Points without costs sent the planner's
    /// threshold check to CIVVIS's own market formula, which quoted 60-ish
    /// where the live timeline wanted hundreds: run civvis-20260815T033823Z
    /// recorded 45 `gp_cannot_recruit` refusals — the recruit order finally
    /// crossed the bridge (#1596) only for the live game to answer "not yet"
    /// every time, because the ask itself was priced against the wrong game.
    #[serde(default, deserialize_with = "map_or_empty_sequence")]
    pub great_person_costs: Option<BTreeMap<String, f64>>,
    /// The named Great Person currently offered by Firaxis for each class,
    /// including the hard prerequisites the class label cannot express.
    ///
    /// A Great Scientist is not necessarily usable at a Campus: Hildegard of
    /// Bingen requires a Holy Site, while Mary Leakey requires a Theater
    /// district and an Archaeological Museum slot. James of St. George also
    /// requires the Castle building to be missing because his activation adds
    /// the Medieval Walls. The planner previously read only the class and
    /// could spend a whole Campus-project race on an individual it had no
    /// possible way to activate. Carry the host's three exact prerequisite
    /// columns without attempting to recreate every named effect in CIVVIS's
    /// ruleset.
    #[serde(default, deserialize_with = "map_or_empty_sequence")]
    pub great_person_offers: Option<BTreeMap<String, StateGreatPersonOffer>>,
    /// Total Governor Titles obtained and spent according to Firaxis. These are
    /// separate from the roster because a title can be held unspent.
    #[serde(default)]
    pub governor_points: Option<i64>,
    #[serde(default)]
    pub governor_points_spent: Option<i64>,
    /// Authoritative appointed roster. `None` means unknown; `Some([])` means empty.
    #[serde(default)]
    pub governors: Option<Vec<StateGovernor>>,
    #[serde(default)]
    pub cities: Vec<StateCity>,
    #[serde(default)]
    pub units: Vec<StateUnit>,
    /// Routes currently travelling for this seat.  The control mod gets these
    /// from each of our cities' `GetOutgoingRoutes()`; routes belonging to a
    /// rival remain hidden just as their unseen units do.
    #[serde(default)]
    pub trade_routes: Vec<StateTradeRoute>,
    #[serde(default)]
    pub rivals: Vec<StateRival>,
    #[serde(default)]
    pub minors: Vec<StateMinor>,
    /// Who this seat actually is, from the run's `seat` event.
    ///
    /// ⚠ Not part of the `state` event — [`state_from_events`] merges it in, so
    /// every existing caller gets identity without changing its signature.
    #[serde(default)]
    pub seat: Seat,
    /// Sites Civilization VI has refused to found on, AXIAL, from `found_refused`
    /// events. Merged in by [`state_from_events`] for the same reason as `seat`.
    #[serde(default)]
    pub refused_sites: std::collections::BTreeSet<crate::Pos>,
    /// Tiles Civilization VI refused to improve, AXIAL, from `improve_refused`.
    #[serde(default)]
    pub refused_improves: std::collections::BTreeSet<crate::Pos>,
    /// Promotions the host refused, per Civilization VI unit id.
    /// See `Game::blocked_promotions` for why this exists.
    ///
    /// ⚠ `#[serde(default)]` is LOAD-BEARING: this is merged in by
    /// [`state_from_events`] and never appears in the host's `state` JSON, so
    /// without it the whole `StateSnapshot` fails to deserialize and the board is
    /// silently lost — the failure documented on [`map_or_empty_sequence`]. Adding
    /// this field without the attribute took a replay from 4,339 orders to 0.
    #[serde(default)]
    pub refused_promotions: std::collections::BTreeMap<i64, std::collections::BTreeSet<String>>,
    /// Strikes the host refused THIS TURN, as `(unit, x, y)` in Civilization VI
    /// ids and offset plots, from `range_attack_refused` and `war_refused`.
    /// Per turn, not cumulative: a shot the host refused for line of sight last
    /// turn may be open after a move, and a war refusal is answered by the
    /// diplomacy arm, not by never striking that plot again.
    /// See `Game::blocked_strikes`.
    ///
    /// ⚠ `#[serde(default)]` is load-bearing here for the same reason as above.
    #[serde(default)]
    pub refused_strikes: std::collections::BTreeSet<(i64, i32, i32)>,
    /// The host's answers to this turn's `preview` orders, keyed
    /// `(unit, x, y, verb)` in Civilization VI ids and offset plots. Per turn:
    /// a simulation is a reading of the board as it stood when it was asked.
    /// See `Game::host_previews`.
    ///
    /// ⚠ `#[serde(default)]` is load-bearing here for the same reason as above.
    #[serde(default)]
    pub host_previews:
        std::collections::BTreeMap<(i64, i32, i32, String), crate::game::HostStrikePreview>,
    /// Origin/destination pairs Firaxis rejected for trade-route pathing.
    #[serde(default)]
    pub refused_trade_routes: std::collections::BTreeSet<(crate::Pos, crate::Pos)>,
    /// Policy cards Civilization VI has retired, as its OWN names, harvested from the
    /// `obsolete_<POLICY>` refusal reasons already in the stream. Translated where the
    /// ruleset is in hand; see [`refused_policies`].
    #[serde(default)]
    pub refused_policy_names: std::collections::BTreeSet<String>,
    /// Pantheon beliefs Civilization VI refused as already taken by another
    /// player, as its OWN names, harvested from the `taken_<BELIEF>` refusal
    /// reasons already in the stream. Translated where the ruleset is in hand;
    /// see [`refused_pantheons`] and `Game::blocked_pantheons`.
    #[serde(default)]
    pub refused_pantheons: std::collections::BTreeSet<String>,
    /// Districts Civilization VI refused to place, by ITS city id, from
    /// `build_no_plot`. Mapped onto CIVVIS cities where `city_ids` is in hand; see
    /// [`refused_districts`].
    #[serde(default)]
    pub refused_districts: std::collections::BTreeMap<i64, std::collections::BTreeSet<String>>,
    /// Fresh, host-approved alternatives for a district CIVVIS asked to place
    /// on the wrong tile.  Like [`StateSnapshot::refused_districts`], these
    /// arrive through `build_no_plot` rather than the state payload; unlike a
    /// refusal, their coordinates are a positive, short-lived fact that lets
    /// the next decision name a plot Firaxis has already approved.
    #[serde(default)]
    pub host_district_sites: BTreeMap<i64, BTreeMap<String, BTreeSet<crate::Pos>>>,
    /// Fresh, host-approved alternatives for a wonder CIVVIS asked to place on
    /// the wrong tile. They use the same short-lived `build_no_plot` evidence as
    /// districts, but the Firaxis event identifies wonders under `building`.
    #[serde(default)]
    pub host_wonder_sites: BTreeMap<i64, BTreeMap<String, BTreeSet<crate::Pos>>>,
    /// Wonders Civilization VI refused to place, by ITS city id, from the same
    /// `build_no_plot` event. Kept apart from [`StateSnapshot::refused_districts`]
    /// because the mod reports a refused wonder under `building` and a refused
    /// district under `district`, and the two translate against different rulesets.
    #[serde(default)]
    pub refused_wonders: std::collections::BTreeMap<i64, std::collections::BTreeSet<String>>,
    /// World-unique wonders a `build_no_plot` answer ruled out on every host
    /// location. This is deliberately separate from [`StateSnapshot::refused_wonders`]:
    /// a city-local refusal can be a terrain mismatch, while an explicit
    /// `offered: 0` is the host saying the requested wonder has no target at all.
    #[serde(default)]
    pub host_unavailable_wonders: std::collections::BTreeSet<String>,
    /// Production items the host has recently reported as unplayable, by its city id.
    /// These are translated and applied as a cooldown rather than a permanent ban.
    #[serde(default)]
    pub refused_production: std::collections::BTreeMap<i64, std::collections::BTreeSet<String>>,
    /// Purchases Civilization VI recently rejected, by its city id. Kept apart
    /// from production refusals so a failed purchase causes a build fallback
    /// instead of suppressing that build as well.
    #[serde(default)]
    pub refused_purchases: std::collections::BTreeMap<i64, std::collections::BTreeSet<String>>,
    /// Barbarian units this seat can SEE.
    ///
    /// ★★★★ The rival export is built from `GetAliveMajorIDs`, so barbarians could
    /// never appear in it and could never show `at_war`. A city lost to them read as
    /// "lost at peace with everyone", which is how the analysis of how cities are
    /// lost was made with an instrument blind to the likeliest cause.
    #[serde(default)]
    pub hostiles: Vec<StateUnit>,
    /// Raw export keys that this adapter does not recognize. These remain
    /// non-fatal, but flow into `unmapped` so adding a Lua field can never again
    /// look like successful mirroring while serde silently discards it.
    #[serde(skip)]
    pub schema_gaps: Vec<String>,
}

/// Put fog-safe host standings on a reconstructed seat without manufacturing
/// the hidden cities, wonders, or weapons that produced them.
fn apply_public_empire_stats(
    game: &mut crate::game::Game,
    owner: usize,
    source: &StatePublicEmpireStats,
) {
    let count = |value: Option<i64>| {
        value
            .filter(|value| *value >= 0)
            .and_then(|value| usize::try_from(value).ok())
    };
    let population = source
        .population
        .filter(|value| *value >= 0)
        .and_then(|value| i32::try_from(value).ok());
    let observed = Arc::make_mut(&mut game.observed_public_empire_stats)
        .entry(owner)
        .or_default();
    observed.city_count = count(source.city_count);
    observed.population = population;
    observed.wonder_count = count(source.wonder_count);
    observed.suzerain_count = count(source.suzerain_count);
    observed.nuclear_devices = source.nuclear_devices.filter(|value| *value >= 0);
    observed.thermonuclear_devices = source.thermonuclear_devices.filter(|value| *value >= 0);
}

/// Put a rival's tree by name on its seat, and let its border be derived
/// from that tree when the host sent the names but not the one-bit answer.
///
/// The names are the same `HasTech`/`HasCivic` loop the counts come from,
/// mapped through `civvis_node_name` exactly as the local seat's lists are,
/// and ASSIGNED (not merged) so a seat that changed hands between exports
/// does not keep a tree it no longer holds. With the tree on the seat,
/// `Game::player_era`, the unit roster and `Game::enforces_borders` (Early
/// Empire's `open_borders` tree effect) all read a rival the way they read a
/// native player. A name CIVVIS lacks is filed in `unmapped`, as ours are.
///
/// The border: the host's own `enforces_borders` bit wins when it crossed;
/// without it, a tree that crossed lets the civic decide (`None`, the native
/// rule); and an export with neither reads as enforced — the conservative
/// answer and the measured one (run civvis-20260826T184456Z, 37 military
/// steps into a rival's closed border, none arrived).
fn apply_rival_tree(
    game: &mut crate::game::Game,
    owner: usize,
    rival: &StateRival,
    unmapped: &mut Vec<String>,
) {
    if let Some(names) = &rival.tech_names {
        let mut techs = std::collections::BTreeSet::new();
        for civ6 in names {
            match civvis_node_name(&game.rules.techs, civ6, "TECH_") {
                Some(name) => {
                    techs.insert(crate::name::Name::new(&name));
                }
                None if !unmapped.contains(civ6) => unmapped.push(civ6.clone()),
                None => {}
            }
        }
        game.players[owner].techs = techs;
    }
    if let Some(names) = &rival.civic_names {
        let mut civics = std::collections::BTreeSet::new();
        for civ6 in names {
            match civvis_node_name(&game.rules.civics, civ6, "CIVIC_") {
                Some(name) => {
                    civics.insert(crate::name::Name::new(&name));
                }
                None if !unmapped.contains(civ6) => unmapped.push(civ6.clone()),
                None => {}
            }
        }
        game.players[owner].civics = civics;
    }
    game.players[owner].borders_enforced = match rival.enforces_borders {
        Some(enforced) => Some(enforced),
        None if rival.civic_names.is_some() => None,
        None => Some(true),
    };
}

/// Put a rival's host-reported public standings and economy on its seat:
/// treasury and Faith balances directly, all five top-bar yields as a
/// host-to-model delta, and aggregate HUD totals separately. This makes the
/// standings exact even when the city and unit records are intentionally
/// fog-limited. Fields the host could not read (`-1`) or an older export never
/// sent (NaN) leave the model's own derivation in place.
fn apply_rival_public_economy(
    game: &mut crate::game::Game,
    owner: usize,
    rival: &StateRival,
    unmapped: &mut Vec<String>,
) {
    let known = |value: f64| value.is_finite() && value >= 0.0;
    if let Some(civ6) = rival.government.as_deref() {
        match civvis_node_name(&game.rules.governments, civ6, "GOVERNMENT_") {
            Some(government) => game.players[owner].government = Some(government),
            None if !unmapped.iter().any(|entry| entry == civ6) => unmapped.push(civ6.to_string()),
            None => {}
        }
    }
    apply_observed_age(
        &mut game.players[owner],
        rival.heroic_golden_age,
        rival.golden_age,
        rival.dark_age,
    );
    apply_public_empire_stats(game, owner, &rival.public_stats);
    let count = |value: f64| {
        (value.is_finite() && value >= 0.0 && value <= usize::MAX as f64)
            .then(|| value.round() as usize)
    };
    {
        let observed = Arc::make_mut(&mut game.observed_public_empire_stats)
            .entry(owner)
            .or_default();
        // The science lane's own number (`GetNumTechsResearched`) where it
        // crossed, else the counted loop an older mod sends; they agree.
        observed.techs = rival
            .techs_researched
            .filter(|value| *value >= 0)
            .map(|value| value as usize)
            .or_else(|| count(rival.techs));
        observed.civics = count(rival.civics);
        observed.cities_following_religion = rival
            .cities_following_religion
            .filter(|value| *value >= 0)
            .map(|value| value as usize);
        observed.military_no_treasury = rival
            .military_no_treasury
            .filter(|value| value.is_finite() && *value >= 0.0);
        observed.tourism_per_turn = known(rival.tourism).then_some(rival.tourism);
        // World Rankings owns the science-race distance, target, and current
        // rate. Do not derive the rate from a laser-project count: repeatable
        // Terrestrial stations contribute only while the host has them powered.
        observed.science_victory_points =
            known(rival.science_victory_points).then_some(rival.science_victory_points);
        observed.science_victory_points_per_turn = known(rival.science_victory_points_per_turn)
            .then_some(rival.science_victory_points_per_turn);
        observed.science_victory_points_needed = (rival.science_victory_points_needed.is_finite()
            && rival.science_victory_points_needed > 0.0)
            .then_some(rival.science_victory_points_needed);
        if known(rival.tourism) {
            Arc::make_mut(&mut game.observed_tourism_per_turn).insert(owner, rival.tourism);
        } else {
            Arc::make_mut(&mut game.observed_tourism_per_turn).remove(&owner);
        }
        // Like `techs`/`civics`: the observed table is rebuilt from each
        // snapshot (`apply_observed_host_metrics` clears it), so absent or
        // refused reads honestly say None for THIS snapshot. The durable
        // record is the player's `science_projects` below.
        observed.foreign_tourists = count(rival.foreign_tourists);
        observed.domestic_tourists = count(rival.domestic_tourists);
    }
    // The rival's majority religion, by the CIVVIS name its founded religion
    // takes (`civvis_religion_name`): `Game::majority_religion_of` reads it
    // before counting the cities the board holds, so the religion lane of the
    // victory tracker (`victory_races`, `religious_conversion_tally`) counts
    // a rival converted by cities the seat has never seen.
    if let Some(name) = rival.religion.as_deref().and_then(civvis_religion_name) {
        Arc::make_mut(&mut game.observed_majority_religion).insert(owner, name);
    }
    // Our draw from this rival, keyed the way `visiting_tourists_from` asks:
    // (tourism source = us, where the tourists come from = them).
    if let Some(tourists) = rival.tourists_visiting_us.filter(|value| *value >= 0) {
        Arc::make_mut(&mut game.observed_visiting_tourists).insert((0, owner), tourists);
    }
    // Era Score on the rival's own seat, the slot `apply_player_ages` fills
    // for ours; the standings (`obs.rs`) show it per player.
    if let Some(score) = rival.era_score.filter(|value| *value >= 0) {
        game.players[owner].era_score = score;
    }
    // The rival's space-race milestones land on its own player record, exactly
    // as the local seat's do — `rival_victory_pressure_with_culture` reads
    // `player.science_projects`, so the science lane of the victory tracker
    // sees the host's truth instead of an empty reconstruction.
    if let Some(projects) =
        completed_strategic_projects(rival.science_projects.as_deref(), unmapped)
    {
        game.players[owner].science_projects = projects;
    }
    if known(rival.gold) {
        game.players[owner].gold = rival.gold;
    }
    // Net income is legitimately negative, so like our own seat's
    // `gold_per_turn` any finite figure is taken (a refused read's -1 is
    // indistinguishable from a real -1 net; an older export is NaN).
    if rival.gold_per_turn.is_finite() {
        game.players[owner].gold_per_turn = rival.gold_per_turn;
    }
    if known(rival.faith) {
        game.players[owner].faith = rival.faith;
    }
    let stats = &rival.public_stats;
    if !known(rival.science)
        && !known(rival.culture)
        && !stats.food.is_some_and(known)
        && !stats.production.is_some_and(known)
        && !known(rival.faith_per_turn)
    {
        Arc::make_mut(&mut game.observed_yield_adjustments).remove(&owner);
        return;
    }
    let mut derived = crate::rules::Yields::default();
    for cid in game.player_city_ids(owner) {
        derived.add(game.city_yields(cid));
    }
    derived.add(game.player_yield_extras(owner));
    derived.add(game.arena_side_yields(owner));
    let mut adjustment = crate::rules::Yields::default();
    if let Some(food) = stats.food.filter(|value| known(*value)) {
        adjustment.food = food - derived.food;
    }
    if let Some(production) = stats.production.filter(|value| known(*value)) {
        adjustment.production = production - derived.production;
    }
    if known(rival.science) {
        adjustment.science = rival.science - derived.science;
    }
    if known(rival.culture) {
        adjustment.culture = rival.culture - derived.culture;
    }
    if known(rival.faith_per_turn) {
        adjustment.faith = rival.faith_per_turn - derived.faith;
    }
    Arc::make_mut(&mut game.observed_yield_adjustments).insert(owner, adjustment);
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
/// `seat_of_host` intentionally contains only players whose identity is known
/// to the mirror (plus mapped city-states). That is the right map for most
/// host data, but it made a numeric Congress target such as `"1"` disappear
/// even though the same Congress export had already listed player 1's
/// standing. Use the same deterministic ascending-player assignment as
/// [`apply_congress_dvp`], without changing the global map and accidentally
/// assigning an unseen major to a city-state slot in unrelated systems.
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
    for (seat, entry) in (state.rivals.len() + 1..major_seats).zip(unmet) {
        congress_seats.insert(entry.player, seat);
    }
    congress_seats
}

/// Put the host's binding World Congress resolutions on the board.
///
/// The model has its own Congress and simulates one when it plans ahead, but on
/// a mirrored board the host's is the one in force: Trade Policy A on this seat
/// (run civvis-20260816T200454Z, t82-101) paid +4 Gold per incoming foreign
/// route in Cumae and +1 route capacity, and Luxury Policy changes what every
/// city's Amenities are. An export without `resolutions` (older mod) leaves the
/// model's own list alone. Effects expire when the host says the next session
/// convenes (`congress_turns_left`), or a standard session length out if the
/// export does not say. Called BEFORE the host-to-model corrections are
/// measured, for the same reason as the age and Dedications: anything that
/// changes model yields must be on the board first.
fn apply_host_congress(
    game: &mut crate::game::Game,
    state: &StateSnapshot,
    seat_of_host: &std::collections::BTreeMap<usize, usize>,
    unmapped: &mut Vec<String>,
) {
    let Some(resolutions) = &state.resolutions else {
        return;
    };
    let turns_left = state
        .congress_turns_left
        .filter(|turns| *turns >= 0)
        .map(|turns| turns as u32)
        .unwrap_or_else(|| game.standard_duration(30));
    let expires = game.turn.saturating_add(turns_left).saturating_add(1);
    game.active_congress_effects.clear();
    let congress_seat_of_host = congress_seat_of_host(state, seat_of_host, game.players.len());
    for resolution in resolutions {
        match civvis_congress_effect(&game.rules, resolution, &congress_seat_of_host, expires) {
            Some(effect) => game.active_congress_effects.push(effect),
            None if is_known_congress_noop(resolution) => {}
            None => {
                let issue = format!(
                    "congress:{}:{}:{}",
                    resolution.kind, resolution.option, resolution.target
                );
                if !unmapped.contains(&issue) {
                    unmapped.push(issue);
                }
            }
        }
    }
}

/// Replace CIVVIS's inferred Governor state with the authoritative Firaxis roster.
///
/// Rebuilding cities and completed Civics alone is insufficient: title sources are
/// not all Civics, and neither appointments, promotions nor assignments are derived
/// facts. Without this pass CIVVIS spends the same titles on every reconstructed
/// turn and repeatedly appoints Governors already visible in the host game.
/// Carry the Great Person race across from the host.
///
/// Deliberately **not** part of `apply_governor_state`: that function returns
/// early when the host reports no governors, and the Great Person race has
/// nothing to do with governors. Bundling them would have made this arrive only
/// in games that already had a Governor appointed, which is precisely the kind
/// of silent conditional the mirror has been bitten by before.
fn apply_great_person_points(
    game: &mut crate::game::Game,
    state: &StateSnapshot,
    unmapped: &mut Vec<String>,
) {
    apply_live_great_person_activation_needs(game, state, unmapped);
    // Civilization VI names the class `GREAT_PERSON_CLASS_SCIENTIST`; CIVVIS
    // keys the same thing `scientist`. The nine classes correspond one to one,
    // so the translation is the suffix, lowercased — and an unrecognised class
    // is reported rather than dropped, because a new expansion adding one is
    // exactly the case where silence would be wrong.
    if let Some(points) = state.great_person_points.as_ref() {
        let mut gpp = BTreeMap::new();
        for (class, total) in points {
            match class.strip_prefix("GREAT_PERSON_CLASS_") {
                Some(kind) if !kind.is_empty() => {
                    gpp.insert(kind.to_ascii_lowercase(), *total);
                }
                _ => {
                    let issue = format!("great_person_class:{class}");
                    if !unmapped.contains(&issue) {
                        unmapped.push(issue);
                    }
                }
            }
        }
        game.players[0].gpp = gpp;
    }
    // The live recruit cost lands on CIVVIS's idea of the class's current
    // person via `great_person_offer_costs`, which `gp_cost` consults before
    // its market formula. The two games disagree about WHO is on offer, but
    // the number that gates the recruit decision — cost minus banked points —
    // is the live game's, which is the one the order will be judged by.
    if let Some(costs) = state.great_person_costs.as_ref() {
        for (class, cost) in costs {
            let Some(kind) = class.strip_prefix("GREAT_PERSON_CLASS_") else {
                let issue = format!("great_person_cost_class:{class}");
                if !unmapped.contains(&issue) {
                    unmapped.push(issue);
                }
                continue;
            };
            let kind = kind.to_ascii_lowercase();
            let id = game
                .current_great_person(&kind)
                .map(|(id, _)| id.to_string());
            if let Some(id) = id {
                game.great_person_offer_costs.insert(id, *cost);
            }
        }
    }
    // Which classes have nobody left to recruit anywhere on the host's
    // timeline — the last Great Scientist claimed by anyone — because
    // Firaxis pays such a class's points out as Faith from then on. The mod
    // says so outright (`great_person_exhausted`); before that field, the
    // cost map named every class with an unclaimed entry, so a class with
    // points and no cost was the same answer — except on the turn every
    // class is gone, when the map is `nil` and says nothing. Carry the host's
    // roster rather than CIVVIS's, whose named list is not the host's; `None`
    // on an older export keeps the engine's own answer.
    let kind_of = |class: &str| {
        class
            .strip_prefix("GREAT_PERSON_CLASS_")
            .filter(|kind| !kind.is_empty())
            .map(str::to_ascii_lowercase)
    };
    game.players[0].live_great_person_exhausted = match (
        state.great_person_exhausted.as_ref(),
        state.great_person_costs.as_ref(),
    ) {
        (Some(exhausted), _) => Some(
            exhausted
                .iter()
                .filter_map(|class| kind_of(class))
                .collect(),
        ),
        (None, Some(costs)) => Some(
            state
                .great_person_points
                .iter()
                .flat_map(|points| points.keys())
                .filter(|class| !costs.contains_key(*class))
                .filter_map(|class| kind_of(class))
                .collect(),
        ),
        (None, None) => None,
    };
    apply_live_great_person_offer_blockers(game, state, unmapped);
}

/// Carry physical Great People with no currently legal use back into planning.
///
/// The unit-order bridge can already activate a person in place or walk them to
/// any host-provided activation plot. An empty plot list is the remaining hard
/// case: movement cannot solve it, and without this signal the production AI
/// never learns that a district, Great Work slot, Wonder, or eligible military
/// unit is now a prerequisite for consuming an asset the empire already owns.
fn apply_live_great_person_activation_needs(
    game: &mut crate::game::Game,
    state: &StateSnapshot,
    unmapped: &mut Vec<String>,
) {
    let mut needs = Vec::new();
    for unit in &state.units {
        let Some(person) = unit.great_person.as_ref() else {
            continue;
        };
        if person.can_activate {
            continue;
        }
        // A non-empty plot list is only proof of a *place*, not of a use:
        // `GetActivationHighlightPlots` highlights a cultural person's
        // district whether or not a compatible Great Work slot is free.
        // Seven Writers, Artists and Musicians stood on one Theater plot for
        // thirty-plus turns on run civvis-20260817T010950Z, unactivatable,
        // while this gate read their nine highlighted plots as "nothing to
        // build".
        //
        // The tiebreaker was the host's empire-wide empty-slot count, and it
        // was the wrong question — see `StateGreatPerson::slot_starved`. Nine
        // cultural people idled the WHOLE of run civvis-20260822T020434Z with
        // that count reading 24, 4 and 2 while every plot the host offered
        // them read `slot_open: false`. Ask instead whether this person can
        // reach a slot; zero empire-wide is still a need, and still counted.
        let slot_starved = person.slot_starved();
        if !person.activation_plots.is_empty() && !slot_starved {
            continue;
        }
        let kind = person
            .class
            .as_deref()
            .and_then(|class| class.strip_prefix("GREAT_PERSON_CLASS_"))
            .filter(|kind| !kind.is_empty())
            .map(str::to_ascii_lowercase)
            .or_else(|| {
                unit.kind
                    .strip_prefix("UNIT_GREAT_")
                    .filter(|kind| !kind.is_empty())
                    .map(str::to_ascii_lowercase)
            });
        let Some(kind) = kind else {
            let issue = format!("great_person_unit_class:{}", unit.kind);
            if !unmapped.contains(&issue) {
                unmapped.push(issue);
            }
            continue;
        };

        let required_district = person.required_district.as_deref().and_then(|required| {
            if required.eq_ignore_ascii_case("DISTRICT_CITY_CENTER") {
                return Some("city_center".to_string());
            }
            let district = civvis_node_name(&game.rules.districts, required, "DISTRICT_");
            if district.is_none() {
                let issue = format!("great_person_unit_district:{required}");
                if !unmapped.contains(&issue) {
                    unmapped.push(issue);
                }
            }
            district.map(|district| {
                game.district_family(crate::name::Name::new(&district))
                    .to_string()
            })
        });
        let required_missing_building = person
            .required_missing_building
            .as_deref()
            .filter(|required| !required.trim().is_empty())
            .and_then(|required| {
                let building = civvis_node_name(&game.rules.buildings, required, "BUILDING_");
                if building.is_none() {
                    let issue = format!("great_person_unit_building:{required}");
                    if !unmapped.contains(&issue) {
                        unmapped.push(issue);
                    }
                }
                building
            });
        let required_great_work = person
            .required_great_work
            .as_deref()
            .filter(|required| !required.trim().is_empty())
            .and_then(|required| {
                let work = great_person_required_work_kind(required);
                if work.is_none() {
                    let issue = format!("great_person_unit_work:{required}");
                    if !unmapped.contains(&issue) {
                        unmapped.push(issue);
                    }
                }
                work.map(str::to_string)
            });
        let individual = person
            .individual
            .as_deref()
            .and_then(|individual| individual.strip_prefix("GREAT_PERSON_INDIVIDUAL_"))
            .filter(|individual| !individual.is_empty())
            .map(str::to_ascii_lowercase);
        needs.push(crate::game::LiveGreatPersonActivationNeed {
            kind,
            individual,
            required_district,
            required_missing_building,
            required_great_work,
        });
    }
    game.players[0].live_great_person_activation_needs = needs;
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

fn apply_live_great_person_offer_blockers(
    game: &mut crate::game::Game,
    state: &StateSnapshot,
    unmapped: &mut Vec<String>,
) {
    let mut blockers = BTreeMap::new();
    let mut individuals = BTreeMap::new();
    let Some(offers) = state.great_person_offers.as_ref() else {
        // A persistent mirror may have received this field last turn from a
        // newer mod and omit it after a rollback. Never keep a stale live-only
        // refusal alive when the current host frame no longer knows it.
        game.players[0].live_great_person_offers = None;
        game.players[0].live_great_person_offer_individuals.clear();
        game.players[0].live_great_person_offer_blockers.clear();
        return;
    };
    let mut offered_classes = BTreeSet::new();

    for (class, offer) in offers {
        let Some(kind) = class
            .strip_prefix("GREAT_PERSON_CLASS_")
            .filter(|kind| !kind.is_empty())
            .map(str::to_ascii_lowercase)
        else {
            let issue = format!("great_person_offer_class:{class}");
            if !unmapped.contains(&issue) {
                unmapped.push(issue);
            }
            continue;
        };
        offered_classes.insert(kind.clone());
        if let Some(individual) = offer
            .individual
            .as_deref()
            .map(str::trim)
            .filter(|individual| !individual.is_empty())
            .map(|individual| {
                individual
                    .strip_prefix("GREAT_PERSON_INDIVIDUAL_")
                    .unwrap_or(individual)
                    .to_ascii_lowercase()
            })
        {
            individuals.insert(kind.clone(), individual);
        }
        let mut reasons = Vec::new();

        if let Some(required_district) = offer
            .required_district
            .as_deref()
            .filter(|district| !district.trim().is_empty())
        {
            // Check the exact host name first. This covers
            // `DISTRICT_CITY_CENTER`, which CIVVIS deliberately does not store
            // in `City::districts` because every city already owns one. Then
            // compare CIVVIS district families so a unique like Russia's
            // Lavra satisfies Firaxis's `DISTRICT_HOLY_SITE` prerequisite
            // without pretending the two literal names are equal.
            let required_family =
                civvis_node_name(&game.rules.districts, required_district, "DISTRICT_")
                    .map(|district| game.district_family(crate::name::Name::new(&district)));
            let active = (required_district.eq_ignore_ascii_case("DISTRICT_CITY_CENTER")
                && !state.cities.is_empty())
                || state.cities.iter().any(|city| {
                    city.districts.iter().any(|district| {
                        if !district.complete || district.pillaged {
                            return false;
                        }
                        district.kind.eq_ignore_ascii_case(required_district)
                            || required_family.is_some_and(|family| {
                                civvis_node_name(&game.rules.districts, &district.kind, "DISTRICT_")
                                    .is_some_and(|district| {
                                        game.district_family(crate::name::Name::new(&district))
                                            == family
                                    })
                            })
                    })
                });
            if !active {
                if required_family.is_none() {
                    let issue = format!("great_person_offer_district:{required_district}");
                    if !unmapped.contains(&issue) {
                        unmapped.push(issue);
                    }
                }
                reasons.push(format!("requires an active {required_district}"));
            }
        }

        if let Some(required_missing_building) = offer
            .required_missing_building
            .as_deref()
            .filter(|building| !building.trim().is_empty())
        {
            match civvis_node_name(
                &game.rules.buildings,
                required_missing_building,
                "BUILDING_",
            ) {
                Some(required_building) => {
                    let missing = state.cities.iter().all(|city| {
                        !city.buildings.iter().any(|building| {
                            civvis_node_name(&game.rules.buildings, building, "BUILDING_")
                                .is_some_and(|building| {
                                    game.building_is_family(
                                        crate::name::Name::new(&building),
                                        crate::name::Name::new(&required_building),
                                    )
                                })
                        })
                    });
                    if !missing {
                        reasons.push(format!(
                            "requires a city without {required_missing_building}"
                        ));
                    }
                }
                None => {
                    let issue = format!("great_person_offer_building:{required_missing_building}");
                    if !unmapped.contains(&issue) {
                        unmapped.push(issue);
                    }
                    reasons.push(format!(
                        "requires an unmapped missing building {required_missing_building}"
                    ));
                }
            }
        }

        if let Some(required_great_work) = offer
            .required_great_work
            .as_deref()
            .filter(|work| !work.trim().is_empty())
        {
            match great_person_required_work_kind(required_great_work) {
                Some(work) if live_great_work_offer_has_capacity(game, 0, work) => {}
                Some(work) => reasons.push(format!("requires an open {work} Great Work slot")),
                None => {
                    let issue = format!("great_person_offer_work:{required_great_work}");
                    if !unmapped.contains(&issue) {
                        unmapped.push(issue);
                    }
                    reasons.push(format!(
                        "requires an unmapped Great Work object {required_great_work}"
                    ));
                }
            }
        }

        if !reasons.is_empty() {
            let individual = offer
                .individual
                .as_deref()
                .filter(|individual| !individual.trim().is_empty())
                .unwrap_or(class);
            blockers.insert(
                kind,
                format!("the live {individual} offer {}", reasons.join("; ")),
            );
        }
    }
    game.players[0].live_great_person_offers = Some(offered_classes);
    game.players[0].live_great_person_offer_individuals = individuals;
    game.players[0].live_great_person_offer_blockers = blockers;
}

fn apply_governor_state(
    game: &mut crate::game::Game,
    state: &StateSnapshot,
    unmapped: &mut Vec<String>,
) {
    let Some(governors) = state.governors.as_deref() else {
        return;
    };

    let mut roster = crate::specmap::SpecMap::default();
    let mut assigned_own_cities = Vec::new();
    for observed in governors {
        let Some(name) = civvis_governor_name(&observed.kind) else {
            let issue = format!("{}:governor", observed.kind);
            if !unmapped.contains(&issue) {
                unmapped.push(issue);
            }
            continue;
        };
        let Some(spec) = game.rules.governors.get(name) else {
            let issue = format!("{}:governor_rules", observed.kind);
            if !unmapped.contains(&issue) {
                unmapped.push(issue);
            }
            continue;
        };
        let establish_turns = game.standard_duration(spec.establish_turns);
        let located_city = (observed.x >= 0 && observed.y >= 0)
            .then(|| game.city_at(crate::hex::offset_to_axial(observed.x, observed.y)))
            .flatten();
        if observed.city >= 0 && located_city.is_none() {
            let issue = format!(
                "{}:governor_city@{},{}",
                observed.kind, observed.x, observed.y
            );
            if !unmapped.contains(&issue) {
                unmapped.push(issue);
            }
        }
        let city = located_city.filter(|cid| {
            game.cities.get(cid).is_some_and(|target| {
                target.owner == 0
                    || (name == "amani"
                        && game.players[target.owner].is_minor
                        && !game.players[target.owner].is_barbarian)
            })
        });
        if located_city.is_some() && city.is_none() {
            let issue = format!("{}:invalid_governor_owner", observed.kind);
            if !unmapped.contains(&issue) {
                unmapped.push(issue);
            }
        }
        if let Some(cid) = city {
            if game.cities[&cid].owner == 0 {
                assigned_own_cities.push(cid);
            }
        }
        let assigned_turn = if city.is_none() {
            game.turn
        } else if observed.established {
            game.turn.saturating_sub(establish_turns)
        } else {
            game.turn
                .saturating_sub(observed.turns_on_site.max(0) as u32)
        };
        let mut promotions = std::collections::BTreeSet::new();
        for host_promotion in &observed.promotions {
            if civ6_governor_base_promotion(name) == Some(host_promotion.as_str()) {
                // Firaxis includes the Governor's appointment/base ability in
                // GetPromotions(). CIVVIS stores that ability on GovernorSpec,
                // outside GovernorState.promotions, so importing it would either
                // duplicate its effects or falsely report a known host fact as
                // unmapped.
                continue;
            }
            match civvis_governor_promotion(host_promotion) {
                Some(promotion) if spec.promotions.contains_key(promotion) => {
                    promotions.insert(promotion.to_string());
                }
                _ => {
                    let issue = format!("{}:governor_promotion", host_promotion);
                    if !unmapped.contains(&issue) {
                        unmapped.push(issue);
                    }
                }
            }
        }
        roster.insert(
            name.to_string(),
            crate::game::GovernorState {
                city,
                assigned_turn,
                disabled_until: game
                    .turn
                    .saturating_add(observed.neutralized_turns.max(0) as u32),
                promotions,
            },
        );
    }

    assigned_own_cities.sort_unstable();
    assigned_own_cities.dedup();
    game.players[0].governor_roster = roster;
    game.players[0].governors = assigned_own_cities;
    if let Some(spent) = state.governor_points_spent.filter(|value| *value >= 0) {
        game.players[0].governor_titles_spent = spent as usize;
    }
    if let Some(total) = state.governor_points.filter(|value| *value >= 0) {
        let civic_titles: usize = game.players[0]
            .civics
            .iter()
            .filter_map(|civic| game.rules.civics.get(civic))
            .map(|civic| civic.governor_title)
            .sum();
        let other = (total as usize).saturating_sub(civic_titles);
        if total as usize >= civic_titles {
            game.players[0]
                .counters
                .insert("district_governor_titles".to_string(), other as i64);
        } else {
            game.players[0].counters.remove("district_governor_titles");
            let issue = format!("governor_titles:{total}<civic:{civic_titles}");
            if !unmapped.contains(&issue) {
                unmapped.push(issue);
            }
        }
    }
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

/// Copy the host's victory checkboxes onto the mirrored game. Firaxis's
/// `technology`/`conquest` names map to CIVVIS's `science`/`domination`.
fn apply_seat_victories(game: &mut crate::game::Game, seat: &Seat) {
    let Some(v) = seat.victories.as_ref() else {
        return;
    };
    let conditions = &mut game.victory_conditions;
    if let Some(on) = v.technology {
        conditions.science = on;
    }
    if let Some(on) = v.culture {
        conditions.culture = on;
    }
    if let Some(on) = v.religious {
        conditions.religious = on;
    }
    if let Some(on) = v.diplomatic {
        conditions.diplomatic = on;
    }
    if let Some(on) = v.conquest {
        conditions.domination = on;
    }
    if let Some(on) = v.score {
        conditions.score = on;
    }
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

/// Give every seat the civilization Civilization VI is actually playing.
///
/// ⚠ MUST RUN BEFORE ANY CITY IS PLACED. `found_city_for` reads `players[pid].civ`
/// to name a city, so setting identity afterwards leaves the old roster's names on
/// the board — the visible half of the very bug this fixes.
fn apply_identity(game: &mut crate::game::Game, state: &StateSnapshot) -> Vec<String> {
    // ★★★★★ THE GAME SPEED CROSSED THE BRIDGE AND WAS THROWN AWAY, so every cost
    // CIVVIS reasoned about was DOUBLE the real one.
    //
    // The ladder plays `GAMESPEED_ONLINE`, the `seat` event carries it, and `Seat`
    // even deserializes it — and `grep -n game_speed src/mirror.rs` answered ZERO,
    // so a mirrored game kept `GameSpeed::Standard`, the `#[default]`. Online is
    // `cost_percent: 50`.
    //
    // What that scales is not a corner: `Game::game_speed` multiplies tech cost,
    // civic cost, the growth threshold, item/production cost and turn durations —
    // `src/lib.rs` has a test called `game_speed_scales_every_cost`. So CIVVIS
    // planned against a world where a settler, a tech and a district each took
    // twice as long as the game would actually charge, on every turn of every run.
    //
    // Same shape as the districts that were carried on `StateCity` and never
    // written onto a city: the field crossed, and nothing read it.
    //
    // ⚠ DIFFICULTY IS DELIBERATELY NOT SET HERE. `Seat` carries it too, but a
    // mirrored rival's strength ALREADY includes its handicap — that is what the
    // export measured — so applying the difficulty on top would count the bonus
    // twice. Speed has no such double-count: it is a cost curve, not a bonus.
    if let Some(speed) = civvis_game_speed(&state.seat.speed) {
        game.game_speed = speed;
    }
    apply_seat_victories(game, &state.seat);
    Arc::make_mut(&mut game.observed_leader_types).clear();
    if !state.seat.leader.is_empty() {
        Arc::make_mut(&mut game.observed_leader_types).insert(0, state.seat.leader.clone());
    }
    for (index, rival) in state.rivals.iter().enumerate() {
        if !rival.leader.is_empty() {
            Arc::make_mut(&mut game.observed_leader_types).insert(index + 1, rival.leader.clone());
        }
    }
    let mut unmapped = Vec::new();
    let mut known: std::collections::BTreeMap<usize, &'static str> = Default::default();
    let mut note = |seat: usize, civ6: &str, unmapped: &mut Vec<String>| {
        if civ6.is_empty() || seat >= game.players.len() {
            return;
        }
        match civvis_civ_name(civ6) {
            Some(name) => {
                known.insert(seat, name);
            }
            None => unmapped.push(civ6.to_string()),
        }
    };
    note(0, &state.seat.civ, &mut unmapped);
    // Rival entities are deliberately compacted into seats 1..n in export order;
    // `rival.player` is the original Firaxis id and is used only when translating
    // an order back to the host. Identity must follow the compacted entity owner,
    // otherwise the board gives their cities one civilization and their player
    // record another.
    for (index, rival) in state.rivals.iter().enumerate() {
        note(index + 1, &rival.civ, &mut unmapped);
    }
    for (minor, seat) in minor_actor_assignments(game, state)
        .into_iter()
        .filter(|(minor, _)| minor.is_city_state())
    {
        match mirrored_city_state_name(game, minor) {
            Some(name) => game.players[seat].civ = name,
            None if !minor.civ.is_empty() => unmapped.push(minor.civ.clone()),
            None => {}
        }
    }
    for (&seat, &name) in &known {
        game.players[seat].civ = name.to_string();
    }

    // ⚠ MOVE THE DEFAULTS OUT OF THE WAY. Seats we have not met keep CIVVIS's roster
    // name, and that roster can collide with one we just learned: Civilization VI
    // named player 1 Greece, `CIV_NAMES[2]` is also Greece, and the standings table
    // showed TWO Greeces. A duplicate is worse than an unknown, because it looks like
    // a real second civilization rather than a seat nobody has met yet.
    let taken: std::collections::BTreeSet<String> =
        known.values().map(|name| name.to_string()).collect();
    let mut spare = crate::game::CIV_NAMES
        .iter()
        .filter(|name| !taken.contains(**name));
    for seat in 0..game.players.len() {
        if known.contains_key(&seat) || game.players[seat].is_minor {
            continue;
        }
        // Free Cities and the barbarians are seats, but they are not civilizations
        // and renaming them would invent a rival that does not exist.
        if !taken.contains(&game.players[seat].civ) {
            continue;
        }
        if let Some(name) = spare.next() {
            game.players[seat].civ = name.to_string();
        }
    }
    unmapped
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
    "religion_next",
    "religion_turns",
    "pantheon_active",
    "districts",
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
    "queue",
    "food",
    "loyalty_per_turn",
    "falls_to",
    "x",
    "y",
    "pop",
    "capital",
    "defense",
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
    "promotions",
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
    "upgrade_blocked_reason",
    "maintenance",
    "religious_strength",
    "max_moves",
    "activity",
    "spy_operation",
    "spy_operation_end_turn",
    "spy_missions_available",
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

/// The field names `state_schema_gaps` will accept for one struct, extracted from
/// this file's own source.
///
/// ⚠ Only used by the drift test below. It is deliberately crude — `pub name:` lines
/// inside the named `pub struct` block — because the alternative was another
/// hand-maintained list, which is the failure it exists to catch.
#[cfg(test)]
fn declared_fields(struct_name: &str) -> Vec<String> {
    let source = include_str!("mirror.rs");
    let head = format!("pub struct {struct_name} {{");
    let start = source
        .find(&head)
        .expect("the struct is declared in this file");
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
        "research_progress", "civic", "civic_progress", "government", "used_governments",
        "pantheon",
        "founded_religion", "founded_religions", "religion_beliefs",
        "taken_religion_beliefs", "religions", "prophet_pending",
        "policies", "policy_slots", "gold", "gold_per_turn",
        "unit_maintenance_total", "building_maintenance_total", "district_maintenance_total",
        "faith", "faith_per_turn",
        "faith_sources", "science",
        "culture", "public_stats", "score", "dvp", "favor", "congress_dvp",
        "spy_capacity",
        // Consumed since the stockpile import (`seat.strategic_resources`) and
        // absent here until 2026-08-26, so every live state record filed a
        // `schema:state.strategic_resources` gap that was not one.
        "strategic_resources",
        "foreign_tourists", "domestic_tourists",
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
        "golden_age_threshold", "world_era", "dark_age", "golden_age",
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
    const RIVAL: &[&str] = &[
        "player",
        "civ",
        "leader",
        "government",
        "dark_age",
        "golden_age",
        "heroic_golden_age",
        "can_declare",
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
        // Rival victory progress as the shipped World Rankings screen shows it.
        // `the_schema_allowlists_cover_every_declared_field` fails if a new
        // StateRival field is missing here.
        "science_projects",
        "science_victory_points",
        "science_victory_points_per_turn",
        "science_victory_points_needed",
        "foreign_tourists",
        "domestic_tourists",
        // Both border fields: `open_borders` had crossed since the buy lane
        // shipped and filed `schema:rival.open_borders` on every live turn.
        "open_borders",
        "enforces_borders",
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
        keys(rival, RIVAL, "rival", &mut gaps);
        public_stats(rival.get("public_stats"), "rival.public_stats", &mut gaps);
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
    let raw = std::fs::read_to_string(path).ok()?;
    let mut best: Option<StateSnapshot> = None;
    // Identity rides in the `seat` event, which is emitted once at startup rather
    // than every turn, so it is collected separately and merged into whichever
    // state wins. Newest-wins here too: a run that reloads re-emits it.
    let mut seat: Option<Seat> = None;
    for line in raw.lines() {
        if line.contains("\"seat\"") {
            if let Ok(found) = serde_json::from_str::<Seat>(line) {
                if !found.civ.is_empty() {
                    seat = Some(found);
                }
            }
        }
        if !line.contains("\"state\"") {
            continue;
        }
        let Ok(state) = state_from_json(line) else {
            continue;
        };
        match turn {
            Some(want) if state.turn != want => continue,
            _ => {}
        }
        if best.as_ref().map(|b| state.turn >= b.turn).unwrap_or(true) {
            best = Some(state);
        }
    }
    if let (Some(state), Some(seat)) = (best.as_mut(), seat) {
        state.seat = seat;
    }
    if let Some(state) = best.as_mut() {
        state.refused_sites = refused_sites_of_kind_through(path, "found_refused", turn);
        state.refused_improves = refused_sites_of_kind_through(path, "improve_refused", turn);
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
        // These two unique unit specifications are still absent from CIVVIS.
        // Firaxis's own UnitReplaces table names their exact stock role, which
        // is preferable to deleting a visible hostile from the board entirely.
        "scottish_highlander" => "ranger".to_string(),
        "korean_hwacha" => "field_cannon".to_string(),
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
pub(crate) fn grow_frontier(game: &mut crate::game::Game, snapshot: &Snapshot, depth: u32) {
    // Recompute rather than accumulate. As the revealed edge advances, yesterday's
    // frontier may lie beyond today's configured depth.
    for tile in game.map.tiles.values_mut() {
        if tile.terrain == "unknown" {
            tile.assumed_traversable = false;
            tile.assumed_navigable = false;
        }
    }
    if depth == 0 {
        return;
    }
    grow_frontier_from(game, snapshot, depth, false);
    grow_frontier_from(game, snapshot, depth, true);
}

/// One domain's half of [`grow_frontier`]: seed from the revealed tiles of that
/// domain (`water` selects revealed water, else revealed land) and mark the
/// matching prior on the unknown tiles reached within `depth` rings.
fn grow_frontier_from(game: &mut crate::game::Game, snapshot: &Snapshot, depth: u32, water: bool) {
    let width = snapshot.width.max(1);
    let height = snapshot.height.max(1);
    // Grown one ring at a time so depth means "tiles beyond what we have seen",
    // and so each ring is seeded only by ground the previous ring established.
    //
    // ⚠ ONE RING WAS NOT ENOUGH, and the failure was quiet: CIVVIS could only ever
    // aim one tile past its own border, and the map refreshes on a cadence, so
    // exploration crawled. Measured on civvis-20260730T120107Z: revealed plots went
    // 25 -> 109 across 64 turns, `met = 1`, and **zero** rival cities ever seen —
    // so the army had nothing to attack and domination was unreachable. The
    // heuristic path, which hands scouts to AUTOMATE_EXPLORE, had 468 by t190.
    // `seen` is the seed set — revealed land, or revealed passable water — and then
    // every unknown tile the growth has already claimed.
    let mut seen: std::collections::BTreeSet<crate::Pos> = std::collections::BTreeSet::new();
    for y in 0..height {
        for x in 0..width {
            if !snapshot.is_revealed((x, y)) {
                continue;
            }
            let pos = crate::hex::offset_to_axial(x, y);
            if game
                .map
                .get(pos)
                .map(|tile| {
                    !game.rules.is_unknown(tile)
                        && game.rules.is_water(tile) == water
                        && (!water || game.rules.is_passable(tile))
                })
                .unwrap_or(false)
            {
                seen.insert(pos);
            }
        }
    }
    let mut edge: Vec<crate::Pos> = seen.iter().copied().collect();
    for _ in 0..depth {
        let mut next_edge: Vec<crate::Pos> = Vec::new();
        for pos in &edge {
            for neighbour in crate::hex::neighbors(*pos) {
                let (nx, ny) = crate::hex::axial_to_offset(neighbour.0, neighbour.1);
                if nx < 0 || ny < 0 || nx >= width || ny >= height {
                    continue;
                }
                // Never mark ground the seat has actually seen as speculative.
                if snapshot.is_revealed((nx, ny)) || seen.contains(&neighbour) {
                    continue;
                }
                if let Some(tile) = game.map.tiles.get_mut(&neighbour) {
                    debug_assert_eq!(tile.terrain.as_str(), "unknown");
                    if water {
                        tile.assumed_navigable = true;
                    } else {
                        tile.assumed_traversable = true;
                    }
                }
                seen.insert(neighbour);
                next_edge.push(neighbour);
            }
        }
        if next_edge.is_empty() {
            break;
        }
        edge = next_edge;
    }
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
    let mut refused: std::collections::BTreeSet<crate::Pos> = Default::default();
    let Ok(raw) = std::fs::read_to_string(path) else {
        return refused;
    };
    for line in raw.lines() {
        if !line.contains(kind) {
            continue;
        }
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
        refused.insert(crate::hex::offset_to_axial(x as i32, y as i32));
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
    let Ok(raw) = std::fs::read_to_string(path) else {
        return refused;
    };
    for line in raw.lines() {
        if !line.contains("promotion_refused") {
            continue;
        }
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

/// Strikes the host refused on exactly `turn`, keyed by Civilization VI unit id
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
    let Ok(raw) = std::fs::read_to_string(path) else {
        return refused;
    };
    for line in raw.lines() {
        if !line.contains("range_attack_refused") && !line.contains("war_refused") {
            continue;
        }
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if !matches!(
            event.get("kind").and_then(|k| k.as_str()),
            Some("range_attack_refused" | "war_refused")
        ) {
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
    let Ok(raw) = std::fs::read_to_string(path) else {
        return previews;
    };
    for line in raw.lines() {
        if !line.contains("\"preview\"") {
            continue;
        }
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
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Default::default();
    };
    for line in raw
        .lines()
        .filter(|line| line.contains("trade_route_refused"))
    {
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

/// Translate fresh, host-approved district plots onto the reconstructed city ids.
///
/// A `build_no_plot` with `offered > 0` says the district is valid in this city,
/// but the direct CIVVIS order named a different coordinate.  The companion
/// `offered_plots` list is Firaxis's authoritative replacement candidate set.  It
/// is deliberately separate from [`blocked_districts_from`]: one is a negative
/// feedback signal and this is the positive way out of that same mismatch.
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

/// Translate recent host production refusals onto CIVVIS city ids and typed keys.
/// Translate host promotion refusals onto CIVVIS unit ids.
///
/// Keyed by unit AND promotion: a refusal is specific to both, and another unit of
/// the same kind may legitimately take a promotion this one cannot.
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
        let translated: std::collections::BTreeSet<crate::name::Name> = names
            .iter()
            .filter_map(|name| civvis_node_name(&rules.promotions, name, "PROMOTION_"))
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
    pub purchasable: BTreeMap<u32, BTreeMap<String, crate::game::HostPurchaseEntry>>,
    pub district_plots: BTreeMap<u32, BTreeMap<crate::name::Name, BTreeSet<crate::Pos>>>,
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
            let mut plots: BTreeMap<crate::name::Name, BTreeSet<crate::Pos>> = BTreeMap::new();
            for row in menu {
                let Some(key) = host_production_key(rules, &row.t, row.f) else {
                    continue;
                };
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
            if !plots.is_empty() {
                out.district_plots.insert(*cid, plots);
            }
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

/// The host's own count of strikes a FOREIGN unit has left this turn
/// (`Unit:GetAttacksRemaining()`, the shipped `SelectedUnit.lua:62` read),
/// applied after [`apply_unit_observation`] planted it. The seat's own units
/// have taken it onto `Unit::attacks_left` since the seat capability landed
/// (`LiveMirror::new`, `sync`); `hostiles[]`, `rivals[].units[]` and
/// `minors[].units[]` never carried the key, so an enemy that had already
/// struck stood on the board as one that could still strike. Every foreign
/// planting site — hostiles, rivals and city-states, both import paths —
/// calls this. An export without the key (every recording before 2026-09-01)
/// leaves the fresh-turn allowance `spawn_unit` gave, exactly as before; a
/// negative reading is the mod's "could not read" sentinel and is clamped to
/// none left, as the own-unit paths clamp it.
fn apply_foreign_unit_strikes(game: &mut crate::game::Game, uid: u32, unit: &StateUnit) {
    if let Some(attacks) = unit.attacks_remaining {
        if let Some(live) = game.units.get_mut(&uid) {
            live.attacks_left = attacks.max(0);
        }
    }
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
    let exported = unit.upgrade_to.is_some()
        || unit.upgrade_cost.is_some()
        || unit.upgrade_blocked_reason.is_some()
        || unit.maintenance.is_some()
        || unit.religious_strength.is_some()
        || unit.max_moves.is_some()
        || unit.activity.is_some()
        || unit.spy_operation.is_some()
        || unit.spy_operation_end_turn.is_some()
        || unit.spy_missions_available.is_some();
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
    Arc::make_mut(&mut game.host_unit_facts).insert(
        uid,
        crate::game::HostUnitFacts {
            civ6_id: Some(unit.id),
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
        },
    );
}

/// The host treasury's bill by source for the seat — see
/// `Game::host_maintenance`. A reading that is absent, non-finite or negative
/// is no reading, and a state with none of the three leaves the board billing
/// itself.
fn apply_host_maintenance(game: &mut crate::game::Game, state: &StateSnapshot) {
    let reading = |value: Option<f64>| value.filter(|value| value.is_finite() && *value >= 0.0);
    let bill = crate::game::HostMaintenance {
        units: reading(state.unit_maintenance_total),
        buildings: reading(state.building_maintenance_total),
        districts: reading(state.district_maintenance_total),
    };
    if bill.units.is_some() || bill.buildings.is_some() || bill.districts.is_some() {
        Arc::make_mut(&mut game.host_maintenance).insert(0, bill);
    } else {
        Arc::make_mut(&mut game.host_maintenance).remove(&0);
    }
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

fn seat_live_spies(game: &mut crate::game::Game) {
    game.spies.retain(|_, spy| spy.owner != 0);
    let turn = game.turn;
    let live: Vec<LiveSpySeat> = game
        .units
        .values()
        .filter(|unit| unit.owner == 0 && unit.kind == "spy")
        .map(|unit| {
            let city = game
                .cities
                .iter()
                .find(|(_, city)| city.pos == unit.pos)
                .map(|(id, _)| *id);
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
            LiveSpySeat {
                id: unit.id,
                level: (unit.level - 1).max(0) as i64,
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
    let Ok(raw) = std::fs::read_to_string(path) else {
        return unavailable;
    };
    for line in raw.lines().filter(|line| line.contains("build_no_plot")) {
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
    let Ok(raw) = std::fs::read_to_string(path) else {
        return BTreeMap::new();
    };
    let oldest = current_turn.saturating_sub(PRODUCTION_REFUSAL_TTL);
    // Keyed by (player, item); the newest refusal turn for that pair and the
    // plots it named, if it named any.
    type Newest = BTreeMap<(i64, String), (u64, Option<BTreeSet<crate::Pos>>)>;
    let mut newest: Newest = BTreeMap::new();
    for line in raw.lines().filter(|line| line.contains("build_no_plot")) {
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
    let Ok(raw) = std::fs::read_to_string(path) else {
        return refused;
    };
    for line in raw.lines() {
        if !line.contains("build_no_plot") {
            continue;
        }
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
    let Ok(raw) = std::fs::read_to_string(path) else {
        return refused;
    };
    let oldest = current_turn.saturating_sub(PRODUCTION_REFUSAL_TTL);
    for line in raw.lines() {
        if !line.contains(event_kind) {
            continue;
        }
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
    let Ok(raw) = std::fs::read_to_string(path) else {
        return refused;
    };
    for line in raw.lines() {
        if !line.contains("obsolete_") {
            continue;
        }
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
    let Ok(raw) = std::fs::read_to_string(path) else {
        return refused;
    };
    for line in raw.lines() {
        if !line.contains("taken_BELIEF_") {
            continue;
        }
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
    let raw = std::fs::read_to_string(path).ok()?;
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

/// Put Firaxis's Era Score and age thresholds on the reconstructed board.
///
/// Without this the age CIVVIS reasons about is `Player::default`'s: era score
/// 0 against a golden threshold of 26 and a normal threshold of 12, on turn 1
/// and on turn 200 alike. Both age-reading decisions — `choose_dedications` and
/// the Dark Age wildcard filter in `ai/advanced.rs` — then run against a
/// standing fiction rather than against Rome's actual standing.
///
/// Every field is optional and a negative value means "the host did not
/// answer", so a build whose `Game.GetEras()` is missing a getter leaves that
/// field at whatever the board already held instead of writing a lie into it. A
/// real era score of 0 is ordinary on turn 1 and is applied.
fn apply_observed_age(
    player: &mut crate::game::Player,
    heroic_golden_age: Option<bool>,
    golden_age: Option<bool>,
    dark_age: Option<bool>,
) {
    // Heroic outranks Golden. An all-false response is the host's explicit
    // Normal answer; an absent field belongs to an older control mod and must
    // leave the previous observation intact.
    match (heroic_golden_age, golden_age, dark_age) {
        (Some(true), _, _) => player.age = "heroic".to_string(),
        (_, Some(true), _) => player.age = "golden".to_string(),
        (_, _, Some(true)) => player.age = "dark".to_string(),
        (Some(false), Some(false), Some(false)) => player.age = "normal".to_string(),
        _ => {}
    }
}

fn apply_player_ages(game: &mut crate::game::Game, state: &StateSnapshot) {
    let player = &mut game.players[0];
    if let Some(score) = state.era_score.filter(|value| *value >= 0) {
        player.era_score = score;
    }
    if let Some(baseline) = state.era_score_baseline.filter(|value| *value >= 0) {
        player.era_score_baseline = baseline;
    }
    // Firaxis's Dark Age threshold IS this codebase's normal-age threshold: the
    // score at or above which the next age is Normal rather than Dark.
    if let Some(normal) = state.normal_age_threshold.filter(|value| *value >= 0) {
        player.normal_age_threshold = normal;
    }
    if let Some(golden) = state.golden_age_threshold.filter(|value| *value >= 0) {
        player.golden_age_threshold = golden;
    }
    // ★★★★ THE AGE ITSELF. The three flags crossed and were read by nothing:
    // `Player::age` stayed at its "normal" default on every mirrored board, so
    // `dedication_active` was false for the whole of every live Golden Age and
    // no Dedication ever paid — the host's production ledger showing "+10 from
    // Campus" (Heartbeat of Steam) against a model paying nothing was the
    // largest gap of run civvis-20260816T132247Z.
    apply_observed_age(
        player,
        state.heroic_golden_age,
        state.golden_age,
        state.dark_age,
    );
    // And what it pays: the host's active Commemorations, by their CIVVIS ids.
    // An older export (None) leaves the model's own list alone.
    if let Some(active) = &state.dedications {
        player.dedications.clear();
        for name in active {
            if let Some(dedication) = civvis_dedication_name(name) {
                player.dedications.insert(dedication.to_string());
            }
        }
    }
    if let Some(choices) = state.dedication_choices.filter(|value| *value >= 0) {
        player.dedication_choices = choices as usize;
    }
    if let Some(era) = state.world_era.filter(|value| *value >= 0) {
        // `ERA_NAMES` bounds the model's era ladder; a build that reports an era
        // past its end is clamped rather than allowed to index out of range.
        game.world_era = (era as usize).min(crate::rules::ERA_NAMES.len() - 1);
    }
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

fn apply_player_religion(
    game: &mut crate::game::Game,
    state: &StateSnapshot,
    unmapped: &mut Vec<String>,
) {
    game.players[0].prophet_pending = state.prophet_pending;
    let local_religion = state
        .founded_religion
        .as_deref()
        .and_then(civvis_religion_name);
    game.players[0].religion = local_religion.clone();

    // Religion availability is capped by the number already founded worldwide.
    // Preserve exact Firaxis names on otherwise-unidentified major seats so
    // `religions_founded()` sees the same count without claiming knowledge of an
    // unmet founder's civilization. Globally claimed beliefs are handled below.
    if !state.founded_religions.is_empty() {
        let foreign_seats = game
            .players
            .iter()
            .enumerate()
            .skip(1)
            .filter(|(_, player)| !player.is_minor && !player.is_barbarian)
            .map(|(seat, _)| seat)
            .collect::<Vec<_>>();
        for seat in &foreign_seats {
            game.players[*seat].religion = None;
        }
        let mut foreign = Vec::new();
        for civ6 in &state.founded_religions {
            if let Some(name) = civvis_religion_name(civ6) {
                if local_religion.as_ref() != Some(&name) && !foreign.contains(&name) {
                    foreign.push(name);
                }
            }
        }
        for (seat, name) in foreign_seats.into_iter().zip(foreign) {
            game.players[seat].religion = Some(name);
        }
    }

    let mut local = Vec::new();
    for civ6 in &state.religion_beliefs {
        match civvis_belief_name(&game.rules, civ6) {
            Some(name) if !local.contains(&name) => local.push(name),
            Some(_) => {}
            None => {
                let gap = format!("{civ6}:belief");
                if !unmapped.contains(&gap) {
                    unmapped.push(gap);
                }
            }
        }
    }
    game.players[0].religion_beliefs = local.clone();

    // With the host's per-religion list, every religion's beliefs sit on its
    // founder's seat and nowhere else: a city following Catholicism then reads
    // exactly Catholicism's follower beliefs (`city_religion_belief_effect`),
    // whoever founded it and whether or not the mirror has met them. Rivals
    // hold seats 1..n in the order the host lists them (see the rival loop in
    // `reconstruct`), so a founder id maps straight onto a seat; a founder the
    // seat has not met keeps the anonymous seat `founded_religions` gave the
    // religion above. Every belief named here is also globally claimed, which
    // is all `taken_religion_beliefs` ever said.
    if !state.religions.is_empty() {
        let seat_of_host: BTreeMap<i64, usize> = state
            .rivals
            .iter()
            .enumerate()
            .map(|(index, rival)| (rival.player as i64, index + 1))
            .collect();
        let foreign_seats: Vec<usize> = game
            .players
            .iter()
            .enumerate()
            .skip(1)
            .filter(|(_, player)| !player.is_minor && !player.is_barbarian)
            .map(|(seat, _)| seat)
            .collect();
        let mut translated: Vec<(String, Option<usize>, Vec<String>)> = Vec::new();
        for religion in &state.religions {
            let Some(name) = civvis_religion_name(&religion.religion) else {
                continue;
            };
            if translated.iter().any(|(known, _, _)| *known == name) {
                continue;
            }
            let mut beliefs = Vec::new();
            for civ6 in &religion.beliefs {
                match civvis_belief_name(&game.rules, civ6) {
                    Some(belief) if !beliefs.contains(&belief) => beliefs.push(belief),
                    Some(_) => {}
                    None => {
                        let gap = format!("{civ6}:belief");
                        if !unmapped.contains(&gap) {
                            unmapped.push(gap);
                        }
                    }
                }
            }
            let seat = if local_religion.as_ref() == Some(&name) {
                Some(0)
            } else {
                seat_of_host
                    .get(&religion.founder)
                    .copied()
                    .filter(|seat| foreign_seats.contains(seat))
            };
            translated.push((name, seat, beliefs));
        }
        // Founders the seat has met take their religion onto their own seat;
        // the rest go to whatever foreign seats are left, so the count the
        // engine gates Prophets on (`religions_founded`) stays the host's.
        for &seat in &foreign_seats {
            game.players[seat].religion = None;
            game.players[seat].religion_beliefs.clear();
        }
        let mut spare = foreign_seats
            .iter()
            .copied()
            .filter(|seat| !translated.iter().any(|(_, taken, _)| *taken == Some(*seat)))
            .collect::<Vec<_>>()
            .into_iter();
        for (name, seat, beliefs) in translated {
            let seat = match seat {
                Some(seat) => seat,
                None => match spare.next() {
                    Some(seat) => seat,
                    None => continue,
                },
            };
            if seat != 0 {
                game.players[seat].religion = Some(name);
            }
            game.players[seat].religion_beliefs = beliefs;
        }
        return;
    }

    // Firaxis belief availability is global. One non-local seat can retain the
    // union without pretending we know which unseen founder owns which belief.
    if game.players.len() > 1 {
        let mut claimed_elsewhere = Vec::new();
        for civ6 in &state.taken_religion_beliefs {
            if let Some(name) = civvis_belief_name(&game.rules, civ6) {
                if !local.contains(&name) && !claimed_elsewhere.contains(&name) {
                    claimed_elsewhere.push(name);
                }
            }
        }
        game.players[1].religion_beliefs = claimed_elsewhere;
    }
}

/// Firaxis player id -> mirrored seat, for the two per-city player fields the
/// capture decision carries (`StateCity::captured_from`, `::original_owner`).
/// The same rule the war bond and `apply_territory` use: rivals take seats
/// `i + 1` in export order, city-states and the Free Cities actor take the
/// seats `minor_actor_assignments` gives them, the local player is seat 0.
fn host_capture_seats(
    game: &crate::game::Game,
    state: &StateSnapshot,
) -> std::collections::BTreeMap<i64, usize> {
    let mut seat_of: std::collections::BTreeMap<i64, usize> = Default::default();
    if state.seat.local_player >= 0 {
        seat_of.insert(i64::from(state.seat.local_player), 0);
    }
    for (index, rival) in state.rivals.iter().enumerate() {
        seat_of.insert(rival.player as i64, index + 1);
    }
    for (minor, seat) in minor_actor_assignments(game, state) {
        seat_of.insert(minor.player as i64, seat);
    }
    seat_of
}

/// ★★★ THE CAPTURE DECISION REACHES THE BOARD. `captured_from` is assigned
/// from every export — the host names the loser only while its own
/// keep/raze/liberate decision is open for this city (`GetNextCapturedCity()`,
/// `Popups/RazeCity.lua:71`), so an export without it CLEARS the flag and the
/// board stops asking. `original_owner` moves only when the founder sits on a
/// mirrored seat; an unmapped founder (never met, or no seat left) keeps the
/// board's default, under which `city_can_be_razed_by` and the liberate
/// clause both say no and only Keep is offered. A loser that maps to no seat
/// leaves the flag clear: the engine's `do_keep_city` pays the capture
/// rewards to `players[defeated]`, and a seat that is not there is a panic,
/// not a decision.
fn apply_city_capture(
    live: &mut crate::game::City,
    state: &StateCity,
    seat_of: &std::collections::BTreeMap<i64, usize>,
) {
    if let Some(founder) = state
        .original_owner
        .filter(|player| *player >= 0)
        .and_then(|player| seat_of.get(&player))
    {
        live.original_owner = *founder;
    }
    live.captured_from = state
        .captured_from
        .filter(|player| *player >= 0)
        .and_then(|player| seat_of.get(&player).copied())
        .filter(|seat| *seat != live.owner);
}

fn apply_city_religion(live: &mut crate::game::City, state: &StateCity) {
    // `City::pressure` is the model's only existing representation of a
    // conversion in progress.  The host gives us the more useful clock, but
    // not the individual pressure values, so retain the exact majority above
    // and add only a bounded warning signal for a flip that is close enough to
    // require a response.  A marker of 1.0 keeps a religionless city below its
    // 50-point atheist pressure and therefore cannot invent a majority; when a
    // city already has a majority, 60.0 is the smallest value that reaches
    // AdvancedAi's existing 60%-of-top-pressure defense threshold while the
    // observed majority remains at 100.0.
    const ACTIONABLE_CONVERSION_TURNS: i64 = 20;
    const RELIGIONLESS_CONVERSION_MARKER: f64 = 1.0;
    const MAJORITY_CONVERSION_MARKER: f64 = 60.0;

    let current = state.religion.as_deref().and_then(civvis_religion_name);
    live.pressure.clear();
    match current.as_ref() {
        Some(religion) => {
            live.atheist_pressure = 0.0;
            live.pressure.insert(religion.clone(), 100.0);
        }
        None => live.atheist_pressure = 50.0,
    }

    let Some(next) = state
        .religion_next
        .as_deref()
        .and_then(civvis_religion_name)
    else {
        return;
    };
    if current.as_deref() == Some(next.as_str())
        || state.religion_turns < 0
        || state.religion_turns > ACTIONABLE_CONVERSION_TURNS
    {
        return;
    }
    let marker = if current.is_some() {
        MAJORITY_CONVERSION_MARKER
    } else {
        RELIGIONLESS_CONVERSION_MARKER
    };
    live.pressure.insert(next, marker);
}

/// Apply a city's districts and wonders to both representations CIVVIS uses.
/// City collections drive yields; tile fields drive placement and Builder legality.
/// Give a mirrored city's Encampment the health Firaxis reports for it.
///
/// ★★★★★ WITHOUT THIS THE CITY BUILDS NOTHING, FOREVER. `City::encampment_hp`
/// defaults to 0 and nothing ever set it on a reconstructed board.
/// `Game::can_produce` gates `repair_encampment` on `encampment_hp < 100`, so
/// that test passed permanently for every city holding an Encampment: the AI
/// queued the repair every turn, `civvis_orders` correctly declined to translate
/// a project Civilization VI does not have, the order was discarded, and no
/// other production was chosen for that city. Two of five cities in live run
/// `civvis-20260810T040916Z` produced nothing from turn 67 to turn 188 this way.
///
/// The **default when the host does not answer is full health, not zero** — that
/// asymmetry is the whole point. A wrong "healthy" costs one skipped repair; a
/// wrong "destroyed" costs the city's entire production for the rest of the game.
fn apply_encampment_health(game: &mut crate::game::Game, state: &StateCity, cid: u32) {
    let encampment = state
        .districts
        .iter()
        .find(|district| district.kind.eq_ignore_ascii_case("DISTRICT_ENCAMPMENT"));
    // Read the wall maximum before taking the mutable borrow below.
    let Some(max_wall) = game
        .cities
        .get(&cid)
        .map(|city| game.city_max_wall_hp(city))
    else {
        return;
    };
    let Some(city) = game.cities.get_mut(&cid) else {
        return;
    };
    let Some(encampment) = encampment else {
        // No Encampment: `can_produce` already refuses on the district test, so
        // the value cannot be read. Keep it full so it can never be the reason.
        city.encampment_hp = 100;
        return;
    };
    // Firaxis reports DAMAGE against a maximum; this model holds REMAINING health
    // on a 0..=100 scale. Rescale rather than subtract, because a district's
    // maximum is not always 100 on this build and an unscaled remainder would
    // read as full when it is not.
    city.encampment_hp = if encampment.max_damage > 0 && encampment.damage >= 0 {
        let remaining = (encampment.max_damage - encampment.damage).max(0);
        ((100 * remaining) / encampment.max_damage).clamp(0, 100)
    } else {
        100
    };
    city.encampment_wall_hp = if encampment.max_wall_damage > 0 && encampment.wall_damage >= 0 {
        (encampment.max_wall_damage - encampment.wall_damage).max(0)
    } else {
        // Unanswered: match the city's own maximum so `encampment_wall_hp <
        // max_wall` cannot fire on a number nobody measured.
        max_wall
    };
}

/// Carry the host's per-building pillage state onto a reconstructed city.
///
/// The engine already refuses to pay a building in `pillaged_buildings`
/// (yields, housing, amenities, great-work slots all test it), so this is the
/// import matching a rule that was there; only the export was missing. An
/// older export (`None`) leaves the city's own set alone rather than clearing
/// it — unknown is not "nothing pillaged".
fn apply_pillaged_buildings(
    rules: &crate::rules::Rules,
    city: &mut crate::game::City,
    state: &StateCity,
) {
    let Some(pillaged) = &state.pillaged_buildings else {
        return;
    };
    city.pillaged_buildings.clear();
    for civ6 in pillaged {
        if let Some(name) = civvis_node_name(&rules.buildings, civ6, "BUILDING_") {
            if name == "palace" {
                continue;
            }
            let named = crate::name::Name::new(&name);
            if city.buildings.contains(&named) {
                city.pillaged_buildings.insert(named);
            }
        }
    }
}

fn apply_observed_city_infrastructure(
    game: &mut crate::game::Game,
    cid: u32,
    state: &StateCity,
    unmapped: &mut Vec<String>,
) {
    // Rival/public city records do not carry this private roster, and the own-city
    // exporter also leaves both fields absent when Firaxis refuses the plot query.
    // A living Civ VI city cannot lose a completed district or wonder, so an empty
    // observation is not authority to erase remembered infrastructure.
    if state.districts.is_empty() && state.wonders.is_empty() {
        for civ6 in &state.buildings {
            if civvis_node_name(&game.rules.buildings, civ6, "BUILDING_").is_none()
                && civvis_node_name(&game.rules.wonders, civ6, "BUILDING_").is_some()
            {
                let issue = format!("{civ6}:wonder_missing_plot");
                if !unmapped.contains(&issue) {
                    unmapped.push(issue);
                }
            }
        }
        return;
    }
    let Some(city) = game.cities.get(&cid) else {
        return;
    };
    let owner = city.owner;
    let old_districts: Vec<(crate::name::Name, crate::Pos)> = city
        .districts
        .iter()
        .map(|(name, pos)| (*name, *pos))
        .collect();
    let old_wonders: Vec<(crate::name::Name, crate::Pos)> = city
        .wonders
        .iter()
        .map(|(name, pos)| (*name, *pos))
        .collect();
    let old_foundations: Vec<(crate::name::Name, crate::Pos)> = city
        .queue
        .iter()
        .filter_map(|item| match item {
            crate::game::Item::District { district, pos } => Some((*district, *pos)),
            _ => None,
        })
        .collect();

    // Clear only markers this city previously supplied. owner_city can change
    // during capture reconciliation, so it cannot identify infrastructure safely.
    for (name, pos) in old_districts {
        if let Some(tile) = game.map.tiles.get_mut(&pos) {
            if tile.district.as_ref() == Some(&name) {
                tile.district = None;
                tile.pillaged = false;
            }
        }
    }
    for (name, pos) in old_foundations {
        if let Some(tile) = game.map.tiles.get_mut(&pos) {
            if tile
                .district_foundation
                .as_ref()
                .is_some_and(|foundation| foundation.district == name)
            {
                tile.district_foundation = None;
            }
        }
    }
    for (name, pos) in old_wonders {
        if let Some(tile) = game.map.tiles.get_mut(&pos) {
            if tile.wonder.as_ref() == Some(&name) {
                tile.wonder = None;
            }
        }
    }

    let remember_issue = |unmapped: &mut Vec<String>, issue: String| {
        if !unmapped.contains(&issue) {
            unmapped.push(issue);
        }
    };
    let mut completed = Vec::new();
    let mut foundations = Vec::new();
    for district in &state.districts {
        if district.kind.eq_ignore_ascii_case("DISTRICT_CITY_CENTER")
            || district.kind.eq_ignore_ascii_case("DISTRICT_WONDER")
        {
            continue;
        }
        let Some(name) = civvis_node_name(&game.rules.districts, &district.kind, "DISTRICT_")
        else {
            remember_issue(unmapped, format!("{}:district", district.kind));
            continue;
        };
        let pos = crate::hex::offset_to_axial(district.x, district.y);
        if !game.map.tiles.contains_key(&pos) {
            remember_issue(
                unmapped,
                format!(
                    "{}@{},{}:district_plot_missing",
                    district.kind, district.x, district.y
                ),
            );
            continue;
        }
        let observed = (crate::name::Name::new(&name), pos, district.pillaged);
        if district.complete {
            completed.push(observed);
        } else {
            foundations.push(observed);
        }
    }

    let mut wonders = Vec::new();
    for wonder in &state.wonders {
        let Some(name) = civvis_node_name(&game.rules.wonders, &wonder.kind, "BUILDING_") else {
            remember_issue(unmapped, format!("{}:wonder", wonder.kind));
            continue;
        };
        let pos = crate::hex::offset_to_axial(wonder.x, wonder.y);
        if !game.map.tiles.contains_key(&pos) {
            remember_issue(
                unmapped,
                format!(
                    "{}@{},{}:wonder_plot_missing",
                    wonder.kind, wonder.x, wonder.y
                ),
            );
            continue;
        }
        wonders.push((crate::name::Name::new(&name), pos));
    }
    for civ6 in &state.buildings {
        if civvis_node_name(&game.rules.buildings, civ6, "BUILDING_").is_none() {
            if let Some(name) = civvis_node_name(&game.rules.wonders, civ6, "BUILDING_") {
                if !wonders.iter().any(|(wonder, _)| wonder == &name) {
                    remember_issue(unmapped, format!("{civ6}:wonder_missing_plot"));
                }
            }
        }
    }

    if let Some(city) = game.cities.get_mut(&cid) {
        city.districts.clear();
        city.wonders.clear();
        for (name, pos, _) in &completed {
            city.districts.insert(*name, *pos);
        }
        for (name, pos) in &wonders {
            city.wonders.insert(*name, *pos);
        }
    }

    apply_encampment_health(game, state, cid);

    for (name, pos, pillaged) in completed {
        let tile = game.map.tiles.get_mut(&pos).unwrap();
        tile.improvement = None;
        tile.district = Some(name);
        tile.district_foundation = None;
        tile.wonder = None;
        tile.pillaged = pillaged;
    }
    for (name, pos, _) in foundations {
        let item = crate::game::Item::District {
            district: name,
            pos,
        };
        let cost = game.item_cost_for_city(owner, cid, &item);
        let tile = game.map.tiles.get_mut(&pos).unwrap();
        tile.improvement = None;
        tile.district = None;
        tile.district_foundation = Some(crate::world::DistrictFoundation {
            district: name,
            cost,
        });
        tile.wonder = None;
        tile.pillaged = false;
    }
    for (name, pos) in wonders {
        let tile = game.map.tiles.get_mut(&pos).unwrap();
        tile.improvement = None;
        tile.district = None;
        tile.district_foundation = None;
        tile.wonder = Some(name);
        tile.pillaged = false;
    }
}

/// Apply the two health bars Firaxis exposes on a city banner.
fn apply_city_health(game: &mut crate::game::Game, cid: u32, state: &StateCity) {
    if state.damage.is_finite()
        && state.max_damage.is_finite()
        && state.damage >= 0.0
        && state.max_damage > 0.0
    {
        let remaining = (state.max_damage - state.damage).clamp(0.0, state.max_damage);
        if let Some(city) = game.cities.get_mut(&cid) {
            // CIVVIS and the stock City Center both use 200 garrison HP. Scale
            // anyway so a scenario-specific maximum remains truthful.
            city.hp = (200.0 * remaining / state.max_damage)
                .round()
                .clamp(1.0, 200.0) as i32;
        }
    }

    if state.wall_damage.is_finite()
        && state.max_wall_damage.is_finite()
        && state.wall_damage >= 0.0
        && state.max_wall_damage >= 0.0
    {
        let max_wall = state.max_wall_damage.round().max(0.0) as i32;
        Arc::make_mut(&mut game.observed_city_max_wall_hp).insert(cid, max_wall);
        if let Some(city) = game.cities.get_mut(&cid) {
            city.wall_hp = (state.max_wall_damage - state.wall_damage)
                .round()
                .clamp(0.0, state.max_wall_damage) as i32;
        }
    }
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

fn apply_unit_observation(
    live: &mut crate::game::Unit,
    state: &StateUnit,
    progress: ObservedUnitProgress,
) {
    if state.hp.is_finite() && state.hp > 0.0 {
        live.hp = (state.hp.round() as i32).clamp(1, 100);
    }
    if let Some(xp) = state.xp.filter(|xp| *xp >= 0) {
        live.xp = xp;
    }
    if let Some(level) = state.level.filter(|level| *level >= 1) {
        live.level = level;
    }
    if let Some(promotions) = progress.promotions {
        live.promotions = promotions;
    }
    if let Some(religion) = progress.religion {
        live.religion = Some(religion);
    }
    // Zero is an authoritative observation: a Builder or religious unit that
    // spent its final charge must not regain the ruleset default on a fresh
    // rebuild, or retain the previous turn's charge count on sync. Negative
    // values remain the mod's "could not read" sentinel.
    let observed_charges = state
        .build_charges
        .into_iter()
        .chain(state.spread_charges)
        .filter(|charges| *charges >= 0)
        .max();
    if let Some(charges) = observed_charges {
        live.charges = charges;
    }
    live.fortified = state.fortified;
    live.fortify_turns = state.fortify_turns.clamp(0, 2);
    // The host's own `IsEmbarked`, pinned to the observed position: it wins over
    // the "its tile is water" derivation while the unit stands there and lapses
    // the moment the board moves it. Exported since 2026-08 and read by nothing
    // until now; an older export carries `None` and derives as before.
    live.host_embarked = state.embarked.map(|embarked| (live.pos, embarked));
    // ★★★★★ THE LINE THAT MAKES `FORM_ARMY` REACHABLE ON THE LIVE SEAT.
    //
    // `civvis_orders::translate` chooses between `UNITCOMMAND_FORM_CORPS` and
    // `UNITCOMMAND_FORM_ARMY` by reading this unit's `formation` off the mirror
    // (#2373). Nothing ever wrote it, and the live seat rebuilds the mirror from
    // the host export every turn, so every unit was reconstructed at tier 0 and
    // an existing Corps was asked to form another Corps. Firaxis models the two
    // merges as different commands behind different civics — Nationalism and
    // Mobilization — so that is not a near miss, it is the wrong order.
    //
    // ⚠ ONLY A REAL READING WRITES. `None` (an older export) and the mod's `-1`
    // ("asked, could not answer") are unknown, and unknown must not flatten a
    // board that already knows better — a mirror carried across turns may hold a
    // tier CIVVIS itself raised. Accepting the sentinel here is how
    // `GetDefenseStrength` read −1 for the project's whole life unnoticed.
    if let Some(tier) = state.formation.filter(|tier| (0..=2).contains(tier)) {
        live.formation = tier as u8;
    }
    // ⚠ `production_cost` is deliberately NOT rescaled by the tier here.
    // `Game::set_unit_formation` multiplies it by 1.0/1.5/2.0, but that is
    // lifetime accounting only (`unit_accounting_cost`, damage-per-cost) and this
    // function runs on every observation, including repeat syncs of a unit that
    // already carries the tier — so scaling here would compound on each turn
    // rather than converge.
}

/// Seat 0's UNSPENT envoys, from Firaxis's own `GetTokensToGive`.
///
/// ★★★★★ THIS IS THE LINE THAT LETS CIVVIS SPEND AN ENVOY AT ALL.
/// [`crate::game::Game::legal_actions`] enumerates `Action::SendEnvoy` only
/// behind `if p.envoys_free > 0`, and until now nothing wrote that field on a
/// reconstructed board — so every live decision ran on an empire correctly
/// holding zero, and `AdvancedAi::advanced_envoys` (type-aware, suzerainty-
/// priced, denial-aware, and rated in the deployed bundle) never had a token to
/// place. Measured on the twelve Settler games of 2026-08-15/16: runs end
/// holding **40–70 unspent envoys** with 0 suzerainties in 11 of 12.
///
/// The field carries every reading, including a real 0; `None` and the mod's
/// `-1` ("asked, could not answer") leave the board's count alone rather than
/// zeroing an empire that may be holding envoys — an unknown must not read as
/// "nothing to spend". The actuation path is the `envoy` order kind
/// (`civvis_orders` translates `SendEnvoy`; the mod issues one
/// `GIVE_INFLUENCE_TOKEN` per order through a freshly fetched handle), so what
/// the board wants, the bridge can now deliver.
fn apply_mirrored_envoys_free(game: &mut crate::game::Game, state: &StateSnapshot) {
    if let Some(free) = state.envoys_free.filter(|free| *free >= 0) {
        game.players[0].envoys_free = free;
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

/// ★★★★★ THE HOST'S OWN READING OF EVERY PLOT IT EXPORTED, not only the ones a
/// city happens to be working this turn.
///
/// The per-plot correction below this in `apply_observed_city_economy` has
/// always been derived from `state.cities[].worked[].yields` — six or eight
/// plots per city, the ones the host's citizen manager assigned. Every OTHER
/// plot on the board was paid CIVVIS's own catalogue sum, and that sum is short
/// by whatever the ground holds that no row of the ruleset names.
///
/// Disaster fertility is the whole of it. Gathering Storm stores an eruption's
/// or a flood's permanent +1 Food / +1 Production ON THE PLOT
/// (`RandomEvent_Yields`), and Volcanic Soil itself has not one
/// `Feature_YieldChanges` row — so a mirror that reads the feature name sees
/// bare Grassland where the game shows 3 Food 3 Production. The operator
/// reported exactly that tile on the live board.
///
/// The set that was wrong is the set that matters: a plot nobody works yet is
/// precisely what a Builder is choosing to improve, what the citizen governor
/// is choosing to grow onto, and what a Settler is choosing to found beside.
///
/// Four rules keep this a correction rather than an override:
///
/// 1. **Deltas, never absolutes**, like every other correction here — a
///    counterfactual Farm still moves the tile by its modeled amount.
/// 2. **Never over a fresher reading.** The worked and centre plots are
///    corrected first, from THIS turn's state export; this fills in around them
///    with `or_insert` and cannot displace them.
/// 3. **Only a current record.** `Snapshot::revealed` accumulates forever, so a
///    plot the newest sweep missed still sits in it looking authoritative;
///    [`Snapshot::is_current`] refuses those.
/// 4. **Not on a district, foundation, wonder or city centre.** Those pay
///    through their own readers, and a plot's `GetYield` is the ground under
///    them.
fn apply_observed_plot_yields(game: &mut crate::game::Game, snapshot: &Snapshot) {
    for (x, y) in snapshot.revealed_positions().collect::<Vec<_>>() {
        if !snapshot.is_current((x, y)) {
            continue;
        }
        let Some(host) = snapshot.plot((x, y)).and_then(Plot::host_yields) else {
            continue;
        };
        let pos = crate::hex::offset_to_axial(x, y);
        let Some(tile) = game.map.get(pos) else {
            continue;
        };
        if tile.district.is_some()
            || tile.district_foundation.is_some()
            || tile.wonder.is_some()
            || game.city_at(pos).is_some()
        {
            continue;
        }
        if game.observed_tile_yield_adjustments.contains_key(&pos) {
            continue;
        }
        let model = game.modeled_tile_yields(pos);
        let delta = crate::rules::Yields {
            food: host.food - model.food,
            production: host.production - model.production,
            gold: host.gold - model.gold,
            science: host.science - model.science,
            culture: host.culture - model.culture,
            faith: host.faith - model.faith,
        };
        // ⚠ ONLY WHERE THE MODEL IS ACTUALLY WRONG. Most of a map is ordinary
        // ground the catalogue prices exactly, and a zero correction is a
        // no-op that would still be cloned into every planning copy of the
        // board and written into every saved mirror. Recording only the
        // disagreements keeps this map the size of the problem — and makes it
        // readable as "the plots the host and CIVVIS do not agree on", which is
        // what `tools/civ6_yield_drift.py` wants from it.
        if [
            delta.food,
            delta.production,
            delta.gold,
            delta.science,
            delta.culture,
            delta.faith,
        ]
        .iter()
        .all(|value| value.abs() < 1e-9)
        {
            continue;
        }
        Arc::make_mut(&mut game.observed_tile_yield_adjustments).insert(pos, delta);
    }
}

/// Restore exact private city facts before deriving host-to-model corrections.
fn apply_observed_city_economy(
    game: &mut crate::game::Game,
    state: &StateSnapshot,
    snapshot: Option<&Snapshot>,
    unmapped: &mut Vec<String>,
) {
    Arc::make_mut(&mut game.observed_city_yield_adjustments).clear();
    // Clear first: the previous correction is part of `city_amenities` and
    // `city_housing`, and using it while deriving this turn's delta would
    // compound it forever.
    Arc::make_mut(&mut game.observed_city_amenity_adjustments).clear();
    Arc::make_mut(&mut game.observed_city_housing_adjustments).clear();
    Arc::make_mut(&mut game.observed_tile_yield_adjustments).clear();
    Arc::make_mut(&mut game.observed_city_worked_tiles).clear();
    Arc::make_mut(&mut game.observed_city_specialists).clear();

    for observed in &state.cities {
        let pos = crate::hex::offset_to_axial(observed.x, observed.y);
        let Some(cid) = game.city_at(pos) else {
            continue;
        };
        if let Some(worked) = &observed.worked {
            let positions = worked
                .iter()
                .map(|plot| crate::hex::offset_to_axial(plot.x, plot.y))
                // Firaxis includes the city centre in GetWorkedPlots(), but
                // CIVVIS accounts for it separately before assigning citizens.
                // Treating that implicit plot as a citizen assignment rejected
                // the whole authoritative list and silently restored CIVVIS's
                // own governor instead.
                .filter(|worked_pos| *worked_pos != pos)
                // ★★★★★ A DISTRICT PLOT IN THE WORKED LIST IS A SPECIALIST, NOT
                // A TILE. `Citizens:IsPlotWorked` answers true for a Campus a
                // citizen staffs, and the export already names that citizen in
                // `specialists`. Passing the plot through as a worked tile made
                // the model pay the specialist twice — once from `specialists`
                // (+2 Science, correctly) and once as the ground under the
                // district (its terrain Food/Production, which Firaxis removes
                // when the district is placed). Measured on live run
                // civvis-20260816T011314Z: Cumae with two Campus specialists and
                // one Industrial Zone specialist read **+2 Food, +4 Production**
                // over the host for twenty turns; every specialist city showed
                // the same signature. CIVVIS's own governor never offers a
                // district, foundation or wonder plot as a tile job, so this is
                // the import matching the engine, not a new rule.
                .filter(|worked_pos| {
                    game.map.get(*worked_pos).is_none_or(|tile| {
                        tile.district.is_none()
                            && tile.district_foundation.is_none()
                            && tile.wonder.is_none()
                    })
                })
                .collect::<Vec<_>>();
            let all_valid = positions.iter().all(|worked_pos| {
                game.map.get(*worked_pos).is_some() && game.city_at(*worked_pos).is_none()
            });
            if all_valid {
                // Firaxis lets the player swap an empire tile between nearby
                // cities. The tile export carries only player ownership, so the
                // initial mirror assigns it to a nearby city heuristically. A
                // current worked-plot record is stronger evidence: move that
                // tile to the city actually working it before preserving the
                // citizen plan. Without this, Lugdunum's real (58,25) was owned
                // by neighboring Rome in the mirror and the entire observed
                // list was discarded for CIVVIS's freshly optimized substitute.
                for &worked_pos in &positions {
                    let previous = game.map.tiles[&worked_pos].owner_city;
                    if previous == Some(cid) {
                        continue;
                    }
                    if let Some(previous) = previous {
                        if let Some(city) = game.cities.get_mut(&previous) {
                            city.owned_tiles.retain(|tile| *tile != worked_pos);
                        }
                    }
                    if !game.cities[&cid].owned_tiles.contains(&worked_pos) {
                        game.cities
                            .get_mut(&cid)
                            .unwrap()
                            .owned_tiles
                            .push(worked_pos);
                    }
                    game.map.tiles.get_mut(&worked_pos).unwrap().owner_city = Some(cid);
                }
                Arc::make_mut(&mut game.observed_city_worked_tiles).insert(cid, positions);
            } else {
                let issue = format!("{}:worked_plot", observed.name);
                if !unmapped.contains(&issue) {
                    unmapped.push(issue);
                }
            }
        }
        if let Some(specialists) = &observed.specialists {
            let mut translated = Vec::new();
            let mut all_valid = true;
            for civ6 in specialists {
                match civvis_node_name(&game.rules.districts, civ6, "DISTRICT_") {
                    Some(name) => translated.push(
                        game.district_family(crate::name::Name::new(&name))
                            .to_string(),
                    ),
                    None => {
                        all_valid = false;
                        let issue = format!("{civ6}:specialist");
                        if !unmapped.contains(&issue) {
                            unmapped.push(issue);
                        }
                    }
                }
            }
            if all_valid {
                Arc::make_mut(&mut game.observed_city_specialists).insert(cid, translated);
            }
        }
    }

    // Replace only when every own-city query succeeded. A partial export is
    // unknown, not authority to erase works housed in the omitted city.
    if !state.cities.is_empty() && state.cities.iter().all(|city| city.great_works.is_some()) {
        for kind in [
            "writing",
            "art",
            "religious_art",
            "artifact",
            "music",
            "relic",
        ] {
            game.players[0]
                .counters
                .insert(format!("great_work:{kind}"), 0);
        }
        game.players[0].great_work_pieces.clear();
        let mut seen = std::collections::BTreeSet::new();
        // ...and WHERE the host keeps each one. The model's own housing picks
        // the best slot for a work (a Relic goes to St. Basil's over the
        // Palace); the host's placement is what pays, and it read "+6 from
        // GreatWorks" in Rome while the model paid Mediolanum (run
        // civvis-20260816T233226Z t154+).
        let mut housing: std::collections::BTreeMap<
            u32,
            std::collections::BTreeMap<String, usize>,
        > = Default::default();
        for city in &state.cities {
            let cid = game.city_at(crate::hex::offset_to_axial(city.x, city.y));
            for work in city.great_works.as_deref().unwrap_or_default() {
                if !seen.insert(work.kind.clone()) {
                    continue;
                }
                match great_work_kind(&work.object) {
                    Some(kind) => {
                        game.grant_great_work(
                            0,
                            kind,
                            great_work_era(work.era.as_deref()),
                            &work.creator,
                        );
                        if let Some(cid) = cid {
                            *housing
                                .entry(cid)
                                .or_default()
                                .entry(kind.to_string())
                                .or_insert(0) += 1;
                        }
                    }
                    None => {
                        let issue = format!("{}:{}:great_work", work.kind, work.object);
                        if !unmapped.contains(&issue) {
                            unmapped.push(issue);
                        }
                    }
                }
            }
        }
        game.observed_great_work_housing = Some(Arc::new(housing));
    }

    // ★★★★★ THE HOST'S OWN PER-PLOT YIELDS, WHERE THE EXPORT CARRIES THEM.
    //
    // A city total says by how much the model is off; the plot says where. Some
    // of what a tile pays only the host can know — the fertility an eruption or
    // a flood left behind (Rome on run civvis-20260816T003229Z: **+12 Food and
    // +5 Production** over the model for forty turns, all of it volcanic soil),
    // a plantation pillaged between two tile exports, a modifier CIVVIS has no
    // row for. This is the tile-level twin of the city correction below: the
    // difference between `Plot:GetYield` and CIVVIS's own tile model, on the
    // centre and every plot the host says this city works, added inside
    // `workable_tile_yields`. Derived first, so the city correction that
    // follows carries only what is left (buildings, routes, policies).
    //
    // Deltas, never overrides, for the same reason as every other correction
    // here: a Builder's counterfactual mine still moves the tile by its modeled
    // amount. Cleared each turn above, so a plot the host stops working — or a
    // repaired plantation — carries no stale correction. Skipped for district,
    // foundation and wonder plots (specialists, imported separately) and for
    // any plot the mirror lacks. An export older than the field leaves the map
    // empty and everything below exactly as it was.
    for observed in &state.cities {
        let pos = crate::hex::offset_to_axial(observed.x, observed.y);
        let Some(_cid) = game.city_at(pos) else {
            continue;
        };
        let finite = |yields: &crate::rules::Yields| {
            [
                yields.food,
                yields.production,
                yields.gold,
                yields.science,
                yields.culture,
                yields.faith,
            ]
            .iter()
            .all(|value| value.is_finite())
        };
        let delta =
            |host: crate::rules::Yields, model: crate::rules::Yields| crate::rules::Yields {
                food: host.food - model.food,
                production: host.production - model.production,
                gold: host.gold - model.gold,
                science: host.science - model.science,
                culture: host.culture - model.culture,
                faith: host.faith - model.faith,
            };
        if let Some(host) = observed.center_yields.filter(finite) {
            // Against the RAW tile model, not the floored centre: the
            // correction is added before `city_yields_inner` applies its 2 Food
            // / 1 Production floor, and the host's own centre figure already
            // sits at or above that floor, so raw + correction = host survives
            // the floor unchanged.
            let model = game.modeled_tile_yields(pos);
            Arc::make_mut(&mut game.observed_tile_yield_adjustments)
                .insert(pos, delta(host, model));
        }
        for plot in observed.worked.iter().flatten() {
            let Some(host) = plot.yields.filter(finite) else {
                continue;
            };
            let plot_pos = crate::hex::offset_to_axial(plot.x, plot.y);
            if plot_pos == pos {
                continue;
            }
            let Some(tile) = game.map.get(plot_pos) else {
                continue;
            };
            if tile.district.is_some()
                || tile.district_foundation.is_some()
                || tile.wonder.is_some()
                || game.city_at(plot_pos).is_some()
            {
                continue;
            }
            let model = game.modeled_tile_yields(plot_pos);
            Arc::make_mut(&mut game.observed_tile_yield_adjustments)
                .insert(plot_pos, delta(host, model));
        }
    }

    // ...and the same correction for every OTHER plot the sweep read, which is
    // the set a Builder, a Settler and the citizen governor are choosing
    // between. Second, so the worked and centre readings above — taken from
    // this turn's state export rather than the last sweep — are the ones that
    // stand where both exist. See `apply_observed_plot_yields`.
    if let Some(snapshot) = snapshot {
        apply_observed_plot_yields(game, snapshot);
    }

    // Exact citizen assignments and durable Great Work state are now in place.
    // Calibrate the actual happiness band before deriving yield corrections.
    // The correction is a delta, not a host-value override, so a simulated Arena
    // still supplies its modeled Amenities on top of what Firaxis reported now.
    for observed in &state.cities {
        let Some(host_surplus) = host_city_amenity_surplus(observed) else {
            continue;
        };
        let pos = crate::hex::offset_to_axial(observed.x, observed.y);
        let Some(cid) = game.city_at(pos) else {
            continue;
        };
        let modeled_surplus = game.city_amenity_surplus(&game.cities[&cid]);
        Arc::make_mut(&mut game.observed_city_amenity_adjustments)
            .insert(cid, host_surplus - modeled_surplus);
    }

    // Housing the same way: the host's ceiling is what the city grows against,
    // and the board showed CIVVIS's own derivation beside the host's population
    // (Rome 8.5 modelled against 10 reported, Ostia 7.5 against 6, on run
    // civvis-20260816T011314Z t169). `-1` is the mod's could-not-read sentinel
    // and `None` an older export; neither is a claim about the ceiling.
    for observed in &state.cities {
        let Some(host_housing) = observed.housing.filter(|value| *value >= 0.0) else {
            continue;
        };
        let pos = crate::hex::offset_to_axial(observed.x, observed.y);
        let Some(cid) = game.city_at(pos) else {
            continue;
        };
        let modeled_housing = game.city_housing(&game.cities[&cid]);
        Arc::make_mut(&mut game.observed_city_housing_adjustments)
            .insert(cid, host_housing - modeled_housing);
    }

    // What remains is a local correction for host rules CIVVIS has not modeled.
    for observed in &state.cities {
        let Some(host) = observed.yields else {
            continue;
        };
        if ![
            host.food,
            host.production,
            host.gold,
            host.science,
            host.culture,
            host.faith,
        ]
        .iter()
        .all(|value| value.is_finite())
        {
            continue;
        }
        let pos = crate::hex::offset_to_axial(observed.x, observed.y);
        let Some(cid) = game.city_at(pos) else {
            continue;
        };
        let model = game.city_yields_model(cid);
        let adjustment = crate::rules::Yields {
            food: host.food - model.food,
            production: host.production - model.production,
            gold: host.gold - model.gold,
            science: host.science - model.science,
            culture: host.culture - model.culture,
            faith: host.faith - model.faith,
        };
        Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(cid, adjustment);
    }
}

/// Apply city facts that affect `city_yields_model` before deriving the
/// host-to-model correction. A fresh reconstruction initially marks the first
/// planted city as the capital; the host can have moved its Palace elsewhere.
/// Population, loyalty and pillage state also affect the modeled total.
fn apply_observed_city_facts(game: &mut crate::game::Game, state: &StateSnapshot) {
    // Which seats the export names a capital for. A record that flags none
    // (an older export, or a fixture) keeps `place_city`'s own choice rather
    // than clearing every flag and leaving the seat capital-less.
    let flagged_capitals: std::collections::BTreeSet<usize> = state
        .cities
        .iter()
        .chain(state.rivals.iter().flat_map(|rival| rival.cities.iter()))
        .chain(state.minors.iter().flat_map(|minor| minor.cities.iter()))
        .filter(|observed| observed.capital)
        .filter_map(|observed| {
            game.city_at(crate::hex::offset_to_axial(observed.x, observed.y))
                .map(|cid| game.cities[&cid].owner)
        })
        .collect();
    let cities = state
        .cities
        .iter()
        .chain(state.rivals.iter().flat_map(|rival| rival.cities.iter()))
        .chain(state.minors.iter().flat_map(|minor| minor.cities.iter()));
    for observed in cities {
        let pos = crate::hex::offset_to_axial(observed.x, observed.y);
        let Some(cid) = game.city_at(pos) else {
            continue;
        };
        // Population drives Loyalty pressure in a nine-tile radius. Rival and
        // city-state cities are planted at population one, so this has to land
        // before any yield or pressure correction is measured.
        if observed.pop > 0 {
            game.cities.get_mut(&cid).unwrap().pop = observed.pop;
        }
        // `city_has_palace` reads this positional fact; do not leave the
        // reconstruction's first planted city capital after a Palace move.
        if flagged_capitals.contains(&game.cities[&cid].owner) {
            game.cities.get_mut(&cid).unwrap().is_capital = observed.capital;
        }
        apply_city_health(game, cid, observed);
        if observed.loyalty_per_turn.is_finite() {
            Arc::make_mut(&mut game.observed_city_loyalty_per_turn)
                .insert(cid, observed.loyalty_per_turn);
        }
        if observed.defense.is_finite() && observed.defense >= 0.0 {
            Arc::make_mut(&mut game.observed_city_strength).insert(cid, observed.defense);
        }
    }
}

fn apply_observed_host_metrics(
    game: &mut crate::game::Game,
    state: &StateSnapshot,
    snapshot: Option<&Snapshot>,
    unmapped: &mut Vec<String>,
) {
    Arc::make_mut(&mut game.observed_trade_capacity).clear();
    Arc::make_mut(&mut game.observed_yield_adjustments).clear();
    Arc::make_mut(&mut game.observed_public_empire_stats).clear();
    Arc::make_mut(&mut game.observed_majority_religion).clear();
    Arc::make_mut(&mut game.observed_visiting_tourists).clear();
    Arc::make_mut(&mut game.observed_city_loyalty_per_turn).clear();
    Arc::make_mut(&mut game.observed_city_strength).clear();
    Arc::make_mut(&mut game.observed_city_max_wall_hp).clear();
    Arc::make_mut(&mut game.observed_tourism_per_turn).clear();
    if let Some(capacity) = state.trade_capacity.filter(|capacity| *capacity >= 0) {
        Arc::make_mut(&mut game.observed_trade_capacity).insert(0, capacity);
    }
    // Our tourism per turn as the host counts it; `Game::tourism_per_turn`
    // prefers it, the model figure stays behind `tourism_per_turn_model`.
    if let Some(tourism) = state
        .tourism_per_turn
        .filter(|t| t.is_finite() && *t >= 0.0)
    {
        Arc::make_mut(&mut game.observed_tourism_per_turn).insert(0, tourism);
    }
    apply_public_empire_stats(game, 0, &state.public_stats);
    {
        // Same per-snapshot honesty as the rival counters: unknown is None.
        let count = |value: f64| {
            (value.is_finite() && value >= 0.0 && value <= usize::MAX as f64)
                .then(|| value.round() as usize)
        };
        let observed = Arc::make_mut(&mut game.observed_public_empire_stats)
            .entry(0)
            .or_default();
        observed.foreign_tourists = count(state.foreign_tourists);
        observed.domestic_tourists = count(state.domestic_tourists);
        // Match the local player's World Rankings science lane to the host
        // instead of treating its reconstructed fifty-light-year trip as fact.
        observed.science_victory_points = (state.science_victory_points.is_finite()
            && state.science_victory_points >= 0.0)
            .then_some(state.science_victory_points);
        observed.science_victory_points_per_turn =
            (state.science_victory_points_per_turn.is_finite()
                && state.science_victory_points_per_turn >= 0.0)
                .then_some(state.science_victory_points_per_turn);
        observed.science_victory_points_needed = (state.science_victory_points_needed.is_finite()
            && state.science_victory_points_needed > 0.0)
            .then_some(state.science_victory_points_needed);
        observed.cities_following_religion = state
            .cities_following_religion
            .filter(|value| *value >= 0)
            .map(|value| value as usize);
    }

    // These fields participate in the model itself, so settle them before
    // measuring the host-to-model city correction.
    apply_observed_city_facts(game, state);
    apply_observed_city_economy(game, state, snapshot, unmapped);

    let mut derived = crate::rules::Yields::default();
    for cid in game.player_city_ids(0) {
        derived.add(game.city_yields(cid));
    }
    // The empire collects founder-belief income and the Faith for unused
    // Great Person points beside its cities, and every reader of the per-turn
    // figure adds them (`player_yield_extras`), so the residual measured here
    // is only what CIVVIS still cannot derive.
    derived.add(game.player_yield_extras(0));
    derived.add(game.arena_side_yields(0));
    let mut adjustment = crate::rules::Yields::default();
    if let Some(host_food) = state
        .public_stats
        .food
        .filter(|value| value.is_finite() && *value >= 0.0)
    {
        adjustment.food = host_food - derived.food;
    }
    if let Some(host_production) = state
        .public_stats
        .production
        .filter(|value| value.is_finite() && *value >= 0.0)
    {
        adjustment.production = host_production - derived.production;
    }
    if let Some(host_science) = state
        .science
        .filter(|value| value.is_finite() && *value >= 0.0)
    {
        adjustment.science = host_science - derived.science;
    }
    if let Some(host_culture) = state
        .culture
        .filter(|value| value.is_finite() && *value >= 0.0)
    {
        adjustment.culture = host_culture - derived.culture;
    }
    // Faith per turn: the host's top-bar figure against the same sum, applied
    // as a delta like science and culture. Only when the export carries it —
    // an older control mod leaves the model's own figure standing.
    if let Some(host_faith) = state
        .faith_per_turn
        .filter(|value| value.is_finite() && *value >= 0.0)
    {
        adjustment.faith = host_faith - derived.faith;
    }
    if adjustment.food != 0.0
        || adjustment.production != 0.0
        || adjustment.science != 0.0
        || adjustment.culture != 0.0
        || adjustment.faith != 0.0
    {
        Arc::make_mut(&mut game.observed_yield_adjustments).insert(0, adjustment);
    }
    // ★★★★★ THE RIVALS' SEATS LAST, AFTER THEIR CITIES ARE FINISHED.
    //
    // A correction is `host − model`, and the model of a rival city moves
    // with every fact the loop above writes onto it — above all its
    // Population, which is the term every yield is a linear function of and
    // which arrives here (rival cities are planted at population one). Derived
    // before that loop, the delta was measured against a size-one city and
    // then paid on the size-eleven one: on run civvis-20260816T175306Z the
    // board read Nubia at 174 Science against the host's 141, 329 Food against
    // 229, every rival over by its own growth. Same reason the seat's own
    // Dedications are applied before this function runs (`apply_player_ages`
    // in both callers). Seats are 1..n in export order, as the rival loops in
    // `rebuild_from_state` and `LiveMirror::sync` assign them.
    for (index, rival) in state.rivals.iter().enumerate() {
        let owner = index + 1;
        if owner >= game.players.len() {
            break;
        }
        apply_rival_public_economy(game, owner, rival, unmapped);
    }
}

/// Seat the World Congress diplomatic standing, including the majors this seat
/// has not met.
///
/// The rival loops assign seats `1..n` in export order and that list is
/// met-gated, so before this ran `players[*].dvp` could only ever describe
/// contacted empires. `state.seat.players` sizes the board to every major in
/// the game, so the seats past the met rivals already exist as the
/// reconstruction's stand-ins for the ones we have not found — attaching the
/// congress standing to them is the one public fact we hold about them.
///
/// Deliberately conservative in three ways:
///
/// - a met rival keeps its own per-turn export, which is sampled every turn
///   against a congress table refreshed once a session. Only a rival whose
///   `dvp` the mod could not read falls back to the congress value. DVP can
///   go *down* (`WC_RES_DIPLOVICTORY` option B is −2), so preferring the
///   larger of the two would latch a stale high standing.
/// - the seat's own entry is ignored; `state.dvp` is the live read.
/// - unmet entries are seated in ascending host-player order, so the same
///   congress table always lands the same way.
fn apply_congress_dvp(game: &mut crate::game::Game, state: &StateSnapshot) {
    let Some(congress) = &state.congress_dvp else {
        return;
    };
    // Host player id -> mirror seat, exactly as the rival loops assign them.
    let seat_of: std::collections::BTreeMap<usize, usize> = state
        .rivals
        .iter()
        .enumerate()
        .map(|(index, rival)| (rival.player, index + 1))
        .collect();
    let ours = state.seat.local_player.max(0) as usize;
    let mut unmet: Vec<&StateCongressDvpEntry> = Vec::new();
    for entry in &congress.points {
        if entry.player == ours {
            continue;
        }
        match seat_of.get(&entry.player) {
            // A met rival the mod could not read a live `dvp` for. Everyone
            // else keeps the fresher per-turn number.
            Some(&seat) => {
                let stale = state
                    .rivals
                    .get(seat - 1)
                    .is_none_or(|rival| rival.dvp.is_none());
                if stale && seat < game.players.len() {
                    game.players[seat].dvp = entry.points;
                }
            }
            None => unmet.push(entry),
        }
    }
    unmet.sort_by_key(|entry| entry.player);
    for (seat, entry) in (state.rivals.len() + 1..).zip(unmet) {
        if seat >= game.players.len() {
            break;
        }
        game.players[seat].dvp = entry.points;
    }
}

/// Mirror the short-lived World Congress competitions that the host has
/// already made available to this seat. Firaxis grants their projects through
/// `UnlocksFromEffect`, so retaining an old answer would be worse than losing
/// one: it would make CIVVIS issue a project the host has withdrawn.
fn apply_host_competitions(game: &mut crate::game::Game, state: &StateSnapshot) {
    let Some(emergencies) = state.emergencies.as_ref() else {
        // An absent field is an older mod, not proof that a competition ended.
        return;
    };
    let turn = state.turn.max(1);
    let mut competitions: Vec<crate::game::HostCompetition> = emergencies
        .iter()
        .filter_map(|emergency| {
            if !emergency.begun
                || emergency.turns_left < 0
                || !emergency.ours.member
                || emergency.kind.trim().is_empty()
            {
                return None;
            }
            let ours = emergency
                .ours
                .score
                .filter(|score| score.is_finite())
                .unwrap_or(0.0);
            let leader = emergency
                .scores
                .iter()
                .map(|score| score.score)
                .filter(|score| score.is_finite())
                .fold(ours, f64::max);
            let remaining = u32::try_from(emergency.turns_left).unwrap_or(u32::MAX);
            Some(crate::game::HostCompetition {
                kind: emergency.kind.trim().to_string(),
                // `TurnsLeft == 0` is still the host's final playable turn.
                ends: turn.saturating_add(remaining).saturating_add(1),
                ours,
                leader,
            })
        })
        .collect();
    competitions.sort_by(|left, right| left.kind.cmp(&right.kind));
    game.replace_host_competitions(competitions);
}

/// Write the host's own diplomacy for one rival — seat pair `(0, owner)` —
/// onto the board, assigned from every export so a lapsed state clears.
///
/// Before this the only diplomacy that crossed for a rival was `at_war`,
/// `open_borders` and `can_declare`, and the last was mirrored as a faked
/// denouncement (`denounced_until = turn + 1`) so that a Formal War could be
/// legal on a board whose own five-turn wait never matures. `can_declare`
/// says a war is LEGAL; it never said whether it was ruinous. Every decision
/// that reads the ledger — `preferred_war_opening`'s casus belli choice and
/// its wait, the alliance-partner filter (`grievances < 75`), the coalition
/// and joint-war partner screens, `relationship_opinion`, the declared-friend
/// and alliance bars in `legal_actions` — read an empty ledger on every live
/// turn.
///
/// What each host state writes (`civvis_orders --dump-mirror` reads it back):
/// - `DIPLO_STATE_DENOUNCED`: `denounced_until` / `denounced_since` on the
///   side(s) that denounced, from the host's `GetDenounceTurn` and
///   `GetDenounceTimeLimit` (`DiplomacyActionView.lua:1486-1503`), so the
///   Formal War wait on the board is the host's own clock. When the host
///   gives neither side's turn the pair reads as denounced from this turn.
/// - `DIPLO_STATE_DECLARED_FRIEND`: `friends_until` both sides from
///   `GetDeclaredFriendshipTurn` (`:1510-1511`); `legal_actions` then
///   withholds war and denouncement, as the host does.
/// - `DIPLO_STATE_ALLIED`: `alliances` both sides with the host's type, level
///   and turns to expiry, plus the friendship an alliance implies.
/// - Any other state clears all three on both sides.
/// - Independently of the state: the grievance balance both ways, missions
///   both ways, promises both ways, the Open Borders WE grant, and the
///   visibility level both ways (`Player::observed_visibility`, which
///   `Game::diplomatic_visibility` prefers to its derivation).
///
/// ⚠ The `can_declare` permission fake stays, in one case: when the host
/// permits a declaration and the board holds no ACTIVE denouncement of our
/// own, `denounced_until = turn + 1` is still written. The bridge carries no
/// `Denounce` order, so a board denouncement never reaches the host; without
/// the fake `preferred_war_opening` would denounce on the board every turn,
/// be rebuilt without it, and never declare — the 81-turn, zero-declaration
/// history that put the fake there. It is the one board fact that is not the
/// host's, and FIDELITY.md's queue names it.
///
/// An export without `diplomatic_state` (an older mod) writes only the fake,
/// exactly as before.
pub(crate) fn apply_host_diplomacy(game: &mut crate::game::Game, owner: usize, rival: &StateRival) {
    if owner == 0 || owner >= game.players.len() {
        return;
    }
    let turn = game.turn;
    let permitted = rival.can_declare && !rival.at_war;
    let Some(state) = rival.diplomatic_state.as_deref() else {
        if permitted {
            game.players[0].denounced_until.insert(owner, turn + 1);
        } else {
            game.players[0].denounced_until.remove(&owner);
        }
        return;
    };
    let state = state.strip_prefix("DIPLO_STATE_").unwrap_or(state);
    let limit = rival
        .denounce_time_limit
        .filter(|limit| *limit > 0)
        .map(|limit| limit as u32);
    let host_turn = |value: Option<i64>| value.filter(|t| *t > 0).map(|t| t as u32);
    // A state that began on `since` runs `limit` more host turns (the
    // denouncement one more, `:1500`); without both it is simply re-read from
    // the next export.
    let expiry = |since: Option<u32>, extra: u32| match (since, limit) {
        (Some(since), Some(limit)) => since.saturating_add(limit).saturating_add(extra),
        _ => turn + 2,
    };

    // Denouncement, each side from its own denounce turn.
    let denounced = state == "DENOUNCED";
    let ours = denounced
        .then(|| host_turn(rival.our_denounce_turn))
        .flatten();
    let theirs = denounced
        .then(|| host_turn(rival.their_denounce_turn))
        .flatten();
    let side_unknown = denounced && ours.is_none() && theirs.is_none();
    for (denouncer, target, since, active) in [
        (0, owner, ours, ours.is_some() || side_unknown),
        (owner, 0, theirs, theirs.is_some() || side_unknown),
    ] {
        let player = &mut game.players[denouncer];
        if active {
            player.denounced_until.insert(target, expiry(since, 1));
            match since {
                Some(since) => {
                    player.denounced_since.insert(target, since);
                }
                None => {
                    player.denounced_since.remove(&target);
                }
            }
        } else {
            player.denounced_until.remove(&target);
            player.denounced_since.remove(&target);
        }
    }
    // The permission fake (see above): the host permits a declaration and
    // the board holds no active denouncement of our own.
    let own_active = game.players[0]
        .denounced_until
        .get(&owner)
        .is_some_and(|until| *until > turn);
    if permitted && !own_active {
        game.players[0].denounced_until.insert(owner, turn + 1);
        game.players[0].denounced_since.remove(&owner);
    }

    // Declared friendship; an alliance implies it, as the engine's own deal
    // acceptance writes both.
    if matches!(state, "DECLARED_FRIEND" | "ALLIED") {
        let until = expiry(host_turn(rival.friendship_turn), 0);
        game.players[0].friends_until.insert(owner, until);
        game.players[owner].friends_until.insert(0, until);
    } else {
        game.players[0].friends_until.remove(&owner);
        game.players[owner].friends_until.remove(&0);
    }

    // Alliance, with the host's type and level; `ends` past this turn so
    // `alliance_with` sees it, and past the host's expiry when it is given.
    if state == "ALLIED" {
        let kind = rival
            .alliance_type
            .as_deref()
            .map(|kind| {
                kind.strip_prefix("ALLIANCE_")
                    .unwrap_or(kind)
                    .to_ascii_lowercase()
            })
            .unwrap_or_else(|| "unknown".to_string());
        let level = rival.alliance_level.unwrap_or(1).max(1);
        let ends = match rival.alliance_turns_left.filter(|left| *left >= 0) {
            Some(left) => turn.saturating_add(left as u32).saturating_add(1),
            None => turn + 2,
        };
        // The engine's own level thresholds, so a reader of `points` agrees
        // with the level the host reports.
        let points = match level {
            1 => 0.0,
            2 => 80.0,
            _ => 240.0,
        };
        let alliance = crate::game::AllianceState {
            kind,
            points,
            level,
            ends,
        };
        game.players[0].alliances.insert(owner, alliance.clone());
        game.players[owner].alliances.insert(0, alliance);
    } else {
        game.players[0].alliances.remove(&owner);
        game.players[owner].alliances.remove(&0);
    }

    // The grievance balance, each direction on the aggrieved side's ledger
    // (`Player::grievances[offender]`), the way `add_direct_grievances`
    // books it. Untouched when the host gave neither number.
    if rival.our_grievances_against_them.is_some() || rival.grievances_against_us.is_some() {
        for (aggrieved, offender, amount) in [
            (0, owner, rival.our_grievances_against_them),
            (owner, 0, rival.grievances_against_us),
        ] {
            let amount = amount.unwrap_or(0.0);
            if amount > 0.0 && amount.is_finite() {
                game.players[aggrieved].grievances.insert(offender, amount);
            } else {
                game.players[aggrieved].grievances.remove(&offender);
            }
        }
    }

    // Missions: an Embassy replaces a Delegation, at most one per counterpart.
    for (sender, host, embassy, delegation) in [
        (0, owner, rival.embassy_at, rival.delegation_at),
        (owner, 0, rival.their_embassy, rival.their_delegation),
    ] {
        if embassy.is_none() && delegation.is_none() {
            continue;
        }
        let kind = if embassy == Some(true) {
            Some("embassy")
        } else if delegation == Some(true) {
            Some("delegation")
        } else {
            None
        };
        let missions = &mut game.players[sender].diplomatic_missions;
        match kind {
            Some(kind) => {
                let sent = missions
                    .get(&host)
                    .filter(|mission| mission.kind == kind)
                    .map(|mission| mission.sent)
                    .unwrap_or(turn);
                missions.insert(
                    host,
                    crate::game::DiplomaticMission {
                        kind: kind.to_string(),
                        sent,
                    },
                );
            }
            None => {
                missions.remove(&host);
            }
        }
    }

    // Promises, keyed as the engine keys them: the promisor's ledger, by the
    // requester, by kind. The host names a kind from the requester's side
    // ("near ME"); three map onto engine kinds, the rest keep their name.
    let promise_kind = |name: &str| match name {
        "DONT_SETTLE_NEAR_ME" => "no_settling".to_string(),
        "DONT_SPY_ON_ME" => "no_spying".to_string(),
        "DONT_CONVERT_MY_CITIES" => "no_conversion".to_string(),
        other => other.to_ascii_lowercase(),
    };
    for (promisor, requester, names) in [
        (0, owner, rival.promises_made.as_ref()),
        (owner, 0, rival.promises_received.as_ref()),
    ] {
        let Some(names) = names else {
            continue;
        };
        if names.is_empty() {
            game.players[promisor].promises.remove(&requester);
            continue;
        }
        let book: BTreeMap<String, u32> = names
            .iter()
            .map(|name| (promise_kind(name), turn + 2))
            .collect();
        game.players[promisor].promises.insert(requester, book);
    }

    // Visibility both ways; a missing reading falls back to the derivation.
    for (viewer, subject, level) in [
        (0, owner, rival.visibility),
        (owner, 0, rival.their_visibility_on_us),
    ] {
        match level {
            Some(level) => {
                game.players[viewer]
                    .observed_visibility
                    .insert(subject, level.max(0) as f64);
            }
            None => {
                game.players[viewer].observed_visibility.remove(&subject);
            }
        }
    }

    // The Open Borders WE grant, the mirror image of `rival.open_borders`.
    match rival.open_borders_granted {
        Some(true) => {
            game.players[0].open_borders_until.insert(owner, turn + 2);
        }
        Some(false) => {
            game.players[0].open_borders_until.remove(&owner);
        }
        None => {}
    }
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
            ("governor_state", BOTH, step_governor_state),
            ("host_envoys", BOTH, step_host_envoys),
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

fn step_refused_site_blocks(ctx: &mut HostStepCtx<'_>) {
    // Sites the host engine has already rejected, so the planner stops re-deriving
    // them. See `refused_sites_of_kind_through`.
    ctx.game.blocked_city_sites = Arc::new(ctx.state.refused_sites.clone());
    ctx.game.blocked_improvement_sites = Arc::new(ctx.state.refused_improves.clone());
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
    // ★★★ AND HOW MANY TURNS ARE LEFT. `rebuild_game` hardcodes 500; this build's real
    // limit at Tiny/Online reads 250 (`seat.max_turns`, and the HUD shows TURN n/250).
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
}

fn step_loyalty_doomed_sites(ctx: &mut HostStepCtx<'_>) {
    block_loyalty_doomed_settler_sites(ctx.game);
}

fn step_host_climate(ctx: &mut HostStepCtx<'_>) {
    // The host's climate needs the finished map (the lowland bands) and the
    // finished city roster (a Flood Barrier keeps its ground).
    apply_host_climate(ctx.game, ctx.state);
}

fn step_record_host_observed(ctx: &mut HostStepCtx<'_>) {
    // Last, because it reads the finished board: every rival, minor and
    // barbarian for this turn is on it by now, and the previous turn's
    // sightings were removed with them.
    record_host_observed(ctx.game, ctx.snapshot);
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

    // Rivals get seats 1..n in the order Civilization VI reported them, so a
    // CIVVIS `DeclareWar { player }` maps straight back onto a Civ 6 player id.
    for (index, rival) in state.rivals.iter().enumerate() {
        let owner = index + 1;
        if owner >= game.players.len() {
            break;
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
    let mut seat_of_host: std::collections::BTreeMap<usize, usize> = state
        .rivals
        .iter()
        .enumerate()
        .map(|(index, rival)| (rival.player, index + 1))
        .collect();
    seat_of_host.insert(0, 0);
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
    game.replace_host_menus(menus.buildable, menus.purchasable, menus.district_plots);
    seat_live_spies(&mut game);
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
    game.blocked_strikes = Arc::new(blocked_strikes_from(&state.refused_strikes, &unit_ids));
    game.host_previews = Arc::new(host_previews_from(&state.host_previews, &unit_ids));

    // The readings that must survive whatever the board passes scored on their
    // way there. See `HOST_STATE_STEPS`.
    run_host_steps(
        &mut HostStepCtx::new(
            &mut game,
            snapshot,
            state,
            &mut unmapped,
            &mut no_treasury,
            pass,
        ),
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

/// Seat every rival's delegation at a city-state from the host's own count
/// (`envoys_by_player`: `GetTokensReceived` per alive major, the shipped
/// CityStates panel's read). Seat 0's count is `minor.envoys`, seeded beside
/// this call and never touched here. A listed export is authoritative for
/// every major seat: a rival absent from it, or at zero, holds nothing there.
/// `None` (an older export) leaves `seed_mirrored_suzerainty`'s minimum
/// winning delegation to do what it did — which could never tell one envoy
/// from five, nor see that we stood one short of a suzerainty.
fn apply_host_rival_envoys(
    game: &mut crate::game::Game,
    minor: &StateMinor,
    owner: usize,
    seat_of_host: &std::collections::BTreeMap<usize, usize>,
) {
    let Some(counts) = minor.envoys_by_player.as_ref() else {
        return;
    };
    for pid in 1..game.players.len() {
        if !game.players[pid].is_minor {
            set_mirrored_envoys(&mut game.players[pid], owner, 0);
        }
    }
    for count in counts {
        if count.player < 0 {
            continue;
        }
        let Some(&seat) = seat_of_host.get(&(count.player as usize)) else {
            continue;
        };
        if seat == 0 || seat >= game.players.len() || game.players[seat].is_minor {
            continue;
        }
        set_mirrored_envoys(&mut game.players[seat], owner, count.envoys.max(0));
    }
}

/// Seat the request a city-state is making of us, from the host's
/// QuestsManager, on the pair (`Player::quests[minor]`) where
/// `Game::city_state_quest` and the `quest-*` genes read it. The board rolled
/// its own quest for every pair from a hash and paid itself the Envoy when its
/// model said so; the host's actual request never crossed.
///
/// The host's `QUEST_*` type is the board's kind by name (`QUEST_TRAIN_UNIT_TYPE`
/// → `train_unit_type`, all eight); the target the mod recovered from the
/// description is translated the way every other host name is, and an
/// untranslatable one is filed under `unmapped` and kept verbatim, so the
/// kind still reads. `None` (an older export) leaves the board's own roll;
/// `Some([])` clears it.
fn apply_host_quest(
    game: &mut crate::game::Game,
    minor: &StateMinor,
    owner: usize,
    unmapped: &mut Vec<String>,
) {
    let Some(quests) = minor.quests.as_ref() else {
        return;
    };
    let mut note = |issue: String| {
        if !unmapped.contains(&issue) {
            unmapped.push(issue);
        }
    };
    game.players[0].quests.remove(&owner);
    let Some(quest) = quests.first() else {
        return;
    };
    let kind = quest
        .kind
        .strip_prefix("QUEST_")
        .unwrap_or(&quest.kind)
        .to_ascii_lowercase();
    if !crate::game::quests::QUEST_KINDS.contains(&kind.as_str()) {
        note(format!("quest:{}", quest.kind));
        return;
    }
    let host_target = quest.target.as_deref().unwrap_or("");
    let translated = match kind.as_str() {
        "train_unit_type" => resolved_civvis_unit_name(&game.rules, host_target),
        "zone_district_type" => civvis_node_name(&game.rules.districts, host_target, "DISTRICT_")
            .map(|district| game.district_family(Name::new(&district)).to_string()),
        "trigger_tech_boost" => civvis_node_name(&game.rules.techs, host_target, "TECH_"),
        "trigger_civic_boost" => civvis_node_name(&game.rules.civics, host_target, "CIVIC_"),
        "recruit_great_person_class" => host_target
            .strip_prefix("GREAT_PERSON_CLASS_")
            .map(|class| class.to_ascii_lowercase()),
        _ => Some(String::new()),
    };
    let target = match translated {
        Some(target) => target,
        None => {
            note(format!(
                "quest_target:{kind}:{}",
                if host_target.is_empty() {
                    "none"
                } else {
                    host_target
                }
            ));
            host_target.to_ascii_lowercase()
        }
    };
    let (pos, mark) = match kind.as_str() {
        "clear_barbarian_camp" => (
            game.camps_near_city_state(owner).into_iter().next(),
            game.players[0].counters.get("camps").copied().unwrap_or(0),
        ),
        "recruit_great_person_class" => (
            None,
            game.players[0]
                .gp_claimed
                .get(&target)
                .copied()
                .unwrap_or(0),
        ),
        _ => (None, 0),
    };
    game.players[0].quests.insert(
        owner,
        crate::game::quests::CityStateQuest {
            kind,
            target,
            era: game.world_era,
            pos,
            mark,
        },
    );
}

/// Carry the host's climate onto the board: the level is the phase
/// (`Game::mirror_set_climate_phase`, which also floods the lowland bands the
/// shipped `CoastalLowlands` table names for it), our own CO2 is the seat's
/// `co2_emissions`, and the rest — the world's CO2, the temperature, the
/// sea-level and disaster forecasts — is `Game::observed_climate`, which
/// `global_co2_emissions` prefers to the board's own sum. `None` (an older
/// export, or a ruleset without `GameClimate`) leaves all of it alone.
fn apply_host_climate(game: &mut crate::game::Game, state: &StateSnapshot) {
    let Some(climate) = state.climate.as_ref() else {
        // An older export: the phase the board holds stays, and the bands it
        // floods are re-marked on the map the sync has just re-applied.
        if game.climate_phase > 0 {
            let phase = game.climate_phase;
            game.mirror_set_climate_phase(phase);
        }
        return;
    };
    if (0..=7).contains(&climate.level) {
        game.mirror_set_climate_phase(climate.level as u8);
    }
    let finite = |value: Option<f64>| value.filter(|value| value.is_finite() && *value >= 0.0);
    let counted = |value: Option<i64>| value.filter(|value| *value >= 0);
    if let Some(ours) = finite(climate.co2_ours) {
        game.players[0].co2_emissions = ours;
    }
    game.observed_climate = Some(crate::game::ObservedClimate {
        temperature: climate.temperature.filter(|value| value.is_finite()),
        co2_total: finite(climate.co2_total),
        sea_level_turns: counted(climate.sea_level_turns),
        tiles_flooded: counted(climate.tiles_flooded),
        storm_pct: finite(climate.storm_pct),
        flood_pct: finite(climate.flood_pct),
        drought_pct: finite(climate.drought_pct),
    });
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

fn apply_territory(game: &mut crate::game::Game, snapshot: &Snapshot, state: &StateSnapshot) {
    // Civ 6 player id -> CIVVIS seat. Rivals are remapped `i -> i + 1`, the same
    // mapping the war bond uses; see `LiveMirror::sync`.
    let mut seat_of: std::collections::BTreeMap<i32, usize> = Default::default();
    for (index, rival) in state.rivals.iter().enumerate() {
        seat_of.insert(rival.player as i32, index + 1);
    }
    for (minor, seat) in minor_actor_assignments(game, state) {
        seat_of.insert(minor.player as i32, seat);
    }
    for city in &state.cities {
        if let Some(plot) = snapshot.plot((city.x, city.y)) {
            if plot.o >= 0 && !seat_of.contains_key(&plot.o) {
                seat_of.insert(plot.o, 0);
            }
        }
    }
    let mut centres: std::collections::BTreeMap<usize, Vec<(u32, crate::Pos)>> = Default::default();
    for (cid, city) in &game.cities {
        centres
            .entry(city.owner)
            .or_default()
            .push((*cid, city.pos));
    }
    // Decided first, applied second: the nearest-city lookup needs `game` immutably
    // while the assignment needs it mutably.
    let mut assign: Vec<(crate::Pos, Option<u32>)> = Vec::new();
    // Ground somebody else holds that we cannot attribute to a mirrored seat.
    let mut blocked: std::collections::BTreeSet<crate::Pos> = Default::default();
    // The same ground, minus whatever this seat may already walk through. See
    // [`crate::game::Game::closed_borders`]: `blocked` answers "cannot found
    // here", which is not the same question as "cannot enter here" — war and
    // open borders open the second while leaving the first shut, and a settler
    // barred by loyalty is barred from founding on ground it may cross freely.
    let mut sealed: std::collections::BTreeSet<crate::Pos> = Default::default();
    // Ground a MET major owns whose city attribution is unresolved: either the
    // seat holds no city on this board at all, or its nearest known city lies
    // on or beyond the fifth ownership ring. At five, the host cannot tell that
    // city from a nearer one still in fog; beyond five, the visible city cannot
    // own it at all. See [`crate::game::Game::unseen_major_borders`].
    let mut unseen_major: std::collections::BTreeSet<crate::Pos> = Default::default();
    // Who seals how much, majors only — the passage-purchase lane's shopping
    // list. See [`crate::game::Game::sealed_border_owners`].
    let mut sealed_by: std::collections::BTreeMap<usize, u32> = Default::default();
    let is_major = |seat: usize| {
        game.players
            .get(seat)
            .is_some_and(|p| seat != 0 && !p.is_minor && !p.is_barbarian)
    };
    for y in 0..snapshot.height.max(1) {
        for x in 0..snapshot.width.max(1) {
            let Some(plot) = snapshot.plot((x, y)) else {
                continue;
            };
            let pos = crate::hex::offset_to_axial(x, y);
            if !game.map.tiles.contains_key(&pos) {
                continue;
            }
            let Some(&seat) = seat_of.get(&plot.o) else {
                // `o = -1` is nobody, and Civilization VI is authoritative about that
                // too: a tile CIVVIS thinks it owns and the game says is neutral is
                // the same class of error in the other direction.
                if plot.o < 0 {
                    assign.push((pos, None));
                    continue;
                }
                // ★★★★★ SOMEBODY OWNS IT AND WE CANNOT NAME THEM — USUALLY A
                // CITY-STATE. `state.rivals` carries the MAJOR civilizations this seat
                // has met, so a minor's territory maps to no seat at all and used to
                // arrive as free land. It is not free: Civilization VI will not let a
                // settler found there, and CIVVIS will happily pick it because on its
                // board the tile is unowned, high-yield and often already improved.
                //
                // Measured on run `civvis-20260731T052021Z`, which is the whole 53-turn
                // stall in one line: CIVVIS chose offset (15,11) — plains hills, worth
                // 99.6, with a MINE at (14,11) and a FARM at (16,11) beside it, all
                // three exported as `o: 6`. That is a city-state's improved land. The
                // settler walked to the border, could not take the last step, and
                // bounced between two tiles for the rest of the game while the empire
                // held one city.
                //
                // Recorded through `blocked_city_sites`, the channel the host's own
                // `found` refusals already use, rather than as a new kind of fact: it
                // means the same thing — ground this seat cannot found on — and it is
                // read by the same planner. We do not invent a city to own it, because
                // we do not know where their city is and a phantom owner is worse than
                // a known prohibition.
                blocked.insert(pos);
                // Nobody we can name holds it, so there is no diplomacy to
                // consult and no war that could have opened it. A city-state's
                // land is shut to us until we are its Suzerain, which is the
                // conservative reading and the one that matches the settler
                // stall this arm was written for.
                sealed.insert(pos);
                // ⚠ And it is certainly not OURS. `place_city` hands a mirrored
                // city its whole first ring, so a rival or minor plot that falls
                // inside one reads back as our own territory — worse than the
                // "reads unowned" error this function was written to fix, because
                // it inflates the yields and workable tiles we plan on. Strip it.
                assign.push((pos, None));
                continue;
            };
            // The city that would work it: the owner's nearest. Civ 6 records only
            // the owning PLAYER per plot, so which of their cities holds it is not in
            // the export and the nearest is the only defensible reconstruction.
            let nearest = centres.get(&seat).and_then(|list| {
                list.iter()
                    .min_by_key(|(cid, centre)| (game.wdist(pos, *centre), *cid))
                    .map(|(cid, centre)| (*cid, game.wdist(pos, *centre)))
            });
            let owner = nearest.map(|(cid, _)| cid);
            if owner.is_some() {
                // A plot on the outermost known-city ownership ring is
                // ambiguous: the export names only a player, so it may instead
                // belong to a nearer city this seat has not seen. Beyond that
                // ring it must be unseen. Mark either case for the live
                // settlement Loyalty guard.
                if is_major(seat)
                    && nearest.is_some_and(|(_, distance)| distance >= CIV6_CITY_OWNERSHIP_REACH)
                {
                    unseen_major.insert(pos);
                }
                // Attributed ground is gated by `can_enter` through
                // `has_open_borders`, which now reads the host's own Early
                // Empire answer (`Player::borders_enforced`) — no seal needed.
                // The passage-purchase lane still has to know WHO shuts how
                // much: a major whose border we can see and cannot cross is
                // exactly the seat Open Borders is bought from, and until now
                // only fogged ground made that list.
                if is_major(seat) && !game.is_at_war(0, seat) && game.enforces_borders(seat) {
                    let granted = game
                        .players
                        .get(seat)
                        .and_then(|p| p.open_borders_until.get(&0))
                        .is_some_and(|until| *until > game.turn);
                    if !granted {
                        *sealed_by.entry(seat).or_insert(0) += 1;
                    }
                }
                assign.push((pos, owner));
            } else if seat != 0 {
                if is_major(seat) {
                    unseen_major.insert(pos);
                }
                // A seat we can NAME but that holds no city on this board —
                // their centre is unrevealed, or refused planting. Their
                // ground is still not ours to found on; leaving it unassigned
                // would re-open the exact hole the unattributable arm above
                // closes.
                blocked.insert(pos);
                // ...and not ours to walk into either, unless we are at war.
                // Asking here rather than in `can_enter` is what keeps a
                // declaration of war from sealing our own invasion out: the seat
                // is named, so `at_war` is answerable from the export even
                // though their cities are unseen.
                //
                // ⚠ `has_open_borders` is deliberately NOT consulted. It returns
                // TRUE whenever the owner has no `open_borders` tree effect —
                // the correct Civ 6 rule that a civ without Early Empire has no
                // enforced border — and this mirror does not model a RIVAL's
                // civics, so it answers "free passage" for every rival by
                // default. Consulting it would defeat the seal in exactly the
                // live case it exists for. Sealing ground we could in truth have
                // crossed costs a peacetime shortcut; not sealing it cost 74
                // turns of a scout re-sending one blocked step.
                //
                // An EXPLICIT grant is a different fact from a default: when
                // the export says this rival granted us Open Borders (the
                // import above wrote it onto the seat), the host itself will
                // let our units through, so sealing would waste exactly the
                // passage the buy lane just paid for.
                let granted = game
                    .players
                    .get(seat)
                    .and_then(|p| p.open_borders_until.get(&0))
                    .is_some_and(|until| *until > game.turn);
                if !game.is_at_war(0, seat) && !granted {
                    sealed.insert(pos);
                    if is_major(seat) {
                        *sealed_by.entry(seat).or_insert(0) += 1;
                    }
                }
                // Nor ours to count as territory — see the arm above.
                assign.push((pos, None));
            }
        }
    }
    // State can report a city before the terrain export includes its centre.
    // `plant_city` correctly refuses to invent that missing terrain, but the
    // host still enforces the city's four-tile settlement floor on every
    // revealed neighbour. Reserve the visible portion of that floor without
    // inventing a city, its yields, ownership, or diplomacy.
    for city in state
        .cities
        .iter()
        .chain(state.rivals.iter().flat_map(|rival| rival.cities.iter()))
        .chain(state.minors.iter().flat_map(|minor| minor.cities.iter()))
    {
        let centre = crate::hex::offset_to_axial(city.x, city.y);
        if game.city_at(centre).is_some() {
            continue;
        }
        for site in game.wdisk(centre, 3) {
            if game.map.tiles.contains_key(&site) {
                blocked.insert(site);
            }
        }
    }
    Arc::make_mut(&mut game.blocked_city_sites).extend(blocked);
    // Assigned, not extended: recomputed from the export every turn, so ground
    // that opens — a war declared, borders granted, or simply their city coming
    // into view so the ordinary gate takes over — stops being sealed next turn.
    game.closed_borders = sealed;
    game.unseen_major_borders = unseen_major;
    game.sealed_border_owners = sealed_by;
    for (pos, owner) in assign {
        let previous = game.map.tiles.get(&pos).and_then(|tile| tile.owner_city);
        if previous == owner {
            continue;
        }
        if let Some(old) = previous.and_then(|cid| game.cities.get_mut(&cid)) {
            old.owned_tiles.retain(|held| *held != pos);
        }
        if let Some(tile) = game.map.tiles.get_mut(&pos) {
            tile.owner_city = owner;
        }
        if let Some(new) = owner.and_then(|cid| game.cities.get_mut(&cid)) {
            if !new.owned_tiles.contains(&pos) {
                new.owned_tiles.push(pos);
            }
        }
    }
    apply_foreign_infrastructure(game, snapshot);
}

/// Put the districts and wonders the tiles export shows on rival and
/// city-state ground onto the city that owns it.
///
/// A rival city record carries no districts, so until the plot export named
/// them (`d`, `wo`) a rival's economy and defence were modelled from
/// population alone. Our own cities are left to their city record, which
/// carries completion and pillage; here a plot with a district is a standing
/// district. Rebuilt from the export each time it arrives, so a razed or
/// captured district does not linger.
fn apply_foreign_infrastructure(game: &mut crate::game::Game, snapshot: &Snapshot) {
    // Per player: the districts seen on their plots, and the wonders.
    type Placed =
        std::collections::BTreeMap<u32, (Vec<(Name, crate::Pos)>, Vec<(Name, crate::Pos)>)>;
    let mut placed: Placed = Default::default();
    let mut any_seen: std::collections::BTreeSet<u32> = Default::default();
    for y in 0..snapshot.height.max(1) {
        for x in 0..snapshot.width.max(1) {
            let Some(plot) = snapshot.plot((x, y)) else {
                continue;
            };
            if plot.d.is_none() && plot.wo.is_none() {
                continue;
            }
            let pos = crate::hex::offset_to_axial(x, y);
            let Some(cid) = game.map.get(pos).and_then(|tile| tile.owner_city) else {
                continue;
            };
            let Some(city) = game.cities.get(&cid) else {
                continue;
            };
            if city.owner == 0 {
                continue;
            }
            any_seen.insert(cid);
            let entry = placed.entry(cid).or_default();
            if let Some(kind) = plot.wo.as_deref() {
                if let Some(name) = civvis_node_name(&game.rules.wonders, kind, "BUILDING_") {
                    entry.1.push((Name::new(&name), pos));
                }
                continue;
            }
            if let Some(kind) = plot.d.as_deref() {
                // A placed-but-unbuilt district is not on the board yet: it
                // pays no adjacency, no route row, no yields of its own.
                if plot.dc == Some(false) {
                    continue;
                }
                if matches!(kind, "DISTRICT_CITY_CENTER" | "DISTRICT_WONDER") {
                    continue;
                }
                if let Some(name) = civvis_node_name(&game.rules.districts, kind, "DISTRICT_") {
                    entry.0.push((Name::new(&name), pos));
                }
            }
        }
    }
    for cid in any_seen {
        let (districts, wonders) = placed.remove(&cid).unwrap_or_default();
        let Some(city) = game.cities.get_mut(&cid) else {
            continue;
        };
        city.districts.clear();
        city.wonders.clear();
        for (name, pos) in &districts {
            city.districts.insert(*name, *pos);
        }
        for (name, pos) in &wonders {
            city.wonders.insert(*name, *pos);
        }
        for (name, pos) in districts {
            if let Some(tile) = game.map.tiles.get_mut(&pos) {
                tile.improvement = None;
                tile.district = Some(name);
                tile.district_foundation = None;
                tile.wonder = None;
            }
        }
        for (name, pos) in wonders {
            if let Some(tile) = game.map.tiles.get_mut(&pos) {
                tile.improvement = None;
                tile.district = None;
                tile.district_foundation = None;
                tile.wonder = Some(name);
            }
        }
    }
}

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
        let loyalty = forecast.city_loyalty_per_turn(&forecast.cities[&city]);
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
    /// The turn and treasury of the previous sync, so net income can be derived.
    /// See [`LiveMirror::mirror_net_income`] — the export carries the treasury
    /// BALANCE and no rate at all, and `gold_per_turn` is what CIVVIS's
    /// bankruptcy response reads.
    last_treasury: Option<(u32, f64)>,
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
            // Seed the baseline here so the very first sync can be differenced.
            // A resync rebuilds through this constructor too, which is exactly
            // when the previous baseline has become meaningless.
            last_treasury: match state.gold >= 0 {
                true => Some((state.turn.max(1), state.gold as f64)),
                false => None,
            },
        }
    }

    /// ★★★★★ CARRY THE TREASURY BASELINE ACROSS A `--fresh-board` REBUILD.
    ///
    /// `mirror_net_income` differences the treasury between CONSECUTIVE turns and
    /// `last_treasury` lives on this struct — but the bridge runs
    /// `civvis_orders --serve --fresh-board`, which builds a NEW `LiveMirror`
    /// every turn and reaches `decide` without ever calling `sync`. So the only
    /// place the rate is derived is never executed, `last_treasury` is re-seeded
    /// to the current turn on every construction, and `gold_per_turn` keeps its
    /// `0.0` default for the whole game.
    ///
    /// Measured by instrumenting `faith_military_is_affordable` and replaying run
    /// `civvis-20260803T044538Z`: **`gold_per_turn` was 0.00 in 963 of 963 live
    /// decisions.** Nothing that reads it can work:
    ///
    /// - `economic_recovery` in `BasicAi::product_for` needs `gold_per_turn < -0.5`,
    ///   so the entire bankruptcy response **cannot fire in deployment** — which is
    ///   the standing explanation for `civvis-civ6-the-treasury-confiscates-the-army`
    ///   recurring after it was called fixed.
    /// - the income arm of `#962`'s faith-purchase gate allowed **0 of 737**
    ///   decisions; every allowance came from its balance arm instead.
    ///
    /// `src/bin/civvis_orders.rs` already carries `ours` and the unit-id memory
    /// across the same rebuild for the same reason. This is the treasury's turn.
    pub fn carry_treasury_baseline(&mut self, previous: Option<(u32, f64)>) {
        let Some(previous) = previous else { return };
        self.last_treasury = Some(previous);
        let gold = self.game.players[0].gold;
        if let Some(net) = self.mirror_net_income(self.game.turn, gold) {
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
                    apply_city_capture(live, city, &capture_seats);
                    // Firaxis exports the current item, not a speculative
                    // multi-item queue.  Clear even when the item is absent: a
                    // finished build is an empty queue in the real game, not the
                    // last thing CIVVIS happened to see.
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

        // This host rule is permanent, unlike a recent refusal cooldown. Apply it after
        // each replacement and after newly observed cities have been placed.
        seat_live_spies(&mut self.game);
        block_live_spy_production(&mut self.game, state.spy_capacity);

        // --- rivals ----------------------------------------------------------
        // Rebuilt wholesale: what we can see of them is fog-dependent and they carry
        // no plan of ours worth preserving.
        if skip_rivals {
            apply_governor_state(&mut self.game, state, &mut self.unmapped);
            apply_great_person_points(&mut self.game, state, &mut self.unmapped);
            apply_strategic_stockpiles(&mut self.game, state, &mut self.unmapped);
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

        for (index, rival) in state.rivals.iter().enumerate() {
            let owner = index + 1;
            if owner >= self.game.players.len() {
                break;
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

        let mut seat_of_host: std::collections::BTreeMap<usize, usize> = state
            .rivals
            .iter()
            .enumerate()
            .map(|(index, rival)| (rival.player, index + 1))
            .collect();
        seat_of_host.insert(0, 0);
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
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod transient_refusal_tests;

#[cfg(test)]
mod host_fact_tests;
