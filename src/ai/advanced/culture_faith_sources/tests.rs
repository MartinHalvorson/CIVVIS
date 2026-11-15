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
fn the_parallel_claim_starts_a_source_in_every_theater_city_at_once() {
    let (mut g, mut ai, plan, cities) = fixture(4);
    ai.enable_culture_faith_site_per_theater();
    let first = claim(&mut g, &ai, &plan).expect("the first source");
    assert!(
        claim(&mut g, &ai, &plan).is_none(),
        "version one without a host quote keeps one source in the empire"
    );
    ai.disable_culture_faith_site_per_theater();
    ai.enable_culture_faith_site_per_theater_3();
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
fn version_three_carries_version_one() {
    let (mut g, mut ai, plan, _) = fixture(3);
    g.players[0].techs.remove(&crate::name!("astrology"));
    ai.enable_culture_faith_site_per_theater_3();
    assert!(ai.culture_faith_astrology_due(&g, 0));
    assert_eq!(ai.culture_faith_source_budget(&g, 0), Some(3));
    g.players[0].techs.insert(crate::name!("astrology"));
    assert!(ai.culture_faith_source_target(&g, 0, &plan).is_some());
}

#[test]
fn the_parallel_claim_goes_ahead_of_a_wonder_but_not_a_spy() {
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
    ai.disable_culture_faith_site_per_theater();
    ai.enable_culture_faith_site_per_theater_3();
    let (city, item, _) = ai
        .culture_faith_source_target(&g, 0, &plan)
        .expect("the parallel claim takes a source over the wonder");
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
fn version_three_keeps_a_settler() {
    let (mut g, mut ai, plan, cities) = fixture(3);
    ai.enable_culture_faith_site_per_theater_3();
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

#[test]
fn gold_fund_keeps_the_wanted_shrine_price_from_the_general_pass() {
    let (mut g, ai, plan, cid) = shrine_board();
    let price = g.building_gold_purchase_cost(0, cid, "shrine").unwrap();
    let floor = ai.purchase_upkeep_floor(&g, 0, 0.0) + CULTURE_SHRINE_GOLD_RESERVE;
    assert_eq!(ai.culture_shrine_gold_fund(&g, 0, &plan), price + floor);

    let mut off = ai.clone();
    off.disable_culture_gold_buys_the_shrines();
    assert_eq!(off.culture_shrine_gold_fund(&g, 0, &plan), 0.0);

    let mut far = g.clone();
    far.players[0].gold = 0.0;
    far.players[0].gold_per_turn = 1.0;
    assert_eq!(
        ai.culture_shrine_gold_fund(&far, 0, &plan),
        0.0,
        "a price beyond six turns of income does not freeze the treasury"
    );

    for building in ["shrine", "temple"] {
        g.cities
            .get_mut(&cid)
            .unwrap()
            .buildings
            .push(Name::new(building));
    }
    assert_eq!(
        ai.culture_shrine_gold_fund(&g, 0, &plan),
        0.0,
        "nothing left to save for"
    );
}

#[test]
fn braves_the_barbarians_claims_beside_a_barbarian() {
    let (mut g, mut ai, plan, cities) = fixture(3);
    ai.enable_culture_faith_site_per_theater();
    let barbarian = g
        .players
        .iter()
        .position(|player| player.is_barbarian)
        .expect("the fixture has a Barbarian seat");
    for &cid in &cities {
        let near = g
            .wdisk(g.cities[&cid].pos, 2)
            .into_iter()
            .find(|pos| {
                *pos != g.cities[&cid].pos
                    && g.map
                        .get(*pos)
                        .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
                    && !g.units.values().any(|unit| unit.pos == *pos)
            })
            .expect("room beside the city");
        g.spawn_test_unit("warrior", barbarian, near);
    }
    assert!(
        ai.culture_faith_source_target(&g, 0, &plan).is_none(),
        "off: a Barbarian within six tiles holds every Theater city"
    );
    ai.enable_culture_faith_braves_the_barbarians();
    assert!(
        ai.culture_faith_source_target(&g, 0, &plan).is_some(),
        "on: the Barbarians no longer hold the claim"
    );
}

/// The specialty district families `cid`'s menu offers.
fn menu_families(g: &Game, cid: u32) -> Vec<Name> {
    g.producible_items(0, cid)
        .into_iter()
        .filter_map(|item| match item {
            Item::District { district, .. } if g.rules.districts[district].specialty => {
                Some(g.district_family(district))
            }
            _ => None,
        })
        .collect()
}

/// The cities with a Holy Site plot and a Campus on the menu; the small
/// fixture territories leave some without.
fn ready_cities(g: &Game, cities: &[u32]) -> Vec<u32> {
    cities
        .iter()
        .copied()
        .filter(|cid| {
            BasicAi::first_district_item(g, 0, *cid, "holy_site").is_some()
                && menu_families(g, *cid).contains(&crate::name!("campus"))
        })
        .collect()
}

#[test]
fn version_three_keeps_the_last_slot_for_the_holy_site() {
    let (mut g, mut ai, _, cities) = fixture(3);
    let ready = ready_cities(&g, &cities);
    assert!(ready.len() >= 2, "the fixture has two ready cities");
    let (kept, open) = (ready[0], ready[1]);
    // Population four: two slots, the Theater Square in one.
    g.cities.get_mut(&kept).unwrap().pop = 4;
    assert!(
        menu_families(&g, kept).contains(&crate::name!("campus")),
        "the fixture offers a Campus beside the Theater Square"
    );
    ai.enable_culture_faith_site_per_theater();
    assert!(
        ai.culture_faith_hold_the_slots(&mut g, 0).is_none(),
        "version one holds nothing"
    );
    ai.disable_culture_faith_site_per_theater();
    ai.enable_culture_faith_site_per_theater_3();
    let before = Arc::clone(&g.blocked_districts);
    let held = ai.culture_faith_hold_the_slots(&mut g, 0);
    assert!(held.is_some(), "the one-slot Theater city keeps it");
    let menu = menu_families(&g, kept);
    assert!(
        menu.contains(&crate::name!("holy_site")),
        "the Holy Site stays on the menu: {menu:?}"
    );
    assert!(
        menu.iter().all(|family| *family == "holy_site"),
        "every other specialty district stands down: {menu:?}"
    );
    assert!(
        menu_families(&g, open).contains(&crate::name!("campus")),
        "a city with two free slots keeps its Campus"
    );
    AdvancedAi::culture_faith_release_the_slots(&mut g, held);
    assert!(Arc::ptr_eq(&before, &g.blocked_districts));
    assert!(menu_families(&g, kept).contains(&crate::name!("campus")));
}

#[test]
fn version_three_releases_the_slot_once_the_holy_site_is_laid() {
    let (mut g, mut ai, _, cities) = fixture(3);
    let kept = ready_cities(&g, &cities)[0];
    g.cities.get_mut(&kept).unwrap().pop = 4;
    ai.enable_culture_faith_site_per_theater_3();
    let site = BasicAi::first_district_item(&g, 0, kept, "holy_site").expect("a Holy Site plot");
    g.apply(
        0,
        &Action::Produce {
            city: kept,
            item: site,
        },
    )
    .unwrap();
    assert!(
        !ai.culture_faith_keeps_the_slot(&g, 0, kept),
        "the Holy Site's foundation fills the slot"
    );
}

#[test]
fn version_three_keeps_the_slot_before_astrology() {
    let (mut g, mut ai, _, cities) = fixture(3);
    let kept = ready_cities(&g, &cities)[0];
    g.cities.get_mut(&kept).unwrap().pop = 4;
    g.players[0].techs.remove(&crate::name!("astrology"));
    ai.enable_culture_faith_site_per_theater_3();
    assert!(ai.culture_faith_keeps_the_slot(&g, 0, kept));
    let held = ai.culture_faith_hold_the_slots(&mut g, 0);
    assert!(
        !menu_families(&g, kept).contains(&crate::name!("campus")),
        "the Campus waits for the Holy Site"
    );
    AdvancedAi::culture_faith_release_the_slots(&mut g, held);
}

#[test]
fn version_three_lifts_the_site_cap_and_the_faith_ceiling() {
    let (mut g, mut ai, _, _) = fixture(7);
    ai.enable_culture_faith_site_per_theater();
    assert_eq!(ai.culture_faith_source_budget(&g, 0), Some(6));
    ai.disable_culture_faith_site_per_theater();
    ai.enable_culture_faith_site_per_theater_3();
    assert_eq!(
        ai.culture_faith_source_budget(&g, 0),
        Some(7),
        "a site for every Theater city"
    );
    Arc::make_mut(&mut g.observed_yield_adjustments).insert(
        0,
        Yields {
            faith: 170.0,
            ..Default::default()
        },
    );
    assert!(ai.culture_faith_source_budget(&g, 0).is_some());
    ai.disable_culture_faith_site_per_theater_3();
    ai.enable_culture_faith_site_per_theater();
    assert!(
        ai.culture_faith_source_budget(&g, 0).is_none(),
        "version one stops at 140 for seven cities"
    );
    ai.disable_culture_faith_site_per_theater();
    ai.enable_culture_faith_site_per_theater_3();
    Arc::make_mut(&mut g.observed_yield_adjustments).insert(
        0,
        Yields {
            faith: 280.0,
            ..Default::default()
        },
    );
    assert!(
        ai.culture_faith_source_budget(&g, 0).is_none(),
        "forty a city"
    );
}

#[test]
fn version_three_takes_astrology_first_and_carries_version_two() {
    let (mut g, mut ai, plan, _) = fixture(4);
    g.players[0].techs.remove(&crate::name!("astrology"));
    ai.enable_culture_faith_site_per_theater();
    assert!(ai.culture_faith_astrology_due(&g, 0));
    assert!(!ai.culture_faith_astrology_first(&g, 0), "version one only");
    ai.disable_culture_faith_site_per_theater();
    ai.enable_culture_faith_site_per_theater_3();
    assert!(ai.culture_faith_astrology_first(&g, 0));
    g.players[0].techs.insert(crate::name!("astrology"));
    assert!(!ai.culture_faith_astrology_first(&g, 0));
    let mut claimed = Vec::new();
    while let Some(city) = claim(&mut g, &ai, &plan) {
        assert!(!claimed.contains(&city), "one source per city at a time");
        claimed.push(city);
    }
    assert!(claimed.len() >= 2, "the parallel claim: {claimed:?}");
}

#[test]
fn version_three_keeps_the_slot_through_an_attack() {
    let (mut g, mut ai, plan, cities) = fixture(3);
    let kept = ready_cities(&g, &cities)[0];
    let turn = g.turn;
    let city = g.cities.get_mut(&kept).unwrap();
    city.pop = 4;
    city.last_attacked = turn;
    ai.enable_culture_faith_site_per_theater_3();
    assert!(ai.culture_faith_keeps_the_slot(&g, 0, kept));
    assert!(
        ai.culture_faith_source_target(&g, 0, &plan)
            .is_none_or(|(city, _, _)| city != kept),
        "the claim itself still waits out the attack"
    );
}

#[test]
fn version_three_claims_every_theater_city_in_one_pass() {
    let (mut g, mut ai, plan, _) = fixture(4);
    ai.enable_culture_faith_site_per_theater_3();
    let mut expected = Vec::new();
    let mut board = g.clone();
    while let Some(city) = claim(&mut board, &ai, &plan) {
        expected.push(city);
    }
    assert!(expected.len() >= 2, "the fixture has two sources to claim");
    ai.claim_culture_faith_source(&mut g, 0, &plan);
    for city in expected {
        assert!(
            g.cities[&city]
                .queue
                .first()
                .is_some_and(|item| AdvancedAi::culture_faith_item(&g, item)),
            "city {city} claimed its source in the same pass"
        );
    }
}

/// A fixture city that keeps its last slot for the Holy Site (population
/// four: two slots, the Theater Square in one), with `head` alone in its
/// queue.
fn kept_slot_board(head: Option<Item>) -> (Game, AdvancedAi, StrategicPlan, u32) {
    let (mut g, mut ai, plan, cities) = fixture(3);
    let kept = ready_cities(&g, &cities)[0];
    g.cities.get_mut(&kept).unwrap().pop = 4;
    for &cid in &cities {
        g.cities.get_mut(&cid).unwrap().queue = head.iter().cloned().collect();
    }
    ai.enable_culture_faith_site_per_theater_3();
    assert!(ai.culture_faith_slot_is_kept(&g, 0, kept));
    (g, ai, plan, kept)
}

fn holy_site_at_the_head(g: &Game, cid: u32) -> bool {
    matches!(g.cities[&cid].queue.first(),
        Some(Item::District { district, .. }) if g.district_family(*district) == "holy_site")
}

/// v3 claims, then v4 claims on a copy of the same board: whether the kept
/// city's Holy Site went to the head under each.
fn claim_under_three_and_four(
    g: &Game,
    ai: &AdvancedAi,
    plan: &StrategicPlan,
    kept: u32,
) -> (bool, bool) {
    let mut three = g.clone();
    ai.claim_culture_faith_source(&mut three, 0, plan);
    let mut four_ai = ai.clone();
    four_ai.disable_culture_faith_site_per_theater_3();
    four_ai.enable_culture_faith_site_per_theater_4();
    let mut four = g.clone();
    four_ai.claim_culture_faith_source(&mut four, 0, plan);
    (
        holy_site_at_the_head(&three, kept),
        holy_site_at_the_head(&four, kept),
    )
}

#[test]
fn version_four_puts_the_kept_slots_holy_site_ahead_of_an_amphitheater() {
    let amphitheater = Item::Building {
        building: crate::name!("amphitheater"),
    };
    let (mut g, mut ai, plan, kept) = kept_slot_board(Some(amphitheater.clone()));
    // No Amphitheater stands yet, so the first-building claim holds this one;
    // two Theater Squares open the sources without it.
    for cid in g.player_city_ids(0) {
        g.cities
            .get_mut(&cid)
            .unwrap()
            .buildings
            .retain(|building| *building != "amphitheater");
    }
    ai.enable_culture_faith_opens_at_the_theaters();
    assert!(ai.culture_faith_slot_is_kept(&g, 0, kept));
    assert!(ai.culture_theater_holds(&g, 0, kept, &plan, &amphitheater));
    let (three, four) = claim_under_three_and_four(&g, &ai, &plan, kept);
    assert!(!three, "version three keeps the Amphitheater at the head");
    assert!(four, "version four puts the Holy Site ahead of it");
}

#[test]
fn version_four_passes_a_backlog_builder_unless_it_is_the_only_cover() {
    let builder = Item::Unit {
        unit: crate::name!("builder"),
    };
    let (mut g, mut ai, plan, kept) = kept_slot_board(Some(builder.clone()));
    ai.enable_backlog_builder_takes_the_queue();
    // Only the kept city trains a Builder, and every worked plot is a bare
    // grassland hill a Mine improves (the backlog fixture's ground).
    for cid in g.player_city_ids(0) {
        if cid != kept {
            g.cities.get_mut(&cid).unwrap().queue.clear();
        }
        let center = g.cities[&cid].pos;
        for position in g.cities[&cid].owned_tiles.to_vec() {
            let tile = g.map.tiles.get_mut(&position).unwrap();
            if position == center || tile.district.is_some() {
                continue;
            }
            tile.terrain = crate::name!("grassland");
            tile.hills = true;
            tile.feature = None;
            tile.resource = None;
            tile.improvement = None;
        }
    }
    // Slow enough that the Builder takes more than a turn (one finishing
    // within the turn keeps its queue under every version).
    let builder_cost = g.item_cost_for_city(0, kept, &builder);
    let base = g.city_yields(kept).production - 20.0;
    Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
        kept,
        Yields {
            production: builder_cost * 0.6 - base,
            ..Default::default()
        },
    );
    assert!(builder_cost > g.city_yields(kept).production);
    assert!(ai.culture_faith_slot_is_kept(&g, 0, kept));
    assert!(
        ai.backlog_builder_holds(&g, 0, kept, &plan, &builder),
        "the worked backlog wants the Builder"
    );
    let (three, four) = claim_under_three_and_four(&g, &ai, &plan, kept);
    assert!(!three, "version three keeps the backlog Builder");
    assert!(
        !four,
        "the empire's only Builder cover keeps its queue under version four"
    );
    let other = g
        .player_city_ids(0)
        .into_iter()
        .find(|cid| *cid != kept)
        .unwrap();
    let pos = g.cities[&other].pos;
    g.spawn_unit("builder", 0, pos);
    assert!(
        ai.backlog_builder_holds(&g, 0, kept, &plan, &builder),
        "one Builder afield still leaves the backlog wanting this one"
    );
    let (three, four) = claim_under_three_and_four(&g, &ai, &plan, kept);
    assert!(!three, "version three still keeps the backlog Builder");
    assert!(four, "with a Builder afield, the Holy Site goes first");
}

#[test]
fn version_four_passes_a_settler_past_the_floor_but_not_one_under_it() {
    let settler = Item::Unit {
        unit: crate::name!("settler"),
    };
    let (mut g, ai, mut plan, kept) = kept_slot_board(Some(settler));
    let (three, four) = claim_under_three_and_four(&g, &ai, &plan, kept);
    assert!(!three, "version three keeps every Settler");
    assert!(four, "no floor wants this Settler: the Holy Site goes first");
    let mut floor = ai.clone();
    floor.enable_early_settler_floor();
    g.turn = 30;
    plan.desired_cities = 6;
    assert!(floor.early_settler_floor_holds(&g, 0, kept, &plan));
    assert!(floor.culture_faith_slot_is_kept(&g, 0, kept));
    let (three, four) = claim_under_three_and_four(&g, &floor, &plan, kept);
    assert!(!three && !four, "a Settler under the early floor keeps its queue");
}

#[test]
fn version_four_yields_to_walls_and_a_project() {
    let walls = Item::Building {
        building: crate::name!("walls"),
    };
    let (g, ai, plan, kept) = kept_slot_board(Some(walls));
    let mut four = ai.clone();
    four.disable_culture_faith_site_per_theater_3();
    four.enable_culture_faith_site_per_theater_4();
    let head = g.cities[&kept].queue.first().cloned().unwrap();
    assert!(!four.culture_faith_kept_slot_passes_the_head(&g, 0, kept, &plan, &head, false));
    let (three, four_claimed) = claim_under_three_and_four(&g, &ai, &plan, kept);
    assert!(!three && !four_claimed, "Walls keep the queue");
    let spy = Item::Unit {
        unit: crate::name!("spy"),
    };
    assert!(!four.culture_faith_kept_slot_passes_the_head(&g, 0, kept, &plan, &spy, false));
    let granary = Item::Building {
        building: crate::name!("granary"),
    };
    assert!(four.culture_faith_kept_slot_passes_the_head(&g, 0, kept, &plan, &granary, false));
    assert!(
        !ai.culture_faith_kept_slot_passes_the_head(&g, 0, kept, &plan, &granary, false),
        "version three never passes a routine head"
    );
}

#[test]
fn version_four_releases_the_kept_slot_after_its_patience() {
    let (mut g, mut ai, _, kept) = kept_slot_board(None);
    let patience = g.standard_duration(CULTURE_FAITH_KEPT_SLOT_PATIENCE);
    assert_eq!(patience, 8, "eight Online turns");
    // Version three keeps the slot however long it waits.
    for _ in 0..=patience + 2 {
        g.turn += 1;
        let held = ai.culture_faith_hold_the_slots(&mut g, 0);
        assert!(!menu_families(&g, kept).contains(&crate::name!("campus")));
        AdvancedAi::culture_faith_release_the_slots(&mut g, held);
    }
    ai.disable_culture_faith_site_per_theater_3();
    ai.enable_culture_faith_site_per_theater_4();
    for turn in 1..=patience {
        g.turn += 1;
        let held = ai.culture_faith_hold_the_slots(&mut g, 0);
        // A second frame of the same turn counts nothing more.
        AdvancedAi::culture_faith_release_the_slots(&mut g, held);
        let held = ai.culture_faith_hold_the_slots(&mut g, 0);
        assert!(
            !menu_families(&g, kept).contains(&crate::name!("campus")),
            "turn {turn} of {patience}: the slot is still kept"
        );
        assert!(ai.culture_faith_slot_is_kept(&g, 0, kept));
        AdvancedAi::culture_faith_release_the_slots(&mut g, held);
    }
    g.turn += 1;
    let held = ai.culture_faith_hold_the_slots(&mut g, 0);
    assert!(
        menu_families(&g, kept).contains(&crate::name!("campus")),
        "past its patience the slot is released"
    );
    assert!(!ai.culture_faith_slot_is_kept(&g, 0, kept));
    AdvancedAi::culture_faith_release_the_slots(&mut g, held);
    // The condition lapses for a turn and re-arms: the count starts again.
    g.turn += 2;
    let held = ai.culture_faith_hold_the_slots(&mut g, 0);
    assert!(
        !menu_families(&g, kept).contains(&crate::name!("campus")),
        "kept afresh, the slot waits for the Holy Site again"
    );
    AdvancedAi::culture_faith_release_the_slots(&mut g, held);
}
