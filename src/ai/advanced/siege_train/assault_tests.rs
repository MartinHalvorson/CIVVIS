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
/// gene the ring's melee joins once the force's blows can take the city.
#[test]
fn melee_joins_the_assault_on_an_unwalled_city_its_blows_can_take() {
    let mut outcome = Vec::new();
    for gene in [false, true] {
        let (mut g, cid) = walled_city();
        {
            let city = g.cities.get_mut(&cid).unwrap();
            city.wall_hp = 0;
            city.hp = 200;
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
    // The baseline may already strike a breached city (the taker's own
    // pressure); the gene must do at least as well and make progress on a
    // full-health one. The Stage test below is the case the baseline misses.
    let (off, on) = (outcome[0], outcome[1]);
    assert!(on.0 == 0 || on.1 < 200, "the assault makes progress: {on:?}");
    assert!(on.0 == 0 || (off.0 != 0 && on.1 <= off.1), "never worse than without: off {off:?}, on {on:?}");
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

/// Live King civvis-20261004T114858Z (game 55), Nicomedia at turn 200:
/// 41/200 behind fallen walls, a Llanero beside it, the siege in Stage — and
/// the Llanero walked away. Under the gene the Stage step strikes the city.
#[test]
fn a_stage_siege_finishes_a_dying_breached_city_beside_it() {
    let mut outcome = Vec::new();
    for gene in [false, true] {
        let (mut g, cid) = walled_city();
        {
            let city = g.cities.get_mut(&cid).unwrap();
            city.wall_hp = 0;
            city.hp = 41;
        }
        let swordsman = g.spawn_unit("swordsman", 0, ring_of(&g, cid)[0]);
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        ai.enable_siege_train();
        if gene {
            ai.enable_breach_assault();
        }
        let plan = plan_against(&g, cid);
        let city = CityView::of(&g, cid).unwrap();
        ai.siege_stage_step(&mut g, 0, swordsman, &city, &plan);
        let city = &g.cities[&cid];
        outcome.push((city.owner, city.hp));
    }
    assert_eq!(outcome[0], (1, 41), "without the gene Stage leaves the city alone");
    assert!(
        outcome[1].0 == 0 || outcome[1].1 < 41,
        "with the gene the blow lands: {:?}",
        outcome[1]
    );
}

/// See `closing_in`: a healthy melee unit off the ring, within
/// `CLOSING_REACH` of a city with no wall standing and a free tile beside
/// it, is one the assault can call in; walls, wounds, distance or a full
/// ring rule it out.
#[test]
fn closing_in_calls_healthy_melee_off_the_ring_to_an_unwalled_city() {
    let (mut g, cid) = walled_city();
    g.cities.get_mut(&cid).unwrap().wall_hp = 0;
    let centre = g.cities[&cid].pos;
    let at = |g: &Game, distance: i32| {
        let mut tiles: Vec<Pos> = g
            .wdisk(centre, distance)
            .into_iter()
            .filter(|pos| {
                g.wdist(*pos, centre) == distance
                    && g.unit_ids_at(*pos).is_empty()
                    && g.map.get(*pos).is_some_and(|tile| !g.rules.is_water(tile))
            })
            .collect();
        tiles.sort();
        tiles[0]
    };
    let near = g.spawn_unit("horseman", 0, at(&g, 3));
    let far = g.spawn_unit("horseman", 0, at(&g, 6));
    let ai = AdvancedAi::targeting(VictoryTarget::Domination);
    let view = |g: &Game| CityView::of(g, cid).unwrap();
    assert!(
        ai.closing_in(&g, near, &view(&g)),
        "three tiles out, healthy"
    );
    assert!(!ai.closing_in(&g, far, &view(&g)), "beyond the reach");
    g.units.get_mut(&near).unwrap().hp = ASSAULT_MIN_HP - 1;
    assert!(!ai.closing_in(&g, near, &view(&g)), "wounded");
    g.units.get_mut(&near).unwrap().hp = 100;
    g.cities.get_mut(&cid).unwrap().wall_hp = 100;
    assert!(!ai.closing_in(&g, near, &view(&g)), "a wall standing");
    g.cities.get_mut(&cid).unwrap().wall_hp = 0;
    for pos in ring_of(&g, cid) {
        if g.unit_ids_at(pos).is_empty() {
            g.spawn_unit("warrior", 0, pos);
        }
    }
    assert!(
        !ai.closing_in(&g, near, &view(&g)),
        "no free tile beside the city"
    );
}

/// Under `breach-assault-closes-in`, horsemen three tiles from an unwalled
/// city do at least as well as without the gene, and against a standing
/// wall the turn is unchanged.
#[test]
fn closing_in_never_does_worse_and_leaves_a_walled_city_alone() {
    let run = |closes_in: bool, walls: i32| {
        let (mut g, cid) = walled_city();
        {
            let city = g.cities.get_mut(&cid).unwrap();
            city.wall_hp = walls;
            city.hp = 120;
        }
        let centre = g.cities[&cid].pos;
        let mut posts: Vec<Pos> = g
            .wdisk(centre, 3)
            .into_iter()
            .filter(|pos| {
                g.wdist(*pos, centre) == 3
                    && g.unit_ids_at(*pos).is_empty()
                    && g.city_at(*pos).is_none()
                    && g.map.get(*pos).is_some_and(|tile| !g.rules.is_water(tile))
            })
            .collect();
        posts.sort();
        for pos in posts.iter().take(4) {
            g.spawn_unit("horseman", 0, *pos);
        }
        let mut ai = AdvancedAi::targeting(VictoryTarget::Domination);
        ai.enable_siege_train();
        ai.enable_breach_assault();
        if closes_in {
            ai.enable_breach_assault_closes_in();
        }
        let plan = plan_against(&g, cid);
        play(&mut ai, &mut g, 0, &plan);
        let city = &g.cities[&cid];
        let mut units: Vec<(Pos, i32)> = g
            .player_unit_ids(0)
            .into_iter()
            .map(|uid| (g.units[&uid].pos, g.units[&uid].hp))
            .collect();
        units.sort();
        (city.owner, city.hp, city.wall_hp, units)
    };
    let (off, on) = (run(false, 0), run(true, 0));
    assert!(
        on.0 == 0 || (off.0 != 0 && on.1 <= off.1),
        "never worse: off {:?}, on {:?}",
        (off.0, off.1),
        (on.0, on.1)
    );
    assert_eq!(
        run(true, 100),
        run(false, 100),
        "a standing wall: the same turn"
    );
}
