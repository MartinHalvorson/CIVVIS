use super::*;
use crate::game::GovernorState;
use crate::setup::GameSpeed;

fn board(governor: &str, promotions: &[&str]) -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let mut g = Game::new(3, 32, 22, 71, 250, 0);
    g.game_speed = GameSpeed::Online;
    let settler = g
        .player_unit_ids(0)
        .into_iter()
        .find(|id| g.units[id].kind == "settler")
        .unwrap();
    let old = g.found_city_for(0, g.units[&settler].pos, None);
    g.remove_unit(settler);
    let new = g.found_city_for(0, (10, 10), None);
    g.cities.get_mut(&old).unwrap().pop = 2;
    g.cities.get_mut(&new).unwrap().pop = 20;
    for cid in [old, new] {
        g.cities.get_mut(&cid).unwrap().queue.clear();
    }
    g.players[0].governor_roster.insert(
        governor.to_string(),
        GovernorState {
            city: Some(old),
            assigned_turn: 0,
            disabled_until: 0,
            promotions: promotions.iter().map(|p| p.to_string()).collect(),
        },
    );
    g.players[0].governors = vec![old];
    g.turn = g.standard_duration(16) * 8;
    let plan = StrategicPlan {
        strategy: GrandStrategy::Science,
        target_player: None,
        target_city: None,
        threatened_city: None,
        desired_cities: 5,
        assessed_turn: g.turn,
        rush: false,
    };
    (g, AdvancedAi::new(), plan, old, new)
}

fn queue_job(g: &mut Game, cid: u32, kind: &str, turns: f64) {
    let item = Item::Unit {
        unit: Name::new(kind),
    };
    g.cities.get_mut(&cid).unwrap().queue = vec![item.clone()];
    let production = g.item_remaining_cost_for_city(0, cid, &item) / turns;
    let old = g.city_yields(cid).production;
    let adjustment = std::sync::Arc::make_mut(&mut g.observed_city_yield_adjustments)
        .entry(cid)
        .or_default();
    adjustment.production += production - old;
}

#[test]
fn registered_and_off_in_both_controllers() {
    use super::super::test_support::opt_in_off_in_both_controllers as check;
    check("pingala-follows-research", |a| a.pingala_follows_research);
    check("magnus-follows-settlers", |a| a.magnus_follows_settlers);
    check("liang-follows-builders", |a| a.liang_follows_builders);
    check("reyna-follows-revenue", |a| a.reyna_follows_revenue);
    check("amani-follows-suzerainty", |a| a.amani_follows_suzerainty);
}

#[test]
fn research_and_revenue_relocations_change_real_orders_and_pay_after_establishment() {
    for (governor, promotions) in [
        ("pingala", vec!["researcher", "connoisseur"]),
        ("reyna", vec!["tax_collector"]),
    ] {
        let (mut g, mut ai, plan, old, new) = board(governor, &promotions);
        ai.relocate_governors_for_dividends(&mut g, 0, &plan);
        assert_eq!(g.players[0].governor_roster[governor].city, Some(old));
        let before = AdvancedAi::governor_dividend_reading(&g, 0, governor);
        if governor == "pingala" {
            ai.enable_pingala_follows_research();
        } else {
            ai.enable_reyna_follows_revenue();
        }
        assert_eq!(
            ai.governor_dividend_destination(&g, 0, governor, &plan),
            Some(new)
        );
        ai.relocate_governors_for_dividends(&mut g, 0, &plan);
        assert_eq!(g.players[0].governor_roster[governor].city, Some(new));
        assert_eq!(g.players[0].governor_roster[governor].assigned_turn, g.turn);
        g.turn += g.standard_duration(g.rules.governors[governor].establish_turns);
        assert!(AdvancedAi::governor_dividend_reading(&g, 0, governor) > before);
    }
}

#[test]
fn unit_governors_arrive_before_the_existing_job_finishes() {
    for (governor, job, promotions) in [
        ("magnus", "settler", vec!["provision"]),
        ("liang", "builder", vec![]),
    ] {
        let (mut g, mut ai, plan, old, new) = board(governor, &promotions);
        if governor == "magnus" {
            ai.enable_magnus_follows_settlers();
        } else {
            ai.enable_liang_follows_builders();
        }
        assert!(
            ai.governor_dividend_destination(&g, 0, governor, &plan)
                .is_none(),
            "no job, no relocation"
        );
        queue_job(&mut g, new, job, 1.0);
        assert!(
            ai.governor_dividend_destination(&g, 0, governor, &plan)
                .is_none(),
            "too late to affect completion"
        );
        queue_job(&mut g, new, job, 8.0);
        let queue = g.cities[&new].queue.clone();
        assert_eq!(
            ai.governor_dividend_destination(&g, 0, governor, &plan),
            Some(new)
        );
        queue_job(&mut g, old, job, 10.0);
        assert!(
            ai.governor_dividend_destination(&g, 0, governor, &plan)
                .is_none(),
            "keep the existing job"
        );
        g.cities.get_mut(&old).unwrap().queue.clear();
        ai.relocate_governors_for_dividends(&mut g, 0, &plan);
        assert_eq!(g.players[0].governor_roster[governor].city, Some(new));
        assert_eq!(
            g.cities[&new].queue, queue,
            "a move does not replace production"
        );
        g.turn += g.standard_duration(g.rules.governors[governor].establish_turns);
        if governor == "magnus" {
            assert!(!g.settler_consumes_population(0, new));
        } else {
            assert!(g.governor_effect(0, new, "builder_charges") > 0.0);
        }
    }
}

#[test]
fn moves_respect_residency_clock_threats_loyalty_and_other_governors() {
    let (mut g, mut ai, mut plan, old, new) = board("pingala", &["researcher", "connoisseur"]);
    ai.enable_pingala_follows_research();
    assert_eq!(
        ai.governor_dividend_destination(&g, 0, "pingala", &plan),
        Some(new)
    );
    g.players[0]
        .governor_roster
        .get_mut("pingala")
        .unwrap()
        .assigned_turn = g.turn - 1;
    assert!(ai
        .governor_dividend_destination(&g, 0, "pingala", &plan)
        .is_none());
    g.players[0]
        .governor_roster
        .get_mut("pingala")
        .unwrap()
        .assigned_turn = 0;
    g.players[0]
        .governor_roster
        .get_mut("pingala")
        .unwrap()
        .disabled_until = g.turn + 10;
    assert!(ai
        .governor_dividend_destination(&g, 0, "pingala", &plan)
        .is_none());
    g.players[0]
        .governor_roster
        .get_mut("pingala")
        .unwrap()
        .disabled_until = 0;
    g.cities.get_mut(&old).unwrap().loyalty = 89.0;
    assert!(ai
        .governor_dividend_destination(&g, 0, "pingala", &plan)
        .is_none());
    g.cities.get_mut(&old).unwrap().loyalty = 100.0;
    plan.threatened_city = Some(new);
    assert!(ai
        .governor_dividend_destination(&g, 0, "pingala", &plan)
        .is_none());
    plan.threatened_city = None;
    let mut victor = g.players[0].governor_roster["pingala"].clone();
    victor.city = Some(new);
    g.players[0].governor_roster.insert("victor".into(), victor);
    assert!(ai
        .governor_dividend_destination(&g, 0, "pingala", &plan)
        .is_none());
    g.players[0].governor_roster.remove("victor");
    plan.strategy = GrandStrategy::Recovery;
    ai.relocate_governors_for_dividends(&mut g, 0, &plan);
    assert_eq!(g.players[0].governor_roster["pingala"].city, Some(old));
    plan.strategy = GrandStrategy::Science;
    g.turn = 249;
    assert!(ai
        .governor_dividend_destination(&g, 0, "pingala", &plan)
        .is_none());
}

#[test]
fn amani_gains_a_new_suzerainty_without_trading_away_the_old_one() {
    let (mut g, mut ai, plan, _, _) = board("amani", &[]);
    let first = g.found_city_for(1, (20, 10), None);
    let second_pos = *g
        .map
        .tiles
        .keys()
        .find(|p| g.city_at(**p).is_none() && **p != (20, 10))
        .unwrap();
    let second = g.found_city_for(2, second_pos, None);
    for pid in [1, 2] {
        g.players[pid].is_minor = true;
        g.record_contact(0, pid);
    }
    g.players[0].envoys = vec![(1, 1), (2, 1)];
    g.players[0].governor_roster.get_mut("amani").unwrap().city = Some(first);
    g.players[0].governors.clear();
    ai.enable_amani_follows_suzerainty();
    assert_eq!(g.suzerain_of(1), Some(0));
    assert!(ai
        .governor_dividend_destination(&g, 0, "amani", &plan)
        .is_none());
    g.players[0].envoys[0].1 = 3;
    assert_eq!(
        ai.governor_dividend_destination(&g, 0, "amani", &plan),
        Some(second)
    );
    ai.relocate_governors_for_dividends(&mut g, 0, &plan);
    assert_eq!(g.players[0].governor_roster["amani"].city, Some(second));
    assert_eq!(g.suzerain_of(1), Some(0));
    assert_ne!(g.suzerain_of(2), Some(0), "establishment still takes time");
    g.turn += g.standard_duration(g.rules.governors["amani"].establish_turns);
    assert_eq!(g.suzerain_of(2), Some(0));
}

#[test]
fn counterfactuals_leave_the_real_governor_and_yields_unchanged() {
    let (g, ai, plan, old, _) = board("pingala", &["researcher", "connoisseur"]);
    let roster = g.players[0].governor_roster.clone();
    let reading = AdvancedAi::governor_dividend_reading(&g, 0, "pingala");
    // Also test inside a caller-owned memo, the shape of the real turn.
    let _memo = g.query_memo();
    assert!(ai
        .governor_dividend_destination(&g, 0, "pingala", &plan)
        .is_some());
    assert!(g.players[0].governor_roster == roster);
    assert_eq!(g.players[0].governors, vec![old]);
    assert_eq!(
        AdvancedAi::governor_dividend_reading(&g, 0, "pingala"),
        reading
    );
}
