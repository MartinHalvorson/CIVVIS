use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;

#[test]
fn religion_core_ends_expansion_is_opt_in() {
    opt_in_off_in_both_controllers("religion-core-ends-expansion", |ai| {
        ai.religion_core_ends_expansion
    });
}

/// A Religion seat on open land at turn 40 of 250, short of its city target.
/// Before founding it races for its religion either way; once founded and
/// below the core it expands either way; at the core the gene hands the plan
/// to the lane while the stock seat keeps reading Expansion. The city target
/// the Settler pipeline reads does not move.
#[test]
fn a_founded_religion_core_follows_the_lane() {
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
    found_next(&mut game);
    assert_eq!(game.player_city_ids(0).len(), RELIGION_CORE_CITIES - 1);
    game.current = 0;
    game.turn = 40;

    let mut stock = AdvancedAi::targeting(VictoryTarget::Religion);
    stock.city_target_floor = 8;
    let mut core = AdvancedAi::targeting(VictoryTarget::Religion);
    core.city_target_floor = 8;
    core.enable_religion_core_ends_expansion();
    assert!(!stock.phase_specialization_active(&game));

    game.players[0].religion = Some("Home Faith".to_string());
    let below = stock.assess(&game, 0);
    assert_eq!(
        below.strategy,
        GrandStrategy::Expansion,
        "the fixture is the reviewed case: a founded Religion seat in its Expansion half"
    );
    assert!(below.desired_cities > RELIGION_CORE_CITIES);
    assert_eq!(
        core.assess(&game, 0).strategy,
        GrandStrategy::Expansion,
        "below the core the gene expands as before"
    );

    found_next(&mut game);
    assert_eq!(game.player_city_ids(0).len(), RELIGION_CORE_CITIES);

    // Before founding nothing changes: the Prophet race reads Religion.
    game.players[0].religion = None;
    for ai in [&stock, &core] {
        assert_eq!(ai.assess(&game, 0).strategy, GrandStrategy::Religion);
    }

    game.players[0].religion = Some("Home Faith".to_string());
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
        GrandStrategy::Religion,
        "on: the founded core follows its lane"
    );
    assert_eq!(
        core_plan.desired_cities, stock_plan.desired_cities,
        "the city target the Settlers read is unchanged"
    );
    assert!(journal.since(0).thoughts.iter().any(|thought| thought
        .detail
        .contains("following the assigned victory lane")));
}

/// Only a founded Religion lane at its core: the same cities on a Science
/// seat, or before founding, are untouched.
#[test]
fn only_the_founded_religion_lane_moves() {
    let ai = {
        let mut ai = AdvancedAi::targeting(VictoryTarget::Religion);
        ai.enable_religion_core_ends_expansion();
        ai
    };
    assert!(!ai.religion_core_ends_expansion_for(Some(VictoryTarget::Science), true, 6));
    assert!(!ai.religion_core_ends_expansion_for(None, true, 6));
    assert!(!ai.religion_core_ends_expansion_for(Some(VictoryTarget::Religion), false, 6));
    assert!(ai.religion_core_ends_expansion_for(Some(VictoryTarget::Religion), true, 3));
    assert!(!ai.religion_core_ends_expansion_for(Some(VictoryTarget::Religion), true, 2));
    let off = AdvancedAi::targeting(VictoryTarget::Religion);
    assert!(!off.religion_core_ends_expansion_for(Some(VictoryTarget::Religion), true, 9));
}
