use super::*;

fn at(x: i32, y: i32) -> Pos {
    crate::hex::offset_to_axial(x, y)
}

fn breach() -> (Game, u32, u32, Action) {
    let mut g =
        crate::doctrine::build(crate::doctrine::position("the_reserve").unwrap(), 3).unwrap();
    g.tactics.heal = true;
    for pid in 0..2 {
        for uid in g.player_unit_ids(pid) {
            g.remove_unit(uid);
        }
    }
    let ours = g.spawn_unit("modern_armor", 0, at(10, 7));
    g.units.get_mut(&ours).unwrap().hp = 73;
    let city = g.found_city_for(1, at(11, 7), None);
    g.cities.get_mut(&city).unwrap().hp = 1;
    g.cities.get_mut(&city).unwrap().wall_hp = 0;
    // Taking this city must not remove the other player's entire army or
    // manufacture safety through an observed-world elimination.
    g.found_city_for(1, at(19, 12), None);
    let attack = Action::Attack {
        unit: ours,
        target: g.cities[&city].pos,
    };
    std::sync::Arc::make_mut(&mut g.host_previews).insert(
        (ours, g.cities[&city].pos, false),
        crate::game::HostStrikePreview {
            attacker_strength: 95.0,
            defender_strength: 92.0,
            damage_to_defender: 23,
            ..Default::default()
        },
    );
    (g, ours, city, attack)
}

fn policy(recovering: Option<u32>) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_unit_preservation();
    if let Some(uid) = recovering {
        ai.battle_planner_recovering.insert(uid);
    }
    ai
}

#[test]
fn recovering_adjacent_armor_keeps_a_survivable_guaranteed_city_capture() {
    let (g, ours, city, attack) = breach();
    let outcome = replay(&g, 0, std::slice::from_ref(&attack), &BTreeSet::new());
    assert_eq!(outcome.board.cities[&city].owner, 0);
    assert!(outcome.uncertain_captures.is_empty());
    assert!(policy(None).preservation_finishing_safe(&g, 0, std::slice::from_ref(&attack)));
    let mut ai = policy(Some(ours));
    let kept = ai.preserve_unit_actions(&g, 0, std::slice::from_ref(&attack));
    assert!(
        kept.contains(&attack),
        "recovery must not erase this safe capture"
    );
    let mut executed = g.clone();
    for action in &kept {
        executed.apply(0, action).unwrap();
    }
    assert_eq!(executed.cities[&city].owner, 0);
    assert_eq!(executed.units[&ours].pos, g.cities[&city].pos);
    assert!(executed.units[&ours].hp > 0);
    assert_eq!(g.cities[&city].owner, 1);
    assert_eq!(g.units[&ours].hp, 73);
}

#[test]
fn the_finisher_admission_keeps_the_same_capture_available_during_recovery() {
    let (g, ours, _, attack) = breach();
    assert!(policy(Some(ours)).preservation_finishing_safe(&g, 0, std::slice::from_ref(&attack),));
}

#[test]
fn a_nonrecovering_unit_keeps_the_existing_capture() {
    let (g, _, city, attack) = breach();
    let mut ai = policy(None);
    let kept = ai.preserve_unit_actions(&g, 0, std::slice::from_ref(&attack));
    assert!(kept.contains(&attack));
    let mut executed = g.clone();
    for action in &kept {
        executed.apply(0, action).unwrap();
    }
    assert_eq!(executed.cities[&city].owner, 0);
}

#[test]
fn a_ready_city_does_not_release_an_ordinary_recovery_move() {
    let (g, ours, _, _) = breach();
    let movement = Action::MoveTo {
        unit: ours,
        to: at(9, 7),
    };
    let mut ai = policy(Some(ours));
    let kept = ai.preserve_unit_actions(&g, 0, std::slice::from_ref(&movement));
    assert!(!kept.contains(&movement));
    assert!(ai.battle_planner_recovering.contains(&ours));
}

#[test]
fn a_city_without_a_guaranteed_capture_keeps_the_unit_recovering() {
    let (mut g, ours, city, attack) = breach();
    g.cities.get_mut(&city).unwrap().hp = 200;
    let mut ai = policy(Some(ours));
    let kept = ai.preserve_unit_actions(&g, 0, std::slice::from_ref(&attack));
    assert!(!kept.contains(&attack));
    assert!(!ai.preservation_finishing_safe(&g, 0, std::slice::from_ref(&attack)));
}

#[test]
fn standing_walls_do_not_release_the_recovery_latch() {
    let (mut g, ours, city, attack) = breach();
    g.cities.get_mut(&city).unwrap().wall_hp = 50;
    let mut ai = policy(Some(ours));
    let kept = ai.preserve_unit_actions(&g, 0, std::slice::from_ref(&attack));
    assert!(!kept.contains(&attack));
}

#[test]
fn a_lethal_native_retaliation_bound_still_withholds_the_capture() {
    let (mut g, ours, city, attack) = breach();
    let preview = std::sync::Arc::make_mut(&mut g.host_previews)
        .get_mut(&(ours, g.cities[&city].pos, false))
        .unwrap();
    preview.attacker_strength = 40.0;
    preview.defender_strength = 140.0;
    assert!(!policy(None).preservation_finishing_safe(&g, 0, std::slice::from_ref(&attack)));
    let mut ai = policy(Some(ours));
    let kept = ai.preserve_unit_actions(&g, 0, std::slice::from_ref(&attack));
    assert!(!kept.contains(&attack));
    assert!(ai.battle_planner_recovering.contains(&ours));
}

#[test]
fn an_approach_from_two_tiles_away_keeps_the_recovery_latch() {
    let (mut g, ours, _, attack) = breach();
    g.relocate(ours, at(9, 7));
    let movement = Action::MoveTo {
        unit: ours,
        to: at(10, 7),
    };
    let mut ai = policy(Some(ours));
    let kept = ai.preserve_unit_actions(&g, 0, &[movement, attack.clone()]);
    assert!(!kept.contains(&attack));
}

#[test]
fn a_ranged_strike_cannot_release_recovery_for_city_occupation() {
    let (mut g, _, city, _) = breach();
    let archer = g.spawn_unit("archer", 0, at(10, 8));
    g.units.get_mut(&archer).unwrap().hp = 73;
    let shot = Action::Ranged {
        unit: archer,
        target: g.cities[&city].pos,
    };
    let mut ai = policy(Some(archer));
    let kept = ai.preserve_unit_actions(&g, 0, std::slice::from_ref(&shot));
    assert!(!kept.contains(&shot));
    assert!(!ai.preservation_finishing_safe(&g, 0, std::slice::from_ref(&shot)));
}

#[test]
fn a_peaceful_city_does_not_release_recovery_or_start_a_war() {
    let (mut g, ours, _, attack) = breach();
    g.at_war.remove(&(0, 1));
    assert!(!g.is_at_war(0, 1));
    let mut ai = policy(Some(ours));
    let kept = ai.preserve_unit_actions(&g, 0, std::slice::from_ref(&attack));
    assert!(!kept.contains(&attack));
    assert!(!g.is_at_war(0, 1));
}

#[test]
fn a_guaranteed_capture_does_not_bypass_the_enemy_reply() {
    let (mut g, ours, city, attack) = breach();
    g.spawn_unit("modern_armor", 1, at(12, 7));
    g.spawn_unit("modern_armor", 1, at(12, 8));
    let outcome = replay(&g, 0, std::slice::from_ref(&attack), &BTreeSet::new());
    assert_eq!(outcome.board.cities[&city].owner, 0);
    assert!(outcome.uncertain_captures.is_empty());
    assert!(!policy(None).preservation_finishing_safe(&g, 0, std::slice::from_ref(&attack)));
    let mut ai = policy(Some(ours));
    let kept = ai.preserve_unit_actions(&g, 0, std::slice::from_ref(&attack));
    assert!(!kept.contains(&attack));
}

#[test]
fn an_attacker_without_movement_cannot_release_recovery() {
    let (mut g, ours, city, attack) = breach();
    g.units.get_mut(&ours).unwrap().moves_left = 0.0;
    assert!(g.clone().apply(0, &attack).is_err());
    let mut ai = policy(Some(ours));
    let kept = ai.preserve_unit_actions(&g, 0, std::slice::from_ref(&attack));
    assert!(!kept.contains(&attack));
    assert_eq!(g.cities[&city].owner, 1);
}
