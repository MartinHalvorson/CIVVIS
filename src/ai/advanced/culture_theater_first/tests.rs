use super::*;
use crate::rules::Yields;
use crate::setup::GameSpeed;
use std::sync::Arc;

/// Three cities of four citizens with Drama and Poetry and Writing in hand,
/// production 10, no district but the centre: every city has a free slot and
/// can build its first Theater Square and its first Campus.
fn theater_less_empire() -> (Game, Vec<u32>) {
    let mut game = Game::new_full(
        1,
        44,
        26,
        crate::rng::fixture_seed("CULTURETHEATER", 1),
        250,
        0,
        false,
    );
    // A plain Theater Square: Greece's Acropolis needs hills the fixture
    // does not promise, and Rome founds with a Monument.
    game.players[0].civ = "Gran Colombia".to_string();
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|unit| game.units[unit].kind == "settler")
        .unwrap();
    game.apply(0, &Action::FoundCity { unit: settler }).unwrap();
    while game.player_city_ids(0).len() < 3 {
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
            .expect("room for three cities");
        game.found_city_for(0, pos, None);
    }
    for unit in game.player_unit_ids(0) {
        game.remove_unit(unit);
    }
    game.game_speed = GameSpeed::Online;
    game.turn = 70;
    // Forest, jungle and marsh sites need their removal technologies.
    for tech in [
        "writing",
        "mining",
        "bronze_working",
        "irrigation",
        "pottery",
        "animal_husbandry",
    ] {
        game.players[0].techs.insert(Name::new(tech));
    }
    game.players[0].civics.insert(Name::new("drama_poetry"));
    let cities = game.player_city_ids(0);
    for cid in &cities {
        game.cities.get_mut(cid).unwrap().pop = 4;
        Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
            *cid,
            Yields {
                // Low enough that a routine Warrior does not finish this turn.
                production: 10.0,
                ..Default::default()
            },
        );
        assert!(
            BasicAi::first_theater_item(&game, 0, *cid).is_some(),
            "fixture: the first Theater Square is buildable"
        );
    }
    (game, cities)
}

fn seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    if gene {
        ai.enable_culture_seat_theater_first();
    }
    ai
}

fn unit(name: &str) -> Item {
    Item::Unit {
        unit: Name::new(name),
    }
}

fn is_theater(game: &Game, item: Option<&Item>) -> bool {
    matches!(item, Some(Item::District { district, .. })
        if game.district_family(*district) == "theater_square")
}

fn claim(ai: &AdvancedAi, game: &mut Game) {
    let plan = ai.assess(game, 0);
    ai.claim_culture_theaters(game, 0, &plan);
}

/// A Culture seat's cities training Warriors put the Theater first; off, or
/// for a Domination seat, nothing moves; a Settler keeps its queue.
#[test]
fn a_culture_seat_claims_its_theaters_over_routine_units() {
    let (game, cities) = theater_less_empire();
    let mut busy = game.clone();
    for cid in &cities {
        busy.cities.get_mut(cid).unwrap().queue = vec![unit("warrior")];
    }

    for (ai, why) in [
        (seat(VictoryTarget::Culture, false), "off"),
        (seat(VictoryTarget::Domination, true), "a Domination seat"),
    ] {
        let mut kept = busy.clone();
        claim(&ai, &mut kept);
        for cid in &cities {
            assert_eq!(
                kept.cities[cid].queue.first(),
                Some(&unit("warrior")),
                "{why}: no claim"
            );
        }
    }

    let ai = seat(VictoryTarget::Culture, true);
    let mut on = busy.clone();
    claim(&ai, &mut on);
    let plan = ai.assess(&on, 0);
    for cid in &cities {
        let head = on.cities[cid].queue.first().cloned();
        assert!(is_theater(&on, head.as_ref()), "the Theater goes first");
        assert!(ai.culture_theater_holds(&on, 0, *cid, &plan, head.as_ref().unwrap()));
    }

    let mut pump = busy.clone();
    pump.cities.get_mut(&cities[0]).unwrap().queue = vec![unit("settler")];
    claim(&ai, &mut pump);
    assert_eq!(
        pump.cities[&cities[0]].queue.first(),
        Some(&unit("settler")),
        "a Settler keeps its queue"
    );
}

/// The free-slot Campus claim yields a Culture seat's city to its Theater;
/// with the gene off it takes the slot as before.
#[test]
fn the_free_slot_campus_yields_to_the_theater() {
    let (game, cities) = theater_less_empire();
    let mut on = seat(VictoryTarget::Culture, true);
    on.enable_campus_claims_the_free_slot_2();
    let plan = on.assess(&game, 0);
    assert!(on
        .free_slot_campus_item(&game, 0, cities[0], &plan)
        .is_none());

    let mut off = seat(VictoryTarget::Culture, false);
    off.enable_campus_claims_the_free_slot_2();
    let plan = off.assess(&game, 0);
    assert!(off
        .free_slot_campus_item(&game, 0, cities[0], &plan)
        .is_some());
}

fn seat_two() -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.enable_culture_seat_theater_first_2();
    ai
}

/// Version two: with no Campus the empire claims one Theater, then the rest
/// wait; two standing Campuses admit a second.
#[test]
fn version_two_balances_theaters_against_campuses() {
    let (game, cities) = theater_less_empire();
    let mut busy = game.clone();
    for cid in &cities {
        busy.cities.get_mut(cid).unwrap().queue = vec![unit("warrior")];
    }
    let ai = seat_two();
    assert!(!ai.culture_seat_theater_first, "version two turns one off");

    let mut none = busy.clone();
    claim(&ai, &mut none);
    let claimed = cities
        .iter()
        .filter(|cid| is_theater(&none, none.cities[cid].queue.first()))
        .count();
    assert_eq!(claimed, 1, "the first Theater always goes ahead");

    let mut two_campuses = busy.clone();
    for cid in &cities[1..] {
        crate::game::install_test_district(&mut two_campuses, *cid, "campus");
    }
    claim(&ai, &mut two_campuses);
    let claimed = cities
        .iter()
        .filter(|cid| is_theater(&two_campuses, two_campuses.cities[cid].queue.first()))
        .count();
    assert_eq!(claimed, 2, "two Campuses admit two Theaters");
}

/// Version two: once the Theaters lead, the free-slot Campus keeps the slot.
#[test]
fn version_two_returns_the_slot_to_the_campus() {
    let (mut game, cities) = theater_less_empire();
    let theater = BasicAi::first_theater_item(&game, 0, cities[0]).unwrap();
    game.cities.get_mut(&cities[0]).unwrap().queue = vec![theater];
    let mut ai = seat_two();
    ai.enable_campus_claims_the_free_slot_2();
    let plan = ai.assess(&game, 0);

    assert!(ai
        .culture_theater_item(&game, 0, cities[1], &plan)
        .is_none());
    assert!(ai
        .free_slot_campus_item(&game, 0, cities[1], &plan)
        .is_some());
}
