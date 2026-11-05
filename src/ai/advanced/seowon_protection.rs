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
        let mut loss = 0.0;
        for seowon_pos in g.nbrs(pos) {
            let Some(neighbor) = g.map.get(seowon_pos) else {
                continue;
            };
            let kind = neighbor
                .district
                .or_else(|| neighbor.district_foundation.as_ref().map(|f| f.district));
            if kind != Some(crate::name!("seowon"))
                || !neighbor
                    .owner_city
                    .and_then(|cid| g.cities.get(&cid))
                    .is_some_and(|city| city.owner == pid)
            {
                continue;
            }
            let before = g.district_adjacency_assuming(
                crate::name!("seowon"),
                seowon_pos,
                Some(&assume),
                None,
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
                None,
                crate::name!("campus"),
                &neighbors,
            );
            loss += (self.yield_value(before, lane) - self.yield_value(after, lane)).max(0.0);
        }
        loss
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
mod tests {
    use super::*;
    use crate::ai::advanced::{StrategicPlan, VictoryTarget};
    use crate::rules::Yields;
    use crate::world::DistrictFoundation;

    fn fixture() -> (Game, u32, Pos, Pos) {
        let mut g = Game::new_full(2, 28, 18, 91779, 250, 0, false);
        for tile in g.map.tiles.values_mut() {
            tile.terrain = crate::name!("plains");
            tile.hills = false;
            tile.feature = None;
            tile.resource = None;
            tile.improvement = None;
            tile.district = None;
            tile.district_foundation = None;
            tile.owner_city = None;
            tile.river_edges = [false; 6];
        }
        g.players[0].civ = "Korea".to_string();
        let center = (8, 8);
        let cid = g.found_city_for(0, center, None);
        let positions = g.wdisk(center, 3);
        for pos in &positions {
            g.map.tiles.get_mut(pos).unwrap().owner_city = Some(cid);
        }
        let city = g.cities.get_mut(&cid).unwrap();
        city.owned_tiles = positions;
        city.pop = 10;
        let seowon = (10, 8);
        let site = (10, 7);
        let tile = g.map.tiles.get_mut(&seowon).unwrap();
        tile.hills = true;
        tile.district = Some(crate::name!("seowon"));
        g.cities
            .get_mut(&cid)
            .unwrap()
            .districts
            .insert(crate::name!("seowon"), seowon);
        g.players[0].techs.insert(crate::name!("writing"));
        g.players[0].civics.insert(crate::name!("drama_poetry"));
        (g, cid, seowon, site)
    }

    fn on() -> AdvancedAi {
        let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
        ai.enable_districts_protect_seowon_science();
        ai
    }

    fn loss(ai: &AdvancedAi, g: &Game, site: Pos, district: &str) -> f64 {
        ai.seowon_site_loss_value(g, 0, Name::new(district), site, GrandStrategy::Science)
    }

    #[test]
    fn seowon_protection_uses_the_engine_loss_and_policy_multiplier() {
        let (mut g, _, seowon, site) = fixture();
        let ai = on();
        let one = ai.yield_value(
            Yields {
                science: 1.0,
                ..Yields::default()
            },
            GrandStrategy::Science,
        );
        assert!((loss(&ai, &g, site, "theater_square") - one).abs() < 1e-9);
        g.players[0]
            .policies
            .insert(crate::name!("natural_philosophy"));
        let quoted = loss(&ai, &g, site, "theater_square");
        assert!((quoted - 2.0 * one).abs() < 1e-9);
        let before = g.district_yields(crate::name!("seowon"), seowon).science;
        g.map.tiles.get_mut(&site).unwrap().district = Some(crate::name!("theater_square"));
        let after = g.district_yields(crate::name!("seowon"), seowon).science;
        assert_eq!(
            before - after,
            2.0,
            "the projected loss equals actual engine output"
        );
    }

    #[test]
    fn seowon_protection_leaves_plazas_distant_sites_and_control_unchanged() {
        let (g, _, _, site) = fixture();
        let ai = on();
        assert_eq!(loss(&ai, &g, site, "government_plaza"), 0.0);
        assert_eq!(loss(&ai, &g, (6, 8), "theater_square"), 0.0);
        let control = AdvancedAi::targeting(VictoryTarget::Science);
        assert_eq!(loss(&control, &g, site, "theater_square"), 0.0);
    }

    #[test]
    fn seowon_protection_prices_started_seowons_without_recharging_placed_districts() {
        let (mut g, cid, seowon, site) = fixture();
        let ai = on();
        let before = loss(&ai, &g, site, "theater_square");
        g.cities
            .get_mut(&cid)
            .unwrap()
            .districts
            .remove(crate::name!("seowon"));
        let tile = g.map.tiles.get_mut(&seowon).unwrap();
        tile.district = None;
        tile.district_foundation = Some(DistrictFoundation {
            district: crate::name!("seowon"),
            cost: 27.0,
        });
        assert_eq!(loss(&ai, &g, site, "theater_square"), before);
        g.map.tiles.get_mut(&site).unwrap().district_foundation = Some(DistrictFoundation {
            district: crate::name!("theater_square"),
            cost: 54.0,
        });
        assert_eq!(loss(&ai, &g, site, "theater_square"), 0.0);
    }

    #[test]
    fn seowon_protection_counts_multiple_owned_neighbors_but_not_rivals() {
        let (mut g, cid, _, site) = fixture();
        let ai = on();
        let one = loss(&ai, &g, site, "theater_square");
        let second = (11, 7);
        let tile = g.map.tiles.get_mut(&second).unwrap();
        tile.district_foundation = Some(DistrictFoundation {
            district: crate::name!("seowon"),
            cost: 27.0,
        });
        assert_eq!(loss(&ai, &g, site, "theater_square"), 2.0 * one);
        g.cities.get_mut(&cid).unwrap().owner = 1;
        assert_eq!(loss(&ai, &g, site, "theater_square"), 0.0);
    }

    #[test]
    fn seowon_protection_changes_production_site_preference_and_exposes_alternatives() {
        let (g, cid, _, site) = fixture();
        let ai = on();
        let control = AdvancedAi::targeting(VictoryTarget::Science);
        let plan = StrategicPlan {
            strategy: GrandStrategy::Science,
            target_player: None,
            target_city: None,
            threatened_city: None,
            desired_cities: 4,
            assessed_turn: g.turn,
            rush: false,
        };
        let counts = ai.counts(&g, 0);
        let crowded = Item::District {
            district: crate::name!("theater_square"),
            pos: site,
        };
        let clean = Item::District {
            district: crate::name!("theater_square"),
            pos: (6, 8),
        };
        let a = control.production_value(&g, 0, cid, &crowded, &plan, &counts);
        let b = ai.production_value(&g, 0, cid, &crowded, &plan, &counts);
        assert!(
            b < a,
            "the production scorer actually charges the externality"
        );
        assert_eq!(
            control.production_value(&g, 0, cid, &clean, &plan, &counts),
            ai.production_value(&g, 0, cid, &clean, &plan, &counts)
        );
        assert!(
            ai.production_value(&g, 0, cid, &clean, &plan, &counts) > b,
            "the clean district site must beat the crowded site in the Science lane"
        );
        let mut menu = vec![crowded.clone()];
        control.seowon_protection_expand_menu(&g, 0, cid, &mut menu);
        assert_eq!(menu, vec![crowded], "the control retains its exact menu");
        ai.seowon_protection_expand_menu(&g, 0, cid, &mut menu);
        assert!(
            menu.contains(&clean),
            "a clean legal site survives the two-site shortlist"
        );
        assert!(menu.iter().all(|item| g.can_produce(0, cid, item)));
    }
}
