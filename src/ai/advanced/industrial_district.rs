//! An Industrial Zone unlocks a production building as well as adjacency.
//!
//! Price the already-researched first building's projected net production,
//! after paying for it and waiting for both construction steps. The ordinary
//! district scorer already prices the district's own yields and build time.
//! This adds the missing second step without counting that adjacency twice.

use super::{AdvancedAi, BasicAi, GrandStrategy};
use crate::game::{Game, Item};
use crate::name::Name;
use crate::rules::Yields;
use crate::Pos;

/// A future second build is less certain than a standing yield source.
const FOLLOW_ON_BUILD_DISCOUNT: f64 = 0.5;

impl AdvancedAi {
    pub(super) fn industrial_district_path_value(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        district: &Name,
        pos: Pos,
        strategy: GrandStrategy,
    ) -> f64 {
        if self.active_victory_target(g).is_none()
            || g.district_family(*district) != "industrial_zone"
        {
            return 0.0;
        }
        let city = &g.cities[&cid];
        let player = &g.players[pid];
        let item = Item::District {
            district: *district,
            pos,
        };
        let district_turns = self.production_build_turns(g, pid, cid, &item);
        let remaining = if g.max_turns > 0 {
            g.max_turns.saturating_sub(g.turn) as f64
        } else {
            g.game_speed.turn_limit() as f64
        };
        let displaced = if g.city_citizen_plan(cid).worked_tiles.contains(&pos) {
            g.workable_tile_yields(pos).production
        } else {
            0.0
        };
        let rate = (g.city_yields(cid).production + g.district_yields(*district, pos).production
            - displaced)
            .max(1.0);
        g.rules
            .buildings
            .iter()
            .filter(|(building, spec)| {
                spec.buildable
                    && !spec.wonder
                    && spec.yields.production > 0.0
                    && spec.regional_range == 0
                    && spec.requires.is_empty()
                    && spec.requires_any.is_empty()
                    && spec
                        .district
                        .is_some_and(|d| g.district_family(d) == "industrial_zone")
                    && spec
                        .unique_to
                        .as_deref()
                        .is_none_or(|civ| civ == player.civ.as_str())
                    && spec
                        .tech
                        .as_ref()
                        .is_none_or(|tech| player.techs.contains(tech))
                    && spec
                        .civic
                        .as_ref()
                        .is_none_or(|civic| player.civics.contains(civic))
                    && (!spec.coastal || BasicAi::city_has_open_water(g, cid))
                    && !city.buildings.iter().any(|built| {
                        g.building_is_family(*built, **building)
                            || spec
                                .excludes
                                .iter()
                                .any(|excluded| g.building_is_family(*built, *excluded))
                    })
            })
            .map(|(building, spec)| {
                let follow_on = Item::Building {
                    building: *building,
                };
                // The future building cannot spend the city's current overflow a
                // second time; the district's remaining cost already used it.
                let cost = g.item_cost_for_city(pid, cid, &follow_on);
                let build_rate = (rate * g.item_prod_mult(pid, cid, Some(&follow_on))).max(1.0);
                let earning_turns = (remaining - district_turns - cost / build_rate).max(0.0);
                let net_production = (earning_turns * spec.yields.production - cost).max(0.0);
                self.yield_value(
                    Yields {
                        production: net_production,
                        ..Yields::default()
                    },
                    strategy,
                ) * FOLLOW_ON_BUILD_DISCOUNT
            })
            .fold(0.0, f64::max)
    }
}

#[cfg(test)]
mod tests;
