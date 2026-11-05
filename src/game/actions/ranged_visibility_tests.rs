use super::*;
use crate::name;

fn fixture() -> (Game, u32, u32, Pos) {
    let mut g = Game::new_full(2, 28, 18, 91779, 250, 0, false);
    for uid in g.units.keys().copied().collect::<Vec<_>>() {
        g.remove_unit(uid);
    }
    for tile in g.map.tiles.values_mut() {
        tile.terrain = name!("grassland");
        tile.feature = None;
        tile.hills = false;
        tile.resource = None;
        tile.improvement = None;
    }
    g.at_war.clear();
    g.at_war.insert((0, 1));
    for pid in 0..2 {
        g.players[pid].met.insert(1 - pid);
        g.players[pid].observed_visibility.insert(1 - pid, 0.0);
    }
    let target = (8, 9);
    let attacker = g.spawn_test_unit("archer", 0, (7, 9));
    let defender = g.spawn_test_unit("warrior", 1, target);
    (g, attacker, defender, target)
}

#[test]
fn ranged_visibility_all_directional_pairs_apply_once_to_the_advantaged_side() {
    let (mut g, a, d, target) = fixture();
    let base_ranged = g.ranged_strike_strengths(a, d, target).unwrap();
    let base_melee = g.melee_exchange_strengths(a, d).unwrap();
    for ours in 0_i32..=3 {
        for theirs in 0_i32..=3 {
            g.players[0].observed_visibility.insert(1, f64::from(ours));
            g.players[1]
                .observed_visibility
                .insert(0, f64::from(theirs));
            let ranged = g.ranged_strike_strengths(a, d, target).unwrap();
            let melee = g.melee_exchange_strengths(a, d).unwrap();
            let attack_bonus = 3.0 * f64::from((ours - theirs).max(0));
            let defense_bonus = 3.0 * f64::from((theirs - ours).max(0));
            assert_eq!(ranged.0 - base_ranged.0, attack_bonus);
            assert_eq!(ranged.1 - base_ranged.1, defense_bonus);
            assert_eq!(melee.0 - base_melee.0, attack_bonus);
            assert_eq!(melee.1 - base_melee.1, defense_bonus);
        }
    }
}

#[test]
fn ranged_visibility_stacks_with_wounds_and_fortification_without_double_counting() {
    let (mut g, a, d, target) = fixture();
    for hp in [100, 61, 35] {
        for fortification in [0, 2] {
            let defender = g.units.get_mut(&d).unwrap();
            defender.hp = hp;
            defender.fortify_turns = fortification;
            g.players[1].observed_visibility.insert(0, 0.0);
            let base = g.ranged_strike_strengths(a, d, target).unwrap().1;
            g.players[1].observed_visibility.insert(0, 2.0);
            assert_eq!(
                g.ranged_strike_strengths(a, d, target).unwrap().1,
                base + 6.0
            );
        }
    }
}

#[test]
fn ranged_visibility_nominal_forecast_counts_the_known_defenders_advantage() {
    let (mut g, a, d, target) = fixture();
    let baseline = g
        .nominal_ranged_damage_from_kind(name!("archer"), 0, d, target)
        .unwrap();
    g.players[1].observed_visibility.insert(0, 2.0);
    let pair = g.ranged_strike_strengths(a, d, target).unwrap();
    let nominal = g
        .nominal_ranged_damage_from_kind(name!("archer"), 0, d, target)
        .unwrap();
    assert_eq!(nominal, expected_damage(pair.0, pair.1));
    assert!(nominal < baseline);
}

#[test]
fn ranged_visibility_resolver_uses_the_forecast_and_preserves_the_random_roll() {
    let (mut g, a, d, target) = fixture();
    let mut no_advantage = g.clone();
    g.players[1].observed_visibility.insert(0, 2.0);
    let (attack, defense) = g.ranged_strike_strengths(a, d, target).unwrap();
    let mut expected_rng = g.rng.clone();
    let expected = damage(attack, defense, &mut expected_rng);
    g.do_ranged(0, a, target).expect("adjacent visible shot");
    no_advantage
        .do_ranged(0, a, target)
        .expect("same shot without defender visibility");
    assert_eq!(g.units[&d].hp, 100 - expected);
    assert!(g.units[&d].hp > no_advantage.units[&d].hp);
}
