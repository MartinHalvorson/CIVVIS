use super::*;

/// Two majors: our founded capital at population 4 with a pillaged Campus
/// beside it and a Warrior at its head, every unit removed.
fn pillaged_capital() -> (Game, u32, Pos) {
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
    let capital = game.player_city_ids(0)[0];
    let home = game.cities[&capital].pos;
    let site =
        game.wdisk(home, 1)
            .into_iter()
            .filter(|pos| *pos != home)
            .find(|pos| {
                game.map.tiles.get(pos).is_some_and(|tile| {
                    !game.rules.is_water(tile) && tile.owner_city == Some(capital)
                })
            })
            .expect("fixture: an owned land tile beside the capital");
    let city = game.cities.get_mut(&capital).unwrap();
    city.pop = 4;
    city.queue.clear();
    city.queue.push(Item::Unit {
        unit: crate::name!("warrior"),
    });
    city.districts.insert(crate::name!("campus"), site);
    let tile = game.map.tiles.get_mut(&site).unwrap();
    tile.district = Some(crate::name!("campus"));
    tile.pillaged = true;
    game.turn = 40;
    (game, capital, site)
}

fn seat(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    if gene {
        ai.enable_pillaged_districts_repair_first();
    }
    ai
}

fn is_district_repair(item: Option<&Item>, at: Pos) -> bool {
    matches!(item, Some(Item::Repair { repair, pos }) if *repair == "district" && *pos == at)
}

/// The pillaged Campus is repaired ahead of the Warrior, which keeps its
/// place behind it; off, the queue is untouched.
#[test]
fn a_pillaged_district_is_repaired_first() {
    let (game, capital, site) = pillaged_capital();
    assert!(
        game.producible_items(0, capital)
            .iter()
            .any(|item| is_district_repair(Some(item), site)),
        "fixture: the engine offers the repair"
    );

    let mut on = game.clone();
    seat(true).claim_pillaged_repairs(&mut on, 0);
    assert!(
        is_district_repair(on.cities[&capital].queue.first(), site),
        "the head: {:?}",
        on.cities[&capital].queue.first()
    );

    let mut off = game.clone();
    seat(false).claim_pillaged_repairs(&mut off, 0);
    assert_eq!(off.cities[&capital].queue, game.cities[&capital].queue);
}

/// A barbarian soldier beside the site holds the repair, and a head that is
/// already a repair is left alone.
#[test]
fn a_raider_beside_the_site_holds_the_repair() {
    let (game, capital, site) = pillaged_capital();
    let ai = seat(true);

    let barbarian = game
        .players
        .iter()
        .position(|player| player.is_barbarian)
        .expect("fixture: a barbarian player");
    let mut raided = game.clone();
    let beside = raided
        .wdisk(site, 1)
        .into_iter()
        .find(|pos| {
            *pos != site
                && *pos != raided.cities[&capital].pos
                && raided
                    .map
                    .tiles
                    .get(pos)
                    .is_some_and(|tile| !raided.rules.is_water(tile))
        })
        .expect("fixture: land beside the site");
    raided.spawn_unit("warrior", barbarian, beside);
    assert!(ai.pillaged_repair_item(&raided, 0, capital).is_none());

    let mut repairing = game.clone();
    ai.claim_pillaged_repairs(&mut repairing, 0);
    let head = repairing.cities[&capital].queue.first().cloned();
    assert!(ai.pillaged_repair_item(&repairing, 0, capital).is_none());
    ai.claim_pillaged_repairs(&mut repairing, 0);
    assert_eq!(repairing.cities[&capital].queue.first().cloned(), head);
}
