use super::*;
use crate::game::DealItems;

fn battle() -> (Game, u32, u32, u32) {
    let mut g = Game::new_full(3, 28, 18, 41, 30, 0, false);
    for tile in g.map.tiles.values_mut() {
        tile.terrain = "grassland".into();
        tile.feature = None;
        tile.hills = false;
    }
    let found = g
        .legal_actions(0)
        .into_iter()
        .find(|a| matches!(a, Action::FoundCity { .. }))
        .unwrap();
    g.apply(0, &found).unwrap();
    let city = g.player_city_ids(0)[0];
    g.at_war.insert((0, 1));
    g.record_contact(0, 2);
    let target = g.spawn_unit("warrior", 1, (8, 6));
    g.units.get_mut(&target).unwrap().hp = 1000;
    let gun = g.spawn_unit("archer", 0, (7, 6));
    (g, city, target, gun)
}

fn unaffordable_trade(open_borders: bool) -> Action {
    Action::Trade {
        player: 2,
        offer: Box::new(DealItems {
            gold: 1_000_000.0,
            open_borders,
            ..Default::default()
        }),
        request: Box::default(),
    }
}

fn batch(g: &mut Game, rejected: Action, gun: u32, target: u32) {
    let shot = Action::Ranged {
        unit: gun,
        target: g.units[&target].pos,
    };
    assert!(
        execute_frame(g, 0, &Default::default(), [(0, rejected), (0, shot)].iter()),
        "a rejected hypothesis still requests a fresh observation"
    );
    assert_eq!(g.players[0].counters["player:refused"], 1);
}

#[test]
fn rejected_production_does_not_discard_the_armys_shot() {
    let (mut g, city, target, gun) = battle();
    let queue = g.cities[&city].queue.clone();
    let rejected = Action::Produce {
        city,
        item: Item::Wonder {
            wonder: "temple_artemis".into(),
            pos: g.cities[&city].pos,
        },
    };
    assert_eq!(
        g.clone().apply(0, &rejected).unwrap_err(),
        "cannot produce that"
    );
    batch(&mut g, rejected, gun, target);
    assert!(
        g.units[&target].hp < 1000,
        "rejected production starved the army"
    );
    assert_eq!(g.cities[&city].queue, queue);
}

#[test]
fn rejected_financial_trade_does_not_discard_the_armys_shot() {
    let (mut g, _, target, gun) = battle();
    let gold = g.players[0].gold;
    batch(&mut g, unaffordable_trade(false), gun, target);
    assert!(
        g.units[&target].hp < 1000,
        "rejected trade starved the army"
    );
    assert_eq!(g.players[0].gold, gold);
}

#[test]
fn rejected_movement_still_invalidates_the_tactical_tail() {
    let (mut g, _, target, gun) = battle();
    batch(
        &mut g,
        Action::Move {
            unit: u32::MAX,
            to: (8, 6),
        },
        gun,
        target,
    );
    assert_eq!(g.units[&target].hp, 1000);
    assert_eq!(g.units[&gun].attacks_left, 1);
}

#[test]
fn rejected_territory_access_still_requires_immediate_observation() {
    let (mut g, _, target, gun) = battle();
    batch(&mut g, unaffordable_trade(true), gun, target);
    assert_eq!(g.units[&target].hp, 1000);
}

#[test]
fn rejected_district_placement_still_invalidates_the_tactical_tail() {
    let (mut g, city, target, gun) = battle();
    let rejected = Action::Produce {
        city,
        item: Item::District {
            district: "encampment".into(),
            pos: g.cities[&city].pos,
        },
    };
    assert_eq!(
        g.clone().apply(0, &rejected).unwrap_err(),
        "cannot produce that"
    );
    batch(&mut g, rejected, gun, target);
    assert_eq!(g.units[&target].hp, 1000);
}

/// A production order the authoritative board refuses is remembered for the
/// rest of the turn, and the next frame's view blocks it — so the replanned
/// frame cannot choose it again and leave the city on an empty queue.
#[test]
fn a_refused_production_order_is_blocked_in_the_next_frames_view() {
    let mut game = Game::new_full(2, 24, 16, 41, 20, 0, false);
    let home = game.units[&game.player_unit_ids(0)[0]].pos;
    let city = game.found_city_for(0, home, None);
    // No site and no unlock: the board must refuse this wonder.
    let item = Item::Wonder {
        wonder: crate::name!("pyramids"),
        pos: home,
    };
    assert!(!game.can_produce(0, city, &item));
    let mut refused = Vec::new();
    let refresh = execute_observed_action_recorded(
        &mut game,
        0,
        &Action::Produce {
            city,
            item: item.clone(),
        },
        &mut Vec::new(),
        &mut refused,
        &mut BTreeSet::new(),
    );
    assert_eq!(
        refresh,
        Some(true),
        "an economic refusal keeps the batch going"
    );
    assert_eq!(refused, vec![(city, item.clone())]);
    let mut view = game.player_decision_view(0);
    assert!(view
        .blocked_production
        .get(&city)
        .is_none_or(|blocked| blocked.is_empty()));
    block_refused_production(&mut view, &refused);
    assert!(view.blocked_production[&city].contains(&Game::production_block_key(&item)));
    assert!(
        game.blocked_production.get(&city).is_none(),
        "the authoritative board is never changed"
    );
}

/// The start Settler of seat 0 on a site three tiles from a rival city the
/// seat has never seen: the board refuses a city there and the fog-honest
/// view cannot say why.
fn site_beside_a_hidden_city() -> (Game, u32, crate::Pos) {
    let mut game = Game::new_full(2, 40, 26, 91_170, 200, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .unwrap();
    let site = game.units[&settler].pos;
    let hidden = game
        .map
        .tiles
        .values()
        .find(|tile| {
            game.wdist(site, tile.pos) == 3
                && !game.rules.is_water(tile)
                && game.rules.is_passable(tile)
                && !game.player_can_see(0, tile.pos)
                && game.city_at(tile.pos).is_none()
        })
        .unwrap()
        .pos;
    game.found_city_for(1, hidden, None);
    assert!(!game.player_can_see(0, hidden));
    assert!(!game.can_found_city(settler), "the board refuses the site");
    assert!(
        game.player_decision_view(0).can_found_city(settler),
        "the view cannot see why"
    );
    (game, settler, site)
}

#[test]
fn a_refused_city_site_is_blocked_in_every_later_view() {
    let (mut game, settler, site) = site_beside_a_hidden_city();
    let mut sites = BTreeSet::new();
    let refresh = execute_observed_action_recorded(
        &mut game,
        0,
        &Action::FoundCity { unit: settler },
        &mut Vec::new(),
        &mut Vec::new(),
        &mut sites,
    );
    assert_eq!(refresh, None, "a refused founding still ends the frame");
    assert_eq!(sites, BTreeSet::from([site]));
    assert_eq!(game.players[0].counters["player:refused_city_site"], 1);
    let mut later = game.player_decision_view(0);
    block_refused_city_sites(&mut later, &sites);
    assert!(
        !later.can_found_city(settler),
        "no later frame plans the site again"
    );
    assert!(
        game.blocked_city_sites.is_empty(),
        "the authoritative board is never changed"
    );
}

#[test]
fn a_founding_refused_for_another_reason_blocks_no_site() {
    let mut game = Game::new_full(2, 24, 16, 41, 20, 0, false);
    let warrior = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind != "settler")
        .unwrap();
    assert!(game.can_found_city(warrior), "open ground");
    let mut sites = BTreeSet::new();
    execute_observed_action_recorded(
        &mut game,
        0,
        &Action::FoundCity { unit: warrior },
        &mut Vec::new(),
        &mut Vec::new(),
        &mut sites,
    );
    assert_eq!(game.players[0].counters["player:refused"], 1);
    assert!(
        sites.is_empty(),
        "only a site the board rejects is condemned"
    );
}

#[test]
fn every_frame_of_a_turn_honours_the_seats_refused_sites() {
    use crate::ai::Ai as _;
    let fresh = || {
        let game = Game::new_full(2, 24, 16, 8, 20, 0, false);
        let mut ai = AdvancedAi::new();
        ai.enable_live_bridge_universe();
        assert!(ai.uses_player_observation());
        (game, ai)
    };
    let (mut game, mut ai) = fresh();
    let start = game
        .player_unit_ids(0)
        .into_iter()
        .map(|uid| &game.units[&uid])
        .find(|unit| unit.kind == "settler")
        .unwrap()
        .pos;
    take_turn(&mut ai, &mut game, 0);
    assert_eq!(
        game.city_at(start).map(|cid| game.cities[&cid].owner),
        Some(0),
        "the seat founds where it stands"
    );
    let (mut game, mut ai) = fresh();
    ai.refused_city_sites.insert(start);
    take_turn(&mut ai, &mut game, 0);
    assert!(
        game.city_at(start).is_none(),
        "a refused site is never planned"
    );
    assert!(
        game.players[0].counters.get("player:refused").is_none(),
        "and never ordered"
    );
}

#[test]
fn a_seat_is_never_refused_the_same_city_site_twice() {
    use crate::ai::Ai as _;
    let mut game = Game::new_full(2, 24, 16, 8, 20, 0, false);
    let settler = game
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| game.units[uid].kind == "settler")
        .unwrap();
    let start = game.units[&settler].pos;
    let hidden = game
        .map
        .tiles
        .values()
        .find(|tile| {
            game.wdist(start, tile.pos) == 3
                && !game.rules.is_water(tile)
                && game.rules.is_passable(tile)
                && !game.player_can_see(0, tile.pos)
                && game.city_at(tile.pos).is_none()
        })
        .unwrap()
        .pos;
    game.found_city_for(1, hidden, None);
    let mut ai = AdvancedAi::new();
    ai.enable_live_bridge_universe();
    assert!(ai.uses_player_observation());
    for _ in 0..4 {
        take_turn(&mut ai, &mut game, 0);
        game.apply(1, &Action::EndTurn).unwrap();
    }
    let counters = &game.players[0].counters;
    assert!(
        ai.refused_city_sites.contains(&start),
        "the start was refused and remembered"
    );
    assert_eq!(
        counters.get("player:refused"),
        counters.get("player:refused_city_site"),
        "every refusal was a site not tried before: {counters:?}"
    );
}
