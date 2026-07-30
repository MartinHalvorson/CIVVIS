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

    for y in 0..height {
        for x in 0..width {
            let pos = (x, y);
            let Some(tile) = game.map.tiles.get_mut(&pos) else {
                continue;
            };
            let Some(plot) = snapshot.plot(pos) else {
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
    game
}
