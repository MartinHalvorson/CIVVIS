use super::*;
use super::super::objective_board::{ObjectiveKey, TaskForce};

fn fixture() -> (Game, AdvancedAi, StrategicPlan, ForceGroup, u32, u32) {
    let mut g = Game::new_full(2, 24, 16, 59006400, 250, 0, false);
    g.units.clear();
    g.barb_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.resource = None;
        tile.owner_city = None;
    }
    g.found_city_for(0, (4, 4), None);
    g.found_city_for(0, (9, 4), None);
    let capital = g.city_at((4, 4)).unwrap();
    let frontier = g.city_at((9, 4)).unwrap();
    let group = ForceGroup {
        id: 17, domain: ForceDomain::Land, units: vec![], anchor: (6, 4),
        objective: (5, 4), focus_target: Some((5, 4)),
        posture: ForcePosture::Engage, readiness: 0.0, local_strength_ratio: 0.4,
    };
    let mut ai = AdvancedAi::targeting(VictoryTarget::Religion);
    ai.objective_board = true;
    ai.enable_anvil_follows_the_defense();
    ai.force_groups = vec![group.clone()];
    ai.objective_board_state.forces.push(TaskForce {
        id: 17, objective_key: ObjectiveKey::Defend(capital), domain: ForceDomain::Land,
        units: vec![], rally: (4, 4), doctrine_state: ForcePosture::Engage,
        aimed_at: (4, 4), formed: g.turn,
    });
    let plan = StrategicPlan { strategy: GrandStrategy::Recovery, target_player: None,
        target_city: None, threatened_city: Some(frontier), desired_cities: 3,
        assessed_turn: g.turn, rush: false };
    (g, ai, plan, group, capital, frontier)
}

#[test]
fn board_defend_beats_the_global_threat_and_contact_tile() {
    let (g, ai, plan, group, capital, _) = fixture();
    assert_eq!(ai.anvil_defense_city(&g, 0, &plan, &group), Some(capital));
}

#[test]
fn withheld_preserves_the_original_global_threat_city() {
    let (g, mut ai, plan, group, _, frontier) = fixture();
    ai.disable_anvil_follows_the_defense();
    assert_eq!(ai.anvil_defense_city(&g, 0, &plan, &group), Some(frontier));
}

#[test]
fn board_disabled_preserves_the_original_selection() {
    let (g, mut ai, plan, group, _, frontier) = fixture();
    ai.objective_board = false;
    assert_eq!(ai.anvil_defense_city(&g, 0, &plan, &group), Some(frontier));
}

#[test]
fn other_missions_are_not_taken_for_the_global_threat() {
    let (g, mut ai, plan, group, capital, _) = fixture();
    for key in [ObjectiveKey::Siege(capital), ObjectiveKey::Relieve(capital),
                ObjectiveKey::Recon(0), ObjectiveKey::Reserve] {
        ai.objective_board_state.forces[0].objective_key = key;
        assert_eq!(ai.anvil_defense_city(&g, 0, &plan, &group), None, "{key:?}");
    }
    ai.objective_board_state.forces.clear();
    assert_eq!(ai.anvil_defense_city(&g, 0, &plan, &group), None);
}

#[test]
fn an_explicit_defend_does_not_need_a_global_threat() {
    let (g, ai, mut plan, group, capital, _) = fixture();
    plan.threatened_city = None;
    assert_eq!(ai.anvil_defense_city(&g, 0, &plan, &group), Some(capital));
}

#[test]
fn a_lost_missing_or_distant_assigned_city_is_rejected() {
    let (mut g, mut ai, plan, mut group, capital, _) = fixture();
    g.cities.get_mut(&capital).unwrap().owner = 1;
    assert_eq!(ai.anvil_defense_city(&g, 0, &plan, &group), None);
    g.cities.get_mut(&capital).unwrap().owner = 0;
    group.anchor = (18, 12);
    assert_eq!(ai.anvil_defense_city(&g, 0, &plan, &group), None);
    group.anchor = (6, 4);
    ai.objective_board_state.forces[0].objective_key = ObjectiveKey::Defend(u32::MAX);
    assert_eq!(ai.anvil_defense_city(&g, 0, &plan, &group), None);
}

#[test]
fn an_unassigned_closer_force_cannot_take_the_defend_force_off_its_city() {
    let (g, mut ai, plan, group, capital, _) = fixture();
    let mut other = group.clone(); other.id = 18; other.anchor = (4, 4);
    ai.force_groups.push(other);
    assert_eq!(ai.anvil_defense_city(&g, 0, &plan, &group), Some(capital));
}

#[test]
fn default_and_toggle_are_explicit() {
    let mut ai = AdvancedAi::targeting(VictoryTarget::Religion);
    assert!(!ai.anvil_follows_the_defense);
    ai.enable_anvil_follows_the_defense(); assert!(ai.anvil_follows_the_defense);
    ai.disable_anvil_follows_the_defense(); assert!(!ai.anvil_follows_the_defense);
}
