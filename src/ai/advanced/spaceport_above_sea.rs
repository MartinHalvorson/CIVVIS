//! A new native Spaceport need not occupy a coastal lowland when another
//! legal site exists. In the resumed 171007Z Science win, the original pad
//! was built on known one-metre lowland despite four other native offers.
//! Its tile flooded at t176 and was submerged at t191. This opt-in changes
//! only a proposed fresh site's position; it makes no saved-turn claim.

use super::*;

impl AdvancedAi {
    pub fn native_spaceport_above_sea_position(
        &self,
        g: &Game,
        pid: usize,
        cid: u32,
        proposed: Pos,
    ) -> Option<Pos> {
        if !self.spaceport_above_the_sea {
            return None;
        }
        let city = g.cities.get(&cid)?;
        let pad = crate::name!("spaceport");
        if city.owner != pid
            || city.districts.keys().any(|d| g.district_family(*d) == pad)
            || city.queue.iter().any(|item| {
                matches!(item, Item::District { district, .. }
                    if g.district_family(*district) == pad)
            })
            || (city.buildings.contains(&crate::name!("flood_barrier"))
                && !city
                    .pillaged_buildings
                    .contains(&crate::name!("flood_barrier")))
            || g.map.get(proposed)?.coastal_lowland == 0
            || !g.can_produce(
                pid,
                cid,
                &Item::District {
                    district: pad,
                    pos: proposed,
                },
            )
        {
            return None;
        }
        // The host-constrained production menu supplies legality. Keep the
        // existing positional tie-break among safe sites, never invent a plot
        // or switch the producing city. No safe offer releases the override.
        g.producible_items(pid, cid)
            .into_iter()
            .filter_map(|item| match item {
                Item::District { district, pos } if g.district_family(district) == pad => g
                    .map
                    .get(pos)
                    .filter(|tile| tile.coastal_lowland == 0 && !tile.flooded && !tile.submerged)
                    .map(|_| pos),
                _ => None,
            })
            .min()
    }
}

#[cfg(test)]
mod tests;
