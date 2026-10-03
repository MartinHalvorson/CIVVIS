//! `siege-needs-a-breaker`, and the live seat's siege gun firing after it
//! moves. Live King civvis-20261003T135713Z held Kwadukuza's ring with
//! Knights and Men-at-Arms against Medieval Walls while its Bombards healed
//! or walked in circles and its Siege Tower stood seven tiles off.

use std::sync::Arc;

use super::super::ForcePosture;
use super::tests::{at_distance, plan_against, ring_of, walled_city};
use super::*;

/// The board as the live seat sees it: every tile observed by the host.
fn mirrored(g: &mut Game) {
    g.host_observed = Arc::new(g.map.tiles.keys().copied().collect());
}

/// Open grassland everywhere but the city tile, so steps cost one move and
/// every line is clear.
fn flatten(g: &mut Game, cid: u32) {
    let city = g.cities[&cid].pos;
    for tile in g.map.tiles.values_mut() {
        if tile.pos != city {
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
        }
    }
}

/// `walled_city` behind Medieval Walls at full strength, on open ground.
fn medieval_city() -> (Game, u32) {
    let (mut g, cid) = walled_city();
    flatten(&mut g, cid);
    g.map_script = crate::setup::MapScript::Pangaea;
    g.turn = 30;
    g.at_war.insert((0, 1));
    let city = g.cities.get_mut(&cid).unwrap();
    city.buildings = vec![crate::name!("walls"), crate::name!("medieval_walls")];
    // The position states its wall pool as observed; read the buildings.
    Arc::make_mut(&mut g.observed_city_max_wall_hp).remove(&cid);
    let max = g.city_max_wall_hp(&g.cities[&cid]);
    g.cities.get_mut(&cid).unwrap().wall_hp = max;
    assert_eq!(max, 200);
    (g, cid)
}

/// A land force group on the city, as the board projects one.
fn group_on(g: &Game, cid: u32, units: &[u32]) -> ForceGroup {
    ForceGroup {
        id: units[0],
        domain: ForceDomain::Land,
        units: units.to_vec(),
        anchor: g.units[&units[0]].pos,
        objective: g.cities[&cid].pos,
        focus_target: None,
        posture: ForcePosture::Advance,
        readiness: 1.0,
        local_strength_ratio: 2.0,
    }
}

fn reducing(ai: &mut AdvancedAi, cid: u32) {
    ai.sieges.insert(
        cid,
        Siege {
            stage: SiegeStage::Reduce,
            taker: None,
            entered: 25,
            assessed: 29,
            posts: BTreeMap::new(),
            short_since: None,
        },
    );
}

/// The gun two tiles out on a line to the city, stepped there from three.
fn a_step_into_range(g: &Game, cid: u32) -> (Pos, Pos) {
    let city = g.cities[&cid].pos;
    for post in at_distance(g, cid, 2) {
        if !g.line_of_sight_from(post, city) {
            continue;
        }
        if let Some(from) = g
            .nbrs(post)
            .into_iter()
            .find(|from| g.wdist(*from, city) == 3 && g.map.get(*from).is_some_and(|t| g.rules.is_passable(t) && !g.rules.is_water(t)))
        {
            return (from, post);
        }
    }
    panic!("the storming board has an approach");
}

#[test]
fn the_live_seat_fires_a_siege_gun_after_it_moves_and_a_native_board_does_not() {
    for (live, expect_fire) in [(false, false), (true, true)] {
        let (mut g, cid) = walled_city();
        flatten(&mut g, cid);
        g.at_war.insert((0, 1));
        if live {
            mirrored(&mut g);
        }
        let (from, post) = a_step_into_range(&g, cid);
        let gun = g.spawn_unit("catapult", 0, from);
        g.apply(0, &Action::Move { unit: gun, to: post }).unwrap();
        assert!(g.units[&gun].moved && g.units[&gun].moves_left > 0.0);
        let walls = g.cities[&cid].wall_hp;
        let target = g.cities[&cid].pos;
        let shot = g.apply(0, &Action::Ranged { unit: gun, target });
        assert_eq!(shot.is_ok(), expect_fire, "live {live}: {shot:?}");
        if expect_fire {
            assert!(g.cities[&cid].wall_hp < walls, "the shot lands on the wall");
        }
    }
}

#[test]
fn a_hostile_siege_gun_on_the_live_board_keeps_the_rule() {
    let (mut g, cid) = walled_city();
    mirrored(&mut g);
    let far = at_distance(&g, cid, 4);
    let ours = g.spawn_unit("catapult", 0, far[0]);
    let theirs = g.spawn_unit("catapult", 1, far[far.len() / 2]);
    assert!(g.siege_may_attack_after_moving(&g.units[&ours]));
    assert!(!g.siege_may_attack_after_moving(&g.units[&theirs]));
    let mut native = g.clone();
    native.host_observed = Arc::new(BTreeSet::new());
    assert!(!native.siege_may_attack_after_moving(&native.units[&ours]));
}

/// Kwadukuza, turns 113 to 122: melee on the ring, the one gun in reach
/// healing. The train falls back to Stage on the second short reading and
/// does not enter Invest again while nothing can open the walls.
#[test]
fn melee_do_not_hold_the_ring_of_walls_nothing_can_open() {
    let (mut g, cid) = medieval_city();
    let mut units: Vec<u32> = ring_of(&g, cid)
        .into_iter()
        .take(3)
        .map(|pos| g.spawn_unit("man_at_arms", 0, pos))
        .collect();
    let gun_pos = at_distance(&g, cid, 4)[0];
    let gun = g.spawn_unit("bombard", 0, gun_pos);
    g.units.get_mut(&gun).unwrap().hp = 29;
    units.push(gun);
    let group = group_on(&g, cid, &units);
    let plan = plan_against(&g, cid);

    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_siege_positive_damage_budget();
    ai.force_groups.push(group.clone());
    reducing(&mut ai, cid);
    let mut off = ai.clone();
    ai.enable_siege_needs_a_breaker();

    let city = CityView::of(&g, cid).unwrap();
    let reading = ai.breach_reading(&g, 0, &city, &units);
    assert_eq!((reading.guns, reading.wounded_guns, reading.support), (0, 1, 0));
    assert!(!reading.at_hand(&city));

    ai.assess_siege(&g, 0, cid, &plan, &group);
    assert_eq!(ai.sieges[&cid].stage, SiegeStage::Reduce, "one short turn holds");
    // The hold is recorded, and the healing gun is a breaker on its way.
    assert_eq!(ai.siege_breaker_waits.get(&city.pos), Some(&(30, 30)));
    assert!(ai.waiting_for_a_breaker(&g, 0, cid));
    assert!(!off.waiting_for_a_breaker(&g, 0, cid), "gene off");
    g.turn = 31;
    ai.assess_siege(&g, 0, cid, &plan, &group);
    assert_eq!(ai.sieges[&cid].stage, SiegeStage::Stage);
    g.turn = 32;
    ai.assess_siege(&g, 0, cid, &plan, &group);
    assert_eq!(ai.sieges[&cid].stage, SiegeStage::Stage, "no breaker, no Invest");

    // The budget counted the healing gun as firing every turn.
    off.assess_siege(&g, 0, cid, &plan, &group);
    g.turn = 33;
    off.assess_siege(&g, 0, cid, &plan, &group);
    assert_eq!(off.sieges[&cid].stage, SiegeStage::Reduce);

    // A staged man-at-arms steps out of the city's reach.
    let melee = units[0];
    let view = CityView::of(&g, cid).unwrap();
    for _ in 0..4 {
        if g.units[&melee].moves_left <= 0.0 || !ai.siege_stage_step(&mut g, 0, melee, &view, &plan) {
            break;
        }
    }
    assert!(g.wdist(g.units[&melee].pos, view.pos) > CITY_STRIKE_RANGE);
}

#[test]
fn a_fit_gun_or_low_walls_are_a_breaker_and_shooters_count_by_what_they_breach() {
    let (mut g, cid) = medieval_city();
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_siege_needs_a_breaker();
    let near = at_distance(&g, cid, 3);

    let gun = g.spawn_unit("bombard", 0, near[0]);
    let city = CityView::of(&g, cid).unwrap();
    let reading = ai.breach_reading(&g, 0, &city, &[gun]);
    assert_eq!(reading.guns, 1);
    assert!(reading.at_hand(&city));
    ai.battle_planner_recovering.insert(gun);
    let healing = ai.breach_reading(&g, 0, &city, &[gun]);
    assert_eq!((healing.guns, healing.wounded_guns), (0, 1));
    assert!(!healing.at_hand(&city));
    g.remove_unit(gun);

    // Two Crossbows chip Medieval Walls at a few points a shot: not a breach.
    let bows: Vec<u32> = near[1..3]
        .iter()
        .map(|pos| g.spawn_unit("crossbowman", 0, *pos))
        .collect();
    let city = CityView::of(&g, cid).unwrap();
    let reading = ai.breach_reading(&g, 0, &city, &bows);
    assert!(reading.shooter_walls > 0.0);
    assert!(!reading.at_hand(&city), "{reading:?}");
    // The same Crossbows against 30 points of wall are.
    g.cities.get_mut(&cid).unwrap().wall_hp = 50;
    let city = CityView::of(&g, cid).unwrap();
    assert!(ai.breach_reading(&g, 0, &city, &bows).at_hand(&city));
    // And walls a melee blow opens need no breaker at all.
    g.cities.get_mut(&cid).unwrap().wall_hp = 30;
    let city = CityView::of(&g, cid).unwrap();
    assert!(ai.breach_reading(&g, 0, &city, &[]).at_hand(&city));
}

#[test]
fn a_siege_tower_beside_a_melee_member_is_a_breaker_against_medieval_walls() {
    let (mut g, cid) = medieval_city();
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_siege_needs_a_breaker();
    let near = at_distance(&g, cid, 3);
    let melee = g.spawn_unit("man_at_arms", 0, near[0]);
    let tower = g.spawn_unit("siege_tower", 0, near[1]);
    let city = CityView::of(&g, cid).unwrap();
    assert_eq!(ai.breach_reading(&g, 0, &city, &[melee]).support, 1);
    // Renaissance Walls are immune to it.
    g.cities
        .get_mut(&cid)
        .unwrap()
        .buildings
        .push(crate::name!("renaissance_walls"));
    g.cities.get_mut(&cid).unwrap().wall_hp = 300;
    let city = CityView::of(&g, cid).unwrap();
    assert_eq!(ai.breach_reading(&g, 0, &city, &[melee]).support, 0);
    assert!(g.units.contains_key(&tower));
}

/// The Siege Tower walks onto a melee member of the train and links to it,
/// and leaves a carrier that is not one.
#[test]
fn a_siege_tower_joins_a_melee_member_of_the_train() {
    let (mut g, cid) = medieval_city();
    let city = g.cities[&cid].pos;
    let stand = at_distance(&g, cid, 4)[0];
    let melee = g.spawn_unit("man_at_arms", 0, stand);
    let start = g
        .wring(stand, 2)
        .into_iter()
        .find(|pos| g.wdist(*pos, city) >= 4 && g.map.get(*pos).is_some_and(|t| g.rules.is_passable(t)))
        .unwrap();
    let tower = g.spawn_unit("siege_tower", 0, start);
    // A stray warrior carries it now: not a member of the train.
    let stray = g.spawn_unit("warrior", 0, start);
    g.apply(0, &Action::LinkUnits { unit: stray, with: tower }).unwrap();
    let group = group_on(&g, cid, &[melee]);
    let plan = plan_against(&g, cid);
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_siege_needs_a_breaker();
    ai.force_groups.push(group);
    reducing(&mut ai, cid);

    let acted = ai.breach_support_step(&mut g, 0, tower, &plan);
    assert_eq!(acted, Some(true));
    assert_eq!(g.units[&stray].linked_to, None, "the stray lets it go");
    assert_eq!(g.units[&tower].pos, stand, "it walks onto the member");
    assert_eq!(g.units[&tower].linked_to, Some(melee));
    assert_eq!(g.units[&melee].linked_to, Some(tower));
    // Linked to a member, it rides: the carrier's orders move the pair.
    assert_eq!(ai.breach_support_step(&mut g, 0, tower, &plan), Some(false));

    // Without the gene the doctrine leaves it to the ladder.
    let mut off = AdvancedAi::new();
    off.enable_siege_train();
    assert_eq!(off.siege_doctrine_step(&mut g, 0, tower, &plan), None);
}
