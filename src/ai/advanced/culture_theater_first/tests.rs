use super::*;
use crate::rules::Yields;
use crate::setup::GameSpeed;
use std::sync::Arc;

/// Three cities of four citizens with Drama and Poetry and Writing in hand,
/// production 30, no district but the centre: every city has a free slot and
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
    game.players[0].techs.insert(Name::new("writing"));
    game.players[0].civics.insert(Name::new("drama_poetry"));
    let cities = game.player_city_ids(0);
    for cid in &cities {
        game.cities.get_mut(cid).unwrap().pop = 4;
        Arc::make_mut(&mut game.observed_city_yield_adjustments).insert(
            *cid,
            Yields {
                production: 30.0,
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
