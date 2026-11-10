//! `sweden-builds-open-air-museums`: Sweden's Builders price the Open-Air
//! Museum.
//!
//! **The rule.** `DLC/Expansion2/Data/Expansion2_Improvements_Major.xml`:
//! `IMPROVEMENT_OPEN_AIR_MUSEUM` unlocks at `CIVIC_NATIONALISM`, is
//! `OnePerCity`, stands on Snow, Tundra, Desert, Plains or Grassland (flat or
//! Hills, no feature rows), and carries
//! `OPEN_AIR_MUSEUM_CULTURE_FOR_TERRAIN_CLASS_CITIES` (Amount 2,
//! `YIELD_CULTURE`) plus `Improvement_Tourism` `TOURISMSOURCE_CULTURE` at
//! `ScalingFactor` 100. The description reads "+2 Culture and +2 Tourism for
//! each type of terrain (Snow, Tundra, Desert, Plains or Grassland) in which
//! at least one Swedish city is founded. One per city."
//!
//! **Why it was never built.** `data/improvements.json` carries no `yields`
//! and no `tourism` effect for it, and
//! `AdvancedAi::improvement_value_with_appeal` prices only those, so the
//! museum scored about 0 and lost every tile to a Farm or a Mine. Kristina
//! Deity `civvis-20261110T140731Z` reached Nationalism at t122 with 8 cities
//! on three terrain classes and had built none by t150: +48 Culture and +48
//! Tourism a turn left standing, against 96 Culture and 42 Tourism earned.
//!
//! **The gene.** On Sweden's own ground the Builder prices the museum at
//! 2 Culture per terrain class of Sweden's city centres, through the lane's
//! own yield weights, plus as much Tourism at the culture lane's durable
//! tourism price. That outranks any ordinary tile, so each city's museum is
//! the next Builder charge once it unlocks. Among one city's legal sites the
//! museum gives up least where the best other improvement is worth least, so
//! that value is taken off the site. Only the Builder's valuation changes:
//! the rules, the engine's yields and every other civilization are
//! untouched, and with the gene off nothing changes.

use super::*;

/// Culture, and Tourism, per terrain class (the modifier's `Amount`; the
/// tourism source scales the culture at 100%).
const PER_TERRAIN_CLASS: f64 = 2.0;

/// The culture lane's price of a point of durable Tourism, the same rate
/// `improvement_value_with_appeal` pays a resort or a park.
const TOURISM_PRICE: f64 = 35.0;

/// The terrain classes the modifier counts. Hills share their flat class.
const TERRAIN_CLASSES: [&str; 5] = ["snow", "tundra", "desert", "plains", "grassland"];

const OPEN_AIR_MUSEUM: &str = "open_air_museum";

impl AdvancedAi {
    /// How many of the five terrain classes hold at least one of `pid`'s
    /// city centres.
    pub(super) fn open_air_museum_terrain_classes(g: &Game, pid: usize) -> usize {
        g.cities
            .values()
            .filter(|city| city.owner == pid)
            .filter_map(|city| g.map.get(city.pos))
            .filter_map(|tile| {
                TERRAIN_CLASSES
                    .iter()
                    .position(|class| *class == tile.terrain.as_str())
            })
            .collect::<BTreeSet<_>>()
            .len()
    }

    /// `sweden-builds-open-air-museums`: what an Open-Air Museum on `pos` is
    /// worth to the Builder beyond the zero its spec prices. Zero with the
    /// gene off, for any other improvement, on ground no Swedish city owns,
    /// and for a Sweden whose cities stand on no counted terrain.
    pub(super) fn open_air_museum_value(
        &self,
        g: &Game,
        pos: Pos,
        improvement: &str,
        strategy: GrandStrategy,
    ) -> f64 {
        if !self.sweden_builds_open_air_museums || improvement != OPEN_AIR_MUSEUM {
            return 0.0;
        }
        let Some(pid) = g
            .map
            .get(pos)
            .and_then(|tile| tile.owner_city)
            .and_then(|city| g.cities.get(&city))
            .map(|city| city.owner)
        else {
            return 0.0;
        };
        if g.players[pid].civ != "Sweden" {
            return 0.0;
        }
        let per_turn = PER_TERRAIN_CLASS * Self::open_air_museum_terrain_classes(g, pid) as f64;
        if per_turn <= 0.0 {
            return 0.0;
        }
        let culture = Yields {
            culture: per_turn,
            ..Yields::default()
        };
        let worth = self.yield_value(culture, strategy) + per_turn * TOURISM_PRICE;
        // What the museum's site gives up: the best other Builder job on it.
        // The museum's culture belongs to the city, not to the plot, so the
        // plot's own yield is kept either way.
        let appeal = g.tile_appeal(pos).max(0) as f64;
        let given_up = g
            .valid_improvements(pid, pos)
            .into_iter()
            .filter(|other| {
                other.as_str() != OPEN_AIR_MUSEUM && g.rules.improvements[other].builder_buildable
            })
            .map(|other| self.improvement_value_with_appeal(g, pos, &other, strategy, appeal))
            .fold(0.0_f64, f64::max);
        worth - given_up
    }
}

#[cfg(test)]
mod tests;
