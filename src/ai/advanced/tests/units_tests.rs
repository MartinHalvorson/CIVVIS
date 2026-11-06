//! Tests of `advanced/units.rs`: each calls methods of that module and no
//! other domain module's.
//!
//! Moved verbatim out of `advanced/tests.rs` (2026-11-04); a `super::` path
//! there is `super::super::` here. Shared fixtures stay in `tests.rs` and
//! arrive through `use super::*`.

use super::*;

#[test]
fn tactical_melee_preflight_matches_the_engine_entry_cost_rule() {
    let mut game = Game::new_full(2, 24, 16, 81_701, 80, 0, false);
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
    }
    game.current = 0;
    game.at_war.insert((0, 1));
    let target = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| game.nbrs(*position).len() == 6)
        .expect("fixture needs an interior tile");
    let origin = game.nbrs(target)[0];
    let attacker = game.spawn_test_unit("warrior", 0, origin);
    game.spawn_test_unit("warrior", 1, target);
    game.map.tiles.get_mut(&target).unwrap().feature = Some(crate::name!("forest"));
    assert!(game.map.set_river_edge(origin, target, true));
    let attack = Action::Attack {
        unit: attacker,
        target,
    };
    let engine_offers_attack = |game: &Game| {
        game.legal_actions(0)
            .into_iter()
            .any(|action| action == attack)
    };

    game.units.get_mut(&attacker).unwrap().moves_left = 1.0;
    assert!(!engine_offers_attack(&game));
    assert!(
        !AdvancedAi::tactical_melee_candidate_is_legal(&game, 0, attacker, target),
        "the tactical scan must not score an attack that cannot pay its entry cost"
    );

    game.units.get_mut(&attacker).unwrap().moves_left = 2.0;
    assert!(engine_offers_attack(&game));
    assert!(
        AdvancedAi::tactical_melee_candidate_is_legal(&game, 0, attacker, target),
        "the preflight must retain the full-movement attack the engine allows"
    );
}

#[test]
fn tactical_melee_preflight_rejects_a_peaceful_bystander_plot() {
    let mut game = Game::new_full(3, 24, 16, 81_702, 80, 0, false);
    for unit in game.units.keys().copied().collect::<Vec<_>>() {
        game.remove_unit(unit);
    }
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
    }
    game.current = 0;
    game.at_war.insert((0, 1));
    let target = game
        .map
        .tiles
        .keys()
        .copied()
        .find(|position| game.nbrs(*position).len() == 6)
        .expect("fixture needs an interior tile");
    let origin = game.nbrs(target)[0];
    let attacker = game.spawn_test_unit("warrior", 0, origin);
    game.spawn_test_unit("warrior", 1, target);
    game.spawn_test_unit("builder", 2, target);

    let attack = Action::Attack {
        unit: attacker,
        target,
    };
    assert!(game.peaceful_foreign_unit_at(0, target));
    assert!(
        !game
            .legal_actions(0)
            .into_iter()
            .any(|action| action == attack),
        "the engine must not offer a plot-addressed strike through a peaceful bystander"
    );
    assert!(
        !AdvancedAi::tactical_melee_candidate_is_legal(&game, 0, attacker, target),
        "the tactical scorer must use the same peaceful-bystander veto"
    );
}
