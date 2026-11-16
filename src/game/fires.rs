//! Forest and jungle fires: a tile catches, burns two turns, lies burnt four
//! more and regrows richer, and a burning tile spreads into its neighbours.
//!
//! Every number is a row of `GranColombia_Maya_Expansion2.xml`, which
//! `GranColombia_Maya.modinfo` loads under the `GranColombiaMaya_Expansion2`
//! criteria (`any="1"`: a Bolivar or Lady Six Sky seat, or
//! `RULESET_EXPANSION_2`), so every Gathering Storm game has fires with every
//! game mode off — the same file and criteria as the meteor shower
//! (`game/goody_huts.rs`):
//!
//! ```xml
//! <Row RandomEventType="RANDOM_EVENT_FOREST_FIRE" ... EffectOperatorType="FIRE" Severity="1" Hexes="1" Duration="9" ChanceIncreasePerDegree="50" .../>
//! <Row RandomEventType="RANDOM_EVENT_FOREST_FIRE" FeatureType="FEATURE_FOREST"/>
//! <Row RandomEventType="RANDOM_EVENT_FOREST_FIRE" DamageType="IMPROVEMENT_PILLAGED" Percentage="101" MinTurn="0" MaxTurn="2"/>
//! <Row RandomEventType="RANDOM_EVENT_FOREST_FIRE" DamageType="UNIT_KILLED_CIVILIAN" Percentage="101" MinTurn="0" MaxTurn="2"/>
//! <Row RandomEventType="RANDOM_EVENT_FOREST_FIRE" DamageType="UNIT_DAMAGE_LAND" Percentage="101" MinHP="50" MaxHP="101" MinTurn="0" MaxTurn="2"/>
//! <Row RandomEventType="RANDOM_EVENT_FOREST_FIRE" DamageType="SPREAD" Percentage="50" MinTurn="1" MaxTurn="2"/>
//! <Row RandomEventType="RANDOM_EVENT_FOREST_FIRE" YieldType="YIELD_FOOD" Amount="0" FeatureType="FEATURE_BURNING_FOREST" Percentage="100" ReplaceFeature="true" Turn="0"/>
//! <Row RandomEventType="RANDOM_EVENT_FOREST_FIRE" YieldType="YIELD_FOOD" Amount="1" FeatureType="FEATURE_BURNT_FOREST" Percentage="100" ReplaceFeature="true" Turn="2"/>
//! <Row RandomEventType="RANDOM_EVENT_FOREST_FIRE" YieldType="YIELD_PRODUCTION" Amount="1" FeatureType="FEATURE_FOREST" Percentage="100" ReplaceFeature="true" Turn="6"/>
//! ```
//!
//! `RANDOM_EVENT_JUNGLE_FIRE` has the same rows over `FEATURE_JUNGLE`. How
//! often each class breaks out is `RandomEvent_Frequencies`, carried as
//! `per_intensity` in `data/disasters.json` and rolled by
//! `Game::process_disasters` like every other class.
//!
//! Measured from live games' tiles exports (2026-11-12..15, every mode off):
//! on 40 games, all 204 regrowths came six turns after the tile caught, two
//! burning and four burnt; the regrown tile usually paid +1 Food +1
//! Production over what it paid before (146 of them); and a fire spread only
//! into its own feature (102 of 102). On 60 games a burning tile lit each
//! eligible neighbour on each of its next two turns about half the time
//! (0.49 with one such neighbour, 0.54 with two). A tile lit by spread keeps
//! its own clock, so a fire can walk a forest for several turns.
//!
//! Two shipped rows are not modelled. `POPULATION_LOSS` needs a city centre
//! on the tile, and founding clears the feature a fire needs. And the
//! `ChanceIncreasePerDegree` of 50 is read through the climate multiplier
//! every class shares (`CLIMATE_DISASTER_SCALE`), as the storms' and floods'
//! own values are.

use super::*;

/// A burning tile turns burnt on its second turn (`Turn="2"`).
const BURNING_TURNS: u32 = 2;
/// A burnt tile regrows on the sixth turn after it caught (`Turn="6"`).
const REGROWS_AFTER: u32 = 6;
/// Each neighbour of the fire's own feature catches with this chance on each
/// of the burning tile's turns 1 and 2 (`SPREAD Percentage="50"`).
const SPREAD_CHANCE: f64 = 0.5;
/// HP a land unit on a burning tile loses each burning turn (`MinHP="50"
/// MaxHP="101"`, the top exclusive).
const UNIT_DAMAGE: (i32, i32) = (50, 100);

/// Which ground cover a fire burns.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FireKind {
    Forest,
    Jungle,
}

impl FireKind {
    /// The disaster class in `data/disasters.json` that starts this fire.
    pub fn from_class(class: &str) -> Option<Self> {
        match class {
            "forest_fire" => Some(Self::Forest),
            "jungle_fire" => Some(Self::Jungle),
            _ => None,
        }
    }

    pub fn class(self) -> &'static str {
        match self {
            Self::Forest => "forest_fire",
            Self::Jungle => "jungle_fire",
        }
    }

    /// The feature that burns, and that grows back.
    pub fn living(self) -> Name {
        match self {
            Self::Forest => crate::name!("forest"),
            Self::Jungle => crate::name!("jungle"),
        }
    }

    pub fn burning(self) -> Name {
        match self {
            Self::Forest => crate::name!("burning_forest"),
            Self::Jungle => crate::name!("burning_jungle"),
        }
    }

    pub fn burnt(self) -> Name {
        match self {
            Self::Forest => crate::name!("burnt_forest"),
            Self::Jungle => crate::name!("burnt_jungle"),
        }
    }
}

/// One tile of a fire: burning until `ignited + 2`, burnt until
/// `ignited + 6`, then its feature again.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Fire {
    pub pos: Pos,
    pub kind: FireKind,
    /// The turn this tile caught.
    pub ignited: u32,
}

impl Game {
    /// A fire breaks out on one tile of its class's feature, anywhere on the
    /// map: the shipped rows name no territory rule.
    pub(super) fn trigger_fire(&mut self, class: &str, rng: &mut Rng) {
        let Some(kind) = FireKind::from_class(class) else {
            return;
        };
        let living = kind.living();
        let sources: Vec<Pos> = self
            .map
            .tiles
            .iter()
            .filter(|(_, tile)| tile.feature == Some(living))
            .map(|(position, _)| *position)
            .collect();
        if sources.is_empty() {
            return;
        }
        let position = sources[rng.below(sources.len())];
        self.ignite(position, kind);
        let text = match kind {
            FireKind::Forest => "a forest fire broke out",
            FireKind::Jungle => "a rainforest fire broke out",
        };
        self.note_disaster(position, text.to_string());
    }

    /// Set one tile alight: its feature becomes the burning one and whatever
    /// stands there takes the turn-0 damage.
    pub(super) fn ignite(&mut self, position: Pos, kind: FireKind) {
        let Some(tile) = self.map.tiles.get_mut(&position) else {
            return;
        };
        if tile.feature != Some(kind.living()) {
            return;
        }
        tile.feature = Some(kind.burning());
        self.fires.push(Fire {
            pos: position,
            kind,
            ignited: self.turn,
        });
        let mut rng = self.fire_rng(kind, position, 0);
        self.scorch(position, &mut rng);
    }

    /// Carry every fire one turn: spread and burn on turns 1 and 2, lie burnt
    /// from turn 2, regrow on turn 6. A tile whose feature changed under it
    /// (an eruption's ash, say) has gone out and does not regrow.
    pub(super) fn advance_fires(&mut self) {
        if self.fires.is_empty() {
            return;
        }
        let fires = std::mem::take(&mut self.fires);
        let mut kept = Vec::with_capacity(fires.len());
        let mut sparks: Vec<(Pos, FireKind)> = Vec::new();
        for fire in fires {
            let age = self.turn.saturating_sub(fire.ignited);
            let feature = self.map.get(fire.pos).and_then(|tile| tile.feature);
            if age <= BURNING_TURNS {
                if feature != Some(fire.kind.burning()) {
                    continue;
                }
                if age >= 1 {
                    let mut rng = self.fire_rng(fire.kind, fire.pos, age);
                    let neighbours: Vec<Pos> = self.nbrs(fire.pos).into_iter().collect();
                    for next in neighbours {
                        let catches = self
                            .map
                            .get(next)
                            .is_some_and(|tile| tile.feature == Some(fire.kind.living()));
                        if catches
                            && rng.chance(SPREAD_CHANCE)
                            && !sparks.iter().any(|(lit, _)| *lit == next)
                        {
                            sparks.push((next, fire.kind));
                        }
                    }
                    self.scorch(fire.pos, &mut rng);
                }
                if age == BURNING_TURNS {
                    self.map.tiles.get_mut(&fire.pos).unwrap().feature = Some(fire.kind.burnt());
                    self.fertilize_tile(fire.pos, Fertility::Food);
                }
                kept.push(fire);
            } else if age < REGROWS_AFTER {
                if feature == Some(fire.kind.burnt()) {
                    kept.push(fire);
                }
            } else if feature == Some(fire.kind.burnt()) {
                self.map.tiles.get_mut(&fire.pos).unwrap().feature = Some(fire.kind.living());
                self.fertilize_tile(fire.pos, Fertility::Production);
            }
        }
        self.fires = kept;
        for (position, kind) in sparks {
            self.ignite(position, kind);
        }
    }

    /// One burning turn on one tile: a buildable improvement is pillaged,
    /// civilians die, every other unit loses 50 to 100 HP. Reinforced
    /// Materials spares the tile, as it does every disaster's
    /// (`disaster_immune`).
    fn scorch(&mut self, position: Pos, rng: &mut Rng) {
        if self.disaster_immune(position, None) {
            return;
        }
        let pillage = self.map.get(position).is_some_and(|tile| {
            !tile.pillaged
                && tile.improvement.is_some_and(|improvement| {
                    self.rules
                        .improvements
                        .get(improvement.as_str())
                        .is_some_and(|spec| !spec.unbuildable)
                })
        });
        if pillage {
            self.map.tiles.get_mut(&position).unwrap().pillaged = true;
        }
        for unit_id in self.units_at(position) {
            let Some(unit) = self.units.get(&unit_id) else {
                continue;
            };
            let civilian = self
                .rules
                .units
                .get(unit.kind.as_str())
                .is_some_and(|spec| spec.class == "civilian");
            let hp = unit.hp - rng.randint(UNIT_DAMAGE.0, UNIT_DAMAGE.1);
            if civilian || hp <= 0 {
                self.remove_unit(unit_id);
            } else {
                self.units.get_mut(&unit_id).unwrap().hp = hp;
            }
        }
    }

    /// A stream per tile and fire turn, off the disaster stream, so a fire
    /// never shifts the main RNG sequence or another class's rolls.
    fn fire_rng(&self, kind: FireKind, position: Pos, age: u32) -> Rng {
        let tile = (u64::from(position.0 as u32) << 32) | u64::from(position.1 as u32);
        self.disaster_rng(kind.class(), tile.rotate_left(13) ^ u64::from(age))
    }
}
