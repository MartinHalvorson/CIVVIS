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
    /// Improvement type name already built here, e.g. `IMPROVEMENT_FARM`.
    #[serde(default)]
    pub im: Option<String>,
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
            im: None,
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
                im: None,
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
            // ★★★ WHAT IS ALREADY IMPROVED. An unimproved-looking world makes CIVVIS
            // order builders forever: 19 of them for one city in one measured run.
            // ⚠ Mapped by name with no vocabulary, so a Civ 6 improvement CIVVIS does
            // not know becomes None rather than a wrong improvement — the tile then
            // reads unimproved, which is the honest direction for a name we cannot
            // translate.
            tile.improvement = plot.im.as_ref().and_then(|name| {
                let short = name.strip_prefix("IMPROVEMENT_").unwrap_or(name).to_ascii_lowercase();
                if game_rules_has_improvement(&short) {
                    Some(Name::new(&short))
                } else {
                    None
                }
            });
        }
    }
}

/// Whether the CIVVIS ruleset knows this improvement name.
///
/// Split out so the terrain pass does not need a `&Game` borrow while it holds a
/// mutable tile.
fn game_rules_has_improvement(name: &str) -> bool {
    // The improvement set is small and stable; checking against the shipped ruleset
    // would need a borrow this loop cannot take, so the names CIVVIS actually models
    // are listed. Anything else reads as unimproved, which is the honest direction.
    matches!(
        name,
        "farm" | "mine" | "quarry" | "pasture" | "plantation" | "camp" | "fishing_boats"
            | "lumber_mill" | "oil_well" | "offshore_oil_rig" | "fort" | "airstrip"
            | "national_park" | "industry" | "seaside_resort" | "ski_resort"
    )
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
    /// What Civilization VI is CURRENTLY building here, by type name.
    ///
    /// ★★★★ Exported as a raw hash for the whole project (`producing:
    /// -1743686858`) and therefore unusable, so the mirror had no idea what any
    /// city already had underway and CIVVIS re-decided production every turn blind
    /// to work in progress.
    #[serde(default, deserialize_with = "name_or_nothing")]
    pub producing: Option<String>,
    /// Food stockpiled toward the next citizen.
    #[serde(default)]
    pub food: f64,
    /// Loyalty CHANGE per turn. `loyalty` alone is a level, and a city at 100
    /// falling fast looks identical to one at 100 holding steady — which is exactly
    /// how a city was lost at t98 with loyalty reading 100.
    #[serde(default)]
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
    #[serde(default)]
    pub defense: f64,
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
    pub x: i32,
    pub y: i32,
    #[serde(default)]
    pub hp: f64,
    /// Movement points left, as Civilization VI reports them this turn.
    #[serde(default = "unknown_strength")]
    pub moves: f64,
    /// Already fortified. Civilization VI REFUSES `FORTIFY` on a unit that is, so a
    /// board that did not carry this re-ordered it every turn — 28 refusals in run
    /// 233331Z, exactly one per turn from t196 on.
    #[serde(default)]
    pub fortified: bool,
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
    /// Civ 6 belief type of the pantheon this seat has founded, if any.
    ///
    /// ⚠ Its absence was not silent, only unread: 125 `pantheon` orders in 173 turns,
    /// every one counted applied, against one pantheon. A seat that does not know it
    /// has a pantheon keeps choosing one — and is also missing that belief's yields
    /// from every calculation it makes.
    #[serde(default)]
    pub pantheon: Option<String>,
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
    /// Faith BALANCE. `science` and `culture` are per-turn yields that CIVVIS
    /// derives from its own board, so injecting them would fight the simulation;
    /// faith is a stockpile and crosses cleanly, exactly like gold.
    #[serde(default)]
    pub faith: i64,
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
    /// Barbarian units this seat can SEE.
    ///
    /// ★★★★ The rival export is built from `GetAliveMajorIDs`, so barbarians could
    /// never appear in it and could never show `at_war`. A city lost to them read as
    /// "lost at peace with everyone", which is how the analysis of how cities are
    /// lost was made with an instrument blind to the likeliest cause.
    #[serde(default)]
    pub hostiles: Vec<StateUnit>,
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
}

/// `CIVILIZATION_ROME` -> `Rome`, using CIVVIS's own roster as the authority.
///
/// Returns `None` when Civilization VI names a civilization CIVVIS does not have,
/// which is deliberate: a wrong-but-plausible name is worse than an obvious gap,
/// because it silently reintroduces exactly the mismatch this function exists to
/// remove. Of the Civ 6 roster only the Ottomans currently miss, and they miss on
/// spelling (`Ottomans`), which the plural retry below catches.
pub fn civvis_civ_name(civ6: &str) -> Option<&'static str> {
    let bare = civ6
        .trim()
        .strip_prefix("CIVILIZATION_")
        .unwrap_or(civ6.trim())
        .replace('_', " ");
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

/// Give every seat the civilization Civilization VI is actually playing.
///
/// ⚠ MUST RUN BEFORE ANY CITY IS PLACED. `found_city_for` reads `players[pid].civ`
/// to name a city, so setting identity afterwards leaves the old roster's names on
/// the board — the visible half of the very bug this fixes.
fn apply_identity(game: &mut crate::game::Game, state: &StateSnapshot) -> Vec<String> {
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
    for rival in &state.rivals {
        note(rival.player, &rival.civ.clone(), &mut unmapped);
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
    if let (Some(state), Some(seat)) = (best.as_mut(), seat) {
        state.seat = seat;
    }
    if let Some(state) = best.as_mut() {
        state.refused_sites = refused_city_sites(path);
        state.refused_improves = refused_improve_sites(path);
    }
    best
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
    if table.contains_key(&base) {
        return Some(base);
    }
    let without_article = base.strip_prefix("the_")?;
    if table.contains_key(without_article) {
        return Some(without_article.to_string());
    }
    None
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
        "horse_archer" => "saka_horse_archer".to_string(),
        _ => base,
    }
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
pub fn refused_sites_of_kind(
    path: &std::path::Path,
    kind: &str,
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

/// Sites Civilization VI refused to found a city on.
pub fn refused_city_sites(path: &std::path::Path) -> std::collections::BTreeSet<crate::Pos> {
    refused_sites_of_kind(path, "found_refused")
}

/// Tiles Civilization VI refused to let a builder improve, after the mod had already
/// tried the named improvement, any improvement, and automation.
pub fn refused_improve_sites(path: &std::path::Path) -> std::collections::BTreeSet<crate::Pos> {
    refused_sites_of_kind(path, "improve_refused")
}


/// A Civilization VI production type name as a CIVVIS queue [`Item`].
///
/// ⚠ Returns None rather than guessing. A wrong item would tell CIVVIS a city is
/// busy with something it is not, which is worse than the idle city this fixes: it
/// would suppress a real production decision instead of merely repeating one.
///
/// Districts are deliberately NOT reconstructed — `Item::District` carries a `pos`
/// the export does not give, and inventing one would place a district on arbitrary
/// ground.
fn civvis_production_item(
    rules: &crate::rules::Rules,
    civ6: Option<&str>,
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

    // Sites the host engine has already rejected, so the planner stops re-deriving
    // them. See `refused_city_sites`.
    game.blocked_city_sites = state.refused_sites.clone();
    game.blocked_improvement_sites = state.refused_improves.clone();

    // Identity first: city naming reads it, so this cannot wait until after the
    // cities are placed. See `apply_identity`.
    let unmapped = apply_identity(&mut game, state);
    if !unmapped.is_empty() {
        eprintln!(
            "mirror: no CIVVIS civilization for {unmapped:?} — those seats keep their \
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
        Some(game.place_city(owner, pos, banner(c)))
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
    if state.faith >= 0 {
        game.players[0].faith = state.faith as f64;
    }
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
            Some(name) => game.players[0].government = Some(name),
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

    for city in &state.cities {
        if let Some(cid) = plant_city(&mut game, 0, city) {
            city_ids.insert(cid, city.id);
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
                // ★★★★ SEED THE QUEUE WITH WHAT CIVILIZATION VI IS ALREADY BUILDING.
                //
                // Without it every city reads as idle, so CIVVIS chooses production
                // from scratch each turn with no knowledge of work in progress —
                // which is what a run alternating Builder / Monument / Campus every
                // second turn looks like from the inside.
                if let Some(item) = civvis_production_item(&game_rules, city.producing.as_deref())
                {
                    if built.queue.is_empty() {
                        built.queue.push(item);
                    }
                }
                // ★★★★ WHAT THE CITY ALREADY HAS. Without this a city reads as empty
                // forever and CIVVIS re-orders the same development every turn:
                // measured 19 Builders and 17 Granaries for ONE city, against one
                // Warrior — the mirror image of the old all-army failure.
                for civ6 in &city.buildings {
                    let name = civ6
                        .strip_prefix("BUILDING_")
                        .unwrap_or(civ6)
                        .to_ascii_lowercase();
                    let named = crate::name::Name::new(&name);
                    if !built.buildings.contains(&named) {
                        built.buildings.push(named);
                    }
                }
            }
        }
    }

    let mut dropped: Vec<String> = Vec::new();
    let mut plant_unit = |game: &mut crate::game::Game,
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
        // ⚠ FOG HONESTY IS ABOUT OTHER PEOPLE'S UNITS, NOT OUR OWN. A plot one of OUR
        // units is standing on is revealed by definition — Civilization VI is telling
        // us the unit is there. What the gate was actually catching is the tile export
        // being on a slower cadence than the unit export (`--tile-export-every 4`), so
        // a scout that walks onto fresh ground vanishes from CIVVIS's board until the
        // map catches up. Measured on run `civvis-20260731T090445Z`: units dropped
        // `unrevealed` on 6 of the first 39 turns, two at once at worst — and a unit
        // CIVVIS cannot see gets no order and stands where it was built.
        //
        // Rivals and hostiles keep the gate: reporting an enemy on ground this seat has
        // not seen would grant contact it has not earned.
        if owner != 0 && !snapshot.is_revealed((u.x, u.y)) {
            dropped.push(format!("{}@{},{}:unrevealed", u.kind, u.x, u.y));
            return None;
        }
        let name = civvis_unit_name(&u.kind);
        if !game.rules.units.contains_key(&name) {
            if !unmapped.contains(&u.kind) {
                unmapped.push(u.kind.clone());
            }
            dropped.push(format!("{}@{},{}:untranslatable", u.kind, u.x, u.y));
            return None;
        }
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
        if let Some(unit) = game.units.get_mut(&uid) {
            let hp = u.hp.round() as i32;
            if hp > 0 && hp < 100 {
                unit.hp = hp;
            }
            unit.fortified = u.fortified;
        }
        Some(uid)
    };

    for unit in &state.units {
        if let Some(uid) = plant_unit(&mut game, 0, unit, &mut unmapped, &mut dropped) {
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
        for city in &rival.cities {
            if plant_city(&mut game, owner, city).is_some() {
                placed_rival_cities += 1;
            }
        }
        for unit in &rival.units {
            if plant_unit(&mut game, owner, unit, &mut unmapped, &mut dropped).is_some() {
                placed_rival_units += 1;
            }
        }
    }

    // Barbarians go on CIVVIS's own barbarian seat rather than a rival's, so the
    // threat is visible to the planner without inventing a civilization at war with
    // us. Skipped silently when the roster has no barbarian seat.
    let barbarian_seat = game.players.iter().position(|player| player.is_barbarian);
    if let Some(barb) = barbarian_seat {
        for unit in &state.hostiles {
            if plant_unit(&mut game, barb, unit, &mut unmapped, &mut dropped).is_some() {
                placed_rival_units += 1;
            }
        }
    }

    apply_territory(&mut game, snapshot, state);

    Reconstruction {
        game,
        unit_ids,
        city_ids,
        placed_cities,
        placed_units,
        placed_rival_cities,
        placed_rival_units,
        unmapped,
        dropped_units: dropped,
    }
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
                continue;
            };
            // The city that would work it: the owner's nearest. Civ 6 records only
            // the owning PLAYER per plot, so which of their cities holds it is not in
            // the export and the nearest is the only defensible reconstruction.
            let owner = centres.get(&seat).and_then(|list| {
                list.iter()
                    .min_by_key(|(cid, centre)| (game.wdist(pos, *centre), *cid))
                    .map(|(cid, _)| *cid)
            });
            if owner.is_some() {
                assign.push((pos, owner));
            }
        }
    }
    game.blocked_city_sites.extend(blocked);
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
    /// See [`Reconstruction::dropped_units`]. Carried onto the live mirror so the
    /// decider can report it every turn: a unit that is missing from the board is a
    /// unit that will stand still, and nothing else in the telemetry can say so.
    pub dropped_units: Vec<String>,
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
            dropped_units: rebuilt.dropped_units,
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
        // Rivals are met as the game goes on, so identity is not a one-time job at
        // reconstruction: a civilization first seen on turn 90 arrives here.
        apply_identity(&mut self.game, state);
        self.game.turn = state.turn.max(1);
        if state.gold >= 0 {
            self.game.players[0].gold = state.gold as f64;
        }
        for civ6 in &state.techs {
            if let Some(name) = civvis_node_name(&self.game.rules.techs, civ6, "TECH_") {
                self.game.players[0].techs.insert(crate::name::Name::new(&name));
            }
        }
        for civ6 in &state.civics {
            if let Some(name) = civvis_node_name(&self.game.rules.civics, civ6, "CIVIC_") {
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
                        live.fortified = unit.fortified;
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
            let cid = self.game.place_city(0, pos, banner(city));
            self.cid_of.insert(city.id, cid);
        }
        for city in &state.cities {
            if let Some(cid) = self.cid_of.get(&city.id) {
                if let Some(live) = self.game.cities.get_mut(cid) {
                    if city.pop > 0 {
                        live.pop = city.pop;
                    }
                    if city.loyalty >= 0.0 {
                        live.loyalty = city.loyalty;
                    }
                    for civ6 in &city.buildings {
                        let name = civ6
                            .strip_prefix("BUILDING_")
                            .unwrap_or(civ6)
                            .to_ascii_lowercase();
                        let named = crate::name::Name::new(&name);
                        if !live.buildings.contains(&named) {
                            live.buildings.push(named);
                        }
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
                self.game.place_city(owner, pos, banner(city));
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

#[cfg(test)]
mod host_fact_tests {
    use super::*;

    /// Civilization VI's production names must reach CIVVIS's queue as real items.
    ///
    /// ⚠ The export shipped a raw HASH for the whole project, so this path was dead
    /// and every city read as idle — CIVVIS then chose production from scratch each
    /// turn, blind to work already underway.
    #[test]
    fn civ6_production_names_become_civvis_queue_items() {
        let rules = crate::rules::Rules::shared();
        let settler = civvis_production_item(&rules, Some("UNIT_SETTLER"));
        assert!(
            matches!(settler, Some(crate::game::Item::Unit { .. })),
            "UNIT_SETTLER should map to a CIVVIS unit build, got {settler:?}"
        );
        let monument = civvis_production_item(&rules, Some("BUILDING_MONUMENT"));
        assert!(
            matches!(monument, Some(crate::game::Item::Building { .. })),
            "BUILDING_MONUMENT should map to a CIVVIS building, got {monument:?}"
        );

        // ⚠ Refusing to guess is the point. A wrong item tells CIVVIS a city is busy
        // with something it is not, which SUPPRESSES a real production decision —
        // worse than the repeated one this fixes.
        assert!(civvis_production_item(&rules, Some("UNIT_NOT_A_REAL_THING")).is_none());
        assert!(civvis_production_item(&rules, Some("")).is_none());
        assert!(civvis_production_item(&rules, None).is_none());
        // Districts carry a `pos` the export does not provide; inventing one would
        // place a district on arbitrary ground.
        assert!(civvis_production_item(&rules, Some("DISTRICT_CAMPUS")).is_none());
    }
}
