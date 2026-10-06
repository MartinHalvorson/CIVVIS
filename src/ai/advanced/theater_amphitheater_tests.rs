//! `theater-keeps-its-amphitheater`: on the Domination seat the Great Work
//! veto passes over an Amphitheater owed to a standing Theater Square, and
//! nothing else. See `AdvancedAi::theater_keeps_its_amphitheater`.
use super::*;
use crate::game::install_test_district;

#[test]
fn theater_keeps_its_amphitheater_is_a_native_opt_in_off_in_both_controllers() {
    super::test_support::opt_in_off_in_both_controllers("theater-keeps-its-amphitheater", |ai| {
        ai.theater_keeps_its_amphitheater
    });
}

fn plan(strategy: GrandStrategy) -> StrategicPlan {
    StrategicPlan {
        strategy,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: 90,
        rush: false,
    }
}

fn controller(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    // The live seat carries the Theater Square's debt; the veto is what
    // kept it from ever applying.
    ai.enable_culture_building_debt();
    if gene {
        ai.enable_theater_keeps_its_amphitheater();
    }
    ai
}

/// A one-city empire that knows Drama and Poetry and Humanism, so both the
/// Amphitheater and (once it stands) the Art Museum are in reach.
fn theater_city(seed: u64) -> (Game, u32) {
    let mut game = Game::new_full(2, 74, 46, seed, 200, 0, false);
    game.current = 0;
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .expect("starting settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("found city");
    let city = game.player_city_ids(0)[0];
    for unit in game.player_unit_ids(0) {
        game.remove_unit(unit);
    }
    game.players[0].civics.insert(crate::name!("drama_poetry"));
    game.players[0].civics.insert(crate::name!("humanism"));
    game.players[0].techs.insert(crate::name!("writing"));
    (game, city)
}

/// The live Emperor shape: Domination target, Conquest and Expansion
/// strategies. Off, the veto refuses the Amphitheater; on, the city's
/// standing Theater Square is owed it and the debts that already price it
/// apply.
#[test]
fn a_standing_theater_square_is_owed_its_amphitheater_on_the_domination_seat() {
    let (mut game, city) = theater_city(71_401);
    install_test_district(&mut game, city, "theater_square");
    let amphitheater = Item::Building {
        building: crate::name!("amphitheater"),
    };
    assert!(game.can_produce(0, city, &amphitheater));
    let counts = EmpireCounts::default();
    for strategy in [GrandStrategy::Conquest, GrandStrategy::Expansion] {
        let off = controller(VictoryTarget::Domination, false);
        let on = controller(VictoryTarget::Domination, true);
        let vetoed = off.production_value(&game, 0, city, &amphitheater, &plan(strategy), &counts);
        let owed = on.production_value(&game, 0, city, &amphitheater, &plan(strategy), &counts);
        assert_eq!(vetoed, -10_000.0, "{strategy:?}: off, the veto holds");
        assert!(
            owed > 0.0,
            "{strategy:?}: on, the Amphitheater is priced: {owed}"
        );
    }
}

/// Only the Amphitheater a standing square is owed: the Art Museum keeps the
/// veto, and so does every other lane's handling of the Culture seat.
#[test]
fn the_art_museum_keeps_the_veto_and_the_culture_seat_is_unchanged() {
    let (mut game, city) = theater_city(71_402);
    install_test_district(&mut game, city, "theater_square");
    game.cities
        .get_mut(&city)
        .unwrap()
        .buildings
        .push(crate::name!("amphitheater"));
    let museum = Item::Building {
        building: crate::name!("art_museum"),
    };
    assert!(game.can_produce(0, city, &museum));
    let counts = EmpireCounts::default();
    let on = controller(VictoryTarget::Domination, true);
    assert_eq!(
        on.production_value(
            &game,
            0,
            city,
            &museum,
            &plan(GrandStrategy::Conquest),
            &counts
        ),
        -10_000.0,
        "the gene is the Amphitheater only"
    );

    // The Culture seat never vetoed it: the gene changes nothing there.
    let (mut game, city) = theater_city(71_403);
    install_test_district(&mut game, city, "theater_square");
    let amphitheater = Item::Building {
        building: crate::name!("amphitheater"),
    };
    let off = controller(VictoryTarget::Culture, false);
    let on = controller(VictoryTarget::Culture, true);
    assert_eq!(
        off.production_value(
            &game,
            0,
            city,
            &amphitheater,
            &plan(GrandStrategy::Culture),
            &counts
        ),
        on.production_value(
            &game,
            0,
            city,
            &amphitheater,
            &plan(GrandStrategy::Culture),
            &counts
        ),
    );
}
