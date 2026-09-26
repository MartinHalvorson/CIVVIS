use super::*;

fn at(x: i32, y: i32) -> Pos {
    crate::hex::offset_to_axial(x, y)
}

fn pair(peer_kind: &str, peer_first: bool) -> (Game, u32, u32) {
    let mut g = crate::doctrine::build(
        crate::doctrine::position("the_reserve").expect("fixture"),
        3,
    )
    .expect("buildable");
    g.tactics.heal = true;
    for pid in 0..2 {
        for uid in g.player_unit_ids(pid) {
            g.remove_unit(uid);
        }
    }
    let pos = at(10, 6);
    let (carrier, peer) = if peer_first {
        let peer = g.spawn_test_unit(peer_kind, 0, pos);
        (g.spawn_test_unit("trebuchet", 0, pos), peer)
    } else {
        let carrier = g.spawn_test_unit("trebuchet", 0, pos);
        (carrier, g.spawn_test_unit(peer_kind, 0, pos))
    };
    g.apply(
        0,
        &Action::LinkUnits {
            unit: carrier,
            with: peer,
        },
    )
    .unwrap();
    g.units.get_mut(&carrier).unwrap().hp = 31;
    (g, carrier, peer)
}

fn plan(g: &Game) -> StrategicPlan {
    StrategicPlan {
        strategy: super::super::GrandStrategy::Conquest,
        target_player: Some(1),
        target_city: None,
        threatened_city: None,
        desired_cities: 3,
        assessed_turn: g.turn,
        rush: false,
    }
}

fn planner() -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_battle_planner_2();
    ai.enable_siege_train();
    ai
}

#[test]
fn wounded_siege_support_pair_holds_together_and_stays_out_of_the_unit_ladder() {
    // Native 20260926T190901Z turn115: a 31-HP trebuchet linked to a siege
    // tower skipped recovery and walked into a lethal district strike.
    for peer_first in [false, true] {
        let (mut g, carrier, peer) = pair("siege_tower", peer_first);
        let mut ai = planner();
        let plan = plan(&g);
        let before = g.units[&carrier].pos;
        ai.plan_battle(&mut g, 0, &plan);
        assert!(ai.battle_planner_claims(carrier));
        assert!(ai.battle_planner_claims(peer));
        assert!(g.units[&carrier].fortified);
        assert!(ai.battle_planner_recovering.contains(&carrier));
        for uid in [peer, carrier] {
            ai.advanced_military_step(&mut g, 0, uid, &plan);
        }
        assert_eq!(g.units[&carrier].pos, before);
        assert_eq!(g.units[&peer].pos, before);
        assert_eq!(g.units[&carrier].linked_to, Some(peer));
        assert_eq!(g.units[&peer].linked_to, Some(carrier));
    }
}

#[test]
fn recovery_moves_both_members_out_of_enemy_city_fire() {
    let (mut g, carrier, peer) = pair("siege_tower", true);
    let city = g.found_city_for(1, at(10, 8), None);
    g.cities.get_mut(&city).unwrap().wall_hp = 200;
    std::sync::Arc::make_mut(&mut g.observed_city_ranged_strength).insert(city, 70.0);
    let before = g.units[&carrier].pos;
    let mut field = DangerField::new(&g, 0);
    assert!(field.danger(before, carrier) >= 31.0);
    let mut ai = planner();
    ai.rotate_wounded(&mut g, 0, &mut field, &BTreeSet::new(), &BTreeSet::new());
    let after = g.units[&carrier].pos;
    assert_ne!(after, before);
    assert_eq!(g.units[&peer].pos, after);
    assert!(field.danger(after, carrier) <= NO_DANGER);
    assert!(ai.battle_planner_claims(carrier) && ai.battle_planner_claims(peer));
    assert_eq!(g.units[&carrier].linked_to, Some(peer));
    assert_eq!(g.units[&peer].linked_to, Some(carrier));
}

#[test]
fn a_recovering_formation_in_a_city_waits_until_the_carrier_can_return() {
    for hp in [60, RETURN_HP - 1, RETURN_HP] {
        let (mut g, carrier, peer) = pair("siege_tower", false);
        g.found_city_for(0, g.units[&carrier].pos, None);
        g.units.get_mut(&carrier).unwrap().hp = hp;
        let mut ai = planner();
        ai.battle_planner_recovering.insert(carrier);
        let plan = plan(&g);
        ai.plan_battle(&mut g, 0, &plan);
        assert_eq!(ai.battle_planner_claims(carrier), hp < RETURN_HP);
        assert_eq!(ai.battle_planner_claims(peer), hp < RETURN_HP);
        assert_eq!(
            ai.battle_planner_recovering.contains(&carrier),
            hp < RETURN_HP
        );
    }
}

#[test]
fn civilian_escort_commitments_are_not_taken_over_by_combat_recovery() {
    for peer_kind in ["settler", "builder", "missionary"] {
        let (mut g, carrier, peer) = pair(peer_kind, false);
        let mut ai = planner();
        let mut field = DangerField::new(&g, 0);
        ai.rotate_wounded(&mut g, 0, &mut field, &BTreeSet::new(), &BTreeSet::new());
        assert!(!ai.battle_planner_claims(carrier));
        assert!(!ai.battle_planner_claims(peer));
        assert!(!g.units[&carrier].fortified);
    }
}
