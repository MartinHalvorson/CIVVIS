use super::*;

/// One major: our capital and a second city, every unit removed, turn 30, and
/// 50 Production banked on each city's Monument so it finishes inside the cap.
/// Returns the game, the two cities and the Monument item.
fn two_cities() -> (Game, Vec<u32>, Item) {
    let mut game = Game::new_full(1, 32, 20, 936_431, 200, 0, false);
    // Rome's Trajan's Column founds every city with a Monument.
    game.players[0].civ = "Gran Colombia".to_string();
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("fixture: a settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("fixture: the capital founds");
    for uid in game.player_unit_ids(0) {
        game.remove_unit(uid);
    }
    let capital = game.player_city_ids(0)[0];
    let home = game.cities[&capital].pos;
    let mut sites: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            (5..=8).contains(&game.wdist(**pos, home)) && !game.rules.is_water(tile)
        })
        .map(|(pos, _)| *pos)
        .collect();
    sites.sort();
    game.place_city(0, *sites.first().expect("fixture: a second site"), None);
    let cities = game.player_city_ids(0);
    assert_eq!(cities.len(), 2, "fixture: two cities");
    game.turn = 30;
    let monument = Item::Building {
        building: crate::name!("monument"),
    };
    for cid in &cities {
        let city = game.cities.get_mut(cid).unwrap();
        city.queue.clear();
        city.production_progress
            .insert(Game::item_progress_key(&monument), 50.0);
        assert!(
            game.can_produce(0, *cid, &monument),
            "fixture: every city can build its Monument"
        );
    }
    (game, cities, monument)
}

fn seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    if gene {
        ai.enable_culture_seat_monuments();
    }
    ai
}

fn unit(name: &str) -> Item {
    Item::Unit {
        unit: crate::name::Name::new(name),
    }
}

fn claim(ai: &AdvancedAi, game: &mut Game) {
    let plan = ai.assess(game, 0);
    ai.claim_culture_monuments(game, 0, &plan);
}

/// Every city of a Culture seat training a Scout puts its Monument first and
/// the claim holds; off, or for a Domination seat, nothing moves; a Settler
/// keeps its queue.
#[test]
fn a_culture_seat_claims_every_citys_monument() {
    let (game, cities, monument) = two_cities();
    let mut busy = game.clone();
    for cid in &cities {
        busy.cities.get_mut(cid).unwrap().queue = vec![unit("scout")];
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
                Some(&unit("scout")),
                "{why}: no claim"
            );
        }
    }

    let ai = seat(VictoryTarget::Culture, true);
    let mut on = busy.clone();
    claim(&ai, &mut on);
    let plan = ai.assess(&on, 0);
    for cid in &cities {
        assert_eq!(
            on.cities[cid].queue.first(),
            Some(&monument),
            "the Monument goes first"
        );
        assert!(ai.culture_monument_holds(&on, 0, *cid, &plan, &monument));
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

/// A city with its Monument built claims nothing more.
#[test]
fn a_built_monument_is_not_claimed_again() {
    let (mut game, cities, monument) = two_cities();
    let Item::Building { building } = monument else {
        unreachable!()
    };
    game.cities
        .get_mut(&cities[0])
        .unwrap()
        .buildings
        .push(building);
    let ai = seat(VictoryTarget::Culture, true);
    let plan = ai.assess(&game, 0);
    assert!(ai
        .culture_monument_item(&game, 0, cities[0], &plan)
        .is_none());
    assert!(ai
        .culture_monument_item(&game, 0, cities[1], &plan)
        .is_some());
}
