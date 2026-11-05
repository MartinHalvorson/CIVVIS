//! Keep a new native Science Campus above known coastal lowlands when a
//! safe legal alternative exists. Native 173830Z placed Ternate's Campus
//! on lowland despite three safe offers. It flooded at turn 138; the host
//! refused repair, and the Campus and its buildings stayed pillaged at 167.
//! This opt-in changes a proposed site without adding a district claim.

use super::*;

impl AdvancedAi {
    pub fn native_campus_above_sea_position(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        proposed: Pos,
    ) -> Option<Pos> {
        if !self.campus_above_the_sea
            || self.active_victory_target(g) != Some(VictoryTarget::Science)
        {
            return None;
        }
        let city = g.cities.get(&cid)?;
        if city.owner != pid {
            return None;
        }
        let family = crate::name!("campus");
        let district = g.civ_district_variant(pid, "campus");
        let committed = g.city_has_district_family(city, family)
            || city.queue.iter().any(|item| {
                matches!(item,
                Item::District { district, .. } if g.district_family(*district) == family)
            })
            || city.owned_tiles.iter().any(|pos| {
                g.map
                    .get(*pos)
                    .and_then(|tile| {
                        tile.district.or_else(|| {
                            tile.district_foundation
                                .as_ref()
                                .map(|foundation| foundation.district)
                        })
                    })
                    .is_some_and(|placed| g.district_family(placed) == family)
            });
        if committed
            || g.map.get(proposed)?.coastal_lowland == 0
            || (city.buildings.contains(&crate::name!("flood_barrier"))
                && !city
                    .pillaged_buildings
                    .contains(&crate::name!("flood_barrier")))
            || !g.can_produce(
                pid,
                cid,
                &Item::District {
                    district,
                    pos: proposed,
                },
            )
        {
            return None;
        }
        // Host-offered district sites supply legality and unique replacements.
        // The ordinary production menu keeps only two fresh sites: filtering
        // that shortlist could hide every safe alternative behind lowlands.
        // Among safe offers retain the ordinary Campus picker's yield ranking
        // and positional tie-break, rather than choosing an arbitrary safe tile.
        // A fresh native mirror has not gone through AI turn setup. Seed the
        // optional Indonesia ranking context on a private copy, without
        // changing the caller's board or the ordinary gene-off ranking.
        let nusantara_board = (self.nusantara_reads_the_coast && g.players[pid].civ == "Indonesia")
            .then(|| {
                let mut board = g.clone();
                self.seat_nusantara_site_planner(&mut board, pid);
                board
            });
        let site_yields = |pos| match &nusantara_board {
            Some(board) => board.district_site_yields(district, pos),
            None => g.district_yields(district, pos),
        };
        g.district_sites(cid, district)
            .into_iter()
            .filter(|pos| {
                g.map.get(*pos).is_some_and(|tile| {
                    tile.coastal_lowland == 0 && !tile.flooded && !tile.submerged
                }) && g.can_produce(
                    pid,
                    cid,
                    &Item::District {
                        district,
                        pos: *pos,
                    },
                )
            })
            .max_by(|a, b| {
                site_yields(*a)
                    .total()
                    .total_cmp(&site_yields(*b).total())
                    .then(a.cmp(b))
            })
    }
}

#[cfg(test)]
mod tests;
