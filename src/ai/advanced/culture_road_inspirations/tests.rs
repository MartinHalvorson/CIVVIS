use super::super::test_support::opt_in_off_in_both_controllers;
use super::*;
use crate::game::Action;
use crate::name;

#[test]
fn culture_road_earns_its_inspirations_is_a_native_opt_in_off_in_both_controllers() {
    opt_in_off_in_both_controllers("culture-road-earns-its-inspirations", |ai| {
        ai.culture_road_earns_its_inspirations
    });
}

/// A founded Culture capital with `culture` and `science` a turn, the civics
/// and technologies named known with every prerequisite.
fn culture_seat(seed: u64, culture: f64, science: f64, civics: &[&str], techs: &[&str]) -> Game {
    let mut game = Game::new_full(2, 30, 16, seed, 200, 0, false);
    let position = game
        .map
        .tiles
        .values()
        .find(|tile| {
            game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
                && game.unit_ids_at(tile.pos).is_empty()
        })
        .map(|tile| tile.pos)
        .expect("a land tile");
    let settler = game.spawn_test_unit("settler", 0, position);
    game.current = 0;
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    for uid in game.player_unit_ids(0) {
        game.remove_unit(uid);
    }
    let cid = game.city_at(position).unwrap();
    std::sync::Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
        cid,
        crate::rules::Yields {
            culture,
            science,
            ..Default::default()
        },
    );
    let mut stack: Vec<Name> = civics.iter().map(|civic| Name::new(civic)).collect();
    while let Some(civic) = stack.pop() {
        if game.players[0].civics.insert(civic) {
            stack.extend(game.rules.civics[civic.as_str()].requires.iter().copied());
        }
    }
    let mut stack: Vec<Name> = techs.iter().map(|tech| Name::new(tech)).collect();
    while let Some(tech) = stack.pop() {
        if game.players[0].techs.insert(tech) {
            stack.extend(game.rules.techs[tech.as_str()].requires.iter().copied());
        }
    }
    game
}

fn culture_ai(enabled: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    if enabled {
        ai.enable_culture_road_earns_its_inspirations();
    }
    ai
}

/// Natural History, Urbanization and Scorched Earth known: Mass Media and
/// Mobilization both open, Radio out of reach. Culture fast enough that
/// either civic finishes in a turn.
fn corps_board(seed: u64, warriors: usize) -> Game {
    let mut game = culture_seat(
        seed,
        5_000.0,
        10.0,
        &["natural_history", "urbanization", "scorched_earth"],
        &[],
    );
    let city = game.player_city_ids(0)[0];
    let pos = game.cities[&city].pos;
    for _ in 0..warriors {
        game.spawn_test_unit("warrior", 0, pos);
    }
    game
}

#[test]
fn mass_media_goes_first_while_the_corps_form() {
    let game = corps_board(81_001, 6);
    let available = game.available_civics(0);
    assert!(available.contains(&name!("mass_media")));
    assert!(available.contains(&name!("mobilization")));
    let ai = culture_ai(true);
    assert!(ai.culture_road_active(&game));
    assert_eq!(AdvancedAi::corps_pairs(&game, 0), 3);
    let (eta, trigger) = ai
        .culture_road_inspiration_eta(&game, 0, "mobilization")
        .expect("three pairs stand ready to form the Corps");
    assert_eq!(trigger, "corps");
    assert!(eta > 1.0, "eta {eta}");
    // Radio is not legal, so Mass Media has nothing to wait for.
    assert!(ai
        .culture_road_inspiration_eta(&game, 0, "mass_media")
        .is_none());
    assert_eq!(
        ai.culture_road_pick(
            &game,
            0,
            &available,
            &name!("mobilization"),
            "cold_war",
            GrandStrategy::Culture,
        ),
        Some(name!("mass_media"))
    );
}

#[test]
fn without_the_pairs_mobilization_keeps_its_place() {
    let game = corps_board(81_002, 3);
    let available = game.available_civics(0);
    let ai = culture_ai(true);
    assert_eq!(AdvancedAi::corps_pairs(&game, 0), 1);
    assert!(ai
        .culture_road_inspiration_eta(&game, 0, "mobilization")
        .is_none());
    assert_eq!(
        ai.culture_road_pick(
            &game,
            0,
            &available,
            &name!("mobilization"),
            "cold_war",
            GrandStrategy::Culture,
        ),
        None
    );
}

#[test]
fn an_inspiration_landing_mid_research_does_not_wait() {
    // Slow Culture: Mobilization takes many turns, and the Corps land while
    // it is still being researched.
    let mut game = corps_board(81_003, 6);
    let city = game.player_city_ids(0)[0];
    std::sync::Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
        city,
        crate::rules::Yields {
            culture: 20.0,
            science: 10.0,
            ..Default::default()
        },
    );
    let available = game.available_civics(0);
    assert_eq!(
        culture_ai(true).culture_road_pick(
            &game,
            0,
            &available,
            &name!("mobilization"),
            "cold_war",
            GrandStrategy::Culture,
        ),
        None
    );
}

/// Mercantilism known with Printing: Colonialism and Civil Engineering open,
/// Astronomy legal and on the lane's Radio beeline.
fn astronomy_board(seed: u64) -> Game {
    culture_seat(
        seed,
        5_000.0,
        200.0,
        &["mercantilism"],
        &["printing", "education"],
    )
}

#[test]
fn astronomy_takes_the_research_slot_for_colonialism() {
    let game = astronomy_board(81_004);
    assert!(game.available_techs(0).contains(&name!("astronomy")));
    assert_eq!(
        culture_ai(true).culture_road_trigger_tech(&game, 0),
        Some((name!("astronomy"), name!("colonialism")))
    );
    assert_eq!(culture_ai(false).culture_road_trigger_tech(&game, 0), None);
    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_culture_road_earns_its_inspirations();
    assert_eq!(science.culture_road_trigger_tech(&game, 0), None);
}

#[test]
fn colonialism_waits_for_the_astronomy_in_research() {
    let mut game = astronomy_board(81_005);
    game.players[0].research = Some("astronomy".to_string());
    let available = game.available_civics(0);
    let ai = culture_ai(true);
    let (_, trigger) = ai
        .culture_road_inspiration_eta(&game, 0, "colonialism")
        .expect("Astronomy is in research");
    assert_eq!(trigger, "tech astronomy");
    let choice = ai.culture_road_pick(
        &game,
        0,
        &available,
        &name!("colonialism"),
        "cold_war",
        GrandStrategy::Culture,
    );
    assert!(
        choice
            .is_some_and(|civic| civic != "colonialism"
                && ai.civic_leads_to(&game, civic.as_str(), "cold_war")),
        "{:?}",
        choice.map(Name::as_str)
    );
}

#[test]
fn the_opening_does_not_buy_construction_for_games_and_recreation() {
    let game = culture_seat(
        81_006,
        5.0,
        5.0,
        &["early_empire", "code_of_laws"],
        &["masonry", "horseback_riding"],
    );
    assert!(game.available_techs(0).contains(&name!("construction")));
    assert_eq!(culture_ai(true).culture_road_trigger_tech(&game, 0), None);
}
