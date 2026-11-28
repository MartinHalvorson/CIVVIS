use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

#[test]
fn culture_core_ends_expansion_is_opt_in() {
    opt_in_off_in_both_controllers("culture-core-ends-expansion", |ai| {
        ai.culture_core_ends_expansion
    });
}

/// A Culture seat on open land at turn 40 of 250, short of its city target.
/// Below the core it expands either way; at the core the gene hands the plan
/// to the lane while the stock seat keeps reading Expansion. The city target
/// the Settler pipeline reads does not move.
#[test]
fn a_culture_core_follows_the_lane() {
    let mut game = Game::new_full(2, 40, 28, 7_931, 250, 0, false);
    for pid in 0..2 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        game.found_city_for(pid, game.units[&settler].pos, None);
        game.remove_unit(settler);
    }
    let all: Vec<_> = game.map.tiles.keys().copied().collect();
    game.players[0].explored.extend(all);
    game.game_speed = crate::setup::GameSpeed::Online;
    let capital = game.player_city_ids(0)[0];
    let home = game.cities[&capital].pos;
    let mut sites = game
        .map
        .tiles
        .iter()
        .filter(|(position, tile)| {
            tile.owner_city.is_none()
                && game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && (4..=8).contains(&game.wdist(home, **position))
        })
        .map(|(position, _)| *position)
        .collect::<Vec<_>>();
    sites.sort();
    let found_next = |game: &mut Game| {
        let site = sites
            .iter()
            .copied()
            .find(|site| {
                game.cities
                    .values()
                    .all(|city| game.wdist(city.pos, *site) >= 4)
            })
            .expect("the test map has another city site");
        game.found_city_for(0, site, None)
    };
    for _ in 0..2 {
        found_next(&mut game);
    }
    assert_eq!(game.player_city_ids(0).len(), CULTURE_CORE_CITIES - 1);
    game.current = 0;
    game.turn = 40;

    let mut stock = AdvancedAi::targeting(VictoryTarget::Culture);
    stock.city_target_floor = 8;
    let mut core = AdvancedAi::targeting(VictoryTarget::Culture);
    core.city_target_floor = 8;
    core.enable_culture_core_ends_expansion();
    assert!(!stock.phase_specialization_active(&game));
    let below = stock.assess(&game, 0);
    assert_eq!(
        below.strategy,
        GrandStrategy::Expansion,
        "the fixture is the reviewed case: a Culture seat in its Expansion half"
    );
    assert!(below.desired_cities > CULTURE_CORE_CITIES);
    assert_eq!(
        core.assess(&game, 0).strategy,
        GrandStrategy::Expansion,
        "below the core the gene expands as before"
    );

    found_next(&mut game);
    assert_eq!(game.player_city_ids(0).len(), CULTURE_CORE_CITIES);
    let stock_plan = stock.assess(&game, 0);
    assert_eq!(
        stock_plan.strategy,
        GrandStrategy::Expansion,
        "off: unchanged"
    );
    let journal = crate::reasoning::Journal::recording();
    core.attach_journal(journal.handle());
    let core_plan = core.assess(&game, 0);
    assert_eq!(
        core_plan.strategy,
        GrandStrategy::Culture,
        "on: the core follows its lane"
    );
    assert_eq!(
        core_plan.desired_cities, stock_plan.desired_cities,
        "the city target the Settlers read is unchanged"
    );
    assert!(journal.since(0).thoughts.iter().any(|thought| thought
        .detail
        .contains("following the assigned victory lane")));
}

/// Only an assigned Culture lane: the same four cities on a Science seat
/// are untouched.
#[test]
fn only_the_culture_lane_moves() {
    let ai = {
        let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
        ai.enable_culture_core_ends_expansion();
        ai
    };
    assert!(!ai.culture_core_ends_expansion_for(Some(VictoryTarget::Science), 6));
    assert!(!ai.culture_core_ends_expansion_for(None, 6));
    assert!(ai.culture_core_ends_expansion_for(Some(VictoryTarget::Culture), 4));
    assert!(!ai.culture_core_ends_expansion_for(Some(VictoryTarget::Culture), 3));
    let off = AdvancedAi::targeting(VictoryTarget::Culture);
    assert!(!off.culture_core_ends_expansion_for(Some(VictoryTarget::Culture), 9));
}
