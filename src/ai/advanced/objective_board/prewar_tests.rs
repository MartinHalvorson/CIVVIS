use super::tests::{at, conquest, flat_board};
use super::*;
use crate::ai::VictoryTarget;

fn fixture() -> (Game, AdvancedAi, StrategicPlan, u32, Pos) {
    let objective = at(24, 10);
    let approach = at(12, 10);
    let mut game = flat_board(
        20_261_013_070,
        &[at(6, 10), objective, at(6, 18), at(24, 18)],
        false,
    );
    let target = game.city_at(objective).unwrap();
    for col in 10..14 {
        for row in 9..12 {
            game.spawn_test_unit("warrior", 0, at(col, row));
        }
    }
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_objective_board();
    ai.enable_war_policy_via_board();
    (crate::ai::gene("prewar-siege-board").unwrap().enable)(&mut ai);
    let plan = conquest(&game, Some(target));
    (game, ai, plan, target, approach)
}

#[test]
fn a_named_peace_target_gets_a_staging_row_before_declaration() {
    let (game, mut ai, plan, target, _) = fixture();
    ai.rebuild_force_groups(&game, 0, &plan);
    let row = ai
        .objective_board()
        .rows
        .iter()
        .find(|row| row.key == ObjectiveKey::Siege(target))
        .expect("a declaration that asks for staging needs a preparation row first");
    assert_eq!(row.state, RowState::Staging);
    assert!(!game.is_at_war(0, 1), "allocation cannot declare a war");
}

#[test]
fn the_preparation_force_musters_on_a_legal_near_side_ring() {
    let (game, mut ai, plan, target, approach) = fixture();
    ai.rebuild_force_groups(&game, 0, &plan);
    let force = ai
        .objective_board()
        .forces
        .iter()
        .find(|force| force.objective_key == ObjectiveKey::Siege(target))
        .expect("available field soldiers must prepare instead of remaining reserve");
    let objective = game.cities[&target].pos;
    assert!(!force.units.is_empty());
    assert!((3..=5).contains(&game.wdist(force.rally, objective)));
    assert!(game.wdist(force.rally, approach) < game.wdist(objective, approach));
    assert!(game
        .map
        .get(force.rally)
        .unwrap()
        .owner_city
        .and_then(|city| game.cities.get(&city))
        .is_none_or(|city| city.owner == 0));
    let group = ai
        .force_groups
        .iter()
        .find(|group| group.id == force.id)
        .unwrap();
    assert_eq!(group.posture, ForcePosture::Muster);
    assert!(!game.is_at_war(0, 1));
}

fn no_preparation(game: &Game, ai: &mut AdvancedAi, plan: &StrategicPlan, target: u32) {
    ai.rebuild_force_groups(game, 0, plan);
    assert!(!ai
        .objective_board()
        .rows
        .iter()
        .any(|row| row.key == ObjectiveKey::Siege(target)));
}

#[test]
fn preparation_is_off_by_default_and_can_be_disabled_through_the_registry() {
    let (game, mut ai, plan, target, _) = fixture();
    let policy = crate::ai::gene("prewar-siege-board").unwrap();
    assert_eq!(policy.kind, crate::ai::Kind::OptIn);
    assert!(!AdvancedAi::new().prewar_siege_board);
    (policy.disable)(&mut ai);
    no_preparation(&game, &mut ai, &plan, target);
}

#[test]
fn preparation_does_not_enable_the_objective_board() {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_prewar_siege_board();
    assert!(!ai.objective_board);
}

#[test]
fn preparation_requires_an_active_domination_target() {
    for target_kind in [None, Some(VictoryTarget::Science)] {
        let (game, mut ai, plan, target, _) = fixture();
        ai.victory_target = target_kind;
        no_preparation(&game, &mut ai, &plan, target);
    }
    let (mut game, mut ai, plan, target, _) = fixture();
    game.victory_conditions.domination = false;
    no_preparation(&game, &mut ai, &plan, target);
}

#[test]
fn preparation_requires_the_named_conquest_city_and_its_owner() {
    for case in 0..3 {
        let (game, mut ai, mut plan, target, _) = fixture();
        match case {
            0 => plan.strategy = GrandStrategy::Expansion,
            1 => plan.target_city = None,
            _ => plan.target_player = Some(2),
        }
        no_preparation(&game, &mut ai, &plan, target);
    }
}

#[test]
fn preparation_respects_relationship_commitments() {
    for case in 0..3 {
        let (mut game, mut ai, plan, target, _) = fixture();
        match case {
            0 => {
                game.players[0].friends_until.insert(1, game.turn + 20);
            }
            1 => {
                game.players[0].alliances.insert(
                    1,
                    crate::game::AllianceState {
                        kind: "military".to_string(),
                        points: 0.0,
                        level: 1,
                        ends: game.turn + 20,
                    },
                );
            }
            _ => {
                game.players[0].team = Some(7);
                game.players[1].team = Some(7);
            }
        }
        no_preparation(&game, &mut ai, &plan, target);
    }
}

#[test]
fn preparation_requires_a_living_met_major() {
    for case in 0..4 {
        let (mut game, mut ai, plan, target, _) = fixture();
        match case {
            0 => {
                game.players[0].met.remove(&1);
            }
            1 => game.players[1].alive = false,
            2 => game.players[1].is_minor = true,
            _ => game.players[1].is_barbarian = true,
        }
        no_preparation(&game, &mut ai, &plan, target);
    }
}

#[test]
fn preparation_waits_while_another_major_war_is_active() {
    let (mut game, mut ai, plan, target, _) = fixture();
    super::tests::war(&mut game, 0, 2);
    no_preparation(&game, &mut ai, &plan, target);
}

#[test]
fn a_blocked_unknown_or_foreign_ring_does_not_get_a_preparation_row() {
    for case in 0..4 {
        let (mut game, mut ai, plan, target, _) = fixture();
        let objective = game.cities[&target].pos;
        for pos in game.wdisk(objective, 5) {
            if !(3..=5).contains(&game.wdist(pos, objective)) {
                continue;
            }
            match case {
                0 => game.map.tiles.get_mut(&pos).unwrap().owner_city = Some(target),
                1 => game.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("coast"),
                2 => game.map.tiles.get_mut(&pos).unwrap().terrain = crate::name!("mountain"),
                _ => {
                    game.players[0].explored.remove(&pos);
                }
            }
        }
        no_preparation(&game, &mut ai, &plan, target);
    }
}

#[test]
fn a_far_side_opening_does_not_license_crossing_the_target_city() {
    let (mut game, mut ai, plan, target, approach) = fixture();
    let objective = game.cities[&target].pos;
    for pos in game.wdisk(objective, 5) {
        if (3..=5).contains(&game.wdist(pos, objective))
            && game.wdist(pos, approach) < game.wdist(approach, objective)
        {
            game.map.tiles.get_mut(&pos).unwrap().owner_city = Some(target);
        }
    }
    no_preparation(&game, &mut ai, &plan, target);
}

#[test]
fn the_rally_avoids_observed_foreign_occupants() {
    let (mut game, ai, _, target, approach) = fixture();
    let objective = game.cities[&target].pos;
    let visible = ai.battlefront_visibility(&game, 0);
    let first = ai
        .prewar_rally(&game, 0, objective, approach, &visible)
        .unwrap();
    let mut ai = ai;
    ai.battlefront_observation = false;
    game.spawn_test_unit("warrior", 1, first);
    let alternate = ai
        .prewar_rally(&game, 0, objective, approach, &visible)
        .unwrap();
    assert_ne!(alternate, first);
    assert!(!game
        .units
        .values()
        .any(|unit| unit.owner != 0 && unit.pos == alternate));
}

#[test]
fn preparation_moves_the_column_toward_its_rally_without_declaring() {
    let (mut game, mut ai, plan, target, _) = fixture();
    ai.rebuild_force_groups(&game, 0, &plan);
    let force = ai
        .objective_board()
        .forces
        .iter()
        .find(|force| force.objective_key == ObjectiveKey::Siege(target))
        .unwrap();
    let group = ai
        .force_groups
        .iter()
        .find(|group| group.id == force.id)
        .unwrap()
        .clone();
    let mut steps = Vec::new();
    for uid in &group.units {
        let before = game.units[uid].pos;
        let moves = game.unit_max_moves(*uid);
        game.units.get_mut(uid).unwrap().moves_left = moves;
        ai.coordinated_tactical_step(&mut game, 0, *uid, &group, &[], false);
        let after = game.units[uid].pos;
        steps.push((
            *uid,
            before,
            after,
            game.wdist(before, group.anchor),
            game.wdist(after, group.anchor),
        ));
        assert!(game
            .map
            .get(after)
            .unwrap()
            .owner_city
            .is_none_or(|cid| game.cities[&cid].owner == 0));
    }
    // Cohesion can move one member sideways or backward. The column still
    // has to approach its rally over the military pass as a whole.
    let before: i32 = steps.iter().map(|step| step.3).sum();
    let after: i32 = steps.iter().map(|step| step.4).sum();
    assert!(after < before, "rally {:?}, steps {steps:?}", group.anchor);
    assert!(!game.is_at_war(0, 1));
}

#[test]
fn the_existing_staged_strength_gate_still_blocks_a_remote_army() {
    let (game, mut ai, plan, _, _) = fixture();
    ai.rebuild_force_groups(&game, 0, &plan);
    let hold = ai
        .war_policy_declaration(&game, 0, 1, &plan)
        .unwrap()
        .unwrap_err();
    assert!(hold.contains("staged on its ring"), "{hold}");
}

#[test]
fn urgent_home_defense_keeps_the_army_and_the_declaration_veto() {
    let (mut game, mut ai, plan, target, _) = fixture();
    game.players[2].is_barbarian = true;
    game.barb_pid = Some(2);
    super::tests::war(&mut game, 0, 2);
    let home = game.player_city_ids(0)[0];
    game.cities.get_mut(&home).unwrap().hp = 10;
    for uid in game.player_unit_ids(0) {
        game.units.get_mut(&uid).unwrap().pos = at(18, 8);
    }
    for _ in 0..10 {
        game.spawn_test_unit("musketman", 2, at(7, 10));
    }
    ai.battlefront_observation = false;
    ai.rebuild_force_groups(&game, 0, &plan);
    let board = ai.objective_board();
    let defense = board
        .rows
        .iter()
        .position(|row| row.key == ObjectiveKey::Defend(home))
        .unwrap();
    let preparation = board
        .rows
        .iter()
        .position(|row| row.key == ObjectiveKey::Siege(target))
        .unwrap();
    assert!(board.rows[defense].urgent);
    assert!(defense < preparation);
    let defenders = board
        .forces
        .iter()
        .filter(|force| force.objective_key == ObjectiveKey::Defend(home))
        .map(|force| force.units.len())
        .sum::<usize>();
    assert_eq!(defenders, game.player_unit_ids(0).len());
    let hold = ai
        .war_policy_declaration(&game, 0, 1, &plan)
        .unwrap()
        .unwrap_err();
    assert!(hold.contains("Defend row"), "{hold}");
}

#[test]
fn active_war_keeps_the_existing_siege_doctrine() {
    let (mut game, mut ai, plan, target, _) = fixture();
    super::tests::war(&mut game, 0, 1);
    ai.rebuild_force_groups(&game, 0, &plan);
    let row = ai
        .objective_board()
        .rows
        .iter()
        .find(|row| row.key == ObjectiveKey::Siege(target))
        .unwrap();
    assert_eq!(row.state, RowState::Open);
}
