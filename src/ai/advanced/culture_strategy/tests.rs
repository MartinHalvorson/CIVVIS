use super::*;
use crate::ai::advanced::StrategicPlan;
use crate::game::{Action, DealItems};
use std::sync::Arc;

fn board() -> Game {
    let mut g = Game::new_full(3, 30, 20, 79_208, 250, 0, false);
    for pid in 0..3 {
        g.current = pid;
        let unit = g
            .player_unit_ids(pid)
            .into_iter()
            .find(|uid| g.units[uid].kind == "settler")
            .unwrap();
        g.apply(pid, &Action::FoundCity { unit }).unwrap();
        g.players[pid].gold = 1_000.0;
        g.players[pid].gold_per_turn = 10.0;
        g.players[pid].civics.insert(crate::name!("early_empire"));
    }
    g.current = 0;
    g.turn = 60;
    g.at_war.clear();
    g.record_contact(0, 1);
    g.record_contact(0, 2);
    for (pid, domestic, foreign) in [(0, 100, 0), (1, 60, 55), (2, 80, 0)] {
        let stats = Arc::make_mut(&mut g.observed_public_empire_stats)
            .entry(pid)
            .or_default();
        stats.domestic_tourists = Some(domestic);
        stats.foreign_tourists = Some(foreign);
    }
    g
}

#[test]
fn defense_uses_the_global_bar_and_only_known_living_opponents() {
    let mut g = board();
    let ai = AdvancedAi::targeting(VictoryTarget::Science);
    assert_eq!(ai.culture_trade_threats(&g, 0), BTreeSet::from([1]));
    assert_eq!(ai.culture_defense_urgency(&g, 0), 0.55);
    // Raising a third party's domestic count makes 55 visitors harmless;
    // neither our small count nor a rival's score may trigger an alarm.
    Arc::make_mut(&mut g.observed_public_empire_stats)
        .get_mut(&2)
        .unwrap()
        .domestic_tourists = Some(300);
    assert!(ai.culture_trade_threats(&g, 0).is_empty());
    Arc::make_mut(&mut g.observed_public_empire_stats)
        .get_mut(&1)
        .unwrap()
        .foreign_tourists = Some(180);
    assert_eq!(ai.culture_trade_threats(&g, 0), BTreeSet::from([1]));
    assert_eq!(
        ai.culture_defense_urgency(&g, 0),
        0.0,
        "our culture cannot reach that bar"
    );
    g.players[0].met.remove(&1);
    assert!(ai.culture_trade_threats(&g, 0).is_empty());
    g.record_contact(0, 1);
    g.players[1].alive = false;
    assert!(ai.culture_trade_threats(&g, 0).is_empty());
}

#[test]
fn disabled_culture_and_frozen_controller_do_not_raise_new_defenses() {
    let mut g = board();
    assert!(AdvancedAi::legacy().culture_trade_threats(&g, 0).is_empty());
    g.victory_conditions.culture = false;
    let ai = AdvancedAi::new();
    assert!(ai.culture_trade_threats(&g, 0).is_empty());
    assert_eq!(ai.culture_defense_urgency(&g, 0), 0.0);
    assert_eq!(
        ai.culture_route_bonus(&g, 0, 1, GrandStrategy::Culture),
        0.0
    );
}

#[test]
fn defensive_trade_refuses_tourism_sales_but_keeps_income_and_purchases() {
    let threats = BTreeSet::from([1]);
    let mut deal = QuickDeal {
        partner: 1,
        category: "diplomatic".into(),
        item: "open_borders".into(),
        direction: "sell".into(),
        offer: DealItems::default(),
        request: DealItems::default(),
        my_value: 100.0,
        partner_value: 100.0,
    };
    assert!(!AdvancedAi::culture_deal_safe(&deal, &threats));
    deal.direction = "buy".into();
    assert!(AdvancedAi::culture_deal_safe(&deal, &threats));
    deal.direction = "sell".into();
    deal.item = "silk".into();
    deal.category = "luxury".into();
    assert!(AdvancedAi::culture_deal_safe(&deal, &threats));
    deal.category = "great_work".into();
    assert!(!AdvancedAi::culture_deal_safe(&deal, &threats));
    deal.partner = 2;
    assert!(
        !AdvancedAi::culture_deal_safe(&deal, &threats),
        "do not sell away defensive culture to a different buyer"
    );
}

#[test]
fn strategic_trade_does_not_grant_passage_to_the_culture_threat() {
    let mut g = board();
    // We already bought their passage; the dangerous quote sells OURS.
    g.players[1].open_borders_until.insert(0, 100);
    g.players[2].alive = false;
    let mut baseline = g.clone();
    AdvancedAi::legacy().strategic_bilateral_trade(&mut baseline, 0, None, GrandStrategy::Science);
    assert!(
        baseline.has_open_borders(1, 0),
        "fixture offers a profitable passage sale"
    );
    AdvancedAi::targeting(VictoryTarget::Science).strategic_bilateral_trade(
        &mut g,
        0,
        None,
        GrandStrategy::Science,
    );
    assert!(!g.has_open_borders(1, 0));
}

#[test]
fn tourism_routes_only_reward_new_major_markets_and_favor_the_largest_bar() {
    let mut g = board();
    let ai = AdvancedAi::targeting(VictoryTarget::Culture);
    Arc::make_mut(&mut g.observed_tourism_per_turn).insert(0, 100.0);
    let first = ai.culture_route_bonus(&g, 0, 1, GrandStrategy::Culture);
    let second = ai.culture_route_bonus(&g, 0, 2, GrandStrategy::Culture);
    assert!(second > first && first > 0.0);
    assert_eq!(
        ai.culture_route_bonus(&g, 0, 0, GrandStrategy::Culture),
        0.0
    );
    assert_eq!(
        ai.culture_route_bonus(&g, 0, 1, GrandStrategy::Science),
        0.0
    );
    g.routes.push(crate::game::TradeRoute {
        origin: g.player_city_ids(0)[0],
        dest: g.player_city_ids(1)[0],
        owner: 0,
        ends: g.turn + 10,
    });
    assert_eq!(
        ai.culture_route_bonus(&g, 0, 1, GrandStrategy::Culture),
        0.0
    );
    g.routes[0].ends = g.turn;
    assert_eq!(
        ai.culture_route_bonus(&g, 0, 1, GrandStrategy::Culture),
        first
    );
    g.players[1].is_minor = true;
    assert_eq!(
        ai.culture_route_bonus(&g, 0, 1, GrandStrategy::Culture),
        0.0
    );
    g.players[1].is_minor = false;
    g.players[0].team = Some(7);
    g.players[1].team = Some(7);
    assert_eq!(
        ai.culture_route_bonus(&g, 0, 1, GrandStrategy::Culture),
        0.0
    );
}

#[test]
fn culture_chain_stays_valuable_during_expansion_but_must_finish_in_time() {
    let mut g = board();
    let ai = AdvancedAi::targeting(VictoryTarget::Culture);
    let amph = Item::Building {
        building: crate::name!("amphitheater"),
    };
    assert!(ai.culture_race_production_bonus(&g, 0, &amph, GrandStrategy::Expansion, 5.0) > 500.0);
    assert_eq!(
        ai.culture_race_production_bonus(
            &g,
            0,
            &Item::Unit {
                unit: crate::name!("warrior")
            },
            GrandStrategy::Culture,
            5.0
        ),
        0.0
    );
    g.turn = 248;
    assert_eq!(
        ai.culture_race_production_bonus(&g, 0, &amph, GrandStrategy::Culture, 5.0),
        0.0
    );
}

#[test]
fn science_defense_lifts_the_culture_building_veto_without_switching_victory() {
    let mut g = board();
    let ai = AdvancedAi::targeting(VictoryTarget::Science);
    let cid = g.player_city_ids(0)[0];
    let pos = g.cities[&cid].pos;
    g.cities
        .get_mut(&cid)
        .unwrap()
        .districts
        .insert(crate::name!("theater_square"), pos);
    let item = Item::Building {
        building: crate::name!("amphitheater"),
    };
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: g.turn,
        rush: false,
    };
    let defended = ai.production_value(&g, 0, cid, &item, &plan, &ai.counts(&g, 0));
    assert!(
        defended > 0.0,
        "defense must reach the production scorer: {defended}"
    );
    Arc::make_mut(&mut g.observed_public_empire_stats)
        .get_mut(&1)
        .unwrap()
        .foreign_tourists = Some(0);
    let peaceful = ai.production_value(&g, 0, cid, &item, &plan, &ai.counts(&g, 0));
    assert_eq!(peaceful, -10_000.0);
    assert_eq!(ai.active_victory_target(&g), Some(VictoryTarget::Science));
}

#[test]
fn culture_defense_policies_replace_occupied_slots_and_release_after_threat() {
    let mut g = board();
    g.players[0].government = Some("classical_republic".to_string());
    g.players[0].civics.extend([
        crate::name!("political_philosophy"),
        crate::name!("foreign_trade"),
        crate::name!("code_of_laws"),
        crate::name!("state_workforce"),
        crate::name!("mysticism"),
        crate::name!("globalization"),
        crate::name!("space_race"),
        crate::name!("exodus_imperative"),
    ]);
    g.players[1].civics.insert(crate::name!("cold_war"));
    g.players[0].policies.extend([
        crate::name!("caravansaries"),
        crate::name!("urban_planning"),
        crate::name!("international_space_agency"),
        crate::name!("revelation"),
    ]);
    let ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.strategic_policies(&mut g, 0, GrandStrategy::Science);
    assert!(g.players[0]
        .policies
        .contains(&crate::name!("music_censorship")));
    assert!(g.players[0]
        .policies
        .contains(&crate::name!("future_counter_culture")));
    assert!(!g.players[0]
        .policies
        .contains(&crate::name!("international_space_agency")));
    assert_eq!(g.policy_effect(0, "block_foreign_rock_bands"), 1.0);
    assert_eq!(g.policy_effect(0, "incoming_tourism_pct"), -20.0);
    Arc::make_mut(&mut g.observed_public_empire_stats)
        .get_mut(&1)
        .unwrap()
        .foreign_tourists = Some(0);
    ai.strategic_policies(&mut g, 0, GrandStrategy::Science);
    assert!(g.players[0]
        .policies
        .contains(&crate::name!("international_space_agency")));
    assert!(!g.players[0]
        .policies
        .contains(&crate::name!("music_censorship")));
}

#[test]
fn culture_production_recovers_the_treasury_before_adding_upkeep() {
    let mut g = board();
    g.players[0].gold = 0.0;
    g.players[0].gold_per_turn = -12.0;
    g.players[0].techs.insert(crate::name!("pottery"));
    let cid = g.player_city_ids(0)[0];
    let mut ai = AdvancedAi::targeting(VictoryTarget::Culture);
    ai.disable_war_economy();
    let recovery = ai
        .base
        .economic_recovery_item(&g, 0, cid, ai.counts(&g, 0).traders)
        .or_else(|| ai.base.upkeep_free_recovery_item(&g, 0, cid))
        .unwrap();
    let plan = StrategicPlan {
        strategy: GrandStrategy::Culture,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 1,
        assessed_turn: g.turn,
        rush: false,
    };
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&cid].queue.first(), Some(&recovery));
}

#[test]
fn censorship_amenity_cost_ends_even_without_a_replacement_card() {
    let mut g = board();
    g.players[0].government = Some("classical_republic".to_string());
    g.players[0]
        .policies
        .insert(crate::name!("music_censorship"));
    Arc::make_mut(&mut g.observed_public_empire_stats)
        .get_mut(&1)
        .unwrap()
        .foreign_tourists = Some(0);
    // An exhausted/host-blocked menu cannot supply a replacement. The
    // condition ending must itself remove the downside, not await a swap.
    g.blocked_policies.extend(g.rules.policies.keys().copied());
    AdvancedAi::targeting(VictoryTarget::Culture).strategic_policies(
        &mut g,
        0,
        GrandStrategy::Culture,
    );
    assert!(!g.players[0]
        .policies
        .contains(&crate::name!("music_censorship")));
}
