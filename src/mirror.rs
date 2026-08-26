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
    pub fw: bool,
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
        }
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

#[cfg(test)]
mod tests {

    /// ⚠ AN ENEMY UNIT CIVVIS CANNOT SEE IS WORSE THAN A COSMETIC GAP.
    ///
    /// Civilization VI names uniques by CIVILIZATION. Stripping that qualifier
    /// from `UNIT_EGYPTIAN_CHARIOT_ARCHER` gives `chariot_archer`, but
    /// `data/units.json` calls it **maryannu_chariot_archer**, so neither
    /// spelling matched and the unit vanished from the board. Live on
    /// `civvis-20260804T233745Z`:
    ///
    ///     UNITDATA ⚠ UNIT_EGYPTIAN_CHARIOT_ARCHER@(39, 24) count Civ6=1 CIVVIS=0
    #[test]
    fn a_unique_unit_resolves_through_its_noun() {
        let rules = crate::rules::Rules::embedded();
        assert_eq!(
            resolved_civvis_unit_name(&rules, "UNIT_EGYPTIAN_CHARIOT_ARCHER").as_deref(),
            Some("maryannu_chariot_archer"),
            "the observed live failure must resolve"
        );
        // The ordinary paths must keep working exactly as before.
        assert_eq!(
            resolved_civvis_unit_name(&rules, "UNIT_WARRIOR").as_deref(),
            Some("warrior")
        );
        assert_eq!(
            resolved_civvis_unit_name(&rules, "UNIT_ROMAN_LEGION").as_deref(),
            Some("legion"),
            "the civ-qualifier fallback already handled this and must not regress"
        );
        // A Great Person is a MODELLING gap, not a naming one — there is no
        // entry to find and inventing one would be worse than reporting none.
        assert_eq!(
            resolved_civvis_unit_name(&rules, "UNIT_GREAT_SCIENTIST").as_deref(),
            None
        );
        // And a name that matches nothing must stay unresolved.
        assert_eq!(
            resolved_civvis_unit_name(&rules, "UNIT_NOT_A_REAL_UNIT").as_deref(),
            None
        );
    }

    use super::*;

    fn plot(x: i32, y: i32, t: &str) -> Plot {
        Plot {
            x,
            y,
            im: None,
            t: Some(t.to_string()),
            f: None,
            r: None,
            o: -1,
            w: false,
            i: false,
            fw: false,
            rv: 0,
            ri: false,
            ct: None,
            cl: -1,
            p: false,
            d: None,
            dc: None,
            wo: None,
            rt: None,
            rp: false,
        }
    }

    #[test]
    fn the_embedded_vocabulary_is_present_and_complete() {
        // ⚠ The assertion that matters. Every cwd-relative asset read in this
        // project has eventually resolved to None somewhere real — the champion
        // genome, the league roster, and the value net, which has never once
        // loaded. An embedded table cannot do that, and this proves it is not
        // merely embedded but populated.
        let vocab = Vocabulary::embedded();
        assert_eq!(vocab.terrain_count(), 17, "all Civ 6 terrains");
        assert_eq!(vocab.feature_count(), 50, "all Civ 6 features");
        assert_eq!(vocab.resource_count(), 54, "all Civ 6 resources");
    }

    #[test]
    fn persias_pairidaeza_crosses_as_a_real_improvement() {
        let mut site = plot(3, 4, "TERRAIN_GRASS");
        site.im = Some("IMPROVEMENT_PAIRIDAEZA".to_string());
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 1, width: 8, height: 8, chunk: 1, plots: vec![site],
        }]);
        let game = rebuild_game(&snapshot, 2, 1);
        assert_eq!(
            game.map.get(crate::hex::offset_to_axial(3, 4)).unwrap().improvement,
            Some(crate::name!("pairidaeza"))
        );
    }

    #[test]
    fn armaghs_monastery_crosses_as_a_real_improvement() {
        let mut site = plot(3, 4, "TERRAIN_GRASS");
        site.im = Some("IMPROVEMENT_MONASTERY".to_string());
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 1,
            width: 8,
            height: 8,
            chunk: 1,
            plots: vec![site],
        }]);
        let game = rebuild_game(&snapshot, 2, 1);
        assert_eq!(
            game.map
                .get(crate::hex::offset_to_axial(3, 4))
                .unwrap()
                .improvement,
            Some(crate::name!("monastery"))
        );
    }

    #[test]
    fn historical_snapshot_does_not_read_tiles_from_a_future_turn() {
        let dir = std::env::temp_dir().join(format!(
            "civvis-mirror-time-boundary-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("events.jsonl");
        std::fs::write(
            &path,
            [
                r#"{"kind":"tiles","turn":1,"width":8,"height":8,"chunk":1,"plots":[{"x":1,"y":1,"t":"TERRAIN_GRASS"}]}"#,
                r#"{"kind":"tiles","turn":10,"width":8,"height":8,"chunk":1,"plots":[{"x":7,"y":7,"t":"TERRAIN_DESERT"}]}"#,
            ]
            .join("\n"),
        )
        .unwrap();

        let early = snapshot_from_events_at(&path, Some(1)).unwrap();
        assert_eq!(early.revealed_count(), 1);
        assert!(early.plot((1, 1)).is_some());
        assert!(early.plot((7, 7)).is_none(), "turn 1 must not see turn 10");
        let latest = snapshot_from_events(&path).unwrap();
        assert_eq!(latest.revealed_count(), 2);
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_dir(dir);
    }

    /// ★★★★ A TILES DELTA IS NEW GROUND, NOT A NEW SWEEP. The mod sends what
    /// a unit revealed since the last board went out, every turn and frame,
    /// stamped `delta`. It must merge onto the map like any chunk — that is
    /// the whole point — and must NOT move the snapshot's sweep turn, or the
    /// `improved` fold would discard every improvement finished between the
    /// real sweep and the delta (rule 3 of `apply_finished_improvements`).
    #[test]
    fn a_tiles_delta_merges_new_ground_without_standing_for_a_sweep() {
        let dir =
            std::env::temp_dir().join(format!("civvis-mirror-tiles-delta-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("events.jsonl");
        std::fs::write(
            &path,
            [
                r#"{"kind":"tiles","turn":1,"width":8,"height":8,"chunk":1,"plots":[{"x":1,"y":1,"t":"TERRAIN_GRASS"}]}"#,
                r#"{"kind":"improved","turn":3,"x":1,"y":1,"im":"IMPROVEMENT_FARM"}"#,
                r#"{"kind":"tiles","turn":5,"width":8,"height":8,"chunk":1,"delta":true,"frame":1,"plots":[{"x":2,"y":1,"t":"TERRAIN_PLAINS"}]}"#,
            ]
            .join("\n"),
        )
        .unwrap();

        let snapshot = snapshot_from_events(&path).unwrap();
        assert_eq!(
            snapshot.revealed_count(),
            2,
            "the delta's plot is on the map"
        );
        assert_eq!(
            snapshot.plot((2, 1)).and_then(|plot| plot.t.as_deref()),
            Some("TERRAIN_PLAINS")
        );
        assert_eq!(snapshot.turn, 1, "the sweep turn is the last FULL sweep's");
        assert_eq!(
            snapshot.plot((1, 1)).and_then(|plot| plot.im.as_deref()),
            Some("IMPROVEMENT_FARM"),
            "an improvement finished after the sweep survives a later delta"
        );

        // Stream order decides a plot, whichever kind of chunk carried it:
        // a later sweep overrides an earlier delta's owner, and a later
        // delta overrides the sweep's.
        std::fs::write(
            &path,
            [
                r#"{"kind":"tiles","turn":5,"width":8,"height":8,"chunk":1,"delta":true,"plots":[{"x":2,"y":1,"t":"TERRAIN_PLAINS","o":3}]}"#,
                r#"{"kind":"tiles","turn":25,"width":8,"height":8,"chunk":1,"plots":[{"x":2,"y":1,"t":"TERRAIN_PLAINS","o":-1}]}"#,
                r#"{"kind":"tiles","turn":26,"width":8,"height":8,"chunk":1,"delta":true,"plots":[{"x":2,"y":1,"t":"TERRAIN_PLAINS","o":4}]}"#,
            ]
            .join("\n"),
        )
        .unwrap();
        let at_sweep = snapshot_from_events_at(&path, Some(25)).unwrap();
        assert_eq!(at_sweep.plot((2, 1)).map(|plot| plot.o), Some(-1));
        assert_eq!(at_sweep.turn, 25);
        let latest = snapshot_from_events(&path).unwrap();
        assert_eq!(latest.plot((2, 1)).map(|plot| plot.o), Some(4));
        assert_eq!(latest.turn, 25, "the delta at turn 26 is not a sweep");
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_dir(dir);
    }

    #[test]
    fn recent_host_production_refusals_are_city_scoped_typed_and_expire() {
        let dir = std::env::temp_dir().join(format!(
            "civvis-production-refusal-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        let path = dir.join("events.jsonl");
        std::fs::write(
            &path,
            concat!(
                r#"{"kind":"civvis_build_unplayable","turn":81,"city":12,"item":"BUILDING_UNIVERSITY"}"#,
                "\n",
                r#"{"kind":"civvis_build_unplayable","turn":89,"city":12,"item":"BUILDING_LIBRARY","reasons":["LOC_BUILDING_REQUIRES_DISTRICT"]}"#,
                "\n",
                r#"{"kind":"civvis_build_unplayable","turn":90,"city":14,"item":"PROJECT_ENHANCE_DISTRICT_THEATER"}"#,
                "\n",
                r#"{"kind":"civvis_build_unplayable","turn":91,"city":12,"item":"DISTRICT_CAMPUS"}"#,
                "\n",
                r#"{"kind":"purchase_refused","turn":80,"city":12,"item":"UNIT_BUILDER"}"#,
                "\n",
                r#"{"kind":"purchase_refused","turn":89,"city":12,"item":"UNIT_SETTLER","balance":768,"cost":220}"#,
                "\n",
                r#"{"kind":"purchase_refused","turn":90,"city":14,"item":"BUILDING_LIBRARY"}"#,
                "\n",
                r#"{"kind":"purchase_refused","turn":90,"city":14,"item":"DISTRICT_CAMPUS"}"#,
                "\n",
            ),
        )
        .expect("write events");

        let refused = refused_production(&path, 90);
        assert_eq!(
            refused.get(&12),
            Some(&std::collections::BTreeSet::from([
                "BUILDING_LIBRARY".to_string()
            ])),
            "the stale University, future Campus, and unsupported district event are absent"
        );
        assert_eq!(
            refused.get(&14),
            Some(&std::collections::BTreeSet::from([
                "PROJECT_ENHANCE_DISTRICT_THEATER".to_string()
            ]))
        );

        let rules = crate::rules::Rules::embedded();
        let city_ids = BTreeMap::from([(41, 12), (42, 14)]);
        let blocked = blocked_production_from(&refused, &city_ids, &rules);
        assert_eq!(
            blocked.get(&41),
            Some(&std::collections::BTreeSet::from([
                "building:library".to_string()
            ]))
        );
        assert_eq!(
            blocked.get(&42),
            Some(&std::collections::BTreeSet::from([
                "project:theater_square_festival".to_string()
            ])),
            "Firaxis's district-project name must translate through the same alias as orders"
        );
        let purchase_refusals = refused_purchases(&path, 90);
        assert_eq!(
            purchase_refusals.get(&12),
            Some(&std::collections::BTreeSet::from([
                "UNIT_SETTLER".to_string()
            ])),
            "an old purchase refusal expires while the current Settler refusal remains"
        );
        let blocked_purchases = blocked_production_from(&purchase_refusals, &city_ids, &rules);
        assert_eq!(
            blocked_purchases.get(&41),
            Some(&std::collections::BTreeSet::from([
                "unit:settler".to_string()
            ]))
        );
        assert_eq!(
            blocked_purchases.get(&42),
            Some(&std::collections::BTreeSet::from([
                "building:library".to_string(),
                "district:campus".to_string(),
            ])),
            "district purchase refusals do not need a production-placement plot"
        );

        let mut game = crate::game::Game::new(1, 20, 14, 73_001, 120, 0);
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .expect("starting settler");
        game.apply(0, &crate::game::Action::FoundCity { unit: settler })
            .expect("found city");
        let city = game.player_city_ids(0)[0];
        let warrior = crate::game::Item::Unit {
            unit: crate::name!("warrior"),
        };
        assert!(game.can_produce(0, city, &warrior));
        let _ = game.producible_items(0, city);
        game.replace_blocked_production(BTreeMap::from([(
            city,
            std::collections::BTreeSet::from(["unit:warrior".to_string()]),
        )]));
        assert!(
            !game.can_produce(0, city, &warrior),
            "the cooldown must reach the legal-production chokepoint"
        );
        assert!(
            !game.producible_items(0, city).contains(&warrior),
            "and invalidate a production menu cached before the host refusal arrived"
        );

        let settler_item = crate::game::Item::Unit {
            unit: crate::name!("settler"),
        };
        game.blocked_production.clear();
        game.cities.get_mut(&city).unwrap().pop = 4;
        game.players[0].gold = 10_000.0;
        assert!(game.can_produce(0, city, &settler_item));
        assert!(game
            .legal_actions_within(0, crate::game::ActionFamilies::PURCHASES)
            .iter()
            .any(
                |action| matches!(action, crate::game::Action::Buy { city: bought_at, unit, .. }
                if *bought_at == city && unit == "settler")
            ));
        game.replace_blocked_purchases(BTreeMap::from([(
            city,
            std::collections::BTreeSet::from(["unit:settler".to_string()]),
        )]));
        assert!(
            !game
                .legal_actions_within(0, crate::game::ActionFamilies::PURCHASES)
                .iter()
                .any(
                    |action| matches!(action, crate::game::Action::Buy { city: bought_at, unit, .. }
                    if *bought_at == city && unit == "settler")
                ),
            "the rejected host purchase must leave the purchase menu"
        );
        assert!(
            !game.legal_purchase_actions(0).iter().any(
                |action| matches!(action, crate::game::Action::Buy { city: bought_at, unit, .. }
                if *bought_at == city && unit == "settler")
            ),
            "the city-parallel purchase projection must enforce the same cooldown"
        );
        assert!(
            game.can_produce(0, city, &settler_item),
            "a purchase refusal must not suppress the production fallback"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A civilization-unique unit resolves; a Great Person does not, and must not be
    /// forced to by stripping a prefix that is not a civilization.
    ///
    /// ⚠ Both halves matter. Run `civvis-20260731T114437Z` dropped 175 units: 162
    /// `UNIT_AZTEC_EAGLE_WARRIOR` (a real translation failure, since CIVVIS has
    /// `eagle_warrior`) and 13 `UNIT_GREAT_GENERAL` (not a failure — CIVVIS models
    /// Great People in `great_people.json`, not as units). Stripping the first token
    /// unconditionally fixes the first and mis-reads the second as the civilization
    /// "great".
    #[test]
    fn a_civ_qualifier_is_stripped_and_great_is_not() {
        assert_eq!(
            civvis_unit_name_unqualified("UNIT_AZTEC_EAGLE_WARRIOR").as_deref(),
            Some("eagle_warrior"),
            "a civ-unique unit is the bare unit; this is the 162"
        );
        assert_eq!(
            resolved_civvis_unit_name(
                &crate::rules::Rules::embedded(),
                "UNIT_MONGOLIAN_KESHIG"
            )
            .as_deref(),
            Some("keshig"),
            "a visible Keshig is military intelligence and must reach the board"
        );
        assert_eq!(
            resolved_civvis_unit_name(
                &crate::rules::Rules::embedded(),
                "UNIT_POLISH_HUSSAR"
            )
            .as_deref(),
            Some("winged_hussar")
        );
        assert_eq!(
            resolved_civvis_unit_name(
                &crate::rules::Rules::embedded(),
                "UNIT_ETHIOPIAN_OROMO_CAVALRY"
            )
            .as_deref(),
            Some("oromo_cavalry"),
            "the rival unit observed on fixed22 must reach the mirror board"
        );
        assert_eq!(
            resolved_civvis_unit_name(
                &crate::rules::Rules::embedded(),
                "UNIT_SCOTTISH_HIGHLANDER"
            )
            .as_deref(),
            Some("ranger"),
            "Firaxis declares the Highlander as Scotland's Ranger replacement"
        );
        assert_eq!(
            resolved_civvis_unit_name(
                &crate::rules::Rules::embedded(),
                "UNIT_KOREAN_HWACHA"
            )
            .as_deref(),
            Some("field_cannon"),
            "Firaxis declares the Hwacha as Korea's Field Cannon replacement"
        );
        assert_eq!(
            civvis_unit_name_unqualified("UNIT_GREAT_GENERAL"),
            None,
            "`great` is not a civilization, so there is no qualifier to remove"
        );
        assert_eq!(
            civvis_unit_name_unqualified("UNIT_SETTLER"),
            None,
            "a single-token name has no qualifier at all"
        );
    }

    /// Every Great Person is recognised as one, whatever profession it is.
    #[test]
    fn great_people_are_named_as_a_modelling_gap_not_a_translation_failure() {
        for civ6 in [
            "UNIT_GREAT_GENERAL",
            "UNIT_GREAT_PROPHET",
            "UNIT_GREAT_MERCHANT",
            "UNIT_GREAT_ADMIRAL",
            "UNIT_GREAT_ENGINEER",
        ] {
            assert!(is_great_person(civ6), "{civ6} is a Great Person");
        }
        for civ6 in ["UNIT_SETTLER", "UNIT_AZTEC_EAGLE_WARRIOR", "UNIT_WARRIOR"] {
            assert!(!is_great_person(civ6), "{civ6} is an ordinary unit");
        }
    }

    /// Revealed ground is the truth, in both directions.
    ///
    /// Unseen ground has its own explicit terrain state. Whether a bounded frontier may
    /// be probed is recorded separately and must never change that terrain into a guess.
    #[test]
    fn revealed_ground_is_the_truth_in_both_directions() {
        let chunks = vec![TilesChunk {
            turn: 4,
            width: 20,
            height: 20,
            chunk: 1,
            plots: vec![plot(5, 5, "TERRAIN_GRASS"), plot(6, 5, "TERRAIN_OCEAN")],
        }];
        let snapshot = Snapshot::from_chunks(&chunks);
        let game = rebuild_game(&snapshot, 4, 7);

        let seen = game.map.get(crate::hex::offset_to_axial(5, 5)).unwrap();
        assert_eq!(seen.terrain.as_str(), "grassland", "revealed grass is grass");
        assert!(!game.rules.is_water(seen), "and it is not water");

        let seen_water = game.map.get(crate::hex::offset_to_axial(6, 5)).unwrap();
        assert!(
            game.rules.is_water(seen_water),
            "revealed ocean really is water — no frontier change may turn the sea into \
             land where the seat has actually looked"
        );

        let unseen = game.map.get(crate::hex::offset_to_axial(15, 15)).unwrap();
        assert_eq!(
            unseen.terrain.as_str(),
            "unknown",
            "unrevealed terrain must not secretly be generated land or ocean"
        );
        assert!(
            !unseen.assumed_traversable,
            "a bare reconstruction has no planning prior"
        );
    }

    /// ★★★★ Two camps seven tiles from Rome for a whole game, 121 attacks on
    /// their raiders and none on the camps, eight of fourteen Settlers captured
    /// (civvis-20260816T155856Z): the tile carried `barbarian_camp` and
    /// `game.barb_camps` — what the home guard, the settle risk and
    /// `defensibility` read — stayed empty. The host's camps now reach the
    /// register on every apply, and a camp the host cleared leaves it.
    #[test]
    fn the_hosts_barbarian_camps_reach_the_boards_camp_register() {
        let mut camp = plot(12, 10, "TERRAIN_GRASS");
        camp.im = Some("IMPROVEMENT_BARBARIAN_CAMP".to_string());
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 40,
            width: 20,
            height: 20,
            chunk: 1,
            plots: vec![plot(10, 10, "TERRAIN_GRASS"), camp],
        }]);
        let game = rebuild_game(&snapshot, 4, 7);
        let camp_pos = crate::hex::offset_to_axial(12, 10);
        assert_eq!(
            game.map.tiles[&camp_pos].improvement.as_deref(),
            Some("barbarian_camp"),
            "the improvement is modelled"
        );
        assert!(
            game.barb_camps.contains_key(&camp_pos),
            "and the camp is in the register the home guard reads: {:?}",
            game.barb_camps
        );
        assert_eq!(game.barb_camps.len(), 1);

        // Cleared by the host: the next apply forgets it.
        let cleared = Snapshot::from_chunks(&[TilesChunk {
            turn: 41,
            width: 20,
            height: 20,
            chunk: 1,
            plots: vec![plot(10, 10, "TERRAIN_GRASS"), plot(12, 10, "TERRAIN_GRASS")],
        }]);
        let mut game = game;
        apply_terrain(&mut game, &cleared);
        assert!(
            game.barb_camps.is_empty(),
            "a camp the host cleared leaves the register: {:?}",
            game.barb_camps
        );
    }

    #[test]
    fn frontier_access_never_turns_unknown_into_mock_land_or_water() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 4,
            width: 20,
            height: 20,
            chunk: 1,
            plots: vec![plot(10, 10, "TERRAIN_GRASS")],
        }]);
        let mut game = rebuild_game(&snapshot, 4, 7);
        grow_frontier(&mut game, &snapshot, 2);

        let center = crate::hex::offset_to_axial(10, 10);
        let frontier = crate::hex::neighbors(center)
            .into_iter()
            .find(|pos| game.map.tiles.contains_key(pos))
            .expect("the revealed center has an in-bounds neighbor");
        let far = crate::hex::offset_to_axial(0, 0);
        for (label, pos) in [("frontier", frontier), ("far", far)] {
            let tile = &game.map.tiles[&pos];
            assert_eq!(
                tile.terrain.as_str(),
                "unknown",
                "{label} undisclosed ground keeps its actual knowledge state"
            );
            assert!(game.rules.is_unknown(tile));
            assert_eq!(
                game.rules.tile_yields(tile),
                crate::rules::Yields::default(),
                "unknown ground cannot leak generated yields"
            );
            assert_eq!(
                serde_json::to_value(tile).unwrap()["terrain"],
                "unknown",
                "the serialized board exposes the unknown underneath"
            );
        }
        assert!(game.map.tiles[&frontier].assumed_traversable);
        assert!(game.rules.is_passable(&game.map.tiles[&frontier]));
        assert!(!game.map.tiles[&far].assumed_traversable);
        assert!(!game.rules.is_passable(&game.map.tiles[&far]));

        apply_terrain(&mut game, &snapshot);
        assert_eq!(game.map.tiles[&frontier].terrain.as_str(), "unknown");
        assert!(
            game.map.tiles[&frontier].assumed_traversable,
            "an authoritative terrain refresh must not erase the separately owned prior"
        );

        let warrior = game.spawn_test_unit("warrior", 0, center);
        let galley = game.spawn_test_unit("galley", 0, center);
        assert!(
            game.unit_can_traverse(warrior, frontier),
            "land explorers may probe the terrain-neutral frontier"
        );
        assert!(
            game.unit_can_traverse(galley, frontier),
            "naval explorers may probe it without calling it water underneath"
        );

        let saved = serde_json::to_string(&game).expect("the mirror game saves");
        let loaded: crate::game::Game =
            serde_json::from_str(&saved).expect("the mirror game reloads");
        assert!(loaded.rules.is_unknown(&loaded.map.tiles[&frontier]));
        assert!(loaded.map.tiles[&frontier].assumed_traversable);

        grow_frontier(&mut game, &snapshot, 0);
        assert_eq!(game.map.tiles[&frontier].terrain.as_str(), "unknown");
        assert!(!game.map.tiles[&frontier].assumed_traversable);
        assert!(!game.unit_can_traverse(warrior, frontier));
        assert!(!game.unit_can_traverse(galley, frontier));
    }

    /// ★★★★★ A coast revealed to the horizon walled the fleet in. The land
    /// prior is grown from revealed land and stops at every revealed tile, so
    /// the fog beyond a city's three rings of charted water was reached from
    /// nothing: no ship could plan toward it, and the naval recon arm read the
    /// sea as finished. Live run `civvis-20260818T225716Z`: t169, Ostia coastal
    /// since t44, Cartography in hand, no hull ever built, 559 of 3404 plots
    /// seen. The sea now grows its own prior from revealed water; ships read
    /// it, the land army does not, and the arm sees water left to chart.
    #[test]
    fn the_fog_beyond_charted_water_is_the_seas_frontier() {
        let center = crate::hex::offset_to_axial(10, 10);
        let mut plots = vec![plot(10, 10, "TERRAIN_GRASS")];
        for y in 0..20 {
            for x in 0..20 {
                let d = crate::hex::distance(crate::hex::offset_to_axial(x, y), center);
                if (1..=3).contains(&d) {
                    plots.push(plot(x, y, "TERRAIN_COAST"));
                }
            }
        }
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 44,
            width: 20,
            height: 20,
            chunk: 1,
            plots,
        }]);
        let mut game = rebuild_game(&snapshot, 4, 7);
        game.players[0].techs.insert(crate::name!("sailing"));
        apply_explored(&mut game, &snapshot);
        let city = game.place_city(0, center, None);
        assert!(crate::ai::BasicAi::empire_is_coastal(&game, 0));

        // Before the sea prior existed this is where every live game stood: no
        // frontier at the water's edge, so nothing anywhere for a ship to seek.
        assert!(
            !crate::ai::BasicAi::unseen_water_remains(&game, 0),
            "a bare reconstruction has no sea frontier"
        );

        grow_frontier(&mut game, &snapshot, 2);
        let mut fog_by_ring: std::collections::BTreeMap<i32, Vec<crate::Pos>> = Default::default();
        for (pos, tile) in &game.map.tiles {
            if game.rules.is_unknown(tile) {
                fog_by_ring
                    .entry(crate::hex::distance(*pos, center))
                    .or_default()
                    .push(*pos);
            }
        }
        // Two rings beyond the charted water carry the sea prior; the land
        // prior reaches nothing, because every neighbour of the island is
        // revealed water and the growth never crosses revealed ground.
        for ring in [4, 5] {
            for pos in &fog_by_ring[&ring] {
                let tile = &game.map.tiles[pos];
                assert!(
                    tile.assumed_navigable,
                    "ring {ring} {pos:?} is the sea's frontier"
                );
                assert!(
                    !tile.assumed_traversable,
                    "ring {ring} {pos:?} is not land's"
                );
                assert!(!game.rules.is_passable(tile), "the land prior is untouched");
            }
        }
        for pos in &fog_by_ring[&6] {
            assert!(
                !game.map.tiles[pos].assumed_navigable,
                "depth 2 stops at ring 5"
            );
        }
        let edge = fog_by_ring[&4][0];
        let shore = game
            .nbrs(edge)
            .into_iter()
            .find(|pos| game.rules.is_water(&game.map.tiles[pos]))
            .expect("ring 4 touches the charted coast");
        let galley = game.spawn_test_unit("galley", 0, shore);
        let warrior = game.spawn_test_unit("warrior", 0, shore);
        assert!(
            game.unit_can_traverse(galley, edge),
            "a ship may plan toward the fog beyond charted water"
        );
        assert!(
            !game.unit_can_traverse(warrior, edge),
            "the land army may not — `come_ashore` keeps it dry, and fog with no \
             domain must not smuggle it back to sea"
        );

        // And the arm that buys the empire's naval eye sees water left to chart.
        assert!(crate::ai::BasicAi::unseen_water_remains(&game, 0));
        assert!(
            crate::ai::BasicAi::naval_recon_ship_can_chart(&game, 0, galley),
            "the galley on the shore can chart from where it stands"
        );
        game.remove_unit(galley);
        let mut ai = crate::ai::BasicAi::default();
        ai.enable_naval_recon();
        assert!(
            ai.naval_recon_is_the_missing_arm(&game, 0),
            "with no hull afloat and fog past the coast, the sea scout is the missing arm"
        );
        assert!(
            ai.best_naval_recon(&game, 0, city).is_some(),
            "the coastal city can lay the hull down"
        );

        // A refresh of the authoritative terrain keeps the separately owned
        // prior; a save round-trips it; depth 0 clears it.
        apply_terrain(&mut game, &snapshot);
        assert!(game.map.tiles[&edge].assumed_navigable);
        let saved = serde_json::to_string(&game).expect("the mirror game saves");
        let loaded: crate::game::Game =
            serde_json::from_str(&saved).expect("the mirror game reloads");
        assert!(loaded.map.tiles[&edge].assumed_navigable);
        grow_frontier(&mut game, &snapshot, 0);
        assert!(!game.map.tiles[&edge].assumed_navigable);
        assert!(!crate::ai::BasicAi::unseen_water_remains(&game, 0));
        assert!(!ai.naval_recon_is_the_missing_arm(&game, 0));
    }

    #[test]
    fn a_revealed_but_untranslatable_terrain_is_still_unknown_underneath() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 4,
            width: 10,
            height: 10,
            chunk: 1,
            plots: vec![plot(5, 5, "TERRAIN_FROM_A_MOD")],
        }]);
        let game = rebuild_game(&snapshot, 4, 7);
        let tile = &game.map.tiles[&crate::hex::offset_to_axial(5, 5)];
        assert_eq!(tile.terrain.as_str(), "unknown");
        assert!(!tile.assumed_traversable);
        assert!(game.rules.is_unknown(tile));
    }

    /// ★★★★★ The seat must know which ground it has seen, or every adjacent tile
    /// looks like a frontier and the explorer shuffles in place.
    #[test]
    fn the_seat_knows_which_ground_it_has_seen() {
        let chunks = vec![TilesChunk {
            turn: 4,
            width: 20,
            height: 20,
            chunk: 1,
            plots: vec![plot(5, 5, "TERRAIN_GRASS"), plot(5, 6, "TERRAIN_GRASS")],
        }];
        let snapshot = Snapshot::from_chunks(&chunks);
        let game = rebuild_game(&snapshot, 4, 7);
        let explored = &game.players[0].explored;

        // ⚠ This assertion is the one that corrected me. Before the fix this read 35,
        // not 0: `Game::new` generates a CIVVIS map and reveals a start on it, so the
        // set was populated with plots around a capital the real seat has never been
        // near. `apply_explored` must REPLACE that, not extend it.
        assert_eq!(
            explored.len(),
            2,
            "exactly the two plots the mod exported — the generated map's invented \
             start reveal must be gone, not merged with"
        );
        assert!(explored.contains(&crate::hex::offset_to_axial(5, 5)));
        assert!(explored.contains(&crate::hex::offset_to_axial(5, 6)));
        assert!(
            !explored.contains(&crate::hex::offset_to_axial(15, 15)),
            "ground the seat has never seen must not read as explored"
        );

        // Traversability remains a separate frontier policy; merely being unknown is
        // not enough to make a tile reachable.
        assert!(
            game.map
                .tiles
                .keys()
                .any(|pos| !explored.contains(pos)),
            "the seat must not believe it has seen the whole world"
        );
    }

    /// ★★★★★ Explored ground the seat cannot currently see must still be ON THE BOARD.
    ///
    /// This is the operator's report — *"civvis sometimes only shows current
    /// visibility… area has been explored that isn't in civvis map"* — reduced to its
    /// mechanism. `obs.rs` walks `explored` and, for a tile that is not currently
    /// visible, looks it up in `remembered_tiles` inside a `filter_map`; a tile with no
    /// memory is therefore **dropped from the board**, not dimmed. Nothing in this
    /// bridge wrote `remembered_tiles`, so before the fix the seated observation of a
    /// charted continent contained **zero** tiles.
    ///
    /// ⚠ Asserted through `observation_player_view`, not against `remembered_tiles`
    /// directly: the mirror window attaches with `POST /view {"player": 0}` and that is
    /// the surface that was empty. A test that only counted the memory map would pass on
    /// a memory the viewer never consults.
    #[test]
    fn ground_the_seat_has_charted_survives_the_fog_closing_over_it() {
        let plots: Vec<Plot> = (0..6)
            .map(|x| plot(5 + x, 5, "TERRAIN_GRASS"))
            .collect();
        let revealed = plots.len();
        let chunks = vec![TilesChunk {
            turn: 40,
            width: 20,
            height: 20,
            chunk: 1,
            plots,
        }];
        let snapshot = Snapshot::from_chunks(&chunks);
        let game = rebuild_game(&snapshot, 4, 7);

        let seat = &game.players[0];
        assert_eq!(seat.explored.len(), revealed, "the export is the explored set");
        assert_eq!(
            seat.remembered_tiles.len(),
            revealed,
            "and memory must cover it exactly — never more (invented ground from the \
             generated map) and never less (a hole the viewer drops)"
        );

        // ⚠ The test is only meaningful if some charted ground is genuinely under fog.
        // Asserted rather than assumed: `Game::new` reveals a generated start, so which
        // plots the seat can see is a property of the map roll, not of this fixture.
        let visible = game.player_visibility(0);
        let fogged: Vec<crate::Pos> = seat
            .explored
            .iter()
            .filter(|pos| !visible.contains(pos))
            .copied()
            .collect();
        assert!(
            !fogged.is_empty(),
            "no charted plot is under fog, so this fixture cannot exercise the defect"
        );

        let view = crate::obs::observation_player_view(&game, 0);
        let tiles = view["map"]["tiles"].as_array().expect("a board of tiles");
        let on_board: std::collections::BTreeSet<crate::Pos> = tiles
            .iter()
            .filter_map(|tile| {
                let pos = tile["pos"].as_array()?;
                Some((pos[0].as_i64()? as i32, pos[1].as_i64()? as i32))
            })
            .collect();
        assert_eq!(
            tiles.len(),
            revealed,
            "every charted plot must still be on the board once the fog closes over it \
             — before the fix only the currently-visible ones survived, which is the \
             whole defect"
        );
        for pos in &fogged {
            assert!(
                on_board.contains(pos),
                "remembered ground {pos:?} was dropped from the board entirely"
            );
        }

        // And it must arrive as REMEMBERED, not as currently seen. A mirror that
        // reported stale ground as live would be the opposite error, and just as wrong.
        let live: std::collections::BTreeSet<crate::Pos> = view["visible"]
            .as_array()
            .expect("a visible set")
            .iter()
            .filter_map(|pos| {
                let pos = pos.as_array()?;
                Some((pos[0].as_i64()? as i32, pos[1].as_i64()? as i32))
            })
            .collect();
        for pos in &fogged {
            assert!(
                !live.contains(pos),
                "fogged ground {pos:?} must not be reported as currently visible"
            );
        }
    }

    /// ★★★★★ The board's rivers must be Civilization VI's, and ONLY Civilization VI's.
    ///
    /// The generated map `Game::new` builds has its own river network, and nothing used
    /// to remove it — so "does the board have rivers" answered yes while every one of
    /// them was invented. Both halves are asserted here: the exported segment lands on
    /// the right edge of the right tile, **and** no other tile carries a river at all.
    /// The second assertion is the one that fails without `clear_rivers`.
    #[test]
    fn the_rivers_on_the_board_are_the_ones_civ6_exported() {
        let mut wet = plot(5, 6, "TERRAIN_GRASS");
        // W and NE, so the mapping is pinned on two different edges rather than one
        // that might be right by luck.
        wet.rv = 1 | 4;
        let chunks = vec![TilesChunk {
            turn: 8,
            width: 20,
            height: 20,
            chunk: 1,
            plots: vec![
                wet,
                plot(4, 6, "TERRAIN_GRASS"),
                plot(5, 5, "TERRAIN_GRASS"),
                plot(6, 6, "TERRAIN_GRASS"),
            ],
        }];
        let snapshot = Snapshot::from_chunks(&chunks);
        let game = rebuild_game(&snapshot, 4, 7);

        // ⚠⚠ THE ASSERTION THAT MATTERS IS ABOUT THE NEIGHBOUR, NOT THIS PLOT.
        //
        // `IsWOfRiver` means the plot lies WEST OF the river, so the river is on its
        // EAST edge; `IsNEOfRiver` means it lies NORTH-EAST of the river, so the river
        // is on its SOUTH-WEST edge. The first version of this read the flags as
        // "river on the west/north-east edge" and put every segment on the opposite
        // side of the hex.
        //
        // It passed anyway, because `set_river_edge` marks both tiles sharing a
        // segment: the plot that reported the flag came out riverside under either
        // reading, and only the neighbour differed. So this now names the neighbour
        // explicitly, and asserts the OPPOSITE edges carry nothing.
        let pos = crate::hex::offset_to_axial(5, 6);
        let edge = |d: usize| (pos.0 + crate::hex::DIRS[d].0, pos.1 + crate::hex::DIRS[d].1);
        let (east, south_west) = (edge(0), edge(2));
        let (west, north_east) = (edge(3), edge(5));
        assert!(
            game.map.has_river_edge(pos, east),
            "W of the river means the river is on this plot's EAST edge"
        );
        assert!(
            game.map.has_river_edge(pos, south_west),
            "NE of the river means the river is on this plot's SOUTH-WEST edge"
        );
        assert!(
            !game.map.has_river_edge(pos, west),
            "and NOT on the western edge — that is the reading this test exists to \
             rule out, and it is invisible to any check of this plot alone"
        );
        assert!(
            !game.map.has_river_edge(pos, north_east),
            "nor on the north-eastern edge"
        );
        // Written from both sides, so the two tiles cannot disagree about one segment.
        assert!(
            game.map.has_river_edge(east, pos),
            "and the neighbour must carry the same segment"
        );

        assert!(
            game.map.get(pos).is_some_and(|tile| tile.has_river()),
            "the plot itself reads as riverside"
        );

        // ⚠ The assertion that fails without `clear_rivers`. Before this fix the
        // generated world's network survived here: 33 invented river tiles on a live
        // run, only 36.4% of them on ground Civilization VI even calls fresh water.
        let riverside: Vec<crate::Pos> = game
            .map
            .tiles
            .iter()
            .filter(|(_, tile)| tile.has_river())
            .map(|(pos, _)| *pos)
            .collect();
        assert_eq!(
            riverside.len(),
            3,
            "exactly the exporting plot and the two neighbours across its segments \
             carry a river — every other river on this board was invented by the map \
             generator, and found at {riverside:?}"
        );
    }

    #[test]
    fn a_known_river_edge_survives_when_its_firaxis_holder_is_hidden() {
        let mut wet = plot(5, 6, "TERRAIN_GRASS");
        wet.rv = 8;
        wet.ri = true;
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 8,
            width: 20,
            height: 20,
            chunk: 1,
            plots: vec![wet],
        }]);
        let game = rebuild_game(&snapshot, 4, 7);
        let pos = crate::hex::offset_to_axial(5, 6);
        let west = (pos.0 + crate::hex::DIRS[3].0, pos.1 + crate::hex::DIRS[3].1);
        assert!(game.map.has_river_edge(pos, west));
        assert!(game.map.tiles[&pos].has_river());
    }

    /// ★★★★ A card the host has retired must stop being offered.
    ///
    /// `POLICY_ILKUM` was chosen and refused **105 times** on live run
    /// `civvis-20260801T012454Z` — Civilization VI answered `IsPolicyObsolete` every
    /// time and said so in the refusal reason, and nothing read it.
    ///
    /// ⚠ Asserted through `available_policies`, not against `blocked_policies`. That
    /// is the single chokepoint the AI, the observation and `legal_actions` all pass
    /// through, and a test that only checked the field would pass on a set nothing
    /// consults — which is exactly how a populated-but-inert value survives here.
    #[test]
    fn a_card_the_host_retired_stops_being_offered() {
        let mut game = crate::game::Game::new(4, 20, 20, 7, 500, 0);
        // A fresh seat has no civics, so nothing is unlocked yet. Craftsmanship is
        // what the ruleset's own policy test uses to put cards in hand.
        game.players[0].civics.insert(Name::new("craftsmanship"));
        let offered = game.available_policies(0);
        let victim = offered
            .first()
            .cloned()
            .expect("craftsmanship must put at least one card on offer");
        assert!(
            game.available_policies(0).contains(&victim),
            "precondition: the card is on offer before the host retires it"
        );

        game.blocked_policies.insert(victim);
        assert!(
            !game.available_policies(0).contains(&victim),
            "a card the host ruleset retired must not be offered again — this is the \
             105 ILKUM refusals"
        );
        assert_eq!(
            game.available_policies(0).len(),
            offered.len() - 1,
            "and only that card is withdrawn; blocking one must not empty the hand"
        );
    }

    /// The retired cards are already in the stream — no new mod event was needed.
    #[test]
    fn the_hosts_retired_cards_are_read_from_the_refusals_it_already_writes() {
        let dir = std::env::temp_dir().join(format!("civvis-policy-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        let path = dir.join("events.jsonl");
        // Shaped exactly like the live stream: reasons keyed in `refusals`, repeated
        // across turns, mixed with reasons that are not policies at all.
        std::fs::write(
            &path,
            concat!(
                r#"{"kind":"orders","turn":40,"refusals":{"obsolete_POLICY_ILKUM":1,"MOVE_TO":4}}"#,
                "\n",
                r#"{"kind":"orders","turn":41,"refusals":{"obsolete_POLICY_ILKUM":1,"no_params":2}}"#,
                "\n",
                r#"{"kind":"orders","turn":42,"refusals":{"obsolete_POLICY_NOT_A_REAL_CARD":1}}"#,
                "\n",
            ),
        )
        .expect("write events");

        let names = refused_policies(&path);
        assert!(
            names.contains("POLICY_ILKUM"),
            "the reason the agent already writes is the whole source"
        );
        assert_eq!(names.len(), 2, "each distinct card once, however many turns it spans");

        let rules = crate::rules::Rules::embedded();
        let blocked = blocked_policies_from(&names, &rules);
        assert!(
            blocked.contains(&Name::new("ilkum")),
            "and it translates through the shipped policy table"
        );
        assert_eq!(
            blocked.len(),
            1,
            "a card CIVVIS does not model is DROPPED, not inserted under a name that \
             matches nothing — a blocked set full of unmatched names looks populated \
             and filters nothing"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A pantheon a rival holds is already in the stream as `taken_BELIEF_<X>`
    /// — the mod's `pantheon` handler writes it when `IsInSomePantheon` says so
    /// — and until now nothing read it: the mirror seats no rival pantheons, so
    /// the same first choice was re-derived from the same board next turn and,
    /// after two sightings, the mod's blocker fallback chose the first untaken
    /// belief in database order. See `Game::blocked_pantheons` and
    /// `AdvancedAi::expansion_pantheon`.
    #[test]
    fn the_hosts_taken_pantheons_are_read_from_the_refusals_it_already_writes() {
        let dir = std::env::temp_dir().join(format!("civvis-pantheon-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        let path = dir.join("events.jsonl");
        std::fs::write(
            &path,
            concat!(
                r#"{"kind":"orders","turn":18,"refusals":{"taken_BELIEF_RELIGIOUS_SETTLEMENTS":1,"MOVE_TO":4}}"#,
                "
",
                r#"{"kind":"orders","turn":19,"refusals":{"taken_BELIEF_RELIGIOUS_SETTLEMENTS":1,"taken_NOT_A_BELIEF":1}}"#,
                "
",
                r#"{"kind":"orders","turn":20,"refusals":{"taken_BELIEF_NOT_A_REAL_PANTHEON":1,"pantheon_already_founded":1}}"#,
                "
",
                r#"{"kind":"orders","turn":40,"refusals":{"taken_BELIEF_FERTILITY_RITES":1}}"#,
                "
",
            ),
        )
        .expect("write events");

        let names = refused_pantheons(&path);
        assert!(names.contains("BELIEF_RELIGIOUS_SETTLEMENTS"));
        assert!(names.contains("BELIEF_FERTILITY_RITES"));
        assert_eq!(
            names.len(),
            3,
            "each distinct belief once, however many turns it spans; a `taken_` reason              that is not a belief is not one: {names:?}"
        );
        // Bounded by turn, the way every per-turn state read asks for it.
        assert!(
            !refused_pantheons_through(&path, Some(30)).contains("BELIEF_FERTILITY_RITES"),
            "a refusal on turn 40 is not known on turn 30"
        );

        let rules = crate::rules::Rules::embedded();
        let blocked = blocked_pantheons_from(&names, &rules);
        assert!(blocked.contains(&Name::new("religious_settlements")));
        assert!(blocked.contains(&Name::new("fertility_rites")));
        assert_eq!(
            blocked.len(),
            2,
            "a belief CIVVIS does not model is DROPPED, not inserted under a name that              matches nothing"
        );

        // And the board refuses what the host refused, so the chooser moves on.
        let mut game = crate::game::Game::new_full(2, 30, 18, 6_101, 200, 0, false);
        game.current = 0;
        let settler = game
            .player_unit_ids(0)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.apply(0, &crate::game::Action::FoundCity { unit: settler })
            .unwrap();
        game.players[0].faith = 200.0;
        game.blocked_pantheons = blocked;
        assert!(game
            .apply(
                0,
                &crate::game::Action::ChoosePantheon {
                    belief: Name::new("religious_settlements"),
                },
            )
            .is_err());
        assert!(game
            .apply(
                0,
                &crate::game::Action::ChoosePantheon {
                    belief: Name::new("divine_spark"),
                },
            )
            .is_ok());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// ★★★★ The wonder half of `build_no_plot` was being DROPPED ON THE FLOOR.
    ///
    /// The mod emits a refused district under the event's `district` key and a
    /// refused WONDER under `building`. The parser read only `district`, so every
    /// wonder refusal fell straight through it and nothing ever reached the planner.
    /// Measured over 20 live runs: **370 wonder refusals against 55 district ones**,
    /// from 29 distinct (run, city, wonder) combinations — a mean of 12.8 re-asks
    /// each, and 53 consecutive turns at worst of one city ordering one wonder
    /// Civilization VI had no ground for.
    ///
    /// ⚠ Two-sided on purpose: the district side must keep working, and neither key
    /// may leak into the other's set.
    #[test]
    fn a_refused_wonder_is_read_from_the_building_key_not_the_district_key() {
        let dir = std::env::temp_dir().join(format!("civvis-noplot-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        let path = dir.join("events.jsonl");
        // Shaped exactly like the live stream, including the repeat that made this
        // worth fixing and a bare-hash export that names nothing.
        std::fs::write(
            &path,
            concat!(
                r#"{"kind":"build_no_plot","turn":40,"city":65536,"building":"BUILDING_HANGING_GARDENS","x":8,"y":9}"#,
                "\n",
                r#"{"kind":"build_no_plot","turn":41,"city":65536,"building":"BUILDING_HANGING_GARDENS","x":8,"y":9}"#,
                "\n",
                r#"{"kind":"build_no_plot","turn":42,"city":196610,"district":"DISTRICT_THEATER","x":3,"y":4}"#,
                "\n",
                r#"{"kind":"build_no_plot","turn":43,"city":65536,"building":"-1743686858","x":8,"y":9}"#,
                "\n",
            ),
        )
        .expect("write events");

        let wonders = refused_wonders_through(&path, None);
        assert_eq!(
            wonders.get(&65536).map(|set| set.len()),
            Some(1),
            "each distinct wonder once, however many turns it spans"
        );
        assert!(wonders[&65536].contains("BUILDING_HANGING_GARDENS"));
        assert!(
            !wonders.contains_key(&196610),
            "a refused DISTRICT must not appear in the wonder set"
        );

        let districts = refused_districts_through(&path, None);
        assert!(
            districts[&196610].contains("DISTRICT_THEATER"),
            "the district side must keep working"
        );
        assert!(
            !districts.contains_key(&65536),
            "a refused WONDER must not appear in the district set"
        );

        // And it translates through the shipped wonder table, dropping the bare hash
        // rather than inserting a name that matches nothing.
        let rules = crate::rules::Rules::embedded();
        let city_ids: std::collections::BTreeMap<u32, i64> = [(7u32, 65536i64)].into_iter().collect();
        let blocked = blocked_wonders_from(&wonders, &city_ids, &rules);
        assert_eq!(
            blocked.get(&7).map(|set| set.len()),
            Some(1),
            "a wonder CIVVIS does not model is DROPPED, not inserted under a name \
             that matches nothing"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// ★★★★ A refused DISTRICT must cool down like every other production refusal.
    ///
    /// `blocked_production_from` has always had a `DISTRICT_` fallback and
    /// `production_block_key` has always emitted `district:{name}`, but
    /// `refused_production` accepted only `UNIT_`/`BUILDING_`/`PROJECT_`, so no
    /// district name ever reached either and that branch was dead code.
    ///
    /// The prefix list predicted the cooldown exactly. Over 20 live runs, gaps
    /// between successive refusals of the same (run, city, item): every accepted
    /// prefix had **zero** gaps of one turn, and `DISTRICT_` had **13 of them and
    /// none of eight or more** — `DISTRICT_HOLY_SITE` re-proposed in one city on
    /// turns 45 through 58, every consecutive turn, against a TTL of eight.
    ///
    /// ⚠ Asserts the whole chain, not the prefix list: a filter that admits the name
    /// but a translator that drops it would leave the block empty and still pass a
    /// test written against the parser alone.
    #[test]
    fn a_refused_district_cools_down_like_every_other_production_refusal() {
        let dir = std::env::temp_dir().join(format!("civvis-prodref-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        let path = dir.join("events.jsonl");
        std::fs::write(
            &path,
            concat!(
                r#"{"kind":"civvis_build_unplayable","turn":45,"city":65536,"item":"DISTRICT_HOLY_SITE","reasons":[]}"#,
                "\n",
                r#"{"kind":"civvis_build_unplayable","turn":46,"city":65536,"item":"UNIT_SPY","reasons":[]}"#,
                "\n",
                r#"{"kind":"civvis_build_unplayable","turn":30,"city":65536,"item":"DISTRICT_CAMPUS","reasons":[]}"#,
                "\n",
            ),
        )
        .expect("write events");

        // Turn 50: the t45 and t46 refusals are inside the eight-turn window, the
        // t30 one is not.
        let refused = refused_production(&path, 50);
        let names = refused.get(&65536).expect("the city has recent refusals");
        assert!(
            names.contains("DISTRICT_HOLY_SITE"),
            "a district refusal must be carried like any other"
        );
        assert!(names.contains("UNIT_SPY"), "and the kinds that already worked must keep working");
        assert!(
            !names.contains("DISTRICT_CAMPUS"),
            "the TTL still applies — an old refusal is not a permanent ban"
        );

        // ⚠ The half that was dead code: the name has to survive translation into a
        // key `Game::can_produce` actually checks.
        let rules = crate::rules::Rules::embedded();
        let city_ids: std::collections::BTreeMap<u32, i64> = [(7u32, 65536i64)].into_iter().collect();
        let blocked = blocked_production_from(&refused, &city_ids, &rules);
        let keys = blocked.get(&7).expect("translated block for the city");
        assert!(
            keys.contains("district:holy_site"),
            "translated to the same key production_block_key emits, got {keys:?}"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn trade_route_refusals_are_merged_through_the_requested_host_turn() {
        let dir = std::env::temp_dir().join(format!("civvis-route-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        let path = dir.join("events.jsonl");
        // Each pairing is refused twice: this test is about the TURN LIMIT,
        // and a single refusal no longer condemns anything (see
        // `TRADE_ROUTE_REFUSALS_BEFORE_BLOCK`).
        std::fs::write(
            &path,
            concat!(
                r#"{"kind":"state","turn":41}"#,
                "\n",
                r#"{"kind":"trade_route_refused","turn":39,"unit":9,"from_x":6,"from_y":6,"x":9,"y":9}"#,
                "\n",
                r#"{"kind":"trade_route_refused","turn":40,"unit":9,"from_x":6,"from_y":6,"x":9,"y":9}"#,
                "\n",
                r#"{"kind":"trade_route_refused","turn":42,"unit":9,"from_x":6,"from_y":6,"x":10,"y":10}"#,
                "\n",
                r#"{"kind":"trade_route_refused","turn":43,"unit":9,"from_x":6,"from_y":6,"x":10,"y":10}"#,
                "\n",
            ),
        )
        .expect("write events");

        let state = state_from_events(&path, Some(41)).expect("turn 41 state");
        assert_eq!(
            state.refused_trade_routes,
            std::collections::BTreeSet::from([(
                crate::hex::offset_to_axial(6, 6),
                crate::hex::offset_to_axial(9, 9),
            )]),
            "future refusals must not leak into an earlier reconstructed frame"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Three Traders parked in Rome, and the ledger that put them there.
    ///
    /// Live run `civvis-20260822T020434Z` ended with a trade capacity of 20,
    /// only 16 routes running, and four idle Traders. Its refusal ledger holds
    /// 23 distinct pairings, **every one refused exactly once**, and 8 of the
    /// 15 condemned destinations are our OWN cities. `blocked_trade_routes` is
    /// never cleared, so each of those single readings retired a pairing for
    /// the rest of the game and the parked Traders were never offered another.
    #[test]
    fn one_trade_route_refusal_is_a_report_and_two_are_a_verdict() {
        let dir = std::env::temp_dir().join(format!("civvis-route2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        let path = dir.join("events.jsonl");
        let refusal = |turn: u32, x: i32, y: i32| {
            format!(
                r#"{{"kind":"trade_route_refused","turn":{turn},"unit":9,"from_x":6,"from_y":6,"x":{x},"y":{y}}}"#
            )
        };
        std::fs::write(
            &path,
            [
                // Two state anchors: a frame can only be reconstructed at a
                // turn the run actually exported one for.
                r#"{"kind":"state","turn":45}"#.to_string(),
                r#"{"kind":"state","turn":60}"#.to_string(),
                // Refused once, exactly like all 23 pairings in the live run.
                refusal(40, 9, 9),
                // Refused twice: the host has said it twice and means it.
                refusal(41, 12, 12),
                refusal(50, 12, 12),
            ]
            .join("\n")
                + "\n",
        )
        .expect("write events");

        let state = state_from_events(&path, Some(60)).expect("turn 60 state");
        assert_eq!(
            state.refused_trade_routes,
            std::collections::BTreeSet::from([(
                crate::hex::offset_to_axial(6, 6),
                crate::hex::offset_to_axial(12, 12),
            )]),
            "only the corroborated pairing is retired; retiring is forever"
        );

        // And the corroboration must fall inside the reconstructed frame: a
        // second refusal from the future cannot condemn a pairing early.
        let earlier = state_from_events(&path, Some(45)).expect("turn 45 state");
        assert!(
            earlier.refused_trade_routes.is_empty(),
            "the second reading is at turn 50 and this frame is turn 45"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// ★★★★ Landmass identity comes from the export, and invented cliffs come off.
    ///
    /// Same defect as the rivers above, two fields over. On the live board 200 of 776
    /// tiles carried a continent and 576 carried none — the generated world's regions
    /// showing through on a map where every land plot really has one.
    #[test]
    fn the_landmass_is_civ6s_and_the_generated_cliffs_are_gone() {
        let mut home = plot(5, 5, "TERRAIN_GRASS");
        home.ct = Some("CONTINENT_AFRICA".to_string());
        home.cl = 2;
        let mut away = plot(9, 9, "TERRAIN_GRASS");
        away.ct = Some("CONTINENT_ASIA".to_string());
        let mut beside = plot(5, 6, "TERRAIN_GRASS");
        beside.ct = Some("CONTINENT_AFRICA".to_string());
        // Water: Civilization VI gives it no continent, so neither may CIVVIS.
        let sea = plot(6, 5, "TERRAIN_OCEAN");

        let chunks = vec![TilesChunk {
            turn: 12,
            width: 20,
            height: 20,
            chunk: 1,
            plots: vec![home, away, beside, sea],
        }];
        let snapshot = Snapshot::from_chunks(&chunks);
        let game = rebuild_game(&snapshot, 4, 7);

        let at = |x, y| game.map.get(crate::hex::offset_to_axial(x, y)).unwrap();
        assert_eq!(
            at(5, 5).continent,
            at(5, 6).continent,
            "two plots Civilization VI puts on one continent must agree"
        );
        assert_ne!(
            at(5, 5).continent,
            at(9, 9).continent,
            "and two it separates must not — 'another continent' is a rule"
        );
        assert_eq!(
            at(6, 5).continent,
            None,
            "water carries no continent, and must LOSE the generated one rather than \
             keep it"
        );
        assert_eq!(at(5, 5).coastal_lowland, 2, "the flood band crosses");

        // ⚠ The assertion that fails without the clear: 66 invented cliffs on the live
        // board, each able to block embarkation at a shore the real game lets a unit
        // leave from.
        let cliffs = game
            .map
            .tiles
            .values()
            .filter(|tile| tile.cliff_edges.iter().any(|edge| *edge))
            .count();
        assert_eq!(
            cliffs, 0,
            "Civilization VI exposes no cliff accessor, so a cliff on this board can \
             only have been invented by the map generator"
        );
    }

    #[test]
    fn civ6_encodes_hills_in_the_terrain_and_civvis_does_not() {
        let vocab = Vocabulary::embedded();
        assert_eq!(
            vocab.terrain("TERRAIN_GRASS"),
            Resolved::Known((Name::new("grassland"), false))
        );
        assert_eq!(
            vocab.terrain("TERRAIN_GRASS_HILLS"),
            Resolved::Known((Name::new("grassland"), true))
        );
        // A mountain is its own CIVVIS terrain rather than an elevated one.
        assert_eq!(
            vocab.terrain("TERRAIN_GRASS_MOUNTAIN"),
            Resolved::Known((Name::new("mountain"), false))
        );
    }

    #[test]
    fn wonders_whose_two_names_disagree_still_resolve() {
        // These are the pairings that made the first coverage report read 74%:
        // Civ 6 names a wonder by type id, CIVVIS by its common name.
        let vocab = Vocabulary::embedded();
        for (civ6, civvis) in [
            ("FEATURE_DEVILSTOWER", "mato_tipila"),
            ("FEATURE_WHITEDESERT", "sahara_el_beyda"),
            ("FEATURE_CLIFFS_DOVER", "cliffs_of_dover"),
            ("FEATURE_IKKIL", "ik_kil"),
            ("FEATURE_BARRIER_REEF", "great_barrier_reef"),
        ] {
            assert_eq!(
                vocab.feature(civ6),
                Resolved::Known(Name::new(civvis)),
                "{civ6} must resolve"
            );
        }
    }

    #[test]
    fn a_deliberate_exclusion_is_not_the_same_answer_as_a_failure() {
        let vocab = Vocabulary::embedded();
        match vocab.resource("RESOURCE_LEY_LINE") {
            Resolved::Excluded(why) => assert!(
                why.contains("Secret Societies"),
                "the exclusion must carry its reason, got {why:?}"
            ),
            other => panic!("ley line should be excluded, got {other:?}"),
        }
        // And something genuinely absent must be Unknown, never a default.
        assert_eq!(
            vocab.terrain("TERRAIN_INVENTED_BY_NOBODY"),
            Resolved::Unknown("TERRAIN_INVENTED_BY_NOBODY".to_string())
        );
    }

    #[test]
    fn unrevealed_ground_stays_a_hole() {
        // ⚠ The information constraint, made executable. The mod sends only
        // revealed plots; anything absent must read as unknown rather than as
        // whatever a map generator would have put there.
        let chunks = vec![TilesChunk {
            turn: 40,
            width: 10,
            height: 10,
            chunk: 1,
            plots: vec![plot(1, 1, "TERRAIN_GRASS"), plot(1, 2, "TERRAIN_PLAINS")],
        }];
        let snapshot = Snapshot::from_chunks(&chunks);
        assert!(snapshot.is_revealed((1, 1)));
        assert!(!snapshot.is_revealed((5, 5)), "never exported, so unknown");
        assert!(snapshot.plot((5, 5)).is_none());
        assert_eq!(snapshot.revealed_count(), 2);
        // 2 of 100: a ranking computed from this is a very different claim from
        // one computed at 90%, and the caller can see which it has.
        assert!((snapshot.revealed_fraction() - 0.02).abs() < 1e-9);
    }

    #[test]
    fn chunks_reassemble_and_a_re_export_refreshes_rather_than_duplicates() {
        let chunks = vec![
            TilesChunk {
                turn: 40,
                width: 8,
                height: 8,
                chunk: 1,
                plots: vec![plot(0, 0, "TERRAIN_GRASS")],
            },
            TilesChunk {
                turn: 40,
                width: 8,
                height: 8,
                chunk: 2,
                plots: vec![plot(1, 0, "TERRAIN_DESERT"), plot(0, 0, "TERRAIN_TUNDRA")],
            },
        ];
        let snapshot = Snapshot::from_chunks(&chunks);
        assert_eq!(snapshot.revealed_count(), 2, "one entry per plot");
        assert_eq!(
            snapshot.plot((0, 0)).unwrap().t.as_deref(),
            Some("TERRAIN_TUNDRA"),
            "the later chunk wins"
        );
        assert_eq!(snapshot.turn, 40);
    }

    #[test]
    fn untranslatable_ground_is_reported_not_swallowed() {
        let chunks = vec![TilesChunk {
            turn: 1,
            width: 4,
            height: 4,
            chunk: 1,
            plots: vec![plot(0, 0, "TERRAIN_GRASS"), plot(1, 0, "TERRAIN_FROM_A_MOD")],
        }];
        let snapshot = Snapshot::from_chunks(&chunks);
        assert_eq!(
            snapshot.untranslatable(Vocabulary::embedded()),
            vec!["TERRAIN_FROM_A_MOD".to_string()],
            "a type the vocabulary cannot place must surface"
        );
    }

    #[test]
    fn a_revealed_land_plot_becomes_land_and_can_hold_a_city() {
        // ⚠ Written because `rebuild_with_empire` refused to place the capital of a
        // real run on (56,28), a plot the export clearly recorded as TERRAIN_GRASS,
        // not water. Either the tile is not being written or the placement check is
        // wrong, and a test says which.
        let chunks = vec![TilesChunk {
            turn: 10,
            width: 60,
            height: 38,
            chunk: 1,
            plots: vec![Plot {
                x: 56,
                y: 28,
                im: None,
                t: Some("TERRAIN_GRASS".to_string()),
                f: None,
                r: None,
                o: 0,
                w: false,
                i: false,
                fw: true,
                rv: 0,
                ri: false,
                ct: None,
                cl: -1,
                p: false,
                d: None,
                dc: None,
                wo: None,
                rt: None,
                rp: false,
            }],
        }];
        let snapshot = Snapshot::from_chunks(&chunks);
        assert!(snapshot.is_revealed((56, 28)), "the plot must read as revealed");

        let game = rebuild_game(&snapshot, 4, 1);

        let axial = crate::hex::offset_to_axial(56, 28);
        let tile = game
            .map
            .get(axial)
            .expect("the mirrored map must contain a plot the export described");
        assert_eq!(
            tile.terrain.as_str(),
            "grassland",
            "revealed grass must land as grassland, not the generated terrain"
        );
        assert!(
            !game.rules.is_water(tile),
            "a revealed grass plot must not read as water"
        );

        let (with_empire, placed) = rebuild_with_empire(&snapshot, &[(56, 28)], 4, 1);
        assert_eq!(placed, 1, "the capital must be placeable on its own plot");
        assert!(
            with_empire.cities.values().any(|c| c.pos == axial),
            "and the city must actually be there, at the AXIAL position"
        );
    }

    #[test]
    fn the_real_export_shape_deserializes() {
        // Field-for-field what CivvisControlAgent.lua emits, so a rename on
        // either side fails here rather than in a live game.
        let raw = r#"{
            "turn": 25, "width": 44, "height": 26, "chunk": 1,
            "plots": [
                {"x":3,"y":4,"t":"TERRAIN_GRASS_HILLS","f":"FEATURE_FOREST",
                 "r":"RESOURCE_DEER","o":0,"w":false,"i":false,"fw":true},
                {"x":4,"y":4,"t":"TERRAIN_COAST","o":-1,"w":true,"i":false,"fw":false}
            ]
        }"#;
        let chunk: TilesChunk = serde_json::from_str(raw).expect("export shape parses");
        assert_eq!(chunk.plots.len(), 2);
        let hill = &chunk.plots[0];
        assert_eq!(hill.o, 0);
        assert!(hill.fw, "fresh water carries through");
        let vocab = Vocabulary::embedded();
        assert_eq!(
            vocab.terrain(hill.t.as_deref().unwrap()),
            Resolved::Known((Name::new("grassland"), true))
        );
        // A plot with no feature or resource omits them rather than sending 0,
        // which would otherwise be read as a real type.
        assert!(chunk.plots[1].f.is_none() && chunk.plots[1].r.is_none());
    }

    #[test]
    fn new_export_fields_are_reported_instead_of_silently_discarded() {
        let raw = r#"{
            "kind":"state", "ctx":"Gameplay", "run":"contract", "turn":7,
            "cities":[{
                "id":1, "x":2, "y":3, "pantheon_active":"BELIEF_CITY_PATRON_GODDESS",
                "producing_hash":123, "future_city_fact":9,
                "districts":[{"type":"DISTRICT_CAMPUS","x":2,"y":4,"pillaged":false}],
                "wonders":[{"type":"BUILDING_PYRAMIDS","x":1,"y":3}]
            }],
            "units":[{"id":4,"kind":"UNIT_WARRIOR","x":2,"y":3,"combat":20,
                      "ranged":0,"player":0,"formation_count":2,"xp":19,"level":2,
                      "promotions":["PROMOTION_BATTLECRY"],"build_charges":0,
                      "spread_charges":0}],
            "future_empire_fact":true
        }"#;
        let state = state_from_json(raw).expect("the state remains usable");
        assert_eq!(state.cities[0].producing_hash, Some(123));
        assert_eq!(state.units[0].combat, 20.0);
        assert_eq!(state.units[0].formation_count, 2);
        assert_eq!(state.units[0].xp, Some(19));
        assert_eq!(state.units[0].level, Some(2));
        assert_eq!(
            state.units[0].promotions.as_deref(),
            Some(["PROMOTION_BATTLECRY".to_string()].as_slice())
        );
        assert_eq!(
            state.schema_gaps,
            vec![
                "schema:city.future_city_fact".to_string(),
                "schema:state.future_empire_fact".to_string(),
            ],
            "recognized metadata and diagnostic fields stay quiet; every new fact is named"
        );

        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 7, width: 6, height: 6, chunk: 1,
            plots: vec![plot(2, 3, "TERRAIN_GRASS")],
        }]);
        let recon = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
        assert!(recon.unmapped.contains(&"schema:city.future_city_fact".to_string()));
        assert!(recon.unmapped.contains(&"schema:state.future_empire_fact".to_string()));
    }

    #[test]
    fn a_civ6_seat_rebuilds_with_its_setup_rules_and_ui_settings() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 8,
            width: 4,
            height: 4,
            chunk: 1,
            plots: vec![plot(1, 1, "TERRAIN_GRASS")],
        }]);
        let mut state = StateSnapshot {
            turn: 8,
            ..StateSnapshot::default()
        };
        state.seat.speed = "GAMESPEED_ONLINE".to_string();
        state.seat.difficulty = "DIFFICULTY_SETTLER".to_string();
        state.seat.map = "Continents.lua".to_string();

        let recon = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);

        assert_eq!(civvis_game_speed("GAMESPEED_ONLINE"), Some(GameSpeed::Online));
        assert_eq!(
            civvis_difficulty("DIFFICULTY_SETTLER"),
            Some("settler".to_string())
        );
        assert_eq!(
            civvis_map_script("Continents.lua"),
            Some(MapScript::Continents)
        );
        assert_eq!(recon.game.game_speed, GameSpeed::Online);
        assert_eq!(recon.game.speed, "online");
        assert_eq!(recon.game.difficulty, "settler");
        assert_eq!(recon.game.map_script, MapScript::Continents);
    }

    #[test]
    fn rival_identity_follows_the_compacted_mirror_seat() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 20,
            width: 8,
            height: 8,
            chunk: 1,
            plots: vec![plot(3, 3, "TERRAIN_GRASS")],
        }]);
        let state = StateSnapshot {
            turn: 20,
            rivals: vec![StateRival {
                player: 3,
                civ: "CIVILIZATION_SCYTHIA".to_string(),
                leader: "LEADER_TOMYRIS".to_string(),
                ..StateRival::default()
            }],
            ..StateSnapshot::default()
        };

        let recon = rebuild_from_state(&snapshot, &state, 6, 1, 250, 0);
        assert_eq!(
            recon.game.players[1].civ, "Scythia",
            "the first exported rival owns compacted CIVVIS seat 1"
        );
        assert_eq!(
            recon.game.observed_leader_types.get(&1).map(String::as_str),
            Some("LEADER_TOMYRIS")
        );
        let observed = crate::obs::observation_spectator(&recon.game, 0);
        assert_eq!(observed["players"][1]["leader"], serde_json::json!("Tomyris"));
        assert_eq!(
            observed["players"][1]["leader_type"],
            serde_json::json!("LEADER_TOMYRIS")
        );
        assert_ne!(
            recon.game.players[3].civ, "Scythia",
            "Firaxis player id 3 is translation metadata, not the CIVVIS entity owner"
        );
    }

    #[test]
    fn host_routes_land_on_the_engines_ladder() {
        assert_eq!(route_level(None, false), 0);
        assert_eq!(route_level(Some(""), false), 0);
        assert_eq!(route_level(Some("ROUTE_ANCIENT_ROAD"), false), 1);
        assert_eq!(route_level(Some("ROUTE_MEDIEVAL_ROAD"), false), 2);
        assert_eq!(route_level(Some("ROUTE_INDUSTRIAL_ROAD"), false), 3);
        assert_eq!(route_level(Some("ROUTE_MODERN_ROAD"), false), 4);
        assert_eq!(route_level(Some("ROUTE_RAILROAD"), false), 5);
        // A route the ladder does not name is still a road; a pillaged one pays nothing.
        assert_eq!(route_level(Some("ROUTE_SOMETHING_NEW"), false), 1);
        assert_eq!(route_level(Some("ROUTE_MEDIEVAL_ROAD"), true), 0);
    }

    /// Roads were never exported and the board wrote `road = 0` everywhere;
    /// a plot that names its route now carries it, and an older export
    /// without `rt` still reads roadless.
    #[test]
    fn exported_routes_reach_the_board() {
        let mut roaded = plot(3, 3, "TERRAIN_GRASS");
        roaded.rt = Some("ROUTE_MEDIEVAL_ROAD".to_string());
        let mut pillaged = plot(4, 3, "TERRAIN_GRASS");
        pillaged.rt = Some("ROUTE_ANCIENT_ROAD".to_string());
        pillaged.rp = true;
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 8,
            width: 8,
            height: 8,
            chunk: 1,
            plots: vec![roaded, pillaged, plot(5, 3, "TERRAIN_GRASS")],
        }]);
        let state = StateSnapshot {
            turn: 8,
            ..StateSnapshot::default()
        };
        let mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        let at = |x, y| mirror.game.map.tiles[&crate::hex::offset_to_axial(x, y)].road;
        assert_eq!(at(3, 3), 2, "a medieval road on the engine's ladder");
        assert_eq!(at(4, 3), 0, "a pillaged road pays no movement");
        assert_eq!(at(5, 3), 0, "no route, no road");
    }

    /// The export's `moves` is trusted only when the seat says the mod reads
    /// it at the start of the turn and keeps the host from spending it first;
    /// otherwise every unit keeps its full allowance exactly as before.
    #[test]
    fn exported_movement_is_trusted_only_with_the_seat_capability() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 8,
            width: 8,
            height: 8,
            chunk: 1,
            plots: (0..8)
                .flat_map(|x| (0..8).map(move |y| plot(x, y, "TERRAIN_GRASS")))
                .collect(),
        }]);
        let units = || {
            vec![
                StateUnit {
                    id: 11,
                    kind: "UNIT_WARRIOR".to_string(),
                    x: 3,
                    y: 3,
                    moves: 0.0,
                    ..StateUnit::default()
                },
                StateUnit {
                    id: 12,
                    kind: "UNIT_WARRIOR".to_string(),
                    x: 4,
                    y: 3,
                    moves: 2.0,
                    ..StateUnit::default()
                },
                StateUnit {
                    id: 13,
                    kind: "UNIT_WARRIOR".to_string(),
                    x: 5,
                    y: 3,
                    moves: -1.0,
                    ..StateUnit::default()
                },
            ]
        };
        let plain = StateSnapshot {
            turn: 8,
            units: units(),
            ..StateSnapshot::default()
        };
        let mirror = LiveMirror::new(&snapshot, &plain, 4, 1, 250, 0);
        for civ6 in [11, 12, 13] {
            let uid = mirror.uid_of[&civ6];
            assert_eq!(
                mirror.game.units[&uid].moves_left,
                mirror.game.unit_max_moves(uid),
                "without the capability unit {civ6} keeps its full allowance"
            );
        }
        assert_eq!(mirror.units_short_of_movement(), 0);

        let trusted = StateSnapshot {
            turn: 8,
            units: units(),
            seat: Seat {
                moves_at_turn_start: true,
                ..Seat::default()
            },
            ..StateSnapshot::default()
        };
        let mut mirror = LiveMirror::new(&snapshot, &trusted, 4, 1, 250, 0);
        let spent = mirror.uid_of[&11];
        let fresh = mirror.uid_of[&12];
        let unreported = mirror.uid_of[&13];
        assert_eq!(
            mirror.game.units[&spent].moves_left, 0.0,
            "the host already walked it"
        );
        assert_eq!(
            mirror.game.units[&fresh].moves_left,
            mirror.game.unit_max_moves(fresh)
        );
        assert_eq!(
            mirror.game.units[&unreported].moves_left,
            mirror.game.unit_max_moves(unreported),
            "a negative export is 'not reported', not zero"
        );
        assert_eq!(mirror.units_short_of_movement(), 1);

        // The persistent path (`sync`) reads the same truth each turn.
        let mut next = trusted;
        next.turn = 9;
        next.units[0].moves = 2.0;
        next.units[1].moves = 1.0;
        mirror.sync(&snapshot, &next, 0);
        assert_eq!(
            mirror.game.units[&spent].moves_left,
            mirror.game.unit_max_moves(spent)
        );
        assert_eq!(mirror.game.units[&fresh].moves_left, 1.0);
    }

    /// On a mid-turn combat frame the host says how many strikes a unit has
    /// left; a unit that already struck must not be planned to strike again.
    /// Trusted under the same seat capability as movement, on both paths.
    #[test]
    fn attacks_remaining_reach_the_board_with_the_seat_capability() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 8,
            width: 8,
            height: 8,
            chunk: 1,
            plots: (0..8)
                .flat_map(|x| (0..8).map(move |y| plot(x, y, "TERRAIN_GRASS")))
                .collect(),
        }]);
        let units = |attacks: Option<i32>| {
            vec![StateUnit {
                id: 21,
                kind: "UNIT_ARCHER".to_string(),
                x: 3,
                y: 3,
                moves: 1.0,
                attacks_remaining: attacks,
                ..StateUnit::default()
            }]
        };
        let plain = StateSnapshot {
            turn: 8,
            frame: 1,
            units: units(Some(0)),
            ..StateSnapshot::default()
        };
        let mirror = LiveMirror::new(&snapshot, &plain, 4, 1, 250, 0);
        let uid = mirror.uid_of[&21];
        assert_eq!(
            mirror.game.units[&uid].attacks_left, 1,
            "no capability: the fresh-turn allowance"
        );

        let trusted = StateSnapshot {
            turn: 8,
            frame: 1,
            units: units(Some(0)),
            seat: Seat {
                moves_at_turn_start: true,
                ..Seat::default()
            },
            ..StateSnapshot::default()
        };
        let mut mirror = LiveMirror::new(&snapshot, &trusted, 4, 1, 250, 0);
        let uid = mirror.uid_of[&21];
        assert_eq!(
            mirror.game.units[&uid].attacks_left, 0,
            "the host says it already struck"
        );
        let mut next = trusted;
        next.turn = 9;
        next.frame = 0;
        next.units = units(Some(1));
        mirror.sync(&snapshot, &next, 0);
        assert_eq!(mirror.game.units[&uid].attacks_left, 1);
        next.units = units(None);
        mirror.sync(&snapshot, &next, 0);
        assert_eq!(
            mirror.game.units[&uid].attacks_left, 1,
            "an older export means the allowance"
        );
    }

    #[test]
    fn firaxis_babylon_pack_suffix_is_not_a_second_civilization() {
        assert_eq!(civvis_civ_name("CIVILIZATION_BABYLON_STK"), Some("Babylon"));
        assert_eq!(civvis_civ_name("CIVILIZATION_OTTOMAN"), Some("Ottomans"));
    }

    #[test]
    fn active_research_and_civic_progress_follow_the_live_export() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 8,
            width: 8,
            height: 8,
            chunk: 1,
            plots: vec![plot(3, 3, "TERRAIN_GRASS")],
        }]);
        let mut state = StateSnapshot {
            turn: 8,
            research: Some("TECH_MINING".to_string()),
            research_progress: 7.5,
            civic: Some("CIVIC_CODE_OF_LAWS".to_string()),
            civic_progress: 3.0,
            ..StateSnapshot::default()
        };

        let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        assert_eq!(mirror.game.players[0].research.as_deref(), Some("mining"));
        assert_eq!(mirror.game.players[0].research_progress, 7.5);
        assert_eq!(mirror.game.players[0].civic.as_deref(), Some("code_of_laws"));
        assert_eq!(mirror.game.players[0].civic_progress, 3.0);

        state.turn = 9;
        state.research = Some("TECH_ANIMAL_HUSBANDRY".to_string());
        state.research_progress = 11.0;
        state.civic = Some("CIVIC_FOREIGN_TRADE".to_string());
        state.civic_progress = 5.0;
        mirror.sync(&snapshot, &state, 0);

        assert_eq!(
            mirror.game.players[0].research.as_deref(),
            Some("animal_husbandry")
        );
        assert_eq!(mirror.game.players[0].research_progress, 11.0);
        assert_eq!(mirror.game.players[0].civic.as_deref(), Some("foreign_trade"));
        assert_eq!(mirror.game.players[0].civic_progress, 5.0);
    }

    #[test]
    fn public_rival_military_score_survives_rebuild_and_sync() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 40,
            width: 8,
            height: 8,
            chunk: 1,
            plots: vec![plot(3, 3, "TERRAIN_GRASS")],
        }]);
        let mut state = StateSnapshot {
            turn: 40,
            rivals: vec![StateRival {
                player: 3,
                military: 670.0,
                score: 926,
                at_war: true,
                ..StateRival::default()
            }],
            ..StateSnapshot::default()
        };

        let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        assert_eq!(
            mirror.game.units.values().filter(|unit| unit.owner == 1).count(),
            0,
            "the rival army is under fog, so no tactical units may be invented"
        );
        assert_eq!(
            mirror.game.military_power(1),
            670.0,
            "the aggregate score is public information and must still drive strategy"
        );
        assert_eq!(mirror.game.score(1), 926);

        let saved = serde_json::to_string(&mirror.game).expect("save mirrored game");
        let loaded: crate::game::Game =
            serde_json::from_str(&saved).expect("load mirrored game");
        assert_eq!(loaded.military_power(1), 670.0);
        assert_eq!(loaded.score(1), 926);

        state.turn = 41;
        state.rivals[0].military = 342.0;
        state.rivals[0].score = 542;
        mirror.sync(&snapshot, &state, 0);
        assert_eq!(
            mirror.game.military_power(1),
            342.0,
            "persistent sync must refresh the score rather than freezing the rebuild value"
        );
        assert_eq!(mirror.game.score(1), 542);
    }

    /// ★★★★ THE OTHER CIVILIZATIONS' ECONOMIES ARE THE HOST'S FIGURES, NOT A GUESS.
    ///
    /// The standings' rival Science and Culture were CIVVIS's own derivation from
    /// whichever rival cities happened to be visible — usually none. The host reads
    /// them for every player (as its World Rankings screen does), and now so does
    /// the mirror: per-turn Science/Culture/Faith as the seat's own kind of
    /// delta, treasury and banked Faith directly, refreshed by every sync.
    #[test]
    fn rival_economy_reaches_the_rival_seat_and_survives_sync() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 40,
            width: 8,
            height: 8,
            chunk: 1,
            plots: vec![plot(3, 3, "TERRAIN_GRASS")],
        }]);
        let mut state = StateSnapshot {
            turn: 40,
            rivals: vec![StateRival {
                player: 3,
                military: 100.0,
                score: 200,
                science: 41.5,
                culture: 23.25,
                gold: 512.0,
                gold_per_turn: -3.0,
                faith: 88.0,
                faith_per_turn: f64::NAN,
                ..StateRival::default()
            }],
            ..StateSnapshot::default()
        };
        let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        let seat_yields = |game: &crate::game::Game| {
            let mut total = crate::rules::Yields::default();
            for cid in game.player_city_ids(1) {
                total.add(game.city_yields(cid));
            }
            if let Some(adjustment) = game.observed_yield_adjustments.get(&1) {
                total.add(*adjustment);
            }
            total
        };
        let yields = seat_yields(&mirror.game);
        assert!((yields.science - 41.5).abs() < 1e-9, "{yields:?}");
        assert!((yields.culture - 23.25).abs() < 1e-9);
        assert_eq!(mirror.game.players[1].gold, 512.0);
        assert_eq!(mirror.game.players[1].gold_per_turn, -3.0);
        assert_eq!(mirror.game.players[1].faith, 88.0);

        state.turn = 41;
        state.rivals[0].science = 44.0;
        state.rivals[0].gold = 530.0;
        mirror.sync(&snapshot, &state, 0);
        assert!((seat_yields(&mirror.game).science - 44.0).abs() < 1e-9);
        assert_eq!(mirror.game.players[1].gold, 530.0);

        // An older export (NaN) or a refused read (-1) leaves the model's own
        // derivation alone rather than zeroing the seat. The struct literal's
        // derived Default is zero for a scalar, so make the absent Faith rate
        // explicit too.
        state.turn = 42;
        state.rivals[0].science = -1.0;
        state.rivals[0].culture = f64::NAN;
        state.rivals[0].faith_per_turn = f64::NAN;
        state.rivals[0].gold = -1.0;
        mirror.sync(&snapshot, &state, 0);
        assert!(mirror.game.observed_yield_adjustments.get(&1).is_none());
        assert_eq!(mirror.game.players[1].gold, 530.0);
    }

    /// The player HUD must use the host's public empire totals for every
    /// civilization, even when fog deliberately leaves the rival with no
    /// reconstructed city or unit records.
    #[test]
    fn public_empire_hud_totals_reach_every_civilization_and_refresh() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 40,
            width: 8,
            height: 8,
            chunk: 1,
            plots: vec![plot(3, 3, "TERRAIN_GRASS")],
        }]);
        let mut state = StateSnapshot {
            turn: 40,
            science: 12.0,
            culture: 9.0,
            faith_per_turn: Some(7.0),
            gold: 75,
            gold_per_turn: Some(-4.0),
            faith: 44,
            score: 120,
            military: 80.0,
            government: Some("GOVERNMENT_MONARCHY".to_string()),
            dark_age: Some(false),
            golden_age: Some(true),
            heroic_golden_age: Some(false),
            public_stats: StatePublicEmpireStats {
                city_count: Some(4),
                population: Some(31),
                food: Some(48.0),
                production: Some(29.0),
                wonder_count: Some(2),
                suzerain_count: Some(1),
                nuclear_devices: Some(3),
                thermonuclear_devices: Some(2),
            },
            rivals: vec![StateRival {
                player: 3,
                military: 670.0,
                score: 926,
                techs: 53.0,
                civics: 44.0,
                science: 41.5,
                culture: 23.0,
                tourism: 61.0,
                gold: 512.0,
                gold_per_turn: -3.0,
                faith: 88.0,
                faith_per_turn: 19.0,
                government: Some("GOVERNMENT_FASCISM".to_string()),
                dark_age: Some(false),
                golden_age: Some(false),
                heroic_golden_age: Some(true),
                public_stats: StatePublicEmpireStats {
                    city_count: Some(7),
                    population: Some(49),
                    food: Some(76.0),
                    production: Some(43.0),
                    wonder_count: Some(5),
                    suzerain_count: Some(2),
                    nuclear_devices: Some(4),
                    thermonuclear_devices: Some(1),
                },
                ..StateRival::default()
            }],
            ..StateSnapshot::default()
        };
        let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        assert!(
            mirror.game.player_city_ids(1).is_empty(),
            "the aggregate must not fabricate fogged rival cities"
        );

        let observed = crate::obs::observation_spectator(&mirror.game, 0);
        let mine = &observed["players"][0];
        let rival = &observed["players"][1];
        assert_eq!(mine["cities"], serde_json::json!(4));
        assert_eq!(mine["population"], serde_json::json!(31));
        assert_eq!(mine["yields"]["food"], serde_json::json!(48.0));
        assert_eq!(mine["yields"]["production"], serde_json::json!(29.0));
        assert_eq!(mine["yields"]["science"], serde_json::json!(12.0));
        assert_eq!(mine["yields"]["culture"], serde_json::json!(9.0));
        assert_eq!(mine["yields"]["faith"], serde_json::json!(7.0));
        assert_eq!(mine["gold"], serde_json::json!(75.0));
        assert_eq!(mine["gold_per_turn"], serde_json::json!(-4.0));
        assert_eq!(mine["faith"], serde_json::json!(44.0));
        assert_eq!(mine["military"], serde_json::json!(80));
        assert_eq!(mine["score"], serde_json::json!(120));
        assert_eq!(mine["government"], serde_json::json!("monarchy"));
        assert_eq!(mine["age"], serde_json::json!("golden"));
        assert_eq!(mine["wonder_count"], serde_json::json!(2));
        assert_eq!(mine["suzerain_count"], serde_json::json!(1));
        assert_eq!(mine["nuclear_devices"], serde_json::json!(3));
        assert_eq!(mine["thermonuclear_devices"], serde_json::json!(2));

        assert_eq!(rival["cities"], serde_json::json!(7));
        assert_eq!(rival["population"], serde_json::json!(49));
        assert_eq!(rival["yields"]["food"], serde_json::json!(76.0));
        assert_eq!(rival["yields"]["production"], serde_json::json!(43.0));
        assert_eq!(rival["yields"]["science"], serde_json::json!(41.5));
        assert_eq!(rival["yields"]["culture"], serde_json::json!(23.0));
        assert_eq!(rival["yields"]["faith"], serde_json::json!(19.0));
        assert_eq!(rival["gold"], serde_json::json!(512.0));
        assert_eq!(rival["gold_per_turn"], serde_json::json!(-3.0));
        assert_eq!(rival["faith"], serde_json::json!(88.0));
        assert_eq!(rival["military"], serde_json::json!(670));
        assert_eq!(rival["government"], serde_json::json!("fascism"));
        assert_eq!(rival["age"], serde_json::json!("heroic"));
        assert_eq!(rival["nuclear_devices"], serde_json::json!(4));
        assert_eq!(rival["thermonuclear_devices"], serde_json::json!(1));
        assert_eq!(rival["wonder_count"], serde_json::json!(5));
        assert_eq!(rival["suzerain_count"], serde_json::json!(2));
        assert_eq!(rival["score"], serde_json::json!(926));
        assert_eq!(rival["tourism_per_turn"], serde_json::json!(61.0));
        assert_eq!(rival["victories"]["science"]["techs"], serde_json::json!(53));
        assert_eq!(rival["victories"]["culture"]["civics"], serde_json::json!(44));

        let saved = serde_json::to_string(&mirror.game).expect("save mirrored game");
        let loaded: crate::game::Game = serde_json::from_str(&saved).expect("load mirrored game");
        assert_eq!(
            crate::obs::observation_spectator(&loaded, 0)["players"][1]["cities"],
            serde_json::json!(7),
            "the public totals survive a saved spectator frame"
        );
        assert_eq!(
            crate::obs::observation_spectator(&loaded, 0)["players"][1]["government"],
            serde_json::json!("fascism"),
            "a fogged rival's public government survives a saved spectator frame"
        );
        assert_eq!(
            crate::obs::observation_spectator(&loaded, 0)["players"][1]["age"],
            serde_json::json!("heroic"),
            "a fogged rival's public age survives a saved spectator frame"
        );

        state.turn = 41;
        state.public_stats.city_count = Some(5);
        state.public_stats.nuclear_devices = Some(0);
        state.rivals[0].public_stats.population = Some(55);
        state.rivals[0].public_stats.food = Some(80.0);
        state.rivals[0].public_stats.nuclear_devices = Some(0);
        state.rivals[0].faith_per_turn = 23.0;
        state.rivals[0].techs = 54.0;
        state.rivals[0].government = Some("GOVERNMENT_DEMOCRACY".to_string());
        state.rivals[0].heroic_golden_age = Some(false);
        state.rivals[0].golden_age = Some(false);
        state.rivals[0].dark_age = Some(true);
        mirror.sync(&snapshot, &state, 0);
        let refreshed = crate::obs::observation_spectator(&mirror.game, 0);
        assert_eq!(refreshed["players"][0]["cities"], serde_json::json!(5));
        assert_eq!(refreshed["players"][0]["nuclear_devices"], serde_json::json!(0));
        assert_eq!(refreshed["players"][1]["population"], serde_json::json!(55));
        assert_eq!(refreshed["players"][1]["yields"]["food"], serde_json::json!(80.0));
        assert_eq!(refreshed["players"][1]["yields"]["faith"], serde_json::json!(23.0));
        assert_eq!(refreshed["players"][1]["nuclear_devices"], serde_json::json!(0));
        assert_eq!(refreshed["players"][1]["government"], serde_json::json!("democracy"));
        assert_eq!(refreshed["players"][1]["age"], serde_json::json!("dark"));
        assert_eq!(
            refreshed["players"][1]["victories"]["science"]["techs"],
            serde_json::json!(54)
        );

        // All three explicit false flags mean Normal, while a missing field is
        // an older control mod and must not erase the last host observation.
        state.turn = 42;
        state.rivals[0].heroic_golden_age = Some(false);
        state.rivals[0].golden_age = Some(false);
        state.rivals[0].dark_age = Some(false);
        mirror.sync(&snapshot, &state, 0);
        assert_eq!(
            crate::obs::observation_spectator(&mirror.game, 0)["players"][1]["age"],
            serde_json::json!("normal")
        );
        state.turn = 43;
        state.rivals[0].government = None;
        state.rivals[0].heroic_golden_age = None;
        state.rivals[0].golden_age = None;
        state.rivals[0].dark_age = None;
        mirror.sync(&snapshot, &state, 0);
        let old_export = crate::obs::observation_spectator(&mirror.game, 0);
        assert_eq!(old_export["players"][1]["government"], serde_json::json!("democracy"));
        assert_eq!(old_export["players"][1]["age"], serde_json::json!("normal"));
    }

    #[test]
    fn public_empire_hud_fields_are_recognized_on_the_live_wire() {
        let state = state_from_json(
            r#"{
                "kind":"state", "turn":40,
                "public_stats":{"city_count":4,"population":31,"food":48.0,
                  "production":29.0,"wonder_count":2,"suzerain_count":1,
                  "nuclear_devices":3,"thermonuclear_devices":2},
                "rivals":[{"player":3,"government":"GOVERNMENT_FASCISM",
                  "dark_age":false,"golden_age":false,"heroic_golden_age":true,
                  "public_stats":{"city_count":7,"population":49,
                  "food":76.0,"production":43.0,"wonder_count":5,"suzerain_count":2,
                  "nuclear_devices":4,"thermonuclear_devices":1}}]
            }"#,
        )
        .expect("the live public standings wire parses");
        assert_eq!(state.public_stats.city_count, Some(4));
        assert_eq!(state.public_stats.thermonuclear_devices, Some(2));
        assert_eq!(state.rivals[0].public_stats.population, Some(49));
        assert_eq!(state.rivals[0].public_stats.wonder_count, Some(5));
        assert_eq!(state.rivals[0].government.as_deref(), Some("GOVERNMENT_FASCISM"));
        assert_eq!(state.rivals[0].dark_age, Some(false));
        assert_eq!(state.rivals[0].golden_age, Some(false));
        assert_eq!(state.rivals[0].heroic_golden_age, Some(true));
        assert!(
            state.schema_gaps.is_empty(),
            "recognized public standings must not become unmapped diagnostics: {:?}",
            state.schema_gaps
        );
    }

    #[test]
    fn live_diplomatic_totals_reach_rebuild_and_sync_without_legacy_erasure() {
        // This is the wire shape currently produced by CivvisControlAgent.lua.
        // Civilization VI already knows all three values, but before this bridge
        // the reconstructed board silently treated each of them as zero.
        let raw = r#"{
            "kind":"state", "ctx":"Gameplay", "run":"contract", "turn":40,
            "dvp":3, "favor":92.5,
            "used_governments":["GOVERNMENT_CHIEFDOM", "GOVERNMENT_OLIGARCHY"],
            "rivals":[{"player":3, "dvp":18}]
        }"#;
        let mut state = state_from_json(raw).expect("the live diplomatic wire parses");
        assert_eq!(state.dvp, Some(3));
        assert_eq!(state.favor, Some(92.5));
        assert_eq!(state.rivals[0].dvp, Some(18));
        assert_eq!(
            state.used_governments,
            vec!["GOVERNMENT_CHIEFDOM", "GOVERNMENT_OLIGARCHY"],
            "government history is a recognized state field, not an unmapped diagnostic"
        );
        assert!(
            state.schema_gaps.is_empty(),
            "the three diplomatic facts and used_governments must be schema-recognized: {:?}",
            state.schema_gaps
        );

        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 40,
            width: 8,
            height: 8,
            chunk: 1,
            plots: vec![plot(3, 3, "TERRAIN_GRASS")],
        }]);
        let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
        assert_eq!(rebuilt.game.players[0].dvp, 3);
        assert_eq!(rebuilt.game.players[0].diplomatic_favor, 92.5);
        assert_eq!(rebuilt.game.players[1].dvp, 18);

        let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        state.turn = 41;
        state.dvp = Some(4);
        state.favor = Some(11.0);
        state.rivals[0].dvp = Some(19);
        mirror.sync(&snapshot, &state, 0);
        assert_eq!(mirror.game.players[0].dvp, 4);
        assert_eq!(mirror.game.players[0].diplomatic_favor, 11.0);
        assert_eq!(mirror.game.players[1].dvp, 19);

        // An already-loaded older control mod omits a new field. Omission means
        // unknown, not an authoritative zero that should erase live knowledge.
        state.turn = 42;
        state.dvp = None;
        state.favor = None;
        state.rivals[0].dvp = None;
        mirror.sync(&snapshot, &state, 0);
        assert_eq!(mirror.game.players[0].dvp, 4);
        assert_eq!(mirror.game.players[0].diplomatic_favor, 11.0);
        assert_eq!(mirror.game.players[1].dvp, 19);
    }

    /// 🔴🔴🔴 The congress standing seats the majors this seat never met.
    ///
    /// Replays `civvis-20260818T103630Z`, which lost a diplomatic victory at
    /// turn 222 while leading on score by 213. Six majors, and the seat had met
    /// exactly one of them: the per-turn rival export therefore topped out at
    /// LAUTARO's 14 points (70% of the 20 needed), comfortably under the denial
    /// alarm, while the congress table the seat votes from showed player 4
    /// holding 22. `urgent_victory_threat` never fired once in 222 turns.
    #[test]
    fn congress_standing_seats_the_majors_this_seat_never_met() {
        let raw = r#"{
            "kind":"state", "ctx":"Gameplay", "run":"contract", "turn":221,
            "dvp":2, "favor":847.0,
            "congress_dvp":{"turn":221, "points":[
                {"player":0, "points":2}, {"player":1, "points":10},
                {"player":3, "points":14}, {"player":4, "points":22},
                {"player":5, "points":16}]},
            "rivals":[{"player":3, "dvp":14}]
        }"#;
        let mut state = state_from_json(raw).expect("the congress standing wire parses");
        assert!(
            state.schema_gaps.is_empty(),
            "congress_dvp must be schema-recognized: {:?}",
            state.schema_gaps
        );
        // The seat arrives as its own event rather than inside `state`.
        state.seat.local_player = 0;
        state.seat.players = 6;
        let congress = state.congress_dvp.as_ref().expect("the table parses");
        assert_eq!(congress.turn, Some(221));
        assert_eq!(congress.points.len(), 5);

        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 221,
            width: 8,
            height: 8,
            chunk: 1,
            plots: vec![plot(3, 3, "TERRAIN_GRASS")],
        }]);
        let rebuilt = rebuild_from_state(&snapshot, &state, 6, 1, 250, 0);
        // Seat 0 is ours and seat 1 is the one met rival, whose own per-turn
        // export is the fresher number. The three we never met take the free
        // seats in ascending host order: 1, 4, 5.
        assert_eq!(rebuilt.game.players[0].dvp, 2);
        assert_eq!(rebuilt.game.players[1].dvp, 14);
        assert_eq!(rebuilt.game.players[2].dvp, 10);
        assert_eq!(rebuilt.game.players[3].dvp, 22);
        assert_eq!(rebuilt.game.players[4].dvp, 16);
        assert_eq!(
            rebuilt
                .game
                .players
                .iter()
                .map(|player| player.dvp)
                .max()
                .unwrap_or(0),
            22,
            "the empire actually about to win must be visible somewhere on the board"
        );

        // The point of the plumbing: the denial alarm can now see the empire
        // that is one resolution from winning. Diplomatic progress is
        // `dvp * 5`, so 22 points reads as a finished race and 14 reads 70 --
        // under every bar in `urgent_victory_threat`, which is why the shipped
        // seat sat on 847 unspent Favor while the game ended.
        let planner = crate::ai::AdvancedAi::default();
        assert_eq!(planner.rival_pressure(&rebuilt.game, 3).1, 100);
        assert!(
            planner.denial_is_urgent(&rebuilt.game, 3),
            "a rival holding 22 of the 20 points needed is a terminal clock"
        );
        let blind = rebuild_from_state(
            &snapshot,
            &StateSnapshot {
                congress_dvp: None,
                ..state.clone()
            },
            6,
            1,
            250,
            0,
        );
        assert!(
            !planner.denial_is_urgent(&blind.game, 3),
            "and without the congress table it is exactly the silence that lost the game"
        );

        let mut mirror = LiveMirror::new(&snapshot, &state, 6, 1, 250, 0);
        assert_eq!(mirror.game.players[3].dvp, 22);
        // The met rival's live export stays authoritative even when it falls,
        // because `WC_RES_DIPLOVICTORY` option B takes two points away.
        state.turn = 222;
        state.rivals[0].dvp = Some(12);
        mirror.sync(&snapshot, &state, 0);
        assert_eq!(
            mirror.game.players[1].dvp, 12,
            "a met rival's per-turn read outranks a congress table refreshed once a session"
        );
        assert_eq!(mirror.game.players[3].dvp, 22);

        // An older control mod omits the table entirely; that must not erase
        // what a persistent mirror already seated.
        state.turn = 223;
        state.congress_dvp = None;
        mirror.sync(&snapshot, &state, 0);
        assert_eq!(mirror.game.players[3].dvp, 22);
    }

    /// A met rival whose `dvp` the mod could not read still gets the congress
    /// number rather than a silent zero.
    #[test]
    fn congress_standing_backfills_a_met_rival_with_no_live_reading() {
        let raw = r#"{
            "kind":"state", "ctx":"Gameplay", "run":"contract", "turn":180,
            "congress_dvp":{"turn":180, "points":[
                {"player":0, "points":5}, {"player":2, "points":17}]},
            "rivals":[{"player":2}]
        }"#;
        let mut state = state_from_json(raw).expect("the congress standing wire parses");
        state.seat.local_player = 0;
        state.seat.players = 4;
        assert_eq!(state.rivals[0].dvp, None);
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 180,
            width: 8,
            height: 8,
            chunk: 1,
            plots: vec![plot(3, 3, "TERRAIN_GRASS")],
        }]);
        let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
        assert_eq!(rebuilt.game.players[1].dvp, 17);
    }

    /// Rival victory progress crosses the bridge. Five of the twelve runs the
    /// seat was leading on 2026-08-16/17 ended at t229-245 on a rival's
    /// culture, technology or diplomatic victory: rival space programs and
    /// tourist counts never crossed, so the victory tracker read zero for
    /// every rival on exactly the lanes that end games early.
    #[test]
    fn rival_victory_progress_reaches_rebuild_and_sync() {
        let raw = r#"{
            "kind":"state", "ctx":"Gameplay", "run":"contract", "turn":180,
            "rivals":[{"player":3,
                "science_projects":["PROJECT_LAUNCH_EARTH_SATELLITE",
                                     "PROJECT_LAUNCH_MOON_LANDING"],
                "foreign_tourists":41, "domestic_tourists":66}]
        }"#;
        let mut state = state_from_json(raw).expect("the rival progress wire parses");
        assert_eq!(state.rivals[0].foreign_tourists, 41.0);
        assert_eq!(state.rivals[0].domestic_tourists, 66.0);
        assert!(
            state.schema_gaps.is_empty(),
            "rival victory progress must be schema-recognized: {:?}",
            state.schema_gaps
        );

        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 180,
            width: 8,
            height: 8,
            chunk: 1,
            plots: vec![plot(3, 3, "TERRAIN_GRASS")],
        }]);
        let expected = BTreeSet::from([
            "launch_earth_satellite".to_string(),
            "launch_moon_landing".to_string(),
        ]);
        let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
        assert_eq!(rebuilt.game.players[1].science_projects, expected);
        let observed = rebuilt
            .game
            .observed_public_empire_stats
            .get(&1)
            .expect("a rival with progress has observed stats");
        assert_eq!(observed.foreign_tourists, Some(41));
        assert_eq!(observed.domestic_tourists, Some(66));

        let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        assert_eq!(mirror.game.players[1].science_projects, expected);
        state.turn = 181;
        state.rivals[0].science_projects = Some(vec![
            "PROJECT_LAUNCH_EARTH_SATELLITE".to_string(),
            "PROJECT_LAUNCH_MOON_LANDING".to_string(),
            "PROJECT_LAUNCH_MARS_BASE".to_string(),
        ]);
        state.rivals[0].foreign_tourists = 44.0;
        mirror.sync(&snapshot, &state, 0);
        assert!(
            mirror.game.players[1]
                .science_projects
                .contains("launch_mars_colony"),
            "a Gathering Storm Mars base must translate exactly as the local seat's does"
        );
        let observed = mirror.game.observed_public_empire_stats.get(&1).unwrap();
        assert_eq!(observed.foreign_tourists, Some(44));

        // An already-loaded older control mod omits the fields, and a refused
        // host read sends -1. The observed table is honest per snapshot —
        // unknown reads None — while the player's completed-milestone record
        // is history and must survive the silence.
        state.turn = 182;
        state.rivals[0].science_projects = None;
        state.rivals[0].foreign_tourists = f64::NAN;
        state.rivals[0].domestic_tourists = -1.0;
        mirror.sync(&snapshot, &state, 0);
        assert!(mirror.game.players[1]
            .science_projects
            .contains("launch_moon_landing"));
        let observed = mirror.game.observed_public_empire_stats.get(&1).unwrap();
        assert_eq!(observed.foreign_tourists, None);
        assert_eq!(observed.domestic_tourists, None);
    }

    /// The tourist counters the mirror records must outrank the engine's own
    /// reconstruction — a live board has no culture history to derive them
    /// from, so without the preference every rival's culture-victory progress
    /// reads zero (the lane that stole four led runs on 2026-08-16/17).
    #[test]
    fn observed_tourist_counters_outrank_the_reconstruction() {
        let mut game = crate::game::Game::new(2, 8, 8, 42, 250, 0);
        let engine_foreign = game.foreign_tourists(1);
        let engine_domestic = game.domestic_tourists(1);
        {
            let observed = game.observed_public_empire_stats.entry(1).or_default();
            observed.foreign_tourists = Some(41);
            observed.domestic_tourists = Some(66);
        }
        assert_eq!(game.foreign_tourists(1), 41);
        assert_eq!(game.domestic_tourists(1), 66);
        // An entry with no counters falls back to the engine's arithmetic.
        {
            let observed = game.observed_public_empire_stats.entry(1).or_default();
            observed.foreign_tourists = None;
            observed.domestic_tourists = None;
        }
        assert_eq!(game.foreign_tourists(1), engine_foreign);
        assert_eq!(game.domestic_tourists(1), engine_domestic);
    }

    /// The seat's own two counters ride the state event and land on the
    /// observed table's local entry, exactly as each rival's do.
    #[test]
    fn own_tourist_counters_reach_the_observed_table() {
        let raw = r#"{"kind":"state", "turn":120,
                      "foreign_tourists":9, "domestic_tourists":31}"#;
        let state = state_from_json(raw).expect("the own-counter wire parses");
        assert_eq!(state.foreign_tourists, 9.0);
        assert_eq!(state.domestic_tourists, 31.0);
        assert!(
            state.schema_gaps.is_empty(),
            "own tourist counters must be schema-recognized: {:?}",
            state.schema_gaps
        );
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 120,
            width: 8,
            height: 8,
            chunk: 1,
            plots: vec![plot(3, 3, "TERRAIN_GRASS")],
        }]);
        let rebuilt = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
        let observed = rebuilt
            .game
            .observed_public_empire_stats
            .get(&0)
            .expect("the local seat has an observed entry");
        assert_eq!(observed.foreign_tourists, Some(9));
        assert_eq!(observed.domestic_tourists, Some(31));
        assert_eq!(rebuilt.game.foreign_tourists(0), 9);
        assert_eq!(rebuilt.game.domestic_tourists(0), 31);
    }

    /// ★★★ `Game::spies` was empty for the whole of a live game, so the AI's
    /// entire espionage layer — twelve missions, per-lane promotion
    /// priorities, a +90 weight on the denial target — iterated an empty map
    /// and could not choose anything. And the blanket production block is why
    /// the seat never held a Spy to seat: over twelve completed live games it
    /// finished holding the Diplomatic Service civic in 12 of 12 and fielded
    /// zero Spies.
    #[test]
    fn live_spies_are_seated_and_the_block_follows_capacity() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 120,
            width: 8,
            height: 8,
            chunk: 1,
            plots: vec![plot(3, 3, "TERRAIN_GRASS"), plot(4, 4, "TERRAIN_GRASS")],
        }]);
        let mut state = StateSnapshot {
            turn: 120,
            spy_capacity: Some(2),
            ..StateSnapshot::default()
        };
        // A city, or `player_city_ids` is empty and the block is vacuous in
        // both directions — which is how the first draft of this test passed
        // its "unblocked" assertion while proving nothing.
        state.cities.push(StateCity {
            id: 1,
            name: "Roma".to_string(),
            x: 4,
            y: 4,
            pop: 4,
            ..StateCity::default()
        });
        state.units.push(StateUnit {
            id: 77,
            kind: "UNIT_SPY".to_string(),
            x: 3,
            y: 3,
            ..StateUnit::default()
        });
        let rebuilt = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
        let seated: Vec<_> = rebuilt
            .game
            .spies
            .values()
            .filter(|spy| spy.owner == 0)
            .collect();
        assert_eq!(
            seated.len(),
            1,
            "the live Spy reaches the AI's own structure"
        );
        assert_eq!(
            rebuilt.unit_ids.get(&seated[0].id),
            Some(&77),
            "the spy id is its unit id, so an order translates straight back"
        );

        // One of two: there is room, so the production block lifts.
        let spy_item = crate::game::Item::Unit {
            unit: crate::name!("spy"),
        };
        let key = crate::game::Game::production_block_key(&spy_item);
        let blocked_somewhere = rebuilt
            .game
            .blocked_production
            .values()
            .any(|keys| keys.contains(&key));
        assert!(
            !blocked_somewhere,
            "under capacity the empire must be allowed to train the Spy it can field"
        );

        // At capacity it is blocked again — the refusals the blanket block was
        // written for are exactly ordering past the limit.
        let mut full = state.clone();
        full.spy_capacity = Some(1);
        let at_cap = rebuild_from_state(&snapshot, &full, 2, 1, 250, 0);
        assert!(
            at_cap
                .game
                .blocked_production
                .values()
                .any(|keys| keys.contains(&key)),
            "at capacity the order is unplayable and must stay blocked"
        );

        // An older mod cannot report capacity: keep the old unconditional
        // block rather than loosening a bridge that cannot measure itself.
        let mut silent = state.clone();
        silent.spy_capacity = None;
        let unknown = rebuild_from_state(&snapshot, &silent, 2, 1, 250, 0);
        assert!(
            unknown
                .game
                .blocked_production
                .values()
                .any(|keys| keys.contains(&key)),
            "unknown capacity must fail closed"
        );
    }

    /// ★★★★ A FRESH LIVE SPY OWES NO PROMOTION, so the mission layer is
    /// reachable at all. Civilization VI grants a Spy its first promotion at
    /// level 2; the native rule owes one per level. Seating the host's level
    /// unshifted made every fresh live Spy permanently "promotable", and
    /// `legal_spy_actions` returns promotions as the ONLY legal actions while
    /// one is owed — so no live Spy ever received a travel or mission order
    /// (run civvis-20260818T095712Z: the same impossible promotion sent for
    /// 73 consecutive turns). And a Spy that finished its travel must seat in
    /// the RIVAL city it stands in — matching own cities only imported it
    /// with no city, which generates no missions either.
    #[test]
    fn a_fresh_live_spy_owes_no_promotion_and_a_travelled_one_seats_abroad() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 120,
            width: 8,
            height: 8,
            chunk: 1,
            plots: vec![
                plot(3, 3, "TERRAIN_GRASS"),
                plot(4, 4, "TERRAIN_GRASS"),
                plot(5, 5, "TERRAIN_GRASS"),
            ],
        }]);
        let mut state = StateSnapshot {
            turn: 120,
            spy_capacity: Some(3),
            ..StateSnapshot::default()
        };
        state.cities.push(StateCity {
            id: 1,
            name: "Roma".to_string(),
            x: 4,
            y: 4,
            pop: 4,
            ..StateCity::default()
        });
        state.rivals.push(StateRival {
            player: 1,
            cities: vec![StateCity {
                id: 9,
                name: "Aduatuca".to_string(),
                x: 5,
                y: 5,
                pop: 6,
                ..StateCity::default()
            }],
            ..StateRival::default()
        });
        // A fresh Spy at home, a travelled one standing in the rival city, and
        // a genuinely levelled one whose earned pick must survive the shift.
        for (id, x, y, level) in [(77, 4, 4, 1), (78, 5, 5, 1), (79, 4, 4, 2)] {
            state.units.push(StateUnit {
                id,
                kind: "UNIT_SPY".to_string(),
                x,
                y,
                level: Some(level),
                ..StateUnit::default()
            });
        }
        let rebuilt = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
        let uid = |civ6: i64| {
            *rebuilt
                .unit_ids
                .iter()
                .find(|(_, mapped)| **mapped == civ6)
                .map(|(uid, _)| uid)
                .expect("the spy is mirrored")
        };

        let fresh = &rebuilt.game.spies[&uid(77)];
        assert_eq!(fresh.level, 0, "host level 1 is zero promotions owed");
        assert!(
            rebuilt
                .game
                .legal_spy_actions(0, fresh.id)
                .iter()
                .all(|action| !matches!(action, crate::game::Action::PromoteSpy { .. })),
            "a fresh Spy must not be gated behind a promotion the host refuses"
        );

        let travelled = &rebuilt.game.spies[&uid(78)];
        let seat = travelled.city.expect("the travelled Spy seats in a city");
        assert_ne!(
            rebuilt.game.cities[&seat].owner, 0,
            "the city it stands in is the rival's, which is what missions aim from"
        );

        let levelled = &rebuilt.game.spies[&uid(79)];
        assert_eq!(levelled.level, 1, "host level 2 owes exactly one pick");
        assert!(
            rebuilt
                .game
                .legal_spy_actions(0, levelled.id)
                .iter()
                .any(|action| matches!(action, crate::game::Action::PromoteSpy { .. })),
            "the promotion a mission actually earned is still offered"
        );
    }

    /// The host's victory checkboxes have crossed the wire in the seat event
    /// all along and were dropped: a live board always played the all-six
    /// default, so `victory_strategy_enabled` could authorise a lane the
    /// lobby had switched off.
    #[test]
    fn the_seat_victory_checkboxes_reach_the_mirrored_game() {
        let seat: Seat = serde_json::from_str(
            r#"{"local_player":0,
                "victories":{"conquest":false,"score":true,"technology":true,
                             "culture":false,"religious":null,"diplomatic":true}}"#,
        )
        .expect("the seat victory wire parses");
        let victories = seat.victories.expect("checkboxes present");
        assert_eq!(victories.conquest, Some(false));
        assert_eq!(victories.religious, None, "a refused read stays unknown");

        let mut game = crate::game::Game::new(2, 8, 8, 42, 250, 0);
        game.victory_conditions = crate::game::VictoryConditions::default();
        let seat = Seat {
            victories: Some(victories),
            ..Seat::default()
        };
        apply_seat_victories(&mut game, &seat);
        assert!(!game.victory_conditions.domination, "conquest off crosses");
        assert!(!game.victory_conditions.culture);
        assert!(
            game.victory_conditions.science,
            "technology maps to science"
        );
        assert!(game.victory_conditions.score);
        assert!(game.victory_conditions.diplomatic);
        assert!(
            game.victory_conditions.religious,
            "an unknown checkbox keeps the default rather than switching a lane off"
        );

        // An older mod sends no `victories` at all: the default stands whole.
        let mut untouched = crate::game::Game::new(2, 8, 8, 42, 250, 0);
        apply_seat_victories(&mut untouched, &Seat::default());
        assert!(untouched.victory_conditions.domination);
    }

    #[test]
    fn initializing_host_power_cannot_erase_a_visible_starting_warrior() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 1,
            width: 8,
            height: 8,
            chunk: 1,
            plots: vec![plot(3, 3, "TERRAIN_GRASS")],
        }]);
        let state = StateSnapshot {
            turn: 1,
            military: 0.0,
            units: vec![StateUnit {
                id: 1,
                kind: "UNIT_WARRIOR".to_string(),
                x: 3,
                y: 3,
                hp: 100.0,
                ..StateUnit::default()
            }],
            ..StateSnapshot::default()
        };
        let recon = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
        assert!(
            recon.game.military_power(0) >= 20.0,
            "the public aggregate initializes at zero on turn 1, but the visible unit is real"
        );
    }

    #[test]
    fn supported_unique_improvements_and_city_religion_are_not_dropped() {
        let mut improved = plot(4, 4, "TERRAIN_PLAINS");
        improved.im = Some("IMPROVEMENT_KURGAN".to_string());
        let mut resort = plot(6, 4, "TERRAIN_GRASS");
        resort.im = Some("IMPROVEMENT_BEACH_RESORT".to_string());
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 50,
            width: 10,
            height: 10,
            chunk: 1,
            plots: vec![improved, plot(5, 4, "TERRAIN_PLAINS"), resort],
        }]);
        let state = StateSnapshot {
            turn: 50,
            cities: vec![StateCity {
                id: 10,
                name: "Faith City".to_string(),
                x: 5,
                y: 4,
                pop: 4,
                religion: Some("RELIGION_ORTHODOXY".to_string()),
                ..StateCity::default()
            }],
            ..StateSnapshot::default()
        };

        let recon = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
        let kurgan = crate::hex::offset_to_axial(4, 4);
        assert_eq!(recon.game.map.tiles[&kurgan].improvement.as_deref(), Some("kurgan"));
        let resort = crate::hex::offset_to_axial(6, 4);
        assert_eq!(
            recon.game.map.tiles[&resort].improvement.as_deref(),
            Some("seaside_resort")
        );
        let city = recon.game.cities.values().find(|city| city.owner == 0).unwrap();
        assert_eq!(recon.game.city_religion(city), Some("Orthodoxy"));
    }

    /// Each founded religion's beliefs land on its founder's seat, and a city
    /// following that religion reads exactly those follower beliefs. Rome
    /// followed a Catholicism it did not found and read 23 Faith in the
    /// model against the host's 35 for the last twenty turns of run
    /// civvis-20260816T123936Z: three Wonders under Divine Inspiration, a
    /// belief the union `taken_religion_beliefs` could not place.
    #[test]
    fn each_religions_beliefs_sit_on_its_founders_seat() {
        let mut center = plot(5, 4, "TERRAIN_PLAINS");
        center.o = 0;
        let mut wonder = plot(6, 4, "TERRAIN_GRASS");
        wonder.o = 0;
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 120,
            width: 10,
            height: 10,
            chunk: 1,
            plots: vec![center, wonder, plot(2, 2, "TERRAIN_PLAINS")],
        }]);
        let state = StateSnapshot {
            turn: 120,
            founded_religions: vec![
                "RELIGION_CATHOLICISM".to_string(),
                "RELIGION_ISLAM".to_string(),
                "RELIGION_JUDAISM".to_string(),
            ],
            taken_religion_beliefs: vec![
                "BELIEF_DIVINE_INSPIRATION".to_string(),
                "BELIEF_FEED_THE_WORLD".to_string(),
                "BELIEF_TITHE".to_string(),
                "BELIEF_WORK_ETHIC".to_string(),
            ],
            religions: vec![
                StateReligion {
                    religion: "RELIGION_CATHOLICISM".to_string(),
                    founder: 4,
                    beliefs: vec![
                        "BELIEF_DIVINE_INSPIRATION".to_string(),
                        "BELIEF_TITHE".to_string(),
                    ],
                },
                StateReligion {
                    religion: "RELIGION_ISLAM".to_string(),
                    founder: 2,
                    beliefs: vec!["BELIEF_FEED_THE_WORLD".to_string()],
                },
                // A founder this seat has never met: still counted, still
                // carrying its own beliefs, on a seat nobody else took.
                StateReligion {
                    religion: "RELIGION_JUDAISM".to_string(),
                    founder: 9,
                    beliefs: vec!["BELIEF_WORK_ETHIC".to_string()],
                },
            ],
            rivals: vec![
                StateRival {
                    player: 2,
                    civ: "CIVILIZATION_ARABIA".to_string(),
                    ..StateRival::default()
                },
                StateRival {
                    player: 4,
                    civ: "CIVILIZATION_SPAIN".to_string(),
                    ..StateRival::default()
                },
            ],
            cities: vec![StateCity {
                id: 10,
                name: "Rome".to_string(),
                x: 5,
                y: 4,
                pop: 6,
                loyalty: 100.0,
                religion: Some("RELIGION_CATHOLICISM".to_string()),
                wonders: vec![StateWonder {
                    kind: "BUILDING_STONEHENGE".to_string(),
                    x: 6,
                    y: 4,
                }],
                ..StateCity::default()
            }],
            ..StateSnapshot::default()
        };
        let recon = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
        let game = &recon.game;
        // Rivals hold seats in host order: Arabia (host 2) is seat 1, Spain
        // (host 4) is seat 2; Judaism's unmet founder takes seat 3.
        assert_eq!(game.players[2].religion.as_deref(), Some("Catholicism"));
        assert_eq!(
            game.players[2].religion_beliefs,
            vec!["divine_inspiration".to_string(), "tithe".to_string()]
        );
        assert_eq!(game.players[1].religion.as_deref(), Some("Islam"));
        assert_eq!(game.players[1].religion_beliefs, vec!["feed_the_world".to_string()]);
        assert_eq!(game.players[3].religion.as_deref(), Some("Judaism"));
        assert_eq!(game.players[3].religion_beliefs, vec!["work_ethic".to_string()]);
        assert_eq!(game.religions_founded(), 3);
        assert!(game.players[0].religion.is_none());
        assert!(game.players[0].religion_beliefs.is_empty());
        // Rome follows Catholicism, so its Wonder pays Divine Inspiration's
        // four Faith in the model itself, not in a correction.
        let rome = game.player_city_ids(0)[0];
        let city = &game.cities[&rome];
        assert_eq!(game.city_religion(city), Some("Catholicism"));
        assert!(city.wonders.contains_key(&crate::name!("stonehenge")));
        let mut without = recon.game.clone();
        without.players[2].religion_beliefs.clear();
        assert_eq!(
            game.city_yields_model(rome).faith,
            without.city_yields_model(rome).faith + 4.0,
            "Divine Inspiration reaches a following city's own Faith"
        );
    }

    /// The host's Faith per turn is a correction on the empire figure, like
    /// science and culture; the Faith paid for unused Great Person points is
    /// part of the model's figure and so of what the correction is measured
    /// against — and a class absent from the host's cost map is what makes
    /// its points unused.
    #[test]
    fn host_faith_per_turn_and_unused_great_person_classes_reach_the_board() {
        let mut center = plot(5, 4, "TERRAIN_PLAINS");
        center.o = 0;
        let mut campus = plot(6, 4, "TERRAIN_GRASS");
        campus.o = 0;
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 220,
            width: 10,
            height: 10,
            chunk: 1,
            plots: vec![center, campus],
        }]);
        let mut points = BTreeMap::new();
        points.insert("GREAT_PERSON_CLASS_SCIENTIST".to_string(), 700.0);
        points.insert("GREAT_PERSON_CLASS_MERCHANT".to_string(), 40.0);
        let mut costs = BTreeMap::new();
        costs.insert("GREAT_PERSON_CLASS_MERCHANT".to_string(), 660.0);
        let state = StateSnapshot {
            turn: 220,
            faith_per_turn: Some(61.5),
            faith_sources: Some("+35 from Cities\n+26.5 from Other".to_string()),
            great_person_points: Some(points),
            great_person_costs: Some(costs),
            cities: vec![StateCity {
                id: 10,
                name: "Rome".to_string(),
                x: 5,
                y: 4,
                pop: 8,
                loyalty: 100.0,
                districts: vec![StateDistrict {
                    kind: "DISTRICT_CAMPUS".to_string(),
                    x: 6,
                    y: 4,
                    complete: true,
                    ..StateDistrict::default()
                }],
                ..StateCity::default()
            }],
            ..StateSnapshot::default()
        };
        let recon = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
        let game = &recon.game;
        assert_eq!(
            game.players[0].live_great_person_exhausted,
            Some(["scientist".to_string()].into_iter().collect()),
            "points without a cost: the class has nobody left on the host's timeline"
        );
        assert!(!game.great_person_class_earnable(0, "scientist"));
        assert!(game.great_person_class_earnable(0, "merchant"));
        let rate = game.great_person_points_per_turn(0);
        let scientist = rate.get("scientist").copied().unwrap_or(0.0);
        assert!(scientist > 0.0, "the Campus pays Scientist points: {rate:?}");
        assert_eq!(game.unused_great_person_faith(0), scientist);
        // Cities plus the empire's extras plus the correction equal the host.
        let mut yields = crate::rules::Yields::default();
        for cid in game.player_city_ids(0) {
            yields.add(game.city_yields(cid));
        }
        yields.add(game.player_yield_extras(0));
        yields.add(game.observed_yield_adjustments[&0]);
        assert!((yields.faith - 61.5).abs() < 1e-9, "board faith {} vs host 61.5", yields.faith);
        assert_eq!(state.faith_sources.as_deref(), Some("+35 from Cities\n+26.5 from Other"));

        // An older export without the cost map leaves the engine's own roster
        // in charge, and without a host figure leaves the model's Faith alone.
        let older = StateSnapshot {
            great_person_costs: None,
            faith_per_turn: None,
            ..state.clone()
        };
        let recon = rebuild_from_state(&snapshot, &older, 2, 1, 250, 0);
        assert_eq!(recon.game.players[0].live_great_person_exhausted, None);
        assert_eq!(
            recon.game.observed_yield_adjustments.get(&0).map(|adjustment| adjustment.faith),
            None
        );

        // The mod's own list wins over the cost-map inference, and an empty
        // list is the real answer "everyone is still available" — even when
        // the cost map is `nil`, which alone could not tell that from an old export.
        let explicit = StateSnapshot {
            great_person_exhausted: Some(vec!["GREAT_PERSON_CLASS_WRITER".to_string()]),
            great_person_costs: None,
            ..state.clone()
        };
        let recon = rebuild_from_state(&snapshot, &explicit, 2, 1, 250, 0);
        assert_eq!(
            recon.game.players[0].live_great_person_exhausted,
            Some(["writer".to_string()].into_iter().collect())
        );
        assert!(recon.game.great_person_class_earnable(0, "scientist"));
        let nobody = StateSnapshot {
            great_person_exhausted: Some(Vec::new()),
            great_person_costs: None,
            ..state.clone()
        };
        let recon = rebuild_from_state(&snapshot, &nobody, 2, 1, 250, 0);
        assert_eq!(recon.game.players[0].live_great_person_exhausted, Some(BTreeSet::new()));
    }

    #[test]
    fn host_economy_loyalty_and_city_defense_survive_the_mirror_save() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 50,
            width: 10,
            height: 10,
            chunk: 1,
            plots: vec![plot(5, 4, "TERRAIN_PLAINS")],
        }]);
        let state = StateSnapshot {
            turn: 50,
            science: 6.75,
            culture: 6.03125,
            trade_capacity: Some(3),
            cities: vec![StateCity {
                id: 10,
                name: "Istanbul".to_string(),
                x: 5,
                y: 4,
                pop: 9,
                loyalty: 100.0,
                loyalty_per_turn: 10.2656,
                defense: 40.0,
                ..StateCity::default()
            }],
            ..StateSnapshot::default()
        };

        let recon = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
        let city = recon.game.player_city_ids(0)[0];
        assert_eq!(recon.game.city_loyalty_per_turn(&recon.game.cities[&city]), 10.2656);
        assert_eq!(recon.game.city_strength(city), 40.0);
        assert_eq!(recon.game.trade_capacity(0), 3);
        let mut yields = crate::rules::Yields::default();
        for cid in recon.game.player_city_ids(0) {
            yields.add(recon.game.city_yields(cid));
        }
        yields.add(recon.game.observed_yield_adjustments[&0]);
        assert!((yields.science - 6.75).abs() < 1e-9);
        assert!((yields.culture - 6.03125).abs() < 1e-9);

        let saved = serde_json::to_string(&recon.game).expect("save mirrored game");
        let loaded: crate::game::Game = serde_json::from_str(&saved).expect("load mirrored game");
        assert_eq!(loaded.city_loyalty_per_turn(&loaded.cities[&city]), 10.2656);
        assert_eq!(loaded.city_strength(city), 40.0);
        assert_eq!(loaded.trade_capacity(0), 3);
        assert_eq!(loaded.observed_yield_adjustments[&0], recon.game.observed_yield_adjustments[&0]);
    }

    #[test]
    fn exact_city_economy_and_great_work_survive_reconstruction() {
        let mut center = plot(5, 4, "TERRAIN_PLAINS");
        center.o = 0;
        let mut worked = plot(6, 4, "TERRAIN_GRASS");
        worked.o = 0;
        let mut theater = plot(5, 5, "TERRAIN_PLAINS");
        theater.o = 0;
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 60,
            width: 10,
            height: 10,
            chunk: 1,
            plots: vec![center, worked, theater],
        }]);
        let host_yields = crate::rules::Yields {
            food: 8.25,
            production: 7.5,
            gold: 6.75,
            science: 5.5,
            culture: 9.25,
            faith: 2.0,
        };
        let state = StateSnapshot {
            turn: 60,
            science: host_yields.science,
            culture: host_yields.culture,
            cities: vec![StateCity {
                id: 10,
                name: "Wroclaw".to_string(),
                x: 5,
                y: 4,
                pop: 2,
                buildings: vec!["BUILDING_AMPHITHEATER".to_string()],
                districts: vec![StateDistrict {
                    kind: "DISTRICT_THEATER".to_string(),
                    x: 5,
                    y: 5,
                    complete: true,
                    ..StateDistrict::default()
                }],
                worked: Some(vec![StateWorkedPlot { x: 6, y: 4, yields: None }]),
                specialists: Some(vec!["DISTRICT_THEATER".to_string()]),
                great_works: Some(vec![StateGreatWork {
                    kind: "GREATWORK_QU_YUAN_1".to_string(),
                    object: "GREATWORKOBJECT_WRITING".to_string(),
                    era: Some("ERA_CLASSICAL".to_string()),
                    creator: "LOC_GREAT_PERSON_INDIVIDUAL_QU_YUAN_NAME".to_string(),
                    building: "BUILDING_AMPHITHEATER".to_string(),
                    slot: 0,
                }]),
                yields: Some(host_yields),
                producing: Some("UNIT_WARRIOR".to_string()),
                production_progress: 12.5,
                ..StateCity::default()
            }],
            ..StateSnapshot::default()
        };

        let recon = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
        let cid = recon.game.player_city_ids(0)[0];
        let plan = recon.game.city_citizen_plan(cid);
        assert_eq!(
            plan.worked_tiles,
            vec![crate::hex::offset_to_axial(6, 4)],
            "the host assignment, not a freshly optimized replacement, is current state"
        );
        assert_eq!(plan.specialists, vec!["theater_square"]);
        assert_eq!(recon.game.players[0].counters["great_work:writing"], 1);
        assert_eq!(recon.game.players[0].great_work_pieces.len(), 1);
        // And the host's own housing is the model's: the work sits where the
        // export says, not where the model's best-slot heuristic would put it.
        assert_eq!(
            recon.game.observed_great_work_housing.as_ref().and_then(|h| h.get(&cid)).and_then(|k| k.get("writing")),
            Some(&1)
        );
        assert_eq!(recon.game.housed_great_works(0).get(&cid).and_then(|k| k.get("writing")), Some(&1));
        assert_eq!(recon.game.cities[&cid].production, 12.5);
        assert_eq!(recon.game.city_yields(cid), host_yields);

        let saved = serde_json::to_string(&recon.game).expect("save exact city mirror");
        let loaded: crate::game::Game = serde_json::from_str(&saved).expect("load exact city mirror");
        assert_eq!(loaded.city_citizen_plan(cid).worked_tiles, plan.worked_tiles);
        assert_eq!(loaded.city_yields(cid), host_yields);
        assert_eq!(loaded.players[0].counters["great_work:writing"], 1);
    }

    /// ★★★★★ A DISTRICT PLOT IN THE HOST'S WORKED LIST IS A SPECIALIST, NOT A TILE.
    ///
    /// `Citizens:IsPlotWorked` answers true for a Campus a citizen staffs, and the
    /// export names that citizen in `specialists`. Importing the plot as a worked
    /// tile as well paid the specialist twice — its slot yield AND the terrain
    /// under the district. Measured on live run civvis-20260816T011314Z: Cumae
    /// with two Campus specialists and one Industrial Zone specialist read +2
    /// Food, +4 Production over the host for twenty turns.
    #[test]
    fn a_worked_district_plot_is_the_specialist_not_a_second_tile() {
        let mut center = plot(5, 4, "TERRAIN_PLAINS");
        center.o = 0;
        let mut worked = plot(6, 4, "TERRAIN_GRASS");
        worked.o = 0;
        let mut campus = plot(5, 5, "TERRAIN_GRASS_HILLS");
        campus.o = 0;
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 60,
            width: 10,
            height: 10,
            chunk: 1,
            plots: vec![center, worked, campus],
        }]);
        let state = StateSnapshot {
            turn: 60,
            cities: vec![StateCity {
                id: 10,
                name: "Cumae".to_string(),
                x: 5,
                y: 4,
                pop: 2,
                // `StateCity::default()` is loyalty 0 — the revolt band, which
                // multiplies every yield by zero. A real export always carries
                // loyalty; a fixture must say so or its city yields nothing.
                loyalty: 100.0,
                districts: vec![StateDistrict {
                    kind: "DISTRICT_CAMPUS".to_string(),
                    x: 5,
                    y: 5,
                    complete: true,
                    ..StateDistrict::default()
                }],
                // Firaxis lists the centre, the farmed tile AND the Campus plot.
                worked: Some(vec![
                    StateWorkedPlot { x: 5, y: 4, yields: None },
                    StateWorkedPlot { x: 6, y: 4, yields: None },
                    StateWorkedPlot { x: 5, y: 5, yields: None },
                ]),
                specialists: Some(vec!["DISTRICT_CAMPUS".to_string()]),
                ..StateCity::default()
            }],
            ..StateSnapshot::default()
        };
        let recon = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
        let cid = recon.game.player_city_ids(0)[0];
        let plan = recon.game.city_citizen_plan(cid);
        assert_eq!(
            plan.worked_tiles,
            vec![crate::hex::offset_to_axial(6, 4)],
            "the Campus plot is the specialist's seat, not a tile job"
        );
        assert_eq!(plan.specialists, vec!["campus"]);
        let ledger = recon.game.city_yield_ledger(cid);
        assert_eq!(ledger.tiles.len(), 1);
        assert_eq!(ledger.specialists.len(), 1);
    }

    /// ★★★★★ THE HOST'S PER-PLOT YIELDS CROSS AS TILE-LEVEL CORRECTIONS.
    ///
    /// Some of what a tile pays only the host can know — the fertility an
    /// eruption left (Rome on run civvis-20260816T003229Z read +12 Food over the
    /// model on volcanic soil for forty turns). With `worked[].yields` and
    /// `center_yields` in the export, the mirror pays each plot what the host
    /// pays it, the city correction carries only what is left, and the modelled
    /// tile stays readable beside the correction.
    #[test]
    fn host_plot_yields_become_tile_corrections_and_the_model_stays_readable() {
        let mut center = plot(5, 4, "TERRAIN_PLAINS");
        center.o = 0;
        let mut worked = plot(6, 4, "TERRAIN_GRASS");
        worked.o = 0;
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 60,
            width: 10,
            height: 10,
            chunk: 1,
            plots: vec![center, worked],
        }]);
        // Grassland pays 2 Food in the ruleset; the host says this one pays 4
        // Food and 1 Production (fertile ground the tile catalogue cannot see).
        let host_plot = crate::rules::Yields {
            food: 4.0,
            production: 1.0,
            ..crate::rules::Yields::default()
        };
        // Plains centre floors to 2 Food / 1 Production; the host says 3 / 2.
        let host_center = crate::rules::Yields {
            food: 3.0,
            production: 2.0,
            ..crate::rules::Yields::default()
        };
        // Centre 3/2 plus the tile 4/1: the food and production are entirely
        // the two plots; the rest is the city's own (Palace, citizen).
        let host_city = crate::rules::Yields {
            food: 7.0,
            production: 3.0,
            gold: 1.0,
            science: 0.5,
            culture: 1.3,
            faith: 0.0,
        };
        let state = StateSnapshot {
            turn: 60,
            cities: vec![StateCity {
                id: 10,
                name: "Ravenna".to_string(),
                x: 5,
                y: 4,
                pop: 1,
                loyalty: 100.0,
                worked: Some(vec![
                    StateWorkedPlot { x: 5, y: 4, yields: Some(host_center) },
                    StateWorkedPlot { x: 6, y: 4, yields: Some(host_plot) },
                ]),
                center_yields: Some(host_center),
                yields: Some(host_city),
                ..StateCity::default()
            }],
            ..StateSnapshot::default()
        };
        let recon = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
        let cid = recon.game.player_city_ids(0)[0];
        let worked_pos = crate::hex::offset_to_axial(6, 4);
        let center_pos = crate::hex::offset_to_axial(5, 4);
        let tile_fix = recon.game.observed_tile_yield_adjustments[&worked_pos];
        assert!((tile_fix.food - 2.0).abs() < 1e-9, "host 4 against modelled 2: {tile_fix:?}");
        assert!((tile_fix.production - 1.0).abs() < 1e-9);
                // The ledger reads the model, not the corrected board.
        // The ledger reads the model, not the corrected board.
        let ledger = recon.game.city_yield_ledger(cid);
        let center_fix = recon.game.observed_tile_yield_adjustments[&center_pos];
        assert!((center_fix.food - 2.0).abs() < 1e-9, "host 3 against the raw plains 1: {center_fix:?}");
        assert!((center_fix.production - 1.0).abs() < 1e-9);
        assert!((ledger.center.food - 2.0).abs() < 1e-9, "the ledger shows the floored model centre");
        assert!((ledger.tiles[0].1.food - 2.0).abs() < 1e-9);
        assert_eq!(ledger.tile_adjustments.len(), 2);
        // And the board still agrees with the host to the last yield.
        assert_eq!(recon.game.city_yields(cid), host_city);
        // The tile-level part is out of the city-level correction: nothing but
        // the two plots pays Food here, so the city's own Food correction is
        // exactly zero (Production still carries the Palace's own term).
        let city_fix = recon.game.observed_city_yield_adjustments[&cid];
        assert!((city_fix.food - 0.0).abs() < 1e-9, "food is fully explained by the tiles: {city_fix:?}");

        let saved = serde_json::to_string(&recon.game).expect("save");
        let loaded: crate::game::Game = serde_json::from_str(&saved).expect("load");
        assert_eq!(loaded.city_yields(cid), host_city);
        assert_eq!(loaded.observed_tile_yield_adjustments.len(), 2);
    }

    /// ★★★★★ A PILLAGED BUILDING PAYS NOTHING, AND THE EXPORT NOW SAYS WHICH.
    ///
    /// `HasBuilding` stays true for a pillaged Library. Without the pillage list
    /// the mirror paid Antium +6 Science on a raided Campus for twenty turns
    /// (run civvis-20260816T011314Z t147-t170: host 5.9, model 11.2).
    #[test]
    fn pillaged_buildings_cross_the_bridge_and_stop_paying() {
        let mut center = plot(5, 4, "TERRAIN_PLAINS");
        center.o = 0;
        let mut campus = plot(5, 5, "TERRAIN_GRASS_HILLS");
        campus.o = 0;
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 60,
            width: 10,
            height: 10,
            chunk: 1,
            plots: vec![center, campus],
        }]);
        let city = |pillaged: Option<Vec<String>>| StateCity {
            id: 10,
            name: "Antium".to_string(),
            x: 5,
            y: 4,
            pop: 1,
            loyalty: 100.0,
            buildings: vec!["BUILDING_LIBRARY".to_string()],
            pillaged_buildings: pillaged,
            // Pin the citizen so the only difference between the two boards is
            // the Library itself: left to its own governor, the intact city
            // seats its citizen in the Library's specialist slot (+2 more).
            worked: Some(vec![StateWorkedPlot { x: 5, y: 4, yields: None }]),
            specialists: Some(vec![]),
            districts: vec![StateDistrict {
                kind: "DISTRICT_CAMPUS".to_string(),
                x: 5,
                y: 5,
                complete: true,
                ..StateDistrict::default()
            }],
            ..StateCity::default()
        };
        let intact = StateSnapshot {
            turn: 60,
            cities: vec![city(Some(vec![]))],
            ..StateSnapshot::default()
        };
        let raided = StateSnapshot {
            turn: 60,
            cities: vec![city(Some(vec!["BUILDING_LIBRARY".to_string()]))],
            ..StateSnapshot::default()
        };
        let intact = rebuild_from_state(&snapshot, &intact, 2, 1, 250, 0);
        let raided = rebuild_from_state(&snapshot, &raided, 2, 1, 250, 0);
        let intact_cid = intact.game.player_city_ids(0)[0];
        let raided_cid = raided.game.player_city_ids(0)[0];
        assert!(intact.game.cities[&intact_cid].pillaged_buildings.is_empty());
        assert!(raided.game.cities[&raided_cid]
            .pillaged_buildings
            .contains(&crate::name::Name::new("library")));
        let intact_science = intact.game.city_yields_model(intact_cid).science;
        let raided_science = raided.game.city_yields_model(raided_cid).science;
        assert!(
            (intact_science - raided_science - 2.0).abs() < 1e-9,
            "the Library's 2 Science must stop while it is pillaged: {intact_science} vs {raided_science}"
        );
        // An older export says nothing about pillage and must not clear anything.
        let unknown = StateSnapshot {
            turn: 60,
            cities: vec![city(None)],
            ..StateSnapshot::default()
        };
        let unknown = rebuild_from_state(&snapshot, &unknown, 2, 1, 250, 0);
        let unknown_cid = unknown.game.player_city_ids(0)[0];
        assert!(unknown.game.cities[&unknown_cid].pillaged_buildings.is_empty());
    }

    /// The host's Housing ceiling reaches the board as a delta, the Amenity map's
    /// twin: the number beside population is the host's, and a counterfactual
    /// Granary still moves it by its modelled amount.
    #[test]
    fn host_housing_reaches_the_board_as_a_delta() {
        let mut center = plot(5, 4, "TERRAIN_PLAINS");
        center.o = 0;
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 60,
            width: 10,
            height: 10,
            chunk: 1,
            plots: vec![center],
        }]);
        let state = StateSnapshot {
            turn: 60,
            cities: vec![StateCity {
                id: 10,
                name: "Ostia".to_string(),
                x: 5,
                y: 4,
                pop: 3,
                loyalty: 100.0,
                housing: Some(9.0),
                amenities: 1.0,
                amenities_needed: 2.0,
                ..StateCity::default()
            }],
            ..StateSnapshot::default()
        };
        let recon = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
        let cid = recon.game.player_city_ids(0)[0];
        let city = &recon.game.cities[&cid];
        assert!((recon.game.city_housing(city) - 9.0).abs() < 1e-9);
        assert_eq!(recon.game.city_amenities(city), 1, "the count reads the host's, not the model's");
        assert_eq!(recon.game.city_amenity_surplus(city), -1);
        let saved = serde_json::to_string(&recon.game).expect("save");
        let loaded: crate::game::Game = serde_json::from_str(&saved).expect("load");
        assert!((loaded.city_housing(&loaded.cities[&cid]) - 9.0).abs() < 1e-9);
    }

    /// ★★★★★ THE PALACE SITS WHERE THE HOST'S CAPITAL IS.
    ///
    /// `place_city` flags the first city seated for a player as its capital, so
    /// after the founding city fell the mirror paid the Palace in whichever city
    /// the export listed first (Antium) while the host had moved it (Aquileia):
    /// 5 Gold, 2 Production, 2 Science, 1 Culture wrong in two cities for the
    /// rest of run civvis-20260816T040537Z.
    #[test]
    fn the_palace_follows_the_hosts_capital_flag() {
        let mut first = plot(5, 4, "TERRAIN_PLAINS");
        first.o = 0;
        let mut second = plot(8, 4, "TERRAIN_PLAINS");
        second.o = 0;
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 90,
            width: 12,
            height: 10,
            chunk: 1,
            plots: vec![first, second],
        }]);
        let city = |id: i64, name: &str, x: i32, capital: bool| StateCity {
            id,
            name: name.to_string(),
            x,
            y: 4,
            pop: 3,
            loyalty: 100.0,
            capital,
            ..StateCity::default()
        };
        // Listed first, but NOT the capital: the host moved the Palace to the
        // second city after the founding city was lost.
        let state = StateSnapshot {
            turn: 90,
            cities: vec![city(2, "Antium", 5, false), city(3, "Aquileia", 8, true)],
            ..StateSnapshot::default()
        };
        let recon = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
        let antium = recon.game.city_at(crate::hex::offset_to_axial(5, 4)).unwrap();
        let aquileia = recon.game.city_at(crate::hex::offset_to_axial(8, 4)).unwrap();
        assert!(!recon.game.cities[&antium].is_capital);
        assert!(recon.game.cities[&aquileia].is_capital);
        assert!(!recon.game.city_has_palace(&recon.game.cities[&antium]));
        assert!(recon.game.city_has_palace(&recon.game.cities[&aquileia]));
    }

    /// The tiles export's pillage bit reaches the tile, and only where an
    /// improvement stands.
    #[test]
    fn a_pillaged_improvement_crosses_as_pillaged_and_pays_nothing() {
        let mut center = plot(5, 4, "TERRAIN_PLAINS");
        center.o = 0;
        let mut pasture = plot(6, 4, "TERRAIN_PLAINS");
        pasture.o = 0;
        pasture.r = Some("RESOURCE_HORSES".to_string());
        pasture.im = Some("IMPROVEMENT_PASTURE".to_string());
        pasture.p = true;
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 60,
            width: 10,
            height: 10,
            chunk: 1,
            plots: vec![center, pasture],
        }]);
        let state = StateSnapshot {
            turn: 60,
            cities: vec![StateCity {
                id: 10,
                name: "Aquileia".to_string(),
                x: 5,
                y: 4,
                pop: 1,
                loyalty: 100.0,
                worked: Some(vec![StateWorkedPlot { x: 6, y: 4, yields: None }]),
                ..StateCity::default()
            }],
            ..StateSnapshot::default()
        };
        let recon = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
        let pos = crate::hex::offset_to_axial(6, 4);
        let tile = recon.game.map.get(pos).expect("the pasture plot is on the board");
        assert_eq!(tile.improvement.as_deref(), Some("pasture"));
        assert!(tile.pillaged, "the host's pillage bit must reach the tile");
        // Pillaged, the pasture's Production stops: plains + horses only.
        let paid = recon.game.modeled_tile_yields(pos);
        let mut unpillaged = tile.clone();
        unpillaged.pillaged = false;
        let full = recon.game.rules.tile_yields(&unpillaged);
        assert!(paid.production + 1.0 - full.production < 1e-9 && full.production - paid.production >= 1.0 - 1e-9,
            "pillaged {paid:?} vs standing {full:?}");
    }

    /// ★★★★ THE AGE AND ITS DEDICATIONS CROSS THE BRIDGE.
    ///
    /// The three age flags were exported and read by nothing, so every mirrored
    /// board sat in a Normal Age and no Dedication ever paid. Heartbeat of Steam
    /// ("+10 from Campus" under Production in the host's own ledger, run
    /// civvis-20260816T132247Z) was the largest gap of that game's Golden Age.
    #[test]
    fn the_age_and_its_dedications_reach_the_seat() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 180,
            width: 8,
            height: 8,
            chunk: 1,
            plots: vec![plot(3, 3, "TERRAIN_GRASS")],
        }]);
        let mut state = StateSnapshot {
            turn: 180,
            golden_age: Some(true),
            dark_age: Some(false),
            heroic_golden_age: Some(false),
            dedications: Some(vec![
                "COMMEMORATION_INDUSTRIAL".to_string(),
                "COMMEMORATION_ECONOMIC".to_string(),
                "COMMEMORATION_NOT_A_THING".to_string(),
            ]),
            ..StateSnapshot::default()
        };
        let mut mirror = LiveMirror::new(&snapshot, &state, 2, 1, 250, 0);
        assert_eq!(mirror.game.players[0].age, "golden");
        assert!(mirror.game.players[0].dedications.contains("heartbeat_of_steam"));
        assert!(mirror.game.players[0].dedications.contains("reform_the_coinage"));
        assert_eq!(mirror.game.players[0].dedications.len(), 2, "unknown types are dropped");

        // The age turns over: the sync follows the flags, heroic outranking golden.
        state.turn = 181;
        state.heroic_golden_age = Some(true);
        mirror.sync(&snapshot, &state, 0);
        assert_eq!(mirror.game.players[0].age, "heroic");
        state.turn = 182;
        state.heroic_golden_age = Some(false);
        state.golden_age = Some(false);
        state.dark_age = Some(true);
        state.dedications = Some(vec![]);
        mirror.sync(&snapshot, &state, 0);
        assert_eq!(mirror.game.players[0].age, "dark");
        assert!(mirror.game.players[0].dedications.is_empty());
        // An older export says nothing and changes nothing.
        state.turn = 183;
        state.dark_age = None;
        state.golden_age = None;
        state.heroic_golden_age = None;
        state.dedications = None;
        mirror.sync(&snapshot, &state, 0);
        assert_eq!(mirror.game.players[0].age, "dark");
    }

    /// ★★★★★ A CORRECTION IS MEASURED AFTER EVERYTHING IT CORRECTS FOR IS ON THE BOARD.
    ///
    /// The rival's per-turn correction was derived before the loop that writes
    /// a rival city's Population (planted at one) — measured against a size-one
    /// city, paid on the size-eleven one: Nubia read 174 Science against the
    /// host's 141 on run civvis-20260816T175306Z. And the seat's own Dedications
    /// were applied after its correction: Ravenna read 14.5 Science against 9.5.
    /// Both boards must read the host's figure exactly, rebuild and sync alike.
    #[test]
    fn corrections_are_measured_after_population_and_dedications_are_on_the_board() {
        let side = 16;
        let plots: Vec<Plot> = (0..side)
            .flat_map(|x| {
                (0..side).map(move |y| Plot {
                    x,
                    y,
                    im: None,
                    t: Some("TERRAIN_GRASS".to_string()),
                    f: None,
                    r: None,
                    o: if (x, y) == (3, 3) {
                        0
                    } else if (x, y) == (11, 11) {
                        3
                    } else {
                        -1
                    },
                    w: false,
                    i: false,
                    fw: false,
                    rv: 0,
                    ri: false,
                    ct: None,
                    cl: -1,
                    p: false,
                    d: None,
                    dc: None,
                    wo: None,
                    rt: None,
                    rp: false,
                })
            })
            .collect();
        let snapshot = Snapshot::from_chunks(&[TilesChunk { turn: 90, width: side, height: side, chunk: 1, plots }]);
        let mut state = StateSnapshot {
            turn: 90,
            science: 30.0,
            culture: 12.0,
            golden_age: Some(true),
            dark_age: Some(false),
            heroic_golden_age: Some(false),
            dedications: Some(vec!["COMMEMORATION_SCIENTIFIC".to_string()]),
            cities: vec![StateCity {
                id: 1, name: "Rome".to_string(), x: 3, y: 3, pop: 6, loyalty: 100.0, capital: true,
                districts: vec![StateDistrict {
                    kind: "DISTRICT_COMMERCIAL_HUB".to_string(), x: 4, y: 3, complete: true,
                    ..StateDistrict::default()
                }],
                yields: Some(crate::rules::Yields { food: 20.0, production: 9.0, gold: 8.0, science: 9.5, culture: 6.0, faith: 0.0 }),
                ..StateCity::default()
            }],
            rivals: vec![StateRival {
                player: 3, civ: "CIVILIZATION_NUBIA".to_string(),
                science: 41.0, culture: 22.0,
                cities: vec![StateCity {
                    id: 3, name: "Meroe".to_string(), x: 11, y: 11, pop: 11, loyalty: 100.0, capital: true,
                    ..StateCity::default()
                }],
                ..StateRival::default()
            }],
            ..StateSnapshot::default()
        };
        let seat_yields = |game: &crate::game::Game, seat: usize| {
            let mut total = crate::rules::Yields::default();
            for cid in game.player_city_ids(seat) {
                total.add(game.city_yields(cid));
            }
            if let Some(adjustment) = game.observed_yield_adjustments.get(&seat) {
                total.add(*adjustment);
            }
            total.add(game.player_yield_extras(seat));
            total
        };
        let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        let rome = mirror.game.player_city_ids(0)[0];
        assert!((mirror.game.city_yields(rome).science - 9.5).abs() < 1e-9,
            "the city reads the host after its Dedication is on the seat: {:?}", mirror.game.city_yields(rome));
        assert!((seat_yields(&mirror.game, 0).science - 30.0).abs() < 1e-9);
        let meroe = mirror.game.player_city_ids(1)[0];
        assert_eq!(mirror.game.cities[&meroe].pop, 11);
        assert!((seat_yields(&mirror.game, 1).science - 41.0).abs() < 1e-9,
            "the rival seat reads the host after its city's Population is on the board: {:?}", seat_yields(&mirror.game, 1));

        // And after a sync that grows the rival and moves our Dedication.
        state.turn = 91;
        state.rivals[0].cities[0].pop = 14;
        state.rivals[0].science = 47.0;
        state.dedications = Some(vec!["COMMEMORATION_INDUSTRIAL".to_string()]);
        mirror.sync(&snapshot, &state, 0);
        assert_eq!(mirror.game.cities[&meroe].pop, 14);
        assert!((seat_yields(&mirror.game, 1).science - 47.0).abs() < 1e-9);
        assert!((mirror.game.city_yields(rome).science - 9.5).abs() < 1e-9);
    }

    #[test]
    fn a_rivals_route_into_our_city_is_seated_and_the_hosts_trade_policy_pays_it_before_the_correction() {
        let side = 16;
        let plots: Vec<Plot> = (0..side)
            .flat_map(|x| {
                (0..side).map(move |y| Plot {
                    x,
                    y,
                    im: None,
                    t: Some("TERRAIN_GRASS".to_string()),
                    f: None,
                    r: None,
                    o: if (x, y) == (3, 3) {
                        0
                    } else if (x, y) == (11, 11) {
                        3
                    } else {
                        -1
                    },
                    w: false,
                    i: false,
                    fw: false,
                    rv: 0,
                    ri: false,
                    ct: None,
                    cl: -1,
                    p: false,
                    d: None,
                    dc: None,
                    wo: None,
                    rt: None,
                    rp: false,
                })
            })
            .collect();
        let snapshot = Snapshot::from_chunks(&[TilesChunk { turn: 90, width: side, height: side, chunk: 1, plots }]);
        let host_gold = 12.0;
        let mut state = StateSnapshot {
            turn: 90,
            science: 30.0,
            culture: 12.0,
            resolutions: Some(vec![
                StateResolution { kind: "WC_RES_TRADE_TREATY".to_string(), option: 1, target: "0".to_string() },
                StateResolution { kind: "WC_RES_LUXURY".to_string(), option: 2, target: "RESOURCE_SILK".to_string() },
                StateResolution { kind: "WC_RES_ARMS_CONTROL".to_string(), option: 1, target: "".to_string() },
            ]),
            congress_turns_left: Some(11),
            cities: vec![StateCity {
                id: 1, name: "Cumae".to_string(), x: 3, y: 3, pop: 6, loyalty: 100.0, capital: true,
                yields: Some(crate::rules::Yields { food: 20.0, production: 9.0, gold: host_gold, science: 9.5, culture: 6.0, faith: 0.0 }),
                incoming_routes: Some(StateIncomingRoutes {
                    foreign: 1,
                    domestic: 0,
                    origins: vec![StateRouteOrigin { x: 11, y: 11, player: 3 }],
                }),
                ..StateCity::default()
            }],
            rivals: vec![StateRival {
                player: 3, civ: "CIVILIZATION_MAORI".to_string(),
                cities: vec![StateCity {
                    id: 3, name: "Auckland".to_string(), x: 11, y: 11, pop: 8, loyalty: 100.0, capital: true,
                    ..StateCity::default()
                }],
                ..StateRival::default()
            }],
            ..StateSnapshot::default()
        };
        let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        let cumae = mirror.game.player_city_ids(0)[0];
        let auckland = mirror.game.player_city_ids(1)[0];
        // The route is on the board, owned by the rival's SEAT, from its city.
        let seated: Vec<_> = mirror.game.routes.iter().filter(|route| route.dest == cumae).collect();
        assert_eq!(seated.len(), 1, "one incoming route: {:?}", mirror.game.routes);
        assert_eq!(seated[0].origin, auckland);
        assert_eq!(seated[0].owner, 1);
        // The host's Congress is the model's Congress: Trade Policy A on our
        // seat, Luxury Policy B on silk, and the resolution the model has no
        // rule for is reported rather than guessed.
        assert!(mirror.game.congress_effect_active("trade_policy", "A", "0"));
        assert!(mirror.game.congress_effect_active("luxury_policy", "B", "silk"));
        assert_eq!(mirror.game.active_congress_effects.len(), 2);
        assert_eq!(mirror.game.active_congress_effects[0].expires, 90 + 11 + 1);
        assert!(mirror.unmapped.iter().any(|issue| issue == "congress:WC_RES_ARMS_CONTROL:1:"),
            "unmapped: {:?}", mirror.unmapped);
        // The model pays the +4 itself, so the correction it derives is the
        // host's number minus a model that already includes it — the city reads
        // the host either way, and the model's own view carries the treaty.
        assert!((mirror.game.city_yields(cumae).gold - host_gold).abs() < 1e-9);
        let model = mirror.game.city_yields_model(cumae).gold;
        mirror.game.active_congress_effects.clear();
        let without_treaty = mirror.game.city_yields_model(cumae).gold;
        assert!((model - without_treaty - 4.0).abs() < 1e-9,
            "Trade Policy A pays the destination +4 per incoming foreign route: {} vs {}", model, without_treaty);
        mirror.game.routes.clear();
        let without_route = mirror.game.city_yields_model(cumae).gold;
        assert!(without_route <= without_treaty);

        // The next sync re-seats the route and re-reads the Congress; when the
        // host drops both, the board follows and the correction stays honest.
        state.turn = 91;
        mirror.sync(&snapshot, &state, 0);
        assert_eq!(mirror.game.routes.iter().filter(|route| route.dest == cumae).count(), 1);
        assert!(mirror.game.congress_effect_active("trade_policy", "A", "0"));
        state.turn = 92;
        state.resolutions = Some(vec![]);
        state.cities[0].incoming_routes = Some(StateIncomingRoutes::default());
        mirror.sync(&snapshot, &state, 0);
        assert!(mirror.game.active_congress_effects.is_empty());
        assert_eq!(mirror.game.routes.iter().filter(|route| route.dest == cumae).count(), 0);
        assert!((mirror.game.city_yields(cumae).gold - host_gold).abs() < 1e-9);
        // An older export (no `resolutions`) leaves the model's own Congress alone.
        state.turn = 93;
        state.resolutions = None;
        mirror.game.active_congress_effects.push(crate::game::CongressEffect {
            resolution: "patronage".to_string(), outcome: "A".to_string(),
            target: "scientist".to_string(), expires: 200,
        });
        mirror.sync(&snapshot, &state, 0);
        assert!(mirror.game.congress_effect_active("patronage", "A", "scientist"));
    }

    #[test]
    fn host_resolutions_translate_into_the_models_congress_vocabulary() {
        let rules = crate::rules::Rules::shipped();
        let seats: std::collections::BTreeMap<usize, usize> = [(0, 0), (5, 2)].into_iter().collect();
        let map = |kind: &str, option: i64, target: &str| {
            civvis_congress_effect(
                &rules,
                &StateResolution { kind: kind.to_string(), option, target: target.to_string() },
                &seats,
                120,
            )
            .map(|effect| (effect.resolution, effect.outcome, effect.target))
        };
        assert_eq!(map("WC_RES_TRADE_TREATY", 1, "5"), Some(("trade_policy".into(), "A".into(), "2".into())));
        assert_eq!(map("WC_RES_TRADE_TREATY", 2, "0"), Some(("trade_policy".into(), "B".into(), "0".into())));
        assert_eq!(map("WC_RES_TRADE_TREATY", 1, "9"), None, "an unseated player is not guessed");
        assert_eq!(map("WC_RES_MERCENARY_COMPANIES", 1, "YIELD_PRODUCTION"), Some(("mercenary_companies".into(), "A".into(), "production".into())));
        assert_eq!(map("WC_RES_LUXURY", 1, "RESOURCE_WHALES"), Some(("luxury_policy".into(), "A".into(), "whales".into())));
        assert_eq!(map("WC_RES_URBAN_DEVELOPMENT", 2, "DISTRICT_CAMPUS"), Some(("urban_development_treaty".into(), "B".into(), "campus".into())));
        assert_eq!(map("WC_RES_URBAN_DEVELOPMENT", 1, "DISTRICT_CITY_CENTER"), Some(("urban_development_treaty".into(), "A".into(), "city_center".into())));
        assert_eq!(map("WC_RES_PATRONAGE", 1, "GREAT_PERSON_CLASS_SCIENTIST"), Some(("patronage".into(), "A".into(), "scientist".into())));
        assert_eq!(map("WC_RES_MILITARY_ADVISORY", 2, "PROMOTION_CLASS_MELEE"), Some(("military_advisory".into(), "B".into(), "melee".into())));
        assert_eq!(map("WC_RES_ESPIONAGE_PACT", 1, "UNITOPERATION_SPY_SIPHON_FUNDS"), Some(("espionage_pact".into(), "A".into(), "siphon_funds".into())));
        assert_eq!(map("WC_RES_HERITAGE_ORG", 1, "GREATWORKOBJECT_WRITING"), Some(("heritage_organization".into(), "A".into(), "writing".into())));
        assert_eq!(map("WC_RES_DEFORESTATION_TREATY", 1, "FEATURE_FOREST"), Some(("deforestation_treaty".into(), "A".into(), "forest".into())));
        assert_eq!(map("WC_RES_DEFORESTATION_TREATY", 2, "FEATURE_JUNGLE"), Some(("deforestation_treaty".into(), "B".into(), "jungle".into())));
        assert_eq!(map("WC_RES_HERITAGE_ORG", 2, "GREATWORKOBJECT_SCULPTURE"), Some(("heritage_organization".into(), "B".into(), "art".into())));
        assert_eq!(map("WC_RES_MILITARY_ADVISORY", 1, "PROMOTION_CLASS_APOSTLE"), Some(("military_advisory".into(), "A".into(), "religious_apostle".into())));
        assert_eq!(map("WC_RES_GLOBAL_ENERGY_TREATY", 1, "BUILDING_FOSSIL_FUEL_POWER_PLANT"), Some(("global_energy_treaty".into(), "A".into(), "oil_power_plant".into())));
        assert_eq!(map("WC_RES_WORLD_IDEOLOGY", 1, "GOVERNMENT_DEMOCRACY"), Some(("world_ideology".into(), "A".into(), "democracy".into())));
        assert_eq!(map("WC_RES_PUBLIC_WORKS", 1, "PROJECT_MANHATTAN_PROJECT"), Some(("public_works_program".into(), "A".into(), "manhattan_project".into())));
        assert_eq!(map("WC_RES_TRADE_TREATY", 0, "0"), None, "an option the mod could not read is not guessed");
        assert_eq!(map("WC_RES_SOVEREIGNTY", 1, "MINOR_CIV_TRADE"), None, "no model rule, no effect");
    }

    #[test]
    fn observed_worker_swap_overrides_the_nearest_city_guess() {
        let mut first_center = plot(2, 2, "TERRAIN_PLAINS");
        first_center.o = 0;
        let mut second_center = plot(6, 2, "TERRAIN_PLAINS");
        second_center.o = 0;
        let mut swapped = plot(3, 2, "TERRAIN_GRASS");
        swapped.o = 0;
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 70,
            width: 10,
            height: 10,
            chunk: 1,
            plots: vec![first_center, second_center, swapped],
        }]);
        let state = StateSnapshot {
            turn: 70,
            cities: vec![
                StateCity {
                    id: 1, name: "Rome".to_string(), x: 2, y: 2, pop: 2,
                    worked: Some(vec![]),
                    ..StateCity::default()
                },
                StateCity {
                    id: 2, name: "Lugdunum".to_string(), x: 6, y: 2, pop: 2,
                    worked: Some(vec![StateWorkedPlot { x: 3, y: 2, yields: None }]),
                    ..StateCity::default()
                },
            ],
            ..StateSnapshot::default()
        };

        let recon = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
        let first = recon.game.city_at(crate::hex::offset_to_axial(2, 2)).unwrap();
        let second = recon.game.city_at(crate::hex::offset_to_axial(6, 2)).unwrap();
        let worked = crate::hex::offset_to_axial(3, 2);

        assert_eq!(recon.game.city_citizen_plan(second).worked_tiles, vec![worked]);
        assert_eq!(recon.game.map.tiles[&worked].owner_city, Some(second));
        assert!(!recon.game.cities[&first].owned_tiles.contains(&worked));
        assert!(recon.game.cities[&second].owned_tiles.contains(&worked));
    }

    #[test]
    fn firaxis_city_center_is_implicit_and_palace_yields_are_counted_once() {
        let mut center = plot(5, 4, "TERRAIN_PLAINS");
        center.o = 0;
        let mut worked = plot(6, 4, "TERRAIN_GRASS");
        worked.o = 0;
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 3,
            width: 10,
            height: 10,
            chunk: 1,
            plots: vec![center, worked],
        }]);
        let state = StateSnapshot {
            turn: 3,
            seat: Seat {
                civ: "CIVILIZATION_CHINA".to_string(),
                leader: "LEADER_QIN_SHI_HUANG".to_string(),
                ..Seat::default()
            },
            cities: vec![StateCity {
                id: 10,
                name: "Xi'an".to_string(),
                x: 5,
                y: 4,
                pop: 1,
                capital: true,
                buildings: vec!["BUILDING_PALACE".to_string()],
                worked: Some(vec![
                    StateWorkedPlot { x: 5, y: 4, yields: None },
                    StateWorkedPlot { x: 6, y: 4, yields: None },
                ]),
                ..StateCity::default()
            }],
            ..StateSnapshot::default()
        };

        let recon = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
        let cid = recon.game.player_city_ids(0)[0];
        assert_eq!(
            recon.game.city_citizen_plan(cid).worked_tiles,
            vec![crate::hex::offset_to_axial(6, 4)],
            "Firaxis's explicit city centre is not a second citizen assignment"
        );
        assert!(
            !recon.unmapped.contains(&"Xi'an:worked_plot".to_string()),
            "the host's normal GetWorkedPlots shape must be accepted"
        );
        assert!(
            !recon.game.cities[&cid]
                .buildings
                .contains(&crate::name!("palace")),
            "the intrinsic Palace must not also enter the ordinary building list"
        );
        let mut without_explicit_palace = state;
        without_explicit_palace.cities[0].buildings.clear();
        let control = rebuild_from_state(&snapshot, &without_explicit_palace, 2, 1, 250, 0);
        let control_city = control.game.player_city_ids(0)[0];
        assert_eq!(
            recon.game.city_yields_model(cid),
            control.game.city_yields_model(control_city),
            "Firaxis's explicit Palace row must not add a second copy of its yields"
        );
    }

    #[test]
    fn met_city_state_is_an_actor_instead_of_anonymous_blocked_land() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 30,
            width: 20,
            height: 20,
            chunk: 1,
            plots: vec![plot(6, 6, "TERRAIN_PLAINS"), plot(7, 6, "TERRAIN_PLAINS")],
        }]);
        let state = StateSnapshot {
            turn: 30,
            minors: vec![StateMinor {
                player: 6,
                civ: "CIVILIZATION_KABUL".to_string(),
                score: 91,
                military: 74.0,
                suzerain: 0,
                envoys: 3,
                cities: vec![StateCity {
                    id: 70,
                    name: "Kabul".to_string(),
                    x: 6,
                    y: 6,
                    pop: 4,
                    defense: 28.0,
                    ..StateCity::default()
                }],
                units: vec![StateUnit {
                    id: 71,
                    kind: "UNIT_WARRIOR".to_string(),
                    x: 7,
                    y: 6,
                    hp: 100.0,
                    ..StateUnit::default()
                }],
                ..StateMinor::default()
            }],
            ..StateSnapshot::default()
        };

        let recon = rebuild_from_state(&snapshot, &state, 6, 1, 250, 0);
        let minor = recon
            .game
            .players
            .iter()
            .find(|player| player.is_minor && player.civ == "Kabul")
            .expect("Kabul minor seat");
        assert!(recon.game.has_met(0, minor.id));
        assert_eq!(recon.game.score(minor.id), 91);
        assert_eq!(recon.game.military_power(minor.id), 74.0);
        assert_eq!(recon.game.envoys_at(0, minor.id), 3);
        assert_eq!(recon.game.suzerain_of(minor.id), Some(0));
        assert!(recon.game.cities.values().any(|city| {
            city.owner == minor.id && city.name == "Kabul"
        }));
        assert!(recon.game.units.values().any(|unit| unit.owner == minor.id));
    }

    /// ⚠ `suzerain: -1` is the export's NO-suzerain sentinel, and skipping the
    /// seeding is not enough to mirror it: our own factual envoys are already
    /// on the board and no rival delegation is, so three unopposed envoys
    /// elect seat 0 by walkover. Measured live on `civvis-20260808T003040Z`:
    /// `taruga suzerain Civ6=-1 CIVVIS=0`.
    #[test]
    fn no_suzerain_sentinel_does_not_elect_seat_zero_by_walkover() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 30,
            width: 20,
            height: 20,
            chunk: 1,
            plots: vec![plot(6, 6, "TERRAIN_PLAINS")],
        }]);
        let state = StateSnapshot {
            turn: 30,
            minors: vec![StateMinor {
                player: 6,
                civ: "CIVILIZATION_TARUGA".to_string(),
                suzerain: -1,
                envoys: 3,
                cities: vec![StateCity {
                    id: 70,
                    name: "Taruga".to_string(),
                    x: 6,
                    y: 6,
                    pop: 4,
                    ..StateCity::default()
                }],
                ..StateMinor::default()
            }],
            ..StateSnapshot::default()
        };

        let recon = rebuild_from_state(&snapshot, &state, 6, 1, 250, 0);
        let minor = recon
            .game
            .players
            .iter()
            .find(|player| player.is_minor && player.civ == "Taruga")
            .expect("Taruga minor seat");
        // Our delegation is the export's fact and must survive untouched…
        assert_eq!(recon.game.envoys_at(0, minor.id), 3);
        // …while the host's "none" answer must be the board's answer too.
        assert_eq!(
            recon.game.suzerain_of(minor.id),
            None,
            "Civ 6 reported no suzerain (-1); the mirror must not read as ours"
        );
    }

    /// ★★★★★ The envoys the seat is holding reach the board, so `SendEnvoy` is
    /// enumerated against a met city-state — the one input the deployed
    /// `advanced_envoys` pass never had on a live board. Measured on the twelve
    /// Settler games of 2026-08-15/16: 40–70 unspent at the end, 0 suzerainties
    /// in 11 of 12. The host's `-1` ("could not answer") and an absent field
    /// must leave the board's count alone rather than zero it.
    #[test]
    fn unspent_envoys_reach_the_board_and_send_envoy_is_enumerated() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 60,
            width: 20,
            height: 20,
            chunk: 1,
            plots: vec![plot(6, 6, "TERRAIN_PLAINS"), plot(12, 12, "TERRAIN_PLAINS")],
        }]);
        let minor = StateMinor {
            player: 9,
            civ: "CIVILIZATION_GENEVA".to_string(),
            suzerain: -1,
            envoys: 1,
            cities: vec![StateCity {
                id: 90,
                name: "Geneva".to_string(),
                x: 6,
                y: 6,
                pop: 4,
                ..StateCity::default()
            }],
            ..StateMinor::default()
        };
        let state = StateSnapshot {
            turn: 60,
            envoys_free: Some(4),
            minors: vec![minor.clone()],
            ..StateSnapshot::default()
        };

        let recon = rebuild_from_state(&snapshot, &state, 6, 1, 250, 0);
        let geneva = recon
            .game
            .players
            .iter()
            .find(|player| player.is_minor && player.civ == "Geneva")
            .expect("Geneva minor seat");
        assert_eq!(recon.game.players[0].envoys_free, 4, "the held count is the host's fact");
        assert!(
            recon
                .game
                .legal_actions(0)
                .iter()
                .any(|action| matches!(action, crate::game::Action::SendEnvoy { player } if *player == geneva.id)),
            "a held envoy and a met city-state must enumerate SendEnvoy"
        );
        // Sending one on the planning board spends one and lands on Geneva.
        let mut planned = recon.game.clone();
        planned
            .apply(0, &crate::game::Action::SendEnvoy { player: geneva.id })
            .expect("the envoy is legal");
        assert_eq!(planned.players[0].envoys_free, 3);
        assert_eq!(planned.envoys_at(0, geneva.id), 2);

        // The host that did not answer, in both shapes.
        let silent = StateSnapshot { turn: 60, envoys_free: None, minors: vec![minor.clone()], ..StateSnapshot::default() };
        assert_eq!(rebuild_from_state(&snapshot, &silent, 6, 1, 250, 0).game.players[0].envoys_free, 0);
        let failed = StateSnapshot { turn: 60, envoys_free: Some(-1), minors: vec![minor], ..StateSnapshot::default() };
        assert_eq!(rebuild_from_state(&snapshot, &failed, 6, 1, 250, 0).game.players[0].envoys_free, 0);
    }

    #[test]
    fn renamed_city_state_uses_exported_capital_instead_of_legacy_type_id() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 30,
            width: 20,
            height: 20,
            chunk: 1,
            plots: vec![plot(6, 6, "TERRAIN_PLAINS")],
        }]);
        let state = StateSnapshot {
            turn: 30,
            minors: vec![StateMinor {
                player: 8,
                civ: "CIVILIZATION_JAKARTA".to_string(),
                cities: vec![StateCity {
                    id: 65_536,
                    name: "Bandar Brunei".to_string(),
                    x: 6,
                    y: 6,
                    pop: 2,
                    capital: true,
                    ..StateCity::default()
                }],
                ..StateMinor::default()
            }],
            ..StateSnapshot::default()
        };

        let recon = rebuild_from_state(&snapshot, &state, 6, 1, 250, 0);
        assert!(recon
            .game
            .players
            .iter()
            .any(|player| player.is_minor && player.civ == "Bandar Brunei"));
        assert!(!recon
            .unmapped
            .iter()
            .any(|name| name == "CIVILIZATION_JAKARTA"));
    }

    #[test]
    fn dormant_free_cities_does_not_turn_kabul_into_a_turn_one_enemy() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 1,
            width: 12,
            height: 12,
            chunk: 1,
            plots: vec![plot(5, 5, "TERRAIN_PLAINS")],
        }]);
        let state = StateSnapshot {
            turn: 1,
            minors: vec![StateMinor {
                player: 62,
                civ: "CIVILIZATION_FREE_CITIES".to_string(),
                at_war: true,
                ..StateMinor::default()
            }],
            ..StateSnapshot::default()
        };

        let recon = rebuild_from_state(&snapshot, &state, 6, 1, 250, 0);
        assert_eq!(
            recon
                .game
                .players
                .iter()
                .filter(|player| player.is_minor && !player.is_barbarian)
                .count(),
            0,
            "an empty Firaxis Free Cities placeholder must consume no city-state seat"
        );
        let free = recon.game.players.iter().find(|player| player.is_free_city).unwrap();
        assert!(!free.alive);
        assert!(!recon.game.at_war.contains(&(0, free.id)));
    }

    #[test]
    fn a_present_free_city_uses_the_dedicated_free_cities_seat() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 80,
            width: 12,
            height: 12,
            chunk: 1,
            plots: vec![plot(5, 5, "TERRAIN_PLAINS")],
        }]);
        let state = StateSnapshot {
            turn: 80,
            minors: vec![StateMinor {
                player: 62,
                civ: "CIVILIZATION_FREE_CITIES".to_string(),
                score: 20,
                military: 35.0,
                at_war: true,
                cities: vec![StateCity {
                    id: 70,
                    name: "Free City".to_string(),
                    x: 5,
                    y: 5,
                    pop: 4,
                    ..StateCity::default()
                }],
                ..StateMinor::default()
            }],
            ..StateSnapshot::default()
        };

        let recon = rebuild_from_state(&snapshot, &state, 6, 1, 250, 0);
        let free = recon.game.players.iter().find(|player| player.is_free_city).unwrap();
        assert!(free.alive);
        assert!(recon.game.is_at_war(0, free.id));
        assert_eq!(recon.game.score(free.id), 20);
        assert_eq!(recon.game.military_power(free.id), 35.0);
        assert!(recon.game.cities.values().any(|city| city.owner == free.id));
    }

    #[test]
    fn a_city_state_met_later_uses_a_seat_reserved_by_the_lobby() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 1,
            width: 24,
            height: 24,
            chunk: 1,
            plots: vec![plot(5, 5, "TERRAIN_PLAINS"), plot(6, 5, "TERRAIN_PLAINS")],
        }]);
        let mut state = StateSnapshot {
            turn: 1,
            seat: Seat {
                city_states: 2,
                ..Seat::default()
            },
            ..StateSnapshot::default()
        };
        let mut mirror = LiveMirror::new(&snapshot, &state, 6, 1, 250, 0);
        assert_eq!(
            mirror
                .game
                .players
                .iter()
                .filter(|player| player.is_minor && !player.is_barbarian)
                .count(),
            2
        );

        state.turn = 2;
        state.minors.push(StateMinor {
            player: 6,
            civ: "CIVILIZATION_KABUL".to_string(),
            cities: vec![StateCity {
                id: 70,
                name: "Kabul".to_string(),
                x: 6,
                y: 5,
                pop: 2,
                ..StateCity::default()
            }],
            ..StateMinor::default()
        });
        mirror.sync(&snapshot, &state, 0);
        let kabul = mirror
            .game
            .players
            .iter()
            .find(|player| player.civ == "Kabul")
            .expect("the newly met city-state uses a reserved seat");
        assert!(mirror.game.cities.values().any(|city| city.owner == kabul.id));
    }

    #[test]
    fn current_city_production_follows_every_live_state() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 8,
            width: 20,
            height: 20,
            chunk: 1,
            plots: vec![plot(5, 5, "TERRAIN_GRASS"), plot(5, 6, "TERRAIN_GRASS")],
        }]);
        let mut state = StateSnapshot {
            turn: 8,
            cities: vec![StateCity {
                id: 1,
                name: "Delhi".to_string(),
                x: 5,
                y: 5,
                pop: 2,
                producing: Some("UNIT_SCOUT".to_string()),
                ..StateCity::default()
            }],
            ..StateSnapshot::default()
        };

        let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        let city = mirror.cid_of[&1];
        assert!(matches!(
            mirror.game.cities[&city].queue.first(),
            Some(crate::game::Item::Unit { unit }) if unit == "scout"
        ));

        state.turn = 9;
        state.cities[0].producing = Some("UNIT_SETTLER".to_string());
        mirror.sync(&snapshot, &state, 0);
        assert!(matches!(
            mirror.game.cities[&city].queue.first(),
            Some(crate::game::Item::Unit { unit }) if unit == "settler"
        ));

        state.turn = 10;
        state.cities[0].producing = None;
        mirror.sync(&snapshot, &state, 0);
        assert!(
            mirror.game.cities[&city].queue.is_empty(),
            "the completed item must not remain as a phantom queue entry"
        );
    }

    /// A city Civilization VI reports as building a WONDER is busy, on both the
    /// first reconstruction and every later sync — the mirror must not seed it
    /// idle and let the planner replace the wonder the next turn (Hagia Sophia,
    /// Rome, run civvis-20260815T202611Z t124→t125).
    #[test]
    fn a_wonder_under_construction_keeps_the_city_queue_busy() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 40,
            width: 20,
            height: 20,
            chunk: 1,
            plots: vec![plot(5, 5, "TERRAIN_GRASS"), plot(6, 5, "TERRAIN_GRASS")],
        }]);
        let mut state = StateSnapshot {
            turn: 40,
            cities: vec![StateCity {
                id: 1,
                name: "Rome".to_string(),
                x: 5,
                y: 5,
                pop: 6,
                producing: Some("BUILDING_HAGIA_SOPHIA".to_string()),
                ..StateCity::default()
            }],
            ..StateSnapshot::default()
        };
        let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        let city = mirror.cid_of[&1];
        assert!(
            matches!(
                mirror.game.cities[&city].queue.first(),
                Some(crate::game::Item::Wonder { wonder, .. }) if wonder == "hagia_sophia"
            ),
            "fresh reconstruction: {:?}",
            mirror.game.cities[&city].queue
        );

        state.turn = 41;
        mirror.sync(&snapshot, &state, 0);
        assert!(
            matches!(
                mirror.game.cities[&city].queue.first(),
                Some(crate::game::Item::Wonder { wonder, .. }) if wonder == "hagia_sophia"
            ),
            "later sync: {:?}",
            mirror.game.cities[&city].queue
        );
    }

    #[test]
    fn live_mirror_permanently_blocks_host_granted_spy_production() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 8,
            width: 20,
            height: 20,
            chunk: 1,
            plots: vec![plot(5, 5, "TERRAIN_GRASS"), plot(6, 5, "TERRAIN_GRASS")],
        }]);
        let mut state = StateSnapshot {
            turn: 8,
            civics: vec!["CIVIC_DIPLOMATIC_SERVICE".to_string()],
            cities: vec![StateCity {
                id: 1,
                name: "Delhi".to_string(),
                x: 5,
                y: 5,
                pop: 2,
                ..StateCity::default()
            }],
            ..StateSnapshot::default()
        };

        let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        let spy = crate::game::Item::Unit {
            unit: crate::name!("spy"),
        };
        let city = mirror.cid_of[&1];
        assert!(
            !mirror.game.can_produce(0, city, &spy),
            // ⚠ This state reports no `spy_capacity`, which is now what the
            // block keys on: an export that cannot say how many Spies the
            // empire may field fails CLOSED, exactly as before. A build that
            // DOES report capacity is allowed to train one while under it —
            // see `live_spies_are_seated_and_the_block_follows_capacity`.
            "an unknown Spy capacity keeps the unconditional block"
        );
        assert_eq!(
            mirror.game.blocked_production[&city],
            std::collections::BTreeSet::from(["unit:spy".to_string()]),
            "the live-only block must not suppress unrelated production"
        );

        // `sync` replaces temporary host-refusal cooldowns. Its permanent host-rule
        // block must survive that replacement and cover a city first seen this turn.
        state.turn = 9;
        state.cities.push(StateCity {
            id: 2,
            name: "Agra".to_string(),
            x: 6,
            y: 5,
            pop: 2,
            ..StateCity::default()
        });
        mirror.sync(&snapshot, &state, 0);
        for host_city in [1, 2] {
            let city = mirror.cid_of[&host_city];
            assert!(
                mirror.game.blocked_production[&city].contains("unit:spy"),
                "city {host_city} must retain the permanent host rule"
            );
            assert!(
                !mirror.game.can_produce(0, city, &spy),
                "city {host_city} must not offer an untrainable Spy after sync"
            );
        }
    }

    /// ★★★★★ Building aliases cross; a truly unknown building stays observable.
    /// ★★★★★ A building CIVVIS does not model must not take the decider down.
    ///
    /// `BUILDING_CASTLE` **panicked the whole decider** on live run
    /// `civvis-20260801T012454Z` at turn 238:
    ///
    /// ```text
    /// panicked at src/specmap.rs: no ruleset entry named "castle"
    ///   Game::building_district_is_active -> Game::spawn_unit
    ///     -> mirror::rebuild_from_state -> LiveMirror::new
    /// ```
    ///
    /// The city's buildings were lowercased rather than translated, so the Firaxis
    /// internal name `castle` entered the list and `rules.buildings[..]` panicked.
    /// Castle is not unmodelled: it is CIVVIS's `medieval_walls`. Dropping it prevents
    /// the crash but also removes a real building and gives the city the wrong state.
    ///
    /// ⚠ The assertion is that the rebuild SURVIVES and SAYS SO. A silent drop would
    /// also stop the panic and would be the wrong fix — the name has to be counted.
    #[test]
    fn building_aliases_cross_and_unknown_buildings_are_reported_not_fatal() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 8,
            width: 20,
            height: 20,
            chunk: 1,
            plots: vec![plot(5, 5, "TERRAIN_GRASS"), plot(5, 6, "TERRAIN_GRASS")],
        }]);
        let mut state = StateSnapshot {
            turn: 8,
            ..StateSnapshot::default()
        };
        state.cities.push(StateCity {
            id: 1,
            name: "London".to_string(),
            x: 5,
            y: 5,
            pop: 6,
            buildings: vec![
                "BUILDING_MONUMENT".to_string(),
                // This exact building name is also a unique prefix of
                // UNIVERSITY_OF_SANKORE in the wonder table.
                "BUILDING_UNIVERSITY".to_string(),
                "BUILDING_CASTLE".to_string(),
                "BUILDING_STAR_FORT".to_string(),
                // Deliberately absent from both rule sets.
                "BUILDING_CIVVIS_MIRROR_SENTINEL".to_string(),
            ],
            ..StateCity::default()
        });

        // Before the fix this line panicked rather than returning.
        let recon = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);

        let city = recon
            .game
            .cities
            .values()
            .find(|c| c.owner == 0)
            .expect("the seat's city must be on the board");
        assert!(
            city.buildings.contains(&Name::new("monument")),
            "a building CIVVIS does model still crosses"
        );
        assert!(city.buildings.contains(&Name::new("university")));
        assert!(
            city.buildings.contains(&Name::new("medieval_walls")),
            "Firaxis's BUILDING_CASTLE is CIVVIS's medieval walls"
        );
        assert!(
            city.buildings.contains(&Name::new("renaissance_walls")),
            "Firaxis's BUILDING_STAR_FORT is CIVVIS's Renaissance walls"
        );
        assert!(
            recon
                .unmapped
                .iter()
                .any(|entry| entry.contains("BUILDING_CIVVIS_MIRROR_SENTINEL")),
            "and it must be COUNTED, not silently dropped: {:?}",
            recon.unmapped
        );
        assert!(
            !recon.unmapped.iter().any(|entry| entry.contains("BUILDING_CASTLE")
                || entry.contains("BUILDING_STAR_FORT")
                || entry.contains("BUILDING_UNIVERSITY")),
            "known buildings and aliases must not be reported as fidelity gaps: {:?}",
            recon.unmapped
        );

        // Incremental state sync has its own city update path and must make the
        // same cross-table decision.
        let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
        state.turn += 1;
        mirror.sync(&snapshot, &state, 0);
        assert!(
            !mirror.unmapped.iter().any(|entry| entry.contains("BUILDING_UNIVERSITY")),
            "sync must not reclassify an ordinary University as a wonder: {:?}",
            mirror.unmapped
        );
    }

    /// ⚠ A mirrored city's buildings are the EXPORT's statement, and `place_city`
    /// disagrees for a founding-bonus civilization: Rome's Trajan's Column pushes
    /// a free monument on every placement, while Civilization VI grants it at
    /// founding only. Run `civvis-20260807T172510Z` (#1366): two cities Rome
    /// CAPTURED, whose export building lists were empty, mirrored with
    /// `extra=['monument']` — ghost culture in exactly the captured cities the
    /// recovery planner was re-valuing. Founded cities masked the seed because
    /// their real monument is exported and the translation deduplicates.
    #[test]
    fn a_captured_city_does_not_inherit_the_seats_founding_bonus_monument() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 160,
            width: 20,
            height: 20,
            chunk: 1,
            plots: vec![plot(5, 5, "TERRAIN_GRASS"), plot(9, 5, "TERRAIN_GRASS")],
        }]);
        let mut state = StateSnapshot { turn: 160, ..StateSnapshot::default() };
        state.seat.civ = "CIVILIZATION_ROME".to_string();
        state.cities.push(StateCity {
            id: 1,
            name: "Rome".to_string(),
            x: 5,
            y: 5,
            pop: 9,
            capital: true,
            buildings: vec![
                "BUILDING_MONUMENT".to_string(),
                "BUILDING_GRANARY".to_string(),
            ],
            ..StateCity::default()
        });
        // Captured this game: Civ 6 reports its building list as empty.
        state.cities.push(StateCity {
            id: 2,
            name: "Karkar".to_string(),
            x: 9,
            y: 5,
            pop: 4,
            ..StateCity::default()
        });

        let recon = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
        let karkar = recon
            .game
            .cities
            .values()
            .find(|city| city.name == "Karkar")
            .expect("the captured city must be on the board");
        assert!(
            karkar.buildings.is_empty(),
            "the export lists no buildings; the mirror must not model a monument: {:?}",
            karkar.buildings
        );
        let rome = recon
            .game
            .cities
            .values()
            .find(|city| city.name == "Rome")
            .expect("the capital must be on the board");
        assert_eq!(
            rome.buildings
                .iter()
                .filter(|building| **building == Name::new("monument"))
                .count(),
            1,
            "the founded capital's real, exported monument still crosses exactly once"
        );
    }

    #[test]
    fn a_completed_wonder_keeps_its_type_and_plot() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 40, width: 8, height: 8, chunk: 1,
            plots: vec![plot(3, 3, "TERRAIN_GRASS"), plot(4, 3, "TERRAIN_GRASS")],
        }]);
        let mut state = StateSnapshot { turn: 40, ..StateSnapshot::default() };
        state.cities.push(StateCity {
            id: 1, name: "Memphis".to_string(), x: 3, y: 3, pop: 7,
            // Firaxis reports wonders through HasBuilding as well as the exact
            // plot record. It must not be classified as an unknown building.
            buildings: vec!["BUILDING_PYRAMIDS".to_string()],
            wonders: vec![StateWonder {
                kind: "BUILDING_PYRAMIDS".to_string(), x: 4, y: 3,
            }],
            ..StateCity::default()
        });
        let recon = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
        let city = recon.game.cities.values().find(|city| city.owner == 0).unwrap();
        let city_id = city.id;
        assert_eq!(
            city.wonders.get(&Name::new("pyramids")),
            Some(&crate::hex::offset_to_axial(4, 3))
        );
        let wonder_pos = crate::hex::offset_to_axial(4, 3);
        assert_eq!(
            recon.game.map.tiles[&wonder_pos].wonder.as_deref(),
            Some("pyramids"),
            "the tile representation must agree with the city's wonder map"
        );
        assert!(recon.game.valid_improvements(0, wonder_pos).is_empty(),
            "a Builder must not target a completed wonder");
        let mut bare = recon.game.clone();
        let tile = bare.map.tiles.get_mut(&wonder_pos).unwrap();
        tile.wonder = None;
        tile.owner_city = Some(city_id);
        assert!(!bare.valid_improvements(0, wonder_pos).is_empty(),
            "the fixture must otherwise be improvable, or the rejection proves nothing");
        assert!(
            !recon.unmapped.iter().any(|entry| entry.contains("PYRAMIDS")),
            "a modeled wonder is neither an unknown building nor missing its plot: {:?}",
            recon.unmapped
        );
    }

    /// ★★★★ A district the host will not place must stop being chosen — IN THAT CITY.
    ///
    /// `DISTRICT_GOVERNMENT` was refused **24** times by turn 115 on live run
    /// `civvis-20260801T024428Z`, and `build_no_plot` fired **39** times, every one the
    /// same district. A Government Plaza is one per civilization, so once it exists
    /// Civilization VI offers no plot anywhere, and CIVVIS re-chose it from the same
    /// board turn after turn. Each discard leaves the city with nothing queued and the
    /// hand-written ladder picks instead.
    ///
    /// ⚠⚠ The second assertion is the one that matters. The host refuses a district
    /// for two opposite reasons — impossible anywhere, or no room in THIS city — and a
    /// global block would stop CIVVIS building Campuses across the empire the first
    /// time one city ran out of space. That would trade a small waste for a large one.
    #[test]
    fn a_district_the_host_will_not_place_is_blocked_in_that_city_only() {
        let mut game = crate::game::Game::new(4, 20, 20, 7, 500, 0);
        let mut ours: Vec<u32> = game
            .cities
            .values()
            .filter(|c| c.owner == 0)
            .map(|c| c.id)
            .collect();
        while ours.len() < 2 {
            // A one-city fixture cannot show the scoping, which is the whole point.
            let seed = ours.len() as i32;
            let pos = (seed * 5 + 6, seed * 5 + 6);
            if !game.map.tiles.contains_key(&pos) {
                break;
            }
            game.place_city(0, pos, None);
            ours = game
                .cities
                .values()
                .filter(|c| c.owner == 0)
                .map(|c| c.id)
                .collect();
        }
        assert!(ours.len() >= 2, "need two cities to prove the block is scoped");
        let (blocked_city, other_city) = (ours[0], ours[1]);
        // A fresh city has one population and no research, so it can site nothing at
        // all — the fixture, not the change, is what would fail. Unlock everything and
        // grow both cities so the question under test is the block and only the block.
        let techs: Vec<Name> = game.rules.techs.keys().map(|t| Name::new(t.as_str())).collect();
        for tech in techs {
            game.players[0].techs.insert(tech);
        }
        let civics: Vec<Name> = game.rules.civics.keys().map(|c| Name::new(c.as_str())).collect();
        for civic in civics {
            game.players[0].civics.insert(civic);
        }
        for cid in [blocked_city, other_city] {
            if let Some(city) = game.cities.get_mut(&cid) {
                city.pop = 12;
            }
        }

        // ⚠ DISCOVERED, not hardcoded. Which districts a fresh city can site depends on
        // population and tech, so naming one made the precondition fail on an
        // unremarkable fixture rather than on anything to do with this change.
        let district = game
            .rules
            .districts
            .keys()
            .map(|name| crate::name::Name::new(name.as_str()))
            .find(|name| {
                !game.district_sites(blocked_city, name).is_empty()
                    && !game.district_sites(other_city, name).is_empty()
            })
            .expect("some district must be sitable in both cities for this to prove anything");

        game.blocked_districts
            .entry(blocked_city)
            .or_default()
            .insert(district);

        assert!(
            game.district_sites(blocked_city, district).is_empty(),
            "the city the host refused must stop offering it"
        );
        assert!(
            !game.district_sites(other_city, district).is_empty(),
            "and every OTHER city must be untouched — a global block would cost far \
             more than the waste it prevents"
        );
    }

    /// A zero-target answer is stronger than a city-local site disagreement. The
    /// host cannot see a location for this world unique anywhere, so every city must
    /// stop valuing it — including through the prerequisite-reach query.
    #[test]
    fn a_world_unique_the_host_cannot_place_is_blocked_in_every_city() {
        let mut game = crate::game::Game::new(4, 20, 20, 71, 500, 0);
        let mut ours: Vec<u32> = game
            .cities
            .values()
            .filter(|city| city.owner == 0)
            .map(|city| city.id)
            .collect();
        while ours.len() < 2 {
            let seed = ours.len() as i32;
            let pos = (seed * 5 + 6, seed * 5 + 6);
            if !game.map.tiles.contains_key(&pos) {
                break;
            }
            game.place_city(0, pos, None);
            ours = game
                .cities
                .values()
                .filter(|city| city.owner == 0)
                .map(|city| city.id)
                .collect();
        }
        assert!(ours.len() >= 2, "need two cities to prove the world scope");
        let (first_city, second_city) = (ours[0], ours[1]);
        game.players[0].techs = game.rules.techs.keys().copied().collect();
        game.players[0].civics = game.rules.civics.keys().copied().collect();
        for city in [first_city, second_city] {
            game.cities.get_mut(&city).unwrap().pop = 12;
        }
        let wonder = game
            .rules
            .wonders
            .keys()
            .copied()
            .find(|wonder| {
                !game.wonder_sites(first_city, wonder).is_empty()
                    && !game.wonder_sites(second_city, wonder).is_empty()
            })
            .expect("some wonder must be sitable in both cities for this to prove anything");

        game.host_unavailable_wonders.insert(wonder);

        for city in [first_city, second_city] {
            assert!(
                game.wonder_sites(city, wonder.as_str()).is_empty(),
                "the host's zero-target response must block {wonder:?} in city {city}"
            );
        }
    }

    /// A positive host answer must beat the temporary block emitted beside it.
    /// Otherwise the bridge learns the legal coordinates and still leaves the
    /// district unavailable for all eight cooldown turns.
    #[test]
    fn a_host_approved_district_site_reopens_the_same_city() {
        let mut game = crate::game::Game::new(4, 20, 20, 71, 500, 0);
        assert!(game.map.tiles.contains_key(&(6, 6)), "fixture city site exists");
        let city = game.place_city(0, (6, 6), None);
        game.players[0].techs = game.rules.techs.keys().copied().collect();
        game.players[0].civics = game.rules.civics.keys().copied().collect();
        game.cities.get_mut(&city).unwrap().pop = 12;
        let mut candidate = None;
        for district in game.rules.districts.keys().copied() {
            for site in game.district_sites(city, district) {
                let item = crate::game::Item::District {
                    district,
                    pos: site,
                };
                if game.can_produce(0, city, &item) {
                    candidate = Some((district, site));
                    break;
                }
            }
            if candidate.is_some() {
                break;
            }
        }
        let (district, site) =
            candidate.expect("an unlocked grown city needs a buildable district");

        game.blocked_districts.entry(city).or_default().insert(district);
        assert!(
            game.district_sites(city, district).is_empty(),
            "precondition: the paired refusal blocks the normal model"
        );
        game.host_district_sites
            .entry(city)
            .or_default()
            .entry(district)
            .or_default()
            .insert(site);

        assert_eq!(
            game.district_sites(city, district),
            vec![site],
            "the host-approved tile must be the sole fresh candidate"
        );
        assert!(
            game.can_produce(
                0,
                city,
                &crate::game::Item::District {
                    district,
                    pos: site,
                }
            ),
            "the approved coordinate has to reach the production gate, not merely a field"
        );
    }

    /// A positive wonder placement response is the escape hatch from its paired
    /// temporary refusal, just as it is for districts. This uses Pyramids because
    /// its flat desert rule is easy to make explicit in a tiny mirrored board.
    #[test]
    fn a_host_approved_wonder_site_reopens_the_same_city() {
        let mut game = crate::game::Game::new(4, 20, 20, 71, 500, 0);
        assert!(game.map.tiles.contains_key(&(6, 6)), "fixture city site exists");
        let city = game.place_city(0, (6, 6), None);
        let site = (7, 6);
        assert!(game.map.tiles.contains_key(&site), "fixture wonder site exists");
        game.players[0].techs = game.rules.techs.keys().copied().collect();
        game.players[0].civics = game.rules.civics.keys().copied().collect();
        {
            let tile = game.map.tiles.get_mut(&site).unwrap();
            tile.terrain = crate::name!("desert");
            tile.hills = false;
            tile.feature = None;
            tile.resource = None;
            tile.improvement = None;
            tile.district = None;
            tile.district_foundation = None;
            tile.wonder = None;
            tile.owner_city = Some(city);
        }
        let owned_tiles = &mut game.cities.get_mut(&city).unwrap().owned_tiles;
        if !owned_tiles.contains(&site) {
            owned_tiles.push(site);
        }
        let wonder = crate::name!("pyramids");
        let item = crate::game::Item::Wonder { wonder, pos: site };
        assert!(
            game.can_produce(0, city, &item),
            "precondition: the configured Pyramids site is buildable"
        );

        game.blocked_wonders.entry(city).or_default().insert(wonder);
        assert!(
            game.wonder_sites(city, &wonder).is_empty(),
            "precondition: the paired refusal blocks the normal model"
        );
        game.host_wonder_sites
            .entry(city)
            .or_default()
            .entry(wonder)
            .or_default()
            .insert(site);

        assert_eq!(
            game.wonder_sites(city, &wonder),
            vec![site],
            "the host-approved tile must be the sole fresh candidate"
        );
        assert!(
            game.can_produce(0, city, &item),
            "the approved coordinate has to reach the production gate, not merely a field"
        );
    }

    /// ★★★★★ A MIRRORED CAPITAL MUST NOT BE PAID FOR ITS PALACE TWICE.
    ///
    /// CIVVIS models the palace positionally — `city_has_palace` derives it from
    /// capital status, and four separate sites add its yields, housing, amenity and
    /// great-work slots off that predicate. Nothing native ever pushes "palace" into
    /// a `buildings` list. Civilization VI exports `BUILDING_PALACE`, the translation
    /// put it in the list, and every one of those four sites then paid twice.
    ///
    /// Measured on run `civvis-20260802T014139Z`, turn 3 — one city, pop 1, palace
    /// only. Civ 6 reported **2.5** science and the reconstruction reported **5.0**:
    /// palace 2 twice, plus 0.5 for the citizen. With the seat re-dealt to Rome (a
    /// civ carrying no invented per-city yield) the same replay reads
    /// `science 2.5/2.5 +0%` afterwards, against `2.5/5.0 +98%` before.
    ///
    /// ⚠ **THIS TEST PINS THE MECHANISM, NOT THE NUMBER, AND THAT IS A COMPROMISE
    /// WORTH KNOWING ABOUT.** The number is pinned by the replay above, on real
    /// exported data, which is the stronger evidence of the two.
    ///
    /// It cannot be pinned here because a game built by `rebuild_from_state` in a
    /// unit test yields **nothing at all**: `city_yields` on this fixture's capital
    /// reads science 0, production 0 and *food 0* — through a hard `.max(2.0)` floor
    /// on the city-centre tile, so the value is impossible and the body plainly never
    /// runs. `city_yields_weighted`, which is documented as never being on the cached
    /// path, reads 0 as well, so it is not the query memo.
    ///
    /// RESOLVED (yield-fidelity work, 2026-08-16): the body runs; the LAST line
    /// zeroes it. `StateCity::default()` is `loyalty: 0.0` — the serde default
    /// `unknown_strength` (-1) applies only when deserializing — the mirror copies
    /// any non-negative loyalty onto the city, and `loyalty_yield_mult(0.0)` is the
    /// revolt band's **0**. A fixture that wants numbers says `loyalty: 100.0`
    /// (see `host_plot_yields_become_tile_corrections_and_the_model_stays_readable`);
    /// a real export always carries loyalty, so live boards were never affected.
    ///
    /// ⚠⚠ That silently weakens the sibling test below: it asserts only that the
    /// drift string carries **Civ 6's** number and a `%`, never CIVVIS's own, so it
    /// passes just as happily on a reconstruction yielding zero. Both halves of
    /// that comparison are assertable once the fixture carries loyalty.
    #[test]
    fn a_mirrored_capital_is_not_paid_for_its_palace_twice() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 3,
            width: 20,
            height: 20,
            chunk: 1,
            plots: vec![plot(5, 5, "TERRAIN_GRASS"), plot(5, 6, "TERRAIN_GRASS")],
        }]);
        let mut state = StateSnapshot {
            turn: 3,
            ..StateSnapshot::default()
        };
        state.cities.push(StateCity {
            id: 1,
            name: "Lisbon".to_string(),
            x: 5,
            y: 5,
            pop: 1,
            // Both halves matter: Civ 6 marks the seat's capital AND exports the
            // palace inside it, and it is the pair that used to pay twice.
            capital: true,
            buildings: vec!["BUILDING_PALACE".to_string()],
            ..StateCity::default()
        });
        let recon = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
        let city = recon
            .game
            .cities
            .values()
            .find(|city| city.owner == 0)
            .expect("the exported city must be on the board");

        assert!(
            !city.buildings.iter().any(|b| b.as_str() == "palace"),
            "the palace is positional in CIVVIS; listing it is what pays it twice"
        );
        assert!(
            !city.buildings.iter().any(|b| b.as_str() == "palace"),
            "the palace is positional in CIVVIS; listing it is what paid it twice"
        );
        assert!(
            recon.game.city_has_palace(city),
            "and it must still be paid ONCE — city_has_palace is the payer, and it \
             is true for exactly the city Civ 6 exported the palace in"
        );
    }

    /// ★★★★★ AND IT MUST NAME THE PART THAT IS NOT A DEFECT.
    ///
    /// CIVVIS's civilization abilities are not Civilization VI's — `data/civs.json`
    /// gives Arabia "House of Wisdom: +1 science and +1 faith in every city" where
    /// the real ability grants no flat per-city yield. A mirrored seat therefore
    /// runs hot by exactly `effect x cities`, and on run civvis-20260802T064240Z
    /// that was the ENTIRE residual: science +18% median, culture -0%.
    ///
    /// Without attribution that 18% gets re-investigated every time somebody reads
    /// it. With it, a reader separates the known offset from a new defect at a
    /// glance.
    ///
    /// ⚠ Asserted in BOTH directions. A civ with no flat effect must not grow the
    /// clause at all — a line that always fires says nothing.
    #[test]
    fn the_drift_attributes_the_civ_ability_it_knows_about() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 8,
            width: 20,
            height: 20,
            chunk: 1,
            plots: vec![plot(5, 5, "TERRAIN_GRASS"), plot(5, 6, "TERRAIN_GRASS")],
        }]);
        let mut state = StateSnapshot {
            turn: 8,
            science: 5.0,
            culture: 3.0,
            ..StateSnapshot::default()
        };
        state.cities.push(StateCity {
            id: 1,
            name: "Mecca".to_string(),
            x: 5,
            y: 5,
            pop: 4,
            capital: true,
            ..StateCity::default()
        });
        let mut recon = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);

        recon.game.players[0].civ = "Arabia".to_string();
        let arabia = economy_drift(&recon.game, &state).expect("yields present");
        assert!(
            arabia.contains("of which civ ability Arabia"),
            "the known offset must be named: {arabia}"
        );
        assert!(
            arabia.contains("science +1.0"),
            "and quantified at its real size — city_science 1 over one city: {arabia}"
        );

        // ⚠ Rome carries no flat per-city science or culture, so the clause must be
        // absent entirely rather than reading "+0.0".
        recon.game.players[0].civ = "Rome".to_string();
        let rome = economy_drift(&recon.game, &state).expect("yields present");
        assert!(
            !rome.contains("of which civ ability"),
            "a civ with no flat effect must not grow the clause: {rome}"
        );
    }

    /// ★★★★ The reconstruction's economic error must be a NUMBER, not a shrug.
    ///
    /// Measured live on `civvis-20260801T024428Z`: `economy civ6/civvis science
    /// 5.8/9.2 +59% culture 7.1/9.4 +33%`. Research valuations are spent in these
    /// units, so a rate half again too fast makes an unaffordable plan look
    /// affordable — and until this line existed nothing said so.
    #[test]
    fn the_economic_drift_is_reported_and_an_old_export_reads_as_unknown() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 8,
            width: 20,
            height: 20,
            chunk: 1,
            plots: vec![plot(5, 5, "TERRAIN_GRASS"), plot(5, 6, "TERRAIN_GRASS")],
        }]);
        let mut state = StateSnapshot {
            turn: 8,
            ..StateSnapshot::default()
        };
        state.cities.push(StateCity {
            id: 1,
            name: "Washington".to_string(),
            x: 5,
            y: 5,
            pop: 4,
            ..StateCity::default()
        });
        let recon = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);

        // ⚠ An export with no yields must read as UNKNOWN, never as agreement. An
        // older mod that reports nothing would otherwise look like a perfect match,
        // which is the failure mode this bridge specialises in.
        assert!(
            economy_drift(&recon.game, &state).is_none(),
            "no yields exported means no claim about drift"
        );

        state.science = 5.8;
        state.culture = 7.1;
        let drift = economy_drift(&recon.game, &state).expect("yields present");
        assert!(
            drift.contains("science 5.8/"),
            "the game's own number leads, so the comparison is readable: {drift}"
        );
        assert!(
            drift.contains('%'),
            "and the gap is expressed as a percentage: {drift}"        );

        // ⚠⚠ PRODUCTION was exported by #845 and never deserialized, so it could not
        // appear here at all. It is the yield that decides what every city builds,
        // and since #867 CIVVIS chooses that for every city every turn.
        assert!(
            !drift.contains("production"),
            "a city reporting no production figure must stay silent, not claim a \
             100% drift: {drift}"
        );
        state.cities[0].production = 12.0;
        let drift = economy_drift(&recon.game, &state).expect("yields present");
        assert!(
            drift.contains("production 12.0/"),
            "the game's own production leads, as science and culture do: {drift}"
        );
    }

    /// ★★★★★ A barbarian that appears AFTER the mirror is built must reach the board.
    ///
    /// `LiveMirror::sync` had **no reference to `state.hostiles` or `barb_pid` at
    /// all**, so barbarians were whatever the construction rebuild found and nothing
    /// after. At turn 1 that is normally none — so the decider played entire games
    /// against an empty barbarian seat while the export named them every turn.
    ///
    /// Measured on live run `civvis-20260801T040700Z`: Montréal founded turn 26, gone
    /// by turn 42, loyalty 100 throughout and at war with nobody it had met — so
    /// neither revolt nor a rival took it, and `hostiles` was non-empty in the export.
    /// ⚠⚠ `gold_per_turn` gates the whole bankruptcy response and the bridge
    /// never wrote it, so `economic_recovery` was unreachable in every real
    /// game. A treasury pinned at zero is the case that matters: Civilization VI
    /// clamps the balance there and disbands units to pay, so differencing alone
    /// reports a healthy zero exactly when the empire is broke.
    #[test]
    fn an_empty_treasury_reports_insolvency_rather_than_a_flat_balance() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 1,
            width: 20,
            height: 20,
            chunk: 1,
            plots: vec![plot(5, 5, "TERRAIN_GRASS")],
        }]);
        let mut state = StateSnapshot {
            turn: 1,
            gold: 120,
            ..StateSnapshot::default()
        };
        let mut mirror = LiveMirror::new(&snapshot, &state, 1, 1, 500, 0);

        // The first sample cannot be differenced against anything.
        state.turn = 2;
        state.gold = 108;
        mirror.sync(&snapshot, &state, 0);
        assert_eq!(
            mirror.game.players[0].gold_per_turn, -12.0,
            "a falling treasury is negative net income"
        );

        state.turn = 3;
        state.gold = 130;
        mirror.sync(&snapshot, &state, 0);
        assert_eq!(mirror.game.players[0].gold_per_turn, 22.0);

        // The defect: broke, and staying broke. The delta is zero and the old
        // reading would have called that solvent.
        state.turn = 4;
        state.gold = 0;
        mirror.sync(&snapshot, &state, 0);
        assert!(mirror.game.players[0].gold_per_turn < -0.5);
        state.turn = 5;
        state.gold = 0;
        mirror.sync(&snapshot, &state, 0);
        assert!(
            mirror.game.players[0].gold_per_turn < -0.5,
            "a treasury pinned at zero is insolvency, not thrift — this is the \
             reading that makes economic_recovery reachable at all"
        );

        // A gap of unknown length is not a rate. Leave the last reading alone
        // rather than inventing one across a resync.
        state.turn = 40;
        state.gold = 400;
        mirror.sync(&snapshot, &state, 0);
        assert!(mirror.game.players[0].gold_per_turn < -0.5);
    }

    /// A seat that cannot see barbarians cannot garrison against them.
    #[test]
    fn a_barbarian_that_appears_after_construction_reaches_the_board() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 4,
            width: 20,
            height: 20,
            chunk: 1,
            plots: vec![
                plot(5, 5, "TERRAIN_GRASS"),
                plot(5, 6, "TERRAIN_GRASS"),
                plot(6, 6, "TERRAIN_GRASS"),
            ],
        }]);
        let mut state = StateSnapshot {
            turn: 4,
            ..StateSnapshot::default()
        };
        state.cities.push(StateCity {
            id: 1,
            name: "Ottawa".to_string(),
            x: 5,
            y: 5,
            pop: 3,
            ..StateCity::default()
        });

        // Turn 4: no barbarian in sight, which is the ordinary opening.
        let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
        let barb = mirror.game.barb_pid.expect("a mirrored roster has a barbarian seat");
        assert_eq!(
            mirror.game.units.values().filter(|u| u.owner == barb).count(),
            0,
            "precondition: the board starts with no barbarians"
        );

        // Turn 8: one walks into view. Before the fix nothing here looked at it.
        state.turn = 8;
        state.hostiles.push(StateUnit {
            kind: "UNIT_WARRIOR".to_string(),
            x: 5,
            y: 6,
            hp: 35.0,
            fortified: true,
            fortify_turns: 1,
            ..StateUnit::default()
        });
        mirror.sync(&snapshot, &state, 0);

        assert_eq!(
            mirror.game.units.values().filter(|u| u.owner == barb).count(),
            1,
            "a barbarian the export named must be on the board — this is the whole \
             defect, and before the fix it stayed invisible for the rest of the game"
        );
        let hostile = mirror.game.units.values().find(|unit| unit.owner == barb).unwrap();
        assert_eq!(hostile.hp, 35, "a visible hostile's damage is useful combat state");
        assert!(hostile.fortified);
        assert_eq!(hostile.fortify_turns, 1);

        // And it must leave again when it dies or moves out of sight, or the board
        // accumulates ghosts that never attack anything.
        state.hostiles.clear();
        state.turn = 12;
        mirror.sync(&snapshot, &state, 0);
        assert_eq!(
            mirror.game.units.values().filter(|u| u.owner == barb).count(),
            0,
            "and one the export no longer names must go, or the threat list only grows"
        );
    }

    /// ⚠⚠ A GAP LIST THAT REPORTS A FIELD THE MIRROR DOES READ IS A BROKEN
    /// INSTRUMENT, and this project navigates by `unmapped`.
    ///
    /// `state_schema_gaps` keeps its own names beside `StateCity`/`StateUnit` and
    /// nothing kept the two in step. #877 added `production`, `production_cost` and
    /// `production_turns` to the struct and the decider went on printing
    /// `unmapped: schema:city.production,...` every turn while reading them
    /// perfectly well. `class` had been doing it for longer.
    ///
    /// A superset is fine — serde aliases mean `kind` also answers to `type`, and
    /// only the export side needs both. What must never happen again is a struct
    /// field with no entry.
    #[test]
    fn the_schema_allowlists_cover_every_declared_field() {
        for (struct_name, allowed) in [
            ("StateCity", CITY_KEYS),
            ("StateUnit", UNIT_KEYS),
            ("StatePublicEmpireStats", PUBLIC_STATS_KEYS),
        ] {
            let declared = declared_fields(struct_name);
            assert!(
                !declared.is_empty(),
                "{struct_name} parsed to no fields — the extractor broke, not the list"
            );
            let missing: Vec<&String> = declared
                .iter()
                .filter(|field| !allowed.contains(&field.as_str()))
                .collect();
            assert!(
                missing.is_empty(),
                "{struct_name} declares {missing:?}, which state_schema_gaps would \
                 report as unmapped even though the mirror reads them"
            );
        }
    }

    /// ★★★★★ `DISTRICT_GOVERNMENT` is CIVVIS's `government_plaza`, and missing that
    /// cost two separate bugs.
    ///
    /// Prefix-stripping gives `government`, which is in no table, so `civvis_node_name`
    /// returned None and both callers did the honest thing with a wrong answer:
    /// `civvis_production_item` read a city building one as IDLE (60 repeat orders in
    /// one measured run), and #729's blocked-districts reader dropped the name, so the
    /// block never engaged for the one district it was built for —
    /// `no_params_DISTRICT_GOVERNMENT` still read **9** after it shipped.
    #[test]
    fn a_civ6_name_that_truncates_a_civvis_one_resolves_only_when_unambiguous() {
        let rules = crate::rules::Rules::embedded();
        assert_eq!(
            civvis_node_name(&rules.districts, "DISTRICT_GOVERNMENT", "DISTRICT_").as_deref(),
            Some("government_plaza"),
            "the truncated Civilization VI name must reach CIVVIS's fuller one"
        );
        // The ordinary case must not regress: an exact name still wins outright.
        assert_eq!(
            civvis_node_name(&rules.districts, "DISTRICT_CAMPUS", "DISTRICT_").as_deref(),
            Some("campus")
        );
        // ⚠ And a stem that is not a whole word must NOT match. `dam` is a real
        // district; without the boundary check it would swallow anything starting
        // "dam...".
        assert!(
            civvis_node_name(&rules.districts, "DISTRICT_DAM", "DISTRICT_").as_deref()
                == Some("dam"),
            "an exact short name resolves to itself, not to a longer neighbour"
        );
        // A name CIVVIS genuinely does not have still answers None rather than
        // guessing at the nearest thing.
        assert!(
            civvis_node_name(&rules.districts, "DISTRICT_NOT_A_REAL_ONE", "DISTRICT_").is_none(),
            "an unknown district must not resolve to something plausible"
        );
    }

    /// ★★★★★ A district Civilization VI has built must be ON the reconstructed city.
    ///
    /// `StateDistrict` was defined, carried on `StateCity`, handed to
    /// `civvis_production_item` to locate a production plot, and used in tests —
    /// and never written onto a city. `grep '\.districts\.insert' src/mirror.rs`
    /// found nothing. So every Campus, Holy Site and Commercial Hub the real game had
    /// built was invisible, and the city read as bare ground: the same shape as the
    /// improvements gap, where a mirror showing an undeveloped empire made CIVVIS
    /// re-order what it already had.
    #[test]
    fn the_districts_a_city_has_built_reach_the_board() {
        let historical: StateDistrict = serde_json::from_value(serde_json::json!({
            "type": "DISTRICT_CAMPUS", "x": 5, "y": 6, "pillaged": false
        })).unwrap();
        assert!(historical.complete,
            "pre-completion-bit event streams keep their historical completed semantics");
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 30,
            width: 20,
            height: 20,
            chunk: 1,
            plots: vec![
                plot(5, 5, "TERRAIN_GRASS"),
                plot(5, 6, "TERRAIN_GRASS"),
                plot(6, 6, "TERRAIN_GRASS"),
            ],
        }]);
        let mut state = StateSnapshot {
            turn: 30,
            ..StateSnapshot::default()
        };
        state.cities.push(StateCity {
            id: 1,
            name: "Canberra".to_string(),
            x: 5,
            y: 5,
            pop: 8,
            districts: vec![
                // The centre is implicit in CIVVIS and must NOT be inserted.
                StateDistrict {
                    kind: "DISTRICT_CITY_CENTER".to_string(),
                    x: 5,
                    y: 5,
                    pillaged: false,
                    complete: true,
                    ..StateDistrict::default()
                },
                StateDistrict {
                    kind: "DISTRICT_CAMPUS".to_string(),
                    x: 5,
                    y: 6,
                    pillaged: true,
                    complete: true,
                    ..StateDistrict::default()
                },
            ],
            ..StateCity::default()
        });

        let recon = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
        let city = recon
            .game
            .cities
            .values()
            .find(|c| c.owner == 0)
            .expect("the seat's city must be on the board");
        let city_id = city.id;

        assert_eq!(
            city.districts.get(Name::new("campus")).copied(),
            Some(crate::hex::offset_to_axial(5, 6)),
            "a built district must reach the board, on the plot the export named"
        );
        // ⚠ `found_city_for` gives a native CIVVIS city `Districts::default()`, so the
        // centre is implicit. Inserting it would put a district on the board that
        // CIVVIS's own games never have — checked in the source, not assumed.
        assert!(
            !city.districts.contains_key(Name::new("city_center")),
            "the city centre stays implicit, as it is in an ordinary CIVVIS game"
        );

        let campus = crate::hex::offset_to_axial(5, 6);
        let campus_tile = &recon.game.map.tiles[&campus];
        assert_eq!(campus_tile.district.as_deref(), Some("campus"));
        assert!(campus_tile.pillaged, "district pillage state must reach its tile");
        assert!(recon.game.valid_improvements(0, campus).is_empty(),
            "a completed district must never be offered to a Builder");
        let mut bare = recon.game.clone();
        let tile = bare.map.tiles.get_mut(&campus).unwrap();
        tile.district = None;
        tile.pillaged = false;
        tile.owner_city = Some(city_id);
        assert!(!bare.valid_improvements(0, campus).is_empty(),
            "the fixture must otherwise be improvable, or the rejection proves nothing");

        // Incremental sync must preserve the distinction between a placed
        // foundation and a completed district.
        let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
        state.turn += 1;
        state.cities[0].producing = Some("DISTRICT_HOLY_SITE".to_string());
        state.cities[0].districts.push(StateDistrict {
            kind: "DISTRICT_HOLY_SITE".to_string(),
            x: 6,
            y: 6,
            pillaged: false,
            complete: false,
            ..StateDistrict::default()
        });
        mirror.sync(&snapshot, &state, 0);
        let holy_site = crate::hex::offset_to_axial(6, 6);
        let tile = &mirror.game.map.tiles[&holy_site];
        assert!(tile.district.is_none());
        assert_eq!(tile.district_foundation.as_ref()
            .map(|foundation| foundation.district.as_str()), Some("holy_site"));
        assert!(!mirror.game.cities[&mirror.cid_of[&1]].districts
            .contains_key(Name::new("holy_site")));
        assert!(mirror.game.valid_improvements(0, holy_site).is_empty());

        state.turn += 1;
        state.cities[0].producing = None;
        state.cities[0].districts[2].complete = true;
        mirror.sync(&snapshot, &state, 0);
        let tile = &mirror.game.map.tiles[&holy_site];
        assert_eq!(tile.district.as_deref(), Some("holy_site"));
        assert!(tile.district_foundation.is_none());
        assert!(mirror.game.cities[&mirror.cid_of[&1]].districts
            .contains_key(Name::new("holy_site")));

        // An omitted fog/public roster is unknown, not evidence that permanent
        // infrastructure vanished.
        state.turn += 1;
        state.cities[0].districts.clear();
        mirror.sync(&snapshot, &state, 0);
        assert_eq!(mirror.game.map.tiles[&campus].district.as_deref(), Some("campus"));
        assert_eq!(mirror.game.map.tiles[&holy_site].district.as_deref(), Some("holy_site"));

        state.turn += 1;
        state.cities.clear();
        mirror.sync(&snapshot, &state, 0);
        assert!(mirror.game.map.tiles[&campus].district.is_none());
        assert!(mirror.game.map.tiles[&holy_site].district.is_none());
    }

    /// ★★★★★ A walled city Civilization VI reports as UNDAMAGED must not read as razed.
    ///
    /// `wall_hp` was never written, so it kept its 0 default while `city_max_wall_hp`
    /// summed the walls the city had — and CIVVIS's gate is `wall_hp < max`. Every
    /// walled city therefore looked destroyed forever. Replaying run
    /// `civvis-20260801T065721Z` showed **47 turns** wanting
    /// `PROJECT_REPAIR_OUTER_DEFENSES` while the exported defence was RISING.
    #[test]
    fn a_walled_city_reported_undamaged_is_not_read_as_razed() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 30, width: 8, height: 8, chunk: 1,
            plots: vec![plot(3, 3, "TERRAIN_GRASS")],
        }]);
        let build = |wall_damage: f64| {
            let mut state = StateSnapshot { turn: 30, ..StateSnapshot::default() };
            state.cities.push(StateCity {
                id: 1, name: "Rome".to_string(), x: 3, y: 3, pop: 6,
                buildings: vec!["BUILDING_WALLS".to_string()],
                damage: 0.0,
                max_damage: 200.0,
                wall_damage,
                max_wall_damage: 100.0,
                ..StateCity::default()
            });
            let recon = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
            let city = recon.game.cities.values().find(|c| c.owner == 0)
                .expect("the seat's city must be on the board").clone();
            let max = recon.game.city_max_wall_hp(&city);
            (city.wall_hp, max)
        };

        let (hp, max) = build(0.0);
        // ⚠ The precondition. With no walls modelled `max` is 0 and `wall_hp < max`
        // is false for any hp, so the test would pass for the wrong reason.
        assert!(max > 0, "the fixture city must actually have walls, or this proves nothing");
        assert_eq!(hp, max, "an undamaged walled city must read at FULL wall hp");

        let (hurt, max2) = build(20.0);
        assert_eq!(hurt, max2 - 20, "reported damage must come off the wall hp");
        assert!(hurt < max2, "a damaged city must still be repairable");

        // Damage beyond the wall total must floor at zero, not go negative:
        // `damage` is a `try` read in Lua and cannot be trusted to be in range.
        let (floored, _) = build(9_999.0);
        assert_eq!(floored, 0, "wall hp must clamp at zero");
    }

    #[test]
    fn city_health_is_refreshed_on_every_live_sync() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 30, width: 8, height: 8, chunk: 1,
            plots: vec![plot(3, 3, "TERRAIN_GRASS")],
        }]);
        let mut state = StateSnapshot { turn: 30, ..StateSnapshot::default() };
        state.cities.push(StateCity {
            id: 1, name: "Rome".to_string(), x: 3, y: 3, pop: 6,
            buildings: vec!["BUILDING_WALLS".to_string()],
            damage: 0.0, max_damage: 200.0,
            wall_damage: 0.0, max_wall_damage: 100.0,
            ..StateCity::default()
        });
        let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
        let cid = mirror.cid_of[&1];
        assert_eq!(mirror.game.cities[&cid].hp, 200);
        assert_eq!(mirror.game.cities[&cid].wall_hp, 100);

        state.turn += 1;
        state.cities[0].damage = 50.0;
        state.cities[0].wall_damage = 40.0;
        mirror.sync(&snapshot, &state, 0);
        assert_eq!(mirror.game.cities[&cid].hp, 150);
        assert_eq!(mirror.game.cities[&cid].wall_hp, 60);
        assert_eq!(mirror.game.city_max_wall_hp(&mirror.game.cities[&cid]), 100);
    }

    #[test]
    fn city_capture_reconciles_both_rosters_and_ownership() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 20, width: 10, height: 10, chunk: 1,
            plots: vec![
                plot(3, 3, "TERRAIN_GRASS"),
                plot(4, 3, "TERRAIN_GRASS"),
                plot(6, 3, "TERRAIN_GRASS"),
            ],
        }]);
        let city = |id, name: &str, x| StateCity {
            id, name: name.to_string(), x, y: 3, pop: 5, ..StateCity::default()
        };
        let mut state = StateSnapshot { turn: 20, ..StateSnapshot::default() };
        state.cities.push(city(10, "Home", 3));
        state.cities[0].districts.push(StateDistrict {
            kind: "DISTRICT_CAMPUS".to_string(),
            x: 4,
            y: 3,
            pillaged: false,
            complete: true,
            ..StateDistrict::default()
        });
        state.rivals.push(StateRival {
            player: 3, civ: "CIVILIZATION_ROME".to_string(),
            cities: vec![city(20, "Rome", 6)], ..StateRival::default()
        });
        let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);

        state.turn += 1;
        state.cities = vec![city(20, "Rome", 6)];
        state.rivals[0].cities = vec![city(10, "Home", 3)];
        mirror.sync(&snapshot, &state, 0);

        let ours = mirror.game.city_at(crate::hex::offset_to_axial(6, 3)).unwrap();
        let theirs = mirror.game.city_at(crate::hex::offset_to_axial(3, 3)).unwrap();
        assert_eq!(mirror.game.cities[&ours].owner, 0);
        assert_eq!(mirror.game.cities[&theirs].owner, 1);
        assert_eq!(mirror.cid_of.get(&20), Some(&ours));
        assert!(!mirror.cid_of.contains_key(&10));
        assert_eq!(mirror.game.player_city_ids(0), vec![ours]);
        let campus = crate::hex::offset_to_axial(4, 3);
        assert_eq!(mirror.game.map.tiles[&campus].district.as_deref(), Some("campus"));
        assert!(mirror.game.cities[&theirs].districts.contains_key(Name::new("campus")),
            "a public rival record omits infrastructure; capture must preserve what was known");
    }

    #[test]
    fn a_razed_own_city_does_not_survive_in_the_mirror() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 20, width: 8, height: 8, chunk: 1,
            plots: vec![plot(3, 3, "TERRAIN_GRASS")],
        }]);
        let mut state = StateSnapshot { turn: 20, ..StateSnapshot::default() };
        state.cities.push(StateCity {
            id: 10, name: "Home".to_string(), x: 3, y: 3, pop: 5,
            ..StateCity::default()
        });
        let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
        assert_eq!(mirror.game.player_city_ids(0).len(), 1);

        state.turn += 1;
        state.cities.clear();
        mirror.sync(&snapshot, &state, 0);
        assert!(mirror.game.player_city_ids(0).is_empty());
        assert!(mirror.game.city_at(crate::hex::offset_to_axial(3, 3)).is_none());
    }

    /// ★★★★ A rival's unique unit must reach the board as what it REPLACES.
    ///
    /// `UNIT_NORWEGIAN_LONGSHIP` was dropped on every turn it was visible on live run
    /// `civvis-20260801T145302Z` — CIVVIS models no Norwegian uniques — so an enemy
    /// warship was not on the board at all. A Longship replaces a Galley, which
    /// CIVVIS does model.
    #[test]
    fn a_rivals_unique_unit_lands_as_what_it_replaces() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 12, width: 8, height: 8, chunk: 1,
            plots: vec![plot(2, 2, "TERRAIN_OCEAN"), plot(4, 4, "TERRAIN_GRASS")],
        }]);
        let build = |kind: &str, base: Option<&str>| {
            let mut state = StateSnapshot { turn: 12, ..StateSnapshot::default() };
            state.units.push(StateUnit {
                id: 7,
                kind: kind.to_string(),
                base: base.map(|b| b.to_string()),
                x: 2, y: 2, hp: 100.0, ..StateUnit::default()
            });
            rebuild_from_state(&snapshot, &state, 4, 1, 500, 0)
        };

        // ⚠ Precondition: the unique must genuinely be untranslatable, or this test
        // passes for the wrong reason.
        let bare = build("UNIT_NORWEGIAN_LONGSHIP", None);
        assert!(
            bare.game.units.is_empty(),
            "the fixture must be a unit CIVVIS cannot name, or the fallback is untested"
        );

        let with_base = build("UNIT_NORWEGIAN_LONGSHIP", Some("UNIT_GALLEY"));
        let unit = with_base.game.units.values().next()
            .expect("a unique with a known base must reach the board");
        assert_eq!(unit.kind.as_str(), "galley", "it lands as what it replaces");
        // ⚠ And it must SAY it approximated. A collapsed distinction that nobody can
        // see is the failure the mapping rule names.
        assert!(
            with_base.dropped_units.iter().any(|d| d.contains("approximated_as_galley")),
            "the approximation must be reported, not silent: {:?}", with_base.dropped_units
        );

        // A base CIVVIS also cannot name must still not invent a unit.
        let nonsense = build("UNIT_NORWEGIAN_LONGSHIP", Some("UNIT_NOT_A_REAL_UNIT"));
        assert!(nonsense.game.units.is_empty(), "an unknown base must not be guessed at");
    }

    /// ★★★★★ A STANDALONE unique — no `UnitReplaces` row — must land by its class.
    ///
    /// Run `civvis-20260801T175955Z` was lost with two `UNIT_MAPUCHE_MALON_RAIDER`
    /// two tiles from the final city, dropped as untranslatable: the conquering
    /// army was not on CIVVIS's board at all. `base` cannot save it (there is no
    /// base); `class` must.
    #[test]
    fn a_standalone_unique_lands_by_its_promotion_class() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 12, width: 8, height: 8, chunk: 1,
            plots: vec![plot(2, 2, "TERRAIN_GRASS"), plot(4, 4, "TERRAIN_GRASS")],
        }]);
        let build = |kind: &str, class: Option<&str>| {
            let mut state = StateSnapshot { turn: 12, ..StateSnapshot::default() };
            state.units.push(StateUnit {
                id: 9,
                kind: kind.to_string(),
                class: class.map(|c| c.to_string()),
                x: 2, y: 2, hp: 100.0, ..StateUnit::default()
            });
            rebuild_from_state(&snapshot, &state, 4, 1, 500, 0)
        };

        // ⚠ Precondition: with neither base nor class the unit must genuinely drop,
        // or the fallback under test is not what put it on the board.
        let bare = build("UNIT_MAPUCHE_MALON_RAIDER", None);
        assert!(
            bare.game.units.is_empty(),
            "the fixture must be a unit CIVVIS cannot name, or the fallback is untested"
        );

        let classed = build("UNIT_MAPUCHE_MALON_RAIDER", Some("PROMOTION_CLASS_LIGHT_CAVALRY"));
        let unit = classed.game.units.values().next()
            .expect("a standalone unique with a known class must reach the board");
        assert_eq!(unit.kind.as_str(), "horseman", "it lands as the class representative");
        assert!(
            classed.dropped_units.iter()
                .any(|d| d.contains("approximated_as_horseman_from_light_cavalry")),
            "the approximation must be reported, not silent: {:?}", classed.dropped_units
        );

        // A class CIVVIS has no representative for must still not invent a unit.
        let nonsense = build("UNIT_MAPUCHE_MALON_RAIDER", Some("PROMOTION_CLASS_NOT_REAL"));
        assert!(nonsense.game.units.is_empty(), "an unknown class must not be guessed at");

        // RANGED_CAVALRY was missing from the first fallback table. Preserve a
        // representative for an otherwise-unmodelled standalone unique.
        let ranged_unique = build(
            "UNIT_EXAMPLE_RANGED_RIDER",
            Some("PROMOTION_CLASS_RANGED_CAVALRY"),
        );
        let unit = ranged_unique.game.units.values().next()
            .expect("a ranged-cavalry unique must reach the board");
        assert_eq!(unit.kind.as_str(), "saka_horse_archer");

        // Keshig is now modelled exactly; an exact name must outrank the class
        // approximation so its distinct strength and upgrade path survive.
        let keshig = build("UNIT_MONGOLIAN_KESHIG", Some("PROMOTION_CLASS_RANGED_CAVALRY"));
        let unit = keshig.game.units.values().next()
            .expect("a modelled Keshig must reach the board");
        assert_eq!(unit.kind.as_str(), "keshig");

        // ⚠ And a REPLACING unique keeps preferring its base: class must only be
        // the rung below `base`, or a Longship would land as a generic hull even
        // when the ruleset models what it replaces.
        let mut state = StateSnapshot { turn: 12, ..StateSnapshot::default() };
        state.units.push(StateUnit {
            id: 10,
            kind: "UNIT_NORWEGIAN_LONGSHIP".to_string(),
            base: Some("UNIT_GALLEY".to_string()),
            class: Some("PROMOTION_CLASS_NAVAL_MELEE".to_string()),
            x: 2, y: 2, hp: 100.0, ..StateUnit::default()
        });
        let both = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
        let unit = both.game.units.values().next().expect("the base rung must still fire");
        assert_eq!(unit.kind.as_str(), "galley", "base outranks class");
    }

    /// ★★★★★ The game speed Civilization VI is running must reach the board.
    ///
    /// The ladder plays `GAMESPEED_ONLINE`, whose costs are HALF of Standard, and a
    /// mirrored game kept `GameSpeed::Standard` because nothing read the field —
    /// so every tech, civic, district and unit cost CIVVIS reasoned about was
    /// double what the game would charge, on every turn of every run.
    #[test]
    fn the_game_speed_civ6_is_running_reaches_the_board() {
        assert_eq!(
            civvis_game_speed("GAMESPEED_ONLINE"),
            Some(crate::setup::GameSpeed::Online),
            "the export's GameSpeedType must map onto CIVVIS's own speed"
        );
        // ⚠ The two must actually DIFFER in cost, or this fix is decoration.
        assert_ne!(
            crate::setup::GameSpeed::Online.scale(100.0),
            crate::setup::GameSpeed::Standard.scale(100.0),
            "Online and Standard must price differently for this to matter"
        );
        assert_eq!(
            civvis_game_speed("GAMESPEED_NOT_A_SPEED"), None,
            "an unknown speed must leave the default alone, not guess"
        );

        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 30, width: 8, height: 8, chunk: 1,
            plots: vec![plot(3, 3, "TERRAIN_GRASS")],
        }]);
        let mut state = StateSnapshot { turn: 30, ..StateSnapshot::default() };
        state.seat.speed = "GAMESPEED_ONLINE".to_string();
        let recon = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
        assert_eq!(
            recon.game.game_speed,
            crate::setup::GameSpeed::Online,
            "a reconstruction must run at the speed Civilization VI reported"
        );
    }

    /// ★★★★★ A city Civilization VI says is AT its district cap must stop offering
    /// district sites on the board.
    ///
    /// Run `civvis-20260801T065721Z` (Rome, 195 turns, defeat) discarded **157**
    /// district requests through `build_no_plot`, and **157 of 157** were made while
    /// the city was at or over Civilization VI's population cap:
    ///
    /// ```text
    /// city      pop  specialty  cap=ceil(pop/3)   requests
    /// Ravenna     4          3                2        79
    /// Gao         5          3                2        23
    /// Ostia       8          4                3        19
    /// ```
    ///
    /// CIVVIS models the cap correctly and always has — `Game::district_sites`
    /// computes `1 + (pop - 1) / 3`, the same 1/4/7 ladder Civilization VI uses. So
    /// the rule is not the defect; the only way CIVVIS can ask anyway is if the
    /// MIRRORED city carries the wrong population or is missing the districts it has
    /// already built. This test pins both through the reconstruction rather than
    /// asserting the rule in isolation, which `Game`'s own tests already do.
    ///
    /// ⚠ Two-sided on purpose. "No sites" passes trivially when the city owns no
    /// workable ground, so the under-cap case must FIRST prove a site is offered.
    #[test]
    fn a_city_at_its_civ6_district_cap_offers_no_more_sites() {
        // A city needs workable ground before `district_sites` can offer anything.
        // ⚠ Ownership is not decoration here. A mirrored city works only the ground
        // the export says it owns, so plots left at `o: -1` give it none and
        // `district_sites` is empty for every district regardless of the cap.
        let plots: Vec<_> = (0..12)
            .flat_map(|y| (0..12).map(move |x| (x, y)))
            .map(|(x, y)| {
                let mut p = plot(x, y, "TERRAIN_GRASS");
                if (x - 5).abs() <= 3 && (y - 5).abs() <= 3 {
                    p.o = 0;
                }
                p
            })
            .collect();
        let build = |districts: Vec<&str>, pop: i32| {
            let snapshot = Snapshot::from_chunks(&[TilesChunk {
                turn: 30,
                width: 12,
                height: 12,
                chunk: 1,
                plots: plots.clone(),
            }]);
            let mut state = StateSnapshot {
                turn: 30,
                ..StateSnapshot::default()
            };
            state.cities.push(StateCity {
                id: 1,
                name: "Ravenna".to_string(),
                x: 5,
                y: 5,
                pop,
                districts: districts
                    .iter()
                    .enumerate()
                    .map(|(i, kind)| StateDistrict {
                        kind: (*kind).to_string(),
                        x: 4 + i as i32 % 3,
                        y: 4,
                        pillaged: false,
                        complete: true,
                        ..StateDistrict::default()
                    })
                    .collect(),
                ..StateCity::default()
            });
            let mut recon = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
            // Districts are tech- and civic-gated, and a reconstruction starts with
            // neither, so without this the precondition fails on unlocks rather than
            // on anything to do with the cap.
            let techs: Vec<Name> = recon
                .game
                .rules
                .techs
                .keys()
                .map(|t| Name::new(t.as_str()))
                .collect();
            let civics: Vec<Name> = recon
                .game
                .rules
                .civics
                .keys()
                .map(|c| Name::new(c.as_str()))
                .collect();
            for tech in techs {
                recon.game.players[0].techs.insert(tech);
            }
            for civic in civics {
                recon.game.players[0].civics.insert(civic);
            }
            recon
        };

        // Ravenna's real shape: population 4, so the cap is 2.
        let under = build(vec!["DISTRICT_CITY_CENTER"], 4);
        let (&cid, city) = under
            .game
            .cities
            .iter()
            .find(|(_, c)| c.owner == 0)
            .expect("the seat's city must be on the board");
        assert_eq!(
            city.pop, 4,
            "the mirrored city must carry the population Civilization VI reported"
        );

        // ⚠ DISCOVERED, not hardcoded — placement rules differ per district, so
        // naming one risks failing on siting rather than on the cap.
        let probe = under
            .game
            .rules
            .districts
            .iter()
            .filter(|(_, spec)| spec.specialty)
            .map(|(name, _)| Name::new(name.as_str()))
            .find(|name| !under.game.district_sites(cid, name).is_empty())
            .expect("a pop-4 city under its cap must be able to site SOME specialty district");

        // Same city, same population, but three specialty districts already built.
        let at_cap = build(
            vec![
                "DISTRICT_CITY_CENTER",
                "DISTRICT_CAMPUS",
                "DISTRICT_HOLY_SITE",
                "DISTRICT_INDUSTRIAL_ZONE",
            ],
            4,
        );
        let (&capped, city) = at_cap
            .game
            .cities
            .iter()
            .find(|(_, c)| c.owner == 0)
            .expect("the seat's city must be on the board");
        let built = city
            .districts
            .keys()
            .filter(|name| at_cap.game.rules.districts[*name].specialty)
            .count();
        assert_eq!(
            built, 3,
            "every specialty district Civilization VI has built must reach the board — \
             a city that reads as bare ground is exactly how CIVVIS asked 79 times"
        );
        assert!(
            at_cap.game.district_sites(capped, probe).is_empty(),
            "population 4 allows 1 + (4-1)/3 = 2 specialty districts and this city has 3, \
             so CIVVIS must stop choosing {probe}"
        );
    }

    /// ★★★★★ An enemy city under fog must stay on the board — the SAME defect as the
    /// tile memory, one field over, and I only fixed the tiles.
    ///
    /// Measured on live run `civvis-20260801T045406Z` at turn 198, at war and losing:
    /// 7 enemy cities in the export, all revealed, on land and unoccupied; **7 placed
    /// on the reconstruction** (`follow.log`: "7 rival cities"); and **1** visible in
    /// the seated observation. `grep -c remembered_cities src/mirror.rs` answered 0.
    ///
    /// ⚠ `findWarTarget` needs a revealed rival city, and "no enemy city is ever
    /// revealed … domination is arithmetically impossible" is a standing note in this
    /// project. The cities were on the board the whole time; the seat could not
    /// remember them.
    ///
    /// ⚠ Asserted through `observation_player_view`, never against
    /// `remembered_cities`: a test that counted the memory map would pass on a memory
    /// the viewer never consults — exactly the trap the tile-memory test had to avoid.
    #[test]
    fn an_enemy_city_under_fog_stays_on_the_board() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 30,
            width: 20,
            height: 20,
            chunk: 1,
            plots: (4..9)
                .flat_map(|x| (4..9).map(move |y| plot(x, y, "TERRAIN_GRASS")))
                .collect(),
        }]);
        let mut state = StateSnapshot {
            turn: 30,
            ..StateSnapshot::default()
        };
        state.cities.push(StateCity {
            id: 1,
            name: "Canberra".to_string(),
            x: 4,
            y: 4,
            pop: 4,
            ..StateCity::default()
        });
        state.rivals.push(StateRival {
            player: 3,
            at_war: true,
            cities: vec![StateCity {
                id: 2,
                name: "Berlin".to_string(),
                x: 8,
                y: 8,
                pop: 6,
                ..StateCity::default()
            }],
            ..StateRival::default()
        });

        let recon = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
        let enemy = recon
            .game
            .cities
            .values()
            .find(|c| c.owner != 0)
            .expect("the rival city must be planted on the board");
        let enemy_pos = enemy.pos;

        // The seat has no unit near Berlin, so it is fogged — precisely the case that
        // used to erase it.
        let visible = recon.game.player_visibility(0);
        assert!(
            !visible.contains(&enemy_pos),
            "the enemy city must genuinely be under fog for this to mean anything"
        );

        let view = crate::obs::observation_player_view(&recon.game, 0);
        let cities = view["cities"].as_array().expect("a city list");
        let names: Vec<&str> = cities
            .iter()
            .filter_map(|c| c["name"].as_str())
            .collect();
        assert!(
            names.contains(&"Berlin"),
            "a fogged enemy city the seat has seen must still be on the board — this \
             is what made domination unreachable: {names:?}"
        );
    }

    /// ★★★★ A trader cannot be walked in Civilization VI, and CIVVIS kept trying.
    ///
    /// CIVVIS's ruleset gives `trader` 2 moves; Civ 6 gives it
    /// `AiType="UNITTYPE_TRADE"` and reports `moves: 0` on every export. Granting it
    /// full ruleset movement made CIVVIS plan steps the host refuses every time:
    /// measured with the `move_refused` instrument on run
    /// `civvis-20260801T065721Z`, ONE trader produced **22 of 33** move refusals by
    /// turn 70, shuffling between four tiles for 38 turns.
    /// ★★★★ A SPY CANNOT BE WALKED EITHER, and unlike the trader the export gives
    /// it real movement points — so the ruleset value cannot be trusted here.
    ///
    /// Measured over every run recorded on 2026-08-03: **893 of 1,197 refused
    /// adjacent moves (75%) were `UNIT_SPY`**, all on our own territory, with
    /// single spies stuck and re-ordered for 43 to 81 consecutive turns.
    #[test]
    fn a_spy_is_given_no_movement_even_though_civ6_reports_some() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 20,
            width: 20,
            height: 20,
            chunk: 1,
            plots: vec![plot(5, 5, "TERRAIN_GRASS"), plot(5, 6, "TERRAIN_GRASS")],
        }]);
        let mut state = StateSnapshot {
            turn: 20,
            ..StateSnapshot::default()
        };
        state.cities.push(StateCity {
            id: 1,
            name: "Canberra".to_string(),
            x: 5,
            y: 5,
            pop: 4,
            ..StateCity::default()
        });
        // ⚠ The precondition that makes this test worth having: Civilization VI
        // exports a spy WITH movement (1, 2 and 3 were all observed), so nothing
        // in the export tells the bridge this unit cannot walk.
        state.units.push(StateUnit {
            id: 5439532,
            kind: "UNIT_SPY".to_string(),
            x: 5,
            y: 6,
            moves: 2.0,
            ..StateUnit::default()
        });
        state.units.push(StateUnit {
            id: 5439533,
            kind: "UNIT_WARRIOR".to_string(),
            x: 5,
            y: 5,
            ..StateUnit::default()
        });

        let mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
        let moves_of = |board: &LiveMirror, want: &str| -> Option<f64> {
            board
                .game
                .units
                .values()
                .find(|u| u.kind.as_str() == want)
                .map(|u| u.moves_left)
        };
        assert_eq!(
            moves_of(&mirror, "spy"),
            Some(0.0),
            "a spy must be given no walking movement — Civilization VI refuses every \
             MOVE_TO for one however many movement points it reports"
        );
        assert!(
            moves_of(&mirror, "warrior").is_some_and(|m| m > 0.0),
            "every other unit keeps its ruleset movement"
        );
    }

    #[test]
    fn an_embarked_unit_keeps_dynamic_fresh_turn_movement() {
        let mut plots = (3..=9)
            .flat_map(|x| (3..=9).map(move |y| plot(x, y, "TERRAIN_GRASS")))
            .collect::<Vec<_>>();
        plots
            .iter_mut()
            .find(|site| site.x == 6 && site.y == 5)
            .expect("the embarked unit's plot is in the fixture")
            .t = Some("TERRAIN_COAST".to_string());
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 20,
            width: 12,
            height: 12,
            chunk: 1,
            plots,
        }]);
        let mut state = StateSnapshot {
            turn: 20,
            techs: vec![
                "TECH_SAILING".to_string(),
                "TECH_SHIPBUILDING".to_string(),
                "TECH_CARTOGRAPHY".to_string(),
                "TECH_SQUARE_RIGGING".to_string(),
                "TECH_STEAM_POWER".to_string(),
            ],
            ..StateSnapshot::default()
        };
        state.cities.push(StateCity {
            id: 1,
            name: "Canberra".to_string(),
            x: 5,
            y: 5,
            pop: 4,
            capital: true,
            ..StateCity::default()
        });
        state.units.push(StateUnit {
            id: 42,
            kind: "UNIT_SETTLER".to_string(),
            x: 6,
            y: 5,
            moves: 0.0,
            ..StateUnit::default()
        });

        let mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
        let uid = *mirror.uid_of.get(&42).expect("the Settler is mirrored");
        let unit = &mirror.game.units[&uid];
        let static_moves = mirror.game.rules.units["settler"].moves;
        let dynamic_moves = mirror.game.unit_max_moves(uid);
        assert!(
            dynamic_moves > static_moves,
            "the test needs a real embarked movement bonus"
        );
        assert_eq!(
            unit.moves_left, dynamic_moves,
            "fresh-turn mirror movement must include dynamic embarked bonuses"
        );
        let land_step = mirror
            .game
            .nbrs(unit.pos)
            .into_iter()
            .find(|pos| {
                mirror
                    .game
                    .map
                    .get(*pos)
                    .is_some_and(|tile| !mirror.game.rules.is_water(tile))
            })
            .expect("the coast has a revealed land neighbor");
        assert!(
            mirror.game.can_move(uid, land_step),
            "the dynamic allowance must pay the first disembark step"
        );
    }

    /// The seat's strategic stockpiles reach the board: a Bombard needs Niter,
    /// a Trebuchet is obsolete once a Bombard can be built. The won game
    /// civvis-20260816T054344Z ordered a Trebuchet the host refused on 29 turns
    /// because the board had no Niter and no Bombard.
    #[test]
    fn the_seats_strategic_stockpiles_reach_the_board() {
        let plots = (3..=9)
            .flat_map(|x| (3..=9).map(move |y| plot(x, y, "TERRAIN_GRASS")))
            .collect::<Vec<_>>();
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 120,
            width: 12,
            height: 12,
            chunk: 1,
            plots,
        }]);
        let mut state = StateSnapshot {
            turn: 120,
            ..StateSnapshot::default()
        };
        state.cities.push(StateCity {
            id: 1,
            name: "Rome".to_string(),
            x: 5,
            y: 5,
            pop: 8,
            capital: true,
            ..StateCity::default()
        });
        state.strategic_resources = Some(BTreeMap::from([
            ("RESOURCE_NITER".to_string(), 40.0),
            ("RESOURCE_IRON".to_string(), 12.0),
            ("RESOURCE_UNOBTAINIUM".to_string(), 3.0),
        ]));
        let mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
        let game = &mirror.game;
        assert_eq!(game.strategic_stockpile(0, crate::name!("niter")), 40.0);
        assert_eq!(game.strategic_stockpile(0, crate::name!("iron")), 12.0);
        assert_eq!(game.strategic_stockpile(0, crate::name!("horses")), 0.0);
        assert!(
            mirror.unmapped.iter().any(|issue| issue == "strategic_resource:RESOURCE_UNOBTAINIUM"),
            "a resource the ruleset does not know is reported: {:?}",
            mirror.unmapped
        );

        // And nothing stocked reads as nothing, not as a deserialisation failure.
        state.strategic_resources = None;
        let empty = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
        assert_eq!(empty.game.strategic_stockpile(0, crate::name!("niter")), 0.0);
        let parsed: StateSnapshot = serde_json::from_str(
            r#"{"turn":5,"strategic_resources":[]}"#,
        )
        .expect("an empty stockpile list still parses");
        assert!(parsed.strategic_resources.is_none() || parsed.strategic_resources.as_ref().is_some_and(|m| m.is_empty()));
    }

    /// A Great Person is not a unit CIVVIS models, but the ground it stands on
    /// is occupied all the same. Run civvis-20260816T003229Z: the founded
    /// zero-charge Prophet stood beside the capital for 130 turns and a Builder
    /// was ordered onto its tile on 25 consecutive turns.
    #[test]
    fn a_great_persons_plot_is_occupied_ground_the_builder_routes_around() {
        let plots = (3..=9)
            .flat_map(|x| (3..=9).map(move |y| plot(x, y, "TERRAIN_GRASS")))
            .collect::<Vec<_>>();
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 80,
            width: 12,
            height: 12,
            chunk: 1,
            plots,
        }]);
        let mut state = StateSnapshot {
            turn: 80,
            ..StateSnapshot::default()
        };
        state.cities.push(StateCity {
            id: 1,
            name: "Rome".to_string(),
            x: 5,
            y: 5,
            pop: 4,
            capital: true,
            ..StateCity::default()
        });
        state.units.push(StateUnit {
            id: 7,
            kind: "UNIT_BUILDER".to_string(),
            x: 5,
            y: 5,
            moves: 2.0,
            build_charges: Some(2),
            ..StateUnit::default()
        });
        state.units.push(StateUnit {
            id: 9,
            kind: "UNIT_GREAT_PROPHET".to_string(),
            x: 6,
            y: 5,
            moves: 0.0,
            ..StateUnit::default()
        });
        state.rivals.push(StateRival {
            civ: "CIVILIZATION_SWEDEN".to_string(),
            units: vec![StateUnit {
                id: 11,
                kind: "UNIT_GREAT_GENERAL".to_string(),
                x: 8,
                y: 8,
                moves: 0.0,
                ..StateUnit::default()
            }],
            ..StateRival::default()
        });

        let mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
        let game = &mirror.game;
        let prophet_plot = crate::hex::offset_to_axial(6, 5);
        let general_plot = crate::hex::offset_to_axial(8, 8);
        assert!(
            !mirror.uid_of.contains_key(&9),
            "the Prophet is still not a unit on the board"
        );
        assert_eq!(
            game.great_person_plots.get(&prophet_plot),
            Some(&0),
            "but its plot is recorded as ground the seat's own Great Person holds"
        );
        assert!(
            game.great_person_plots
                .get(&general_plot)
                .is_some_and(|owner| *owner != 0),
            "and a rival's Great Person is recorded to its owner"
        );
        assert!(
            game.valid_improvements(0, prophet_plot).is_empty(),
            "the plot offers a Builder nothing, so it is never chosen as a target"
        );
        let uid = *mirror.uid_of.get(&7).expect("the Builder is mirrored");
        assert!(
            !game.can_move(uid, prophet_plot),
            "and the Builder cannot step onto it, as Firaxis will refuse the step"
        );
        let open = game
            .nbrs(game.units[&uid].pos)
            .into_iter()
            .find(|pos| *pos != prophet_plot && game.map.get(*pos).is_some())
            .expect("the capital has another neighbour");
        assert!(
            game.can_move(uid, open),
            "the neighbouring plots without a Great Person stay open"
        );
    }

    #[test]
    fn a_promotion_the_host_refused_is_not_offered_again() {
        let dir = std::env::temp_dir().join(format!("civvis_promo_{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        let path = dir.join("events.jsonl");
        std::fs::write(
            &path,
            concat!(
                r#"{"kind":"promotion_refused","unit":3342338,"promotion":"PROMOTION_TRANSLATOR","turn":40}"#,
                "\n",
                r#"{"kind":"promotion_refused","unit":3342338,"promotion":"PROMOTION_TRANSLATOR","turn":41}"#,
                "\n",
                r#"{"kind":"promotion_refused","unit":5111818,"promotion":"PROMOTION_ECHELON","turn":42}"#,
                "\n",
                r#"{"kind":"promotion_refused","unit":3342338,"promotion":"PROMOTION_CHAPLAIN","turn":90}"#,
                "\n",
            ),
        )
        .expect("write events");

        let refused = refused_promotions_through(&path, Some(50));
        assert_eq!(
            refused.get(&3342338).map(|names| names.len()),
            Some(1),
            "the turn limit keeps the turn-90 Chaplain refusal out"
        );
        assert!(
            refused[&3342338].contains("PROMOTION_TRANSLATOR"),
            "the refused promotion is recorded under its Civilization VI unit id"
        );
        assert!(refused[&5111818].contains("PROMOTION_ECHELON"));

        let later = refused_promotions_through(&path, Some(120));
        assert_eq!(
            later[&3342338].len(),
            2,
            "both distinct refusals are in hand once the game reaches turn 90"
        );

        let rules = crate::rules::Rules::embedded();
        let unit_ids: std::collections::BTreeMap<u32, i64> =
            [(7u32, 3342338i64), (9u32, 5111818i64)].into_iter().collect();
        let blocked = blocked_promotions_from(&later, &unit_ids, &rules);
        assert!(
            blocked[&7].contains(&crate::name::Name::new("translator")),
            "the host name PROMOTION_TRANSLATOR is TRANSLATED to the CIVVIS rule name, \
             not interned raw — `available_promotions` compares CIVVIS names"
        );
        assert!(blocked[&9].contains(&crate::name::Name::new("echelon")));
        assert!(
            !blocked.contains_key(&11),
            "a unit the host never refused carries no block"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_trader_is_given_no_movement_because_civ6_gives_it_none() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 20,
            width: 20,
            height: 20,
            chunk: 1,
            plots: vec![plot(5, 5, "TERRAIN_GRASS"), plot(5, 6, "TERRAIN_GRASS")],
        }]);
        let mut state = StateSnapshot {
            turn: 20,
            ..StateSnapshot::default()
        };
        state.cities.push(StateCity {
            id: 1,
            name: "Canberra".to_string(),
            x: 5,
            y: 5,
            pop: 4,
            ..StateCity::default()
        });
        state.units.push(StateUnit {
            id: 786439,
            kind: "UNIT_TRADER".to_string(),
            x: 5,
            y: 6,
            ..StateUnit::default()
        });
        state.units.push(StateUnit {
            id: 786440,
            kind: "UNIT_WARRIOR".to_string(),
            x: 5,
            y: 5,
            ..StateUnit::default()
        });

        let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);

        let moves_of = |board: &LiveMirror, want: &str| -> Option<f64> {
            board
                .game
                .units
                .values()
                .find(|u| u.kind.as_str() == want)
                .map(|u| u.moves_left)
        };
        assert_eq!(
            moves_of(&mirror, "trader"),
            Some(0.0),
            "a trader must be given no movement — Civilization VI reports moves: 0 for \
             it on every export, and every walk CIVVIS planned for one was refused"
        );
        // ⚠ And nothing else is grounded by this. A warrior keeps the movement the
        // ruleset gives it; the fix is about one unit class, not about movement.
        assert!(
            moves_of(&mirror, "warrior").is_some_and(|m| m > 0.0),
            "every other unit keeps its ruleset movement"
        );

        // `civvis_orders --serve --fresh-board` follows this exact construction
        // path and never calls `sync`, so the constructor must carry the rule.
        mirror.sync(&snapshot, &state, 0);
        assert_eq!(moves_of(&mirror, "trader"), Some(0.0));
    }

    #[test]
    fn active_trade_routes_follow_the_host_and_keep_the_visible_trader() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 20,
            width: 12,
            height: 12,
            chunk: 1,
            plots: (0..12)
                .flat_map(|x| (0..12).map(move |y| plot(x, y, "TERRAIN_GRASS")))
                .collect(),
        }]);
        let mut state = StateSnapshot {
            turn: 20,
            seat: Seat {
                city_states: 1,
                ..Seat::default()
            },
            civics: vec!["CIVIC_FOREIGN_TRADE".to_string()],
            cities: vec![
                StateCity {
                    id: 7,
                    name: "Roma".to_string(),
                    x: 5,
                    y: 5,
                    pop: 3,
                    capital: true,
                    loyalty: 100.0,
                    ..StateCity::default()
                },
                StateCity {
                    id: 8,
                    name: "Antium".to_string(),
                    x: 6,
                    y: 6,
                    pop: 3,
                    loyalty: 100.0,
                    ..StateCity::default()
                },
            ],
            units: vec![StateUnit {
                id: 42,
                kind: "UNIT_TRADER".to_string(),
                x: 6,
                y: 6,
                moves: 0.0,
                ..StateUnit::default()
            }],
            trade_routes: vec![StateTradeRoute {
                trader: 42,
                origin: 8,
                destination: 7,
                origin_x: 6,
                origin_y: 6,
                destination_x: 5,
                destination_y: 5,
                posts_own: Some(2),
                posts_foreign: Some(1),
                // The host's Trade Overview is authoritative here: a route
                // can earn from a destination district that this seat has not
                // revealed, which the model must not invent from the fog.
                yields: Some(crate::rules::Yields {
                    food: 2.0,
                    production: 3.0,
                    gold: 7.0,
                    science: 5.0,
                    culture: 11.0,
                    faith: 13.0,
                }),
                ..StateTradeRoute::default()
            }],
            // Firaxis allocates city ids per player. This city-state's first city
            // deliberately has the same id as our Antium.
            minors: vec![StateMinor {
                player: 6,
                civ: "CIVILIZATION_ZANZIBAR".to_string(),
                cities: vec![StateCity {
                    id: 8,
                    name: "Zanzibar".to_string(),
                    x: 9,
                    y: 9,
                    pop: 3,
                    ..StateCity::default()
                }],
                ..StateMinor::default()
            }],
            ..StateSnapshot::default()
        };

        let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        let trader = mirror.uid_of[&42];
        assert!(mirror.game.units.contains_key(&trader));
        assert_eq!(mirror.game.active_routes(0), 1);
        assert!(mirror.active_trade_route_traders.contains(&42));
        assert_eq!(mirror.game.routes[0].origin, mirror.cid_of[&8]);
        assert_eq!(mirror.game.routes[0].dest, mirror.cid_of[&7]);
        assert_eq!(
            mirror.game.cities[&mirror.game.routes[0].origin].owner, 0,
            "a colliding city-state id must not steal the route origin"
        );
        // The host's own path and its Trading Posts stand in for the model's
        // straight-line walk, and survive a save.
        let key = (mirror.game.routes[0].origin, mirror.game.routes[0].dest);
        assert_eq!(mirror.game.observed_route_posts.get(&key), Some(&(2, 1)));
        let host_route = crate::rules::Yields {
            food: 2.0,
            production: 3.0,
            gold: 7.0,
            science: 5.0,
            culture: 11.0,
            faith: 13.0,
        };
        assert_eq!(mirror.game.observed_route_yields.get(&key), Some(&host_route));
        // The host's total replaces the model's complete route calculation,
        // rather than being added to it — otherwise an unseen Campus earns
        // twice. Removing the route leaves exactly its six host values behind.
        let origin = mirror.game.routes[0].origin;
        let routed = mirror.game.city_yields(origin);
        let mut no_route = mirror.game.clone();
        no_route.routes.clear();
        let baseline = no_route.city_yields(origin);
        for (label, observed, got, base) in [
            ("food", host_route.food, routed.food, baseline.food),
            ("production", host_route.production, routed.production, baseline.production),
            ("gold", host_route.gold, routed.gold, baseline.gold),
            ("science", host_route.science, routed.science, baseline.science),
            ("culture", host_route.culture, routed.culture, baseline.culture),
            ("faith", host_route.faith, routed.faith, baseline.faith),
        ] {
            assert!(
                ((got - base) - observed).abs() < 1e-9,
                "the host's {label} replaces the route model: {base} + {observed} != {got}"
            );
        }
        let saved: crate::game::Game =
            serde_json::from_str(&serde_json::to_string(&mirror.game).unwrap()).unwrap();
        assert_eq!(saved.observed_route_posts.get(&key), Some(&(2, 1)));
        assert_eq!(saved.observed_route_yields.get(&key), Some(&host_route));

        // The next authoritative state is the only thing allowed to complete a
        // route.  A persistent mirror must stop counting it immediately once the
        // host reports it gone, rather than waiting for CIVVIS's guessed duration.
        state.turn = 21;
        state.trade_routes.clear();
        mirror.sync(&snapshot, &state, 0);
        assert_eq!(mirror.game.active_routes(0), 0);
        assert!(mirror.active_trade_route_traders.is_empty());
        assert!(mirror.game.units.contains_key(&trader));
        assert!(mirror.game.observed_route_posts.is_empty());
        assert!(mirror.game.observed_route_yields.is_empty());
    }

    #[test]
    fn a_city_carries_the_religion_it_follows_and_the_one_converting_it() {
        // ⚠ THIS FIELD EXISTED AND WAS NEVER FILLED. `religion` was null on all
        // 26,954 city records ever exported — the schema had it, the mod never sent
        // it, and nothing failed. So the test is not "does the struct have a field",
        // it is "does the export shape actually deserialize into one".
        let raw = r#"{
            "id": 7, "name": "Nidaros", "x": 12, "y": 9, "pop": 6,
            "buildings": ["BUILDING_MONUMENT"],
            "religion": "RELIGION_CATHOLICISM",
            "religion_next": "RELIGION_BUDDHISM",
            "religion_turns": 4
        }"#;
        let city: StateCity = serde_json::from_str(raw).expect("city shape parses");
        assert_eq!(city.religion.as_deref(), Some("RELIGION_CATHOLICISM"));
        // The level alone cannot distinguish a city holding steady from one about
        // to flip, which is the `loyalty` / `loyalty_per_turn` lesson again.
        assert_eq!(city.religion_next.as_deref(), Some("RELIGION_BUDDHISM"));
        assert_eq!(city.religion_turns, 4);

        // An unconverted city omits them rather than sending an index, and must
        // still parse — "could not ask" and "follows nothing" both read as None.
        let bare = r#"{"id": 8, "name": "Ålesund", "x": 1, "y": 2, "pop": 1}"#;
        let plain: StateCity = serde_json::from_str(bare).expect("bare city parses");
        assert!(plain.religion.is_none() && plain.religion_next.is_none());
        assert_eq!(plain.religion_turns, 0);
    }

    /// ★★★★ A border that grows after the mirror is built must still be learned.
    ///
    /// `apply_territory` ran only in `rebuild_from_state`, which a persistent mirror
    /// calls once — at construction. Every border that grew afterwards stayed unowned
    /// on CIVVIS's board for the rest of the game. Measured on live run
    /// `civvis-20260801T012454Z` at turn 43: **28 of 243** paired plots were owned in
    /// Civilization VI and unowned in CIVVIS, and **none** the other way.
    ///
    /// ⚠ Asserted through `valid_improvements`, not against `owner_city` directly,
    /// because that is where the cost lands: the function returns an empty list for a
    /// tile whose `owner_city` is None, so a builder on ground the seat really owns is
    /// offered nothing to build. A test that only compared the ownership field would
    /// pass on an ownership nothing consults.
    #[test]
    fn a_border_that_grows_after_construction_is_still_learned() {
        let founded = |x: i32, y: i32| StateCity {
            id: 1,
            name: "Nidaros".to_string(),
            x,
            y,
            pop: 4,
            ..StateCity::default()
        };
        // Turn 4: one plot revealed and owned, and the city that owns it.
        let owned = |x: i32, y: i32, owner: i32| {
            let mut p = plot(x, y, "TERRAIN_GRASS");
            p.o = owner;
            p
        };
        let first = Snapshot::from_chunks(&[TilesChunk {
            turn: 4,
            width: 20,
            height: 20,
            chunk: 1,
            plots: vec![owned(5, 5, 0), owned(5, 6, -1)],
        }]);
        let mut state = StateSnapshot {
            turn: 4,
            ..StateSnapshot::default()
        };
        state.cities.push(founded(5, 5));

        let mut mirror = LiveMirror::new(&first, &state, 4, 1, 500, 0);
        let grown = crate::hex::offset_to_axial(5, 6);
        assert!(
            mirror.game.map.get(grown).is_some_and(|t| t.owner_city.is_none()),
            "the plot starts unowned, which is what the export said on turn 4"
        );

        // Turn 8: the border has grown over it. Nothing else about the world changed.
        let later = Snapshot::from_chunks(&[TilesChunk {
            turn: 8,
            width: 20,
            height: 20,
            chunk: 1,
            plots: vec![owned(5, 5, 0), owned(5, 6, 0)],
        }]);
        state.turn = 8;
        mirror.sync(&later, &state, 0);

        assert!(
            mirror.game.map.get(grown).is_some_and(|t| t.owner_city.is_some()),
            "a border that grew after construction must be learned — this is the whole \
             defect, and before the fix it stayed unowned for the rest of the game"
        );
        // And the consequence that actually costs games: ground the seat owns must
        // offer a builder something to do on it.
        assert!(
            !mirror.game.valid_improvements(0, grown).is_empty(),
            "owned ground must offer improvements; an unowned tile offers none, which \
             is how a stale border silently stops an empire developing"
        );
    }

    /// ★★★★★ THE EXPORT IS THE HOST'S VISIBILITY ANSWER, AND WE RE-DERIVED IT.
    ///
    /// Civilization VI exports a rival's units only under CURRENT visibility, the
    /// bridge plants exactly those, and then `player_vision_now` recomputes what
    /// the seat can see from this engine's sight radii on a reconstructed map.
    /// Where the two disagree, an enemy the host is showing us is invisible to the
    /// agent deciding whether to shoot it — and `ForcePosture` only reaches
    /// `Engage` through `g.sees(..) && battlefront_unit_visible(..)`.
    ///
    /// ⚠⚠⚠ Measured on live run `civvis-20260803T191900Z` across the 49 turns of
    /// Kongo's war (t203-250), which cost Arpinum and Arretium and ended the game
    /// at 479 against the winner's 1214: an enemy was in the export on 49 of 49
    /// turns, our units stood adjacent to one on 95 unit-turns and within range 2
    /// on 197. **37 attacks were issued** -- 81% of the shots the host was showing
    /// the army were declined, and the force logged "still gathering" instead.
    ///
    /// ⚠ The second assertion is the one that keeps this honest. The inference
    /// "a foreign unit is on the board, so we must be able to see it" is sound
    /// ONLY for a mirrored board. Applied to an ordinary game it would hand every
    /// AI perfect vision of the world, so the set must stay empty there.
    #[test]
    fn a_rival_the_host_exported_is_visible_however_sight_is_derived() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 12,
            width: 20,
            height: 20,
            chunk: 1,
            plots: vec![plot(5, 5, "TERRAIN_GRASS"), plot(15, 15, "TERRAIN_GRASS")],
        }]);
        let mut state = StateSnapshot {
            turn: 12,
            ..StateSnapshot::default()
        };
        state.cities.push(StateCity {
            id: 1,
            name: "Roma".to_string(),
            x: 5,
            y: 5,
            pop: 4,
            ..StateCity::default()
        });
        state.units.push(StateUnit {
            id: 10,
            kind: "UNIT_WARRIOR".to_string(),
            x: 5,
            y: 5,
            hp: 100.0,
            ..StateUnit::default()
        });
        // Ten hexes away — far outside anything our own sight model reaches, and
        // in the export only because Civilization VI can see it.
        state.rivals.push(StateRival {
            player: 3,
            at_war: true,
            units: vec![StateUnit {
                id: 20,
                kind: "UNIT_WARRIOR".to_string(),
                x: 15,
                y: 15,
                hp: 100.0,
                ..StateUnit::default()
            }],
            ..StateRival::default()
        });

        let mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
        let enemy = crate::hex::offset_to_axial(15, 15);
        let empty = crate::hex::offset_to_axial(18, 18);

        assert!(
            mirror.game.units.values().any(|unit| unit.owner != 0 && unit.pos == enemy),
            "the exported rival must reach the board at all — the rest of this test \
             is about whether the agent is allowed to notice it"
        );
        assert!(
            mirror.game.player_can_see(0, enemy),
            "a unit Civilization VI exported is a unit Civilization VI is showing \
             us; before this fix the engine re-derived sight on a reconstructed map \
             and answered no, and the army declined 81% of its shots in a war it lost"
        );
        assert!(
            !mirror.game.player_can_see(0, empty),
            "and only that ground: far tiles with nothing exported on them must stay \
             dark, or the repair is just omniscience wearing a fix's clothes"
        );

        // ★ The invariant that keeps ordinary play honest. A native game holds the
        // FULL simulation, so foreign units on the board prove nothing about sight.
        let native = crate::game::Game::new(4, 20, 20, 7, 500, 0);
        assert!(
            native.host_observed.is_empty(),
            "an ordinary CIVVIS game must leave this set empty; reading the board the \
             mirrored way there would give every AI player perfect vision"
        );
    }

    /// ★★★★★ A RIVAL'S BORDER IS INVISIBLE PRECISELY BECAUSE IT IS IN THE WAY.
    ///
    /// `can_enter` reads `territory_owner_at`, which resolves a plot through
    /// `owner_city -> cities -> owner`. A rival whose cities this seat has never
    /// SEEN owns no city on the mirrored board, so their border resolves to `None`
    /// and reads as free ground — and we cannot see the city that would fix that,
    /// because the border is what stops us walking to it.
    ///
    /// ⚠⚠⚠ Measured on live run `civvis-20260803T191900Z` (Rome, SETTLER, small).
    /// Scout `196608` reached offset (12,24) on turn 42 and was ordered
    /// `MOVE_TO (11,24)` — one hex — on **74 separate turns** without ever moving.
    /// (11,24) is exported `o: 4`: Kongo's, with no war and no open borders.
    /// **81 of 670 `MOVE_TO` orders targeted foreign ground and all 81 were
    /// counted `applied`** — a blocked move is a silent no-op, not a refusal, so
    /// every turn read as healthy while the empire went blind. Exploration
    /// flatlined at 283 of 3404 tiles, no rival city was seen in 96 snapshots,
    /// `plan.target_city` stayed `None`, and forty turns of `strategy=conquest`
    /// with `war_legal=9` produced no war at all.
    ///
    /// ⚠ Asserted through `can_move`, not against `closed_borders` directly, for
    /// the same reason the border-growth test above asserts through
    /// `valid_improvements`: the field only matters where it is consulted, and a
    /// test on the field alone would pass on a set nothing reads.
    /// ★★★★ A met major's border plot whose city cannot be safely resolved is
    /// a city in the fog. Run civvis-20260826T030045Z founded Lugdunum five
    /// tiles from Germany's border; the only visible German city was ten tiles
    /// away, so the settle-site forecast passed it before the host reported
    /// −22 Loyalty a turn. `unseen_major_borders` names a rival with no city on
    /// the board, a plot beyond every known city ownership ring, and a fifth-ring
    /// plot whose reported owner could equally be a nearer unseen city. A minor's
    /// ground and a plot securely inside a known rival city's ring are not in it.
    #[test]
    fn a_met_majors_border_without_a_safe_city_attribution_is_recorded_as_unseen() {
        let owned = |x: i32, y: i32, owner: i32| {
            let mut p = plot(x, y, "TERRAIN_GRASS");
            p.o = owner;
            p
        };
        let mut plots = vec![owned(5, 5, 0)];
        // Rival 3 owns (5,7) and (5,8) and none of their cities is in sight.
        plots.push(owned(5, 7, 3));
        plots.push(owned(5, 8, 3));
        // Rival 4 owns (14, 5) beside their known city at (15, 5), (10, 5) on
        // the fifth ring of that city, and (5, 12) ten tiles from it. The
        // latter two could belong to a closer city we cannot see.
        plots.push(owned(15, 5, 4));
        plots.push(owned(14, 5, 4));
        plots.push(owned(10, 5, 4));
        plots.push(owned(5, 12, 4));
        // A city-state owns (9, 9); minors exert no loyalty pressure and are
        // not majors.
        plots.push(owned(9, 9, 7));
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 12,
            width: 20,
            height: 20,
            chunk: 1,
            plots,
        }]);
        let mut state = StateSnapshot {
            turn: 12,
            ..StateSnapshot::default()
        };
        state.cities.push(StateCity {
            id: 1,
            name: "Roma".to_string(),
            x: 5,
            y: 5,
            pop: 4,
            ..StateCity::default()
        });
        state.rivals.push(StateRival {
            player: 3,
            at_war: false,
            cities: Vec::new(),
            ..StateRival::default()
        });
        state.rivals.push(StateRival {
            player: 4,
            at_war: false,
            cities: vec![StateCity {
                id: 2,
                name: "Hue".to_string(),
                x: 15,
                y: 5,
                pop: 5,
                ..StateCity::default()
            }],
            ..StateRival::default()
        });
        state.minors.push(StateMinor {
            player: 7,
            civ: "CIVILIZATION_KUMASI".to_string(),
            ..StateMinor::default()
        });

        let mirror = LiveMirror::new(&snapshot, &state, 5, 1, 500, 0);
        let at = |x: i32, y: i32| crate::hex::offset_to_axial(x, y);
        let unseen = &mirror.game.unseen_major_borders;
        assert!(
            unseen.contains(&at(5, 7)) && unseen.contains(&at(5, 8)),
            "a met major with no city on the board: its ground is an unseen border, got {unseen:?}"
        );
        assert!(
            unseen.contains(&at(5, 12)),
            "ten tiles from the only known city of theirs: a city we cannot see owns it"
        );
        assert!(
            unseen.contains(&at(10, 5)),
            "the fifth ring is ambiguous: a nearer unseen city can own it"
        );
        assert!(
            !unseen.contains(&at(14, 5)),
            "beside their known city: the forecast can count that city"
        );
        assert!(
            !unseen.contains(&at(9, 9)),
            "a minor's ground presses no loyalty and is not a major's border"
        );
        assert!(
            !unseen.contains(&at(5, 5)),
            "our own ground is never an unseen border"
        );
    }

    #[test]
    fn a_rival_border_whose_city_is_unseen_still_stops_the_unit() {
        let owned = |x: i32, y: i32, owner: i32| {
            let mut p = plot(x, y, "TERRAIN_GRASS");
            p.o = owner;
            p
        };
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 12,
            width: 20,
            height: 20,
            chunk: 1,
            plots: vec![owned(5, 5, 0), owned(5, 6, 3), owned(4, 5, -1)],
        }]);
        let mut state = StateSnapshot {
            turn: 12,
            ..StateSnapshot::default()
        };
        state.cities.push(StateCity {
            id: 1,
            name: "Roma".to_string(),
            x: 5,
            y: 5,
            pop: 4,
            ..StateCity::default()
        });
        state.units.push(StateUnit {
            id: 10,
            kind: "UNIT_SCOUT".to_string(),
            x: 5,
            y: 5,
            hp: 100.0,
            ..StateUnit::default()
        });
        // Met, nameable, and NOT at war — but not one of their cities is in sight,
        // which is the whole condition. `cities` is deliberately empty.
        state.rivals.push(StateRival {
            player: 3,
            at_war: false,
            cities: Vec::new(),
            ..StateRival::default()
        });

        let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
        let theirs = crate::hex::offset_to_axial(5, 6);
        let neutral = crate::hex::offset_to_axial(4, 5);
        let scout = *mirror
            .game
            .player_unit_ids(0)
            .first()
            .expect("the scout must reach the board");

        assert!(
            mirror.game.map.get(theirs).is_some_and(|t| t.owner_city.is_none()),
            "their plot has no owning city on this board — that is the premise, not \
             the defect: we have never seen the city that holds it"
        );
        assert!(
            !mirror.game.can_move(scout, theirs),
            "a rival's ground must stop the unit even when the mirror cannot name the \
             city that owns it — before this fix the step was legal on CIVVIS's board, \
             was ordered 74 times on one live run, and silently did nothing every time"
        );
        assert!(
            mirror.game.can_move(scout, neutral),
            "genuinely neutral ground must stay open, or the fix would seal the empire \
             in instead of the border out"
        );

        // ★ And war must OPEN it again on the next sync. The seat is named, so its
        // diplomacy is answerable even with its cities unseen; sealing ground we have
        // just declared war on would lock our own invasion out — which is a worse
        // failure than the one being repaired.
        state.rivals[0].at_war = true;
        state.turn = 13;
        mirror.sync(&snapshot, &state, 0);
        let scout = *mirror
            .game
            .player_unit_ids(0)
            .first()
            .expect("the scout survives the sync");
        assert!(
            !mirror.game.closed_borders.contains(&theirs),
            "war opens the border: the seal is recomputed from the export every turn \
             and must not outlive the peace that justified it"
        );
        assert!(
            mirror.game.can_move(scout, theirs),
            "once at war the unit must be able to cross — the repair must not cost us \
             the invasion it exists to make possible"
        );
    }

    #[test]
    fn a_bought_open_borders_grant_unseals_the_rival_ground() {
        let owned = |x: i32, y: i32, owner: i32| {
            let mut p = plot(x, y, "TERRAIN_GRASS");
            p.o = owner;
            p
        };
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 12,
            width: 20,
            height: 20,
            chunk: 1,
            plots: vec![owned(5, 5, 0), owned(5, 6, 3), owned(5, 7, 3)],
        }]);
        let mut state = StateSnapshot {
            turn: 12,
            ..StateSnapshot::default()
        };
        state.cities.push(StateCity {
            id: 1,
            name: "Roma".to_string(),
            x: 5,
            y: 5,
            pop: 4,
            ..StateCity::default()
        });
        state.units.push(StateUnit {
            id: 10,
            kind: "UNIT_SCOUT".to_string(),
            x: 5,
            y: 5,
            hp: 100.0,
            ..StateUnit::default()
        });
        state.rivals.push(StateRival {
            player: 3,
            at_war: false,
            cities: Vec::new(),
            ..StateRival::default()
        });

        let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
        let theirs = crate::hex::offset_to_axial(5, 6);
        assert!(
            mirror.game.closed_borders.contains(&theirs),
            "without a grant the fogged border stays sealed — the premise of the lane"
        );
        assert_eq!(
            mirror.game.sealed_border_owners.get(&1).copied(),
            Some(2),
            "the seal must name its owner and its size, or the buy lane cannot \
             know whom to pay: seat 1 seals both exported plots"
        );

        // The host reports the purchase: this rival now grants us Open
        // Borders. The next sync must stop sealing exactly the ground the
        // seat just paid to cross, and the shopping list must go quiet so
        // the lane never pays the same rival twice.
        state.rivals[0].open_borders = Some(true);
        state.turn = 13;
        mirror.sync(&snapshot, &state, 0);
        let scout = *mirror
            .game
            .player_unit_ids(0)
            .first()
            .expect("the scout survives the sync");
        assert!(
            !mirror.game.closed_borders.contains(&theirs),
            "an explicit grant opens the border: sealing ground the seat just \
             bought passage through would waste exactly what it paid for"
        );
        assert!(
            mirror.game.can_move(scout, theirs),
            "the bought passage must be walkable on the planning board, or the \
             gold buys a fact the planner never uses"
        );
        assert!(
            mirror.game.sealed_border_owners.is_empty(),
            "a granted rival leaves the shopping list, got {:?}",
            mirror.game.sealed_border_owners
        );

        // And a lapsed agreement re-seals on the next export, the same
        // assigned-not-extended rule as war and the seal itself.
        state.rivals[0].open_borders = Some(false);
        state.turn = 14;
        mirror.sync(&snapshot, &state, 0);
        assert!(
            mirror.game.closed_borders.contains(&theirs),
            "a lapsed grant must not leave the border open forever"
        );
        assert_eq!(
            mirror.game.sealed_border_owners.get(&1).copied(),
            Some(2),
            "a lapsed grant puts the rival back on the shopping list"
        );
    }

    #[test]
    fn sync_discards_units_that_only_civvis_simulated_from_production() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 4,
            width: 12,
            height: 12,
            chunk: 1,
            plots: vec![plot(5, 5, "TERRAIN_GRASS"), plot(5, 6, "TERRAIN_GRASS")],
        }]);
        let mut state = StateSnapshot {
            turn: 4,
            ..StateSnapshot::default()
        };
        state.units.push(StateUnit {
            id: 42,
            kind: "UNIT_WARRIOR".to_string(),
            x: 5,
            y: 5,
            hp: 73.0,
            fortified: true,
            fortify_turns: 2,
            ..StateUnit::default()
        });

        let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
        let phantom = mirror.game.spawn_test_unit(
            "archer",
            0,
            crate::hex::offset_to_axial(5, 6),
        );
        assert!(
            !mirror.civ6_of.contains_key(&phantom),
            "CIVVIS can simulate a queued production result before Firaxis creates it"
        );

        state.turn = 5;
        mirror.sync(&snapshot, &state, 0);

        assert!(
            !mirror.game.units.contains_key(&phantom),
            "the next live state must remove a locally simulated unit with no Civ VI id"
        );
        assert_eq!(
            mirror.game.units.values().filter(|unit| unit.owner == 0).count(),
            1,
            "only the exported warrior remains; otherwise CIVVIS plans with a phantom army"
        );
        let warrior = mirror.game.units.values().find(|unit| unit.owner == 0).unwrap();
        assert_eq!(warrior.hp, 73);
        assert!(warrior.fortified, "sync must not overwrite the observed fortification");
        assert_eq!(warrior.fortify_turns, 2);
    }

    #[test]
    fn live_units_keep_firaxis_charges_promotions_experience_and_religion() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 93,
            width: 12,
            height: 12,
            chunk: 1,
            plots: vec![plot(5, 5, "TERRAIN_GRASS")],
        }]);
        let state = StateSnapshot {
            turn: 93,
            units: vec![StateUnit {
                id: 91,
                kind: "UNIT_APOSTLE".to_string(),
                x: 5,
                y: 5,
                xp: Some(37),
                level: Some(2),
                promotions: Some(vec!["PROMOTION_TRANSLATOR".to_string()]),
                build_charges: Some(0),
                spread_charges: Some(2),
                religion: Some("RELIGION_CATHOLICISM".to_string()),
                ..StateUnit::default()
            }],
            ..StateSnapshot::default()
        };

        let mirror = LiveMirror::new(&snapshot, &state, 4, 1, 250, 0);
        let apostle = mirror
            .game
            .units
            .values()
            .find(|unit| unit.owner == 0)
            .expect("the Apostle is mirrored");
        assert_eq!(apostle.xp, 37);
        assert_eq!(apostle.level, 2);
        assert_eq!(apostle.charges, 2);
        assert_eq!(apostle.religion.as_deref(), Some("Catholicism"));
        assert_eq!(
            apostle
                .promotions
                .iter()
                .map(|promotion| (*promotion).as_str())
                .collect::<Vec<_>>(),
            vec!["translator"]
        );
    }

    #[test]
    fn firaxis_promotion_prefix_aliases_land_on_modelled_nodes() {
        assert_eq!(
            civvis_unit_promotion_name("PROMOTION_MONK_COBRA_STRIKE"),
            "cobra_strike"
        );
        assert_eq!(
            civvis_unit_promotion_name("PROMOTION_SUPER_CARRIER"),
            "supercarrier"
        );
        assert_eq!(
            civvis_unit_promotion_name("PROMOTION_SURF_ROCK"),
            "surf_band"
        );
        assert_eq!(
            civvis_unit_promotion_name("PROMOTION_SPY_ACE_DRIVER"),
            "ace_driver"
        );
        assert_eq!(
            civvis_unit_promotion_name("PROMOTION_SPY_GUERILLA_LEADER"),
            "guerrilla_leader"
        );
        // Every espionage promotion the bridge writes out has to come back in
        // under the name the ruleset actually holds, or an observed Spy loses
        // its promotions to `unmapped`.
        let rules = crate::rules::Rules::embedded();
        for promotion in crate::game::Game::SPY_PROMOTIONS {
            let host = if promotion == "guerrilla_leader" {
                "PROMOTION_SPY_GUERILLA_LEADER".to_string()
            } else {
                format!("PROMOTION_SPY_{}", promotion.to_ascii_uppercase())
            };
            let name = civvis_unit_promotion_name(&host);
            assert_eq!(name, promotion, "{host} does not round-trip");
            assert!(
                rules.promotions.contains_key(&name),
                "{name} is not in the ruleset"
            );
        }
    }

    #[test]
    fn a_hostile_lands_on_the_barbarian_seat_and_not_on_dormant_free_cities() {
        // ⚠ The roster has TWO players carrying `is_barbarian`, and only one of them
        // is alive. Measured on run `civvis-20260731T172058Z`: all nine barbarians
        // were owned by seat 4, Free Cities, `alive = false`, while `barb_pid` was
        // seat 5. Nothing reported it — a planted unit never reaches `dropped_units`,
        // and the seat it landed on is barbarian by flag.
        let chunks = vec![TilesChunk {
            turn: 4,
            width: 8,
            height: 8,
            chunk: 1,
            plots: (0..8)
                .flat_map(|x| {
                    (0..8).map(move |y| Plot {
                        x,
                        y,
                        im: None,
                        t: Some("TERRAIN_GRASS".to_string()),
                        f: None,
                        r: None,
                        o: -1,
                        w: false,
                        i: false,
                        fw: false,
                        rv: 0,
                        ri: false,
                        ct: None,
                        cl: -1,
                        p: false,
                        d: None,
                        dc: None,
                        wo: None,
                        rt: None,
                        rp: false,
                    })
                })
                .collect(),
        }];
        let snapshot = Snapshot::from_chunks(&chunks);
        let mut state = StateSnapshot {
            turn: 4,
            ..StateSnapshot::default()
        };
        state.hostiles.push(StateUnit {
            kind: "UNIT_WARRIOR".to_string(),
            x: 3,
            y: 3,
            ..StateUnit::default()
        });

        let recon = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
        assert_eq!(recon.placed_rival_units, 1, "the hostile must reach the board");

        let barb = recon.game.barb_pid.expect("a mirrored roster has a barbarian seat");
        let owner = recon
            .game
            .units
            .values()
            .find(|unit| unit.owner != 0)
            .map(|unit| unit.owner)
            .expect("the hostile must be on the board");
        assert_eq!(
            owner, barb,
            "a barbarian belongs to barb_pid, not to whichever seat carries the flag first"
        );
        assert!(
            recon.game.players[owner].alive,
            "and that seat must be alive — Free Cities is dormant until a revolt"
        );
        assert!(
            !recon.game.players[owner].is_free_city,
            "Free Cities is not the barbarian seat, however its flags read"
        );
    }

    fn open_grass_board(side: i32) -> Snapshot {
        let chunks = vec![TilesChunk {
            turn: 4,
            width: side,
            height: side,
            chunk: 1,
            plots: (0..side)
                .flat_map(|x| {
                    (0..side).map(move |y| Plot {
                        x,
                        y,
                        im: None,
                        t: Some("TERRAIN_GRASS".to_string()),
                        f: None,
                        r: None,
                        o: -1,
                        w: false,
                        i: false,
                        fw: false,
                        rv: 0,
                        ri: false,
                        ct: None,
                        cl: -1,
                        p: false,
                        d: None,
                        dc: None,
                        wo: None,
                        rt: None,
                        rp: false,
                    })
                })
                .collect(),
        }];
        Snapshot::from_chunks(&chunks)
    }

    #[test]
    fn a_city_states_city_reaches_the_board_and_blocks_the_ring_civ6_refuses() {
        // ★★★★ The defect in one board: run civvis-20260801T224944Z was refused
        // founding six times, every one `can_start=false,no_reasons`, and every
        // early one 2-3 tiles from a city-state city the export never mentioned.
        // `can_found_city`'s four-tile floor was correct and blind — the city it
        // needed was structurally absent, because `rivals` is built from
        // `GetAliveMajorIDs`.
        let snapshot = open_grass_board(12);
        let mut state = StateSnapshot {
            turn: 4,
            ..StateSnapshot::default()
        };
        state.units.push(StateUnit {
            kind: "UNIT_SETTLER".to_string(),
            x: 4,
            y: 6,
            ..StateUnit::default()
        });
        state.minors.push(StateMinor {
            player: 7,
            civ: "CIVILIZATION_KABUL".to_string(),
            cities: vec![StateCity {
                id: 5,
                name: "Kabul".to_string(),
                x: 6,
                y: 6,
                ..StateCity::default()
            }],
            ..StateMinor::default()
        });

        let recon = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
        assert_eq!(recon.placed_minor_cities, 1, "the city-state's city must be planted");
        let minor_city = recon
            .game
            .cities
            .values()
            .find(|city| city.owner != 0)
            .expect("the minor's city must be on the board");
        let seat = minor_city.owner;
        assert!(recon.game.players[seat].is_minor, "a city-state seats as a minor");
        assert!(
            seat >= 4,
            "a minor must never take a 1..n seat — those indices are the \
             DeclareWar-to-Civ-6-id mapping and a minor in the middle would aim a \
             declaration at the wrong civilization"
        );
        let (uid, _) = recon
            .game
            .units
            .iter()
            .find(|(_, unit)| unit.owner == 0)
            .expect("our settler must be on the board");
        assert!(
            !recon.game.can_found_city(*uid),
            "two tiles from Kabul the four-tile floor must refuse — before this \
             fix the city was invisible and CIVVIS aimed here every time"
        );
    }

    #[test]
    fn an_unplanted_known_city_still_blocks_its_settlement_ring() {
        // Firaxis keeps a met city-state in the state roster even when its
        // centre has not arrived in the terrain feed. Its nearby revealed
        // tiles must still inherit the four-tile founding floor.
        let centre = (6, 6);
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 4,
            width: 12,
            height: 12,
            chunk: 1,
            plots: (0..12)
                .flat_map(|x| (0..12).map(move |y| plot(x, y, "TERRAIN_GRASS")))
                .filter(|plot| (plot.x, plot.y) != centre)
                .collect(),
        }]);
        let settler_offset = (4, 6);
        let settler_pos = crate::hex::offset_to_axial(settler_offset.0, settler_offset.1);
        let mut state = StateSnapshot {
            turn: 4,
            ..StateSnapshot::default()
        };
        state.units.push(StateUnit {
            id: 17,
            kind: "UNIT_SETTLER".to_string(),
            x: settler_offset.0,
            y: settler_offset.1,
            ..StateUnit::default()
        });

        let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);
        let settler = mirror.uid_of[&17];
        assert!(
            mirror.game.can_found_city(settler),
            "without a reported city the fixture must be legal"
        );

        state.turn = 5;
        state.minors.push(StateMinor {
            player: 7,
            civ: "CIVILIZATION_KABUL".to_string(),
            cities: vec![StateCity {
                id: 5,
                name: "Kabul".to_string(),
                x: centre.0,
                y: centre.1,
                ..StateCity::default()
            }],
            ..StateMinor::default()
        });
        mirror.sync(&snapshot, &state, 0);

        assert!(
            mirror.game.city_at(crate::hex::offset_to_axial(centre.0, centre.1)).is_none(),
            "the fixture deliberately omits Kabul's terrain centre"
        );
        assert!(mirror.game.blocked_city_sites.contains(&settler_pos));
        assert!(
            !mirror.game.can_found_city(settler),
            "a persistent mirror must reject the host-illegal nearby site"
        );

        let fresh = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
        let fresh_settler = fresh
            .unit_ids
            .iter()
            .find_map(|(uid, civ6)| (*civ6 == 17).then_some(*uid))
            .expect("the fresh board must retain the settler");
        assert!(fresh.game.city_at(crate::hex::offset_to_axial(centre.0, centre.1)).is_none());
        assert!(fresh.game.blocked_city_sites.contains(&settler_pos));
        assert!(
            !fresh.game.can_found_city(fresh_settler),
            "a fresh-board decision must receive the same prohibition"
        );

        let legal = crate::hex::offset_to_axial(2, 6);
        let control = mirror.game.spawn_test_unit("settler", 0, legal);
        assert!(!mirror.game.blocked_city_sites.contains(&legal));
        assert!(
            mirror.game.can_found_city(control),
            "exactly four tiles from Kabul remains a legal city site"
        );
    }

    /// ★★★★ WHAT STANDS ON A RIVAL'S GROUND CROSSES WITH THE PLOTS.
    ///
    /// A rival city record carries no districts, so a rival's economy and
    /// defence were modelled from population alone. The tiles export now names
    /// the district (`d`) and wonder (`wo`) on any revealed plot; the mirror
    /// puts them on the owning rival city and rebuilds them from every export,
    /// so a razed district does not linger.
    #[test]
    fn a_rivals_districts_and_wonders_cross_with_the_plots() {
        let side = 20;
        let mut plots: Vec<Plot> = (0..side)
            .flat_map(|x| {
                (0..side).map(move |y| Plot {
                    x,
                    y,
                    im: None,
                    t: Some("TERRAIN_GRASS".to_string()),
                    f: None,
                    r: None,
                    o: -1,
                    w: false,
                    i: false,
                    fw: false,
                    rv: 0,
                    ri: false,
                    ct: None,
                    cl: -1,
                    p: false,
                    d: None,
                    dc: None,
                    wo: None,
                    rt: None,
                    rp: false,
                })
            })
            .collect();
        // The rival (Civ 6 player 3) owns a centre at (10,10), a Campus at
        // (11,10) and the Pyramids at (10,11); we sit far away at (2,2).
        for plot in plots.iter_mut() {
            match (plot.x, plot.y) {
                (10, 10) => { plot.o = 3; plot.d = Some("DISTRICT_CITY_CENTER".to_string()); }
                (11, 10) => { plot.o = 3; plot.d = Some("DISTRICT_CAMPUS".to_string()); plot.dc = Some(true); }
                (10, 11) => { plot.o = 3; plot.d = Some("DISTRICT_WONDER".to_string()); plot.wo = Some("BUILDING_PYRAMIDS".to_string()); }
                // A PLACED Encampment: `GetDistrictType` names it, `IsComplete`
                // says no. It is not on the board until it is built.
                (9, 10) => { plot.o = 3; plot.d = Some("DISTRICT_ENCAMPMENT".to_string()); plot.dc = Some(false); }
                // An older export says nothing about completion: planted.
                (10, 9) => { plot.o = 3; plot.d = Some("DISTRICT_HOLY_SITE".to_string()); }
                (11, 11) => { plot.o = 3; }
                (2, 2) => { plot.o = 0; }
                _ => {}
            }
        }
        let snapshot = Snapshot::from_chunks(&[TilesChunk { turn: 60, width: side, height: side, chunk: 1, plots }]);
        let city = |id, name: &str, x, y| StateCity {
            id, name: name.to_string(), x, y, pop: 5, loyalty: 100.0, ..StateCity::default()
        };
        let mut state = StateSnapshot { turn: 60, ..StateSnapshot::default() };
        let mut rome = city(1, "Rome", 2, 2);
        rome.capital = true;
        state.cities.push(rome);
        state.rivals.push(StateRival {
            player: 3, civ: "CIVILIZATION_SCOTLAND".to_string(),
            cities: vec![city(3, "Stirling", 10, 10)], ..StateRival::default()
        });
        let recon = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
        let stirling = recon.known_city_ids[&3];
        let campus = crate::hex::offset_to_axial(11, 10);
        let pyramids = crate::hex::offset_to_axial(10, 11);
        assert_eq!(recon.game.cities[&stirling].districts.get(crate::name!("campus")), Some(&campus));
        assert_eq!(recon.game.map.tiles[&campus].district.as_deref(), Some("campus"));
        assert_eq!(recon.game.cities[&stirling].wonders.get(&crate::name!("pyramids")), Some(&pyramids));
        assert_eq!(recon.game.map.tiles[&pyramids].wonder.as_deref(), Some("pyramids"));
        let encampment = crate::hex::offset_to_axial(9, 10);
        assert!(recon.game.cities[&stirling].districts.get(crate::name!("encampment")).is_none(),
            "a placed, unbuilt district is not on the board");
        assert!(recon.game.map.tiles[&encampment].district.is_none());
        let holy_site = crate::hex::offset_to_axial(10, 9);
        assert_eq!(recon.game.cities[&stirling].districts.get(crate::name!("holy_site")), Some(&holy_site),
            "an export without the flag is read as it always was");
        // Our own city takes nothing from this path.
        let rome_id = recon.game.player_city_ids(0)[0];
        assert!(recon.game.cities[&rome_id].districts.is_empty());
    }

    #[test]
    fn a_settler_does_not_found_a_city_that_population_pressure_will_erase() {
        // Geometry reproduces the live failure at a smaller offset: the doomed
        // site is eight tiles from our population-six city and six from the rival's,
        // while the control site is four from us and twelve from them.
        let snapshot = open_grass_board(40);
        let city = |id, name: &str, x, y| StateCity {
            id, name: name.to_string(), x, y, pop: 6, ..StateCity::default()
        };
        let mut state = StateSnapshot { turn: 45, ..StateSnapshot::default() };
        let mut rome = city(1, "Rome", 30, 2);
        rome.pop = 9;
        rome.capital = true;
        state.cities.extend([rome, city(2, "Ostia", 18, 10)]);
        state.rivals.push(StateRival {
            player: 3, civ: "CIVILIZATION_SCOTLAND".to_string(),
            cities: vec![city(3, "Stirling", 14, 13)], ..StateRival::default()
        });
        state.units.extend([
            StateUnit {
                id: 10, kind: "UNIT_SETTLER".to_string(), x: 10, y: 10,
                ..StateUnit::default()
            },
            StateUnit {
                id: 11, kind: "UNIT_SETTLER".to_string(), x: 16, y: 6,
                ..StateUnit::default()
            },
        ]);

        let recon = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
        let doomed = crate::hex::offset_to_axial(10, 10);
        let supported = crate::hex::offset_to_axial(16, 6);
        let supported_settler = recon.unit_ids.iter()
            .find_map(|(unit, civ6)| (*civ6 == 11).then_some(*unit))
            .expect("the supported Settler must cross the mirror");
        let doomed_settler = recon.unit_ids.iter()
            .find_map(|(unit, civ6)| (*civ6 == 10).then_some(*unit))
            .expect("the doomed Settler must cross the mirror");
        let stirling = recon.known_city_ids[&3];
        assert_eq!(recon.placed_rival_cities, 1);
        assert_eq!(recon.game.wdist(doomed, recon.game.cities[&stirling].pos), 6);
        let stirling_owner = recon.game.cities[&stirling].owner;
        assert_ne!(stirling_owner, 0);
        assert!(!recon.game.players[stirling_owner].is_minor);
        assert!(!recon.game.players[stirling_owner].is_barbarian);
        assert_eq!(recon.game.cities[&stirling].pop, 6);
        assert!(!recon.game.same_team(0, stirling_owner));
        assert!(recon.game.wdist(
            doomed, recon.game.cities[&recon.known_city_ids[&1]].pos
        ) > 9);
        let mut forecast = recon.game.clone();
        forecast.blocked_city_sites.remove(&doomed);
        assert!(forecast.can_found_city(doomed_settler));
        let forecast_city = forecast.found_city_for(0, doomed, None);
        let forecast_loyalty = forecast.city_loyalty_per_turn(&forecast.cities[&forecast_city]);
        assert_eq!(recon.game.wdist(
            doomed, recon.game.cities[&recon.known_city_ids[&2]].pos
        ), 8);
        assert!(
            recon.game.blocked_city_sites.contains(&doomed),
            "a city forecast at {forecast_loyalty:+.1} Loyalty/turn with stronger visible \
             foreign pressure must not consume the Settler"
        );
        assert!(
            !recon.game.blocked_city_sites.contains(&supported),
            "the filter must preserve a nearby domestically supported alternative"
        );
        assert!(
            recon.game.can_found_city(supported_settler),
            "the safe control site must remain immediately settleable"
        );
    }

    /// ⚠ The export carries a rival's units ONLY under current visibility, so a
    /// unit arriving here is one the HOST has already let the seat see — its own
    /// detection rules included. Re-deriving Naval Raider stealth on the mirror
    /// vetoed that ground truth: run `civvis-20260807T162004Z`, turns 237–251,
    /// `UNITDATA ⚠ UNIT_NUCLEAR_SUBMARINE@(4, 36) count Civ6=1 CIVVIS=0` — the
    /// sub was planted, then hidden from the seat's board, orders and threat
    /// reads because no destroyer of ours stood beside it (#1362).
    #[test]
    fn a_visible_rival_naval_raider_is_not_hidden_by_our_own_stealth_rule() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 240,
            width: 30,
            height: 30,
            chunk: 1,
            plots: vec![plot(20, 9, "TERRAIN_COAST"), plot(5, 5, "TERRAIN_GRASS")],
        }]);
        let mut state = StateSnapshot { turn: 240, ..StateSnapshot::default() };
        state.cities.push(StateCity {
            id: 1,
            name: "Rome".to_string(),
            x: 5,
            y: 5,
            pop: 6,
            capital: true,
            ..StateCity::default()
        });
        state.rivals.push(StateRival {
            player: 5,
            civ: "CIVILIZATION_AMERICA".to_string(),
            // The exact unit shape from the live export under `rivals[4]`.
            units: vec![serde_json::from_str(
                r#"{"build_charges": 0, "class": "PROMOTION_CLASS_NAVAL_RAIDER",
                    "combat": 80, "fortified": false, "fortify_turns": 0, "hp": 100,
                    "kind": "UNIT_NUCLEAR_SUBMARINE", "level": 1, "moves": 0,
                    "promotions": [], "ranged": 85, "spread_charges": 0,
                    "x": 20, "y": 9, "xp": 0}"#,
            )
            .expect("the issue's unit shape deserializes")],
            ..StateRival::default()
        });

        let recon = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
        let sub = recon
            .game
            .units
            .values()
            .find(|unit| unit.kind == "nuclear_submarine")
            .expect("the exported submarine must be planted, not dropped");
        assert_ne!(sub.owner, 0, "it is the rival's unit");
        assert!(
            recon.game.unit_visible_to(sub.id, 0),
            "the host proved the seat can see this raider; the mirror's own \
             stealth model must not veto it"
        );
        // End to end: the seat's fogged board dump — what the planner and the
        // mirror checker read — must carry the unit.
        let view = crate::obs::observation_player_view(&recon.game, 0);
        assert!(
            view["units"]
                .as_array()
                .expect("units array")
                .iter()
                .any(|unit| unit["type"] == "nuclear_submarine"),
            "the raider must appear on the seat's board"
        );
    }

    #[test]
    fn a_seated_but_cityless_minors_ground_is_still_blocked() {
        // Borders are visible before centres are: a city-state's territory can
        // arrive while its city is still under fog. A seat we can NAME but that
        // holds no city must not read as free land — that is the same hole the
        // unattributable-owner arm closes for minors we cannot name.
        let mut chunks = vec![TilesChunk {
            turn: 4,
            width: 8,
            height: 8,
            chunk: 1,
            plots: (0..8)
                .flat_map(|x| {
                    (0..8).map(move |y| Plot {
                        x,
                        y,
                        im: None,
                        t: Some("TERRAIN_GRASS".to_string()),
                        f: None,
                        r: None,
                        o: if (5..=6).contains(&x) && (5..=6).contains(&y) { 7 } else { -1 },
                        w: false,
                        i: false,
                        fw: false,
                        rv: 0,
                        ri: false,
                        ct: None,
                        cl: -1,
                        p: false,
                        d: None,
                        dc: None,
                        wo: None,
                        rt: None,
                        rp: false,
                    })
                })
                .collect(),
        }];
        let snapshot = Snapshot::from_chunks(&std::mem::take(&mut chunks));
        let mut state = StateSnapshot {
            turn: 4,
            ..StateSnapshot::default()
        };
        state.minors.push(StateMinor {
            player: 7,
            civ: "CIVILIZATION_KABUL".to_string(),
            ..StateMinor::default()
        });

        let recon = rebuild_from_state(&snapshot, &state, 4, 1, 500, 0);
        assert_eq!(recon.placed_minor_cities, 0, "no city was visible to plant");
        let pos = crate::hex::offset_to_axial(5, 5);
        assert!(
            recon.game.blocked_city_sites.contains(&pos),
            "ground a named-but-cityless minor owns must stay unfoundable"
        );
    }

    #[test]
    fn persistent_sync_keeps_a_scythian_horse_archer_on_the_board() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 4,
            width: 8,
            height: 8,
            chunk: 1,
            plots: (0..8)
                .flat_map(|x| (0..8).map(move |y| plot(x, y, "TERRAIN_GRASS")))
                .collect(),
        }]);
        let mut state = StateSnapshot {
            turn: 4,
            ..StateSnapshot::default()
        };
        let mut mirror = LiveMirror::new(&snapshot, &state, 4, 1, 500, 0);

        state.turn = 5;
        state.hostiles.push(StateUnit {
            kind: "UNIT_SCYTHIAN_HORSE_ARCHER".to_string(),
            x: 3,
            y: 3,
            ..StateUnit::default()
        });
        mirror.sync(&snapshot, &state, 0);

        let barb = mirror.game.barb_pid.expect("the mirrored roster has barbarians");
        assert!(mirror.game.units.values().any(|unit| {
            unit.owner == barb && unit.kind == "saka_horse_archer"
        }));
        assert!(
            !mirror.unmapped.contains(&"UNIT_SCYTHIAN_HORSE_ARCHER".to_string()),
            "a real Firaxis unit must not disappear after persistent sync"
        );
    }

    #[test]
    fn a_unique_great_person_is_a_modelling_gap_not_a_bridge_defect() {
        // Gran Colombia's Great General keeps its own name, so the `UNIT_GREAT_*`
        // prefix does not catch it, and it was being reported as `untranslatable` —
        // which reads as "add a vocabulary entry" when there is no entry to add.
        assert!(
            is_great_person("UNIT_COMANDANTE_GENERAL"),
            "a civilization's unique Great Person is still a Great Person"
        );
        assert!(is_great_person("UNIT_GREAT_GENERAL"), "and the prefix still works");
        assert!(
            !is_great_person("UNIT_AZTEC_EAGLE_WARRIOR"),
            "a genuinely untranslatable unit must stay a bridge defect"
        );
    }
}

/// Read every `tiles` chunk out of a run's `events.jsonl`.
///
/// The stream is append-only and a chunk is self-describing, so re-reading from
/// the start is both simplest and correct — later chunks overwrite earlier ones
/// for the same plot, which is what [`Snapshot::from_chunks`] does.
pub fn snapshot_from_events(path: &std::path::Path) -> std::io::Result<Snapshot> {
    snapshot_from_events_at(path, None)
}

/// Read the explored map as it existed through `turn`, never from its future.
pub fn snapshot_from_events_at(
    path: &std::path::Path,
    turn: Option<u32>,
) -> std::io::Result<Snapshot> {
    let raw = std::fs::read_to_string(path)?;
    // In stream order, so a later chunk's plot wins whichever kind it is;
    // a delta (`CivvisTiles.sweep`) merges without standing for a sweep —
    // see `Snapshot::merge_delta`.
    let mut snapshot = Snapshot::default();
    for line in raw.lines() {
        if !line.contains("\"tiles\"") {
            continue;
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
    apply_finished_improvements(&raw, turn, &mut snapshot);
    Ok(snapshot)
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
fn apply_finished_improvements(raw: &str, turn: Option<u32>, snapshot: &mut Snapshot) {
    for line in raw.lines() {
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
            let resolved = plot.t.as_deref().and_then(|name| match vocab.terrain(name) {
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
            // The host's road, on the engine's own ladder. An older export
            // carries no `rt` and reads 0, exactly what the mirror wrote before.
            tile.road = route_level(plot.rt.as_deref(), plot.rp);
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
            seat.strategic_resources.insert(Name::new(&name), amount.max(0.0));
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
        if plot.rv == 0 {
            continue;
        }
        let pos = crate::hex::offset_to_axial(x, y);
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
    /// ⚠⚠ THE CITY'S PRODUCTION YIELD PER TURN, AND IT WAS BEING THROWN AWAY.
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
    #[serde(default)]
    pub envoys: i64,
    #[serde(default)]
    pub most_envoys: i64,
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

    fn is_present_free_cities(&self) -> bool {
        self.is_free_cities() && (!self.cities.is_empty() || !self.units.is_empty())
    }
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
        } else if minor.is_present_free_cities() {
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
    #[serde(default)]
    pub science: f64,
    #[serde(default)]
    pub culture: f64,
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
    /// `ai::choose_dedications` is gated on `dedication_choices` (0 live, so a
    /// Dedication was never once chosen), and `ai/advanced.rs` filters
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
    /// including the one hard prerequisite the class label cannot express.
    ///
    /// A Great Scientist is not necessarily usable at a Campus: Hildegard of
    /// Bingen requires a Holy Site, while Mary Leakey requires a Theater
    /// district. The planner previously read only the class and could spend a
    /// whole Campus-project race on an individual it had no possible way to
    /// activate. The host's `ActionRequiresCompletedDistrictType` is the
    /// authoritative necessary condition, carried here without attempting to
    /// recreate every named effect in CIVVIS's ruleset.
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
    pub refused_promotions:
        std::collections::BTreeMap<i64, std::collections::BTreeSet<String>>,
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
    pub refused_districts:
        std::collections::BTreeMap<i64, std::collections::BTreeSet<String>>,
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
    pub refused_production:
        std::collections::BTreeMap<i64, std::collections::BTreeSet<String>>,
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
    let observed = game
        .observed_public_empire_stats
        .entry(owner)
        .or_default();
    observed.city_count = count(source.city_count);
    observed.population = population;
    observed.wonder_count = count(source.wonder_count);
    observed.suzerain_count = count(source.suzerain_count);
    observed.nuclear_devices = source.nuclear_devices.filter(|value| *value >= 0);
    observed.thermonuclear_devices = source
        .thermonuclear_devices
        .filter(|value| *value >= 0);
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
            None if !unmapped.iter().any(|entry| entry == civ6) => {
                unmapped.push(civ6.to_string())
            }
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
        let observed = game.observed_public_empire_stats.entry(owner).or_default();
        observed.techs = count(rival.techs);
        observed.civics = count(rival.civics);
        observed.tourism_per_turn = known(rival.tourism).then_some(rival.tourism);
        // Like `techs`/`civics`: the observed table is rebuilt from each
        // snapshot (`apply_observed_host_metrics` clears it), so absent or
        // refused reads honestly say None for THIS snapshot. The durable
        // record is the player's `science_projects` below.
        observed.foreign_tourists = count(rival.foreign_tourists);
        observed.domestic_tourists = count(rival.domestic_tourists);
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
        game.observed_yield_adjustments.remove(&owner);
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
    game.observed_yield_adjustments.insert(owner, adjustment);
}

/// Traders that Firaxis says are already servicing a route.
///
/// This remains separate from `Game::routes`: an international destination can
/// be outside the currently retained city memory, but that still never makes the
/// Trader idle or available for a second route.
pub fn active_trade_route_traders(
    state: &StateSnapshot,
) -> std::collections::BTreeSet<i64> {
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
    game.observed_route_posts.clear();
    game.observed_route_yields.clear();
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
                game.observed_route_posts
                    .insert((origin, destination), (own, foreign));
            }
        }
        // And what the host says the route pays its origin, which covers a
        // destination whose districts the seat has never seen.
        if let Some(yields) = route.yields {
            let finite = [yields.food, yields.production, yields.gold, yields.science, yields.culture, yields.faith]
                .iter()
                .all(|value| value.is_finite() && *value >= 0.0);
            if finite {
                game.observed_route_yields.insert((origin, destination), yields);
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
    for city in cities {
        let Some(incoming) = city.incoming_routes.as_ref() else {
            continue;
        };
        if incoming.origins.is_empty() {
            continue;
        }
        let Some(dest) = game.city_at(crate::hex::offset_to_axial(city.x, city.y)) else {
            unresolved.push(format!("incoming_route:{}:destination", city.name));
            continue;
        };
        let dest_owner = game.cities[&dest].owner;
        for origin in &incoming.origins {
            if origin.x < 0 || origin.y < 0 {
                unresolved.push(format!("incoming_route:{}:origin", city.name));
                continue;
            }
            let Some(origin_city) =
                game.city_at(crate::hex::offset_to_axial(origin.x, origin.y))
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

/// The engine id and target vocabulary of one host World Congress resolution,
/// or `None` when the model has no such resolution (Arms Control, Sovereignty,
/// the Diplomatic Victory resolution) or the target does not translate.
///
/// Targets follow the engine's own `congress_resolution` rosters: a player is
/// its SEAT as a decimal string, a resource/district/building/feature/project
/// its CIVVIS node name, and the class-like targets (Great Person class,
/// promotion class, great-work object, spy operation, yield) the Firaxis
/// suffix in lower case, which is what the engine keys them by.
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
    let target = resolution.target.trim();
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
    for resolution in resolutions {
        match civvis_congress_effect(&game.rules, resolution, seat_of_host, expires) {
            Some(effect) => game.active_congress_effects.push(effect),
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
        (Some(exhausted), _) => Some(exhausted.iter().filter_map(|class| kind_of(class)).collect()),
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
/// all of Firaxis's positional conditions here; `required_district` is the
/// authoritative *necessary* infrastructure condition, so the absence of that
/// district is a safe reason not to spend another point, project, or patronage
/// purchase on the live offer.
fn apply_live_great_person_offer_blockers(
    game: &mut crate::game::Game,
    state: &StateSnapshot,
    unmapped: &mut Vec<String>,
) {
    let mut blockers = BTreeMap::new();
    let Some(offers) = state.great_person_offers.as_ref() else {
        // A persistent mirror may have received this field last turn from a
        // newer mod and omit it after a rollback. Never keep a stale live-only
        // refusal alive when the current host frame no longer knows it.
        game.players[0].live_great_person_offers = None;
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
        let Some(required_district) = offer
            .required_district
            .as_deref()
            .filter(|district| !district.trim().is_empty())
        else {
            continue;
        };

        // Check the exact host name first. This covers `DISTRICT_CITY_CENTER`,
        // which CIVVIS deliberately does not store in `City::districts` because
        // every city already owns one. Then compare CIVVIS district families so
        // a unique like Russia's Lavra satisfies Firaxis's `DISTRICT_HOLY_SITE`
        // prerequisite without pretending the two literal names are equal.
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
                                    game.district_family(crate::name::Name::new(&district)) == family
                                })
                        })
                })
            });
        if active {
            continue;
        }
        if required_family.is_none() {
            let issue = format!("great_person_offer_district:{required_district}");
            if !unmapped.contains(&issue) {
                unmapped.push(issue);
            }
        }
        let individual = offer
            .individual
            .as_deref()
            .filter(|individual| !individual.trim().is_empty())
            .unwrap_or(class);
        blockers.insert(
            kind,
            format!(
                "the live {individual} offer requires an active {required_district}"
            ),
        );
    }
    game.players[0].live_great_person_offers = Some(offered_classes);
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
    ("foreign_investor", "GOVERNOR_PROMOTION_AMBASSADOR_FOREIGN_INVESTOR"),
    ("puppeteer", "GOVERNOR_PROMOTION_AMBASSADOR_PUPPETEER"),
    ("zoning_commissioner", "GOVERNOR_PROMOTION_ZONING_COMMISSIONER"),
    ("aquaculture", "GOVERNOR_PROMOTION_AQUACULTURE"),
    ("reinforced_materials", "GOVERNOR_PROMOTION_REINFORCED_INFRASTRUCTURE"),
    ("water_works", "GOVERNOR_PROMOTION_WATER_WORKS"),
    ("parks_and_recreation", "GOVERNOR_PROMOTION_PARKS_RECREATION"),
    ("grand_inquisitor", "GOVERNOR_PROMOTION_CARDINAL_GRAND_INQUISITOR"),
    ("laying_on_of_hands", "GOVERNOR_PROMOTION_CARDINAL_LAYING_ON_OF_HANDS"),
    ("citadel_of_god", "GOVERNOR_PROMOTION_CARDINAL_CITADEL_OF_GOD"),
    ("patron_saint", "GOVERNOR_PROMOTION_CARDINAL_PATRON_SAINT"),
    ("divine_architect", "GOVERNOR_PROMOTION_CARDINAL_DIVINE_ARCHITECT"),
    ("garrison_commander", "GOVERNOR_PROMOTION_GARRISON_COMMANDER"),
    ("defense_logistics", "GOVERNOR_PROMOTION_DEFENSE_LOGISTICS"),
    ("embrasure", "GOVERNOR_PROMOTION_EMBRASURE"),
    ("air_defense_initiative", "GOVERNOR_PROMOTION_AIR_DEFENSE_INITIATIVE"),
    ("arms_race_proponent", "GOVERNOR_PROMOTION_EDUCATOR_ARMS_RACE_PROPONENT"),
    ("connoisseur", "GOVERNOR_PROMOTION_EDUCATOR_CONNOISSEUR"),
    ("researcher", "GOVERNOR_PROMOTION_EDUCATOR_RESEARCHER"),
    ("grants", "GOVERNOR_PROMOTION_EDUCATOR_GRANTS"),
    ("space_initiative", "GOVERNOR_PROMOTION_EDUCATOR_SPACE_INITIATIVE"),
    ("curator", "GOVERNOR_PROMOTION_MERCHANT_CURATOR"),
    ("harbormaster", "GOVERNOR_PROMOTION_MERCHANT_HARBORMASTER"),
    ("forestry_management", "GOVERNOR_PROMOTION_MERCHANT_FORESTRY_MANAGEMENT"),
    ("tax_collector", "GOVERNOR_PROMOTION_MERCHANT_TAX_COLLECTOR"),
    ("contractor", "GOVERNOR_PROMOTION_MERCHANT_CONTRACTOR"),
    ("renewable_subsidizer", "GOVERNOR_PROMOTION_MERCHANT_RENEWABLE_ENERGY"),
    ("surplus_logistics", "GOVERNOR_PROMOTION_RESOURCE_MANAGER_SURPLUS_LOGISTICS"),
    ("provision", "GOVERNOR_PROMOTION_RESOURCE_MANAGER_EXPEDITION"),
    ("industrialist", "GOVERNOR_PROMOTION_RESOURCE_MANAGER_INDUSTRIALIST"),
    ("black_marketeer", "GOVERNOR_PROMOTION_RESOURCE_MANAGER_BLACK_MARKETEER"),
    ("vertical_integration", "GOVERNOR_PROMOTION_RESOURCE_MANAGER_VERTICAL_INTEGRATION"),
];

const GOVERNOR_BASE_PROMOTION_TYPES: &[(&str, &str)] = &[
    ("amani", "GOVERNOR_PROMOTION_AMBASSADOR_MESSENGER"),
    ("liang", "GOVERNOR_PROMOTION_BUILDER_GUILDMASTER"),
    ("moksha", "GOVERNOR_PROMOTION_CARDINAL_BISHOP"),
    ("victor", "GOVERNOR_PROMOTION_REDOUBT"),
    ("pingala", "GOVERNOR_PROMOTION_EDUCATOR_LIBRARIAN"),
    ("reyna", "GOVERNOR_PROMOTION_MERCHANT_LAND_ACQUISITION"),
    ("magnus", "GOVERNOR_PROMOTION_RESOURCE_MANAGER_GROUNDBREAKER"),
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
    game.observed_leader_types.clear();
    if !state.seat.leader.is_empty() {
        game.observed_leader_types.insert(0, state.seat.leader.clone());
    }
    for (index, rival) in state.rivals.iter().enumerate() {
        if !rival.leader.is_empty() {
            game.observed_leader_types
                .insert(index + 1, rival.leader.clone());
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
    "id", "name", "buildings", "pillaged_buildings", "religion", "religion_next",
    "religion_turns",
    "pantheon_active", "districts", "wonders", "worked", "specialists", "great_works",
    "yields", "producing", "producing_hash", "production_progress", "production",
    "production_cost", "production_turns", "food", "loyalty_per_turn", "falls_to",
    "x", "y", "pop", "capital", "defense", "damage", "max_damage", "wall_damage",
    "max_wall_damage", "loyalty", "housing", "housing_from_improvements",
    // The host's own amenity ledger and the multiplier it puts on every non-food
    // yield. `the_schema_allowlists_cover_every_declared_field` caught these missing
    // on the first run, which is the whole reason that test exists.
    "amenities", "amenities_needed", "happiness", "happiness_yield_mult",
    "amenities_luxuries", "amenities_entertainment", "amenities_civics",
    "amenities_city_states", "amenities_war_weariness", "amenities_bankruptcy",
    // The complete amenity and housing ledgers, the host's growth arithmetic and
    // the per-yield source tooltips: the fields the yield-fidelity instrument
    // reads. `the_schema_allowlists_cover_every_declared_field` fails if a
    // StateCity field is missing here.
    "amenities_great_people", "amenities_religion", "amenities_national_parks",
    "amenities_starting_era", "amenities_improvements", "amenities_districts",
    "amenities_natural_wonders",
    "housing_from_water", "housing_from_buildings", "housing_from_districts",
    "housing_from_civics", "housing_from_great_people", "housing_from_starting_era",
    "housing_from_great_works",
    "food_surplus", "growth_threshold", "growth_turns", "housing_growth_mult",
    "happiness_growth_mult", "overall_growth_mult",
    "yield_sources", "center_yields", "incoming_routes",
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
];

const PUBLIC_STATS_KEYS: &[&str] = &[
    "city_count", "population", "food", "production", "wonder_count", "suzerain_count",
    "nuclear_devices", "thermonuclear_devices",
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
    let start = source.find(&head).expect("the struct is declared in this file");
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
        let Some(object) = value.as_object() else { return };
        for key in object.keys() {
            if !allowed.contains(&key.as_str()) {
                gaps.insert(format!("schema:{path}.{key}"));
            }
        }
    }

    #[rustfmt::skip]
    const STATE: &[&str] = &[
        "kind", "event", "run", "ctx", "turn", "frame", "techs", "civics", "research",
        "science_projects", "boosted_techs", "boosted_civics",
        "research_progress", "civic", "civic_progress", "government", "used_governments",
        "pantheon",
        "founded_religion", "founded_religions", "religion_beliefs",
        "taken_religion_beliefs", "religions", "prophet_pending",
        "policies", "policy_slots", "gold", "gold_per_turn", "faith", "faith_per_turn",
        "faith_sources", "science",
        "culture", "public_stats", "score", "dvp", "favor", "congress_dvp",
        "spy_capacity",
        "foreign_tourists", "domestic_tourists",
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
        "heroic_golden_age", "dedications", "resolutions", "congress_turns_left",
        "emergencies",
        "governors", "cities", "units", "trade_routes", "rivals", "minors", "hostiles",
        // Unspent envoys. `the_schema_allowlists_cover_every_declared_field` fails
        // if a new StateSnapshot field is missing here — this list is a second
        // copy of the struct's names and nothing keeps them in step automatically.
        "envoys_free",
    ];
    const CITY: &[&str] = CITY_KEYS;
    const DISTRICT: &[&str] = &[
        "type", "x", "y", "pillaged", "complete",
        // Hit points. `the_schema_allowlists_cover_every_declared_field` fails if
        // a StateDistrict field is missing here.
        "damage", "max_damage", "wall_damage", "max_wall_damage",
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
        "type", "city", "city_player", "x", "y", "established", "turns_on_site",
        "turns_to_establish", "neutralized_turns", "promotions",
    ];
    const RIVAL: &[&str] = &[
        "player", "civ", "leader", "government", "dark_age", "golden_age",
        "heroic_golden_age", "can_declare", "score", "dvp", "military", "at_war",
        "techs", "civics", "cities", "units",
        "science", "culture", "tourism", "gold", "gold_per_turn", "faith", "faith_per_turn",
        "public_stats",
        // Rival victory progress as the shipped World Rankings screen shows it.
        // `the_schema_allowlists_cover_every_declared_field` fails if a new
        // StateRival field is missing here.
        "science_projects",
        "foreign_tourists",
        "domestic_tourists",
    ];
    const MINOR: &[&str] = &[
        "player", "civ", "score", "military", "at_war", "suzerain", "envoys",
        "most_envoys", "cities", "units",
    ];
    const RELIGION: &[&str] = &["type", "founder", "beliefs"];

    fn cities(value: Option<&serde_json::Value>, gaps: &mut std::collections::BTreeSet<String>) {
        for city in value.and_then(|v| v.as_array()).into_iter().flatten() {
            keys(city, CITY, "city", gaps);
            for district in city.get("districts").and_then(|v| v.as_array()).into_iter().flatten() {
                keys(district, DISTRICT, "district", gaps);
            }
            for wonder in city.get("wonders").and_then(|v| v.as_array()).into_iter().flatten() {
                keys(wonder, WONDER, "wonder", gaps);
            }
            for plot in city.get("worked").and_then(|v| v.as_array()).into_iter().flatten() {
                keys(plot, WORKED, "worked", gaps);
            }
            for work in city.get("great_works").and_then(|v| v.as_array()).into_iter().flatten() {
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
    for governor in value.get("governors").and_then(|v| v.as_array()).into_iter().flatten() {
        keys(governor, GOVERNOR, "governor", &mut gaps);
    }
    for route in value.get("trade_routes").and_then(|v| v.as_array()).into_iter().flatten() {
        keys(route, ROUTE, "trade_route", &mut gaps);
    }
    for rival in value.get("rivals").and_then(|v| v.as_array()).into_iter().flatten() {
        keys(rival, RIVAL, "rival", &mut gaps);
        public_stats(rival.get("public_stats"), "rival.public_stats", &mut gaps);
        cities(rival.get("cities"), &mut gaps);
        units(rival.get("units"), &mut gaps);
    }
    for minor in value.get("minors").and_then(|v| v.as_array()).into_iter().flatten() {
        keys(minor, MINOR, "minor", &mut gaps);
        cities(minor.get("cities"), &mut gaps);
        units(minor.get("units"), &mut gaps);
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

pub fn state_from_events(
    path: &std::path::Path,
    turn: Option<u32>,
) -> Option<StateSnapshot> {
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
        || base_game_mars.iter().all(|project| reported.contains(project))
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
    let base = civ6.strip_prefix(prefix).unwrap_or(civ6).to_ascii_lowercase();
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
    let base = civ6.strip_prefix("UNIT_").unwrap_or(civ6).to_ascii_lowercase();
    // ★★★ CIVILIZATION VI'S BARBARIAN VARIANTS ARE THE ORDINARY UNIT WITH A PREFIX.
    //
    // `UNIT_BARBARIAN_HORSEMAN` is a Horseman and `UNIT_BARBARIAN_HORSE_ARCHER` is a
    // horse archer; CIVVIS models `horseman` and `saka_horse_archer` but neither
    // `barbarian_horseman` nor `barbarian_horse_archer`, so both were dropped from the
    // board entirely — 276 sightings across tonight's runs, every one a raider CIVVIS
    // could not see while its settlers walked past.
    //
    // ⚠ This is a rename, not a substitution: the barbarian variants ARE these units
    // in the shipped database. Where the stripped name is not modelled either it still
    // falls through to `dropped_units` as untranslatable rather than being guessed at —
    // `horse_archer` has no plain entry, so it resolves to the closest CIVVIS actually
    // has rather than inventing one.
    let base = base.strip_prefix("barbarian_").map(str::to_string).unwrap_or(base);
    match base.as_str() {
        // Firaxis's Scythian type name includes the civilization, whereas
        // CIVVIS stores the unit by its actual Saka name.
        "horse_archer" | "scythian_horse_archer" => "saka_horse_archer".to_string(),
        // Firaxis retained Poland's implementation id after the unit's display
        // name became Winged Hussar.
        "polish_hussar" => "winged_hussar".to_string(),
        // CIVVIS does not yet carry these unique unit specifications. Firaxis's
        // own UnitReplaces table names their exact stock role, which is preferable
        // to deleting a visible hostile from the board entirely.
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
        // The shipped type id predates the final Civilopedia name.
        "beach_resort" => "seaside_resort".to_string(),
        _ => base,
    }
}

/// The CIVVIS unit that stands in for a Civilization VI promotion class, for a
/// unique whose own name is unmodelled and that REPLACES nothing — `UnitReplaces`
/// has no row for a Malón Raider, a Varu or a Nihang, so the `base` fallback one
/// rung up never fires for them.
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
    candidates.iter().copied().find(|c| rules.units.contains_key(c))
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
/// A full guard needs an adjective map: `data/civs.json` is keyed by display name
/// (`Rome`, `Aztec`), so `aztec` needs a case-insensitive match and `roman`/`nubian`
/// need the adjectival form. What is certain under any casing is that **`great` is not
/// a civilization**, and that is the one prefix measured doing this — see
/// [`GREAT_PERSON_PREFIX`].
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
fn resolved_civvis_unit_name(
    rules: &crate::rules::Rules,
    civ6: &str,
) -> Option<String> {
    let direct = civvis_unit_name(civ6);
    if rules.units.contains_key(&direct) {
        return Some(direct);
    }
    let bare = civvis_unit_name_unqualified(civ6);
    if let Some(bare) = bare.as_deref().filter(|bare| rules.units.contains_key(*bare)) {
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
        .filter(|name| name.as_str().ends_with(suffix.as_str()));
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
pub(crate) fn grow_frontier(
    game: &mut crate::game::Game,
    snapshot: &Snapshot,
    depth: u32,
) {
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
            event.get("turn").and_then(|value| value.as_u64()).unwrap_or(0) > limit as u64
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
            event.get("turn").and_then(|value| value.as_u64()).unwrap_or(0) > limit as u64
        }) {
            continue;
        }
        let (Some(unit), Some(promotion)) = (
            event.get("unit").and_then(|v| v.as_i64()),
            event.get("promotion").and_then(|v| v.as_str()),
        ) else {
            continue;
        };
        refused.entry(unit).or_default().insert(promotion.to_string());
    }
    refused
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
    for line in raw.lines().filter(|line| line.contains("trade_route_refused")) {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if event.get("kind").and_then(|value| value.as_str()) != Some("trade_route_refused")
            || turn.is_some_and(|limit| {
                event.get("turn").and_then(|value| value.as_u64()).unwrap_or(0)
                    > limit as u64
            })
        {
            continue;
        }
        let values = ["from_x", "from_y", "x", "y"]
            .map(|key| event.get(key).and_then(|value| value.as_i64()).map(|v| v as i32));
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
            .filter_map(|name| {
                civvis_production_item(rules, Some(name), &[], None)
                    .map(|item| crate::game::Game::production_block_key(&item))
                    .or_else(|| {
                        civvis_node_name(&rules.districts, name, "DISTRICT_")
                            .map(|district| format!("district:{district}"))
                    })
            })
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
    let mut blocked = std::mem::take(&mut game.blocked_production);
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
fn seat_live_spies(game: &mut crate::game::Game) {
    game.spies.retain(|_, spy| spy.owner != 0);
    let live: Vec<(u32, i64, std::collections::BTreeSet<String>, Option<u32>)> = game
        .units
        .values()
        .filter(|unit| unit.owner == 0 && unit.kind == "spy")
        .map(|unit| {
            let city = game
                .cities
                .iter()
                .find(|(_, city)| city.pos == unit.pos)
                .map(|(id, _)| *id);
            (
                unit.id,
                (unit.level - 1).max(0) as i64,
                unit.promotions
                    .iter()
                    .map(|name| name.to_string())
                    .collect(),
                city,
            )
        })
        .collect();
    for (id, level, promotions, city) in live {
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
    let mut newest: BTreeMap<(i64, String), (u64, Option<BTreeSet<crate::Pos>>)> =
        BTreeMap::new();
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
        if turn > u64::from(current_turn)
            || turn < u64::from(oldest)
            || !item.starts_with(prefix)
        {
            continue;
        }
        let key = (city, item.to_string());
        if newest.get(&key).is_some_and(|(known_turn, _)| *known_turn > turn) {
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
            event.get("turn").and_then(|value| value.as_u64()).unwrap_or(0) > limit as u64
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
    if state.science <= 0.0 && state.culture <= 0.0 {
        return None;
    }
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
    // Science and culture arrive as seat totals; production only ever existed
    // per-city, and `StateCity` was not reading it (see the field). Summing the
    // export's own per-city figure gives the same civ6-versus-CIVVIS comparison for
    // the yield that decides what every city builds.
    //
    // ⚠ Only cities the export actually reported a figure for are summed, so an
    // older mod reads as unknown rather than as zero — the same rule the seat totals
    // follow. `unknown_metric` is negative, which is why the filter is `>= 0`.
    let host_production: f64 = state
        .cities
        .iter()
        .map(|city| city.production)
        .filter(|value| *value >= 0.0)
        .sum();
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
    let production_part = match host_production > 0.0 {
        true => format!(
            " production {:.1}/{:.1} {}",
            host_production,
            production,
            pct(production, host_production)
        ),
        false => String::new(),
    };
    Some(format!(
        "economy civ6/civvis science {:.1}/{:.1} {} culture {:.1}/{:.1} {}{}{}{}{}",
        state.science,
        science,
        pct(science, state.science),
        state.culture,
        culture,
        pct(culture, state.culture),
        production_part,
        attributed,
        host_amenity_report(state),
        host_envoy_report(state),
    ))}

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
            event.get("turn").and_then(|value| value.as_u64()).unwrap_or(0) > limit as u64
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
                merge_state(base_map.entry(key.clone()).or_insert(serde_json::Value::Null), value);
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

fn apply_city_religion(live: &mut crate::game::City, state: &StateCity) {
    live.pressure.clear();
    match state.religion.as_deref().and_then(civvis_religion_name) {
        Some(religion) => {
            live.atheist_pressure = 0.0;
            live.pressure.insert(religion, 100.0);
        }
        None => live.atheist_pressure = 50.0,
    }
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
fn apply_encampment_health(
    game: &mut crate::game::Game,
    state: &StateCity,
    cid: u32,
) {
    let encampment = state
        .districts
        .iter()
        .find(|district| district.kind.eq_ignore_ascii_case("DISTRICT_ENCAMPMENT"));
    // Read the wall maximum before taking the mutable borrow below.
    let Some(max_wall) = game.cities.get(&cid).map(|city| game.city_max_wall_hp(city)) else {
        return;
    };
    let Some(city) = game.cities.get_mut(&cid) else { return };
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
    city.encampment_wall_hp =
        if encampment.max_wall_damage > 0 && encampment.wall_damage >= 0 {
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
    let Some(pillaged) = &state.pillaged_buildings else { return };
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
    let Some(city) = game.cities.get(&cid) else { return };
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
            if tile.district_foundation.as_ref()
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
            remember_issue(unmapped, format!(
                "{}@{},{}:district_plot_missing", district.kind, district.x, district.y
            ));
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
            remember_issue(unmapped, format!(
                "{}@{},{}:wonder_plot_missing", wonder.kind, wonder.x, wonder.y
            ));
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
        let item = crate::game::Item::District { district: name, pos };
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
        game.observed_city_max_wall_hp.insert(cid, max_wall);
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
    let observed_charges = state
        .build_charges
        .into_iter()
        .chain(state.spread_charges)
        .filter(|charges| *charges > 0)
        .max();
    if let Some(charges) = observed_charges {
        live.charges = charges;
    }
    live.fortified = state.fortified;
    live.fortify_turns = state.fortify_turns.clamp(0, 2);
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
) {
    if !minor.is_city_state() {
        return;
    }
    if minor.suzerain >= 0 {
        if let Some(&holder) = seat_of_host.get(&(minor.suzerain as usize)) {
            let current = mirrored_envoys(&game.players[holder], owner);
            let winning = if holder == 0 {
                minor.envoys.max(3)
            } else {
                minor.most_envoys.max(minor.envoys.max(0) + 1).max(3)
            };
            set_mirrored_envoys(
                &mut game.players[holder],
                owner,
                current.max(3).max(winning),
            );
        }
        return;
    }
    let ours = mirrored_envoys(&game.players[0], owner);
    let blocker = game
        .players
        .iter()
        .find(|player| player.id != 0 && player.alive && !player.is_minor)
        .map(|player| player.id);
    for pid in 1..game.players.len() {
        if !game.players[pid].is_minor {
            set_mirrored_envoys(&mut game.players[pid], owner, 0);
        }
    }
    if ours >= 3 {
        if let Some(blocker) = blocker {
            set_mirrored_envoys(&mut game.players[blocker], owner, ours);
        }
    }
}

/// Apply public host measurements after the reconstructed economy and city
/// roster are complete. Yield differences are stored as corrections so an AI
/// clone can still measure the effect of a candidate policy or building.
fn great_work_kind(object: &str) -> Option<&'static str> {
    match object {
        "GREATWORKOBJECT_WRITING" => Some("writing"),
        "GREATWORKOBJECT_LANDSCAPE"
        | "GREATWORKOBJECT_PORTRAIT"
        | "GREATWORKOBJECT_SCULPTURE" => Some("art"),
        "GREATWORKOBJECT_RELIGIOUS" => Some("religious_art"),
        "GREATWORKOBJECT_ARTIFACT" => Some("artifact"),
        "GREATWORKOBJECT_MUSIC" => Some("music"),
        "GREATWORKOBJECT_RELIC" => Some("relic"),
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

/// Restore exact private city facts before deriving host-to-model corrections.
fn apply_observed_city_economy(
    game: &mut crate::game::Game,
    state: &StateSnapshot,
    unmapped: &mut Vec<String>,
) {
    game.observed_city_yield_adjustments.clear();
    // Clear first: the previous correction is part of `city_amenities` and
    // `city_housing`, and using it while deriving this turn's delta would
    // compound it forever.
    game.observed_city_amenity_adjustments.clear();
    game.observed_city_housing_adjustments.clear();
    game.observed_tile_yield_adjustments.clear();
    game.observed_city_worked_tiles.clear();
    game.observed_city_specialists.clear();

    for observed in &state.cities {
        let pos = crate::hex::offset_to_axial(observed.x, observed.y);
        let Some(cid) = game.city_at(pos) else { continue };
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
                        game.cities.get_mut(&cid).unwrap().owned_tiles.push(worked_pos);
                    }
                    game.map.tiles.get_mut(&worked_pos).unwrap().owner_city = Some(cid);
                }
                game.observed_city_worked_tiles.insert(cid, positions);
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
                        game.district_family(crate::name::Name::new(&name)).to_string(),
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
                game.observed_city_specialists.insert(cid, translated);
            }
        }
    }

    // Replace only when every own-city query succeeded. A partial export is
    // unknown, not authority to erase works housed in the omitted city.
    if !state.cities.is_empty() && state.cities.iter().all(|city| city.great_works.is_some()) {
        for kind in ["writing", "art", "religious_art", "artifact", "music", "relic"] {
            game.players[0].counters.insert(format!("great_work:{kind}"), 0);
        }
        game.players[0].great_work_pieces.clear();
        let mut seen = std::collections::BTreeSet::new();
        // ...and WHERE the host keeps each one. The model's own housing picks
        // the best slot for a work (a Relic goes to St. Basil's over the
        // Palace); the host's placement is what pays, and it read "+6 from
        // GreatWorks" in Rome while the model paid Mediolanum (run
        // civvis-20260816T233226Z t154+).
        let mut housing: std::collections::BTreeMap<u32, std::collections::BTreeMap<String, usize>> =
            Default::default();
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
                            *housing.entry(cid).or_default().entry(kind.to_string()).or_insert(0) += 1;
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
        game.observed_great_work_housing = Some(housing);
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
        let Some(_cid) = game.city_at(pos) else { continue };
        let finite = |yields: &crate::rules::Yields| {
            [yields.food, yields.production, yields.gold,
             yields.science, yields.culture, yields.faith]
            .iter()
            .all(|value| value.is_finite())
        };
        let delta = |host: crate::rules::Yields, model: crate::rules::Yields| crate::rules::Yields {
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
            game.observed_tile_yield_adjustments
                .insert(pos, delta(host, model));
        }
        for plot in observed.worked.iter().flatten() {
            let Some(host) = plot.yields.filter(finite) else { continue };
            let plot_pos = crate::hex::offset_to_axial(plot.x, plot.y);
            if plot_pos == pos {
                continue;
            }
            let Some(tile) = game.map.get(plot_pos) else { continue };
            if tile.district.is_some()
                || tile.district_foundation.is_some()
                || tile.wonder.is_some()
                || game.city_at(plot_pos).is_some()
            {
                continue;
            }
            let model = game.modeled_tile_yields(plot_pos);
            game.observed_tile_yield_adjustments
                .insert(plot_pos, delta(host, model));
        }
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
        let Some(cid) = game.city_at(pos) else { continue };
        let modeled_surplus = game.city_amenity_surplus(&game.cities[&cid]);
        game.observed_city_amenity_adjustments
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
        let Some(cid) = game.city_at(pos) else { continue };
        let modeled_housing = game.city_housing(&game.cities[&cid]);
        game.observed_city_housing_adjustments
            .insert(cid, host_housing - modeled_housing);
    }

    // What remains is a local correction for host rules CIVVIS has not modeled.
    for observed in &state.cities {
        let Some(host) = observed.yields else { continue };
        if ![
            host.food, host.production, host.gold, host.science, host.culture, host.faith,
        ]
        .iter()
        .all(|value| value.is_finite())
        {
            continue;
        }
        let pos = crate::hex::offset_to_axial(observed.x, observed.y);
        let Some(cid) = game.city_at(pos) else { continue };
        let model = game.city_yields_model(cid);
        let adjustment = crate::rules::Yields {
            food: host.food - model.food,
            production: host.production - model.production,
            gold: host.gold - model.gold,
            science: host.science - model.science,
            culture: host.culture - model.culture,
            faith: host.faith - model.faith,
        };
        game.observed_city_yield_adjustments.insert(cid, adjustment);
    }
}

fn apply_observed_host_metrics(
    game: &mut crate::game::Game,
    state: &StateSnapshot,
    unmapped: &mut Vec<String>,
) {
    game.observed_trade_capacity.clear();
    game.observed_yield_adjustments.clear();
    game.observed_public_empire_stats.clear();
    game.observed_city_loyalty_per_turn.clear();
    game.observed_city_strength.clear();
    game.observed_city_max_wall_hp.clear();
    if let Some(capacity) = state.trade_capacity.filter(|capacity| *capacity >= 0) {
        game.observed_trade_capacity.insert(0, capacity);
    }
    apply_public_empire_stats(game, 0, &state.public_stats);
    {
        // Same per-snapshot honesty as the rival counters: unknown is None.
        let count = |value: f64| {
            (value.is_finite() && value >= 0.0 && value <= usize::MAX as f64)
                .then(|| value.round() as usize)
        };
        let observed = game.observed_public_empire_stats.entry(0).or_default();
        observed.foreign_tourists = count(state.foreign_tourists);
        observed.domestic_tourists = count(state.domestic_tourists);
    }

    apply_observed_city_economy(game, state, unmapped);

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
    if state.science.is_finite() && state.science > 0.0 {
        adjustment.science = state.science - derived.science;
    }
    if state.culture.is_finite() && state.culture > 0.0 {
        adjustment.culture = state.culture - derived.culture;
    }
    // Faith per turn: the host's top-bar figure against the same sum, applied
    // as a delta like science and culture. Only when the export carries it —
    // an older control mod leaves the model's own figure standing.
    if let Some(host_faith) = state.faith_per_turn.filter(|value| value.is_finite() && *value >= 0.0) {
        adjustment.faith = host_faith - derived.faith;
    }
    if adjustment.food != 0.0
        || adjustment.production != 0.0
        || adjustment.science != 0.0
        || adjustment.culture != 0.0
        || adjustment.faith != 0.0
    {
        game.observed_yield_adjustments.insert(0, adjustment);
    }
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
        // Population drives Loyalty pressure in a nine-tile radius. The own-city
        // rebuild copied it earlier, but visible rival and city-state cities stayed
        // at `place_city`'s population-one default. In the live Cumae failure that
        // made population-six Stirling exert one sixth of the pressure Firaxis was
        // applying, so a forecast built on this otherwise exact board was safe only
        // because the most important input had been dropped.
        if observed.pop > 0 {
            game.cities.get_mut(&cid).unwrap().pop = observed.pop;
        }
        // ★★★★ WHERE THE PALACE IS. `place_city` flags the first city it seats
        // for a player as the capital, so a seat that lost its founding city
        // kept its Palace on whichever city the export happened to list first.
        // Measured on run civvis-20260816T040537Z: Rome fell at t79, the host
        // moved the Palace to Aquileia (`capital: true`), and the model paid it
        // in Antium instead — Aquileia short 5 Gold, 2 Production, 2 Science
        // and 1 Culture every turn to the end of the game while Antium was
        // over by the same, the single largest persistent gap of the run. The
        // host's `IsCapital` is the current capital, exactly what
        // `city_has_palace` reads; every mirrored city takes it, rivals and
        // city-states included, so their Palaces sit where the host's do.
        if flagged_capitals.contains(&game.cities[&cid].owner) {
            game.cities.get_mut(&cid).unwrap().is_capital = observed.capital;
        }
        apply_city_health(game, cid, observed);
        if observed.loyalty_per_turn.is_finite() {
            game.observed_city_loyalty_per_turn
                .insert(cid, observed.loyalty_per_turn);
        }
        if observed.defense.is_finite() && observed.defense >= 0.0 {
            game.observed_city_strength.insert(cid, observed.defense);
        }
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

    if let Some(speed) = civvis_game_speed(&state.seat.speed) {
        // These are deliberately redundant in `Game` for save compatibility.
        // The viewer renders `game_speed`; a number of rules still use `speed`
        // to find the speed spec. A half-update is therefore a visual lie or a
        // mathematical one depending on which path reads it first.
        game.speed = speed.id().to_string();
        game.game_speed = speed;
    }
    apply_seat_victories(&mut game, &state.seat);

    if let Some(difficulty) = civvis_difficulty(&state.seat.difficulty)
        .filter(|difficulty| game.rules.difficulties.contains_key(difficulty))
    {
        game.difficulty = difficulty;
    }

    if let Some(map_script) = civvis_map_script(&state.seat.map) {
        game.map_script = map_script;
    }

    // Sites the host engine has already rejected, so the planner stops re-deriving
    // them. See `refused_sites_of_kind_through`.
    game.blocked_city_sites = state.refused_sites.clone();
    game.blocked_improvement_sites = state.refused_improves.clone();
    game.blocked_trade_routes = state.refused_trade_routes.clone();
    game.blocked_policies = blocked_policies_from(&state.refused_policy_names, &game.rules);
    game.blocked_pantheons = blocked_pantheons_from(&state.refused_pantheons, &game.rules);
    // ⚠ Wired after `city_ids` below would be too late for the rebuild, so this is
    // filled in at the end of the function where both are in hand.

    // Identity first: city naming reads it, so this cannot wait until after the
    // cities are placed. See `apply_identity`.
    let identity_unmapped = apply_identity(&mut game, state);
    if !identity_unmapped.is_empty() {
        eprintln!(
            "mirror: no CIVVIS civilization for {identity_unmapped:?} — those seats keep their \
             default roster name and will NOT match the Civilization VI screen"
        );
    }

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
    let mut city_ids = std::collections::BTreeMap::new();
    // Every retained city, including visible rivals.  `city_ids` stays our-city
    // only because order translation must never point a purchase at a rival;
    // active international routes need the broader lookup.
    let mut known_city_ids = std::collections::BTreeMap::new();
    let mut unmapped = identity_unmapped;
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

    // ★★★★ TELL CIVVIS WHAT TURN IT IS. `Game::new` starts at the beginning, and the
    // board is rebuilt from scratch every turn, so without this CIVVIS was answering
    // TURN 1 for the whole game — every time. Measured consequence on run
    // civvis-20260730T111953Z: 15 production orders, ALL of them Warrior, no settler
    // and no district, while its own plan asked for 3 cities. An agent whose strategy
    // is keyed to era and timing cannot plan from a clock stuck at zero.
    game.turn = state.turn.max(1);
    game.observed_score.clear();
    game.observed_military_power.clear();
    if state.score >= 0 {
        game.observed_score.insert(0, state.score);
    }
    if state.military.is_finite() && state.military >= 0.0 {
        game.observed_military_power.insert(0, state.military);
    }
    // ★★★ AND HOW MANY TURNS ARE LEFT. `rebuild_game` hardcodes 500; this build's real
    // limit at Tiny/Online reads 250 (`seat.max_turns`, and the HUD shows TURN n/250).
    // CIVVIS keys several windows on the remaining turns — `expansion_pays_back_for`
    // asks whether a settler can still pay for itself before the game ends, and
    // `expansion_window_open` reserves the endgame — so a horizon that is twice too
    // long makes late expansion look affordable when it is not, and distorts every
    // build-versus-fight trade in the other direction too.
    game.max_turns = max_turns;
    // The treasury and each city's population are read by CIVVIS's buy and build
    // decisions. Defaults made a 20-population empire with 600 gold look like a
    // founding settlement.
    if state.gold >= 0 {
        game.players[0].gold = state.gold as f64;
    }
    // ⚠ THE FRESH-BOARD PATH IS THE ONE THAT MATTERS. `civvis_orders --serve
    // --fresh-board` comes through here every turn, and this rebuild has no
    // predecessor to difference against, so before this line `gold_per_turn` was
    // whatever `Player::default` said — 0 — in every live decision.
    if let Some(net) = state.gold_per_turn.filter(|net| net.is_finite()) {
        game.players[0].gold_per_turn = net;
    }
    if state.faith >= 0 {
        game.players[0].faith = state.faith as f64;
    }
    if let Some(dvp) = state.dvp {
        game.players[0].dvp = dvp;
    }
    apply_congress_dvp(&mut game, state);
    apply_host_competitions(&mut game, state);
    if let Some(favor) = state.favor.filter(|favor| favor.is_finite()) {
        game.players[0].diplomatic_favor = favor;
    }
    apply_mirrored_envoys_free(&mut game, state);
    apply_player_religion(&mut game, state, &mut unmapped);
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
                game.players[0].policies.insert(crate::name::Name::new(&name));
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
                        "{}@{},{}:approximated_as_{from_base}", u.kind, u.x, u.y
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
                            "{}@{},{}:approximated_as_{rep}_from_{label}", u.kind, u.x, u.y
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
    for unit in &state.units {
        if let Some(uid) = plant_unit(&mut game, 0, unit, &mut unmapped, &mut dropped) {
            unit_ids.insert(uid, unit.id);
            placed_units += 1;
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
            game.observed_military_power.insert(owner, rival.military);
        }
        if rival.score >= 0 {
            game.observed_score.insert(owner, rival.score);
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
        if rival.can_declare && !rival.at_war {
            game.players[0].denounced_until.insert(owner, game.turn + 1);
        }
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
            if plant_unit(&mut game, owner, unit, &mut unmapped, &mut dropped).is_some() {
                placed_rival_units += 1;
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
        set_mirrored_envoys(&mut game.players[0], owner, minor.envoys.max(0));
        if minor.score >= 0 {
            game.observed_score.insert(owner, minor.score);
        }
        if minor.military.is_finite() && minor.military >= 0.0 {
            game.observed_military_power.insert(owner, minor.military);
        }
        let bond = (0, owner);
        if minor.at_war {
            game.at_war.insert(bond);
        } else {
            game.at_war.remove(&bond);
        }
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
            if plant_unit(&mut game, owner, unit, &mut unmapped, &mut dropped).is_some() {
                placed_rival_units += 1;
            }
        }
    }
    // The suzerain is public even when it is another major — and so is its
    // absence. Seed the delegations after every host id has a compact seat
    // mapping.
    for (minor, owner) in minor_assignments {
        seed_mirrored_suzerainty(&mut game, minor, owner, &seat_of_host);
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
    match barbarian_seat {
        Some(barb) => {
            for unit in &state.hostiles {
                if plant_unit(&mut game, barb, unit, &mut unmapped, &mut dropped).is_some() {
                    placed_rival_units += 1;
                }
            }
        }
        // ⚠ NEVER SKIP SILENTLY. A roster with no barbarian seat is a reconstruction
        // that cannot hold the threat list, and the planner has to be told rather
        // than left to read an empty board as a safe one.
        None => {
            for unit in &state.hostiles {
                dropped.push(format!("{}@{},{}:no_barbarian_seat", unit.kind, unit.x, unit.y));
            }
        }
    }

    // `place_city` applies native founding rules and clears removable features
    // from the centre. Firaxis is authoritative here: real city centres can
    // retain Floodplains, so restore every exported plot after all cities exist.
    apply_terrain(&mut game, snapshot);
    apply_territory(&mut game, snapshot, state);
    // ⚠ AFTER territory, not before. `apply_terrain` already recorded the seat's memory
    // of every revealed plot, but ownership is written here — so a memory taken earlier
    // would say every fogged tile is unowned, and `obs.rs` reads `memory.owner` for
    // exactly those tiles. Re-recording is idempotent and costs one pass over the
    // revealed set.
    apply_tile_memory(&mut game, snapshot);
    // ⚠ AFTER every city is planted, ours and the rivals', or the seat remembers only
    // the ones that happened to exist earlier in the rebuild.
    apply_city_memory(&mut game);

    // Firaxis leaves an active Trader on the map, while CIVVIS normally removes
    // it into `game.routes`.  Reconstruct the economic state here and retain the
    // physical unit above; the planner removes only active-route traders from its
    // temporary clone.
    unmapped.extend(restore_active_trade_routes(
        &mut game,
        &state.trade_routes,
        &known_city_ids,
    ));
    unmapped.extend(restore_incoming_foreign_routes(&mut game, &state.cities));
    apply_governor_state(&mut game, state, &mut unmapped);
    apply_great_person_points(&mut game, state, &mut unmapped);
    apply_strategic_stockpiles(&mut game, state, &mut unmapped);
    // The age and its Dedications change what the model pays (Heartbeat of
    // Steam's Campus Production, Free Inquiry's Science), so they must be on
    // the seat BEFORE the host-to-model corrections are measured, or the
    // correction is taken against a Normal-Age model and paid on top of a
    // Golden-Age one — Ravenna read 14.5 Science against the host's 9.5 on
    // run civvis-20260816T175306Z. The call at the end of this function
    // repeats it for the era score, which must be written after the cities
    // are planted; this early call is idempotent with it.
    apply_player_ages(&mut game, state);
    // The host's World Congress, likewise before the corrections: Trade Policy
    // and Luxury Policy change what the model pays and supplies.
    apply_host_congress(&mut game, state, &seat_of_host, &mut unmapped);
    apply_observed_host_metrics(&mut game, state, &mut unmapped);
    block_loyalty_doomed_settler_sites(&mut game);

    // Districts the host has refused to place, mapped onto CIVVIS's cities. Done here
    // because it needs `city_ids`, which is only complete once every city is planted.
    game.blocked_districts =
        blocked_districts_from(&state.refused_districts, &city_ids, &game.rules);
    game.host_district_sites =
        host_district_sites_from(&state.host_district_sites, &city_ids, &game.rules);
    game.host_wonder_sites =
        host_wonder_sites_from(&state.host_wonder_sites, &city_ids, &game.rules);
    game.blocked_wonders = blocked_wonders_from(&state.refused_wonders, &city_ids, &game.rules);
    game.host_unavailable_wonders =
        host_unavailable_wonders_from(&state.host_unavailable_wonders, &game.rules);
    let blocked_production =
        blocked_production_from(&state.refused_production, &city_ids, &game.rules);
    game.replace_blocked_production(blocked_production);
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
    game.blocked_promotions =
        blocked_promotions_from(&state.refused_promotions, &unit_ids, &game.rules);

    // ⚠ LAST, and deliberately so. Reconstruction founds this empire's cities on
    // the board, and founding a city AWARDS ERA SCORE — a four-city Rome
    // arrived at Firaxis's 31 plus five of CIVVIS's own. Firaxis's number is
    // the reading; anything the rebuild scored along the way is an artefact of
    // how the board was assembled, so the host's answer is written after it
    // rather than before.
    apply_player_ages(&mut game, state);

    record_host_observed(&mut game);
    Reconstruction {
        game,
        unit_ids,
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
fn record_host_observed(game: &mut crate::game::Game) {
    game.host_observed = game
        .units
        .values()
        .filter(|unit| unit.owner != crate::game::MIRRORED_SEAT)
        .map(|unit| unit.pos)
        .collect();
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

fn apply_territory(
    game: &mut crate::game::Game,
    snapshot: &Snapshot,
    state: &StateSnapshot,
) {
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
        centres.entry(city.owner).or_default().push((*cid, city.pos));
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
    game.blocked_city_sites.extend(blocked);
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
    let mut placed: std::collections::BTreeMap<u32, (Vec<(Name, crate::Pos)>, Vec<(Name, crate::Pos)>)> =
        Default::default();
    let mut any_seen: std::collections::BTreeSet<u32> = Default::default();
    for y in 0..snapshot.height.max(1) {
        for x in 0..snapshot.width.max(1) {
            let Some(plot) = snapshot.plot((x, y)) else { continue };
            if plot.d.is_none() && plot.wo.is_none() {
                continue;
            }
            let pos = crate::hex::offset_to_axial(x, y);
            let Some(cid) = game.map.get(pos).and_then(|tile| tile.owner_city) else { continue };
            let Some(city) = game.cities.get(&cid) else { continue };
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
        let Some(city) = game.cities.get_mut(&cid) else { continue };
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
            game.blocked_city_sites.insert(site);
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
        LiveMirror {
            game,
            civ6_of: rebuilt.unit_ids,
            uid_of,
            cid_of,
            known_city_ids: rebuilt.known_city_ids,
            active_trade_route_traders: active_trade_route_traders(state),
            rival_units: Vec::new(),
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
        let previous = self.last_treasury.replace((turn, gold));
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
        self.game
            .blocked_city_sites
            .extend(state.refused_sites.iter().copied());
        self.game
            .blocked_improvement_sites
            .extend(state.refused_improves.iter().copied());
        self.game
            .blocked_trade_routes
            .extend(state.refused_trade_routes.iter().copied());
        // Union for the same reason as the two above: a card the host retired stays
        // retired, and the set is rebuilt from the whole event log each time.
        let retired = blocked_policies_from(&state.refused_policy_names, &self.game.rules);
        self.game.blocked_policies.extend(retired);
        // And a pantheon a rival holds stays held.
        let taken = blocked_pantheons_from(&state.refused_pantheons, &self.game.rules);
        self.game.blocked_pantheons.extend(taken);
        let refused = blocked_districts_from(
            &state.refused_districts,
            &self.cid_of.iter().map(|(civ6, cid)| (*cid, *civ6)).collect(),
            &self.game.rules,
        );
        for (cid, names) in refused {
            self.game.blocked_districts.entry(cid).or_default().extend(names);
        }
        self.game.host_district_sites = host_district_sites_from(
            &state.host_district_sites,
            &self.cid_of.iter().map(|(civ6, cid)| (*cid, *civ6)).collect(),
            &self.game.rules,
        );
        self.game.host_wonder_sites = host_wonder_sites_from(
            &state.host_wonder_sites,
            &self.cid_of.iter().map(|(civ6, cid)| (*cid, *civ6)).collect(),
            &self.game.rules,
        );
        // The wonder half of the same event, unioned for the same reason.
        let refused_wonders = blocked_wonders_from(
            &state.refused_wonders,
            &self.cid_of.iter().map(|(civ6, cid)| (*cid, *civ6)).collect(),
            &self.game.rules,
        );
        for (cid, names) in refused_wonders {
            self.game.blocked_wonders.entry(cid).or_default().extend(names);
        }
        let unavailable_wonders =
            host_unavailable_wonders_from(&state.host_unavailable_wonders, &self.game.rules);
        self.game
            .host_unavailable_wonders
            .extend(unavailable_wonders);
        // Unlike impossible district plots, a production refusal can be temporary.
        // Replace this cooldown snapshot so entries disappear after their TTL.
        let blocked_production = blocked_production_from(
            &state.refused_production,
            &self.cid_of.iter().map(|(civ6, cid)| (*cid, *civ6)).collect(),
            &self.game.rules,
        );
        self.game.replace_blocked_production(blocked_production);
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
        self.game.blocked_promotions = blocked_promotions_from(
            &state.refused_promotions,
            &self.civ6_of,
            &self.game.rules,
        );
        // Rivals are met as the game goes on, so identity is not a one-time job at
        // reconstruction: a civilization first seen on turn 90 arrives here.
        apply_identity(&mut self.game, state);
        self.game.turn = state.turn.max(1);
        self.game.observed_score.clear();
        self.game.observed_military_power.clear();
        if state.score >= 0 {
            self.game.observed_score.insert(0, state.score);
        }
        if state.military.is_finite() && state.military >= 0.0 {
            self.game.observed_military_power.insert(0, state.military);
        }
        if state.gold >= 0 {
            self.game.players[0].gold = state.gold as f64;
            // The host's own figure first: it needs no history and so survives
            // `--fresh-board`, which is what kills the derived rate. Fall back to
            // the delta only when Firaxis did not answer.
            if let Some(net) = state.gold_per_turn.filter(|net| net.is_finite()) {
                self.game.players[0].gold_per_turn = net;
            } else if let Some(net) =
                self.mirror_net_income(self.game.turn, state.gold as f64)
            {
                self.game.players[0].gold_per_turn = net;
            }
        }
        if state.faith >= 0 {
            self.game.players[0].faith = state.faith as f64;
        }
        if let Some(dvp) = state.dvp {
            self.game.players[0].dvp = dvp;
        }
        apply_congress_dvp(&mut self.game, state);
        apply_host_competitions(&mut self.game, state);
        if let Some(favor) = state.favor.filter(|favor| favor.is_finite()) {
            self.game.players[0].diplomatic_favor = favor;
        }
        apply_mirrored_envoys_free(&mut self.game, state);
        apply_player_religion(&mut self.game, state, &mut self.unmapped);
        if let Some(civ6) = &state.government {
            if let Some(name) = civvis_node_name(&self.game.rules.governments, civ6, "GOVERNMENT_") {
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
            if let Some(name) =
                civvis_node_name(&self.game.rules.governments, civ6, "GOVERNMENT_")
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
                self.game.players[0].techs.insert(crate::name::Name::new(&name));
            }
        }
        if let Some(projects) =
            completed_strategic_projects(state.science_projects.as_deref(), &mut self.unmapped)
        {
            self.game.players[0].science_projects = projects;
        }
        for civ6 in &state.civics {
            if let Some(name) = civvis_node_name(&self.game.rules.civics, civ6, "CIVIC_") {
                self.game.players[0].civics.insert(crate::name::Name::new(&name));
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

        // Newly revealed ground, and the traversability prior redrawn beyond it.
        // Terrain that was already known does not change, but the frontier has to be
        // recomputed because its edge just moved.
        if !skip_terrain {
            apply_terrain(&mut self.game, snapshot);
            grow_frontier(&mut self.game, snapshot, frontier_depth);
        }
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
        // land it just took. It also under-reports the seat's own territory to every
        // consumer that reasons about it.
        //
        // It is cheap: one pass over the revealed set, the same work the rebuild does.
        apply_territory(&mut self.game, snapshot, state);
        // ⚠ Every sync, not just the rebuild. Rival cities are placed as they are
        // revealed, so a memory taken once at construction would hold only whatever
        // existed on turn 1 — the same staleness `apply_territory` above was fixed for.
        apply_city_memory(&mut self.game);

        // --- our units -------------------------------------------------------
        let mut seen: std::collections::BTreeSet<i64> = std::collections::BTreeSet::new();
        for unit in if skip_units { &[][..] } else { &state.units[..] } {
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
                    let progress = observed_unit_progress(
                        &self.game.rules,
                        unit,
                        &mut self.unmapped,
                    );
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
                self.rival_cities.remove(&crate::hex::axial_to_offset(city.pos.0, city.pos.1));
            }
            self.cid_of.remove(&host);
            self.known_city_ids.retain(|_, known| *known != cid);
            let captured = self.game.cities.get(&cid).is_some_and(|city| {
                foreign_city_positions.contains(&crate::hex::axial_to_offset(
                    city.pos.0,
                    city.pos.1,
                ))
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
                self.cid_of.retain(|host, mapped| *mapped != cid || *host == city.id);
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
                apply_observed_city_infrastructure(
                    &mut self.game,
                    *cid,
                    city,
                    &mut self.unmapped,
                );
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
                    // Firaxis exports the current item, not a speculative
                    // multi-item queue.  Clear even when the item is absent: a
                    // finished build is an empty queue in the real game, not the
                    // last thing CIVVIS happened to see.
                    live.queue.clear();
                    if let Some(item) = queued {
                        live.queue.push(item);
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
                        } else if civvis_node_name(
                            &self.game.rules.wonders, civ6, "BUILDING_"
                        ).is_none() {
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
        if let Some(barb) = self.game.barb_pid {
            for unit in &state.hostiles {
                let Some(name) = resolved_civvis_unit_name(&self.game.rules, &unit.kind) else {
                    // ⚠ Counted, not swallowed. A barbarian type CIVVIS cannot name is
                    // a threat it cannot see, and that is the whole of this defect.
                    if !self.unmapped.contains(&unit.kind) {
                        self.unmapped.push(unit.kind.clone());
                    }
                    continue;
                };
                let pos = crate::hex::offset_to_axial(unit.x, unit.y);
                if self.game.map.get(pos).is_none()
                    || self.game.units.values().any(|u| u.pos == pos)
                {
                    self.dropped_units
                        .push(format!("{}@{},{}:hostile_tile", unit.kind, unit.x, unit.y));
                    continue;
                }
                let uid = self.game.spawn_unit(&name, barb, pos);
                let progress = observed_unit_progress(
                    &self.game.rules,
                    unit,
                    &mut self.unmapped,
                );
                if let Some(live) = self.game.units.get_mut(&uid) {
                    apply_unit_observation(live, unit, progress);
                    self.hostile_units.push(uid);
                }
            }
        }

        for (index, rival) in state.rivals.iter().enumerate() {
            let owner = index + 1;
            if owner >= self.game.players.len() {
                break;
            }
            if rival.military.is_finite() && rival.military >= 0.0 {
                self.game
                    .observed_military_power
                    .insert(owner, rival.military);
            }
            if rival.score >= 0 {
                self.game.observed_score.insert(owner, rival.score);
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
            if rival.can_declare && !rival.at_war {
                self.game.players[0].denounced_until.insert(owner, self.game.turn + 1);
            } else {
                self.game.players[0].denounced_until.remove(&owner);
            }
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
                    let water = self.game.map.get(pos)
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
                apply_observed_city_infrastructure(
                    &mut self.game, cid, city, &mut self.unmapped,
                );
            }
            for unit in &rival.units {
                let Some(name) = resolved_civvis_unit_name(&self.game.rules, &unit.kind) else {
                    if !self.unmapped.contains(&unit.kind) {
                        self.unmapped.push(unit.kind.clone());
                    }
                    continue;
                };
                let pos = crate::hex::offset_to_axial(unit.x, unit.y);
                if self.game.map.get(pos).is_none() {
                    continue;
                }
                let uid = self.game.spawn_unit(&name, owner, pos);
                let progress = observed_unit_progress(
                    &self.game.rules,
                    unit,
                    &mut self.unmapped,
                );
                if let Some(live) = self.game.units.get_mut(&uid) {
                    apply_unit_observation(live, unit, progress);
                    self.rival_units.push(uid);
                }
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
        for owner in free_city_seats {
            self.game.players[owner].alive = false;
            self.game.at_war.remove(&(0, owner));
            self.game.observed_score.remove(&owner);
            self.game.observed_military_power.remove(&owner);
        }
        let minor_assignments = minor_actor_assignments(&self.game, state);
        for &(minor, owner) in &minor_assignments {
            seat_of_host.insert(minor.player, owner);
            if self.game.players[owner].is_free_city {
                self.game.players[owner].alive = true;
            }
            self.game.players[0].met.insert(owner);
            self.game.players[owner].met.insert(0);
            set_mirrored_envoys(&mut self.game.players[0], owner, minor.envoys.max(0));
            if minor.score >= 0 {
                self.game.observed_score.insert(owner, minor.score);
            }
            if minor.military.is_finite() && minor.military >= 0.0 {
                self.game.observed_military_power.insert(owner, minor.military);
            }
            if minor.at_war {
                self.game.at_war.insert((0, owner));
            } else {
                self.game.at_war.remove(&(0, owner));
            }
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
                        &mut self.game, cid, city, &mut self.unmapped,
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
                let pos = crate::hex::offset_to_axial(unit.x, unit.y);
                if self.game.map.get(pos).is_some()
                    && !self.game.units.values().any(|live| live.pos == pos)
                {
                    let uid = self.game.spawn_unit(&name, owner, pos);
                    let progress = observed_unit_progress(
                        &self.game.rules,
                        unit,
                        &mut self.unmapped,
                    );
                    if let Some(live) = self.game.units.get_mut(&uid) {
                        apply_unit_observation(live, unit, progress);
                        self.rival_units.push(uid);
                    }
                }
            }
        }
        for (minor, owner) in minor_assignments {
            seed_mirrored_suzerainty(&mut self.game, minor, owner, &seat_of_host);
        }

        self.active_trade_route_traders = active_trade_route_traders(state);
        for issue in restore_active_trade_routes(
            &mut self.game,
            &state.trade_routes,
            &self.known_city_ids,
        )
        .into_iter()
        .chain(restore_incoming_foreign_routes(&mut self.game, &state.cities))
        {
            if !self.unmapped.contains(&issue) {
                self.unmapped.push(issue);
            }
        }

        // City placement is a native CIVVIS action and may clear host terrain;
        // repeat the authoritative passes only after every new own/rival city is
        // present, then take fog memory from that final state.
        if !skip_terrain {
            apply_terrain(&mut self.game, snapshot);
            grow_frontier(&mut self.game, snapshot, frontier_depth);
        }
        apply_territory(&mut self.game, snapshot, state);
        apply_tile_memory(&mut self.game, snapshot);
        apply_city_memory(&mut self.game);
        apply_governor_state(&mut self.game, state, &mut self.unmapped);
        apply_great_person_points(&mut self.game, state, &mut self.unmapped);
        apply_strategic_stockpiles(&mut self.game, state, &mut self.unmapped);
        // Age and Dedications before the corrections are measured — see the
        // rebuild path for why; the trailing call repeats it for era score.
        apply_player_ages(&mut self.game, state);
        apply_host_congress(&mut self.game, state, &seat_of_host, &mut self.unmapped);
        apply_observed_host_metrics(&mut self.game, state, &mut self.unmapped);
        block_loyalty_doomed_settler_sites(&mut self.game);
        // After the city passes, for the same reason as on the rebuild path:
        // planting a city awards era score, and Firaxis's reading must be what
        // survives rather than what the sync happened to add on its way there.
        apply_player_ages(&mut self.game, state);
        // Last, because it reads the finished board: every rival, minor and
        // barbarian for this turn has been re-planted by now, and the previous
        // turn's sightings were removed with them.
        record_host_observed(&mut self.game);
    }
}

#[cfg(test)]
mod transient_refusal_tests {
    use super::*;

    /// Same temp-dir convention as the rest of this file's tests: `tempfile` is
    /// not a dependency of this crate.
    fn events(name: &str, lines: &[&str]) -> std::path::PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("civvis-refusal-{}-{}", name, std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("events.jsonl");
        std::fs::write(&path, lines.join("\n") + "\n").expect("write events");
        path
    }

    /// ⚠⚠⚠ `blocked_improvement_sites` is extended and NEVER cleared, so
    /// anything that reaches it is a permanent verdict on that ground.
    ///
    /// On run `civvis-20260811T103914Z` the builder had `movesRemaining == 0`
    /// on 25 of 26 refusals — a condition that clears itself next turn — and
    /// every one of those tiles was being blacklisted for the rest of the game.
    #[test]
    fn a_builder_out_of_moves_does_not_kill_the_tile_forever() {
        let p = events("outofmoves", &[
            r#"{"kind":"improve_refused","turn":5,"x":10,"y":12,"moves":0}"#,
        ]);
        let refused = refused_sites_of_kind_through(&p, "improve_refused", None);
        assert!(
            refused.is_empty(),
            "a builder that merely ran out of movement must not cost the empire \
             the tile: {refused:?}"
        );
    }

    /// The set still has to do its job. A refusal with movement left is the
    /// engine rejecting the GROUND, which is exactly what it exists to record.
    #[test]
    fn a_refusal_with_movement_left_still_blocks_the_tile() {
        let p = events("hasmoves", &[
            r#"{"kind":"improve_refused","turn":5,"x":10,"y":12,"moves":2}"#,
        ]);
        let refused = refused_sites_of_kind_through(&p, "improve_refused", None);
        assert_eq!(refused.len(), 1, "a genuine refusal must still block");
    }

    /// ⚠⚠ THE SAME FILTER HAS TO COVER `found_refused`, and that must be proven
    /// rather than assumed from the fact that one function serves both.
    ///
    /// `found_refused` feeds `blocked_city_sites`, which is also extended and
    /// never cleared. Across every live run of 2026-08-11, 9 found refusals: the
    /// settler had `movesRemaining == 0` on EIGHT. A condemned city site is the
    /// more expensive half of this defect — expansion is this project's measured
    /// binding constraint, with 36% of games ending on one city.
    #[test]
    fn a_settler_out_of_moves_does_not_kill_the_city_site_forever() {
        let p = events("settler", &[
            r#"{"kind":"found_refused","turn":9,"x":4,"y":7,"moves":0}"#,
            r#"{"kind":"found_refused","turn":9,"x":5,"y":8,"moves":2}"#,
        ]);
        let refused = refused_sites_of_kind_through(&p, "found_refused", None);
        assert_eq!(
            refused.len(),
            1,
            "the spent-move site must survive and the genuine refusal must \
             still block: {refused:?}"
        );
        assert!(refused.contains(&crate::hex::offset_to_axial(5, 8)));
    }

    /// ⚠ EACH CASE NEEDS ITS OWN FILE. `events` builds a path from the name and
    /// the process id, so four tests passing the same name share one events.jsonl
    /// and overwrite each other under `cargo test`'s parallelism. Mine did: the
    /// stale-improvement case failed in the full run and passed alone, which is
    /// the signature of a shared fixture rather than a logic error.
    fn improved_snapshot(name: &str, lines: &[&str]) -> Snapshot {
        let p = events(name, lines);
        snapshot_from_events_at(&p, None).expect("snapshot")
    }

    const SWEEP: &str = r#"{"kind":"tiles","turn":16,"width":4,"height":4,"chunk":1,"plots":[{"x":1,"y":1,"t":"TERRAIN_GRASS","o":0}]}"#;

    /// ★ The point: a finished improvement is on the board before the next sweep
    /// repeats it. 23 duplicate orders in one run came from this gap.
    #[test]
    fn a_finished_improvement_reaches_the_board_before_the_next_sweep() {
        let snap = improved_snapshot("improved_reaches", &[
            SWEEP,
            r#"{"kind":"improved","turn":18,"x":1,"y":1,"im":"IMPROVEMENT_MINE"}"#,
        ]);
        assert_eq!(
            snap.plot((1, 1)).and_then(|p| p.im.clone()),
            Some("IMPROVEMENT_MINE".to_string())
        );
    }

    /// ⚠ RULE 1: only `im` is touched. `from_chunks` REPLACES a plot, so the
    /// cheap version — folding a partial plot in as a one-plot chunk — would
    /// strip the tile's terrain and owner. This is why it is a field mutation.
    #[test]
    fn folding_an_improvement_keeps_the_rest_of_the_plot() {
        let snap = improved_snapshot("improved_keeps", &[
            SWEEP,
            r#"{"kind":"improved","turn":18,"x":1,"y":1,"im":"IMPROVEMENT_MINE"}"#,
        ]);
        let plot = snap.plot((1, 1)).expect("the plot survives");
        assert_eq!(plot.t.as_deref(), Some("TERRAIN_GRASS"), "terrain must survive");
        assert_eq!(plot.o, 0, "owner must survive");
    }

    /// ⚠ RULE 2: never invent ground. An improvement on a plot the seat has not
    /// revealed would hand the simulator information the seat does not have.
    #[test]
    fn an_improvement_on_unrevealed_ground_is_ignored() {
        let snap = improved_snapshot("improved_unseen", &[
            SWEEP,
            r#"{"kind":"improved","turn":18,"x":3,"y":3,"im":"IMPROVEMENT_MINE"}"#,
        ]);
        assert!(snap.plot((3, 3)).is_none(), "unseen ground stays unseen");
    }

    /// ⚠ RULE 3: an older event cannot override a fresher sweep — which is what
    /// keeps a removed improvement from coming back.
    #[test]
    fn a_stale_improvement_never_overrides_a_newer_sweep() {
        let snap = improved_snapshot("improved_stale", &[
            r#"{"kind":"improved","turn":5,"x":1,"y":1,"im":"IMPROVEMENT_MINE"}"#,
            SWEEP,
        ]);
        assert_eq!(
            snap.plot((1, 1)).and_then(|p| p.im.clone()),
            None,
            "the turn-16 sweep says bare and it is newer than the turn-5 event"
        );
    }

    /// ⚠⚠ `build_no_plot` already carries the discriminator and the block ignored
    /// it. `Game::blocked_districts` says zero offered plots means the district is
    /// impossible ANYWHERE (a Government Plaza that already exists), while above
    /// zero is "a placement disagreement in one city that must not stop the
    /// empire" — the district IS placeable there, CIVVIS just named a plot the
    /// engine would not take.
    ///
    /// Across every live run of 2026-08-11, 47 events: **41 had `offered > 0`**.
    #[test]
    fn a_wrong_plot_does_not_block_a_placeable_district() {
        let p = events("noplot", &[
            r#"{"kind":"build_no_plot","turn":40,"city":7,"district":"DISTRICT_CAMPUS","offered":4}"#,
            r#"{"kind":"build_no_plot","turn":41,"city":7,"district":"DISTRICT_GOVERNMENT","offered":0}"#,
        ]);
        let refused = refused_no_plot_through(&p, None, "district", "DISTRICT_");
        let blocked = refused.get(&7).expect("the impossible district still blocks");
        assert!(
            !blocked.contains("DISTRICT_CAMPUS"),
            "a Campus with four offered plots is placeable; only the tile was wrong"
        );
        assert!(
            blocked.contains("DISTRICT_GOVERNMENT"),
            "zero offered plots is the engine saying nowhere, and must still block"
        );
    }

    /// A zero-site wonder response is a world fact, not the city-local cooldown
    /// used for a wrong-coordinate response. Keep only explicit modern telemetry:
    /// an old event without `offered` cannot prove that the wonder is gone.
    #[test]
    fn a_zero_site_wonder_becomes_a_permanent_world_fact() {
        let p = events(
            "world_wonder",
            &[
                r#"{"kind":"build_no_plot","turn":40,"city":7,"building":"BUILDING_GREAT_BATH","offered":0}"#,
                r#"{"kind":"build_no_plot","turn":41,"city":8,"building":"BUILDING_PYRAMIDS","offered":2}"#,
                r#"{"kind":"build_no_plot","turn":42,"city":8,"building":"BUILDING_ORACLE"}"#,
                r#"{"kind":"build_no_plot","city":8,"building":"BUILDING_ORACLE","offered":0}"#,
                r#"{"kind":"build_no_plot","turn":43,"city":8,"building":"BUILDING_NOT_MODELED","offered":0}"#,
                r#"{"kind":"build_no_plot","turn":50,"city":8,"building":"BUILDING_HANGING_GARDENS","offered":0}"#,
                r#"{"kind":"state","turn":49}"#,
            ],
        );
        let state = state_from_events(&p, None).expect("state at the current turn");
        assert_eq!(
            state.host_unavailable_wonders,
            BTreeSet::from([
                "BUILDING_GREAT_BATH".to_string(),
                "BUILDING_NOT_MODELED".to_string(),
            ]),
            "only an explicit, timestamped zero-target answer before this board becomes a world fact"
        );
        assert_eq!(
            host_unavailable_wonders_from(
                &state.host_unavailable_wonders,
                &crate::rules::Rules::embedded(),
            ),
            BTreeSet::from([Name::new("great_bath")]),
            "unknown host names stay observable in the state but cannot populate a dead gate"
        );
    }

    /// ⚠⚠⚠ "Never block it" was the wrong half. #1555 dropped these refusals
    /// entirely and the very next full run showed the loop it recreated:
    /// `civvis-20260811T202458Z`, 28 `build_no_plot` events in 250 turns, **all
    /// 28 the same pair** — one Commercial Hub asked for and refused twenty-eight
    /// times because nothing remembered the previous twenty-seven.
    ///
    /// A fresh placement disagreement blocks, which ends the loop.
    #[test]
    fn a_fresh_placement_disagreement_blocks() {
        let p = events("noplot_fresh", &[
            r#"{"kind":"build_no_plot","turn":40,"city":7,"district":"DISTRICT_CAMPUS","offered":4}"#,
        ]);
        let refused = refused_no_plot_through(&p, Some(42), "district", "DISTRICT_");
        assert!(refused[&7].contains("DISTRICT_CAMPUS"), "or it is asked every turn");
    }

    /// The host already supplied the way out of a wrong-coordinate refusal. Keep
    /// only the latest fresh offer: an old positive answer must not override a newer
    /// zero-site answer, and neither belongs on a later board after the cooldown.
    #[test]
    fn fresh_host_district_sites_follow_the_newest_offer() {
        let p = events("host_sites", &[
            r#"{"kind":"build_no_plot","turn":40,"city":7,"district":"DISTRICT_CAMPUS","offered":2,"offered_plots":[{"x":10,"y":8}]}"#,
            r#"{"kind":"build_no_plot","turn":41,"city":7,"district":"DISTRICT_CAMPUS","offered":1,"offered_plots":[{"x":10,"y":7}]}"#,
            r#"{"kind":"build_no_plot","turn":42,"city":7,"district":"DISTRICT_THEATER","offered":1,"offered_plots":[{"x":9,"y":8}]}"#,
            r#"{"kind":"build_no_plot","turn":43,"city":7,"district":"DISTRICT_THEATER","offered":0,"offered_plots":[]}"#,
            r#"{"kind":"state","turn":49}"#,
        ]);
        let state = state_from_events(&p, Some(49)).expect("state at the current turn");
        let campus = state
            .host_district_sites
            .get(&7)
            .and_then(|by_district| by_district.get("DISTRICT_CAMPUS"))
            .expect("the latest positive Campus offer is fresh");
        assert_eq!(
            campus.iter().copied().collect::<Vec<_>>(),
            vec![crate::hex::offset_to_axial(10, 7)],
            "the newest host location replaces the older coordinate rather than merging it"
        );
        let city_ids: BTreeMap<u32, i64> = [(99, 7)].into_iter().collect();
        let mapped = host_district_sites_from(
            &state.host_district_sites,
            &city_ids,
            &crate::rules::Rules::embedded(),
        );
        assert_eq!(
            mapped
                .get(&99)
                .and_then(|by_district| by_district.get(&crate::name::Name::new("campus"))),
            Some(campus),
            "the CIV6 city/name pair must reach its reconstructed city and district"
        );
        assert!(
            state
                .host_district_sites
                .get(&7)
                .is_none_or(|by_district| !by_district.contains_key("DISTRICT_THEATER")),
            "a newer zero-site answer withdraws the previous positive offer"
        );
        assert!(
            host_district_sites_through(&p, 50).is_empty(),
            "a placement response older than the production cooldown is no longer current"
        );
    }

    /// Wonders carry their production type under `building`, rather than the
    /// district key. Their host candidates must still replace an invalid CIVVIS
    /// coordinate and vanish after a newer zero response or the normal TTL.
    #[test]
    fn fresh_host_wonder_sites_follow_the_newest_offer() {
        let p = events("host_wonder_sites", &[
            r#"{"kind":"build_no_plot","turn":40,"city":7,"building":"BUILDING_PYRAMIDS","offered":2,"offered_plots":[{"x":10,"y":8}]}"#,
            r#"{"kind":"build_no_plot","turn":41,"city":7,"building":"BUILDING_PYRAMIDS","offered":1,"offered_plots":[{"x":10,"y":7}]}"#,
            r#"{"kind":"build_no_plot","turn":42,"city":7,"building":"BUILDING_ORACLE","offered":1,"offered_plots":[{"x":9,"y":8}]}"#,
            r#"{"kind":"build_no_plot","turn":43,"city":7,"building":"BUILDING_ORACLE","offered":0,"offered_plots":[]}"#,
            r#"{"kind":"state","turn":49}"#,
        ]);
        let state = state_from_events(&p, Some(49)).expect("state at the current turn");
        let pyramids = state
            .host_wonder_sites
            .get(&7)
            .and_then(|by_wonder| by_wonder.get("BUILDING_PYRAMIDS"))
            .expect("the latest positive Pyramids offer is fresh");
        assert_eq!(
            pyramids.iter().copied().collect::<Vec<_>>(),
            vec![crate::hex::offset_to_axial(10, 7)],
            "the newest host location replaces the older coordinate rather than merging it"
        );
        let city_ids: BTreeMap<u32, i64> = [(99, 7)].into_iter().collect();
        let mapped = host_wonder_sites_from(
            &state.host_wonder_sites,
            &city_ids,
            &crate::rules::Rules::embedded(),
        );
        assert_eq!(
            mapped
                .get(&99)
                .and_then(|by_wonder| by_wonder.get(&crate::name::Name::new("pyramids"))),
            Some(pyramids),
            "the CIV6 city/name pair must reach its reconstructed city and wonder"
        );
        assert!(
            state
                .host_wonder_sites
                .get(&7)
                .is_none_or(|by_wonder| !by_wonder.contains_key("BUILDING_ORACLE")),
            "a newer zero-site answer withdraws the previous positive offer"
        );
        assert!(
            host_wonder_sites_through(&p, 50).is_empty(),
            "a placement response older than the production cooldown is no longer current"
        );
    }

    /// And expires, which is what keeps the district from being foreclosed in a
    /// city that may yet make room for it — the reason #1555 existed at all.
    #[test]
    fn a_stale_placement_disagreement_stops_blocking() {
        let p = events("noplot_stale", &[
            r#"{"kind":"build_no_plot","turn":40,"city":7,"district":"DISTRICT_CAMPUS","offered":4}"#,
        ]);
        let refused = refused_no_plot_through(
            &p, Some(40 + PRODUCTION_REFUSAL_TTL + 1), "district", "DISTRICT_");
        assert!(
            refused.get(&7).is_none_or(|d| !d.contains("DISTRICT_CAMPUS")),
            "a placement disagreement must not condemn the city forever"
        );
    }

    /// ⚠ Zero offered plots is a different statement — the engine has no target
    /// ANYWHERE, a Government Plaza that already exists — and that does not go
    /// stale. It must still block long after the TTL.
    #[test]
    fn no_plot_anywhere_still_blocks_forever() {
        let p = events("noplot_never", &[
            r#"{"kind":"build_no_plot","turn":40,"city":7,"district":"DISTRICT_GOVERNMENT","offered":0}"#,
        ]);
        let refused = refused_no_plot_through(
            &p, Some(40 + PRODUCTION_REFUSAL_TTL * 10), "district", "DISTRICT_");
        assert!(refused[&7].contains("DISTRICT_GOVERNMENT"));
    }

    /// An absent `offered` is not a reading — older exports sent none, and those
    /// must keep the old behaviour so a replayed run is unchanged.
    #[test]
    fn a_no_plot_event_without_offered_keeps_the_old_behaviour() {
        let p = events("noplot_old", &[
            r#"{"kind":"build_no_plot","turn":40,"city":7,"district":"DISTRICT_CAMPUS"}"#,
        ]);
        let refused = refused_no_plot_through(&p, None, "district", "DISTRICT_");
        assert!(refused[&7].contains("DISTRICT_CAMPUS"));
    }

    /// ⚠ Events written before #1548 carry no `moves`, and an absent reading is
    /// not evidence of anything. Replaying an older run must be unchanged.
    #[test]
    fn a_refusal_that_never_recorded_moves_keeps_the_old_behaviour() {
        let p = events("nomovesfield", &[
            r#"{"kind":"improve_refused","turn":5,"x":10,"y":12}"#,
        ]);
        let refused = refused_sites_of_kind_through(&p, "improve_refused", None);
        assert_eq!(refused.len(), 1, "no reading is not a transient reading");
    }
}

#[cfg(test)]
mod host_fact_tests {
    use super::*;

    fn host_grass(x: i32, y: i32) -> Plot {
        Plot {
            x,
            y,
            t: Some("TERRAIN_GRASS".to_string()),
            f: None,
            r: None,
            o: 0,
            w: false,
            i: false,
            fw: false,
            im: None,
            rv: 0,
            ri: false,
            ct: None,
            cl: -1,
            p: false,
            d: None,
            dc: None,
            wo: None,
            rt: None,
            rp: false,
        }
    }

    /// World Games is one of the ways a diplomatic race moves without a vote:
    /// Firaxis grants `PROJECT_TRAIN_ATHLETES` only to active members, and the
    /// bridge used to discard the exact tracker that says so.
    #[test]
    fn world_games_tracker_opens_then_retires_the_host_project() {
        let raw = r#"{
            "turn": 182,
            "emergencies": [{
                "type": "EMERGENCY_WORLD_GAMES",
                "target": 2,
                "turns_left": 8,
                "begun": true,
                "scores": [
                    {"player": 0, "score": 50, "tier": 2},
                    {"player": 2, "score": 100, "tier": 1}
                ],
                "ours": {"member": true, "score": 50, "tier": 2}
            }]
        }"#;
        let mut state = state_from_json(raw).expect("the competition tracker parses");
        assert!(
            state.schema_gaps.is_empty(),
            "the recognized tracker must not be filed as discarded schema: {:?}",
            state.schema_gaps
        );
        assert_eq!(state.emergencies.as_ref().unwrap()[0].target, 2);
        state.cities.push(StateCity {
            id: 1,
            name: "Rome".to_string(),
            x: 3,
            y: 3,
            pop: 5,
            capital: true,
            ..StateCity::default()
        });
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: state.turn,
            width: 8,
            height: 8,
            chunk: 1,
            plots: vec![host_grass(3, 3)],
        }]);
        let mut mirror = LiveMirror::new(&snapshot, &state, 2, 1, 250, 0);
        let city = mirror.game.player_city_ids(0)[0];
        let athletes = crate::game::Item::Project {
            project: crate::name!("train_athletes"),
        };
        let competition = mirror
            .game
            .host_competition(0, "EMERGENCY_WORLD_GAMES")
            .expect("our active World Games score race reaches the board");
        assert_eq!(competition.ours, 50.0);
        assert_eq!(competition.leader, 100.0);
        assert!(
            mirror.game.can_produce(0, city, &athletes),
            "an active member can run the host-granted athlete project"
        );
        assert!(
            mirror.game.producible_items(0, city).contains(&athletes),
            "the active project appears even after the menu is cached"
        );

        // An older control mod did not export this field. Its omission cannot
        // be treated as a completed event, because a persistent mirror may
        // have learned about World Games before the mod was refreshed.
        let completed = state.emergencies.take();
        state.turn += 1;
        mirror.sync(&snapshot, &state, 0);
        assert!(mirror.game.can_produce(0, city, &athletes));

        // `TurnsLeft < 0` is the host's completed marker. It must withdraw
        // the project and invalidate the menu cached above rather than leave
        // CIVVIS repeatedly ordering a project Firaxis no longer accepts.
        state.turn += 1;
        state.emergencies = completed;
        state.emergencies.as_mut().unwrap()[0].turns_left = -1;
        mirror.sync(&snapshot, &state, 0);
        assert!(mirror
            .game
            .host_competition(0, "EMERGENCY_WORLD_GAMES")
            .is_none());
        assert!(!mirror.game.can_produce(0, city, &athletes));
        assert!(!mirror.game.producible_items(0, city).contains(&athletes));

        // A fresh non-member board is equally closed: the project is a host
        // effect, not part of CIVVIS's ordinary ruleset.
        state.emergencies.as_mut().unwrap()[0].turns_left = 8;
        state.emergencies.as_mut().unwrap()[0].ours.member = false;
        let inactive = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0).game;
        let inactive_city = inactive.player_city_ids(0)[0];
        assert!(!inactive.can_produce(0, inactive_city, &athletes));
    }

    /// Civilization VI's production names must reach CIVVIS's queue as real items.
    ///
    /// ⚠ The export shipped a raw HASH for the whole project, so this path was dead
    /// and every city read as idle — CIVVIS then chose production from scratch each
    /// turn, blind to work already underway.
    #[test]
    fn civ6_production_names_become_civvis_queue_items() {
        let rules = crate::rules::Rules::shared();
        let settler = civvis_production_item(&rules, Some("UNIT_SETTLER"), &[], None);
        assert!(
            matches!(settler, Some(crate::game::Item::Unit { .. })),
            "UNIT_SETTLER should map to a CIVVIS unit build, got {settler:?}"
        );
        let monument = civvis_production_item(&rules, Some("BUILDING_MONUMENT"), &[], None);
        assert!(
            matches!(monument, Some(crate::game::Item::Building { .. })),
            "BUILDING_MONUMENT should map to a CIVVIS building, got {monument:?}"
        );
        let theater =
            civvis_production_item(&rules, Some("PROJECT_ENHANCE_DISTRICT_THEATER"), &[], None);
        assert_eq!(
            theater,
            Some(crate::game::Item::Project {
                project: crate::name!("theater_square_festival"),
            })
        );
        assert_eq!(
            civvis_production_item(&rules, Some("PROJECT_TRAIN_ATHLETES"), &[], None),
            Some(crate::game::Item::Project {
                project: crate::name!("train_athletes"),
            }),
            "the host's active World Games queue must remain visibly committed"
        );

        // ⚠ Refusing to guess is the point. A wrong item tells CIVVIS a city is busy
        // with something it is not, which SUPPRESSES a real production decision —
        // worse than the repeated one this fixes.
        assert!(civvis_production_item(&rules, Some("UNIT_NOT_A_REAL_THING"), &[], None).is_none());
        assert!(civvis_production_item(&rules, Some(""), &[], None).is_none());
        assert!(civvis_production_item(&rules, None, &[], None).is_none());
        // A district still refuses when the export did not say WHERE — inventing a
        // plot would place it on arbitrary ground, which is the one thing worse
        // than repeating the order.
        assert!(civvis_production_item(&rules, Some("DISTRICT_CAMPUS"), &[], None).is_none());
        // ...and resolves once the plot is carried, which is what stops a city
        // building a district from reading as idle for sixty turns.
        let campus = civvis_production_item(
            &rules,
            Some("DISTRICT_CAMPUS"),
            &[StateDistrict {
                kind: "DISTRICT_CAMPUS".into(),
                x: 12,
                y: 7,
                pillaged: false,
                complete: false,
                ..StateDistrict::default()
            }],
            None,
        );
        match campus {
            // ⚠ AXIAL, not the offset the export sent. Mixing the two is this
            // bridge's oldest trap and nothing complains, because both are pairs of
            // small integers.
            Some(crate::game::Item::District { pos, .. }) => {
                assert_eq!(pos, crate::hex::offset_to_axial(12, 7));
            }
            other => panic!("a district with a plot should be an Item::District: {other:?}"),
        }
        // A plot for a DIFFERENT district does not answer for this one.
        assert!(civvis_production_item(
            &rules,
            Some("DISTRICT_CAMPUS"),
            &[StateDistrict {
                kind: "DISTRICT_HOLY_SITE".into(),
                x: 3,
                y: 4,
                pillaged: false,
                complete: false,
                ..StateDistrict::default()
            }],
            None,
        )
        .is_none());

        // ★ A wonder under construction is a busy city. `BUILDING_HAGIA_SOPHIA` is
        // not a `rules.buildings` row and used to fall through to None — the first
        // live wonder the seat ever started was replaced by a University the next
        // turn because the mirror seeded Rome's queue empty. With a centre it is a
        // placed marker; without one (block-key translation) it still names the
        // wonder.
        let centre = crate::hex::offset_to_axial(20, 9);
        match civvis_production_item(&rules, Some("BUILDING_HAGIA_SOPHIA"), &[], Some(centre)) {
            Some(crate::game::Item::Wonder { wonder, pos }) => {
                assert_eq!(wonder, crate::name!("hagia_sophia"));
                assert_eq!(pos, centre, "the placeholder plot is the city centre");
            }
            other => panic!("an in-progress wonder should be an Item::Wonder: {other:?}"),
        }
        assert!(matches!(
            civvis_production_item(&rules, Some("BUILDING_HAGIA_SOPHIA"), &[], None),
            Some(crate::game::Item::Wonder { .. })
        ));
        // And an ordinary building is still a building, not a wonder.
        assert!(matches!(
            civvis_production_item(&rules, Some("BUILDING_LIBRARY"), &[], Some(centre)),
            Some(crate::game::Item::Building { .. })
        ));
    }

    /// ⚠ THE REGRESSION THIS EXISTS TO PREVENT, PINNED AS A TEST.
    ///
    /// The mod's `encode` emits `[]` for an empty Lua table — it takes the array
    /// branch whenever `#v == n`, and an empty table satisfies that with both
    /// zero. `great_person_points` shipped in #983 as a plain `BTreeMap`, every
    /// player has no Great Person points on turn 1, and serde refusing a
    /// sequence took **the whole StateSnapshot** down with it, not just the
    /// field. Three consecutive live attempts reported "no revealed terrain or
    /// no state yet" and 0 orders from turn 1, stalled at turn 6 on an
    /// unanswered research prompt, and were killed by the watchdog.
    ///
    /// The empty array must parse, and — this is the part that actually
    /// mattered — everything *around* it must survive.
    #[test]
    fn an_empty_great_person_table_arrives_as_a_json_array_and_must_not_lose_the_board() {
        let raw = r#"{"turn": 92, "gold": 140, "science": 7.5,
                      "great_person_points": [],
                      "great_person_offers": [],
                      "techs": ["TECH_POTTERY"]}"#;
        let state: StateSnapshot =
            serde_json::from_str(raw).expect("an empty map encoded as [] must still parse");
        assert_eq!(
            state.great_person_points,
            Some(BTreeMap::new()),
            "an empty array is an empty race, not a missing field"
        );
        assert_eq!(state.turn, 92, "and the rest of the board must survive it");
        assert_eq!(state.gold, 140);
        assert_eq!(state.techs, vec!["TECH_POTTERY".to_string()]);
        assert!(
            state
                .great_person_offers
                .as_ref()
                .is_some_and(BTreeMap::is_empty),
            "the same Lua empty-map trap must not lose a new named-offer field"
        );
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 92,
            width: 4,
            height: 4,
            chunk: 1,
            plots: vec![host_grass(2, 2)],
        }]);
        let rebuilt = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0).game;
        assert_eq!(
            rebuilt.players[0].live_great_person_offers,
            Some(BTreeSet::new()),
            "an empty host table means no class is recruitable, not an old export"
        );
        assert!(
            !rebuilt.great_person_class_offered_now(0, "scientist"),
            "the native roster must not reopen a truly empty host screen"
        );

        // The populated and absent forms keep working.
        let populated: StateSnapshot = serde_json::from_str(
            r#"{"turn": 3, "great_person_points": {"GREAT_PERSON_CLASS_SCIENTIST": 18.0}}"#,
        )
        .expect("a populated map parses");
        assert_eq!(
            populated.great_person_points.unwrap()["GREAT_PERSON_CLASS_SCIENTIST"],
            18.0
        );
        let absent: StateSnapshot =
            serde_json::from_str(r#"{"turn": 3}"#).expect("an absent field parses");
        assert_eq!(absent.great_person_points, None);
    }

    /// Housing must survive the wire, including the empty and absent forms.
    ///
    /// This is the field that gates the population every yield is a linear
    /// function of, so it is worth pinning that it parses rather than assuming
    /// it — the last host field I added took every live game down because an
    /// empty value serialised in a shape serde would not read (#983 → #996).
    /// ⚠ The eureka discount must survive the wire, and an older mod that sends
    /// neither field must still parse — a hard error here takes the WHOLE
    /// StateSnapshot down, not just this field (#983 → #996).
    #[test]
    fn the_eureka_reaches_the_planner_from_the_host() {
        let raw = r#"{"turn": 40, "techs": ["TECH_POTTERY"],
                      "boosted_techs": ["TECH_WRITING", "TECH_MASONRY"],
                      "boosted_civics": ["CIVIC_CRAFTSMANSHIP"]}"#;
        let state: StateSnapshot = serde_json::from_str(raw).expect("boosts parse");
        assert_eq!(state.boosted_techs, ["TECH_WRITING", "TECH_MASONRY"]);
        assert_eq!(state.boosted_civics, ["CIVIC_CRAFTSMANSHIP"]);

        // An empty list is the ordinary case on turn 1 and must be a SEQUENCE.
        let empty: StateSnapshot =
            serde_json::from_str(r#"{"turn": 1, "boosted_techs": [], "boosted_civics": []}"#)
                .expect("an empty boost list parses");
        assert!(empty.boosted_techs.is_empty());

        // And an older mod that sends neither field still parses.
        let absent: StateSnapshot =
            serde_json::from_str(r#"{"turn": 1}"#).expect("an older mod still parses");
        assert!(absent.boosted_techs.is_empty());
        assert!(absent.boosted_civics.is_empty());
    }

    /// A completed strategic project disappears from every city queue, so this
    /// must be a player-history field rather than an inference from production.
    ///
    /// On the turn-251 supervised live game, the fresh board saw zero completed
    /// projects and spent five cities' production repeatedly on Manhattan Project.
    /// The live export needs to preserve the host's player-wide completion ledger
    /// so the existing science and nuclear-roadmap gates can skip it.
    #[test]
    fn completed_strategic_projects_cross_the_live_bridge_without_false_mars_progress() {
        let raw = r#"{"turn": 205, "science_projects": [
            "PROJECT_MANHATTAN_PROJECT",
            "PROJECT_OPERATION_IVY",
            "PROJECT_LAUNCH_EARTH_SATELLITE",
            "PROJECT_LAUNCH_MOON_LANDING",
            "PROJECT_LAUNCH_MARS_BASE",
            "PROJECT_LAUNCH_EXOPLANET_EXPEDITION"
        ]}"#;
        let mut state = state_from_json(raw).expect("the strategic project wire parses");
        assert!(state.schema_gaps.is_empty(), "the new wire key is recognized");
        assert_eq!(
            state.science_projects,
            Some(vec![
                "PROJECT_MANHATTAN_PROJECT".to_string(),
                "PROJECT_OPERATION_IVY".to_string(),
                "PROJECT_LAUNCH_EARTH_SATELLITE".to_string(),
                "PROJECT_LAUNCH_MOON_LANDING".to_string(),
                "PROJECT_LAUNCH_MARS_BASE".to_string(),
                "PROJECT_LAUNCH_EXOPLANET_EXPEDITION".to_string(),
            ])
        );

        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 205,
            width: 8,
            height: 8,
            chunk: 1,
            plots: vec![host_grass(3, 3)],
        }]);
        let expected = BTreeSet::from([
            "manhattan_project".to_string(),
            "operation_ivy".to_string(),
            "launch_earth_satellite".to_string(),
            "launch_moon_landing".to_string(),
            "launch_mars_colony".to_string(),
            "exoplanet_expedition".to_string(),
        ]);
        let rebuilt = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
        assert_eq!(rebuilt.game.players[0].science_projects, expected);
        assert!(
            !rebuilt
                .unmapped
                .iter()
                .any(|issue| issue.starts_with("science_project:")),
            "every strategic project on the supported wire must survive: {:?}",
            rebuilt.unmapped
        );

        // The persistent path must use the same truth and retain it if an older
        // mod is later reloaded and does not yet know the field.
        let before_export = StateSnapshot {
            turn: 204,
            ..StateSnapshot::default()
        };
        let mut mirror = LiveMirror::new(&snapshot, &before_export, 2, 1, 250, 0);
        mirror.sync(&snapshot, &state, 0);
        assert_eq!(mirror.game.players[0].science_projects, expected);
        state.turn += 1;
        state.science_projects = None;
        mirror.sync(&snapshot, &state, 0);
        assert_eq!(
            mirror.game.players[0].science_projects, expected,
            "an absent field means an older mod, not that history was erased"
        );

        // Base Civ VI reports Mars as three independent components. CIVVIS has
        // one Mars-colony milestone, so two components are progress but not
        // completion; all three are the one truthful completion transition.
        let partial_mars = vec![
            "PROJECT_LAUNCH_MARS_REACTOR".to_string(),
            "PROJECT_LAUNCH_MARS_HABITATION".to_string(),
        ];
        let mut issues = Vec::new();
        let partial = completed_strategic_projects(Some(&partial_mars), &mut issues)
            .expect("an explicit host list answers");
        assert!(issues.is_empty());
        assert!(!partial.contains("launch_mars_colony"));
        let full_mars = vec![
            "PROJECT_LAUNCH_MARS_REACTOR".to_string(),
            "PROJECT_LAUNCH_MARS_HABITATION".to_string(),
            "PROJECT_LAUNCH_MARS_HYDROPONICS".to_string(),
        ];
        assert!(completed_strategic_projects(Some(&full_mars), &mut issues)
            .expect("an explicit host list answers")
            .contains("launch_mars_colony"));
    }

    #[test]
    fn housing_reaches_the_planner_from_the_host() {
        let raw = r#"{"id": 1, "x": 3, "y": 4, "pop": 12,
                      "housing": 14.0, "housing_from_improvements": 5.0}"#;
        let city: StateCity = serde_json::from_str(raw).expect("housing parses");
        assert_eq!(city.housing, Some(14.0));
        assert_eq!(city.housing_from_improvements, Some(5.0));
        assert_eq!(city.pop, 12, "and the rest of the city survives it");

        // A host that cannot answer sends -1 through `try`, and an older mod
        // sends nothing at all. Neither may cost us the city.
        let refused: StateCity =
            serde_json::from_str(r#"{"id": 1, "x": 3, "y": 4, "pop": 12, "housing": -1}"#)
                .expect("a refused housing read still parses");
        assert_eq!(refused.housing, Some(-1.0));
        assert_eq!(refused.pop, 12);

        let absent: StateCity =
            serde_json::from_str(r#"{"id": 1, "x": 3, "y": 4, "pop": 12}"#)
                .expect("an older mod that sends no housing still parses");
        assert_eq!(absent.housing, None);
        assert_eq!(absent.pop, 12);
    }

    /// The Great Person race the planner prices against must actually exist.
    ///
    /// `district_project_value` reads `players[pid].gpp` for this empire and
    /// every rival, and awards up to 150 for closing on a leader and 240 for
    /// overtaking one. Before this the field was never written from a live
    /// game, so both sides of every one of those comparisons were 0.0 — which
    /// is why the Campus research project, whose entire payoff is Great
    /// Scientist points, was chosen 7 times against 131 for the other district
    /// projects across five live runs.
    #[test]
    fn great_person_points_reach_the_planner_from_the_host() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 92,
            width: 8,
            height: 8,
            chunk: 1,
            plots: vec![host_grass(3, 3)],
        }]);
        let mut points = BTreeMap::new();
        points.insert("GREAT_PERSON_CLASS_SCIENTIST".to_string(), 118.0);
        points.insert("GREAT_PERSON_CLASS_WRITER".to_string(), 12.5);
        // A class Civilization VI could add that CIVVIS has never heard of must
        // be reported, not silently dropped.
        points.insert("GREAT_PERSON_CLASS_ASTRONAUT".to_string(), 4.0);
        points.insert("NOT_A_GREAT_PERSON_CLASS".to_string(), 9.0);
        let state = StateSnapshot {
            turn: 92,
            great_person_points: Some(points),
            ..StateSnapshot::default()
        };
        let report = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
        let game = &report.game;

        assert_eq!(
            game.players[0].gpp.get("scientist").copied(),
            Some(118.0),
            "the Scientist race is what the Campus project is played for"
        );
        assert_eq!(game.players[0].gpp.get("writer").copied(), Some(12.5));
        assert_eq!(
            game.players[0].gpp.get("astronaut").copied(),
            Some(4.0),
            "an unfamiliar class still translates by its suffix"
        );
        assert!(
            report
                .unmapped
                .iter()
                .any(|issue| issue.contains("NOT_A_GREAT_PERSON_CLASS")),
            "a class that does not carry the prefix must be reported: {:?}",
            report.unmapped
        );
        assert!(
            !game.players[0].gpp.contains_key("not_a_great_person_class"),
            "and must not be invented into the race"
        );
    }

    /// The live recruit COST must land on the class's current person, so the
    /// planner's `gp_cost - points` gate answers with the live game's number.
    /// Run civvis-20260815T033823Z: 45 `gp_cannot_recruit` refusals because
    /// the ask was priced by CIVVIS's market formula instead of the timeline
    /// the order is judged by.
    #[test]
    fn live_recruit_costs_reprice_the_current_great_person() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 92,
            width: 8,
            height: 8,
            chunk: 1,
            plots: vec![host_grass(3, 3)],
        }]);
        let mut costs = BTreeMap::new();
        costs.insert("GREAT_PERSON_CLASS_SCIENTIST".to_string(), 385.0);
        costs.insert("NOT_A_GREAT_PERSON_CLASS".to_string(), 9.0);
        let state = StateSnapshot {
            turn: 92,
            great_person_costs: Some(costs),
            ..StateSnapshot::default()
        };
        let report = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
        let game = &report.game;

        assert_eq!(
            game.gp_cost(0, "scientist"),
            385.0,
            "the gate must quote the live timeline, not the market formula"
        );
        assert!(
            report
                .unmapped
                .iter()
                .any(|issue| issue.contains("great_person_cost_class:NOT_A_GREAT_PERSON_CLASS")),
            "an unprefixed class must be reported: {:?}",
            report.unmapped
        );

        // An older mod that sends no costs must parse to None, so the import
        // never runs and the engine's own offer pricing stays in charge. (The
        // offer map itself is NOT empty after a rebuild — the engine prices
        // its own market — so absence is asserted on the wire, not the map.)
        let bare: StateSnapshot =
            serde_json::from_str(r#"{"turn": 3}"#).expect("an absent field parses");
        assert_eq!(bare.great_person_costs, None);
    }

    #[test]
    fn physical_great_people_without_activation_plots_reach_production_planning() {
        let mut game = crate::game::Game::new_full(1, 20, 14, 95_104, 80, 0, false);
        let mut state = StateSnapshot {
            units: vec![StateUnit {
                id: 77,
                kind: "UNIT_GREAT_SCIENTIST".to_string(),
                great_person: Some(StateGreatPerson {
                    individual: Some(
                        "GREAT_PERSON_INDIVIDUAL_HILDEGARD_OF_BINGEN".to_string(),
                    ),
                    class: Some("GREAT_PERSON_CLASS_SCIENTIST".to_string()),
                    required_district: Some("DISTRICT_HOLY_SITE".to_string()),
                    charges: 1,
                    can_activate: false,
                    activation_plots: Vec::new(),
                    empty_slots: None,
                }),
                ..StateUnit::default()
            }],
            ..StateSnapshot::default()
        };
        let mut unmapped = Vec::new();

        apply_great_person_points(&mut game, &state, &mut unmapped);

        assert!(unmapped.is_empty(), "the stock class and district both map");
        assert_eq!(game.players[0].live_great_person_activation_needs.len(), 1);
        let need = &game.players[0].live_great_person_activation_needs[0];
        assert_eq!(need.kind, "scientist");
        assert_eq!(need.individual.as_deref(), Some("hildegard_of_bingen"));
        assert_eq!(need.required_district.as_deref(), Some("holy_site"));

        state.units[0]
            .great_person
            .as_mut()
            .unwrap()
            .activation_plots
            .push(StateActivationPlot {
                x: 8,
                y: 5,
                distance: 2,
                ..StateActivationPlot::default()
            });
        apply_great_person_points(&mut game, &state, &mut unmapped);
        assert!(
            game.players[0]
                .live_great_person_activation_needs
                .is_empty(),
            "a host-valid destination clears the production demand immediately"
        );
    }

    /// A highlighted plot is a *place*, not a *use*. Firaxis highlights a
    /// cultural person's district whether or not a compatible Great Work slot
    /// is free, so seven Writers/Artists/Musicians stood on one Theater plot
    /// for thirty-plus turns on run civvis-20260817T010950Z while the old
    /// plots-non-empty gate read them as needing nothing. The host's own
    /// empty-slot count is the tiebreaker: zero compatible slots anywhere is
    /// a production need exactly as surely as no plot at all.
    #[test]
    fn a_slot_starved_person_with_highlighted_plots_is_still_a_need() {
        let mut game = crate::game::Game::new_full(1, 20, 14, 95_104, 80, 0, false);
        let person = |empty_slots: Option<u32>, can_activate: bool| StateGreatPerson {
            individual: Some("GREAT_PERSON_INDIVIDUAL_MARK_TWAIN".to_string()),
            class: Some("GREAT_PERSON_CLASS_WRITER".to_string()),
            required_district: None,
            charges: 0,
            can_activate,
            activation_plots: vec![StateActivationPlot {
                x: 25,
                y: 23,
                distance: 0,
                ..StateActivationPlot::default()
            }],
            empty_slots,
        };
        let mut state = StateSnapshot {
            units: vec![StateUnit {
                id: 90,
                kind: "UNIT_GREAT_WRITER".to_string(),
                great_person: Some(person(Some(0), false)),
                ..StateUnit::default()
            }],
            ..StateSnapshot::default()
        };
        let mut unmapped = Vec::new();

        apply_great_person_points(&mut game, &state, &mut unmapped);
        assert_eq!(
            game.players[0].live_great_person_activation_needs.len(),
            1,
            "zero empty slots with highlighted plots is a need"
        );
        assert_eq!(
            game.players[0].live_great_person_activation_needs[0].kind,
            "writer"
        );

        // Slots free: the highlighted plot really is actionable — no need.
        state.units[0].great_person = Some(person(Some(3), false));
        apply_great_person_points(&mut game, &state, &mut unmapped);
        assert!(game.players[0]
            .live_great_person_activation_needs
            .is_empty());

        // An older mod that cannot count slots sends nothing: old behaviour,
        // no need while plots are listed.
        state.units[0].great_person = Some(person(None, false));
        apply_great_person_points(&mut game, &state, &mut unmapped);
        assert!(game.players[0].live_great_person_activation_needs.is_empty());

        // And the host saying "activate now" outranks its slot arithmetic.
        state.units[0].great_person = Some(person(Some(0), true));
        apply_great_person_points(&mut game, &state, &mut unmapped);
        assert!(game.players[0].live_great_person_activation_needs.is_empty());
    }

    /// The nine Great People of live run `civvis-20260822T020434Z`, and the
    /// gap they fell through.
    ///
    /// Three Artists, three Writers, three Musicians and a Scientist stood in
    /// Rome at turn 231 with NOT ONE ORDER between them in the whole game.
    /// The test above closed the `empty_slots == Some(0)` case; these nine
    /// were never in it. Their exports read **24, 4 and 2** empty slots —
    /// compatible slots the EMPIRE owns — while every plot the host offered
    /// them read `slot_open: false`, tile by tile: nowhere this person can
    /// put a work. The needs machinery saw a non-empty plot list and a
    /// non-zero count and concluded there was nothing to build, so no city
    /// ever started the Amphitheater or Museum that would have seated them.
    #[test]
    fn every_offered_plot_full_is_a_need_however_many_slots_the_empire_owns() {
        let mut game = crate::game::Game::new_full(1, 20, 14, 95_104, 80, 0, false);
        // As exported at turn 231: three of the Writer's plots, all closed.
        let closed = |x: i32, y: i32, distance: i32| StateActivationPlot {
            x,
            y,
            distance,
            slot_open: Some(false),
        };
        let writer = |empty_slots: Option<u32>| StateGreatPerson {
            individual: Some("GREAT_PERSON_INDIVIDUAL_HG_WELLS".to_string()),
            class: Some("GREAT_PERSON_CLASS_WRITER".to_string()),
            required_district: None,
            charges: 0,
            can_activate: false,
            activation_plots: vec![closed(67, 14, 12), closed(65, 25, 1), closed(64, 27, 2)],
            empty_slots,
        };
        let mut state = StateSnapshot {
            units: vec![StateUnit {
                id: 10_092_559,
                kind: "UNIT_GREAT_WRITER".to_string(),
                great_person: Some(writer(Some(24))),
                ..StateUnit::default()
            }],
            ..StateSnapshot::default()
        };
        let mut unmapped = Vec::new();

        apply_great_person_points(&mut game, &state, &mut unmapped);
        assert_eq!(
            game.players[0].live_great_person_activation_needs.len(),
            1,
            "twenty-four slots the empire owns and none this Writer can reach"
        );
        assert_eq!(
            game.players[0].live_great_person_activation_needs[0].kind,
            "writer"
        );

        // One reachable slot and the need is gone — the empire has somewhere
        // to seat them and should not spend production on another building.
        state.units[0]
            .great_person
            .as_mut()
            .unwrap()
            .activation_plots[1]
            .slot_open = Some(true);
        apply_great_person_points(&mut game, &state, &mut unmapped);
        assert!(
            game.players[0]
                .live_great_person_activation_needs
                .is_empty(),
            "a reachable slot is not a reason to build capacity"
        );

        // ⚠ And an older control mod, which sends `slot_open` on no plot at
        // all, keeps exactly the behaviour it had: `None` is an absence, not
        // a claim, and must never be read as "full".
        let mut older = writer(None);
        for plot in &mut older.activation_plots {
            plot.slot_open = None;
        }
        state.units[0].great_person = Some(older);
        apply_great_person_points(&mut game, &state, &mut unmapped);
        assert!(
            game.players[0]
                .live_great_person_activation_needs
                .is_empty(),
            "an unknowing export must not manufacture a need"
        );
    }

    /// The government HISTORY must reach the planner, so a return switch is
    /// priced at its real Anarchy cost instead of free. Run
    /// civvis-20260815T012010Z: 127 guard blocks and 15 deck-refusal turns
    /// from the planner re-proposing a used government (deck and all) that
    /// its history-less board believed was a fresh, free switch.
    #[test]
    fn used_governments_reach_the_planners_history() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 92,
            width: 8,
            height: 8,
            chunk: 1,
            plots: vec![host_grass(3, 3)],
        }]);
        let state = StateSnapshot {
            turn: 92,
            government: Some("GOVERNMENT_MONARCHY".to_string()),
            used_governments: vec![
                "GOVERNMENT_CHIEFDOM".to_string(),
                "GOVERNMENT_OLIGARCHY".to_string(),
                "GOVERNMENT_MONARCHY".to_string(),
                "GOVERNMENT_FROM_A_FUTURE_EXPANSION".to_string(),
            ],
            ..StateSnapshot::default()
        };
        let report = rebuild_from_state(&snapshot, &state, 2, 1, 250, 0);
        let game = &report.game;

        for used in ["chiefdom", "oligarchy", "monarchy"] {
            assert!(
                game.players[0].past_governments.contains(used),
                "{used} must be in the seeded history"
            );
        }
        assert!(
            report
                .unmapped
                .iter()
                .any(|issue| issue.contains("GOVERNMENT_FROM_A_FUTURE_EXPANSION")),
            "an unknown government must be reported: {:?}",
            report.unmapped
        );

        // An older mod that sends no history must not invent one.
        let bare: StateSnapshot =
            serde_json::from_str(r#"{"turn": 3}"#).expect("an absent field parses");
        assert!(bare.used_governments.is_empty());
    }

    /// A host offer's class does not tell us the infrastructure it needs.
    ///
    /// Run civvis-20260815T042826Z recruited Hildegard of Bingen into an
    /// empire with three Campuses but no Holy Site, then Mary Leakey into the
    /// same Theater-less science empire. They had zero activation plots for
    /// 190 and 74 turns respectively. The live offer's required district must
    /// therefore gate every way the reconstructed game can claim it, while a
    /// later host state that does have the district must immediately reopen the
    /// ordinary class race.
    #[test]
    fn live_offer_district_blocker_prevents_an_unusable_scientist_race() {
        let wire = state_from_json(
            r#"{"turn":92,"great_person_offers":{"GREAT_PERSON_CLASS_SCIENTIST":{"individual":"GREAT_PERSON_INDIVIDUAL_HILDEGARD_OF_BINGEN","required_district":"DISTRICT_HOLY_SITE"}}}"#,
        )
        .expect("the Lua offer shape parses");
        assert!(
            wire.schema_gaps.is_empty(),
            "the recognized offer stays quiet"
        );
        let wire_offer = wire
            .great_person_offers
            .as_ref()
            .and_then(|offers| offers.get("GREAT_PERSON_CLASS_SCIENTIST"))
            .expect("the named Scientist offer crosses the wire");
        assert_eq!(
            wire_offer.required_district.as_deref(),
            Some("DISTRICT_HOLY_SITE")
        );
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 92,
            width: 8,
            height: 8,
            chunk: 1,
            plots: vec![host_grass(2, 3), host_grass(3, 3), host_grass(4, 3)],
        }]);
        let mut offers = BTreeMap::new();
        offers.insert(
            "GREAT_PERSON_CLASS_SCIENTIST".to_string(),
            StateGreatPersonOffer {
                individual: Some("GREAT_PERSON_INDIVIDUAL_HILDEGARD_OF_BINGEN".to_string()),
                required_district: Some("DISTRICT_HOLY_SITE".to_string()),
            },
        );
        let campus_only = StateSnapshot {
            turn: 92,
            cities: vec![StateCity {
                id: 65_536,
                name: "Rome".to_string(),
                x: 3,
                y: 3,
                pop: 6,
                capital: true,
                districts: vec![StateDistrict {
                    kind: "DISTRICT_CAMPUS".to_string(),
                    x: 4,
                    y: 3,
                    ..StateDistrict::default()
                }],
                ..StateCity::default()
            }],
            great_person_offers: Some(offers.clone()),
            ..StateSnapshot::default()
        };
        let mut game = rebuild_from_state(&snapshot, &campus_only, 2, 1, 250, 0).game;

        assert_eq!(
            game.players[0].live_great_person_offers,
            Some(["scientist".to_string()].into_iter().collect()),
            "the host's named screen, not CIVVIS's whole roster, is the live offer set"
        );
        assert!(game.great_person_class_offered_now(0, "scientist"));
        assert!(
            !game.great_person_class_offered_now(0, "merchant"),
            "a class omitted from Firaxis's table cannot receive a local order"
        );

        let blocker = game
            .live_great_person_offer_blocker(0, "scientist")
            .expect("Hildegard cannot use a Campus as a Holy Site");
        assert!(blocker.contains("HILDEGARD_OF_BINGEN"));
        assert!(blocker.contains("DISTRICT_HOLY_SITE"));
        assert!(
            !game.can_activate_current_great_person(0, "scientist"),
            "the generic Campus Scientist must yield to the live named offer"
        );
        let cost = game.gp_cost(0, "scientist");
        game.players[0].gpp.insert("scientist".to_string(), cost);
        assert!(
            game.apply(
                0,
                &crate::game::Action::RecruitGreatPerson {
                    kind: "scientist".to_string(),
                },
            )
            .is_err(),
            "even a ready-point automatic claim must share the live blocker"
        );

        let mut with_holy_site = campus_only.clone();
        with_holy_site.cities[0].districts.push(StateDistrict {
            kind: "DISTRICT_HOLY_SITE".to_string(),
            x: 2,
            y: 3,
            ..StateDistrict::default()
        });
        let reopened = rebuild_from_state(&snapshot, &with_holy_site, 2, 1, 250, 0).game;
        assert!(
            reopened
                .live_great_person_offer_blocker(0, "scientist")
                .is_none(),
            "the necessary district removes only the live hard blocker"
        );
        assert!(
            reopened.can_activate_current_great_person(0, "scientist"),
            "the ordinary Campus-targeted model resumes once Firaxis's condition holds"
        );

        let mut city_center_only = campus_only;
        city_center_only
            .great_person_offers
            .as_mut()
            .and_then(|offers| offers.get_mut("GREAT_PERSON_CLASS_SCIENTIST"))
            .expect("the test offer remains present")
            .required_district = Some("DISTRICT_CITY_CENTER".to_string());
        let centre_open = rebuild_from_state(&snapshot, &city_center_only, 2, 1, 250, 0).game;
        assert!(
            centre_open
                .live_great_person_offer_blocker(0, "scientist")
                .is_none(),
            "every exported city has its implicit City Center even though it is not in districts"
        );

        let bare: StateSnapshot =
            serde_json::from_str(r#"{"turn": 3}"#).expect("an older control mod still parses");
        assert!(bare.great_person_offers.is_none());
    }

    #[test]
    fn firaxis_governors_replace_inferred_titles_roster_and_promotions() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 92,
            width: 8,
            height: 8,
            chunk: 1,
            plots: vec![host_grass(3, 3)],
        }]);
        let state = StateSnapshot {
            turn: 92,
            governor_points: Some(4),
            governor_points_spent: Some(4),
            governors: Some(vec![
                StateGovernor {
                    kind: "GOVERNOR_THE_DEFENDER".to_string(),
                    city: 65_536,
                    city_player: 0,
                    x: 3,
                    y: 3,
                    established: true,
                    turns_on_site: 20,
                    turns_to_establish: 3,
                    promotions: vec![
                        "GOVERNOR_PROMOTION_REDOUBT".to_string(),
                        "GOVERNOR_PROMOTION_GARRISON_COMMANDER".to_string(),
                        "GOVERNOR_PROMOTION_DEFENSE_LOGISTICS".to_string(),
                    ],
                    ..StateGovernor::default()
                },
                StateGovernor {
                    kind: "GOVERNOR_THE_RESOURCE_MANAGER".to_string(),
                    city: -1,
                    promotions: vec![
                        "GOVERNOR_PROMOTION_RESOURCE_MANAGER_GROUNDBREAKER".to_string(),
                        "GOVERNOR_PROMOTION_RESOURCE_MANAGER_SURPLUS_LOGISTICS".to_string(),
                    ],
                    ..StateGovernor::default()
                },
                StateGovernor {
                    kind: "GOVERNOR_THE_EDUCATOR".to_string(),
                    city: -1,
                    promotions: vec!["GOVERNOR_PROMOTION_EDUCATOR_LIBRARIAN".to_string()],
                    ..StateGovernor::default()
                },
            ]),
            cities: vec![StateCity {
                id: 65_536,
                name: "Capital".to_string(),
                x: 3,
                y: 3,
                pop: 6,
                capital: true,
                ..StateCity::default()
            }],
            ..StateSnapshot::default()
        };

        let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
        let player = &rebuilt.game.players[0];
        let victor = player
            .governor_roster
            .get("victor")
            .expect("Victor crosses from the Firaxis roster");
        assert!(victor.city.is_some());
        assert!(victor.promotions.contains("garrison_commander"));
        assert!(victor.promotions.contains("defense_logistics"));
        let magnus = &player.governor_roster["magnus"];
        assert_eq!(
            magnus.promotions.iter().cloned().collect::<Vec<_>>(),
            vec!["surplus_logistics".to_string()]
        );
        assert!(player.governor_roster["pingala"].promotions.is_empty());
        assert!(rebuilt
            .unmapped
            .iter()
            .all(|issue| !issue.ends_with(":governor_promotion")));
        assert_eq!(player.governor_titles_spent, 4);
        assert_eq!(rebuilt.game.governor_titles(0), 4);
        assert_eq!(rebuilt.game.governor_titles_available(0), 0);
        assert!(rebuilt.game.legal_actions(0).iter().all(|action| !matches!(
            action,
            crate::game::Action::AssignGovernor { .. }
                | crate::game::Action::AppointGovernor { .. }
                | crate::game::Action::ReassignGovernor { .. }
                | crate::game::Action::PromoteGovernor { .. }
        )));
    }

    /// ★★★★★ The number that decides whether the empire notices it is going
    /// broke. `--fresh-board` rebuilds the mirror every turn, so the derived
    /// rate has no predecessor and reads 0 forever; live run
    /// `civvis-20260810T191050Z` sat at a zero treasury for its last 75 turns,
    /// lost its army to non-payment and went from six cities to two.
    #[test]
    fn the_hosts_net_income_survives_a_board_rebuilt_from_scratch() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 110,
            width: 8,
            height: 8,
            chunk: 1,
            plots: vec![host_grass(3, 3)],
        }]);
        let state = StateSnapshot {
            turn: 110,
            gold: 0,
            gold_per_turn: Some(-14.0),
            cities: vec![StateCity {
                id: 65_536,
                name: "Roma".to_string(),
                x: 3,
                y: 3,
                pop: 9,
                capital: true,
                ..StateCity::default()
            }],
            ..StateSnapshot::default()
        };

        let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
        assert_eq!(
            rebuilt.game.players[0].gold_per_turn, -14.0,
            "a rebuilt board must still know the empire is losing 14 gold a turn"
        );
    }

    /// A host that does not answer must not be read as break-even.
    #[test]
    fn an_unanswered_net_income_does_not_become_zero() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 40,
            width: 8,
            height: 8,
            chunk: 1,
            plots: vec![host_grass(3, 3)],
        }]);
        let mut state = StateSnapshot {
            turn: 40,
            gold: 120,
            gold_per_turn: None,
            cities: vec![StateCity {
                id: 65_536,
                name: "Roma".to_string(),
                x: 3,
                y: 3,
                pop: 4,
                capital: true,
                ..StateCity::default()
            }],
            ..StateSnapshot::default()
        };
        let silent = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
        state.gold_per_turn = Some(0.0);
        let break_even = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
        assert_eq!(
            break_even.game.players[0].gold_per_turn, 0.0,
            "a real 0 is break-even and must be applied"
        );
        // The silent case must be left at whatever the board already held rather
        // than being told, wrongly, that the books balance.
        assert!(silent.game.players[0].gold_per_turn.is_finite());
    }

    #[test]
    fn firaxis_era_score_and_age_thresholds_reach_the_board() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 92,
            width: 8,
            height: 8,
            chunk: 1,
            plots: vec![host_grass(3, 3)],
        }]);
        let state = StateSnapshot {
            turn: 92,
            era_score: Some(31),
            era_score_baseline: Some(12),
            normal_age_threshold: Some(20),
            golden_age_threshold: Some(40),
            world_era: Some(2),
            cities: vec![StateCity {
                id: 65_536,
                name: "Roma".to_string(),
                x: 3,
                y: 3,
                pop: 6,
                capital: true,
                ..StateCity::default()
            }],
            ..StateSnapshot::default()
        };

        let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
        let player = &rebuilt.game.players[0];
        assert_eq!(player.era_score, 31);
        assert_eq!(player.era_score_baseline, 12);
        assert_eq!(player.normal_age_threshold, 20);
        assert_eq!(player.golden_age_threshold, 40);
        assert_eq!(rebuilt.game.world_era, 2);
        // The point of carrying them: 31 sits between Firaxis's two thresholds,
        // so this is a Normal age. Against `Player::default` (12 and 26) the
        // same empire read as GOLDEN, which is the fiction this closes.
        assert!(player.era_score >= player.normal_age_threshold);
        assert!(player.era_score < player.golden_age_threshold);
    }

    #[test]
    fn an_unanswered_era_getter_leaves_the_board_alone() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 40,
            width: 8,
            height: 8,
            chunk: 1,
            plots: vec![host_grass(3, 3)],
        }]);
        // `try(...)` in the mod yields -1 when a getter is missing on the build.
        // A -1 must not become an era score, and it must not zero a threshold —
        // that would be a worse lie than the default it replaced.
        let state = StateSnapshot {
            turn: 40,
            era_score: Some(-1),
            normal_age_threshold: Some(-1),
            golden_age_threshold: Some(-1),
            world_era: Some(-1),
            cities: vec![StateCity {
                id: 65_536,
                name: "Roma".to_string(),
                x: 3,
                y: 3,
                pop: 4,
                capital: true,
                ..StateCity::default()
            }],
            ..StateSnapshot::default()
        };

        let silent = StateSnapshot {
            era_score: None,
            normal_age_threshold: None,
            golden_age_threshold: None,
            world_era: None,
            ..state.clone()
        };

        let refused = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
        let absent = rebuild_from_state(&snapshot, &silent, 4, 1, 250, 0);
        assert_eq!(
            refused.game.players[0].normal_age_threshold,
            absent.game.players[0].normal_age_threshold,
            "a -1 answer must leave the threshold exactly where no answer leaves it"
        );
        assert_eq!(
            refused.game.players[0].golden_age_threshold,
            absent.game.players[0].golden_age_threshold
        );
        assert_eq!(refused.game.world_era, absent.game.world_era);
    }

    /// The defect this file's `apply_encampment_health` exists for: a city that
    /// owns a HEALTHY Encampment must not be able to produce `repair_encampment`.
    ///
    /// Before the fix `encampment_hp` was 0 on every mirrored board, the gate
    /// `encampment_hp < 100` passed forever, the AI queued the repair every turn,
    /// the bridge discarded it as a project Civ 6 does not have, and the city
    /// built nothing for the rest of the game.
    #[test]
    fn a_healthy_encampment_cannot_be_repaired_forever() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 67,
            width: 12,
            height: 12,
            chunk: 1,
            plots: (2..8)
                .flat_map(|x| (2..8).map(move |y| host_grass(x, y)))
                .collect(),
        }]);
        let state = StateSnapshot {
            turn: 67,
            cities: vec![StateCity {
                id: 65_536,
                name: "Ravenna".to_string(),
                x: 4,
                y: 4,
                pop: 10,
                capital: true,
                districts: vec![StateDistrict {
                    kind: "DISTRICT_ENCAMPMENT".to_string(),
                    x: 5,
                    y: 4,
                    pillaged: false,
                    complete: true,
                    // Firaxis's own reading for an undamaged Encampment.
                    damage: 0,
                    max_damage: 100,
                    wall_damage: 0,
                    max_wall_damage: 0,
                }],
                ..StateCity::default()
            }],
            ..StateSnapshot::default()
        };

        let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
        let cid = *rebuilt.city_ids.keys().next().expect("the city was placed");
        // ⚠ Without this the `can_produce` assertion below passes VACUOUSLY: a
        // board where the Encampment never landed also refuses the repair, for
        // an entirely different reason.
        assert!(
            rebuilt.game.cities[&cid]
                .districts
                .contains_key(Name::new("encampment")),
            "the fixture must actually place the Encampment, or the refusal below \
             proves nothing"
        );
        assert_eq!(
            rebuilt.game.cities[&cid].encampment_hp, 100,
            "an undamaged Encampment is at full health, not the 0 the default left"
        );
        let repair = crate::game::Item::Project {
            project: crate::name::Name::new("repair_encampment"),
        };
        assert!(
            !rebuilt.game.can_produce(0, cid, &repair),
            "a healthy Encampment must not offer a repair — this is the order that \
             was discarded every turn while the city built nothing"
        );
    }

    /// The other half: a genuinely damaged Encampment must still be repairable,
    /// or the fix would have traded one silent failure for another.
    #[test]
    fn a_damaged_encampment_is_still_worth_repairing() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 67,
            width: 12,
            height: 12,
            chunk: 1,
            plots: (2..8)
                .flat_map(|x| (2..8).map(move |y| host_grass(x, y)))
                .collect(),
        }]);
        let state = StateSnapshot {
            turn: 67,
            cities: vec![StateCity {
                id: 65_536,
                name: "Ravenna".to_string(),
                x: 4,
                y: 4,
                pop: 10,
                capital: true,
                districts: vec![StateDistrict {
                    kind: "DISTRICT_ENCAMPMENT".to_string(),
                    x: 5,
                    y: 4,
                    pillaged: false,
                    complete: true,
                    damage: 60,
                    max_damage: 100,
                    wall_damage: 0,
                    max_wall_damage: 0,
                }],
                ..StateCity::default()
            }],
            ..StateSnapshot::default()
        };

        let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
        let cid = *rebuilt.city_ids.keys().next().expect("the city was placed");
        assert_eq!(rebuilt.game.cities[&cid].encampment_hp, 40);
    }

    /// A host that does not answer must leave the Encampment FULL, never 0.
    /// The asymmetry is the point: a wrong "healthy" costs one skipped repair, a
    /// wrong "destroyed" costs the city's whole production for the rest of the
    /// game.
    #[test]
    fn an_unanswered_encampment_reads_healthy_not_destroyed() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 67,
            width: 12,
            height: 12,
            chunk: 1,
            plots: (2..8)
                .flat_map(|x| (2..8).map(move |y| host_grass(x, y)))
                .collect(),
        }]);
        let state = StateSnapshot {
            turn: 67,
            cities: vec![StateCity {
                id: 65_536,
                name: "Ravenna".to_string(),
                x: 4,
                y: 4,
                pop: 10,
                capital: true,
                districts: vec![StateDistrict {
                    kind: "DISTRICT_ENCAMPMENT".to_string(),
                    x: 5,
                    y: 4,
                    pillaged: false,
                    complete: true,
                    // Every getter unanswered, as an older mod build would send.
                    ..StateDistrict::default()
                }],
                ..StateCity::default()
            }],
            ..StateSnapshot::default()
        };

        let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
        let cid = *rebuilt.city_ids.keys().next().expect("the city was placed");
        assert_eq!(rebuilt.game.cities[&cid].encampment_hp, 100);
    }

    #[test]
    fn a_zero_era_score_is_a_reading_not_a_missing_answer() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 3,
            width: 8,
            height: 8,
            chunk: 1,
            plots: vec![host_grass(3, 3)],
        }]);
        let state = StateSnapshot {
            turn: 3,
            era_score: Some(0),
            normal_age_threshold: Some(11),
            golden_age_threshold: Some(25),
            cities: vec![StateCity {
                id: 65_536,
                name: "Roma".to_string(),
                x: 3,
                y: 3,
                pop: 1,
                capital: true,
                ..StateCity::default()
            }],
            ..StateSnapshot::default()
        };

        let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
        let player = &rebuilt.game.players[0];
        assert_eq!(player.era_score, 0);
        assert_eq!(player.normal_age_threshold, 11);
        // On turn 3 with nothing scored yet Rome is genuinely BELOW the normal
        // threshold, which is the Dark Age warning the board could never show.
        assert!(player.era_score < player.normal_age_threshold);
    }

    #[test]
    fn firaxis_escort_formation_survives_the_fresh_board_rebuild() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 93,
            width: 8,
            height: 8,
            chunk: 1,
            plots: vec![host_grass(3, 3)],
        }]);
        let state = StateSnapshot {
            turn: 93,
            units: vec![
                StateUnit {
                    id: 501,
                    kind: "UNIT_SETTLER".to_string(),
                    x: 3,
                    y: 3,
                    hp: 100.0,
                    formation_count: 2,
                    ..StateUnit::default()
                },
                StateUnit {
                    id: 502,
                    kind: "UNIT_WARRIOR".to_string(),
                    x: 3,
                    y: 3,
                    hp: 100.0,
                    formation_count: 2,
                    ..StateUnit::default()
                },
            ],
            ..StateSnapshot::default()
        };

        let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
        let uid_for = |host| {
            rebuilt
                .unit_ids
                .iter()
                .find_map(|(uid, observed)| (*observed == host).then_some(*uid))
                .expect("host unit crosses into the mirror")
        };
        let settler = uid_for(501);
        let warrior = uid_for(502);

        assert_eq!(rebuilt.game.units[&settler].linked_to, Some(warrior));
        assert_eq!(rebuilt.game.units[&warrior].linked_to, Some(settler));
    }

    /// ★★★★★ A CORPS EXPORTED BY THE HOST HAS TO ARRIVE AS A CORPS.
    ///
    /// #2373 wired `Action::CombineUnits` to Firaxis's two merge commands and
    /// chooses between them by reading this exact field off the mirror:
    /// `UNITCOMMAND_FORM_CORPS` for two standard units, `UNITCOMMAND_FORM_ARMY`
    /// for a standard unit joining a Corps. The live seat runs `--fresh-board`,
    /// so the mirror is rebuilt from the host export every turn — and until this
    /// change the export said nothing about the tier, every unit was
    /// reconstructed at 0, and the seat could only ever ask for a Corps. That is
    /// not a near miss: the two are different commands behind different civics
    /// (Nationalism and Mobilization), so an existing Corps was being sent an
    /// order the host must refuse.
    ///
    /// The escort count rides alongside and is a DIFFERENT mechanism: a Corps is
    /// one unit and reports `formation_count` 1, so it must not be linked to
    /// anything by the escort reconstruction directly below.
    #[test]
    fn a_host_corps_and_army_survive_the_fresh_board_rebuild() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 140,
            width: 12,
            height: 12,
            chunk: 1,
            plots: vec![
                host_grass(3, 3),
                host_grass(4, 3),
                host_grass(5, 3),
                host_grass(6, 3),
            ],
        }]);
        let swordsman = |id: i64, x: i32, formation: Option<i32>| StateUnit {
            id,
            kind: "UNIT_SWORDSMAN".to_string(),
            x,
            y: 3,
            hp: 100.0,
            formation,
            ..StateUnit::default()
        };
        let state = StateSnapshot {
            turn: 140,
            units: vec![
                swordsman(601, 3, Some(1)),
                swordsman(602, 4, Some(2)),
                swordsman(603, 5, Some(0)),
                // The mod's "asked, could not answer". Unknown, never standard.
                swordsman(604, 6, Some(-1)),
            ],
            ..StateSnapshot::default()
        };

        let rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
        let uid_for = |host| {
            rebuilt
                .unit_ids
                .iter()
                .find_map(|(uid, observed)| (*observed == host).then_some(*uid))
                .expect("host unit crosses into the mirror")
        };

        assert_eq!(
            rebuilt.game.units[&uid_for(601)].formation,
            1,
            "a host Corps must arrive as a Corps, or CombineUnits sends FORM_CORPS \
             at a unit that already is one"
        );
        assert_eq!(rebuilt.game.units[&uid_for(602)].formation, 2);
        assert_eq!(rebuilt.game.units[&uid_for(603)].formation, 0);

        // A Corps is ONE unit. The escort reconstruction reads `formation_count`,
        // which stays 1 here, so nothing may be linked — the two mechanisms share
        // a word and nothing else.
        for host in [601, 602, 603, 604] {
            assert_eq!(
                rebuilt.game.units[&uid_for(host)].linked_to,
                None,
                "the merge tier must not be mistaken for an escort stack"
            );
        }
    }

    /// ⚠ THE SENTINEL MUST NOT READ AS STANDARD.
    ///
    /// `GetDefenseStrength` answered −1 for the whole project's life because its
    /// fallback was indistinguishable from an answer. The formation tier is the
    /// same shape of risk with a worse failure: 0 is a legal tier, so a fallback
    /// of 0 would silently claim every unit is standard on any build where the
    /// accessor is missing — and the board would keep asking for a Corps forever
    /// with nothing to show it was guessing. Only 0..=2 is a reading; the mod's
    /// −1, an absent field, and anything out of range leave the board alone.
    #[test]
    fn an_unreadable_formation_tier_never_flattens_a_corps_to_standard() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 140,
            width: 8,
            height: 8,
            chunk: 1,
            plots: vec![host_grass(3, 3)],
        }]);
        let state = StateSnapshot {
            turn: 140,
            units: vec![StateUnit {
                id: 701,
                kind: "UNIT_SWORDSMAN".to_string(),
                x: 3,
                y: 3,
                hp: 100.0,
                formation: Some(2),
                ..StateUnit::default()
            }],
            ..StateSnapshot::default()
        };
        let mut rebuilt = rebuild_from_state(&snapshot, &state, 4, 1, 250, 0);
        let uid = *rebuilt
            .unit_ids
            .keys()
            .next()
            .expect("the host unit crosses into the mirror");
        assert_eq!(rebuilt.game.units[&uid].formation, 2);

        for unknown in [None, Some(-1), Some(3), Some(i32::MIN)] {
            let observed = StateUnit {
                formation: unknown,
                ..state.units[0].clone()
            };
            let unit = rebuilt.game.units.get_mut(&uid).expect("addressable");
            apply_unit_observation(
                unit,
                &observed,
                ObservedUnitProgress {
                    promotions: None,
                    religion: None,
                },
            );
            assert_eq!(
                rebuilt.game.units[&uid].formation, 2,
                "{unknown:?} is an unknown tier, not a claim that the Army is a \
                 plain unit"
            );
        }
    }

    #[test]
    fn every_civvis_governor_and_promotion_round_trips_through_firaxis_ids() {
        let rules = crate::rules::Rules::embedded();
        for (governor, spec) in rules.governors.iter() {
            let host = civ6_governor_name(governor)
                .unwrap_or_else(|| panic!("{governor} needs a Firaxis Governor type"));
            assert_eq!(civvis_governor_name(host), Some(governor.as_str()));
            for promotion in spec.promotions.keys() {
                let host = civ6_governor_promotion(promotion).unwrap_or_else(|| {
                    panic!("{governor}.{promotion} needs a Firaxis promotion type")
                });
                assert_eq!(
                    civvis_governor_promotion(host),
                    Some(promotion.as_str()),
                    "{governor}.{promotion} must round-trip"
                );
            }
        }
    }

    /// The wire format is the risk here, not the arithmetic: the Lua field names and
    /// the serde names have to agree or every read silently returns its sentinel and
    /// the empire reconstructs as perfectly happy. This deserializes the exact shape
    /// the mod emits.
    #[test]
    fn the_host_amenity_ledger_crosses_the_bridge_and_names_the_shortfall() {
        let city: StateCity = serde_json::from_str(
            r#"{"id":65536,"name":"Kabasa","pop":15,"x":3,"y":4,
                "amenities":3,"amenities_needed":7,"happiness":2,
                "happiness_yield_mult":-20,
                "amenities_luxuries":2,"amenities_entertainment":1,
                "amenities_civics":0,"amenities_city_states":0,
                "amenities_war_weariness":0,"amenities_bankruptcy":0}"#,
        )
        .expect("the mod's city record deserializes");
        assert_eq!(city.amenities, 3.0, "the field name must match the Lua key");
        assert_eq!(city.amenities_needed, 7.0);
        assert_eq!(city.happiness_yield_mult, -20.0);
        assert_eq!(city.amenities_luxuries, 2.0);
        assert_eq!(host_city_amenity_surplus(&city), Some(-4));

        let state = StateSnapshot {
            turn: 214,
            cities: vec![city],
            ..Default::default()
        };
        let report = host_amenity_report(&state);
        assert!(report.contains("net -4"), "the sign and size must survive: {report}");
        assert!(report.contains("(1 short)"), "{report}");
        assert!(report.contains("host_yield_pct"),
            "the host's own figure is the whole point of the line: {report}");
        assert!(report.contains("luxuries 2"), "{report}");
    }

    /// ⚠ A mirror built before this export must NOT read as a happy empire, and
    /// UNKNOWN ARRIVES IN TWO SHAPES: absent becomes `f64::NAN` via `unknown_metric`,
    /// while a host read that failed arrives as the mod's `-1`. This asserts both are
    /// rejected — a reader testing only `!= -1.0` would let `NAN` through and a
    /// reader testing only `< 0.0` would let it through the other way, since every
    /// comparison against `NAN` is false.
    #[test]
    fn a_host_that_never_reported_amenities_says_nothing_rather_than_zero() {
        let silent: StateCity =
            serde_json::from_str(r#"{"id":65536,"name":"Kabasa","x":3,"y":4}"#)
                .expect("a pre-export city record still deserializes");
        assert!(silent.amenities.is_nan(),
            "an absent amenity read defaults to the unknown_metric sentinel, not zero");
        assert!(silent.amenities_needed.is_nan());
        assert!(silent.happiness_yield_mult.is_nan());
        assert_eq!(host_city_amenity_surplus(&silent), None);

        let state = StateSnapshot {
            turn: 40,
            cities: vec![silent],
            ..Default::default()
        };
        assert_eq!(host_amenity_report(&state), "",
            "silence must print nothing, not a surplus of zero");

        // The other shape: the host was asked and could not answer.
        let failed: StateCity = serde_json::from_str(
            r#"{"id":65536,"name":"Kabasa","x":3,"y":4,
                "amenities":-1,"amenities_needed":-1,"happiness_yield_mult":-1}"#,
        )
        .expect("a failed host read still deserializes");
        let state = StateSnapshot {
            turn: 40,
            cities: vec![failed],
            ..Default::default()
        };
        assert_eq!(host_amenity_report(&state), "",
            "the mod's -1 must be refused as firmly as an absent field");
        assert_eq!(host_city_amenity_surplus(&state.cities[0]), None);
    }

    /// The wire format is the risk: a Lua key that does not match the serde name
    /// silently returns the default and the empire reads as correctly holding zero.
    #[test]
    fn unspent_envoys_cross_the_bridge_and_name_the_suzerainties_we_hold() {
        let state: StateSnapshot = serde_json::from_str(
            r#"{"turn":214,"envoys_free":7,
                "minors":[
                  {"player":8,"civ":"CIVILIZATION_YEREVAN","envoys":3,"suzerain":0},
                  {"player":9,"civ":"CIVILIZATION_VILNIUS","envoys":1,"suzerain":3},
                  {"player":10,"civ":"CIVILIZATION_KABUL","envoys":0}]}"#,
        )
        .expect("the mod's state record deserializes");
        assert_eq!(state.envoys_free, Some(7), "the field name must match the Lua key");

        let report = host_envoy_report(&state);
        assert!(report.contains("unspent 7"), "{report}");
        assert!(report.contains("placed 4"), "{report}");
        // ⚠ Exactly one: seat 0 is ours, seat 3 is a rival's, and the third
        // city-state has no suzerain at all and defaults to -1.
        assert!(report.contains("suzerain 1/3"),
            "an unclaimed city-state must not count as ours: {report}");
    }

    /// ⚠ A mirror built before this export must not read as an empire correctly
    /// holding no envoys — that is the instrument inventing good news.
    #[test]
    fn a_host_that_never_reported_envoys_says_nothing_rather_than_zero() {
        let silent: StateSnapshot =
            serde_json::from_str(r#"{"turn":40,"minors":[]}"#).expect("deserializes");
        assert_eq!(silent.envoys_free, None);
        assert_eq!(host_envoy_report(&silent), "");

        // The other shape: the host was asked and could not answer.
        let failed: StateSnapshot = serde_json::from_str(
            r#"{"turn":40,"envoys_free":-1,"minors":[]}"#).expect("deserializes");
        assert_eq!(host_envoy_report(&failed), "",
            "the mod's -1 must be refused as firmly as an absent field");
    }

    /// ⚠ `GetHappinessNonFoodYieldModifier` is a PERCENTAGE and is NEGATIVE when the
    /// empire is unhappy — first live reading was -10 and -20, not 0.90 and 0.80. The
    /// original filter kept only `>= 0.0`, which discarded every real reading: an
    /// instrument that drops exactly the case it exists to measure.
    #[test]
    fn the_host_happiness_figure_is_a_negative_percentage_and_survives_the_filter() {
        let city = |name: &str, pct: f64| -> StateCity {
            serde_json::from_str(&format!(
                r#"{{"id":1,"name":"{name}","x":1,"y":1,"amenities":2,"amenities_needed":5,
                     "happiness_yield_mult":{pct},"amenities_luxuries":2,
                     "amenities_entertainment":0}}"#
            ))
            .expect("deserializes")
        };
        let state = StateSnapshot {
            turn: 111,
            cities: vec![city("Krakow", -10.0), city("Wroclaw", -20.0)],
            ..Default::default()
        };
        let report = host_amenity_report(&state);
        assert!(report.contains("host_yield_pct -15%"),
            "a taxed empire must report its tax, not be filtered away: {report}");
        assert!(report.contains("(2 short)"), "{report}");
    }

    /// The host ledger must be a planning input, not merely an observability
    /// note.  At the same time, pinning the raw host value would make an Arena
    /// appear to supply nothing in counterfactual scoring, so the bridge keeps
    /// a delta and proves that a modeled repair still lifts the calibrated band.
    #[test]
    fn host_amenity_deficit_calibrates_planning_without_freezing_arena_gain() {
        let snapshot = Snapshot::from_chunks(&[TilesChunk {
            turn: 84,
            width: 16,
            height: 16,
            chunk: 1,
            plots: (0..16)
                .flat_map(|x| (0..16).map(move |y| host_grass(x, y)))
                .collect(),
        }]);
        let state = StateSnapshot {
            turn: 84,
            cities: vec![StateCity {
                id: 65_536,
                name: "Roma".to_string(),
                x: 7,
                y: 7,
                pop: 12,
                amenities: 0.0,
                amenities_needed: 6.0,
                ..StateCity::default()
            }],
            ..StateSnapshot::default()
        };
        let mut rebuilt = rebuild_from_state(&snapshot, &state, 2, 91_001, 250, 0);
        let cid = rebuilt
            .game
            .city_at(crate::hex::offset_to_axial(7, 7))
            .expect("the reported city is mirrored");

        let before = rebuilt.game.city_amenity_surplus(&rebuilt.game.cities[&cid]);
        assert_eq!(before, -6, "the host's own deficit directs the planner");
        assert!(
            rebuilt
                .game
                .observed_city_amenity_adjustments
                .contains_key(&cid),
            "a known host ledger must not remain a diagnostic-only field"
        );
        let saved = serde_json::to_string(&rebuilt.game)
            .expect("the live calibration remains save-compatible");
        let restored: crate::game::Game =
            serde_json::from_str(&saved).expect("the calibration round-trips through a save");
        assert_eq!(
            restored.city_amenity_surplus(&restored.cities[&cid]),
            -6,
            "a saved mirror must not forget the host's current happiness band"
        );

        let site = rebuilt.game.cities[&cid]
            .owned_tiles
            .iter()
            .copied()
            .find(|position| *position != rebuilt.game.cities[&cid].pos)
            .expect("the city has a legal neighboring plot");
        let expected_gain = (rebuilt.game.rules.districts["entertainment_complex"].amenity
            + rebuilt.game.rules.buildings["arena"].amenity)
            .round() as i64;
        {
            let city = rebuilt.game.cities.get_mut(&cid).unwrap();
            city.districts
                .insert(crate::name!("entertainment_complex"), site);
            city.buildings.push(crate::name!("arena"));
        }
        rebuilt.game.map.tiles.get_mut(&site).unwrap().district =
            Some(crate::name!("entertainment_complex"));

        let after = rebuilt.game.city_amenity_surplus(&rebuilt.game.cities[&cid]);
        assert_eq!(
            after - before,
            expected_gain,
            "the additive host correction must retain the Entertainment Complex and Arena's modeled Amenities"
        );

        let mut unavailable = state.clone();
        unavailable.cities[0].amenities = f64::NAN;
        unavailable.cities[0].amenities_needed = f64::NAN;
        let mut unmapped = Vec::new();
        apply_observed_city_economy(&mut rebuilt.game, &unavailable, &mut unmapped);
        assert!(
            !rebuilt
                .game
                .observed_city_amenity_adjustments
                .contains_key(&cid),
            "a later unavailable host query must clear rather than preserve a stale deficit"
        );
    }
}
