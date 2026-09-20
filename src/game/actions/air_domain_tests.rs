use super::*;
use crate::ai::{AdvancedAi, VictoryTarget};
use crate::name;

fn fixture(kind: &str) -> (Game, u32, u32, Pos) {
    let mut g = Game::new_full(2, 40, 24, 936004, 1000, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    g.barb_camps.clear();
    g.barb_naval_camps.clear();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    g.found_city_for(0, (6, 12), None);
    g.found_city_for(1, (18, 12), None);
    let target = (8, 12);
    let attacker = g.spawn_test_unit(kind, 0, (6, 12));
    let defender = g.spawn_test_unit("warrior", 1, target);
    for pid in 0..2 {
        g.players[pid].met.insert(1 - pid);
        g.players[pid].explored.extend(g.map.tiles.keys().copied());
    }
    g.at_war.clear();
    g.at_war.insert((0, 1));
    g.turn = 150;
    g.current = 0;
    (g, attacker, defender, target)
}

#[test]
fn aircraft_cannot_apply_a_ground_ranged_strike_or_spend_its_attack_on_one() {
    for kind in ["bomber", "fighter", "jet_bomber", "jet_fighter"] {
        let (mut g, attacker, defender, target) = fixture(kind);
        let before = (
            g.units[&attacker].moves_left,
            g.units[&attacker].attacks_left,
            g.units[&defender].hp,
        );
        let result = g.apply(
            0,
            &Action::Ranged {
                unit: attacker,
                target,
            },
        );
        assert!(result.is_err(), "{kind} accepted a ground ranged strike");
        assert_eq!(
            (
                g.units[&attacker].moves_left,
                g.units[&attacker].attacks_left,
                g.units[&defender].hp
            ),
            before
        );
        let air = Action::AirStrike {
            unit: attacker,
            target,
        };
        assert!(g.legal_actions(0).contains(&air));
        g.apply(0, &air)
            .expect("the actual air operation remains legal");
        assert!(g.units.get(&defender).is_none_or(|unit| unit.hp < before.2));
    }
}

#[test]
fn tactical_ranged_legality_excludes_aircraft_but_keeps_ground_and_naval_shooters() {
    for (kind, expected) in [
        ("bomber", false),
        ("fighter", false),
        ("archer", true),
        ("battleship", true),
    ] {
        let (g, attacker, _, target) = fixture(kind);
        let visible = g.player_vision_frame(0);
        assert_eq!(
            g.ranged_order_is_legal(0, attacker, target, &visible, &g.visibility_viewers(0)),
            expected,
            "{kind}"
        );
    }
}

#[test]
fn the_native_frame_planner_uses_an_air_strike_for_a_bomber_finishing_blow() {
    let (mut g, bomber, defender, target) = fixture("bomber");
    g.units.get_mut(&defender).unwrap().hp = 10;
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    let (finishing, ordinary_begin) = crate::ai::player::plan_frame(
        &mut ai,
        &mut g,
        0,
        &[(bomber, 10158084)].into_iter().collect(),
    );
    let actions: Vec<_> = finishing
        .actions
        .iter()
        .cloned()
        .chain(
            g.log
                .since(ordinary_begin)
                .filter(|(pid, _)| *pid == 0)
                .map(|(_, action)| action.clone()),
        )
        .collect();
    assert!(
        !actions
            .iter()
            .any(|action| matches!(action, Action::Ranged { unit, .. } if *unit==bomber)),
        "aircraft escaped through a ground strike: {actions:?}"
    );
    assert!(
        actions.contains(&Action::AirStrike {
            unit: bomber,
            target
        }),
        "the bomber's attack must remain actionable: {actions:?}"
    );
}
