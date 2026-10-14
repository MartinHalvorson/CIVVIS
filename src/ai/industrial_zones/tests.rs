use super::*;
use crate::ai::commercial_hub::tests::emperor_governor;
use crate::game::{install_test_district, Action};
use crate::name::Name;
use crate::rules::Yields;
use crate::setup::GameSpeed;
use std::sync::Arc;

/// The Emperor pin's delegated governor (`emperor_governor`, Commercial Hub
/// step off) with or without `industrial-zone-in-the-producers`.
pub(crate) fn governor(gene: bool) -> BasicAi {
    let mut ai = emperor_governor(false);
    ai.industrial_zone_in_the_producers = gene;
    ai
}

/// An Emperor-shaped empire at turn 90 Online with Apprenticeship: six cities
/// of population four (two specialty slots), each with a Monument, a Granary,
/// a Campus and its Library, room to grow and no Amenity shortfall; no
/// Industrial Zone, no army, no unit. `players` majors; the others hold
/// nothing. The cities come back from the most productive (40) down (10).
pub(crate) fn producer_empire(tag: &str, seed: u64, players: usize) -> (Game, Vec<u32>) {
    let mut game = Game::new_full(
        players,
        44,
        26,
        crate::rng::fixture_seed(tag, seed),
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
    game.turn = 90;
    game.difficulty = "emperor".to_string();
    game.players[0].gold = 500.0;
    game.players[0].gold_per_turn = 5.0;
    for tech in [
        "pottery",
        "writing",
        "mining",
        "currency",
        "bronze_working",
        "animal_husbandry",
        "irrigation",
        "masonry",
        "apprenticeship",
    ] {
        game.players[0].techs.insert(Name::new(tech));
    }
    game.players[0].civics.insert(crate::name!("code_of_laws"));
    let cities = game.player_city_ids(0);
    for (rank, cid) in cities.iter().enumerate() {
        let city = game.cities.get_mut(cid).unwrap();
        city.pop = 4;
        for building in ["monument", "granary", "library"] {
            city.buildings.push(Name::new(building));
        }
        install_test_district(&mut game, *cid, "campus");
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
    (game, ranked)
}

/// The governor's pick for `cid` in the six-city empire: no Settler, six
/// Builders, one Trader, no army.
pub(crate) fn pick(ai: &BasicAi, game: &Game, cid: u32) -> Option<Item> {
    ai.pick_item(game, 0, cid, 6, 0, 6, 1, 0, 0, 0, 0)
}

pub(crate) fn is_zone(game: &Game, item: &Option<Item>) -> bool {
    matches!(item, Some(Item::District { district, .. })
        if game.district_family(*district) == "industrial_zone")
}

fn building(name: &str) -> Option<Item> {
    Some(Item::Building {
        building: Name::new(name),
    })
}

/// The zone site of highest Production adjacency `cid` can open.
fn best_production_adjacency(game: &Game, cid: u32) -> f64 {
    let zone = BasicAi::civ_district(game, 0, "industrial_zone");
    game.district_sites(cid, zone)
        .into_iter()
        .map(|site| game.district_yields(zone, site).production)
        .fold(f64::MIN, f64::max)
}

/// See `BasicAi::industrial_zone_in_the_producers`: on the Emperor shape the
/// stock governor spends the cities on the lent army (or the industrial hub's
/// one zone); under the gene the three most productive cities open an
/// Industrial Zone on their best Production-adjacency site and the rest keep
/// the stock pick. Without Apprenticeship, the stock pick.
#[test]
fn the_three_most_productive_cities_open_an_industrial_zone_under_the_gene() {
    let (game, ranked) = producer_empire("PRODUCERZONE", 93_001, 1);
    assert!(
        !governor(false).settler_due(&game, 0, ranked[0], 6, 0),
        "fixture: the city target is met"
    );
    let mut zones = 0;
    for (rank, cid) in ranked.iter().enumerate() {
        let stock = pick(&governor(false), &game, *cid);
        assert!(stock.is_some(), "rank {rank}: the stock governor builds");
        let gene = pick(&governor(true), &game, *cid);
        if rank >= PRODUCER_ZONE_MAX || is_zone(&game, &stock) {
            // The rest, and the industrial hub's own city, keep the stock pick.
            assert_eq!(gene, stock, "rank {rank}: the stock pick");
            continue;
        }
        let Some(Item::District { district, pos }) = gene else {
            panic!("rank {rank}: the zone opens: {gene:?}");
        };
        assert!(is_zone(&game, &gene), "rank {rank}: {gene:?}");
        assert_eq!(
            game.district_yields(district, pos).production,
            best_production_adjacency(&game, *cid),
            "rank {rank}: the best Production-adjacency site"
        );
        zones += 1;
    }
    assert!(zones >= 2, "fixture: the gene changes at least two picks ({zones})");
    let mut early = game.clone();
    early.players[0].techs.remove(&crate::name!("apprenticeship"));
    for cid in &ranked[..PRODUCER_ZONE_MAX] {
        assert_eq!(
            pick(&governor(true), &early, *cid),
            pick(&governor(false), &early, *cid),
            "no Apprenticeship: the stock pick"
        );
    }
}

/// Below six cities the step wants two zones, from six three; a zone queued
/// in a city counts, so the next cities in line take the open ones, and the
/// step stops once the wanted zones stand.
#[test]
fn zones_held_or_queued_close_the_step() {
    let (mut game, ranked) = producer_empire("PRODUCERHELD", 93_003, 1);
    let ai = governor(true);
    let zone = BasicAi::civ_district(&game, 0, "industrial_zone");
    assert!(is_zone(
        &game,
        &ai.industrial_zone_producer_item(&game, 0, ranked[1], 5, None)
    ));
    assert_eq!(
        ai.industrial_zone_producer_item(&game, 0, ranked[2], 5, None),
        None,
        "five cities: two zones wanted"
    );
    // The top city queues its zone: the next two take the open ones.
    let pos = game.district_sites(ranked[0], zone)[0];
    game.cities
        .get_mut(&ranked[0])
        .unwrap()
        .queue
        .push(Item::District {
            district: zone,
            pos,
        });
    assert_eq!(
        ai.industrial_zone_producer_item(&game, 0, ranked[0], 6, None),
        None,
        "its zone is queued"
    );
    for cid in &ranked[1..3] {
        assert!(is_zone(
            &game,
            &ai.industrial_zone_producer_item(&game, 0, *cid, 6, None)
        ));
    }
    assert_eq!(
        ai.industrial_zone_producer_item(&game, 0, ranked[3], 6, None),
        None
    );
    // Three zones standing close it.
    let (mut full, ranked) = producer_empire("PRODUCERHELD", 93_003, 1);
    for cid in &ranked[3..] {
        install_test_district(&mut full, *cid, "industrial_zone");
    }
    for cid in &ranked[..3] {
        assert_eq!(
            ai.industrial_zone_producer_item(&full, 0, *cid, 6, None),
            None,
            "three zones stand"
        );
    }
}

/// A standing zone builds its Workshop ahead of the army in the most
/// productive zone cities; a fourth zone city keeps the stock pick.
#[test]
fn a_standing_zone_builds_its_workshop() {
    let (mut game, ranked) = producer_empire("PRODUCERWORKSHOP", 93_005, 1);
    for cid in &ranked {
        install_test_district(&mut game, *cid, "industrial_zone");
    }
    let workshop = building("workshop");
    for cid in &ranked[..PRODUCER_ZONE_MAX] {
        assert_eq!(pick(&governor(true), &game, *cid), workshop);
    }
    assert_eq!(
        pick(&governor(true), &game, ranked[3]),
        pick(&governor(false), &game, ranked[3]),
        "the fourth zone city keeps the stock pick"
    );
    assert!(
        ranked[..PRODUCER_ZONE_MAX]
            .iter()
            .any(|cid| pick(&governor(false), &game, *cid) != workshop),
        "fixture: the stock governor builds a Workshop in at most the hub city"
    );
    // A built Workshop leaves nothing for the step.
    game.cities
        .get_mut(&ranked[1])
        .unwrap()
        .buildings
        .push(crate::name!("workshop"));
    assert_eq!(
        governor(true).industrial_zone_producer_item(&game, 0, ranked[1], 6, None),
        None
    );
}

/// The city's standing Campus's Library takes the zone's slot; under
/// `campus-buildings-first` that step's University does.
#[test]
fn the_campus_building_takes_the_zones_slot() {
    let (mut game, ranked) = producer_empire("PRODUCERCAMPUS", 93_007, 1);
    let top = ranked[0];
    game.cities
        .get_mut(&top)
        .unwrap()
        .buildings
        .retain(|held| *held != "library");
    assert_eq!(pick(&governor(true), &game, top), building("library"));
    let (mut game, ranked) = producer_empire("PRODUCERCAMPUS", 93_007, 1);
    game.players[0].techs.insert(crate::name!("education"));
    game.players[0].techs.insert(crate::name!("mathematics"));
    let top = ranked[0];
    assert!(
        is_zone(&game, &pick(&governor(true), &game, top)),
        "without campus-buildings-first the University waits"
    );
    let mut both = governor(true);
    both.campus_buildings_first = true;
    assert_eq!(pick(&both, &game, top), building("university"));
}

/// The plan's threatened city and a city attacked within four turns keep
/// the stock pick, and the next city takes the zone instead.
#[test]
fn a_threatened_city_keeps_its_stock_pick() {
    let (game, ranked) = producer_empire("PRODUCERTHREAT", 93_009, 1);
    let top = ranked[0];
    let lent = |gene: bool| {
        let mut ai = governor(gene);
        ai.plan_threatened_city = Some(top);
        ai
    };
    assert_eq!(pick(&lent(true), &game, top), pick(&lent(false), &game, top));
    assert!(
        is_zone(
            &game,
            &lent(true).industrial_zone_producer_item(&game, 0, ranked[3], 6, Some(top))
        ),
        "the fourth city takes the threatened city's zone"
    );
    assert_eq!(
        governor(true).industrial_zone_producer_item(&game, 0, ranked[3], 6, None),
        None,
        "fixture: unthreatened, the fourth city takes none"
    );
    let mut attacked = game.clone();
    attacked.cities.get_mut(&top).unwrap().last_attacked = 88;
    assert_eq!(
        pick(&governor(true), &attacked, top),
        pick(&governor(false), &attacked, top),
        "attacked two turns ago"
    );
    attacked.cities.get_mut(&top).unwrap().last_attacked = 80;
    assert!(
        is_zone(
            &attacked,
            &governor(true).industrial_zone_producer_item(&attacked, 0, top, 6, None)
        ),
        "attacked ten turns ago"
    );
}

/// A housing-bound city builds its Granary first: the housing gene's step
/// stands ahead, and alone this step yields to the Granary.
#[test]
fn a_housing_bound_city_builds_its_granary_first() {
    let (mut game, ranked) = producer_empire("PRODUCERGRANARY", 93_011, 1);
    let top = ranked[0];
    game.cities
        .get_mut(&top)
        .unwrap()
        .buildings
        .retain(|held| *held != "granary");
    let housing = game.city_housing(&game.cities[&top]);
    *Arc::make_mut(&mut game.observed_city_housing_adjustments)
        .get_mut(&top)
        .unwrap() += game.cities[&top].pop as f64 - housing;
    assert_eq!(pick(&governor(true), &game, top), building("granary"));
    assert_eq!(
        governor(true).industrial_zone_producer_item(&game, 0, top, 6, None),
        None,
        "the step yields"
    );
}

/// A due Settler comes first: with the city target above the empire, the
/// gene's pick is the stock pick.
#[test]
fn a_due_settler_comes_before_the_zone() {
    let (game, ranked) = producer_empire("PRODUCERSETTLER", 93_013, 1);
    let roomy = |gene: bool| {
        let mut ai = governor(gene);
        ai.w.city_target = 10.0;
        ai
    };
    assert!(
        roomy(false).settler_due(&game, 0, ranked[0], 6, 0),
        "fixture: a Settler is due"
    );
    assert_eq!(pick(&roomy(true), &game, ranked[0]), pick(&roomy(false), &game, ranked[0]));
}

/// Under `commercial-hub-and-traders` too, the producer's one free slot goes
/// to the zone before the Commercial Hub.
#[test]
fn the_zone_comes_before_the_commercial_hub() {
    let (mut game, ranked) = producer_empire("PRODUCERHUB", 93_015, 1);
    game.players[0].civics.insert(crate::name!("foreign_trade"));
    let both = |gene: bool| {
        let mut ai = emperor_governor(true);
        ai.industrial_zone_in_the_producers = gene;
        ai
    };
    // No route slot: the hub step's first build is the hub itself.
    Arc::make_mut(&mut game.observed_trade_capacity).insert(0, 0);
    let hubs: Vec<u32> = ranked[..PRODUCER_ZONE_MAX]
        .iter()
        .copied()
        .filter(|cid| {
            matches!(pick(&both(false), &game, *cid), Some(Item::District { district, .. })
                if game.district_family(district) == "commercial_hub")
        })
        .collect();
    assert!(!hubs.is_empty(), "fixture: the hub step opens a hub");
    for cid in hubs {
        assert!(is_zone(&game, &pick(&both(true), &game, cid)));
    }
}

/// `industrial_zone_build_holds`: a queued zone or Workshop under the gene,
/// never in a threatened city, never another item, never off.
#[test]
fn a_queued_zone_or_workshop_holds_under_the_gene() {
    let (game, ranked) = producer_empire("PRODUCERHOLDS", 93_017, 1);
    let top = ranked[0];
    let zone = BasicAi::civ_district(&game, 0, "industrial_zone");
    let item = Item::District {
        district: zone,
        pos: game.district_sites(top, zone)[0],
    };
    let workshop = building("workshop").unwrap();
    assert!(governor(true).industrial_zone_build_holds(&game, top, &item, None));
    assert!(governor(true).industrial_zone_build_holds(&game, top, &workshop, None));
    assert!(!governor(false).industrial_zone_build_holds(&game, top, &item, None));
    assert!(!governor(true).industrial_zone_build_holds(&game, top, &item, Some(top)));
    assert!(!governor(true).industrial_zone_build_holds(
        &game,
        top,
        &building("university").unwrap(),
        None
    ));
}
