//! `districts-protect-seowon-science`: a district's site pays for the Science
//! it removes from neighboring owned Seowons, including started foundations.
//!
//! Firaxis: Expansion1_Districts.xml:68 assigns -1 Science per adjacent
//! district to NegativeDistrict_Science; Expansion1_Districts_Major.xml:18
//! attaches that rule to Seowon. The engine's adjacency calculator supplies
//! both sides, including Government Plaza cancellation and policy multipliers.
//! Only local neighbors are inspected; no speculative Game clone is needed.

use super::{AdvancedAi, GrandStrategy};
use crate::game::adjacency::PlanAssumption;
use crate::game::{Game, Item};
use crate::name::Name;
use crate::rules::Yields;
use crate::Pos;
use std::collections::BTreeSet;

impl AdvancedAi {
    /// Lane-weighted lost yield, before the production scorer's 60x value
    /// and build-time normalization. Count only our own Seowons; captured
    /// Korean districts qualify even when this player is another civilization.
    pub(super) fn seowon_site_loss_value(
        &self,
        g: &Game,
        pid: usize,
        district: Name,
        pos: Pos,
        lane: GrandStrategy,
    ) -> f64 {
        if !self.districts_protect_seowon_science {
            return 0.0;
        }
        // A Seowon's lost output is Campus output, irrespective of the
        // district taking its neighbor. Honor the same expansion-phase
        // Science pricing as a new Campus, not the intruding district's lane.
        let lane = self.campus_pricing_lane("campus", lane);
        let Some(tile) = g.map.get(pos) else {
            return 0.0;
        };
        // An existing foundation has already committed its adjacency. This
        // gene must not strand its completion by charging that loss twice.
        if tile.district.is_some() || tile.district_foundation.is_some() {
            return 0.0;
        }
        let mut placed = tile.clone();
        placed.district = Some(district);
        let assume = PlanAssumption {
            city_center: None,
            foundations: true,
        };
        let rationalism_half = g.policy_effect(pid, "campus_building_science_pct") / 200.0;
        let mut loss = 0.0;
        for seowon_pos in g.nbrs(pos) {
            let Some(neighbor) = g.map.get(seowon_pos) else {
                continue;
            };
            let kind = neighbor
                .district
                .or_else(|| neighbor.district_foundation.as_ref().map(|f| f.district));
            if kind != Some(crate::name!("seowon")) {
                continue;
            }
            let Some(city) = neighbor
                .owner_city
                .and_then(|cid| g.cities.get(&cid))
                .filter(|city| city.owner == pid)
            else {
                continue;
            };
            // Rationalism reads raw adjacency, before Natural Philosophy.
            // Only a standing, active Seowon can currently multiply buildings.
            let track_threshold = rationalism_half > 0.0
                && neighbor.district.is_some()
                && !neighbor.pillaged
                && g.city_district_family_position(city, crate::name!("campus"))
                    == Some(seowon_pos);
            let mut before_sources = Vec::new();
            let mut after_sources = Vec::new();
            let before = g.district_adjacency_assuming(
                crate::name!("seowon"),
                seowon_pos,
                Some(&assume),
                track_threshold.then_some(&mut before_sources),
            );
            let mut neighbors = [None; 6];
            for (index, near) in g.nbrs(seowon_pos).into_iter().enumerate() {
                neighbors[index] = if near == pos {
                    Some(&placed)
                } else {
                    g.map.get(near)
                };
            }
            let after = g.district_adjacency_assuming_with_family_and_neighbors(
                crate::name!("seowon"),
                seowon_pos,
                Some(&assume),
                track_threshold.then_some(&mut after_sources),
                crate::name!("campus"),
                &neighbors,
            );
            loss += (self.yield_value(before, lane) - self.yield_value(after, lane)).max(0.0);
            if track_threshold {
                let raw = |sources: &[crate::game::AdjacencySource]| {
                    sources
                        .iter()
                        .filter(|source| source.source != "adjacency_bonus")
                        .map(|source| source.yields.science)
                        .sum::<f64>()
                };
                if raw(&before_sources) >= 4.0 && raw(&after_sources) < 4.0 {
                    let science = Self::seowon_threshold_building_science(g, city);
                    loss += self.yield_value(
                        Yields {
                            science: science * rationalism_half,
                            ..Yields::default()
                        },
                        lane,
                    );
                }
            }
        }
        loss
    }

    /// Science to which the engine applies Rationalism in this Seowon.
    /// `city_yields` adds power, district-building and recruited-scientist
    /// yields before applying the card. Population's half is unchanged by
    /// this placement; only the adjacency half can be lost.
    fn seowon_threshold_building_science(g: &Game, city: &crate::game::City) -> f64 {
        let powered = g.city_is_powered(city);
        let district_science = g.rules.districts[crate::name!("seowon")]
            .effects
            .get("building_science")
            .copied()
            .unwrap_or(0.0);
        city.buildings
            .iter()
            .filter(|name| !city.pillaged_buildings.contains(*name))
            .filter_map(|name| {
                let building = &g.rules.buildings[name];
                if building.district.map(|d| g.district_family(d)) != Some(crate::name!("campus")) {
                    return None;
                }
                let mut science = if building.regional_range > 0 {
                    0.0
                } else {
                    building.yields.science
                };
                science += district_science;
                if powered && building.regional_range <= 0 {
                    science += building
                        .effects
                        .get("powered_science")
                        .copied()
                        .unwrap_or(0.0);
                }
                for (family, counter) in [
                    ("library", "great_person:library_science"),
                    ("university", "great_person:university_science"),
                    ("research_lab", "great_person:research_lab_science"),
                ] {
                    if g.building_is_family(name, Name::new(family)) {
                        science += g.players[city.owner]
                            .counters
                            .get(counter)
                            .copied()
                            .unwrap_or(0) as f64;
                    }
                }
                Some(science)
            })
            .sum()
    }

    /// The same charge in final production-score units. A coordinated plan
    /// may raise an item's score to an infrastructure floor; that floor must
    /// still pay the site cost already priced into the ordinary raw score.
    pub(super) fn seowon_site_loss_score(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: &Item,
        lane: GrandStrategy,
    ) -> f64 {
        if !self.districts_protect_seowon_science {
            return 0.0;
        }
        let Item::District { district, pos } = item else {
            return 0.0;
        };
        let loss = self.seowon_site_loss_value(g, pid, *district, *pos, lane);
        if loss <= 0.0 {
            return 0.0;
        }
        let turns = self.production_build_turns(g, pid, cid, item);
        loss * 60.0 / self.production_time_divisor(g, cid, item, turns)
    }

    /// A reserved district bypasses the scored menu. Keep its kind and city,
    /// but let legal sites compete on the same production value when the
    /// reservation would otherwise remove an owned Seowon's Science.
    pub(super) fn seowon_protection_reserved_site(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        item: Item,
        plan: &super::StrategicPlan,
    ) -> Item {
        let Item::District { district, pos } = &item else {
            return item;
        };
        if self.seowon_site_loss_value(g, pid, *district, *pos, plan.strategy) <= 0.0 {
            return item;
        }
        let district = *district;
        let counts = self.counts(g, pid);
        let mut best_score = self.production_value(g, pid, cid, &item, plan, &counts);
        let mut best = item;
        for pos in g.district_sites(cid, district) {
            let candidate = Item::District { district, pos };
            if !g.can_produce(pid, cid, &candidate) {
                continue;
            }
            let score = self.production_value(g, pid, cid, &candidate, plan, &counts);
            if score > best_score + 1e-9 {
                best = candidate;
                best_score = score;
            }
        }
        best
    }

    /// The ordinary menu keeps two district sites ranked by their own yield.
    /// When a city has a nearby owned Seowon, admit every legal alternative
    /// for the district kinds already offered, so a clean site can compete.
    pub(super) fn seowon_protection_expand_menu(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        items: &mut Vec<Item>,
    ) {
        if !self.districts_protect_seowon_science {
            return;
        }
        let families: BTreeSet<_> = items
            .iter()
            .filter_map(|item| match item {
                Item::District { district, .. } => Some(*district),
                _ => None,
            })
            .collect();
        for district in families {
            let sites = g.district_sites(cid, district);
            if !sites.iter().any(|pos| {
                self.seowon_site_loss_value(g, pid, district, *pos, GrandStrategy::Science) > 0.0
            }) {
                continue;
            }
            for pos in sites {
                let item = Item::District { district, pos };
                if !items.contains(&item) && g.can_produce(pid, cid, &item) {
                    items.push(item);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
