use super::*;

/// Two majors: our capital and `extra` more cities 5-8 tiles off, population
/// 4 each with idle queues, Currency and the feature-clearing techs known,
/// inside the claim's window. Returns the game.
fn empire(extra: usize) -> Game {
    let mut game = Game::new_full(2, 32, 20, 936_221, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .expect("fixture: a settler");
    game.apply(0, &Action::FoundCity { unit: settler })
        .expect("fixture: the capital founds");
    for pid in [0, 1] {
        for uid in game.player_unit_ids(pid) {
            game.remove_unit(uid);
        }
    }
    let home = game.cities[&game.player_city_ids(0)[0]].pos;
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
    let first = sites[0];
    let second = sites
        .iter()
        .copied()
        .find(|pos| game.wdist(*pos, first) >= 4)
        .expect("fixture: a second site");
    for pos in [first, second].into_iter().take(extra) {
        game.place_city(0, pos, None);
    }
    for tech in [
        "pottery",
        "mining",
        "bronze_working",
        "irrigation",
        "currency",
        "writing",
    ] {
        game.players[0].techs.insert(Name::new(tech));
    }
    for cid in game.player_city_ids(0) {
        let city = game.cities.get_mut(&cid).unwrap();
        city.pop = 4;
        city.queue.clear();
    }
    game.turn = game.standard_duration(HUB_CLAIM_FROM_STANDARD) + 2;
    game
}

fn seat(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_commercial_hub_and_traders();
    if gene {
        ai.enable_commercial_hub_claims_the_route_slot();
    }
    ai
}

fn hub_heads(game: &Game) -> usize {
    game.player_city_ids(0)
        .into_iter()
        .filter(|cid| {
            matches!(game.cities[cid].queue.first(), Some(Item::District { district, .. })
                if game.district_family(*district) == "commercial_hub")
        })
        .count()
}

/// Inside the window, with every route slot taken, one city starts its
/// Commercial Hub; off, nothing does.
#[test]
fn a_full_route_capacity_opens_a_commercial_hub() {
    let game = empire(2);

    let mut on = game.clone();
    let ai = seat(true);
    assert!(
        ai.route_slot_hub(&on, 0).is_some(),
        "the hub step has a pick"
    );
    let plan = ai.assess(&on, 0);
    ai.claim_route_slot_hub(&mut on, 0, &plan);
    assert_eq!(hub_heads(&on), 1);

    let mut off = game.clone();
    let ai = seat(false);
    let plan = ai.assess(&off, 0);
    ai.claim_route_slot_hub(&mut off, 0, &plan);
    assert_eq!(hub_heads(&off), 0);
}

/// Before the window, or below three cities, the claim asks for nothing.
#[test]
fn the_claim_waits_for_its_window_and_three_cities() {
    let ai = seat(true);

    let mut early = empire(2);
    early.turn = early.standard_duration(HUB_CLAIM_FROM_STANDARD) - 1;
    assert!(ai.route_slot_hub(&early, 0).is_none());

    let small = empire(1);
    assert!(ai.route_slot_hub(&small, 0).is_none());
}
