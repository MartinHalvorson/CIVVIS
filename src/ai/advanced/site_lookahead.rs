//! Shared district wishlist for settlement and district planning.

use super::{AdvancedAi, GrandStrategy};
use crate::name::Name;

impl AdvancedAi {
    /// The district families a new city on this lane would build, first first,
    /// with the share of full weight each carries in the planner.
    pub(super) fn new_city_district_wishlist(strategy: GrandStrategy) -> Vec<(Name, f64)> {
        let rows: &[(&str, f64)] = match strategy {
            GrandStrategy::Science => &[
                ("campus", 1.0),
                ("commercial_hub", 0.5),
                ("industrial_zone", 0.5),
            ],
            GrandStrategy::Culture => &[
                ("theater_square", 1.0),
                ("campus", 0.6),
                ("commercial_hub", 0.4),
            ],
            GrandStrategy::Religion => {
                &[("holy_site", 1.0), ("campus", 0.5), ("commercial_hub", 0.3)]
            }
            GrandStrategy::Diplomacy => {
                &[("commercial_hub", 0.8), ("harbor", 0.6), ("campus", 0.6)]
            }
            GrandStrategy::Conquest => &[
                ("industrial_zone", 0.8),
                ("campus", 0.6),
                ("commercial_hub", 0.4),
            ],
            GrandStrategy::Recovery => &[("industrial_zone", 1.0), ("campus", 0.4)],
            GrandStrategy::Expansion => {
                &[("campus", 0.8), ("commercial_hub", 0.7), ("harbor", 0.5)]
            }
        };
        rows.iter()
            .map(|(family, weight)| (Name::new(family), *weight))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::Game;

    #[test]
    fn every_lane_wishes_for_adjacency_bearing_districts_first_first() {
        let game = Game::new_full(2, 28, 18, 91_779, 200, 0, false);
        for strategy in [
            GrandStrategy::Science,
            GrandStrategy::Culture,
            GrandStrategy::Religion,
            GrandStrategy::Diplomacy,
            GrandStrategy::Conquest,
            GrandStrategy::Recovery,
            GrandStrategy::Expansion,
        ] {
            let wishlist = AdvancedAi::new_city_district_wishlist(strategy);
            assert!(!wishlist.is_empty());
            assert_eq!(
                wishlist[0].1,
                wishlist.iter().map(|(_, w)| *w).fold(0.0, f64::max),
                "{strategy:?}: the first family carries the full weight"
            );
            for (family, weight) in wishlist {
                let spec = game
                    .rules
                    .districts
                    .get(family.as_str())
                    .unwrap_or_else(|| panic!("{strategy:?} wishes for unknown {family}"));
                assert!(
                    !spec.adjacency.is_empty(),
                    "{family} has no adjacency to plan"
                );
                assert!(weight > 0.0 && weight <= 1.0);
            }
        }
    }
}
