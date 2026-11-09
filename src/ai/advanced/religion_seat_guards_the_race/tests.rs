use super::*;

/// Two majors: our capital with a Holy Site beside it, every unit removed
/// and a Settler at the head of its queue. `pillaged` sets the site's state.
fn seat_board(target: VictoryTarget, pillaged: bool) -> (Game, AdvancedAi, u32, Pos) {
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
    game.barb_camps.clear();
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
        unit: crate::name!("settler"),
    });
    city.districts.insert(crate::name!("holy_site"), site);
    let tile = game.map.tiles.get_mut(&site).unwrap();
    tile.district = Some(crate::name!("holy_site"));
    tile.pillaged = pillaged;
    game.players[0].techs.insert(crate::name!("astrology"));
    game.current = 0;
    game.turn = 20;
    let mut ai = AdvancedAi::targeting(target);
    ai.enable_religion_seat_guards_the_race();
    (game, ai, capital, site)
}

fn prayers() -> Item {
    Item::Project {
        project: crate::name!("holy_site_prayers"),
    }
}

fn is_repair(item: Option<&Item>) -> bool {
    matches!(item, Some(Item::Repair { .. }))
}

#[test]
fn a_pillaged_holy_site_goes_ahead_of_a_settler() {
    let (mut g, ai, capital, _) = seat_board(VictoryTarget::Religion, true);
    let plan = ai.assess(&g, 0);
    ai.claim_religion_seat_guards_the_race(&mut g, 0, &plan, &BTreeMap::new());
    assert!(is_repair(g.cities[&capital].queue.first()));
}

#[test]
fn nothing_changes_off_or_for_another_target() {
    let (mut off, _, capital, _) = seat_board(VictoryTarget::Religion, true);
    let stock = AdvancedAi::targeting(VictoryTarget::Religion);
    let plan = stock.assess(&off, 0);
    assert!(stock.race_guard_heads(&off, 0).is_empty());
    stock.claim_religion_seat_guards_the_race(&mut off, 0, &plan, &BTreeMap::new());
    assert!(!is_repair(off.cities[&capital].queue.first()));

    let (mut science, ai, capital, _) = seat_board(VictoryTarget::Science, true);
    let plan = ai.assess(&science, 0);
    ai.claim_religion_seat_guards_the_race(&mut science, 0, &plan, &BTreeMap::new());
    assert!(!is_repair(science.cities[&capital].queue.first()));
}

#[test]
fn a_hostile_soldier_beside_the_site_holds_the_repair() {
    let (mut g, ai, capital, site) = seat_board(VictoryTarget::Religion, true);
    let barbarian = g
        .players
        .iter()
        .position(|player| player.is_barbarian)
        .expect("fixture: the barbarian seat");
    let beside = g
        .nbrs(site)
        .into_iter()
        .find(|pos| {
            g.map.get(*pos).is_some_and(|tile| {
                !g.rules.is_water(tile) && g.rules.is_passable(tile) && tile.owner_city.is_none()
            }) && *pos != g.cities[&capital].pos
        })
        .or_else(|| {
            g.nbrs(site).into_iter().find(|pos| {
                g.map
                    .get(*pos)
                    .is_some_and(|tile| !g.rules.is_water(tile) && g.rules.is_passable(tile))
                    && *pos != g.cities[&capital].pos
            })
        })
        .expect("fixture: a dry tile beside the site");
    g.spawn_unit("warrior", barbarian, beside);
    let plan = ai.assess(&g, 0);
    assert!(ai.race_guard_repair(&g, 0, capital, &plan).is_none());
}

#[test]
fn another_pillaged_district_is_left_to_the_routine_claim() {
    let (mut g, ai, capital, site) = seat_board(VictoryTarget::Religion, true);
    g.cities.get_mut(&capital).unwrap().districts.clear();
    let city = g.cities.get_mut(&capital).unwrap();
    city.districts.insert(crate::name!("campus"), site);
    g.map.tiles.get_mut(&site).unwrap().district = Some(crate::name!("campus"));
    let plan = ai.assess(&g, 0);
    assert!(ai.race_guard_repair(&g, 0, capital, &plan).is_none());
}

/// Live t14: Prayers needed 2 more and the city made 5, and an Archer took
/// the queue.
#[test]
fn prayers_finishing_this_turn_take_the_head_back() {
    let (mut g, ai, capital, _) = seat_board(VictoryTarget::Religion, false);
    let item = prayers();
    assert!(
        g.can_produce(0, capital, &item),
        "fixture: Prayers on offer"
    );
    let cost = g.item_cost_for_city(0, capital, &item);
    let key = Game::item_progress_key(&item);
    g.cities
        .get_mut(&capital)
        .unwrap()
        .production_progress
        .insert(key, cost - 0.5);
    let heads = BTreeMap::from([(capital, item.clone())]);
    let plan = ai.assess(&g, 0);
    ai.claim_religion_seat_guards_the_race(&mut g, 0, &plan, &heads);
    assert_eq!(g.cities[&capital].queue.first(), Some(&item));
}

#[test]
fn prayers_with_turns_left_stay_displaced() {
    let (mut g, ai, capital, _) = seat_board(VictoryTarget::Religion, false);
    let item = prayers();
    let heads = BTreeMap::from([(capital, item.clone())]);
    let plan = ai.assess(&g, 0);
    ai.claim_religion_seat_guards_the_race(&mut g, 0, &plan, &heads);
    assert_ne!(g.cities[&capital].queue.first(), Some(&item));
}
