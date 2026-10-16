use super::*;

fn grant_tech(game: &mut Game, tech: &str) {
    let prerequisites = game.rules.techs[tech].requires.clone();
    for prerequisite in prerequisites {
        grant_tech(game, &prerequisite);
    }
    game.players[0].techs.insert(Name::new(tech));
}

fn space_person(kind: &str) -> (Game, u32) {
    let mut game = Game::new_full(2, 28, 18, 407_200, 300, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    let city = game.player_city_ids(0)[0];
    game.turn = 178;
    game.players[0].gold = 0.0;
    game.cities.get_mut(&city).unwrap().pop = 10;
    for position in game.cities[&city].owned_tiles.clone() {
        if position == game.cities[&city].pos {
            continue;
        }
        let tile = game.map.tiles.get_mut(&position).unwrap();
        tile.terrain = crate::name!("plains");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
        tile.district = None;
        tile.district_foundation = None;
        tile.wonder = None;
    }
    grant_tech(&mut game, "rocketry");
    game.players[0].live_great_person_activation_needs.push(
        crate::game::LiveGreatPersonActivationNeed {
            kind: kind.into(),
            individual: Some(if kind == "scientist" {
                "stephanie_kwolek".into()
            } else {
                "sergei_korolev".into()
            }),
            required_district: Some("spaceport".into()),
            ..crate::game::LiveGreatPersonActivationNeed::default()
        },
    );
    assert!(BasicAi::live_great_person_district_item(&game, 0, city, "spaceport").is_some());
    (game, city)
}

fn is_spaceport(game: &Game, item: &Item) -> bool {
    matches!(item, Item::District { district, .. } if game.district_family(*district) == "spaceport")
}

fn busy_granary(game: &mut Game, city: u32) -> Item {
    let item = Item::Building {
        building: crate::name!("granary"),
    };
    assert!(game.can_produce(0, city, &item));
    game.apply(
        0,
        &Action::Produce {
            city,
            item: item.clone(),
        },
    )
    .unwrap();
    game.cities.get_mut(&city).unwrap().production = 10.0;
    item
}

#[test]
fn domination_exclusion_blocks_new_space_person_paths_in_idle_cities() {
    for kind in ["scientist", "engineer"] {
        let (game, city) = space_person(kind);
        let mut ai = BasicAi::new();
        ai.exclude_space_race = true;
        assert!(ai
            .live_great_person_activation_item(&game, 0, city)
            .is_none());
    }
}

#[test]
fn domination_exclusion_keeps_a_busy_nonspace_queue() {
    let (mut game, city) = space_person("scientist");
    let queued = busy_granary(&mut game, city);
    let mut ai = BasicAi::new();
    ai.exclude_space_race = true;
    assert!(ai
        .live_great_person_activation_resume_item(&game, 0, city)
        .is_none());
    assert!(!ai.prioritize_live_great_person_activation(&mut game, 0));
    assert_eq!(game.cities[&city].queue.first(), Some(&queued));
}

#[test]
fn domination_exclusion_does_not_resume_a_paused_spaceport_for_a_person() {
    let (mut game, city) = space_person("engineer");
    let queued = busy_granary(&mut game, city);
    let item = BasicAi::live_great_person_district_item(&game, 0, city, "spaceport").unwrap();
    let Item::District { district, pos } = item else {
        unreachable!()
    };
    let cost = game.item_cost_for_city(0, city, &Item::District { district, pos });
    game.map.tiles.get_mut(&pos).unwrap().district_foundation =
        Some(crate::world::DistrictFoundation { district, cost });
    let mut ai = BasicAi::new();
    ai.exclude_space_race = true;
    assert!(ai
        .live_great_person_activation_resume_item(&game, 0, city)
        .is_none());
    assert_eq!(game.cities[&city].queue.first(), Some(&queued));
}

#[test]
fn named_domination_caller_cannot_open_a_spaceport_for_kwolek() {
    let (mut game, city) = space_person("scientist");
    let mut control = game.clone();
    assert!(BasicAi::new().prioritize_live_great_person_activation(&mut control, 0));
    assert!(is_spaceport(
        &control,
        control.cities[&city].queue.first().unwrap()
    ));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.take_turn(&mut game, 0);
    assert!(game.cities[&city]
        .queue
        .iter()
        .all(|item| !is_spaceport(&game, item)));
    assert!(game.cities[&city].owned_tiles.iter().all(|position| {
        game.map.tiles[position]
            .district_foundation
            .as_ref()
            .is_none_or(|foundation| game.district_family(foundation.district) != "spaceport")
    }));
}

#[test]
fn science_and_untargeted_controllers_keep_space_person_paths() {
    let (mut game, city) = space_person("scientist");
    assert!(BasicAi::new().prioritize_live_great_person_activation(&mut game, 0));
    assert!(is_spaceport(
        &game,
        game.cities[&city].queue.first().unwrap()
    ));
    let (mut science, city) = space_person("scientist");
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.take_turn(&mut science, 0);
    assert!(science.cities[&city]
        .queue
        .iter()
        .any(|item| is_spaceport(&science, item)));
}

#[test]
fn domination_exclusion_keeps_nonspace_scientist_prerequisites() {
    let (mut game, city) = space_person("scientist");
    let need = &mut game.players[0].live_great_person_activation_needs[0];
    need.individual = Some("hypatia".into());
    need.required_district = Some("campus".into());
    let mut ai = BasicAi::new();
    ai.exclude_space_race = true;
    let item = ai
        .live_great_person_activation_item(&game, 0, city)
        .unwrap();
    assert!(
        matches!(item, Item::District { district, .. } if game.district_family(district) == "campus")
    );
}

#[test]
fn an_active_spaceport_queue_is_not_canceled_by_the_production_exclusion() {
    let (mut game, city) = space_person("scientist");
    let item = BasicAi::live_great_person_district_item(&game, 0, city, "spaceport").unwrap();
    game.apply(
        0,
        &Action::Produce {
            city,
            item: item.clone(),
        },
    )
    .unwrap();
    let mut ai = BasicAi::new();
    ai.exclude_space_race = true;
    assert!(!ai.prioritize_live_great_person_activation(&mut game, 0));
    assert_eq!(game.cities[&city].queue.first(), Some(&item));
}

#[test]
fn the_targeted_caller_restores_the_delegated_governor_context() {
    let (mut game, _) = space_person("scientist");
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    assert!(!ai.base.exclude_space_race);
    ai.take_turn(&mut game, 0);
    assert!(!ai.base.exclude_space_race);
}
