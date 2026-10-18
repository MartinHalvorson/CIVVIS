use super::*;
use crate::game::install_test_district;
use crate::rules::Yields;
use crate::setup::GameSpeed;
use std::sync::Arc;

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
    let (mut game, ranked) = campus_less_empire(95_107, 1);
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
