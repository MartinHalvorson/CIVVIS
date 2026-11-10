use super::*;
use crate::game::GovernorState;
use crate::setup::GameSpeed;

fn board(governor: &str, promotions: &[&str]) -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let mut g = Game::new(3, 32, 22, 71, 250, 0);
    g.game_speed = GameSpeed::Online;
    // The rule under test is the governor's. Where this seed's Barbarians
    // happen to land must not decide whether either city counts as safe.
    let barbarians: Vec<u32> = g
        .units
        .iter()
        .filter(|(_, unit)| g.players[unit.owner].is_barbarian)
        .map(|(id, _)| *id)
        .collect();
    for id in barbarians {
        g.remove_unit(id);
    }
    g.barb_pid = None;
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
    check("pingala-follows-research-2", |a| {
        a.pingala_follows_research_2
    });
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
        assert!(AdvancedAi::governor_dividend_reading(&g, 0, governor) < before);
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
        if governor == "magnus" {
            assert!(g.settler_consumes_population(0, new));
        } else {
            assert_eq!(g.governor_effect(0, new, "builder_charges"), 0.0);
        }
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

fn clipped_native_post(template: &Game, turn: u32, governor: &str) -> Game {
    let mut g = template.clone();
    g.turn = turn;
    let establish = g.standard_duration(g.rules.governors[governor].establish_turns);
    g.players[0]
        .governor_roster
        .get_mut(governor)
        .unwrap()
        .assigned_turn = turn.saturating_sub(establish);
    g
}

#[test]
fn native_fresh_boards_eventually_relocate_despite_clipped_establishment_age() {
    let (template, mut ai, plan, old, new) = board("pingala", &["researcher", "connoisseur"]);
    ai.enable_pingala_follows_research();
    ai.live_governor_assignment_adapter = true;
    let review = template.standard_duration(16).max(1);
    let mut moved = None;
    for turn in template.turn..=template.turn + review * 2 {
        let mut fresh = clipped_native_post(&template, turn, "pingala");
        let establish = fresh.standard_duration(fresh.rules.governors["pingala"].establish_turns);
        assert!(
            turn - fresh.players[0].governor_roster["pingala"].assigned_turn
                < fresh.standard_duration(20)
        );
        ai.relocate_governors_for_dividends(&mut fresh, 0, &plan);
        if fresh.players[0].governor_roster["pingala"].city == Some(new) {
            assert!(turn - (template.turn - establish) >= fresh.standard_duration(20));
            moved = Some(turn);
            break;
        }
        assert_eq!(fresh.players[0].governor_roster["pingala"].city, Some(old));
    }
    let turn = moved.expect("observing the same post lets its cooldown expire");
    // The host may still show the old post on another frame while assignment
    // is in flight. One request per turn, even though the old residence is mature.
    let mut lagged = clipped_native_post(&template, turn, "pingala");
    ai.relocate_governors_for_dividends(&mut lagged, 0, &plan);
    assert_eq!(lagged.players[0].governor_roster["pingala"].city, Some(old));
}

#[test]
fn native_replan_frames_do_not_count_as_residence_turns() {
    let (template, mut ai, plan, old, _) = board("pingala", &["researcher"]);
    ai.enable_pingala_follows_research();
    ai.live_governor_assignment_adapter = true;
    let establish = template.standard_duration(template.rules.governors["pingala"].establish_turns);
    for _ in 0..30 {
        let mut fresh = clipped_native_post(&template, template.turn, "pingala");
        ai.relocate_governors_for_dividends(&mut fresh, 0, &plan);
        assert_eq!(fresh.players[0].governor_roster["pingala"].city, Some(old));
        assert_eq!(ai.governor_residence_age(&fresh, 0, "pingala"), establish);
    }
}

#[test]
fn native_post_clock_survives_city_id_remapping_but_not_a_different_post() {
    let (template, mut ai, _, old, new) = board("pingala", &["researcher"]);
    ai.live_governor_assignment_adapter = true;
    let first = clipped_native_post(&template, template.turn, "pingala");
    ai.observe_governor_posts(&first, 0);
    let establish = first.standard_duration(first.rules.governors["pingala"].establish_turns);
    let mut remapped = clipped_native_post(&template, template.turn + 1, "pingala");
    let mut city = remapped.cities.remove(&old).unwrap();
    let replacement_id = old + 10000;
    city.id = replacement_id;
    remapped.cities.insert(replacement_id, city);
    remapped.players[0]
        .governor_roster
        .get_mut("pingala")
        .unwrap()
        .city = Some(replacement_id);
    remapped.players[0].governors = vec![replacement_id];
    ai.observe_governor_posts(&remapped, 0);
    assert_eq!(
        ai.governor_residence_age(&remapped, 0, "pingala"),
        establish + 1
    );
    remapped.turn += 1;
    remapped.players[0]
        .governor_roster
        .get_mut("pingala")
        .unwrap()
        .city = Some(new);
    remapped.players[0]
        .governor_roster
        .get_mut("pingala")
        .unwrap()
        .assigned_turn = remapped.turn - establish;
    ai.observe_governor_posts(&remapped, 0);
    assert_eq!(
        ai.governor_residence_age(&remapped, 0, "pingala"),
        establish
    );
}

#[test]
fn native_post_clock_resets_for_neutralization_missing_posts_and_clock_rollback() {
    let (template, mut ai, _, old, _) = board("pingala", &["researcher"]);
    ai.live_governor_assignment_adapter = true;
    let mut g = clipped_native_post(&template, template.turn, "pingala");
    ai.observe_governor_posts(&g, 0);
    g = clipped_native_post(&template, template.turn + 20, "pingala");
    ai.observe_governor_posts(&g, 0);
    assert!(ai.governor_residence_age(&g, 0, "pingala") > g.standard_duration(20));
    g.players[0]
        .governor_roster
        .get_mut("pingala")
        .unwrap()
        .disabled_until = g.turn + 2;
    ai.observe_governor_posts(&g, 0);
    assert!(!ai.remembered_governor_posts.contains_key("pingala"));
    g.players[0]
        .governor_roster
        .get_mut("pingala")
        .unwrap()
        .disabled_until = 0;
    ai.observe_governor_posts(&g, 0);
    let establish = g.standard_duration(g.rules.governors["pingala"].establish_turns);
    assert_eq!(ai.governor_residence_age(&g, 0, "pingala"), establish);
    g.players[0]
        .governor_roster
        .get_mut("pingala")
        .unwrap()
        .city = None;
    ai.observe_governor_posts(&g, 0);
    assert!(!ai.remembered_governor_posts.contains_key("pingala"));
    g.players[0]
        .governor_roster
        .get_mut("pingala")
        .unwrap()
        .city = Some(old);
    ai.observe_governor_posts(&g, 0);
    g = clipped_native_post(&template, template.turn - 1, "pingala");
    ai.observe_governor_posts(&g, 0);
    assert_eq!(ai.governor_residence_age(&g, 0, "pingala"), establish);
}

#[test]
fn native_post_clock_resets_when_establishment_restarts_in_the_same_city() {
    let (template, mut ai, _, _, _) = board("pingala", &["researcher"]);
    ai.live_governor_assignment_adapter = true;
    let g = clipped_native_post(&template, template.turn, "pingala");
    ai.observe_governor_posts(&g, 0);
    let mut g = clipped_native_post(&template, template.turn + 20, "pingala");
    g.players[0]
        .governor_roster
        .get_mut("pingala")
        .unwrap()
        .assigned_turn = g.turn;
    ai.observe_governor_posts(&g, 0);
    assert_eq!(ai.governor_residence_age(&g, 0, "pingala"), 0);
}

#[test]
fn ordinary_governor_cooldowns_ignore_native_observation_memory() {
    let (template, mut ai, plan, old, _) = board("pingala", &["researcher"]);
    ai.enable_pingala_follows_research();
    ai.live_governor_assignment_adapter = true;
    let g = clipped_native_post(&template, template.turn, "pingala");
    ai.observe_governor_posts(&g, 0);
    ai.live_governor_assignment_adapter = false;
    let mut g = clipped_native_post(
        &template,
        template.turn + template.standard_duration(16),
        "pingala",
    );
    ai.relocate_governors_for_dividends(&mut g, 0, &plan);
    assert_eq!(g.players[0].governor_roster["pingala"].city, Some(old));
    assert!(ai.remembered_governor_posts.is_empty());
}

fn launch_board() -> (Game, AdvancedAi, StrategicPlan, u32, u32) {
    let (mut g, mut ai, plan, old, new) = board(
        "pingala",
        &["researcher", "connoisseur", "space_initiative", "curator"],
    );
    ai.enable_pingala_follows_research_2();
    g.players[0].governor_roster.insert(
        "magnus".into(),
        crate::game::GovernorState {
            city: Some(new),
            assigned_turn: 0,
            disabled_until: 0,
            promotions: ["provision".into(), "surplus_logistics".into()]
                .into_iter()
                .collect(),
        },
    );
    g.players[0].governors.push(new);
    let pos = g.cities[&new].pos;
    g.cities
        .get_mut(&new)
        .unwrap()
        .districts
        .insert(Name::new("spaceport"), pos);
    let item = Item::Project {
        project: Name::new("launch_earth_satellite"),
    };
    g.cities.get_mut(&new).unwrap().queue = vec![item.clone()];
    let production = g.item_remaining_cost_for_city(0, new, &item)
        / (20.0 * g.item_prod_mult(0, new, Some(&item)));
    let before = g.city_yields(new).production;
    std::sync::Arc::make_mut(&mut g.observed_city_yield_adjustments)
        .entry(new)
        .or_default()
        .production += production - before;
    (g, ai, plan, old, new)
}

#[test]
fn launch_post_swap_keeps_both_governors_and_delivers_project_bonus_after_establishment() {
    let (mut g, mut ai, plan, old, new) = launch_board();
    let item = g.cities[&new].queue[0].clone();
    let before = g.item_prod_mult(0, new, Some(&item));
    let snapshot = g.clone();
    ai.disable_pingala_follows_research_2();
    ai.relocate_governors_for_dividends(&mut g, 0, &plan);
    assert_eq!(g.players[0].governor_roster["pingala"].city, Some(old));
    ai.enable_pingala_follows_research_2();
    assert_eq!(
        ai.pingala_launch_posts(&g, 0, &plan),
        Some((new, Some(Name::new("magnus"))))
    );
    ai.relocate_governors_for_dividends(&mut g, 0, &plan);
    assert_eq!(g.players[0].governor_roster["pingala"].city, Some(new));
    assert_eq!(g.players[0].governor_roster["magnus"].city, Some(old));
    assert_eq!(
        g.players[0].governor_roster["pingala"].assigned_turn,
        g.turn
    );
    assert_eq!(
        g.item_prod_mult(0, new, Some(&item)),
        before,
        "no premature establishment bonus"
    );
    g.turn += g.standard_duration(g.rules.governors["pingala"].establish_turns);
    assert!((g.item_prod_mult(0, new, Some(&item)) - before - 0.3).abs() < 1e-9);
    assert!(
        AdvancedAi::governor_dividend_reading(&g, 0, "pingala")
            >= AdvancedAi::governor_dividend_reading(&snapshot, 0, "pingala")
    );
}

#[test]
fn launch_post_keeps_existing_spaceport_curator_and_specialist_jobs() {
    let (template, ai, plan, old, new) = launch_board();
    for variant in 0..7 {
        let mut g = template.clone();
        match variant {
            0 => {
                let pos = g.cities[&old].pos;
                g.cities
                    .get_mut(&old)
                    .unwrap()
                    .districts
                    .insert(Name::new("spaceport"), pos);
            }
            1 => {
                g.observed_great_work_housing = Some(std::sync::Arc::new(
                    [(old, [("writing".into(), 1)].into_iter().collect())]
                        .into_iter()
                        .collect(),
                ));
            }
            2 => {
                g.players[0]
                    .governor_roster
                    .get_mut("magnus")
                    .unwrap()
                    .promotions
                    .insert("industrialist".into());
            }
            3 => {
                let governor = g.players[0].governor_roster.remove("magnus").unwrap();
                g.players[0]
                    .governor_roster
                    .insert("victor".into(), governor);
            }
            4 => {
                g.cities.get_mut(&new).unwrap().loyalty = 89.0;
            }
            5 => {
                g.players[0]
                    .governor_roster
                    .get_mut("pingala")
                    .unwrap()
                    .assigned_turn = g.turn;
            }
            6 => {
                g.cities.get_mut(&old).unwrap().queue = g.cities[&new].queue.clone();
            }
            _ => unreachable!(),
        }
        assert!(
            ai.pingala_launch_posts(&g, 0, &plan).is_none(),
            "variant {variant}"
        );
    }
}

#[test]
fn launch_post_respects_war_recovery_and_short_native_establishment_runway() {
    let (template, mut ai, mut plan, old, new) = launch_board();
    let mut g = template.clone();
    plan.strategy = GrandStrategy::Recovery;
    ai.relocate_governors_for_dividends(&mut g, 0, &plan);
    assert_eq!(g.players[0].governor_roster["pingala"].city, Some(old));
    plan.strategy = GrandStrategy::Science;
    g.at_war.insert((0, 1));
    g.at_war.insert((1, 0));
    ai.relocate_governors_for_dividends(&mut g, 0, &plan);
    assert_eq!(g.players[0].governor_roster["pingala"].city, Some(old));
    let mut g = template;
    let item = g.cities[&new].queue[0].clone();
    let before = g.city_yields(new).production;
    let production = g.item_remaining_cost_for_city(0, new, &item)
        / (8.0 * g.item_prod_mult(0, new, Some(&item)));
    std::sync::Arc::make_mut(&mut g.observed_city_yield_adjustments)
        .entry(new)
        .or_default()
        .production += production - before;
    assert_eq!(
        g.standard_duration(5),
        5,
        "the shipped duration table keeps Online's five-turn interval"
    );
    assert!(
        ai.pingala_launch_posts(&g, 0, &plan).is_none(),
        "eight turns leave too little establishment/payback runway"
    );
    ai.live_governor_assignment_adapter = true;
    assert!(
        ai.pingala_launch_posts(&g, 0, &plan).is_none(),
        "native five-turn establishment also rejects the short project"
    );
}

#[test]
fn launch_post_can_establish_during_construction_without_boosting_the_spaceport() {
    let (mut g, mut ai, plan, old, new) = launch_board();
    g.cities
        .get_mut(&new)
        .unwrap()
        .districts
        .remove(Name::new("spaceport"));
    let item = Item::District {
        district: Name::new("spaceport"),
        pos: g.cities[&new].pos,
    };
    g.cities.get_mut(&new).unwrap().queue = vec![item.clone()];
    let before = g.city_yields(new).production;
    let production = g.item_remaining_cost_for_city(0, new, &item) / 12.0;
    std::sync::Arc::make_mut(&mut g.observed_city_yield_adjustments)
        .entry(new)
        .or_default()
        .production += production - before;
    let before_mult = g.item_prod_mult(0, new, Some(&item));
    assert!(ai.pingala_launch_posts(&g, 0, &plan).is_some());
    ai.relocate_governors_for_dividends(&mut g, 0, &plan);
    assert_eq!(g.players[0].governor_roster["pingala"].city, Some(new));
    assert_eq!(g.players[0].governor_roster["magnus"].city, Some(old));
    g.turn += g.standard_duration(g.rules.governors["pingala"].establish_turns);
    assert_eq!(g.item_prod_mult(0, new, Some(&item)), before_mult);
}

#[test]
fn launch_post_does_not_repeat_a_native_swap_on_a_lagged_replan() {
    let (template, mut ai, plan, old, new) = launch_board();
    ai.live_governor_assignment_adapter = true;
    let mut first = template.clone();
    ai.relocate_governors_for_dividends(&mut first, 0, &plan);
    assert_eq!(first.players[0].governor_roster["pingala"].city, Some(new));
    let mut lagged = template;
    ai.relocate_governors_for_dividends(&mut lagged, 0, &plan);
    assert_eq!(lagged.players[0].governor_roster["pingala"].city, Some(old));
    assert_eq!(lagged.players[0].governor_roster["magnus"].city, Some(new));
}

#[test]
fn pingala_versions_are_exclusive_and_original_keeps_the_launch_freeze() {
    let (mut g, mut ai, plan, old, new) = launch_board();
    assert!(ai.pingala_follows_research_2 && !ai.pingala_follows_research);
    ai.enable_pingala_follows_research();
    assert!(ai.pingala_follows_research && !ai.pingala_follows_research_2);
    assert!(ai.pingala_launch_posts(&g, 0, &plan).is_none());
    ai.relocate_governors_for_dividends(&mut g, 0, &plan);
    assert_eq!(g.players[0].governor_roster["pingala"].city, Some(old));
    assert_eq!(g.players[0].governor_roster["magnus"].city, Some(new));
    ai.enable_pingala_follows_research_2();
    assert!(!ai.pingala_follows_research && ai.pingala_follows_research_2);
    ai.disable_pingala_follows_research_2();
    assert!(!ai.pingala_follows_research && !ai.pingala_follows_research_2);
}

#[test]
fn pingala_version_two_retains_the_ordinary_research_relocation() {
    let (mut g, mut ai, plan, old, new) = board("pingala", &["researcher", "connoisseur"]);
    ai.enable_pingala_follows_research_2();
    ai.relocate_governors_for_dividends(&mut g, 0, &plan);
    assert_ne!(old, new);
    assert_eq!(g.players[0].governor_roster["pingala"].city, Some(new));
}
