use super::*;
use crate::rules::Yields;
use crate::setup::GameSpeed;
use std::sync::Arc;

fn fixture(count: usize) -> (Game, AdvancedAi, StrategicPlan, Vec<u32>) {
    let mut g = Game::new_full(
        1,
        64,
        40,
        crate::rng::fixture_seed("CULTUREFAITH", 1),
        250,
        0,
        false,
    );
    g.players[0].civ = "Gran Colombia".into();
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    g.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    while g.player_city_ids(0).len() < count {
        let pos = g
            .map
            .tiles
            .values()
            .filter(|tile| {
                g.rules.is_passable(tile)
                    && !g.rules.is_water(tile)
                    && tile.owner_city.is_none()
                    && tile.district.is_none()
                    && tile.wonder.is_none()
                    && g.cities
                        .values()
                        .all(|city| g.wdist(city.pos, tile.pos) >= 5)
            })
            .map(|tile| tile.pos)
            .min()
            .expect("room for the fixture cities");
        g.found_city_for(0, pos, None);
    }
    for unit in g.player_unit_ids(0) {
        g.remove_unit(unit);
    }
    g.game_speed = GameSpeed::Online;
    g.turn = 100;
    for tech in [
        "astrology",
        "writing",
        "mining",
        "bronze_working",
        "irrigation",
        "pottery",
        "animal_husbandry",
    ] {
        g.players[0].techs.insert(Name::new(tech));
    }
    for civic in ["drama_poetry", "mysticism", "theology", "humanism"] {
        g.players[0].civics.insert(Name::new(civic));
    }
    let cities = g.player_city_ids(0);
    for &cid in &cities {
        g.cities.get_mut(&cid).unwrap().pop = 7;
        crate::game::install_test_district(&mut g, cid, "theater_square");
        Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
            cid,
            Yields {
                production: 20.0,
                ..Default::default()
            },
        );
    }
    g.cities
        .get_mut(&cities[0])
        .unwrap()
        .buildings
        .push(crate::name!("amphitheater"));
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.enable_culture_seat_theater_first();
    ai.base.pursue_religion = false;
    ai.base.skip_prophet_race = true;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Recovery,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: count,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, ai, plan, cities)
}

#[test]
fn culture_builds_faith_without_a_prophet_race_and_keeps_its_queue() {
    let (mut g, mut ai, plan, _) = fixture(3);
    let (cid, item, _) = ai
        .culture_faith_source_target(&g, 0, &plan)
        .expect("a faith source is buildable");
    assert!(
        matches!(&item, Item::District { district, .. } if g.district_family(*district) == "holy_site")
    );
    ai.claim_culture_faith_source(&mut g, 0, &plan);
    assert_eq!(g.cities[&cid].queue.first(), Some(&item));
    assert!(ai.culture_theater_holds(&g, 0, cid, &plan, &item));
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(
        g.cities[&cid].queue.first(),
        Some(&item),
        "routine Recovery rescoring must retain the source"
    );
    assert!(!ai.base.pursue_religion);
    assert!(g.players[0].religion.is_none());
}

#[test]
fn culture_finishes_shrine_and_temple_before_another_site() {
    let (mut g, ai, plan, cities) = fixture(3);
    let cid = cities[0];
    crate::game::install_test_district(&mut g, cid, "holy_site");
    let (_, shrine, _) = ai.culture_faith_source_target(&g, 0, &plan).unwrap();
    assert_eq!(
        shrine,
        Item::Building {
            building: crate::name!("shrine")
        }
    );
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("shrine"));
    let (_, temple, _) = ai.culture_faith_source_target(&g, 0, &plan).unwrap();
    assert_eq!(
        temple,
        Item::Building {
            building: crate::name!("temple")
        }
    );
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("temple"));
    assert!(ai.culture_faith_source_target(&g, 0, &plan).is_none());
}

#[test]
fn culture_faith_stops_at_three_sources_and_at_observed_income() {
    let (mut g, ai, plan, cities) = fixture(9);
    for &cid in cities.iter().take(3) {
        crate::game::install_test_district(&mut g, cid, "holy_site");
        g.cities
            .get_mut(&cid)
            .unwrap()
            .buildings
            .extend([crate::name!("shrine"), crate::name!("temple")]);
    }
    assert!(
        ai.culture_faith_source_target(&g, 0, &plan).is_none(),
        "three complete sources cap district investment"
    );
    let (mut g, ai, plan, _) = fixture(3);
    Arc::make_mut(&mut g.observed_yield_adjustments).insert(
        0,
        Yields {
            faith: 100.0,
            ..Default::default()
        },
    );
    assert!(
        ai.culture_faith_source_target(&g, 0, &plan).is_none(),
        "the native top-bar income includes non-city Faith"
    );
}

#[test]
fn culture_faith_preserves_survival_and_waits_for_writing_slots() {
    let (mut g, ai, mut plan, cities) = fixture(3);
    g.cities.get_mut(&cities[0]).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("settler"),
    }];
    g.cities.get_mut(&cities[1]).unwrap().queue = vec![Item::Building {
        building: crate::name!("walls"),
    }];
    plan.threatened_city = Some(cities[2]);
    assert!(ai.culture_faith_source_target(&g, 0, &plan).is_none());
    let (mut g, ai, plan, cities) = fixture(3);
    g.cities
        .get_mut(&cities[0])
        .unwrap()
        .buildings
        .retain(|building| *building != "amphitheater");
    assert!(ai.culture_faith_source_target(&g, 0, &plan).is_none());
    g.cities
        .get_mut(&cities[0])
        .unwrap()
        .buildings
        .push(crate::name!("amphitheater"));
    g.players[0].gold_per_turn = -10.0;
    g.players[0].gold = 0.0;
    assert!(ai.culture_faith_source_target(&g, 0, &plan).is_none());
}

#[test]
fn culture_faith_leaves_other_lanes_and_disabled_claims_alone() {
    let (g, mut ai, plan, _) = fixture(3);
    ai.disable_culture_seat_theater_first();
    assert!(ai.culture_faith_source_target(&g, 0, &plan).is_none());
    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_culture_seat_theater_first();
    assert!(science.culture_faith_source_target(&g, 0, &plan).is_none());
}

#[test]
fn only_the_selected_faith_factory_yields_its_campus_slot() {
    let (mut g, mut ai, plan, cities) = fixture(3);
    ai.enable_campus_claims_the_free_slot_2();
    let (source, _, _) = ai.culture_faith_source_target(&g, 0, &plan).unwrap();
    ai.claim_free_slot_campus(&mut g, 0, &plan);
    ai.claim_culture_faith_source(&mut g, 0, &plan);
    assert!(g.cities[&source]
        .queue
        .first()
        .is_some_and(|item| AdvancedAi::culture_faith_item(&g, item)));
    assert!(cities.iter().any(|cid| *cid != source && matches!(g.cities[cid].queue.first(), Some(Item::District { district, .. }) if g.district_family(*district) == "campus")));
}
