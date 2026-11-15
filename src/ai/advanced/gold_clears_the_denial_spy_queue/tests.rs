use super::*;
use crate::ai::advanced::test_support::opt_in_off_in_both_controllers;
use crate::ai::advanced::VictoryTarget;

/// Seat 0 with three idle cities (pop 2, 5, 8) and the Diplomatic Service
/// and Nationalism civics (Spy capacity 2); seat 1 a met rival with one
/// city, which has landed the Satellite and the Moon from a Spaceport we
/// have seen (a decisive racer). The same board as
/// `denial_spies_fill_every_slot`'s tests.
fn board() -> (Game, Vec<u32>) {
    let mut g = Game::new_full(2, 32, 22, 79_208, 250, 0, false);
    for pid in 0..2 {
        let settler = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .expect("each fixture major begins with a Settler");
        let position = g.units[&settler].pos;
        g.found_city_for(pid, position, None);
        for uid in g.player_unit_ids(pid) {
            g.remove_unit(uid);
        }
    }
    let home = g.cities[&g.player_city_ids(0)[0]].pos;
    for _ in 0..2 {
        let site = g
            .map
            .tiles
            .keys()
            .copied()
            .filter(|pos| {
                let tile = &g.map.tiles[pos];
                tile.owner_city.is_none()
                    && !g.rules.is_water(tile)
                    && g.cities.values().all(|city| g.wdist(city.pos, *pos) >= 5)
            })
            .min_by_key(|pos| (g.wdist(home, *pos), *pos))
            .expect("the fixture map has room for another city");
        g.found_city_for(0, site, None);
    }
    for civic in ["diplomatic_service", "nationalism"] {
        g.players[0].civics.insert(crate::name::Name::new(civic));
    }
    let ours = g.player_city_ids(0);
    for (i, cid) in ours.iter().enumerate() {
        let city = g.cities.get_mut(cid).unwrap();
        city.queue.clear();
        city.production = 0.0;
        city.pop = 2 + 3 * i as i32;
    }
    g.current = 0;
    g.turn = 120;
    g.at_war.clear();
    g.record_contact(0, 1);
    for project in ["launch_earth_satellite", "launch_moon_landing"] {
        g.players[1].science_projects.insert(project.to_string());
    }
    let rival = g.player_city_ids(1)[0];
    let centre = g.cities[&rival].pos;
    let pad = g
        .wdisk(centre, 1)
        .into_iter()
        .find(|pos| {
            *pos != centre
                && g.map
                    .get(*pos)
                    .is_some_and(|tile| tile.district.is_none() && !g.rules.is_water(tile))
        })
        .expect("the rival city has a free land neighbour");
    let tile = g.map.tiles.get_mut(&pad).unwrap();
    tile.owner_city = Some(rival);
    tile.district = Some(crate::name!("spaceport"));
    tile.improvement = None;
    g.cities
        .get_mut(&rival)
        .unwrap()
        .districts
        .insert(crate::name!("spaceport"), pad);
    g.players[0].explored.insert(pad);
    (g, ours)
}

fn seat(target: VictoryTarget, gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::targeting(target);
    ai.enable_science_denial_trains_spies();
    ai.enable_denial_spies_fill_every_slot();
    if gene {
        ai.enable_gold_clears_the_denial_spy_queue();
    }
    ai
}

fn plan() -> StrategicPlan {
    StrategicPlan {
        strategy: GrandStrategy::Culture,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 4,
        assessed_turn: 0,
        rush: false,
    }
}

fn spy() -> Item {
    Item::Unit {
        unit: crate::name!("spy"),
    }
}

fn warrior() -> Item {
    Item::Unit {
        unit: crate::name!("warrior"),
    }
}

fn training(g: &Game) -> BTreeSet<u32> {
    g.player_city_ids(0)
        .into_iter()
        .filter(|cid| g.cities[cid].queue.first() == Some(&spy()))
        .collect()
}

/// Seat 0's cities by Production, best first (the lower id on a tie).
fn by_production(g: &Game, among: &[u32]) -> Vec<u32> {
    let mut ranked: Vec<(f64, u32)> = among
        .iter()
        .map(|cid| (g.city_yields(*cid).production, *cid))
        .collect();
    ranked.sort_by(|(l, lc), (r, rc)| r.total_cmp(l).then(lc.cmp(rc)));
    ranked.into_iter().map(|(_, cid)| cid).collect()
}

/// The board with a Warrior (which no Spy claim displaces) at the head of
/// the best city, and Gold `above` the purchase reserve plus its price.
fn blocked_board(above: f64) -> (Game, Vec<u32>, f64) {
    let (mut g, ours) = board();
    let ranked = by_production(&g, &ours);
    g.cities.get_mut(&ranked[0]).unwrap().queue = vec![warrior()];
    let ai = seat(VictoryTarget::Culture, true);
    let floor = ai.purchase_upkeep_floor(&g, 0, 0.0);
    let price = g
        .unit_purchase_cost(0, ranked[0], "warrior", "gold")
        .expect("a Warrior is for sale");
    g.players[0].gold = floor + price + above;
    (g, ranked, price)
}

#[test]
fn gold_clears_the_denial_spy_queue_is_opt_in() {
    opt_in_off_in_both_controllers("gold-clears-the-denial-spy-queue", |ai| {
        ai.gold_clears_the_denial_spy_queue
    });
}

#[test]
fn the_fixture_is_what_the_tests_assume() {
    let (g, ranked, _) = blocked_board(50.0);
    assert_eq!(g.spy_capacity(0), 2, "two slots");
    let ai = seat(VictoryTarget::Culture, true);
    assert!(!ai.denial_spy_racers(&g, 0).is_empty(), "a decisive racer");
    let turns = |cid| AdvancedAi::denial_spy_turns(&g, 0, cid).unwrap();
    assert!(
        turns(ranked[0]) + SPY_QUEUE_MIN_TURNS_SAVED <= turns(ranked[1]),
        "the best city saves real time: {} against {}",
        turns(ranked[0]),
        turns(ranked[1])
    );
}

#[test]
fn off_is_an_exact_no_op() {
    let (g, _, _) = blocked_board(50.0);
    let mut off = g.clone();
    let ai = seat(VictoryTarget::Culture, false);
    assert!(ai.denial_spy_queue_purchase(&off, 0, &plan()).is_none());
    assert!(!ai.clear_queue_for_denial_spy(&mut off, 0, &plan()));
    assert_eq!(off.players[0].gold, g.players[0].gold, "no Gold spent");
    for cid in g.player_city_ids(0) {
        assert_eq!(
            off.cities[&cid].queue, g.cities[&cid].queue,
            "no queue moved"
        );
    }
    assert_eq!(off.units.len(), g.units.len(), "nothing bought");
}

#[test]
fn a_free_slot_buys_the_best_citys_head_and_trains_the_spy_there() {
    let (mut g, ranked, price) = blocked_board(50.0);
    // Without the gene the claim passes the blocked best city by.
    let mut without = g.clone();
    seat(VictoryTarget::Culture, false).claim_queues_for_denial_spies(&mut without, 0, &plan());
    assert!(
        !training(&without).contains(&ranked[0]),
        "the claim does not displace the Warrior"
    );
    let ai = seat(VictoryTarget::Culture, true);
    let buy = ai
        .denial_spy_queue_purchase(&g, 0, &plan())
        .expect("a purchase");
    assert_eq!(buy.city, ranked[0]);
    assert_eq!(buy.head, warrior());
    assert_eq!((buy.held, buy.queued, buy.slots), (0, 0, 2));
    let gold = g.players[0].gold;
    let warriors = |g: &Game| {
        g.player_unit_ids(0)
            .into_iter()
            .filter(|uid| g.units[uid].kind == "warrior")
            .count()
    };
    assert_eq!(warriors(&g), 0);
    assert!(ai.clear_queue_for_denial_spy(&mut g, 0, &plan()));
    assert!(
        (gold - g.players[0].gold - price).abs() < 1e-6,
        "paid the price"
    );
    assert_eq!(warriors(&g), 1, "the Warrior is built, not delayed");
    assert_eq!(
        training(&g),
        BTreeSet::from([ranked[0]]),
        "the Spy trains there"
    );
    // The slot is filled now: nothing more to buy this way.
    assert!(ai.denial_spy_queue_purchase(&g, 0, &plan()).is_none());
}

#[test]
fn the_reserve_is_never_spent() {
    let (g, _, _) = blocked_board(-1.0);
    let mut on = g.clone();
    let ai = seat(VictoryTarget::Culture, true);
    assert!(ai.denial_spy_queue_purchase(&on, 0, &plan()).is_none());
    assert!(!ai.clear_queue_for_denial_spy(&mut on, 0, &plan()));
    assert_eq!(on.players[0].gold, g.players[0].gold);
}

#[test]
fn no_purchase_when_every_slot_is_held_or_training() {
    let (g, ranked, _) = blocked_board(50.0);
    let ai = seat(VictoryTarget::Culture, true);
    // Both slots in training, in the two weaker cities.
    let mut training_both = g.clone();
    for cid in &ranked[1..] {
        training_both.cities.get_mut(cid).unwrap().queue = vec![spy()];
    }
    assert!(ai
        .denial_spy_queue_purchase(&training_both, 0, &plan())
        .is_none());
    // One held, one training.
    let mut held = g.clone();
    held.cities.get_mut(&ranked[2]).unwrap().queue = vec![spy()];
    let home = held.cities[&ranked[2]].pos;
    held.spawn_unit("spy", 0, home);
    assert_eq!(held.spy_agents(0), 1);
    assert!(ai.denial_spy_queue_purchase(&held, 0, &plan()).is_none());
    // One slot still free: the purchase stands.
    let mut one_free = g.clone();
    one_free.cities.get_mut(&ranked[2]).unwrap().queue = vec![spy()];
    let buy = ai
        .denial_spy_queue_purchase(&one_free, 0, &plan())
        .expect("one slot free");
    assert_eq!((buy.city, buy.queued), (ranked[0], 1));
}

#[test]
fn no_purchase_without_a_racer_or_when_the_open_city_is_as_good() {
    let ai = seat(VictoryTarget::Culture, true);
    // No rival on the denial board.
    let (mut g, _, _) = blocked_board(50.0);
    g.players[1].science_projects.clear();
    for cid in g.player_city_ids(1) {
        g.cities.get_mut(&cid).unwrap().districts.clear();
    }
    assert!(ai.denial_spy_racers(&g, 0).is_empty());
    assert!(ai.denial_spy_queue_purchase(&g, 0, &plan()).is_none());
    // The blocked city is not the best one: the claim takes the best itself.
    let (mut g, ranked, _) = blocked_board(50.0);
    g.cities.get_mut(&ranked[0]).unwrap().queue.clear();
    g.cities.get_mut(&ranked[2]).unwrap().queue = vec![warrior()];
    assert!(ai.denial_spy_queue_purchase(&g, 0, &plan()).is_none());
    // A head mostly paid finishes on its own.
    let (mut g, ranked, _) = blocked_board(50.0);
    let cost = g.item_cost_for_city(0, ranked[0], &warrior());
    g.cities.get_mut(&ranked[0]).unwrap().production = 0.5 * cost;
    assert!(ai.denial_spy_queue_purchase(&g, 0, &plan()).is_none());
    // A Science seat keeps the old claim.
    let (g, _, _) = blocked_board(50.0);
    assert!(seat(VictoryTarget::Science, true)
        .denial_spy_queue_purchase(&g, 0, &plan())
        .is_none());
}
