use super::super::VictoryTarget;
use super::tests::{plan_against, ring_of, walled_city};
use super::*;

/// The doctrine's turn for every unit of `pid`, looped while it acts.
fn play(ai: &mut AdvancedAi, g: &mut Game, pid: usize, plan: &StrategicPlan) {
    ai.rebuild_force_groups(g, pid, plan);
    let mut ids = g.player_unit_ids(pid);
    ids.sort_unstable();
    for uid in ids {
        for _ in 0..8 {
            if !g.units.contains_key(&uid) || g.units[&uid].moves_left <= 0.0 {
                break;
            }
            if ai.force_groups_dirty {
                ai.rebuild_force_groups(g, pid, plan);
                ai.force_groups_dirty = false;
            }
            match ai.siege_doctrine_step(g, pid, uid, plan) {
                Some(true) => {}
                _ => break,
            }
        }
    }
}

/// Live King 20261004T111442Z (game 53): Madrid without walls, five to eight
/// units staged, two Archers shooting, the melee holding the ring. With the
/// gene the ring's melee joins once the force's blows cover the city.
#[test]
fn melee_joins_the_assault_on_an_unwalled_city_its_blows_can_take() {
    let mut outcome = Vec::new();
    for gene in [false, true] {
        let (mut g, cid) = walled_city();
        {
            let city = g.cities.get_mut(&cid).unwrap();
            city.wall_hp = 0;
            city.hp = 120;
        }
        let ring = ring_of(&g, cid);
        for pos in ring.iter().take(4) {
            g.spawn_unit("swordsman", 0, *pos);
        }
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        ai.enable_siege_train();
        if gene {
            ai.enable_breach_assault();
        }
        let plan = plan_against(&g, cid);
        play(&mut ai, &mut g, 0, &plan);
        let city = &g.cities[&cid];
        outcome.push((city.owner, if city.owner == 0 { 0 } else { city.hp }));
    }
    let (off, on) = (outcome[0], outcome[1]);
    assert!(on.0 == 0 || on.1 < off.1, "the assault must take or wound the city: off {off:?}, on {on:?}");
    assert_ne!(off.0, 0, "without the gene the ring holds: {off:?}");
}

/// A lone unit whose blow cannot take the city in two turns waits, and a
/// wall with no ram or tower beside the attacker still stops the assault.
#[test]
fn a_volley_short_of_the_city_or_a_wounded_unit_holds() {
    let (mut g, cid) = walled_city();
    {
        let city = g.cities.get_mut(&cid).unwrap();
        city.wall_hp = 0;
        city.hp = 200;
    }
    let ring = ring_of(&g, cid);
    let lone = g.spawn_unit("warrior", 0, ring[0]);
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_siege_train();
    ai.enable_breach_assault();
    let plan = plan_against(&g, cid);
    play(&mut ai, &mut g, 0, &plan);
    assert_eq!(g.cities[&cid].hp, 200, "one Warrior's blow is far short of 200");
    assert_eq!(g.units[&lone].hp, 100);

    // Four Swordsmen against a standing wall and no ram: the wall still rules.
    let (mut g, cid) = walled_city();
    g.cities.get_mut(&cid).unwrap().hp = 120;
    for pos in ring_of(&g, cid).iter().take(4) {
        g.spawn_unit("swordsman", 0, *pos);
    }
    let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
    ai.enable_siege_train();
    ai.enable_breach_assault();
    let plan = plan_against(&g, cid);
    play(&mut ai, &mut g, 0, &plan);
    assert_eq!(g.cities[&cid].wall_hp, 100, "no melee blow on an unbreached wall");
}
