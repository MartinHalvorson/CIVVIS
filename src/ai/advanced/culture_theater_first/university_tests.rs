use super::*;
use crate::game::{install_test_district, LiveGreatPersonActivationNeed};

fn university_ready() -> (Game, Vec<u32>, Item, Item, AdvancedAi, StrategicPlan) {
    let (mut game, cities, museum) = theaters_ready_for_art();
    game.players[0].techs.insert(crate::name!("education"));
    for cid in &cities {
        game.cities
            .get_mut(cid)
            .unwrap()
            .buildings
            .push(crate::name!("granary"));
    }
    install_test_district(&mut game, cities[0], "campus");
    game.cities
        .get_mut(&cities[0])
        .unwrap()
        .buildings
        .push(crate::name!("library"));
    Arc::make_mut(&mut game.observed_city_yield_adjustments)
        .get_mut(&cities[0])
        .unwrap()
        .production = 20.0;
    let university = Item::Building {
        building: crate::name!("university"),
    };
    assert!(game.can_produce(0, cities[0], &university));
    let mut ai = seat(VictoryTarget::Culture, true);
    ai.enable_campus_building_claims_the_queue();
    let mut plan = ai.assess(&game, 0);
    plan.threatened_city = None;
    (game, cities, university, museum, ai, plan)
}

#[test]
fn the_museum_claim_keeps_the_selected_university_and_uses_another_city() {
    let (mut game, cities, university, museum, ai, plan) = university_ready();
    ai.claim_campus_building(&mut game, 0, &plan);
    assert_eq!(game.cities[&cities[0]].queue.first(), Some(&university));
    ai.claim_culture_theaters(&mut game, 0, &plan);
    assert_eq!(game.cities[&cities[0]].queue.first(), Some(&university));
    assert_eq!(
        cities[1..]
            .iter()
            .filter(|cid| game.cities[cid].queue.first() == Some(&museum))
            .count(),
        1,
        "the first art slots still open in another city"
    );
}

#[test]
fn a_waiting_artist_or_musician_keeps_the_first_museum_priority() {
    for kind in ["artist", "musician"] {
        let (mut game, cities, university, museum, ai, plan) = university_ready();
        game.players[0]
            .live_great_person_activation_needs
            .push(LiveGreatPersonActivationNeed {
                kind: kind.into(),
                required_district: Some("theater_square".into()),
                ..Default::default()
            });
        ai.claim_campus_building(&mut game, 0, &plan);
        assert_eq!(game.cities[&cities[0]].queue.first(), Some(&university));
        ai.claim_culture_theaters(&mut game, 0, &plan);
        assert_eq!(
            game.cities[&cities[0]].queue.first(),
            Some(&museum),
            "{kind}"
        );
    }
}

#[test]
fn a_released_or_disabled_campus_claim_leaves_the_museum_priority() {
    for release in [false, true] {
        let (mut game, cities, university, museum, mut ai, plan) = university_ready();
        game.cities.get_mut(&cities[0]).unwrap().queue = vec![university];
        if release {
            game.players[0]
                .techs
                .extend(["flight", "radio", "computers"].map(Name::new));
        } else {
            ai.disable_campus_building_claims_the_queue();
        }
        ai.claim_culture_theaters(&mut game, 0, &plan);
        assert_eq!(
            game.cities[&cities[0]].queue.first(),
            Some(&museum),
            "release={release}"
        );
    }
}
