//! Tests of `advanced/battlefront.rs`: each calls methods of that module and no
//! other domain module's.
//!
//! Moved verbatim out of `advanced/tests.rs` (2026-11-04); a `super::` path
//! there is `super::super::` here. Shared fixtures stay in `tests.rs` and
//! arrive through `use super::*`.

use super::*;

/// The defect this exists for: a walled enemy capital the army cannot
/// currently see scored identically to an empty meadow, because
/// `local_strength_ratio` reached its `hostile <= 0.0` sentinel and
/// returned 3.0 — four times the superiority floor. One lone warrior
/// therefore read itself as locally dominant over a city.
#[test]
fn a_fogged_objective_city_is_priced_from_the_last_sighting() {
    let (mut game, _capital, home) = empire_with_a_capital(71_103);
    // An enemy city far enough away that nothing of ours can see it.
    let mut sites: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(position, tile)| {
            game.wdist(**position, home) >= 15
                && game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
        })
        .map(|(position, _)| *position)
        .collect();
    sites.sort_unstable();
    game.current = 1;
    for site in sites {
        let settler = game.spawn_test_unit("settler", 1, site);
        if game.apply(1, &Action::FoundCity { unit: settler }).is_ok() {
            break;
        }
        game.remove_unit(settler);
    }
    game.current = 0;
    let enemy_city = *game
        .player_city_ids(1)
        .first()
        .expect("the enemy must have somewhere to be attacked");
    let objective = game.cities[&enemy_city].pos;
    for unit in game.player_unit_ids(1) {
        game.remove_unit(unit);
    }

    // Korea's Seoul was no frontier outpost, so neither is this: an empire
    // fielding field cannons defends its cities at that standard.
    game.players[1]
        .counters
        .insert("strongest_unit_built".to_string(), 60);

    // One warrior, standing well outside its own sight range of the city.
    let warrior = game.spawn_test_unit("warrior", 0, anchor_at(&game, home, 6));
    assert!(
        !game.player_can_see(0, objective),
        "the setup must leave the objective fogged"
    );

    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    assert!(
        !ai.blind_objective_strength,
        "the shipped default must be unchanged"
    );
    assert_eq!(
        ai.local_strength_ratio(&game, 0, &[warrior], &[1], objective),
        3.0,
        "the shipped rule reads a fogged city as no enemy at all"
    );

    // A scout walks up, sees the city, and leaves. Nothing about the city
    // changed — only what this controller has observed of it.
    let watchtower = game
        .nbrs(objective)
        .into_iter()
        .find(|position| game.map.get(*position).is_some())
        .expect("a tile beside the city");
    let scout = game.spawn_test_unit("scout", 0, watchtower);
    let in_sight = ai.local_strength_ratio(&game, 0, &[warrior], &[1], objective);
    ai.belief.observe(&game, 0);
    game.remove_unit(scout);
    assert!(
        !game.player_can_see(0, objective),
        "the city is fogged again once the scout is gone"
    );
    assert!(
        ai.remembered_city(enemy_city).is_some(),
        "the sighting must survive the fog"
    );

    ai.enable_blind_objective_strength();
    let ratio = ai.local_strength_ratio(&game, 0, &[warrior], &[1], objective);
    assert_eq!(
        ratio, in_sight,
        "an unchanged city must read the same remembered as it did in sight"
    );
    assert!(
        ratio < LOCAL_SUPERIORITY_FLOOR,
        "one warrior is not locally superior to a city it remembers: {ratio}"
    );

    // Memory is still the preferred input, but a city this controller has
    // never seen receives the generic floor rather than scoring as absent.
    let mut unseen = AdvancedAi::targeting(VictoryTarget::Domination);
    unseen.enable_blind_objective_strength();
    let unseen_ratio = unseen.local_strength_ratio(&game, 0, &[warrior], &[1], objective);
    assert!(
        unseen_ratio < LOCAL_SUPERIORITY_FLOOR,
        "one warrior must not read an unseen city as locally dominant: {unseen_ratio}"
    );
}

/// `city_pressure` is the number `threatened_city` already computed and
/// discarded. It must read zero in peace, so a directive stamped every
/// turn of a quiet game costs the governor nothing.
#[test]
fn city_pressure_is_zero_with_no_hostile_force_in_reach() {
    let mut game = Game::new_full(2, 24, 16, 411_004, 120, 1, false);
    found_test_city(&mut game, 0);
    for cid in game.player_city_ids(0) {
        assert_eq!(AdvancedAi::city_pressure(&game, 0, cid), 0.0);
    }
}
