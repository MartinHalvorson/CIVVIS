use super::*;
use crate::game::{DealItems, Item};

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
