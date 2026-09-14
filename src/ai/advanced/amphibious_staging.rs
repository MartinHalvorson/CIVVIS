use super::AdvancedAi;
use crate::{game::Game, Pos};

impl AdvancedAi {
    /// An embarked field unit can assemble beside the enemy's landing shore
    /// when closed borders prevent a peacetime landing. The ordinary staging
    /// predicate still checks range, traversal technology, and legal borders.
    pub(super) fn amphibious_staging_position(
        g: &Game,
        target: usize,
        uid: u32,
        objective: Pos,
        position: Pos,
    ) -> bool {
        let spec = &g.rules.units[g.units[&uid].kind];
        if !matches!(spec.class.as_str(), "military" | "support")
            || matches!(spec.domain.as_deref(), Some("sea" | "air"))
            || !g.nbrs(objective).iter().any(|neighbor| {
                g.map
                    .get(*neighbor)
                    .is_some_and(|tile| g.rules.is_water(tile))
            })
        {
            return false;
        }
        g.nbrs(position).into_iter().any(|landing| {
            let Some(tile) = g.map.get(landing) else {
                return false;
            };
            !g.rules.is_water(tile)
                && g.rules.is_passable(tile)
                && g.city_at(landing).is_none()
                && g.unit_ids_at(landing).is_empty()
                && g.wdist(landing, objective) <= 5
                && tile
                    .owner_city
                    .and_then(|city| g.cities.get(&city))
                    .is_some_and(|city| city.owner == target)
        })
    }
}

#[cfg(test)]
mod tests;
