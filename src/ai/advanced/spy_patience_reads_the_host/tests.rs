use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use super::*;
use crate::game::HostUnitFacts;

fn facts(operation: Option<&str>, menu: bool) -> HostUnitFacts {
    HostUnitFacts {
        spy_operation: operation.map(str::to_string),
        spy_missions: menu.then(|| BTreeSet::from(["gain_sources".to_string()])),
        ..HostUnitFacts::default()
    }
}

#[test]
fn a_spy_the_host_reports_idle_spends_its_patience_on_a_later_turn() {
    let mut g = Game::new(2, 16, 12, 7, 250, 0);
    g.turn = 40;
    let mut ai = AdvancedAi::new();
    // Spy 7: ordered on turn 39, idle now. Spy 8: ordered on turn 39, its
    // operation still running. Spy 9: ordered this very turn, idle (the host
    // has not applied it yet). Spy 10: no host facts at all.
    for spy in [7, 8, 9, 10] {
        ai.spy_orders_until.insert(spy, 44);
    }
    ai.spy_order_turns = BTreeMap::from([(7, 39), (8, 39), (9, 40), (10, 39)]);
    let host = Arc::make_mut(&mut g.host_unit_facts);
    host.insert(7, facts(None, true));
    host.insert(8, facts(Some("steal_tech_boost"), false));
    host.insert(9, facts(None, true));

    ai.release_spent_spy_patience(&g);
    assert!(
        !ai.spy_orders_until.contains_key(&7),
        "idle on a later turn: spent"
    );
    assert!(ai.spy_orders_until.contains_key(&8), "still running: kept");
    assert!(
        ai.spy_orders_until.contains_key(&9),
        "ordered this turn: kept"
    );
    assert!(ai.spy_orders_until.contains_key(&10), "no host facts: kept");
}

#[test]
fn the_turn_of_each_new_or_changed_order_is_remembered() {
    let mut g = Game::new(2, 16, 12, 7, 250, 0);
    g.turn = 50;
    let mut ai = AdvancedAi::new();
    ai.spy_orders_until = BTreeMap::from([(7, 52), (8, 55)]);
    ai.spy_order_turns = BTreeMap::from([(7, 47), (8, 48), (9, 48)]);
    let before = ai.spy_orders_until.clone();
    // This pass re-orders spy 8 and gives spy 11 its first order.
    ai.spy_orders_until.insert(8, 57);
    ai.spy_orders_until.insert(11, 55);
    ai.note_spy_order_turns(&g, &before);
    assert_eq!(
        ai.spy_order_turns.get(&7),
        Some(&47),
        "unchanged order keeps its turn"
    );
    assert_eq!(ai.spy_order_turns.get(&8), Some(&50));
    assert_eq!(ai.spy_order_turns.get(&11), Some(&50));
    assert!(
        !ai.spy_order_turns.contains_key(&9),
        "no patience left: forgotten"
    );
}
