use super::super::tests::{strategic_wonder_fixture, wonder_plan};
use super::*;

fn paused_wonder() -> (Game, AdvancedAi, u32, Item, StrategicPlan) {
    let (mut g, city) = strategic_wonder_fixture(6_402, "great_library");
    let item = Item::Wonder {
        wonder: crate::name!("great_library"),
        pos: g.wonder_sites(city, "great_library")[0],
    };
    g.apply(
        0,
        &Action::Produce {
            city,
            item: item.clone(),
        },
    )
    .unwrap();
    g.cities.get_mut(&city).unwrap().production = g.item_cost_for_city(0, city, &item) / 2.0;
    g.apply(
        0,
        &Action::Produce {
            city,
            item: Item::Unit {
                unit: crate::name!("warrior"),
            },
        },
    )
    .unwrap();
    assert!(g.item_invested_production(city, &item) > 0.0);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Science);
    ai.base.book_pos = 4;
    let plan = wonder_plan(GrandStrategy::Science, g.turn);
    (g, ai, city, item, plan)
}

fn finish_turn(g: &mut Game) {
    let turn = g.turn;
    for _ in 0..32 {
        let pid = g.current;
        g.apply(pid, &Action::EndTurn).unwrap();
        if g.turn != turn {
            return;
        }
    }
    panic!("turn did not advance");
}

#[test]
fn interrupted_wonder_resumes_after_defender_finishes_and_then_completes() {
    let (mut g, mut ai, city, wonder, plan) = paused_wonder();
    let defender = g.cities[&city].queue[0].clone();
    let defenders = g.player_unit_ids(0).len();
    g.cities.get_mut(&city).unwrap().production = g.item_cost_for_city(0, city, &defender);
    finish_turn(&mut g);
    assert!(g.player_unit_ids(0).len() > defenders);
    assert!(g.cities[&city].queue.is_empty());
    // The main governor must recover a banked wonder even though the current
    // Science preference would not start this wonder from scratch.
    ai.advanced_production(&mut g, 0, &plan, false);
    assert_eq!(g.cities[&city].queue.first(), Some(&wonder));
    g.cities.get_mut(&city).unwrap().production = g.item_cost_for_city(0, city, &wonder);
    finish_turn(&mut g);
    assert!(g.cities[&city]
        .wonders
        .contains_key(&crate::name!("great_library")));
    assert_eq!(g.item_invested_production(city, &wonder), 0.0);
    let counts = ai.counts_without_city_queue(&g, 0, city);
    assert!(!ai.resume_city_production(&mut g, 0, city, &plan, &counts));
}

#[test]
fn overflow_does_not_turn_every_item_into_a_commitment() {
    let (mut g, _, city, wonder, _) = paused_wonder();
    let fresh = Item::Unit {
        unit: crate::name!("builder"),
    };
    g.cities.get_mut(&city).unwrap().queue.clear();
    g.cities.get_mut(&city).unwrap().production = 10_000.0;
    assert_eq!(g.item_invested_production(city, &fresh), 0.0);
    assert_eq!(g.item_remaining_cost_for_city(0, city, &fresh), 0.0);
    assert!(g.item_invested_production(city, &wonder) > 0.0);
}

#[test]
fn resumption_preserves_defense_and_recovery_reservations() {
    let (g, ai, city, _, plan) = paused_wonder();
    let counts = ai.counts(&g, 0);
    let mut busy = g.clone();
    assert!(!ai.resume_city_production(&mut busy, 0, city, &plan, &counts));
    for threat in [true, false] {
        let mut crisis = g.clone();
        crisis.cities.get_mut(&city).unwrap().queue.clear();
        let mut urgent = plan.clone();
        if threat {
            urgent.threatened_city = Some(city);
        } else {
            urgent.strategy = GrandStrategy::Recovery;
        }
        assert!(!ai.resume_city_production(&mut crisis, 0, city, &urgent, &counts));
        assert!(crisis.cities[&city].queue.is_empty());
    }
}

#[test]
fn host_rejected_investments_do_not_stick_even_when_review_is_disabled() {
    let (mut g, _, city, wonder, plan) = paused_wonder();
    g.apply(
        0,
        &Action::Produce {
            city,
            item: wonder.clone(),
        },
    )
    .unwrap();
    let mut blocked = BTreeMap::new();
    blocked.insert(
        city,
        [Game::production_block_key(&wonder)].into_iter().collect(),
    );
    g.replace_blocked_production(blocked);
    for margin in [1.0, 100.0] {
        let mut board = g.clone();
        let mut ai = AdvancedAi::new();
        ai.preempt_margin = margin;
        ai.advanced_production(&mut board, 0, &plan, false);
        assert_ne!(board.cities[&city].queue.first(), Some(&wonder));
        assert!(board.cities[&city]
            .queue
            .first()
            .is_some_and(|item| board.can_produce(0, city, item)));
    }
}

#[test]
fn resumption_rejects_an_unavailable_wonder_and_does_not_retry_it() {
    let (mut g, ai, city, wonder, plan) = paused_wonder();
    g.cities.get_mut(&city).unwrap().queue.clear();
    let mut blocked = BTreeMap::new();
    blocked.insert(
        city,
        [Game::production_block_key(&wonder)].into_iter().collect(),
    );
    g.replace_blocked_production(blocked);
    let counts = ai.counts(&g, 0);
    let before = g.log.len();
    for _ in 0..3 {
        assert!(!ai.resume_city_production(&mut g, 0, city, &plan, &counts));
    }
    assert_eq!(g.log.len(), before);
}

#[test]
fn both_governors_get_the_same_idle_resumption_pass() {
    let (g, _, city, wonder, plan) = paused_wonder();
    for mut ai in [
        AdvancedAi::new(),
        AdvancedAi::targeting(VictoryTarget::Science),
    ] {
        let mut board = g.clone();
        board.cities.get_mut(&city).unwrap().queue.clear();
        board.cities.get_mut(&city).unwrap().production = 0.0;
        ai.reconcile_production_commitments(&mut board, 0, &plan);
        assert_eq!(board.cities[&city].queue.first(), Some(&wonder));
        // Further reviews must not restart or double-credit saved progress.
        let progress = board.cities[&city].production;
        ai.advanced_production(&mut board, 0, &plan, false);
        ai.reconcile_production_commitments(&mut board, 0, &plan);
        assert_eq!(board.cities[&city].production, progress);
        assert_eq!(board.cities[&city].queue.first(), Some(&wonder));
    }
}

#[test]
fn the_complete_turn_pipeline_resumes_saved_work_for_both_governors() {
    let (g, _, city, wonder, plan) = paused_wonder();
    for mut ai in [
        AdvancedAi::new(),
        AdvancedAi::targeting(VictoryTarget::Science),
    ] {
        let mut board = g.clone();
        board.cities.get_mut(&city).unwrap().queue.clear();
        board.cities.get_mut(&city).unwrap().production = 0.0;
        ai.base.book_pos = 4;
        ai.plan = Some(plan.clone());
        ai.plan_observed_turn(&mut board, 0);
        assert_eq!(board.cities[&city].queue.first(), Some(&wonder));
    }
}

#[test]
fn ordinary_saved_work_finishes_the_nearest_useful_payoff() {
    let (mut g, ai, city, _, plan) = paused_wonder();
    let near = Item::Unit {
        unit: crate::name!("warrior"),
    };
    let far = Item::Unit {
        unit: crate::name!("builder"),
    };
    let near_investment = g.item_cost_for_city(0, city, &near) - 1.0;
    let c = g.cities.get_mut(&city).unwrap();
    c.queue.clear();
    c.production = 0.0;
    c.production_progress.clear();
    c.production_progress
        .insert("unit:warrior".into(), near_investment);
    c.production_progress.insert("unit:builder".into(), 1.0);
    let counts = ai.counts(&g, 0);
    for item in [&near, &far] {
        assert!(g.can_produce(0, city, item));
        assert!(ai.production_value(&g, 0, city, item, &plan, &counts) > 0.0);
    }
    assert!(ai.resume_city_production(&mut g, 0, city, &plan, &counts));
    assert_eq!(g.cities[&city].queue.first(), Some(&near));
    assert_eq!(g.cities[&city].production, near_investment);
    assert_eq!(g.item_invested_production(city, &far), 1.0);
}

#[test]
fn an_investment_does_not_override_the_completion_deadline_or_its_exact_site() {
    let (mut g, ai, city, wonder, plan) = paused_wonder();
    let Item::Wonder { wonder: name, pos } = wonder else {
        unreachable!()
    };
    let wrong_site = Item::Wonder {
        wonder: name,
        pos: (pos.0 + 1, pos.1),
    };
    assert_eq!(g.item_invested_production(city, &wrong_site), 0.0);
    g.cities.get_mut(&city).unwrap().queue.clear();
    g.cities.get_mut(&city).unwrap().production = 0.0;
    g.max_turns = g.turn + 1;
    let counts = ai.counts(&g, 0);
    assert!(!ai.resume_city_production(&mut g, 0, city, &plan, &counts));
    assert!(g.cities[&city].queue.is_empty());
}

#[test]
fn the_complete_turn_pipeline_replaces_a_blocked_queue_for_both_governors() {
    let (mut g, _, city, wonder, plan) = paused_wonder();
    g.apply(
        0,
        &Action::Produce {
            city,
            item: wonder.clone(),
        },
    )
    .unwrap();
    let mut blocked = BTreeMap::new();
    blocked.insert(
        city,
        [Game::production_block_key(&wonder)].into_iter().collect(),
    );
    g.replace_blocked_production(blocked);
    for mut ai in [
        AdvancedAi::new(),
        AdvancedAi::targeting(VictoryTarget::Science),
    ] {
        let mut board = g.clone();
        ai.base.book_pos = 4;
        ai.plan = Some(plan.clone());
        ai.plan_observed_turn(&mut board, 0);
        let replacement = board.cities[&city].queue.first().unwrap();
        assert_ne!(replacement, &wonder);
        assert!(board.can_produce(0, city, replacement));
    }
}

#[test]
fn a_rival_starting_the_same_wonder_does_not_cancel_our_investment() {
    let (mut g, ai, city, wonder, plan) = paused_wonder();
    let founder = g
        .player_unit_ids(1)
        .into_iter()
        .find(|uid| g.units[uid].kind == "settler")
        .unwrap();
    let rival = g.found_city_for(1, g.units[&founder].pos, None);
    let Item::Wonder { wonder: name, .. } = &wonder else {
        unreachable!()
    };
    g.cities.get_mut(&rival).unwrap().queue = vec![Item::Wonder {
        wonder: *name,
        pos: g.cities[&rival].pos,
    }];
    g.cities.get_mut(&city).unwrap().queue.clear();
    g.cities.get_mut(&city).unwrap().production = 0.0;
    let counts = ai.counts(&g, 0);
    assert!(ai.production_value(&g, 0, city, &wonder, &plan, &counts) > 0.0);
    assert!(ai.resume_city_production(&mut g, 0, city, &plan, &counts));
    assert_eq!(g.cities[&city].queue.first(), Some(&wonder));
}
