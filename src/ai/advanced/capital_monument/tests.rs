use super::*;

/// One major: our capital founded with every unit removed, a second city of
/// ours `second` tiles away when asked, turn 30, and 50 Production already
/// banked on the Monument so it finishes inside the cap. Returns the game,
/// the capital and the Monument item.
fn capital(second: bool) -> Option<(Game, u32, Item)> {
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
    let ours = game.player_city_ids(0)[0];
    if second {
        let home = game.cities[&ours].pos;
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
        let site = *sites.first().expect("fixture: a second site");
        game.place_city(0, site, None);
        assert_eq!(game.player_city_ids(0).len(), 2, "fixture: two cities");
    }
    game.turn = 30;
    let monument = Item::Building {
        building: crate::name!("monument"),
    };
    let city = game.cities.get_mut(&ours).unwrap();
    city.queue.clear();
    city.production_progress
        .insert(Game::item_progress_key(&monument), 50.0);
    assert!(
        game.can_produce(0, ours, &monument),
        "fixture: the capital can build its Monument"
    );
    Some((game, ours, monument))
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_capital_monument_after_the_second_city();
    ai
}

fn head(game: &Game, cid: u32) -> Option<Item> {
    game.cities[&cid].queue.first().cloned()
}

fn queue(game: &mut Game, cid: u32, item: Item) {
    game.cities.get_mut(&cid).unwrap().queue = vec![item];
}

fn unit(name: &str) -> Item {
    Item::Unit {
        unit: crate::name::Name::new(name),
    }
}

fn claim(ai: &AdvancedAi, game: &mut Game) {
    let plan = ai.assess(game, 0);
    ai.claim_capital_monument(game, 0, &plan);
}

/// With the second city standing, a capital training a Warrior puts its
/// Monument first; off, with one city, or with the Monument built, nothing
/// moves.
#[test]
fn the_capital_claims_its_monument_once_the_second_city_stands() {
    let Some((game, cap, monument)) = capital(true) else {
        panic!("fixture: a capital that can build its Monument, two cities");
    };
    let mut busy = game.clone();
    queue(&mut busy, cap, unit("warrior"));

    let mut off = busy.clone();
    claim(&AdvancedAi::new(), &mut off);
    assert_eq!(head(&off, cap), Some(unit("warrior")), "off: no claim");

    let ai = armed();
    let mut on = busy.clone();
    claim(&ai, &mut on);
    assert_eq!(head(&on, cap), Some(monument.clone()), "the Monument first");
    let plan = ai.assess(&on, 0);
    assert!(ai.capital_monument_holds(&on, 0, cap, &plan, &monument));

    let Some((single, single_cap, _)) = capital(false) else {
        panic!("fixture: one city");
    };
    let mut alone = single.clone();
    queue(&mut alone, single_cap, unit("warrior"));
    claim(&ai, &mut alone);
    assert_eq!(
        head(&alone, single_cap),
        Some(unit("warrior")),
        "one city: the pump keeps its queue"
    );

    let mut built = busy.clone();
    built
        .cities
        .get_mut(&cap)
        .unwrap()
        .buildings
        .push(crate::name!("monument"));
    claim(&ai, &mut built);
    assert_eq!(
        head(&built, cap),
        Some(unit("warrior")),
        "built: nothing moves"
    );
}

/// An idle capital takes the Monument; a Settler in progress and Walls keep
/// their queue; an attacked capital does not stop for it.
#[test]
fn settlers_walls_and_an_attacked_capital_keep_their_queue() {
    let Some((game, cap, monument)) = capital(true) else {
        panic!("fixture: a capital that can build its Monument, two cities");
    };
    let ai = armed();

    let mut idle = game.clone();
    claim(&ai, &mut idle);
    assert_eq!(
        head(&idle, cap),
        Some(monument.clone()),
        "an idle queue takes it"
    );

    for keep in [
        unit("settler"),
        Item::Building {
            building: crate::name!("walls"),
        },
    ] {
        let mut kept = game.clone();
        queue(&mut kept, cap, keep.clone());
        claim(&ai, &mut kept);
        assert_eq!(head(&kept, cap), Some(keep), "kept");
    }

    let mut attacked = game.clone();
    queue(&mut attacked, cap, unit("warrior"));
    attacked.cities.get_mut(&cap).unwrap().last_attacked = attacked.turn;
    claim(&ai, &mut attacked);
    assert_eq!(
        head(&attacked, cap),
        Some(unit("warrior")),
        "attacked: no stop"
    );
}
