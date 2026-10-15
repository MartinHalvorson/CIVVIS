use super::*;
use crate::ai::VictoryTarget;

/// A four-major board at the opening: our lone capital with the two opening
/// techs (Animal Husbandry and Mining), every rival founded, no Prophet
/// claimed, and the Domination seat admitted to the race by
/// `enter-the-prophet-race-2`.
fn opening() -> (Game, AdvancedAi, StrategicPlan) {
    let mut game = Game::new_full(4, 34, 20, 76_108, 120, 0, false);
    for pid in 0..4 {
        let settler = game
            .player_unit_ids(pid)
            .into_iter()
            .find(|unit| game.units[unit].kind == "settler")
            .unwrap();
        let at = game.units[&settler].pos;
        game.remove_unit(settler);
        game.found_city_for(pid, at, None);
    }
    for tech in ["animal_husbandry", "mining"] {
        game.players[0].techs.insert(Name::new(tech));
    }
    game.players[0].research = None;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Conquest,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: game.turn,
        rush: false,
    };
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_enter_the_prophet_race_2();
    (game, ai, plan)
}

/// Live Emperor G202334Z at t10: one city, two techs, the race open, and the
/// research slot went to Pottery; Astrology waited for the second city.
#[test]
fn a_lone_capital_researches_astrology_for_an_open_race() {
    let (game, mut ai, plan) = opening();
    assert!(
        !ai.prophet_race_enterable_for(&game, 0, Some(VictoryTarget::Domination)),
        "the race itself still waits for the second city"
    );

    let mut off = game.clone();
    ai.advanced_research(&mut off, 0, &plan);
    assert_ne!(
        off.players[0].research.as_deref(),
        Some("astrology"),
        "off: unchanged"
    );

    ai.enable_astrology_before_the_second_city();
    assert!(ai.astrology_before_the_second_city_due(&game, 0));
    let mut on = game.clone();
    ai.advanced_research(&mut on, 0, &plan);
    assert_eq!(on.players[0].research.as_deref(), Some("astrology"));
    assert!(
        !ai.religious_opening_viable(&on, 0),
        "only the research fee moves: the Holy Site still waits for the second city"
    );
}

#[test]
fn the_capital_waits_for_the_opening_techs() {
    let (mut game, mut ai, plan) = opening();
    ai.enable_astrology_before_the_second_city();
    game.players[0].techs.remove(&Name::new("mining"));
    assert!(!ai.astrology_before_the_second_city_due(&game, 0));
    ai.advanced_research(&mut game, 0, &plan);
    assert_ne!(game.players[0].research.as_deref(), Some("astrology"));
}

#[test]
fn a_closed_race_or_a_science_lane_keeps_the_beeline() {
    let (game, mut ai, _) = opening();
    ai.enable_astrology_before_the_second_city();

    let mut closed = game.clone();
    for (pid, faith) in [(1, "First Faith"), (2, "Second Faith"), (3, "Third Faith")] {
        closed.players[pid].religion = Some(faith.to_string());
    }
    if closed.religions_founded() >= closed.max_religions() {
        assert!(
            !ai.astrology_before_the_second_city_due(&closed, 0),
            "no Prophet is left to race for"
        );
    }

    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_enter_the_prophet_race_2();
    science.enable_astrology_before_the_second_city();
    assert!(
        !science.astrology_before_the_second_city_due(&game, 0),
        "the Science lane never enters the race"
    );
}

#[test]
fn a_second_city_hands_the_slot_back_to_the_race_itself() {
    let (mut game, mut ai, _) = opening();
    ai.enable_astrology_before_the_second_city();
    let capital = game.player_city_ids(0)[0];
    let anchor = game.cities[&capital].pos;
    let site = game
        .map
        .tiles
        .iter()
        .filter(|(_, tile)| {
            tile.owner_city.is_none() && game.rules.is_passable(tile) && !game.rules.is_water(tile)
        })
        .map(|(position, _)| *position)
        .find(|position| {
            (4..=10).contains(&game.wdist(anchor, *position))
                && game
                    .cities
                    .values()
                    .all(|city| game.wdist(city.pos, *position) >= 4)
        })
        .expect("a second city site near the capital");
    game.found_city_for(0, site, None);
    assert!(
        !ai.astrology_before_the_second_city_due(&game, 0),
        "two cities are the race's own gate"
    );
}

/// `prophet-race-takes-divine-spark`: a pantheon board with Religious
/// Settlements already held by a rival, as in live G202334Z (Fertility Rites
/// at t20, the race lost at t40 on 18 of 30 Prophet points).
fn pantheon_board() -> Game {
    let mut game = Game::new_full(2, 30, 18, 6_101, 200, 0, false);
    game.current = 0;
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    game.players[0].faith = 200.0;
    std::sync::Arc::make_mut(&mut game.blocked_pantheons)
        .insert(crate::name!("religious_settlements"));
    game
}

#[test]
fn a_racing_seat_takes_divine_spark_behind_the_founding_pantheon() {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.base.pursue_religion = true;
    ai.enable_expansion_pantheon();

    let mut shipped = pantheon_board();
    ai.base
        .research_with_government(&mut shipped, 0, false, None);
    assert_eq!(
        shipped.players[0].pantheon.as_deref(),
        Some("fertility_rites"),
        "off: the founding pantheons' order"
    );

    ai.base.pantheon_for_the_prophet_race = true;
    let mut racing = pantheon_board();
    ai.base
        .research_with_government(&mut racing, 0, false, None);
    assert_eq!(racing.players[0].pantheon.as_deref(), Some("divine_spark"));

    let mut free = pantheon_board();
    std::sync::Arc::make_mut(&mut free.blocked_pantheons).clear();
    ai.base.research_with_government(&mut free, 0, false, None);
    assert_eq!(
        free.players[0].pantheon.as_deref(),
        Some("religious_settlements"),
        "a free Settler still leads"
    );
}

#[test]
fn the_race_flag_follows_the_race() {
    let (mut game, mut ai, _) = opening();
    ai.enable_prophet_race_earns_its_points();
    ai.take_turn_inner(&mut game, 0);
    assert!(!ai.base.pantheon_for_the_prophet_race, "off");

    ai.enable_prophet_race_takes_divine_spark();
    let mut racing = game.clone();
    ai.take_turn_inner(&mut racing, 0);
    assert!(ai.base.pantheon_for_the_prophet_race);

    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_enter_the_prophet_race_2();
    science.enable_prophet_race_earns_its_points();
    science.enable_prophet_race_takes_divine_spark();
    let mut quiet = game.clone();
    science.take_turn_inner(&mut quiet, 0);
    assert!(
        !science.base.pantheon_for_the_prophet_race,
        "a lane outside the race keeps its pantheon order"
    );
}
