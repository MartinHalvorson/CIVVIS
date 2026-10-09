use super::*;

/// One major with two cities 4-15 tiles apart (a domestic route), Foreign
/// Trade known (one route slot), every unit removed, the capital's queue
/// cleared and the turn past the opening. Returns the game and the capital.
fn two_city_trade_empire() -> Option<(Game, u32)> {
    let mut game = Game::new_full(1, 32, 20, 936_221, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")?;
    game.apply(0, &Action::FoundCity { unit: settler }).ok()?;
    let capital = game.player_city_ids(0)[0];
    let home = game.cities[&capital].pos;
    let mut sites: Vec<Pos> = game
        .map
        .tiles
        .iter()
        .filter(|(pos, tile)| {
            (5..=10).contains(&game.wdist(**pos, home))
                && tile.owner_city.is_none()
                && game.rules.is_passable(tile)
                && !game.rules.is_water(tile)
        })
        .map(|(pos, _)| *pos)
        .collect();
    sites.sort();
    let second = game.spawn_test_unit("settler", 0, *sites.first()?);
    game.current = 0;
    game.apply(0, &Action::FoundCity { unit: second }).ok()?;
    for uid in game.player_unit_ids(0) {
        game.remove_unit(uid);
    }
    game.players[0].civics.insert(crate::name!("foreign_trade"));
    game.turn = 60;
    for cid in game.player_city_ids(0) {
        game.cities.get_mut(&cid).unwrap().queue.clear();
    }
    let trader = trader();
    (game.trade_capacity(0) > 0 && game.can_produce(0, capital, &trader)).then_some((game, capital))
}

fn armed() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_trader_fills_the_idle_route();
    ai
}

fn trader() -> Item {
    Item::Unit {
        unit: crate::name!("trader"),
    }
}

fn unit(name: &str) -> Item {
    Item::Unit {
        unit: Name::new(name),
    }
}

fn heads(game: &Game) -> Vec<Option<Item>> {
    game.player_city_ids(0)
        .into_iter()
        .map(|cid| game.cities[&cid].queue.first().cloned())
        .collect()
}

fn set_all(game: &mut Game, item: Item) {
    for cid in game.player_city_ids(0) {
        game.cities.get_mut(&cid).unwrap().queue = vec![item.clone()];
    }
}

/// The blocked case: every city training a routine Warrior at peace with a
/// route slot empty. On, exactly one city puts a Trader first; off, nothing
/// moves; with the slot filled by a Trader in training, nothing moves either.
#[test]
fn an_empty_slot_claims_one_trader_over_a_routine_unit() {
    let Some((mut game, _capital)) = two_city_trade_empire() else {
        panic!("fixture: two cities and a trade slot");
    };
    set_all(&mut game, unit("warrior"));
    assert!(AdvancedAi::empty_trade_slots(&game, 0, None) > 0, "fixture: a slot stands empty");

    let stock = AdvancedAi::new();
    let mut off = game.clone();
    let plan = stock.assess(&off, 0);
    stock.claim_trader_for_idle_route(&mut off, 0, &plan);
    assert!(heads(&off).iter().all(|head| *head == Some(unit("warrior"))), "off: no claim");

    let ai = armed();
    let mut on = game.clone();
    let plan = ai.assess(&on, 0);
    ai.claim_trader_for_idle_route(&mut on, 0, &plan);
    let traders = heads(&on).iter().filter(|head| **head == Some(trader())).count();
    assert_eq!(traders, 1, "one Trader for one empty slot");
    assert_eq!(AdvancedAi::empty_trade_slots(&on, 0, None), 0, "the slot is answered");

    // A second pass claims nothing more: the queued Trader answers the slot.
    let plan = ai.assess(&on, 0);
    ai.claim_trader_for_idle_route(&mut on, 0, &plan);
    let again = heads(&on).iter().filter(|head| **head == Some(trader())).count();
    assert_eq!(again, 1, "no second Trader for the same slot");

    // The claimed Trader holds against the governor's rescoring while its
    // slot is empty, and a Trader afield closes the slot.
    let claimed = on
        .player_city_ids(0)
        .into_iter()
        .find(|cid| on.cities[cid].queue.first() == Some(&trader()))
        .unwrap();
    let plan = ai.assess(&on, 0);
    assert!(ai.idle_route_trader_holds(&on, 0, claimed, &plan, &trader()));
    let mut afield = game.clone();
    let home = afield.cities[&claimed].pos;
    afield.spawn_test_unit("trader", 0, home);
    assert_eq!(AdvancedAi::empty_trade_slots(&afield, 0, None), 0, "a Trader afield fills it");
    let plan = ai.assess(&afield, 0);
    ai.claim_trader_for_idle_route(&mut afield, 0, &plan);
    assert!(heads(&afield).iter().all(|head| *head == Some(unit("warrior"))), "no claim");
}

/// A Settler, a Builder, Walls and a freshly attacked city keep their queues.
#[test]
fn settlers_builders_walls_and_attacked_cities_keep_their_queues() {
    let Some((game, _capital)) = two_city_trade_empire() else {
        panic!("fixture: two cities and a trade slot");
    };
    let ai = armed();
    for keep in [unit("settler"), unit("builder")] {
        let mut busy = game.clone();
        set_all(&mut busy, keep.clone());
        let plan = ai.assess(&busy, 0);
        ai.claim_trader_for_idle_route(&mut busy, 0, &plan);
        assert!(heads(&busy).iter().all(|head| *head == Some(keep.clone())), "{keep:?} stays");
    }
    let walls = Item::Building {
        building: crate::name!("walls"),
    };
    let mut walled = game.clone();
    set_all(&mut walled, walls.clone());
    if walled
        .player_city_ids(0)
        .into_iter()
        .all(|cid| walled.can_produce(0, cid, &walls))
    {
        let plan = ai.assess(&walled, 0);
        ai.claim_trader_for_idle_route(&mut walled, 0, &plan);
        assert!(heads(&walled).iter().all(|head| *head == Some(walls.clone())), "Walls stay");
    }
    let mut attacked = game.clone();
    set_all(&mut attacked, unit("warrior"));
    let turn = attacked.turn;
    for cid in attacked.player_city_ids(0) {
        attacked.cities.get_mut(&cid).unwrap().last_attacked = turn;
    }
    let plan = ai.assess(&attacked, 0);
    ai.claim_trader_for_idle_route(&mut attacked, 0, &plan);
    assert!(heads(&attacked).iter().all(|head| *head == Some(unit("warrior"))), "attacked: no claim");
}

/// The gene is registered and off by default.
#[test]
fn the_gene_is_registered_and_off_by_default() {
    let gene = crate::ai::advanced::gene("trader-fills-the-idle-route").expect("registered");
    assert_eq!(gene.field, "trader_fills_the_idle_route");
    assert!(!AdvancedAi::new().trader_fills_the_idle_route);
    assert!(armed().trader_fills_the_idle_route);
}
