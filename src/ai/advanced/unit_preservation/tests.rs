use super::*;
use crate::ai::Ai;

fn at(x: i32, y: i32) -> Pos {
    crate::hex::offset_to_axial(x, y)
}

fn field() -> Game {
    let mut g =
        crate::doctrine::build(crate::doctrine::position("the_reserve").unwrap(), 3).unwrap();
    g.tactics.heal = true;
    for pid in 0..2 {
        for uid in g.player_unit_ids(pid) {
            g.remove_unit(uid);
        }
    }
    g
}

fn policy() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_unit_preservation();
    ai
}

#[test]
fn guaranteed_damage_uses_the_roll_before_rounding_and_clamping() {
    assert_eq!(damage_floor(200.0, 20.0), 100);
    assert_eq!(damage_floor(20.0, 200.0), 1);
    assert_eq!(damage_floor(20.0, 20.0), 24);
}

#[test]
fn a_sampled_kill_does_not_remove_a_possible_reply() {
    let mut g = field();
    let ours = g.spawn_unit("warrior", 0, at(10, 7));
    let enemy = g.spawn_unit("warrior", 1, at(11, 7));
    g.units.get_mut(&enemy).unwrap().hp = 30;
    let action = Action::Attack {
        unit: ours,
        target: g.units[&enemy].pos,
    };
    // Supply a host mean that can kill, with a lower roll that cannot.
    std::sync::Arc::make_mut(&mut g.host_previews).insert(
        (ours, g.units[&enemy].pos, false),
        crate::game::HostStrikePreview {
            attacker_strength: 20.0,
            defender_strength: 20.0,
            damage_to_defender: 35,
            ..Default::default()
        },
    );
    let after = replay(&g, 0, &[action], &BTreeSet::new()).board;
    assert!(after.units.contains_key(&enemy));
    assert!(after.units[&enemy].hp >= 6);
    assert_eq!(g.units[&enemy].hp, 30);
}

fn city_capture(hp: i32) -> (Game, u32, u32, Vec<Action>) {
    let mut g = field();
    let ours = g.spawn_unit("horseman", 0, at(8, 7));
    g.units.get_mut(&ours).unwrap().hp = 40;
    let city = g.found_city_for(1, at(11, 7), None);
    g.cities.get_mut(&city).unwrap().hp = hp;
    g.cities.get_mut(&city).unwrap().wall_hp = 0;
    g.spawn_unit("archer", 1, at(12, 7));
    g.spawn_unit("archer", 1, at(12, 8));
    let actions = vec![
        Action::MoveTo {
            unit: ours,
            to: at(10, 7),
        },
        Action::Attack {
            unit: ours,
            target: at(11, 7),
        },
    ];
    (g, ours, city, actions)
}

#[test]
fn uncertain_city_capture_checks_the_actual_approach_after_moving() {
    let (mut g, ours, city, actions) = city_capture(20);
    let outcome = (0..100)
        .find_map(|seed| {
            g.rng = crate::rng::Rng::new(seed);
            let outcome = replay(&g, 0, &actions, &BTreeSet::new());
            (outcome.board.cities[&city].owner == 0).then_some(outcome)
        })
        .expect("at least one sampled roll captures the unwalled city");
    assert_eq!(outcome.board.units[&ours].pos, at(11, 7));
    assert_eq!(outcome.failed_advances[&ours], BTreeSet::from([at(10, 7)]));
    assert!(outcome.uncertain_captures.contains(&city));
    let ai = policy();
    assert!(ai.unit_reply_is_safe(&outcome.board, 0, ours));
    assert!(!ai.reply_outcomes_are_safe(
        &g,
        &outcome,
        0,
        ours,
        &mut ReplyForecast::new(&outcome.board, 0),
    ));
    assert_eq!(g.units[&ours].pos, at(8, 7));
    assert_eq!(g.cities[&city].owner, 1);
}

#[test]
fn minimum_damage_city_capture_can_still_reach_safety() {
    let (g, ours, city, actions) = city_capture(1);
    let outcome = replay(&g, 0, &actions, &BTreeSet::new());
    assert_eq!(outcome.board.cities[&city].owner, 0);
    assert_eq!(outcome.board.units[&ours].pos, at(11, 7));
    assert!(outcome.failed_advances.is_empty());
    assert!(outcome.uncertain_captures.is_empty());
    assert!(policy().preservation_finishing_safe(&g, 0, &actions));
}

#[test]
fn focus_fire_does_not_disappear_when_allies_are_nearby() {
    let mut g = field();
    let ours = g.spawn_unit("warrior", 0, at(10, 7));
    g.units.get_mut(&ours).unwrap().hp = 70;
    g.spawn_unit("archer", 1, at(11, 7));
    g.spawn_unit("archer", 1, at(11, 8));
    let ai = policy();
    assert!(!ai.unit_reply_is_safe(&g, 0, ours));
    g.spawn_unit("warrior", 0, at(9, 7));
    g.spawn_unit("warrior", 0, at(9, 8));
    assert!(
        !ai.unit_reply_is_safe(&g, 0, ours),
        "friendly targets cannot promise where hostile shots land"
    );
}

#[test]
fn a_survivable_first_reply_is_rejected_when_the_second_is_inescapable() {
    let mut g = field();
    let ours = g.spawn_unit("warrior", 0, at(10, 7));
    g.units.get_mut(&ours).unwrap().hp = 65;
    let enemy = g.spawn_unit("warrior", 1, at(11, 7));
    for pos in g.nbrs(g.units[&ours].pos) {
        if pos != g.units[&enemy].pos {
            g.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("mountain");
        }
    }
    let mut first = DangerField::with_reach(&g, 0, true);
    let incoming = reply_damage(&mut first, g.units[&ours].pos, ours, 65);
    assert!(incoming > 0.0 && incoming < 50.0, "first reply {incoming}");
    assert!(
        !policy().unit_reply_is_safe(&g, 0, ours),
        "one-turn survival is insufficient inside a trap"
    );
}

#[test]
fn an_open_escape_keeps_a_survivable_engagement_available() {
    let mut g = field();
    let ours = g.spawn_unit("horseman", 0, at(10, 7));
    g.units.get_mut(&ours).unwrap().hp = 75;
    g.spawn_unit("warrior", 1, at(11, 7));
    assert!(policy().unit_reply_is_safe(&g, 0, ours));
}

#[test]
fn rejected_attack_becomes_a_retreat_and_preserves_the_unit() {
    let mut g = field();
    let ours = g.spawn_unit("horseman", 0, at(10, 7));
    g.units.get_mut(&ours).unwrap().hp = 35;
    let enemy = g.spawn_unit("warrior", 1, at(11, 7));
    let origin = g.units[&ours].pos;
    let attack = Action::Attack {
        unit: ours,
        target: g.units[&enemy].pos,
    };
    let mut ai = policy();
    let actions = ai.preserve_unit_actions(&g, 0, std::slice::from_ref(&attack));
    assert!(!actions.contains(&attack));
    for action in &actions {
        g.apply(0, action).unwrap();
    }
    assert!(g.units.contains_key(&ours));
    assert_ne!(g.units[&ours].pos, origin, "escape must be executed");
    assert!(ai.battle_planner_recovering.contains(&ours));
}

#[test]
fn recovery_keeps_orders_until_full_health_then_releases_the_unit() {
    let mut g = field();
    let ours = g.spawn_unit("warrior", 0, at(8, 6));
    let movement = Action::MoveTo {
        unit: ours,
        to: at(9, 6),
    };
    let mut ai = policy();
    ai.battle_planner_recovering.insert(ours);
    for hp in [80, 90, 99] {
        g.units.get_mut(&ours).unwrap().hp = hp;
        let kept = ai.preserve_unit_actions(&g, 0, std::slice::from_ref(&movement));
        assert!(!kept.contains(&movement), "{hp} hp is still recovering");
        assert!(kept.contains(&Action::Fortify { unit: ours }));
    }
    g.units.get_mut(&ours).unwrap().hp = 100;
    assert_eq!(
        ai.preserve_unit_actions(&g, 0, std::slice::from_ref(&movement)),
        vec![movement]
    );
    assert!(!ai.battle_planner_recovering.contains(&ours));
}

#[test]
fn safe_finishing_kills_remain_available_and_the_authoritative_board_is_untouched() {
    let mut g = field();
    let ours = g.spawn_unit("warrior", 0, at(10, 7));
    let enemy = g.spawn_unit("warrior", 1, at(11, 7));
    g.units.get_mut(&enemy).unwrap().hp = 1;
    let action = Action::Attack {
        unit: ours,
        target: g.units[&enemy].pos,
    };
    let mut ai = policy();
    assert!(ai.live_finishing_actions_survive(&g, 0, [&action]));
    assert_eq!(
        ai.preserve_unit_actions(&g, 0, std::slice::from_ref(&action)),
        vec![action]
    );
    assert_eq!(g.units[&enemy].hp, 1);
    assert_eq!(g.units[&ours].pos, at(10, 7));
}

#[test]
fn proposed_turn_is_validated_on_the_real_entry_point_and_still_ends() {
    let mut g = field();
    let ours = g.spawn_unit("warrior", 0, at(10, 7));
    g.units.get_mut(&ours).unwrap().hp = 35;
    g.spawn_unit("warrior", 1, at(11, 7));
    let mut ai = policy();
    let before = g.log.len();
    ai.take_turn(&mut g, 0);
    assert!(g.units.contains_key(&ours));
    assert!(g
        .log
        .since(before)
        .any(|(pid, action)| *pid == 0 && matches!(action, Action::EndTurn)));
}

#[test]
fn the_gene_is_registered_opt_in_and_reversible() {
    let mut ai = AdvancedAi::new();
    assert!(!ai.unit_preservation);
    let gene = crate::ai::gene("unit-preservation").unwrap();
    assert!(gene.opt_in());
    (gene.enable)(&mut ai);
    assert!(ai.unit_preservation && ai.live_strike_survival_enabled());
    (gene.disable)(&mut ai);
    assert!(!ai.unit_preservation);
}

fn escorted_departure(
    hp: i32,
    origin: Pos,
    destination: Pos,
) -> (Game, AdvancedAi, u32, u32, Vec<Action>) {
    let mut g = field();
    let settler = g.spawn_unit("settler", 0, origin);
    let guard = g.spawn_unit("warrior", 0, origin);
    g.units.get_mut(&guard).unwrap().hp = hp;
    let mut ai = policy();
    ai.settler_guards.insert(settler, guard);
    let actions = vec![
        Action::MoveTo {
            unit: settler,
            to: destination,
        },
        Action::MoveTo {
            unit: guard,
            to: destination,
        },
    ];
    (g, ai, settler, guard, actions)
}

#[test]
fn a_recovering_guard_does_not_leave_its_departing_settler_alone() {
    let (mut g, mut ai, settler, guard, proposed) = escorted_departure(80, at(8, 6), at(9, 6));
    ai.battle_planner_recovering.insert(guard);
    let kept = ai.preserve_unit_actions(&g, 0, &proposed);
    for action in &kept {
        g.apply(0, action).expect("preserved orders must execute");
    }
    assert_eq!(
        g.units[&settler].pos, g.units[&guard].pos,
        "withholding an escort's move must also reconcile its Settler's departure"
    );
    assert_eq!(g.units[&guard].hp, 80);
    assert!(ai.battle_planner_recovering.contains(&guard));
}

#[test]
fn an_unsafe_guard_takes_its_settler_to_the_same_survivable_retreat() {
    let origin = at(10, 7);
    let (mut g, mut ai, settler, guard, proposed) = escorted_departure(35, origin, at(10, 8));
    g.spawn_unit("warrior", 1, at(11, 7));
    assert!(
        !ai.unit_reply_is_safe(&g, 0, guard),
        "the origin requires a retreat"
    );
    let kept = ai.preserve_unit_actions(&g, 0, &proposed);
    for action in &kept {
        g.apply(0, action).expect("preserved orders must execute");
    }
    assert_ne!(
        g.units[&guard].pos, origin,
        "a hold cannot replace the required escape"
    );
    assert!(
        ai.unit_reply_is_safe(&g, 0, guard),
        "the guard's escape must remain survivable"
    );
    assert_eq!(
        g.units[&settler].pos, g.units[&guard].pos,
        "the retreat must preserve the escort's companion too"
    );
}

#[test]
fn a_safe_escorted_departure_keeps_both_original_orders() {
    let (g, mut ai, _, _, proposed) = escorted_departure(100, at(8, 6), at(9, 6));
    assert_eq!(ai.preserve_unit_actions(&g, 0, &proposed), proposed);
}

#[test]
fn an_unrelated_recovering_soldier_does_not_cancel_a_settlers_route() {
    let (g, mut ai, settler, guard, _) = escorted_departure(80, at(8, 6), at(9, 6));
    ai.settler_guards.clear();
    ai.battle_planner_recovering.insert(guard);
    let departure = Action::MoveTo {
        unit: settler,
        to: at(9, 6),
    };
    let proposed = vec![
        departure.clone(),
        Action::MoveTo {
            unit: guard,
            to: at(10, 6),
        },
    ];
    let kept = ai.preserve_unit_actions(&g, 0, &proposed);
    assert!(
        kept.contains(&departure),
        "co-location alone is not an escort assignment"
    );
}

#[test]
fn a_bound_guard_with_a_different_route_does_not_rewrite_the_settlers_plan() {
    let (g, mut ai, settler, guard, _) = escorted_departure(80, at(8, 6), at(9, 6));
    ai.battle_planner_recovering.insert(guard);
    let departure = Action::MoveTo {
        unit: settler,
        to: at(9, 6),
    };
    let proposed = vec![
        departure.clone(),
        Action::MoveTo {
            unit: guard,
            to: at(10, 6),
        },
    ];
    assert!(ai
        .preserve_unit_actions(&g, 0, &proposed)
        .contains(&departure));
}

#[test]
fn shared_single_step_orders_remain_together_during_recovery() {
    let (mut g, mut ai, settler, guard, _) = escorted_departure(80, at(8, 6), at(9, 6));
    ai.battle_planner_recovering.insert(guard);
    let proposed = vec![
        Action::Move {
            unit: settler,
            to: at(9, 6),
        },
        Action::Move {
            unit: guard,
            to: at(9, 6),
        },
    ];
    for action in ai.preserve_unit_actions(&g, 0, &proposed) {
        g.apply(0, &action).expect("preserved orders must execute");
    }
    assert_eq!(g.units[&settler].pos, g.units[&guard].pos);
}

#[test]
fn a_settler_without_movement_does_not_prevent_its_guards_required_escape() {
    let origin = at(10, 7);
    let (mut g, mut ai, settler, guard, proposed) = escorted_departure(35, origin, at(10, 8));
    g.units.get_mut(&settler).unwrap().moves_left = 0.0;
    g.spawn_unit("warrior", 1, at(11, 7));
    assert!(!ai.unit_reply_is_safe(&g, 0, guard));
    for action in ai.preserve_unit_actions(&g, 0, &proposed) {
        // The original Settler order cannot execute. The authoritative turn
        // entry point likewise ignores refused orders and continues the turn.
        let _ = g.apply(0, &action);
    }
    assert_eq!(g.units[&settler].pos, origin);
    assert_ne!(g.units[&guard].pos, origin);
    assert!(ai.unit_reply_is_safe(&g, 0, guard));
}

#[test]
fn full_health_releases_both_original_escort_orders() {
    let (mut g, mut ai, _, guard, proposed) = escorted_departure(80, at(8, 6), at(9, 6));
    ai.battle_planner_recovering.insert(guard);
    g.units.get_mut(&guard).unwrap().hp = 100;
    assert_eq!(ai.preserve_unit_actions(&g, 0, &proposed), proposed);
    assert!(!ai.battle_planner_recovering.contains(&guard));
}
