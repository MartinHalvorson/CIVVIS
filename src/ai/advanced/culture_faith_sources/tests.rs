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
fn first_art_museum_keeps_priority_over_the_faith_factory() {
    let (mut g, mut ai, plan, cities) = fixture(3);
    let museum = Item::Building {
        building: crate::name!("art_museum"),
    };
    assert!(g.can_produce(0, cities[0], &museum));
    ai.claim_culture_theaters(&mut g, 0, &plan);
    assert_eq!(g.cities[&cities[0]].queue.first(), Some(&museum));
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&cities[0]].queue.first(), Some(&museum));
}

#[test]
fn only_the_selected_faith_factory_yields_its_campus_slot() {
    // The ordinary Campus claim waits for the six-city expansion floor.
    let (mut g, mut ai, plan, cities) = fixture(6);
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

fn quoted_parallel_fixture() -> (Game, AdvancedAi, StrategicPlan, u32, Item, u32, Item) {
    let (mut g, ai, plan, cities) = fixture(6);
    let (first, existing, _) = ai.culture_faith_source_target(&g, 0, &plan).unwrap();
    g.apply(
        0,
        &Action::Produce {
            city: first,
            item: existing.clone(),
        },
    )
    .unwrap();
    let (second, next) = cities
        .into_iter()
        .filter(|c| *c != first)
        .find_map(|c| BasicAi::first_district_item(&g, 0, c, "holy_site").map(|item| (c, item)))
        .unwrap();
    Arc::make_mut(&mut g.host_buildable)
        .entry(second)
        .or_default()
        .insert(
            Game::production_block_key(&next),
            crate::game::HostMenuEntry {
                cost: Some(70.0),
                turns: Some(3.0),
            },
        );
    (g, ai, plan, first, existing, second, next)
}

#[test]
fn native_quote_opens_a_second_faith_pipeline_and_preserves_the_first() {
    let (mut g, ai, plan, first, existing, second, next) = quoted_parallel_fixture();
    let target = ai.culture_faith_source_target(&g, 0, &plan).unwrap();
    assert_eq!((target.0, target.1), (second, next.clone()));
    ai.claim_culture_faith_source(&mut g, 0, &plan);
    assert!(ai.culture_faith_source_holds(&g, 0, first, &plan, &existing));
    assert!(ai.culture_faith_source_holds(&g, 0, second, &plan, &next));
    assert_eq!(
        g.player_city_ids(0)
            .iter()
            .filter(|c| g.cities[c]
                .queue
                .first()
                .is_some_and(|i| AdvancedAi::culture_faith_item(&g, i)))
            .count(),
        2
    );
    assert!(
        [first, second].contains(&ai.culture_faith_source_target(&g, 0, &plan).unwrap().0),
        "two pipelines already fill this six-city source budget"
    );
}

#[test]
fn no_native_quote_keeps_the_serialized_headless_commitment() {
    let (mut g, ai, plan, first, existing, _, _) = quoted_parallel_fixture();
    Arc::make_mut(&mut g.host_buildable).clear();
    let target = ai.culture_faith_source_target(&g, 0, &plan).unwrap();
    assert_eq!((target.0, target.1), (first, existing));
}

#[test]
fn a_threatened_or_slow_native_second_queue_keeps_the_existing_commitment() {
    let (mut g, ai, mut plan, first, _, second, next) = quoted_parallel_fixture();
    plan.threatened_city = Some(second);
    assert_eq!(
        ai.culture_faith_source_target(&g, 0, &plan).unwrap().0,
        first
    );
    plan.threatened_city = None;
    Arc::make_mut(&mut g.host_buildable)
        .get_mut(&second)
        .unwrap()
        .get_mut(&Game::production_block_key(&next))
        .unwrap()
        .turns = Some(30.0);
    assert_eq!(
        ai.culture_faith_source_target(&g, 0, &plan).unwrap().0,
        first
    );
}

#[test]
fn two_theaters_open_the_sources_before_an_amphitheater_when_the_gene_is_on() {
    let (mut g, mut ai, plan, cities) = fixture(3);
    for &cid in &cities {
        g.cities
            .get_mut(&cid)
            .unwrap()
            .buildings
            .retain(|building| *building != "amphitheater");
    }
    assert!(ai.culture_faith_source_target(&g, 0, &plan).is_none());
    ai.enable_culture_faith_opens_at_the_theaters();
    assert!(ai.culture_faith_source_target(&g, 0, &plan).is_some());
}

fn complete_source(g: &mut Game, cid: u32) {
    crate::game::install_test_district(g, cid, "holy_site");
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .extend([crate::name!("shrine"), crate::name!("temple")]);
}

#[test]
fn per_theater_budget_opens_a_site_in_every_theater_city() {
    let (mut g, mut ai, plan, cities) = fixture(4);
    complete_source(&mut g, cities[0]);
    assert_eq!(ai.culture_faith_source_budget(&g, 0), Some(1));
    assert!(
        ai.culture_faith_source_target(&g, 0, &plan).is_none(),
        "off: one site per three cities"
    );
    ai.enable_culture_faith_site_per_theater();
    assert_eq!(
        ai.culture_faith_source_budget(&g, 0),
        Some(4),
        "on: one site per city with a standing Theater Square"
    );
    // Only cities with a legal Holy Site plot can take one; the fixture's
    // small territories leave some without.
    let open = cities[1..]
        .iter()
        .filter(|cid| BasicAi::first_district_item(&g, 0, **cid, "holy_site").is_some())
        .count();
    assert!(open >= 1, "the fixture leaves a second site to claim");
    let mut sites = 1;
    while let Some((city, item, _)) = ai.culture_faith_source_target(&g, 0, &plan) {
        assert_ne!(city, cities[0]);
        assert!(
            matches!(&item, Item::District { district, .. } if g.district_family(*district) == "holy_site")
        );
        complete_source(&mut g, city);
        sites += 1;
    }
    assert_eq!(sites, 1 + open, "every open Theater city gets its site");
}

#[test]
fn per_theater_budget_never_outruns_the_theaters() {
    let (mut g, mut ai, plan, cities) = fixture(4);
    ai.enable_culture_faith_site_per_theater();
    let bare = cities[3];
    let removed = g
        .cities
        .get_mut(&bare)
        .unwrap()
        .districts
        .remove(crate::name!("theater_square"))
        .unwrap();
    for pos in removed {
        g.map.tiles.get_mut(&pos).unwrap().district = None;
    }
    for &cid in &cities[..3] {
        complete_source(&mut g, cid);
    }
    assert!(
        ai.culture_faith_source_target(&g, 0, &plan).is_none(),
        "three Theater cities, three sites: the fourth city builds its Theater first"
    );
}

#[test]
fn per_theater_ceiling_is_twenty_a_city() {
    let (mut g, mut ai, plan, _) = fixture(3);
    Arc::make_mut(&mut g.observed_yield_adjustments).insert(
        0,
        Yields {
            faith: 40.0,
            ..Default::default()
        },
    );
    assert!(
        ai.culture_faith_source_target(&g, 0, &plan).is_none(),
        "off: eight a city stops three cities at 24"
    );
    ai.enable_culture_faith_site_per_theater();
    assert!(ai.culture_faith_source_target(&g, 0, &plan).is_some());
    Arc::make_mut(&mut g.observed_yield_adjustments).insert(
        0,
        Yields {
            faith: 60.0,
            ..Default::default()
        },
    );
    assert!(ai.culture_faith_source_target(&g, 0, &plan).is_none());
}

#[test]
fn per_theater_opens_two_theater_cities() {
    let (g, mut ai, plan, _) = fixture(2);
    assert!(ai.culture_faith_source_target(&g, 0, &plan).is_none());
    ai.enable_culture_faith_site_per_theater();
    assert!(ai.culture_faith_source_target(&g, 0, &plan).is_some());
}

#[test]
fn per_theater_researches_astrology_once_a_theater_stands() {
    let (mut g, mut ai, _, cities) = fixture(3);
    g.players[0].techs.remove(&crate::name!("astrology"));
    assert!(
        !ai.culture_faith_astrology_due(&g, 0),
        "off: no research change"
    );
    ai.enable_culture_faith_site_per_theater();
    assert!(ai.culture_faith_astrology_due(&g, 0));
    for &cid in &cities {
        let removed = g
            .cities
            .get_mut(&cid)
            .unwrap()
            .districts
            .remove(crate::name!("theater_square"))
            .unwrap();
        for pos in removed {
            g.map.tiles.get_mut(&pos).unwrap().district = None;
        }
    }
    assert!(
        !ai.culture_faith_astrology_due(&g, 0),
        "no Theater Square stands yet"
    );
    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_culture_seat_theater_first();
    science.enable_culture_faith_site_per_theater();
    let (mut g, _, _, _) = fixture(3);
    g.players[0].techs.remove(&crate::name!("astrology"));
    assert!(!science.culture_faith_astrology_due(&g, 0));
}

/// Claim what the target names, the way `claim_culture_faith_source` does.
fn claim(g: &mut Game, ai: &AdvancedAi, plan: &StrategicPlan) -> Option<u32> {
    let (city, item, _) = ai.culture_faith_source_target(g, 0, plan)?;
    if g.cities[&city].queue.first() == Some(&item) {
        return None;
    }
    g.apply(0, &Action::Produce { city, item }).unwrap();
    Some(city)
}

#[test]
fn version_two_claims_a_source_in_every_theater_city_at_once() {
    let (mut g, mut ai, plan, cities) = fixture(4);
    ai.enable_culture_faith_site_per_theater();
    let first = claim(&mut g, &ai, &plan).expect("the first source");
    assert!(
        claim(&mut g, &ai, &plan).is_none(),
        "version one without a host quote keeps one source in the empire"
    );
    ai.enable_culture_faith_site_per_theater_2();
    let mut claimed = vec![first];
    while let Some(city) = claim(&mut g, &ai, &plan) {
        assert!(!claimed.contains(&city), "one source per city at a time");
        claimed.push(city);
    }
    assert!(
        claimed.len() >= 2 && claimed.len() <= cities.len(),
        "a second Theater city starts its source: {claimed:?}"
    );
}

#[test]
fn version_two_carries_version_one() {
    let (mut g, mut ai, plan, _) = fixture(3);
    g.players[0].techs.remove(&crate::name!("astrology"));
    ai.enable_culture_faith_site_per_theater_2();
    assert!(ai.culture_faith_astrology_due(&g, 0));
    assert_eq!(ai.culture_faith_source_budget(&g, 0), Some(3));
    g.players[0].techs.insert(crate::name!("astrology"));
    assert!(ai.culture_faith_source_target(&g, 0, &plan).is_some());
}

#[test]
fn version_two_goes_ahead_of_a_wonder_but_not_a_spy() {
    let (mut g, mut ai, plan, cities) = fixture(3);
    ai.enable_culture_faith_site_per_theater();
    for &cid in &cities {
        let pos = g.cities[&cid].pos;
        g.cities.get_mut(&cid).unwrap().queue = vec![Item::Wonder {
            wonder: crate::name!("pyramids"),
            pos,
        }];
    }
    assert!(
        ai.culture_faith_source_target(&g, 0, &plan).is_none(),
        "version one keeps a wonder"
    );
    ai.enable_culture_faith_site_per_theater_2();
    let (city, item, _) = ai
        .culture_faith_source_target(&g, 0, &plan)
        .expect("version two claims a source over the wonder");
    assert!(cities.contains(&city));
    assert!(AdvancedAi::culture_faith_item(&g, &item));
    for &cid in &cities {
        g.cities.get_mut(&cid).unwrap().queue = vec![Item::Unit {
            unit: crate::name!("spy"),
        }];
    }
    assert!(
        ai.culture_faith_source_target(&g, 0, &plan).is_none(),
        "the Spy pipeline takes a passed queue back, so a Spy is kept"
    );
}

#[test]
fn version_two_keeps_a_settler() {
    let (mut g, mut ai, plan, cities) = fixture(3);
    ai.enable_culture_faith_site_per_theater_2();
    for &cid in &cities {
        g.cities.get_mut(&cid).unwrap().queue = vec![Item::Unit {
            unit: crate::name!("settler"),
        }];
    }
    assert!(ai.culture_faith_source_target(&g, 0, &plan).is_none());
}

fn shrine_board() -> (Game, AdvancedAi, StrategicPlan, u32) {
    let (mut g, mut ai, plan, cities) = fixture(3);
    ai.enable_culture_gold_buys_the_shrines();
    let cid = cities[0];
    crate::game::install_test_district(&mut g, cid, "holy_site");
    g.players[0].gold = 1_000.0;
    g.players[0].gold_per_turn = 10.0;
    (g, ai, plan, cid)
}

#[test]
fn gold_buys_the_shrine_then_the_temple_behind_a_standing_holy_site() {
    let (mut g, ai, plan, cid) = shrine_board();
    let (city, building, price) = ai
        .culture_shrine_purchase(&g, 0, &plan)
        .expect("a Shrine is for sale");
    assert_eq!((city, building), (cid, crate::name!("shrine")));
    assert_eq!(Some(price), g.building_gold_purchase_cost(0, cid, "shrine"));
    assert!(ai.buy_culture_shrine(&mut g, 0, &plan));
    assert!(g.cities[&cid].buildings.contains(&crate::name!("shrine")));
    assert_eq!(g.players[0].gold, 1_000.0 - price);
    let (_, temple, _) = ai.culture_shrine_purchase(&g, 0, &plan).unwrap();
    assert_eq!(temple, crate::name!("temple"));
    assert!(ai.buy_culture_shrine(&mut g, 0, &plan));
    assert!(g.cities[&cid].buildings.contains(&crate::name!("temple")));
    assert!(
        ai.culture_shrine_purchase(&g, 0, &plan).is_none(),
        "nothing left to buy in the only Holy Site city"
    );
}

#[test]
fn gold_buys_a_queued_shrine_and_the_claim_moves_on() {
    let (mut g, ai, plan, cid) = shrine_board();
    ai.claim_culture_faith_source(&mut g, 0, &plan);
    let shrine = Item::Building {
        building: crate::name!("shrine"),
    };
    assert_eq!(g.cities[&cid].queue.first(), Some(&shrine));
    assert!(ai.buy_culture_shrine(&mut g, 0, &plan));
    assert!(g.cities[&cid].buildings.contains(&crate::name!("shrine")));
    assert_ne!(g.cities[&cid].queue.first(), Some(&shrine));
}

#[test]
fn gold_keeps_its_reserve_and_leaves_other_seats_alone() {
    let (g, ai, plan, cid) = shrine_board();
    let price = g.building_gold_purchase_cost(0, cid, "shrine").unwrap();
    let reserve = ai.purchase_upkeep_floor(&g, 0, 0.0) + CULTURE_SHRINE_GOLD_RESERVE;

    let mut short = g.clone();
    short.players[0].gold = price + reserve - 1.0;
    assert!(ai.culture_shrine_purchase(&short, 0, &plan).is_none());
    short.players[0].gold = price + reserve;
    assert!(ai.culture_shrine_purchase(&short, 0, &plan).is_some());

    let mut draining = g.clone();
    draining.players[0].gold_per_turn = -1.0;
    assert!(ai.culture_shrine_purchase(&draining, 0, &plan).is_none());

    let mut off = ai.clone();
    off.disable_culture_gold_buys_the_shrines();
    let mut board = g.clone();
    assert!(off.culture_shrine_purchase(&board, 0, &plan).is_none());
    assert!(!off.buy_culture_shrine(&mut board, 0, &plan));
    assert_eq!(board.players[0].gold, 1_000.0);

    let mut science = AdvancedAi::targeting(VictoryTarget::Science);
    science.enable_culture_seat_theater_first();
    science.enable_culture_gold_buys_the_shrines();
    assert!(science.culture_shrine_purchase(&g, 0, &plan).is_none());

    let mut threatened = plan.clone();
    threatened.threatened_city = Some(cid);
    assert!(ai.culture_shrine_purchase(&g, 0, &threatened).is_none());
}
