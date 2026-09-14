use super::*;
use crate::ai::advanced::{genes, GrandStrategy};
use crate::game::Action;
use std::sync::Arc;

fn board() -> (Game, u32, u32) {
    let mut g = Game::new(2, 24, 16, 910_3447, 250, 0);
    for pid in 0..2 {
        let pos = g.units[&g.player_unit_ids(pid)[0]].pos;
        g.found_city_for(pid, pos, None);
    }
    g.turn = 60;
    g.current = 0;
    g.record_contact(0, 1);
    let origin = g.player_city_ids(0)[0];
    let dest = g.player_city_ids(1)[0];
    g.players[0].gold = 500.0;
    g.players[0].gold_per_turn = 5.0;
    let city = g.cities.get_mut(&origin).unwrap();
    city.pop = 3;
    city.loyalty = 100.0;
    city.districts.insert(crate::name!("campus"), city.pos);
    let housing = g.city_housing(&g.cities[&origin]);
    Arc::make_mut(&mut g.observed_city_housing_adjustments).insert(origin, 8.0 - housing);
    let food = g.city_yields(origin).food;
    Arc::make_mut(&mut g.observed_city_yield_adjustments).insert(
        origin,
        Yields {
            food: 7.0 - food,
            ..Yields::default()
        },
    );
    g.cities.get_mut(&origin).unwrap().food = g.growth_cost(3) - 18.0;
    assert!(g.city_amenity_surplus(&g.cities[&origin]) >= 0);
    (g, origin, dest)
}

fn growth_ai() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_trade_growth_to_district();
    ai
}

#[test]
fn genes_are_independent_opt_ins_with_reversible_toggles() {
    let mut ai = AdvancedAi::new();
    for tag in ["trade-growth-to-district", "builder-charge-window"] {
        let gene = genes::GENES.iter().find(|gene| gene.tag == tag).unwrap();
        assert!(matches!(gene.kind, genes::Kind::OptIn));
        assert!(!ai.trade_growth_to_district && !ai.builder_charge_window);
        (gene.enable)(&mut ai);
        assert_eq!(
            ai.trade_growth_to_district,
            tag == "trade-growth-to-district"
        );
        assert_eq!(ai.builder_charge_window, tag == "builder-charge-window");
        (gene.disable)(&mut ai);
    }
    assert!(!ai.trade_growth_to_district && !ai.builder_charge_window);
}

#[test]
fn route_food_can_beat_gold_when_it_releases_a_district_slot() {
    let (mut g, origin, dest) = board();
    let stock = AdvancedAi::new();
    let ai = growth_ai();
    let score = |g: &Game, ai: &AdvancedAi| {
        ai.trade_route_destination_value_from(
            g,
            0,
            Some(origin),
            &g.cities[&dest],
            GrandStrategy::Science,
        )
    };
    // Observed options are the host authority; even a foreign route can feed
    // this origin. This exercises the real valuation entry point.
    Arc::make_mut(&mut g.observed_route_options).insert(
        (origin, dest),
        Yields {
            gold: 12.0,
            ..Yields::default()
        },
    );
    let gold_score = score(&g, &stock);
    assert_eq!(score(&g, &ai), gold_score);
    Arc::make_mut(&mut g.observed_route_options).insert(
        (origin, dest),
        Yields {
            food: 3.0,
            production: 1.0,
            ..Yields::default()
        },
    );
    assert!(score(&g, &stock) < gold_score);
    assert!(score(&g, &ai) > gold_score);
    // Once the citizen arrives the same route has only its ordinary yields.
    g.cities.get_mut(&origin).unwrap().pop = 4;
    assert_eq!(score(&g, &ai), score(&g, &stock));
}

#[test]
fn growth_premium_requires_a_reachable_useful_slot_and_solvent_origin() {
    let (g, origin, _) = board();
    let ai = growth_ai();
    let food = Yields {
        food: 3.0,
        ..Yields::default()
    };
    let premium = |g: &Game| ai.trade_growth_to_district_premium(g, 0, Some(origin), food);
    assert!(premium(&g) > 0.0);
    assert_eq!(ai.trade_growth_to_district_premium(&g, 0, None, food), 0.0);
    let mut spare = g.clone();
    spare.cities.get_mut(&origin).unwrap().districts.clear();
    assert_eq!(premium(&spare), 0.0);
    let mut crowded = g.clone();
    Arc::make_mut(&mut crowded.observed_city_housing_adjustments).insert(origin, -10.0);
    assert_eq!(premium(&crowded), 0.0);
    let mut insolvent = g.clone();
    insolvent.players[0].gold = 0.0;
    insolvent.players[0].gold_per_turn = -5.0;
    assert_eq!(premium(&insolvent), 0.0);
    let mut late = g.clone();
    late.turn = late.max_turns - 1;
    assert_eq!(premium(&late), 0.0);
    let mut distant = g.clone();
    distant.cities.get_mut(&origin).unwrap().food = -1000.0;
    assert_eq!(premium(&distant), 0.0);
}

fn builder_board() -> (Game, u32) {
    let (mut g, cid, _) = board();
    g.players[0].civ = "Rome".to_string();
    g.players[0].government = Some("chiefdom".to_string());
    g.players[0].civics.extend([
        crate::name!("political_philosophy"),
        crate::name!("feudalism"),
        crate::name!("the_enlightenment"),
    ]);
    g.apply(
        0,
        &Action::SlotPolicy {
            policy: crate::name!("rationalism"),
        },
    )
    .unwrap();
    let item = Item::Unit {
        unit: crate::name!("builder"),
    };
    let cost = g.item_cost_for_city(0, cid, &item);
    let city = g.cities.get_mut(&cid).unwrap();
    city.queue = vec![item];
    city.production = cost - 1.0;
    (g, cid)
}

#[test]
fn builder_completion_displaces_a_protected_science_card_and_gains_charges() {
    let (mut g, cid) = builder_board();
    let stock = AdvancedAi::new();
    let mut off = g.clone();
    stock.strategic_policies(&mut off, 0, GrandStrategy::Science);
    assert!(!off.has_policy(0, "serfdom"));
    let mut ai = AdvancedAi::new();
    ai.enable_builder_charge_window();
    assert_eq!(ai.builder_charge_window_card(&g, 0), Some("serfdom"));
    ai.strategic_policies(&mut g, 0, GrandStrategy::Science);
    assert!(g.has_policy(0, "serfdom"));
    assert!(!g.has_policy(0, "rationalism"));
    assert_eq!(g.builder_charges(0), off.builder_charges(0) + 2);
    // Reassessment during the window must keep the card even though an
    // already slotted card is absent from available_policies().
    ai.strategic_policies(&mut g, 0, GrandStrategy::Science);
    assert!(g.has_policy(0, "serfdom"));
    let before = g.player_unit_ids(0);
    for _ in 0..4 {
        let pid = g.current;
        g.apply(pid, &Action::EndTurn).unwrap();
    }
    let born = g
        .player_unit_ids(0)
        .into_iter()
        .find(|uid| !before.contains(uid) && g.units[uid].kind == "builder")
        .expect("the queued Builder actually completed");
    assert!(g.units[&born].charges >= off.builder_charges(0) + 2);
    g.cities.get_mut(&cid).unwrap().queue.clear();
    assert_eq!(ai.builder_charge_window_card(&g, 0), None);
    g.current = 0;
    ai.strategic_policies(&mut g, 0, GrandStrategy::Science);
    assert!(!g.has_policy(0, "serfdom"));
    assert!(g.has_policy(0, "rationalism"));
}

#[test]
fn builder_window_respects_unlocks_host_vetoes_and_existing_settler_wave() {
    let (g, cid) = builder_board();
    let mut ai = AdvancedAi::new();
    ai.enable_builder_charge_window();
    let mut locked = g.clone();
    locked.players[0].civics.remove(&crate::name!("feudalism"));
    assert_eq!(ai.builder_charge_window_card(&locked, 0), None);
    let mut vetoed = g.clone();
    Arc::make_mut(&mut vetoed.blocked_policies).insert(crate::name!("serfdom"));
    assert_eq!(ai.builder_charge_window_card(&vetoed, 0), None);
    let mut distant = g.clone();
    distant.cities.get_mut(&cid).unwrap().production = -10_000.0;
    assert_eq!(ai.builder_charge_window_card(&distant, 0), None);
    let mut settlers = g.clone();
    settlers.cities.get_mut(&cid).unwrap().queue = vec![Item::Unit {
        unit: crate::name!("settler"),
    }];
    assert!(!ai.builder_window_can_replace(&settlers, 0, &crate::name!("colonization")));
    assert!(!ai.builder_window_can_replace(&g, 0, &crate::name!("liberalism")));
    assert!(!ai.builder_window_can_replace(&g, 0, &crate::name!("conscription")));
}

#[test]
fn builder_window_cannot_take_an_unwanted_protected_policy_slot() {
    let (mut g, _) = builder_board();
    g.players[0].policies = [crate::name!("liberalism")].into_iter().collect();
    // Model a host menu offering only Serfdom. Liberalism is outside the
    // static Science portfolio, but the Builder window still protects it.
    let blocked = g
        .rules
        .policies
        .keys()
        .copied()
        .filter(|name| *name != crate::name!("serfdom"))
        .collect();
    *Arc::make_mut(&mut g.blocked_policies) = blocked;
    let mut stock = g.clone();
    AdvancedAi::new().strategic_policies(&mut stock, 0, GrandStrategy::Science);
    assert!(stock.has_policy(0, "liberalism"));
    assert!(!stock.has_policy(0, "serfdom"));
    let mut ai = AdvancedAi::new();
    ai.enable_builder_charge_window();
    assert_eq!(ai.builder_charge_window_card(&g, 0), Some("serfdom"));
    ai.strategic_policies(&mut g, 0, GrandStrategy::Science);
    assert!(g.has_policy(0, "liberalism"));
    assert!(!g.has_policy(0, "serfdom"));
}
