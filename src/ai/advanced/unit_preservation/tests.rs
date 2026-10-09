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
    let actions = ai.preserve_unit_actions(&g, 0, &[attack.clone()]);
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
        let kept = ai.preserve_unit_actions(&g, 0, &[movement.clone()]);
        assert!(!kept.contains(&movement), "{hp} hp is still recovering");
        assert!(kept.contains(&Action::Fortify { unit: ours }));
    }
    g.units.get_mut(&ours).unwrap().hp = 100;
    assert_eq!(
        ai.preserve_unit_actions(&g, 0, &[movement.clone()]),
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
        ai.preserve_unit_actions(&g, 0, &[action.clone()]),
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
