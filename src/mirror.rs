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
use std::collections::BTreeMap;

use serde::Deserialize;

use crate::name::Name;

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
}

fn minus_one() -> i32 {
    -1
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
            snapshot.turn = snapshot.turn.max(chunk.turn);
            snapshot.width = snapshot.width.max(chunk.width);
            snapshot.height = snapshot.height.max(chunk.height);
            for plot in &chunk.plots {
                snapshot.revealed.insert((plot.x, plot.y), plot.clone());
            }
        }
        snapshot
    }

    /// Whether this seat has revealed a plot. Everything outside this is unknown
    /// ground and must never be treated as ordinary ground.
    pub fn is_revealed(&self, pos: (i32, i32)) -> bool {
        self.revealed.contains_key(&pos)
    }

    pub fn plot(&self, pos: (i32, i32)) -> Option<&Plot> {
        self.revealed.get(&pos)
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
    use super::*;

    fn plot(x: i32, y: i32, t: &str) -> Plot {
        Plot {
            x,
            y,
            t: Some(t.to_string()),
            f: None,
            r: None,
            o: -1,
            w: false,
            i: false,
            fw: false,
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
                t: Some("TERRAIN_GRASS".to_string()),
                f: None,
                r: None,
                o: 0,
                w: false,
                i: false,
                fw: true,
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
}

/// Read every `tiles` chunk out of a run's `events.jsonl`.
///
/// The stream is append-only and a chunk is self-describing, so re-reading from
/// the start is both simplest and correct — later chunks overwrite earlier ones
/// for the same plot, which is what [`Snapshot::from_chunks`] does.
pub fn snapshot_from_events(path: &std::path::Path) -> std::io::Result<Snapshot> {
    let raw = std::fs::read_to_string(path)?;
    let mut chunks = Vec::new();
    for line in raw.lines() {
        if !line.contains("\"tiles\"") {
            continue;
        }
        if let Ok(chunk) = serde_json::from_str::<TilesChunk>(line) {
            if !chunk.plots.is_empty() {
                chunks.push(chunk);
            }
        }
    }
    Ok(Snapshot::from_chunks(&chunks))
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
/// ⚠ UNREVEALED PLOTS ARE FLATTENED TO OCEAN, and that is a deliberate,
/// load-bearing choice rather than a convenience.
///
/// A generated map arrives full of terrain the seat has never laid eyes on. Left
/// alone, that terrain is a map generator's invention presented as knowledge, and
/// anything reading tile yields — `settle_value` reads them directly, with no
/// visibility filter, because CIVVIS stores no per-player revealed map — would be
/// planning on ground that does not exist. Ocean is the least misleading filler:
/// it scores nothing and cannot be settled, so an unseen plot cannot attract a
/// decision.
///
/// It is still wrong in a way worth naming: it makes unseen land read as water, so
/// a reconstruction is honest about what it KNOWS and pessimistic about what it
/// does not. That is the right direction for a viewer and for settling. It is the
/// WRONG direction for pathfinding, which would route around phantom sea. Use
/// [`Snapshot::is_revealed`] to tell the two apart rather than trusting the map.
pub fn rebuild_game(snapshot: &Snapshot, players: usize, seed: u64) -> crate::game::Game {
    use crate::game::Game;
    let width = snapshot.width.max(1);
    let height = snapshot.height.max(1);
    let mut game = Game::new(players.max(2), width, height, seed, 500, 0);
    let vocab = Vocabulary::embedded();
    let ocean = Name::new("ocean");

    apply_terrain(&mut game, snapshot);
    let _ = (ocean, vocab);
    game
}

/// Write every plot the seat has seen onto the map, and ocean everywhere else.
///
/// Shared by the one-shot rebuild and by [`LiveMirror::sync`], which has to re-apply
/// it as ground is revealed. Idempotent: re-running it on an existing map only
/// overwrites terrain the snapshot has an opinion about.
pub(crate) fn apply_terrain(game: &mut crate::game::Game, snapshot: &Snapshot) {
    let vocab = Vocabulary::embedded();
    let ocean = Name::new("ocean");
    let width = snapshot.width.max(1);
    let height = snapshot.height.max(1);
    for y in 0..height {
        for x in 0..width {
            let pos = crate::hex::offset_to_axial(x, y);
            let Some(tile) = game.map.tiles.get_mut(&pos) else {
                continue;
            };
            let Some(plot) = snapshot.plot((x, y)) else {
                // ⚠ Only stamp ocean where nothing is known. A tile the frontier
                // painted as land must not be reverted on the next sync, or the
                // frontier would flicker between land and sea every turn and CIVVIS
                // would re-plan around it — the oscillation bug in another costume.
                if !snapshot.is_revealed((x, y)) && tile.terrain == Name::new("plains") {
                    continue;
                }
                tile.terrain = ocean;
                tile.hills = false;
                tile.feature = None;
                tile.resource = None;
                continue;
            };
            if let Some(name) = &plot.t {
                if let Resolved::Known((terrain, hills)) = vocab.terrain(name) {
                    tile.terrain = terrain;
                    tile.hills = hills;
                }
            }
            tile.feature = plot.f.as_ref().and_then(|name| match vocab.feature(name) {
                Resolved::Known(value) => Some(value),
                _ => None,
            });
            tile.resource = plot.r.as_ref().and_then(|name| match vocab.resource(name) {
                Resolved::Known(value) => Some(value),
                _ => None,
            });
        }
    }
}

/// The seat's own cities, in the order they appear in the stream.
///
/// Read from `state` events, which the mod emits only under `--export-state`.
pub fn own_cities_from_events(path: &std::path::Path) -> Vec<(i32, i32)> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let mut seen = std::collections::BTreeSet::new();
    let mut order = Vec::new();
    for line in raw.lines() {
        if !line.contains("\"state\"") {
            continue;
        }
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let Some(cities) = value.get("cities").and_then(|c| c.as_array()) else {
            continue;
        };
        for city in cities {
            let x = city.get("x").and_then(|v| v.as_i64());
            let y = city.get("y").and_then(|v| v.as_i64());
            if let (Some(x), Some(y)) = (x, y) {
                let pos = (x as i32, y as i32);
                if seen.insert(pos) {
                    order.push(pos);
                }
            }
        }
    }
    order
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

/// One city as Civilization VI reported it, in OFFSET coordinates.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateCity {
    #[serde(default)]
    pub id: i64,
    pub x: i32,
    pub y: i32,
    #[serde(default)]
    pub pop: i32,
    #[serde(default)]
    pub capital: bool,
    #[serde(default)]
    pub defense: f64,
}

/// One unit as Civilization VI reported it, in OFFSET coordinates.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateUnit {
    #[serde(default)]
    pub id: i64,
    #[serde(default)]
    pub kind: String,
    pub x: i32,
    pub y: i32,
    #[serde(default)]
    pub hp: f64,
    /// Movement points left, as Civilization VI reports them this turn.
    #[serde(default = "unknown_strength")]
    pub moves: f64,
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateRival {
    #[serde(default)]
    pub player: usize,
    /// Whether Civilization VI says this seat may declare war on them RIGHT NOW.
    #[serde(default)]
    pub can_declare: bool,
    #[serde(default)]
    pub score: i64,
    #[serde(default = "unknown_strength")]
    pub military: f64,
    #[serde(default)]
    pub at_war: bool,
    #[serde(default)]
    pub cities: Vec<StateCity>,
    #[serde(default)]
    pub units: Vec<StateUnit>,
}

fn unknown_strength() -> f64 {
    -1.0
}

/// The whole board as one `state` event described it.
#[derive(Clone, Debug, Default, serde::Deserialize)]
pub struct StateSnapshot {
    pub turn: u32,
    /// Civ 6 type names of COMPLETED research, e.g. `TECH_BRONZE_WORKING`.
    #[serde(default)]
    pub techs: Vec<String>,
    #[serde(default)]
    pub civics: Vec<String>,
    #[serde(default)]
    pub gold: i64,
    #[serde(default)]
    pub score: i64,
    #[serde(default = "unknown_strength")]
    pub military: f64,
    #[serde(default)]
    pub cities: Vec<StateCity>,
    #[serde(default)]
    pub units: Vec<StateUnit>,
    #[serde(default)]
    pub rivals: Vec<StateRival>,
}

/// The newest `state` event, or the one for `turn` when asked for a specific one.
///
/// ⚠ Newest-wins rather than first-match: the mod re-emits state as a turn is
/// replayed, and an early partial export would otherwise win forever.
pub fn state_from_events(
    path: &std::path::Path,
    turn: Option<u32>,
) -> Option<StateSnapshot> {
    let raw = std::fs::read_to_string(path).ok()?;
    let mut best: Option<StateSnapshot> = None;
    for line in raw.lines() {
        if !line.contains("\"state\"") {
            continue;
        }
        let Ok(state) = serde_json::from_str::<StateSnapshot>(line) else {
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
    best
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
    pub placed_cities: usize,
    pub placed_units: usize,
    pub placed_rival_cities: usize,
    pub placed_rival_units: usize,
    pub unmapped: Vec<String>,
}

/// `UNIT_BATTERING_RAM` -> `battering_ram`. Mechanical, then CHECKED against the
/// ruleset — `spawn_unit` indexes `rules.units` and panics on a name it does not
/// have, so an unchecked guess would take the brain down mid-game.
fn civvis_unit_name(civ6: &str) -> String {
    civ6.strip_prefix("UNIT_").unwrap_or(civ6).to_ascii_lowercase()
}


/// Paint `depth` rings of neutral land beyond the edge of what the seat has seen.
///
/// ⚠⚠ THIS IS AN INVENTED PRIOR, and it is deliberate. `apply_terrain` fills the
/// unknown with OCEAN, which is honest for scoring and catastrophic for deciding: a
/// seat that has revealed 51 plots sees a 51-tile island, so it has nowhere to settle,
/// nowhere to explore, and nothing worth building but soldiers. Measured: revealed
/// plots crawled 25 -> 150 over 104 turns, `met` stopped at 2, and ZERO rival cities
/// were ever seen.
///
/// A bounded ring is closer to what a human knows — the ground past your border is
/// probably ground — and it cannot invent a continent, because the far unknown stays
/// sea. Each ring becomes real terrain as it is revealed. An order onto ground that
/// turns out to be water is refused by Civilization VI and counted, so the failure
/// mode is a wasted order, not a plan resting on a phantom.
pub(crate) fn grow_frontier(
    game: &mut crate::game::Game,
    snapshot: &Snapshot,
    depth: u32,
) {
    if depth == 0 {
        return;
    }

    let unknown_land = Name::new("plains");
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
    let mut land: std::collections::BTreeSet<crate::Pos> = std::collections::BTreeSet::new();
    for y in 0..height {
        for x in 0..width {
            if !snapshot.is_revealed((x, y)) {
                continue;
            }
            let pos = crate::hex::offset_to_axial(x, y);
            if game
                .map
                .get(pos)
                .map(|tile| !game.rules.is_water(tile))
                .unwrap_or(false)
            {
                land.insert(pos);
            }
        }
    }
    let mut edge: Vec<crate::Pos> = land.iter().copied().collect();
    for _ in 0..depth {
        let mut next_edge: Vec<crate::Pos> = Vec::new();
        for pos in &edge {
            for neighbour in crate::hex::neighbors(*pos) {
                let (nx, ny) = crate::hex::axial_to_offset(neighbour.0, neighbour.1);
                if nx < 0 || ny < 0 || nx >= width || ny >= height {
                    continue;
                }
                // Never overwrite ground the seat has actually seen: a coast we
                // scouted must stay coast, or this would invent land over water
                // we know about.
                if snapshot.is_revealed((nx, ny)) || land.contains(&neighbour) {
                    continue;
                }
                if let Some(tile) = game.map.tiles.get_mut(&neighbour) {
                    tile.terrain = unknown_land;
                    tile.hills = false;
                    tile.feature = None;
                    tile.resource = None;
                }
                land.insert(neighbour);
                next_edge.push(neighbour);
            }
        }
        if next_edge.is_empty() {
            break;
        }
        edge = next_edge;
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
    let mut game = rebuild_game(snapshot, players.max(2), seed);
    let mut unit_ids = std::collections::BTreeMap::new();
    let mut city_ids = std::collections::BTreeMap::new();
    let mut unmapped: Vec<String> = Vec::new();
    let mut placed_cities = 0;
    let mut placed_units = 0;
    let mut placed_rival_cities = 0;
    let mut placed_rival_units = 0;

    // Land only, and revealed only. `place_city` on water or on an unseen tile
    // would put CIVVIS's empire somewhere the seat cannot act.
    let mut plant_city = |game: &mut crate::game::Game, owner: usize, c: &StateCity| -> Option<u32> {
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
        Some(game.place_city(owner, pos, None))
    };

    // ★★★★★ THE FRONTIER RING — without it CIVVIS believes it lives on a tiny island.
    //
    // `rebuild_game` fills every unrevealed plot with OCEAN, and its own doc names the
    // limit: honest about what is known, pessimistic about what is not, and "the WRONG
    // direction for pathfinding, which would route around phantom sea". That was
    // tolerable while this reconstruction only fed a viewer and a settle ranking over
    // ground already seen. It is not tolerable now that CIVVIS is the DECIDER, because
    // a seat that has revealed 51 plots sees a 51-tile island: nowhere to expand,
    // nowhere to explore, nothing to build but soldiers.
    //
    // Measured on run bisect1-114111Z: `desired_cities = 3`, and at turn 34 the empire
    // was still ONE city with 33 production orders, every one of them a Warrior.
    //
    // One ring of neutral land at the edge of what we have seen is the minimum that
    // lets expansion and exploration aim OUTWARD, and it is what a human knows: the
    // ground past your border is probably ground. It stays one ring — the far unknown
    // is still ocean — so this cannot invent a continent, and each ring becomes real
    // terrain as it is revealed. An order onto ground that turns out to be sea is
    // refused by Civilization VI and counted as a refusal, so the failure mode is a
    // wasted order rather than a plan built on a phantom.
    grow_frontier(&mut game, snapshot, frontier_depth);

    // ★★★★ TELL CIVVIS WHAT TURN IT IS. `Game::new` starts at the beginning, and the
    // board is rebuilt from scratch every turn, so without this CIVVIS was answering
    // TURN 1 for the whole game — every time. Measured consequence on run
    // civvis-20260730T111953Z: 15 production orders, ALL of them Warrior, no settler
    // and no district, while its own plan asked for 3 cities. An agent whose strategy
    // is keyed to era and timing cannot plan from a clock stuck at zero.
    game.turn = state.turn.max(1);
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

    // ★ Research first: what a seat KNOWS bounds what it can sensibly do, and a
    // CIVVIS player with an empty tree recommends Slingers in the Medieval era.
    // ⚠ Names that do not exist in the CIVVIS ruleset are counted, not ignored —
    // a silently dropped tech is a capability CIVVIS will not use and nobody sees.
    for civ6 in &state.techs {
        let name = civ6.strip_prefix("TECH_").unwrap_or(civ6).to_ascii_lowercase();
        if game.rules.techs.contains_key(&name) {
            game.players[0].techs.insert(crate::name::Name::new(&name));
        } else if !unmapped.contains(civ6) {
            unmapped.push(civ6.clone());
        }
    }
    for civ6 in &state.civics {
        let name = civ6.strip_prefix("CIVIC_").unwrap_or(civ6).to_ascii_lowercase();
        if game.rules.civics.contains_key(&name) {
            game.players[0].civics.insert(crate::name::Name::new(&name));
        } else if !unmapped.contains(civ6) {
            unmapped.push(civ6.clone());
        }
    }

    for city in &state.cities {
        if let Some(cid) = plant_city(&mut game, 0, city) {
            city_ids.insert(cid, city.id);
            placed_cities += 1;
            if city.pop > 0 {
                if let Some(built) = game.cities.get_mut(&cid) {
                    built.pop = city.pop;
                }
            }
        }
    }

    let mut plant_unit = |game: &mut crate::game::Game,
                          owner: usize,
                          u: &StateUnit,
                          unmapped: &mut Vec<String>|
     -> Option<u32> {
        if !snapshot.is_revealed((u.x, u.y)) {
            return None;
        }
        let name = civvis_unit_name(&u.kind);
        if !game.rules.units.contains_key(&name) {
            if !unmapped.contains(&u.kind) {
                unmapped.push(u.kind.clone());
            }
            return None;
        }
        let pos = crate::hex::offset_to_axial(u.x, u.y);
        let water = game
            .map
            .get(pos)
            .map(|tile| game.rules.is_water(tile))
            .unwrap_or(true);
        if water {
            return None;
        }
        let uid = game.spawn_unit(&name, owner, pos);
        // Carry damage across: a unit at 30 hp is a unit CIVVIS should pull out,
        // and defaulting it to full health is how an army gets thrown away.
        if let Some(unit) = game.units.get_mut(&uid) {
            let hp = u.hp.round() as i32;
            if hp > 0 && hp < 100 {
                unit.hp = hp;
            }
        }
        Some(uid)
    };

    for unit in &state.units {
        if let Some(uid) = plant_unit(&mut game, 0, unit, &mut unmapped) {
            unit_ids.insert(uid, unit.id);
            placed_units += 1;
        }
    }

    // Rivals get seats 1..n in the order Civilization VI reported them, so a
    // CIVVIS `DeclareWar { player }` maps straight back onto a Civ 6 player id.
    for (index, rival) in state.rivals.iter().enumerate() {
        let owner = index + 1;
        if owner >= game.players.len() {
            break;
        }
        // Same as `LiveMirror::sync`: Civilization VI's own `CanDeclareWarOn` is the
        // permission, and CIVVIS's Formal War wait cannot mature here.
        if rival.can_declare && !rival.at_war {
            game.players[0].denounced_until.insert(owner, game.turn + 1);
        }
        for city in &rival.cities {
            if plant_city(&mut game, owner, city).is_some() {
                placed_rival_cities += 1;
            }
        }
        for unit in &rival.units {
            if plant_unit(&mut game, owner, unit, &mut unmapped).is_some() {
                placed_rival_units += 1;
            }
        }
    }

    Reconstruction {
        game,
        unit_ids,
        city_ids,
        placed_cities,
        placed_units,
        placed_rival_cities,
        placed_rival_units,
        unmapped,
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
    /// Rival stand-ins, rebuilt each sync: they need no continuity of their own and
    /// what we can see of them changes with the fog.
    rival_units: Vec<u32>,
    rival_cities: std::collections::BTreeSet<(i32, i32)>,
    pub unmapped: Vec<String>,
    pub turns_synced: u32,
}

/// A unit's full movement allowance from the ruleset.
fn mirror_unit_moves(game: &crate::game::Game, uid: u32) -> f64 {
    let kind = match game.units.get(&uid) {
        Some(unit) => unit.kind,
        None => return 2.0,
    };
    game.rules
        .units
        .get(kind.as_str())
        .map(|spec| spec.moves)
        .unwrap_or(2.0)
}

impl LiveMirror {
    pub fn new(
        snapshot: &Snapshot,
        state: &StateSnapshot,
        players: usize,
        seed: u64,
        max_turns: u32,
        frontier_depth: u32,
    ) -> LiveMirror {
        let rebuilt =
            rebuild_from_state(snapshot, state, players, seed, max_turns, frontier_depth);
        let mut uid_of = std::collections::BTreeMap::new();
        for (uid, civ6) in &rebuilt.unit_ids {
            uid_of.insert(*civ6, *uid);
        }
        let mut cid_of = std::collections::BTreeMap::new();
        for (cid, civ6) in &rebuilt.city_ids {
            cid_of.insert(*civ6, *cid);
        }
        LiveMirror {
            game: rebuilt.game,
            civ6_of: rebuilt.unit_ids,
            uid_of,
            cid_of,
            rival_units: Vec::new(),
            rival_cities: std::collections::BTreeSet::new(),
            unmapped: rebuilt.unmapped,
            turns_synced: 1,
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
        self.game.turn = state.turn.max(1);
        if state.gold >= 0 {
            self.game.players[0].gold = state.gold as f64;
        }
        for civ6 in &state.techs {
            let name = civ6.strip_prefix("TECH_").unwrap_or(civ6).to_ascii_lowercase();
            if self.game.rules.techs.contains_key(&name) {
                self.game.players[0].techs.insert(crate::name::Name::new(&name));
            }
        }
        for civ6 in &state.civics {
            let name = civ6.strip_prefix("CIVIC_").unwrap_or(civ6).to_ascii_lowercase();
            if self.game.rules.civics.contains_key(&name) {
                self.game.players[0].civics.insert(crate::name::Name::new(&name));
            }
        }

        // Newly revealed ground, and the frontier redrawn beyond it. Terrain that was
        // already known does not change, so only the freshly seen needs writing —
        // but the frontier has to be recomputed because its edge just moved.
        if !skip_terrain {
            apply_terrain(&mut self.game, snapshot);
            grow_frontier(&mut self.game, snapshot, frontier_depth);
        }

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
                    let hp = unit.hp.round() as i32;
                    if let Some(live) = self.game.units.get_mut(&uid) {
                        live.hp = if hp > 0 { hp.min(100) } else { 1 };
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
                        let allowance = mirror_unit_moves(&self.game, uid);
                        if let Some(live) = self.game.units.get_mut(&uid) {
                            live.moves_left = allowance;
                            live.acted = false;
                            live.attacks_left = 1;
                            // Cleared by `Game::begin_turn` every turn; on a persistent
                            // game they survive and a unit that "already moved" is
                            // skipped.
                            live.moved = false;
                            live.zoc_stopped = false;
                            live.fortified = false;
                            live.fortify_turns = 0;
                        }
                    }
                }
                _ => {
                    let name = civvis_unit_name(&unit.kind);
                    if !self.game.rules.units.contains_key(&name) {
                        if !self.unmapped.contains(&unit.kind) {
                            self.unmapped.push(unit.kind.clone());
                        }
                        continue;
                    }
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
            let cid = self.game.place_city(0, pos, None);
            self.cid_of.insert(city.id, cid);
        }
        for city in &state.cities {
            if let Some(cid) = self.cid_of.get(&city.id) {
                if city.pop > 0 {
                    if let Some(live) = self.game.cities.get_mut(cid) {
                        live.pop = city.pop;
                    }
                }
            }
        }

        // --- rivals ----------------------------------------------------------
        // Rebuilt wholesale: what we can see of them is fog-dependent and they carry
        // no plan of ours worth preserving.
        if skip_rivals {
            return;
        }
        for uid in std::mem::take(&mut self.rival_units) {
            if self.game.units.contains_key(&uid) {
                self.game.remove_unit(uid);
            }
        }
        for (index, rival) in state.rivals.iter().enumerate() {
            let owner = index + 1;
            if owner >= self.game.players.len() {
                break;
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
            if rival.can_declare && !rival.at_war {
                self.game.players[0].denounced_until.insert(owner, self.game.turn + 1);
            } else {
                self.game.players[0].denounced_until.remove(&owner);
            }
            for city in &rival.cities {
                if self.rival_cities.contains(&(city.x, city.y))
                    || !snapshot.is_revealed((city.x, city.y))
                {
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
                self.game.place_city(owner, pos, None);
                self.rival_cities.insert((city.x, city.y));
            }
            for unit in &rival.units {
                if !snapshot.is_revealed((unit.x, unit.y)) {
                    continue;
                }
                let name = civvis_unit_name(&unit.kind);
                if !self.game.rules.units.contains_key(&name) {
                    continue;
                }
                let pos = crate::hex::offset_to_axial(unit.x, unit.y);
                let water = self
                    .game
                    .map
                    .get(pos)
                    .map(|tile| self.game.rules.is_water(tile))
                    .unwrap_or(true);
                if water {
                    continue;
                }
                let uid = self.game.spawn_unit(&name, owner, pos);
                self.rival_units.push(uid);
            }
        }
    }
}
