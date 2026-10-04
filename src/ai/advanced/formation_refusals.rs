//! `formations-heed-refusals`: a pair of units the live host refused to
//! combine is not combined again while the refusal stands. See
//! `civvis_orders/formation_refusals.rs`: the planner's combine spends both
//! units' turn, so each refused Corps cost two units a turn, every turn,
//! while the host kept them apart.

use super::AdvancedAi;
use std::collections::BTreeSet;

impl AdvancedAi {
    /// The live bridge's pairs of this board's units (lower id first) whose
    /// Corps or Army the host refused. Empty off the bridge.
    pub fn set_refused_combinations(&mut self, pairs: BTreeSet<(u32, u32)>) {
        self.refused_combinations = pairs;
    }

    /// Whether `formations-heed-refusals` keeps `a` and `b` apart.
    pub(super) fn combination_refused(&self, a: u32, b: u32) -> bool {
        self.formations_heed_refusals && self.refused_combinations.contains(&(a.min(b), a.max(b)))
    }
}

#[cfg(test)]
mod tests {
    use super::super::AdvancedAi;
    use crate::game::Game;
    use std::collections::BTreeSet;

    /// Two adjacent warriors under Nationalism: the pair forms a Corps
    /// unless the host refused it and the gene is on.
    #[test]
    fn a_refused_pair_is_not_combined_again() {
        for (gene, refused, combined) in [
            (false, true, true),
            (true, false, true),
            (true, true, false),
        ] {
            let mut g = Game::new_full(1, 24, 16, 8_120, 120, 0, false);
            for id in g.units.keys().copied().collect::<Vec<_>>() {
                g.remove_unit(id);
            }
            for tile in g.map.tiles.values_mut() {
                tile.terrain = crate::name!("grassland");
                tile.feature = None;
                tile.hills = false;
            }
            g.players[0].civics.insert(crate::name!("nationalism"));
            g.found_city_for(0, (3, 3), None);
            // Enough military that the formation reserve leaves one pair.
            let mut ids = Vec::new();
            for x in 0..10 {
                ids.push(g.spawn_test_unit("warrior", 0, (6 + x, 8)));
            }
            let mut ai = AdvancedAi::new();
            if gene {
                ai.enable_formations_heed_refusals();
            }
            if refused {
                let pairs = ids
                    .windows(2)
                    .map(|pair| (pair[0].min(pair[1]), pair[0].max(pair[1])))
                    .collect::<BTreeSet<_>>();
                ai.set_refused_combinations(pairs);
            }
            ai.advanced_formations(&mut g, 0);
            let corps = ids
                .iter()
                .filter(|uid| g.units.get(uid).is_some_and(|unit| unit.formation > 0))
                .count();
            assert_eq!(corps > 0, combined, "gene {gene}, refused {refused}");
        }
    }
}
