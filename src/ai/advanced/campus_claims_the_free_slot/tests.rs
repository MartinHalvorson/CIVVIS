use super::*;
use crate::game::install_test_district;
use crate::rules::Yields;
use crate::setup::GameSpeed;
use std::sync::Arc;

fn culture_with_two_campus_foundations(
    queued: bool,
) -> (Game, AdvancedAi, u32, u32, StrategicPlan, Item) {
    let (mut game, mut ai, cid, plan, campus) = one_citizen_native_korean_city();
    game.cities.get_mut(&cid).unwrap().pop = 4;
    let other = game.found_city_for(0, (25, 5), None);
    game.cities.get_mut(&other).unwrap().pop = 4;
    let quotes = game.host_buildable[&cid].clone();
    Arc::make_mut(&mut game.host_buildable).insert(other, quotes);
    if queued {
        let item = BasicAi::first_campus_item(&game, 0, other).unwrap();
        game.cities.get_mut(&other).unwrap().queue = vec![item];
    } else {
        install_test_district(&mut game, other, "seowon");
    }
    ai.victory_target = Some(VictoryTarget::Culture);
    (game, ai, cid, other, plan, campus)
}

#[test]
fn culture_stops_forcing_a_third_campus_after_two_replacement_foundations() {
    for queued in [false, true] {
        let (mut game, ai, cid, _, plan, campus) = culture_with_two_campus_foundations(queued);
        assert!(game.can_produce(0, cid, &campus));
        assert!(ai.free_slot_campus_item(&game, 0, cid, &plan).is_none());
        game.cities.get_mut(&cid).unwrap().queue = vec![campus.clone()];
        assert!(
            ai.free_slot_campus_holds(&game, 0, cid, &plan, &campus),
            "the budget must not abandon an already committed ordinary Campus"
        );
    }
}

#[test]
fn culture_keeps_the_second_queued_campus_and_replaces_a_lost_foundation() {
    let (mut game, ai, cid, other, plan, _) = culture_with_two_campus_foundations(true);
    let queued = game.cities[&other].queue[0].clone();
    assert!(ai.free_slot_campus_holds(&game, 0, other, &plan, &queued));
    game.cities.get_mut(&other).unwrap().queue.clear();
    assert!(ai.free_slot_campus_item(&game, 0, cid, &plan).is_some());
}

#[test]
fn the_culture_campus_budget_leaves_wars_and_other_targets_unchanged() {
    let (game, ai, cid, _, plan, _) = culture_with_two_campus_foundations(false);
    for target in [
        VictoryTarget::Science,
        VictoryTarget::Religion,
        VictoryTarget::Domination,
    ] {
        let mut other = ai.clone();
        other.victory_target = Some(target);
        assert!(other.free_slot_campus_item(&game, 0, cid, &plan).is_some());
    }
    let mut disabled = game.clone();
    disabled.victory_conditions.culture = false;
    assert!(ai.free_slot_campus_item(&disabled, 0, cid, &plan).is_some());
    let mut war = game;
    war.players[1].alive = true;
    war.at_war.insert((0, 1));
    assert!(ai.free_slot_campus_item(&war, 0, cid, &plan).is_some());
}

#[test]
fn culture_keeps_core_library_claims_and_restores_delegated_openings() {
    let (mut game, mut ai, cid, other, plan, _) = culture_with_two_campus_foundations(false);
    let capital = game
        .player_city_ids(0)
        .into_iter()
        .find(|city| *city != cid && *city != other)
        .unwrap();
    game.cities
        .get_mut(&capital)
        .unwrap()
        .buildings
        .push(crate::name!("granary"));
    Arc::make_mut(&mut game.host_buildable)
        .entry(capital)
        .or_default()
        .insert(
            Game::production_block_key(&Item::Building {
                building: crate::name!("library"),
            }),
            crate::game::HostMenuEntry {
                cost: Some(45.0),
                turns: Some(5.0),
            },
        );
    ai.enable_campus_building_claims_the_queue();
    assert!(
        matches!(ai.campus_building_claim_item(&game, 0, capital, &plan), Some((Item::Building { building }, _, _)) if building == "library")
    );
    ai.base.capital_campus_first = true;
    ai.base.campus_before_harbor = true;
    ai.base.campus_before_the_army = true;
    ai.base.campus_before_the_army_2 = true;
    ai.base.campus_before_the_army_3 = true;
    ai.culture_budgeted_base_cities(&mut game, 0);
    assert!(
        ai.base.capital_campus_first
            && ai.base.campus_before_harbor
            && ai.base.campus_before_the_army
            && ai.base.campus_before_the_army_2
            && ai.base.campus_before_the_army_3
    );
}

fn one_citizen_native_korean_city() -> (Game, AdvancedAi, u32, StrategicPlan, Item) {
    let mut game = Game::new_full(2, 32, 20, 95_301, 250, 0, false);
    game.units.clear();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.hills = true;
        tile.feature = None;
        tile.resource = None;
    }
    game.players[0].civ = "Korea".to_string();
    for tech in ["pottery", "writing", "mining", "bronze_working"] {
        game.players[0].techs.insert(Name::new(tech));
    }
    game.game_speed = GameSpeed::Online;
    game.turn = 43;
    let capital = game.found_city_for(0, (5, 5), None);
    let cid = game.found_city_for(0, (15, 5), None);
    game.cities.get_mut(&capital).unwrap().pop = 4;
    install_test_district(&mut game, capital, "seowon");
    game.cities.get_mut(&cid).unwrap().pop = 1;
    game.cities.get_mut(&cid).unwrap().queue.clear();
    game.cities.get_mut(&cid).unwrap().production = 0.0;
    let production = game.city_yields(cid).production;
    Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
        cid,
        Yields {
            production: 5.0 - production,
            ..Default::default()
        },
    );
    let quote_item = Item::District {
        district: crate::name!("seowon"),
        pos: game.cities[&cid].pos,
    };
    for item in [
        quote_item,
        Item::District {
            district: crate::name!("campus"),
            pos: game.cities[&cid].pos,
        },
    ] {
        Arc::make_mut(&mut game.host_buildable)
            .entry(cid)
            .or_default()
            .insert(
                Game::production_block_key(&item),
                crate::game::HostMenuEntry {
                    cost: Some(26.0),
                    turns: Some(8.0),
                },
            );
    }
    let campus = BasicAi::first_campus_item(&game, 0, cid).expect("native first Seowon is legal");
    let mut ai = AdvancedAi::new();
    ai.enable_campus_claims_the_free_slot_2();
    ai.victory_target = Some(VictoryTarget::Science);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Expansion,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 6,
        assessed_turn: game.turn,
        rush: false,
    };
    (game, ai, cid, plan, campus)
}

#[test]
fn a_one_citizen_korean_science_city_claims_its_native_first_seowon() {
    let (mut game, ai, cid, plan, campus) = one_citizen_native_korean_city();
    assert!(game.can_produce(0, cid, &campus));
    assert!(ai.free_slot_campus_item(&game, 0, cid, &plan).is_some());
    let mut control = game.clone();
    let mut version_one = ai.clone();
    version_one.disable_campus_claims_the_free_slot_2();
    version_one.enable_campus_claims_the_free_slot();
    version_one.claim_free_slot_campus(&mut control, 0, &plan);
    assert_eq!(head(&control, cid), None);
    ai.claim_free_slot_campus(&mut game, 0, &plan);
    assert_eq!(head(&game, cid), Some(campus));
}

#[test]
fn the_one_citizen_seowon_keeps_native_budget_and_existing_picker_guards() {
    let (base, armed, cid, plan, campus) = one_citizen_native_korean_city();
    for case in [
        "version one",
        "other civilization",
        "other victory",
        "science disabled",
        "no native quote",
        "over budget",
        "threat",
        "recent attack",
        "already standing",
        "already queued",
    ] {
        let mut game = base.clone();
        let mut ai = armed.clone();
        let mut plan = plan.clone();
        match case {
            "version one" => {
                ai.disable_campus_claims_the_free_slot_2();
                ai.enable_campus_claims_the_free_slot();
            }
            "other civilization" => game.players[0].civ = "Rome".to_string(),
            "other victory" => ai.victory_target = Some(VictoryTarget::Culture),
            "science disabled" => game.victory_conditions.science = false,
            "no native quote" => {
                game.host_buildable = Arc::default();
                assert!(
                    BasicAi::first_campus_item(&game, 0, cid).is_some(),
                    "the model alone would offer a Campus"
                );
            }
            "over budget" => {
                Arc::make_mut(&mut game.host_buildable)
                    .get_mut(&cid)
                    .unwrap()
                    .get_mut(&Game::production_block_key(&campus))
                    .unwrap()
                    .turns = Some(16.0)
            }
            "threat" => plan.threatened_city = Some(cid),
            "recent attack" => game.cities.get_mut(&cid).unwrap().last_attacked = game.turn - 1,
            "already standing" => {
                install_test_district(&mut game, cid, "seowon");
            }
            "already queued" => game.cities.get_mut(&cid).unwrap().queue = vec![campus.clone()],
            _ => unreachable!(),
        }
        assert!(
            ai.free_slot_campus_item(&game, 0, cid, &plan).is_none(),
            "{case}"
        );
    }
}

#[test]
fn culture_releases_first_campus_claims_after_its_tourism_technologies() {
    let (mut game, ranked) = campus_less_empire(95_120, 1);
    all_queue(&mut game, &ranked, &unit("knight"));
    for tech in ["flight", "radio", "computers"] {
        game.players[0].techs.insert(Name::new(tech));
    }
    let campus = BasicAi::first_campus_item(&game, 0, ranked[0]).unwrap();
    assert!(game.can_produce(0, ranked[0], &campus));
    for version_two in [false, true] {
        let mut ai = armed();
        ai.victory_target = Some(VictoryTarget::Culture);
        if version_two {
            ai.enable_campus_claims_the_free_slot_2();
        }
        let mut other = game.clone();
        let plan = ai.assess(&other, 0);
        assert!(ai
            .free_slot_campus_item(&other, 0, ranked[0], &plan)
            .is_none());
        ai.claim_free_slot_campus(&mut other, 0, &plan);
        assert!(ranked
            .iter()
            .all(|cid| head(&other, *cid) == Some(unit("knight"))));
        other.cities.get_mut(&ranked[0]).unwrap().queue = vec![campus.clone()];
        assert!(!ai.free_slot_campus_holds(&other, 0, ranked[0], &plan, &campus));
        assert!(other.can_produce(0, ranked[0], &campus));
    }
}

#[test]
fn earlier_culture_and_other_targets_keep_their_campus_claims() {
    let (mut game, ranked) = campus_less_empire(95_121, 1);
    all_queue(&mut game, &ranked, &unit("knight"));
    for tech in ["flight", "radio", "computers"] {
        game.players[0].techs.insert(Name::new(tech));
    }
    for missing in ["flight", "radio", "computers"] {
        let mut other = game.clone();
        other.players[0].techs.remove(&Name::new(missing));
        let mut ai = armed();
        ai.victory_target = Some(VictoryTarget::Culture);
        let plan = ai.assess(&other, 0);
        ai.claim_free_slot_campus(&mut other, 0, &plan);
        assert!(
            is_campus(&other, &head(&other, ranked[0])),
            "missing {missing}"
        );
    }
    for target in VictoryTarget::ALL {
        if target == VictoryTarget::Culture {
            continue;
        }
        let mut other = game.clone();
        let mut ai = armed();
        ai.victory_target = Some(target);
        let plan = ai.assess(&other, 0);
        ai.claim_free_slot_campus(&mut other, 0, &plan);
        assert!(is_campus(&other, &head(&other, ranked[0])), "{target:?}");
    }
    game.victory_conditions.culture = false;
    let mut ai = armed();
    ai.victory_target = Some(VictoryTarget::Culture);
    let plan = ai.assess(&game, 0);
    ai.claim_free_slot_campus(&mut game, 0, &plan);
    assert!(is_campus(&game, &head(&game, ranked[0])));
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_campus_claims_the_free_slot();
    ai
}

fn head(game: &Game, cid: u32) -> Option<Item> {
    game.cities[&cid].queue.first().cloned()
}

fn unit(name: &str) -> Item {
    Item::Unit {
        unit: Name::new(name),
    }
}

fn building(name: &str) -> Item {
    Item::Building {
        building: Name::new(name),
    }
}

fn is_campus(game: &Game, item: &Option<Item>) -> bool {
    matches!(item, Some(Item::District { district, .. })
        if game.district_family(*district) == "campus")
}

/// Six Emperor cities of four citizens with Writing in hand, production 40
/// down to 10, and no district but the centre: every city has a free slot
/// and can build its first Campus. Ranked quickest first.
fn campus_less_empire(seed: u64, players: usize) -> (Game, Vec<u32>) {
    let mut game = Game::new_full(
        players,
        44,
        26,
        crate::rng::fixture_seed("FREESLOTCAMPUS", seed),
        250,
        0,
        false,
    );
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    while game.player_city_ids(0).len() < 6 {
        let pos = game
            .map
            .tiles
            .values()
            .filter(|tile| {
                game.rules.is_passable(tile)
                    && !game.rules.is_water(tile)
                    && tile.owner_city.is_none()
                    && tile.district.is_none()
                    && tile.wonder.is_none()
                    && game
                        .cities
                        .values()
                        .all(|city| game.wdist(city.pos, tile.pos) >= 5)
                    && game
                        .cities
                        .values()
                        .any(|city| game.wdist(city.pos, tile.pos) <= 8)
            })
            .map(|tile| tile.pos)
            .min()
            .expect("room for six cities");
        game.found_city_for(0, pos, None);
    }
    for player in 0..players {
        for unit in game.player_unit_ids(player) {
            game.remove_unit(unit);
        }
    }
    game.game_speed = GameSpeed::Online;
    game.turn = 50;
    game.difficulty = "emperor".to_string();
    game.players[0].gold = 200.0;
    game.players[0].gold_per_turn = 5.0;
    for tech in ["pottery", "writing", "mining", "bronze_working", "archery"] {
        game.players[0].techs.insert(Name::new(tech));
    }
    let cities = game.player_city_ids(0);
    for (rank, cid) in cities.iter().enumerate() {
        game.cities.get_mut(cid).unwrap().pop = 4;
        Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
            *cid,
            Yields {
                production: 40.0 - 6.0 * rank as f64,
                ..Default::default()
            },
        );
        Arc::make_mut(&mut game.observed_city_amenity_adjustments).insert(*cid, 3);
    }
    for cid in &cities {
        let housing = game.city_housing(&game.cities[cid]);
        Arc::make_mut(&mut game.observed_city_housing_adjustments).insert(*cid, 8.0 - housing);
    }
    let mut ranked = cities;
    ranked.sort_by(|a, b| {
        game.city_yields(*b)
            .production
            .total_cmp(&game.city_yields(*a).production)
            .then(a.cmp(b))
    });
    for cid in &ranked {
        assert!(
            BasicAi::first_campus_item(&game, 0, *cid).is_some(),
            "fixture: the first Campus is buildable"
        );
    }
    (game, ranked)
}

fn all_queue(game: &mut Game, cities: &[u32], item: &Item) {
    for cid in cities {
        game.cities.get_mut(cid).unwrap().queue = vec![item.clone()];
    }
}

/// Every city training a routine Knight at peace: the quickest city puts its
/// first Campus at the head, one claim a turn, and the claim holds against
/// the rescoring; off, nothing moves and nothing holds.
#[test]
fn the_quickest_city_claims_its_free_slot_over_a_routine_unit() {
    let (mut game, ranked) = campus_less_empire(95_101, 1);
    all_queue(&mut game, &ranked, &unit("knight"));

    let stock = AdvancedAi::new();
    let mut off = game.clone();
    let plan = stock.assess(&off, 0);
    stock.claim_free_slot_campus(&mut off, 0, &plan);
    assert!(
        ranked
            .iter()
            .all(|cid| head(&off, *cid) == Some(unit("knight"))),
        "off: no claim"
    );

    let ai = armed();
    let mut on = game.clone();
    let plan = ai.assess(&on, 0);
    ai.claim_free_slot_campus(&mut on, 0, &plan);
    let claimed: Vec<u32> = ranked
        .iter()
        .copied()
        .filter(|cid| is_campus(&on, &head(&on, *cid)))
        .collect();
    assert_eq!(
        claimed,
        vec![ranked[0]],
        "the quickest city, one claim a turn"
    );
    let campus = head(&on, ranked[0]).unwrap();
    assert!(ai.free_slot_campus_holds(&on, 0, ranked[0], &plan, &campus));
    assert!(!stock.free_slot_campus_holds(&on, 0, ranked[0], &plan, &campus));

    // The next turn's claim goes to the next quickest city.
    ai.claim_free_slot_campus(&mut on, 0, &plan);
    assert!(is_campus(&on, &head(&on, ranked[1])));
}

/// An idle city with no Settler due claims its Campus too.
#[test]
fn an_idle_city_claims_its_free_slot() {
    let (mut game, ranked) = campus_less_empire(95_102, 1);
    all_queue(&mut game, &ranked, &unit("knight"));
    game.cities.get_mut(&ranked[2]).unwrap().queue.clear();
    // Every other city already holds a Campus.
    for cid in &ranked {
        if *cid != ranked[2] {
            install_test_district(&mut game, *cid, "campus");
        }
    }
    let ai = armed();
    let plan = ai.assess(&game, 0);
    let n = game.player_city_ids(0).len();
    let settlers = ai.counts(&game, 0).settlers;
    if ai.base.settler_due(&game, 0, ranked[2], n, settlers) {
        return; // the Settler step owns an idle queue with a Settler due
    }
    ai.claim_free_slot_campus(&mut game, 0, &plan);
    assert!(is_campus(&game, &head(&game, ranked[2])));
}

/// A Settler and Walls at the head keep their queue.
#[test]
fn a_settler_and_walls_keep_their_queue() {
    for item in [unit("settler"), building("walls")] {
        let (mut game, ranked) = campus_less_empire(95_103, 1);
        all_queue(&mut game, &ranked, &item);
        let ai = armed();
        let plan = ai.assess(&game, 0);
        ai.claim_free_slot_campus(&mut game, 0, &plan);
        assert!(
            ranked
                .iter()
                .all(|cid| head(&game, *cid) == Some(item.clone())),
            "{item:?} is never displaced"
        );
    }
}

/// A city of two citizens keeps its slot: the claim needs three.
#[test]
fn a_small_city_keeps_its_slot() {
    let (mut game, ranked) = campus_less_empire(95_104, 1);
    all_queue(&mut game, &ranked, &unit("knight"));
    for cid in &ranked {
        game.cities.get_mut(cid).unwrap().pop = FREE_SLOT_CAMPUS_MIN_POP - 1;
    }
    let ai = armed();
    let plan = ai.assess(&game, 0);
    ai.claim_free_slot_campus(&mut game, 0, &plan);
    assert!(ranked
        .iter()
        .all(|cid| head(&game, *cid) == Some(unit("knight"))));
}

/// A city that already holds a Campus is passed over for the next one.
#[test]
fn a_city_with_a_campus_is_passed_over() {
    let (mut game, ranked) = campus_less_empire(95_105, 1);
    all_queue(&mut game, &ranked, &unit("knight"));
    install_test_district(&mut game, ranked[0], "campus");
    let ai = armed();
    let plan = ai.assess(&game, 0);
    ai.claim_free_slot_campus(&mut game, 0, &plan);
    assert_eq!(head(&game, ranked[0]), Some(unit("knight")));
    assert!(is_campus(&game, &head(&game, ranked[1])));
}

/// At war with a major the military unit keeps its queue.
#[test]
fn at_war_a_unit_keeps_its_queue() {
    let (mut game, ranked) = campus_less_empire(95_106, 2);
    all_queue(&mut game, &ranked, &unit("knight"));
    game.record_contact(0, 1);
    game.at_war.insert((0, 1));
    let ai = armed();
    let plan = ai.assess(&game, 0);
    ai.claim_free_slot_campus(&mut game, 0, &plan);
    assert!(ranked
        .iter()
        .all(|cid| head(&game, *cid) == Some(unit("knight"))));
}

/// Version 2: a two-citizen city claims, and every eligible city claims in
/// the same turn.
#[test]
fn version_two_claims_every_eligible_city_from_two_citizens() {
    let (mut game, ranked) = campus_less_empire(95_108, 1);
    all_queue(&mut game, &ranked, &unit("knight"));
    for cid in &ranked {
        game.cities.get_mut(cid).unwrap().pop = FREE_SLOT_CAMPUS_MIN_POP_2;
    }
    let mut v1 = game.clone();
    let ai = armed();
    let plan = ai.assess(&v1, 0);
    ai.claim_free_slot_campus(&mut v1, 0, &plan);
    assert!(
        ranked
            .iter()
            .all(|cid| head(&v1, *cid) == Some(unit("knight"))),
        "v1: two citizens are under its floor"
    );

    let mut ai = AdvancedAi::new();
    ai.enable_campus_claims_the_free_slot_2();
    let plan = ai.assess(&game, 0);
    ai.claim_free_slot_campus(&mut game, 0, &plan);
    let claimed = ranked
        .iter()
        .filter(|cid| is_campus(&game, &head(&game, **cid)))
        .count();
    assert_eq!(claimed, ranked.len(), "every city claims in one turn");
    let campus = head(&game, ranked[0]).unwrap();
    assert!(ai.free_slot_campus_holds(&game, 0, ranked[0], &plan, &campus));
}

/// The live shape: a long repeated Settler, another walking, and a legal
/// first Seowon quoted at three turns. Other cities' Walls keep their queues.
fn korean_settler_and_short_campus() -> (Game, AdvancedAi, u32, StrategicPlan, Item) {
    korean_settler_and_short_campus_with_cities(3)
}

fn korean_settler_and_short_campus_with_cities(
    count: usize,
) -> (Game, AdvancedAi, u32, StrategicPlan, Item) {
    let mut game = Game::new_full(2, 32, 20, 95_210, 250, 0, false);
    game.units.clear();
    for tile in game.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = true;
        tile.resource = None;
    }
    game.players[0].civ = "Korea".to_string();
    for tech in ["pottery", "writing", "mining", "masonry", "bronze_working"] {
        game.players[0].techs.insert(Name::new(tech));
    }
    let cities: Vec<_> = [(5, 5), (15, 5), (25, 5)]
        .into_iter()
        .take(count)
        .map(|pos| game.found_city_for(0, pos, None))
        .collect();
    game.game_speed = GameSpeed::Online;
    game.turn = 20;
    for &cid in &cities {
        let city = game.cities.get_mut(&cid).unwrap();
        city.pop = 5;
        city.production = 0.0;
        city.queue = vec![building("walls")];
        let production = game.city_yields(cid).production;
        Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
            cid,
            Yields {
                production: 11.0 - production,
                ..Default::default()
            },
        );
    }
    let cid = cities[usize::from(count > 1)];
    game.cities.get_mut(&cid).unwrap().queue = vec![unit("settler")];
    game.cities.get_mut(&cid).unwrap().production = 20.0;
    let home = game.cities[&cities[0]].pos;
    game.spawn_unit("settler", 0, home);
    let campus = BasicAi::first_campus_item(&game, 0, cid).expect("legal first Seowon");
    assert!(matches!(&campus, Item::District { district, .. } if *district == "seowon"));
    for (item, cost, turns) in [
        (campus.clone(), 28.0, 3.0),
        (unit("settler"), 85.0, 6.0),
        // The other-civilization control must also have a legal cheap Campus.
        (
            Item::District {
                district: crate::name!("campus"),
                pos: home,
            },
            56.0,
            3.0,
        ),
    ] {
        Arc::make_mut(&mut game.host_buildable)
            .entry(cid)
            .or_default()
            .insert(
                Game::production_block_key(&item),
                crate::game::HostMenuEntry {
                    cost: Some(cost),
                    turns: Some(turns),
                },
            );
    }
    let mut ai = AdvancedAi::new();
    ai.enable_campus_claims_the_free_slot_2();
    ai.enable_early_settler_floor();
    ai.victory_target = Some(VictoryTarget::Science);
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 9,
        assessed_turn: game.turn,
        rush: false,
    };
    assert!(ai.early_settler_floor_holds(&game, 0, cid, &plan));
    assert!(ai.free_slot_campus_item(&game, 0, cid, &plan).is_some());
    (game, ai, cid, plan, campus)
}

#[test]
fn korea_builds_a_three_turn_first_seowon_before_a_long_repeated_settler() {
    let (mut game, ai, cid, plan, campus) = korean_settler_and_short_campus();
    assert_eq!(ai.early_settler_floor(&game, &plan), 6);
    ai.claim_free_slot_campus(&mut game, 0, &plan);
    assert_eq!(head(&game, cid), Some(campus));
    assert_eq!(
        game.item_invested_production(cid, &unit("settler")),
        20.0,
        "the paused Settler keeps its paid work"
    );
    assert_eq!(
        ai.early_settler_floor(&game, &plan),
        6,
        "expansion target is unchanged"
    );
}

#[test]
fn a_zero_progress_native_seowon_survives_the_next_early_floor_claim() {
    let (mut game, ai, cid, plan, campus) = korean_settler_and_short_campus();
    ai.claim_free_slot_campus(&mut game, 0, &plan);
    assert_eq!(head(&game, cid), Some(campus.clone()));
    game.cities.get_mut(&cid).unwrap().production = 0.0;
    assert_eq!(game.item_invested_production(cid, &campus), 0.0);
    assert!(
        AdvancedAi::early_settler_may_displace(&game, 0, cid),
        "old floor would replace the foundation"
    );
    assert!(ai.short_seowon_holds_settler_floor(&game, 0, cid));
    assert!(ai.base.has_practical_settle_site(&game, 0));
    let mut control = game.clone();
    let mut without_pause = ai.clone();
    without_pause.disable_campus_claims_the_free_slot_2();
    without_pause.claim_early_settler_floor(&mut control, 0, &plan);
    assert_eq!(
        head(&control, cid),
        Some(unit("settler")),
        "the existing floor really would take this queue without the protection"
    );
    ai.claim_early_settler_floor(&mut game, 0, &plan);
    assert_eq!(head(&game, cid), Some(campus));
}

#[test]
fn the_first_city_keeps_its_settler_even_with_a_short_native_seowon_quote() {
    let (mut game, ai, cid, plan, _) = korean_settler_and_short_campus_with_cities(1);
    ai.claim_free_slot_campus(&mut game, 0, &plan);
    assert_eq!(head(&game, cid), Some(unit("settler")));
}

#[test]
fn the_short_seowon_pause_keeps_its_scope_and_expansion_safety_guards() {
    let (base, armed, cid, plan, campus) = korean_settler_and_short_campus();
    for case in [
        "version one",
        "other civilization",
        "other victory",
        "no walker",
        "slow campus",
        "short settler",
        "no native quote",
        "threat",
        "rush",
        "recovery",
        "war",
        "closed window",
    ] {
        let mut game = base.clone();
        let mut ai = armed.clone();
        let mut plan = plan.clone();
        match case {
            "version one" => {
                ai.disable_campus_claims_the_free_slot_2();
                ai.enable_campus_claims_the_free_slot();
            }
            "other civilization" => game.players[0].civ = "Rome".to_string(),
            "other victory" => ai.victory_target = Some(VictoryTarget::Culture),
            "no walker" => {
                for uid in game.player_unit_ids(0) {
                    game.remove_unit(uid);
                }
            }
            "slow campus" => {
                Arc::make_mut(&mut game.host_buildable)
                    .get_mut(&cid)
                    .unwrap()
                    .get_mut(&Game::production_block_key(&campus))
                    .unwrap()
                    .turns = Some(4.0)
            }
            "short settler" => {
                Arc::make_mut(&mut game.host_buildable)
                    .get_mut(&cid)
                    .unwrap()
                    .get_mut(&Game::production_block_key(&unit("settler")))
                    .unwrap()
                    .turns = Some(3.0)
            }
            "no native quote" => game.host_buildable = Arc::default(),
            "threat" => plan.threatened_city = Some(cid),
            "rush" => plan.rush = true,
            "recovery" => plan.strategy = GrandStrategy::Recovery,
            "war" => {
                game.at_war.insert((0, 1));
            }
            "closed window" => {
                game.turn = game.standard_duration(
                    super::super::early_settler_floor::EARLY_SETTLER_FLOOR_STANDARD_TURN,
                )
            }
            _ => unreachable!(),
        }
        ai.claim_free_slot_campus(&mut game, 0, &plan);
        assert_eq!(head(&game, cid), Some(unit("settler")), "{case}");
    }
}

fn korean_campus_and_parity_unit() -> (Game, AdvancedAi, u32, StrategicPlan, Item) {
    let (mut game, mut ai, cid, plan, campus) = one_citizen_native_korean_city();
    game.cities.get_mut(&cid).unwrap().pop = 4;
    game.cities.get_mut(&cid).unwrap().queue = vec![unit("warrior")];
    game.cities.get_mut(&cid).unwrap().production = 7.0;
    Arc::make_mut(&mut game.host_buildable)
        .get_mut(&cid)
        .unwrap()
        .get_mut(&Game::production_block_key(&campus))
        .unwrap()
        .turns = Some(5.0);
    Arc::make_mut(&mut game.host_buildable)
        .get_mut(&cid)
        .unwrap()
        .insert(
            Game::production_block_key(&unit("warrior")),
            crate::game::HostMenuEntry {
                cost: Some(40.0),
                turns: Some(3.0),
            },
        );
    game.place_city(1, (23, 5), None);
    game.players[0].met.insert(1);
    game.players[1].met.insert(0);
    Arc::make_mut(&mut game.observed_military_power).insert(0, 30.0);
    Arc::make_mut(&mut game.observed_military_power).insert(1, 120.0);
    ai.enable_near_rival_deterrence();
    assert!(ai.deterrence_unit_holds(&game, 0, &unit("warrior")));
    (game, ai, cid, plan, campus)
}

#[test]
fn a_five_turn_korean_science_seowon_precedes_parity_without_disabling_it() {
    let (mut game, ai, cid, plan, campus) = korean_campus_and_parity_unit();
    let mut control = game.clone();
    let mut version_one = ai.clone();
    version_one.disable_campus_claims_the_free_slot_2();
    version_one.enable_campus_claims_the_free_slot();
    version_one.claim_free_slot_campus(&mut control, 0, &plan);
    assert_eq!(head(&control, cid), Some(unit("warrior")));
    ai.claim_free_slot_campus(&mut game, 0, &plan);
    assert_eq!(head(&game, cid), Some(campus));
    assert_eq!(game.item_invested_production(cid, &unit("warrior")), 7.0);
    assert!(ai.deterrence_threat(&game, 0).is_some());
}

#[test]
fn parity_keeps_a_zero_progress_first_seowon_when_the_native_quote_changes() {
    let (mut game, ai, cid, plan, campus) = korean_campus_and_parity_unit();
    ai.claim_free_slot_campus(&mut game, 0, &plan);
    assert_eq!(head(&game, cid), Some(campus.clone()));
    assert_eq!(game.item_invested_production(cid, &campus), 0.0);
    Arc::make_mut(&mut game.host_buildable)
        .get_mut(&cid)
        .unwrap()
        .get_mut(&Game::production_block_key(&campus))
        .unwrap()
        .turns = Some(6.0);
    assert!(!ai.short_seowon_precedes_parity(&game, 0, cid, &plan, &campus));
    assert!(ai.first_seowon_holds_parity(&game, 0, cid, &plan));
    ai.claim_deterrence_unit(&mut game, 0, &plan);
    assert_eq!(head(&game, cid), Some(campus));
}

#[test]
fn the_parity_pause_keeps_native_budget_and_emergency_guards() {
    let (base, armed, cid, plan, campus) = korean_campus_and_parity_unit();
    for case in [
        "version one",
        "other civilization",
        "other victory",
        "science disabled",
        "threat",
        "other city threatened",
        "recent attack",
        "rush",
        "recovery",
        "war",
        "no native quote",
        "slow",
        "zero",
        "negative",
        "nonfinite",
        "nan",
    ] {
        let mut game = base.clone();
        let mut ai = armed.clone();
        let mut plan = plan.clone();
        match case {
            "version one" => {
                ai.disable_campus_claims_the_free_slot_2();
                ai.enable_campus_claims_the_free_slot();
            }
            "other civilization" => game.players[0].civ = "Rome".to_string(),
            "other victory" => ai.victory_target = Some(VictoryTarget::Culture),
            "science disabled" => game.victory_conditions.science = false,
            "threat" => plan.threatened_city = Some(cid),
            "other city threatened" => {
                plan.threatened_city = game.player_city_ids(0).into_iter().find(|c| *c != cid)
            }
            "recent attack" => game.cities.get_mut(&cid).unwrap().last_attacked = game.turn - 1,
            "rush" => plan.rush = true,
            "recovery" => plan.strategy = GrandStrategy::Recovery,
            "war" => {
                game.at_war.insert((0, 1));
            }
            "no native quote" => game.host_buildable = Arc::default(),
            _ => {
                Arc::make_mut(&mut game.host_buildable)
                    .get_mut(&cid)
                    .unwrap()
                    .get_mut(&Game::production_block_key(&campus))
                    .unwrap()
                    .turns = Some(match case {
                    "slow" => 6.0,
                    "zero" => 0.0,
                    "negative" => -1.0,
                    "nonfinite" => f64::INFINITY,
                    "nan" => f64::NAN,
                    _ => unreachable!(),
                });
            }
        }
        ai.claim_free_slot_campus(&mut game, 0, &plan);
        assert_eq!(head(&game, cid), Some(unit("warrior")), "{case}");
    }
}

/// `rear-cities-grow-through-the-war-hold`: while the war hold names its rear
/// cities, the free slot's first Campus goes to the quickest REAR city, and
/// the quicker front city keeps its queue. With no rear set (the gene off or
/// no hold) the quickest city claims as before.
#[test]
fn under_the_war_hold_the_first_campus_goes_to_the_quickest_rear_city() {
    // The seed the peacetime claim test above proves: ranked[0] claims first,
    // ranked[1] next.
    let (mut game, ranked) = campus_less_empire(95_101, 1);
    all_queue(&mut game, &ranked, &unit("knight"));

    let mut ai = armed();
    let plan = ai.assess(&game, 0);
    let mut open = game.clone();
    ai.claim_free_slot_campus(&mut open, 0, &plan);
    assert!(
        is_campus(&open, &head(&open, ranked[0])),
        "no rear set: the quickest city claims"
    );

    ai.rear_cities_open_under_the_hold = Some(ranked[1..].iter().copied().collect());
    let mut held = game.clone();
    ai.claim_free_slot_campus(&mut held, 0, &plan);
    assert_eq!(
        head(&held, ranked[0]),
        Some(unit("knight")),
        "the front city keeps its queue"
    );
    assert!(
        is_campus(&held, &head(&held, ranked[1])),
        "the quickest rear city claims its first Campus"
    );
}
