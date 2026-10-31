use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::VictoryTarget;

/// Our city and a rival on one grassland continent, a second rival's city
/// behind a band of ocean.
fn two_shores() -> Game {
    let mut g = Game::new_full(3, 40, 24, 936_611, 650, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for (pos, tile) in g.map.tiles.iter_mut() {
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.terrain = if (20..=24).contains(&pos.0) {
            crate::name!("ocean")
        } else {
            crate::name!("grassland")
        };
    }
    g.found_city_for(0, (10, 12), None);
    g.found_city_for(1, (16, 12), None);
    g.found_city_for(2, (30, 12), None);
    for pid in 0..3 {
        for other in 0..3 {
            if pid != other {
                g.players[pid].met.insert(other);
            }
        }
        g.players[pid].explored.extend(g.map.tiles.keys().copied());
    }
    g.current = 0;
    g.turn = 60;
    g
}

#[test]
fn domination_targets_land_first_is_opt_in() {
    opt_in_off_in_both_controllers("domination-targets-land-first", |ai| {
        ai.domination_targets_land_first
    });
}

/// Under the gene, on a Domination seat, the rival across the water ranks
/// after the one on our land; off, or on another lane, both rank alike.
#[test]
fn a_rival_across_water_ranks_after_one_on_our_land() {
    let g = two_shores();
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    assert!(
        ai.rival_reachable_by_land(&g, 0, 1),
        "fixture: rival 1 shares our land"
    );
    assert!(
        !ai.rival_reachable_by_land(&g, 0, 2),
        "fixture: rival 2 is overseas"
    );
    assert_eq!(ai.domination_land_rank(&g, 0, 2), 0, "off");
    ai.enable_domination_targets_land_first();
    assert_eq!(ai.domination_land_rank(&g, 0, 1), 0);
    assert_eq!(ai.domination_land_rank(&g, 0, 2), 1);
    let mut culture = AdvancedAi::targeting(VictoryTarget::Culture);
    culture.enable_domination_targets_land_first();
    assert_eq!(culture.domination_land_rank(&g, 0, 2), 0, "a Culture seat");
}
