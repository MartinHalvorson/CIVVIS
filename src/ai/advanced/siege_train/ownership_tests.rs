use super::super::ForcePosture;
use super::tests::{at_distance, plan_against, ring_of, walled_city};
use super::*;

fn group(g: &Game, uid: u32, cid: u32) -> ForceGroup {
    ForceGroup {
        id: uid,
        domain: ForceDomain::Land,
        units: vec![uid],
        anchor: g.units[&uid].pos,
        objective: g.cities[&cid].pos,
        focus_target: None,
        posture: ForcePosture::Advance,
        readiness: 1.0,
        local_strength_ratio: 2.0,
    }
}

#[test]
fn a_siege_does_not_reserve_another_citys_capture_unit() {
    let (mut g, first) = walled_city();
    let second_pos = at_distance(&g, first, 3)[0];
    g.found_city_for(1, second_pos, None);
    let second = g.city_at(second_pos).unwrap();
    let warrior_pos = ring_of(&g, second)
        .into_iter()
        .filter(|p| g.wdist(*p, g.cities[&first].pos) <= 3)
        .find(|p| g.city_at(*p).is_none() && !g.rules.is_water(g.map.get(*p).unwrap()))
        .unwrap();
    let warrior = g.spawn_unit("warrior", 0, warrior_pos);
    let gun_pos = at_distance(&g, first, 2)
        .into_iter()
        .find(|p| *p != warrior_pos && *p != second_pos)
        .unwrap();
    let gun = g.spawn_unit("catapult", 0, gun_pos);
    for cid in [first, second] {
        let city = g.cities.get_mut(&cid).unwrap();
        city.hp = 1;
        city.wall_hp = 0;
    }
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.force_groups = vec![group(&g, gun, first), group(&g, warrior, second)];
    let plan = plan_against(&g, first);
    let _ = ai.siege_doctrine_step(&mut g, 0, gun, &plan);
    assert_ne!(
        ai.sieges[&first].taker,
        Some(warrior),
        "a nearby unit assigned to the other city is not this siege's finisher"
    );
    assert!(!ai.sieges[&first].posts.contains_key(&warrior));
    let _ = ai.siege_doctrine_step(&mut g, 0, warrior, &plan);
    assert_eq!(
        g.cities[&second].owner, 0,
        "the warrior completes its assigned capture"
    );
}

#[test]
fn two_groups_assigned_to_one_city_share_its_finisher() {
    let (mut g, cid) = walled_city();
    let warrior = g.spawn_unit("warrior", 0, ring_of(&g, cid)[0]);
    let gun = g.spawn_unit("catapult", 0, at_distance(&g, cid, 2)[0]);
    g.cities.get_mut(&cid).unwrap().hp = 1;
    g.cities.get_mut(&cid).unwrap().wall_hp = 0;
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.force_groups = vec![group(&g, gun, cid), group(&g, warrior, cid)];
    let plan = plan_against(&g, cid);
    let _ = ai.siege_doctrine_step(&mut g, 0, gun, &plan);
    assert_eq!(ai.sieges[&cid].taker, Some(warrior));
    let _ = ai.siege_doctrine_step(&mut g, 0, warrior, &plan);
    assert_eq!(g.cities[&cid].owner, 0);
}

fn taker_with_builder(distance: i32) -> (Game, AdvancedAi, StrategicPlan, u32, u32, u32) {
    let (mut g, cid) = walled_city();
    let origin = ring_of(&g, cid)[0];
    let warrior = g.spawn_unit("warrior", 0, origin);
    let gun = g.spawn_unit("catapult", 0, at_distance(&g, cid, 2)[0]);
    g.cities.get_mut(&cid).unwrap().hp = 1;
    g.cities.get_mut(&cid).unwrap().wall_hp = 0;
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.base.barbarian_settler_capture = true;
    ai.force_groups = vec![group(&g, gun, cid), group(&g, warrior, cid)];
    let plan = plan_against(&g, cid);
    let _ = ai.siege_doctrine_step(&mut g, 0, gun, &plan);
    assert!(ai.unit_is_reserved(warrior));
    let prize = g
        .wring(origin, distance)
        .into_iter()
        .find(|pos| {
            g.city_at(*pos).is_none()
                && g.unit_ids_at(*pos).is_empty()
                && g.map
                    .get(*pos)
                    .is_some_and(|tile| g.rules.is_passable(tile) && !g.rules.is_water(tile))
                && g.wdist(*pos, g.cities[&cid].pos) > 1
        })
        .unwrap();
    let builder = g.spawn_unit("builder", 1, prize);
    let mut pickup = g.clone();
    let can_pick_up = if distance == 1 {
        ai.base
            .capture_adjacent_civilian(&mut pickup, 0, warrior, false)
    } else {
        ai.base
            .pursue_capturable_civilian(&mut pickup, 0, warrior, false)
    };
    assert!(can_pick_up, "the civilian competes with the siege decision");
    (g, ai, plan, cid, warrior, builder)
}

#[test]
fn reserved_city_taker_finishes_before_an_adjacent_builder_pickup() {
    let (mut g, mut ai, plan, cid, warrior, builder) = taker_with_builder(1);
    assert!(ai.advanced_military_step(&mut g, 0, warrior, &plan));
    assert_eq!(g.cities[&cid].owner, 0);
    assert_eq!(g.units[&builder].owner, 1);
}

#[test]
fn reserved_city_taker_finishes_before_a_distant_builder_pursuit() {
    let (mut g, mut ai, plan, cid, warrior, builder) = taker_with_builder(2);
    assert!(ai.advanced_military_step(&mut g, 0, warrior, &plan));
    assert_eq!(g.cities[&cid].owner, 0);
    assert_eq!(g.units[&builder].owner, 1);
}

#[test]
fn siege_shooter_keeps_its_city_assignment_before_a_distant_civilian_pursuit() {
    let (mut g, mut ai, plan, cid, _, builder) = taker_with_builder(2);
    let gun = ai.force_groups[0].units[0];
    assert!(!ai.unit_is_reserved(gun), "only the city taker is reserved");
    let shooter = g.units.get_mut(&gun).unwrap();
    shooter.moves_left = 2.0;
    shooter.attacks_left = 1;
    shooter.acted = false;
    shooter.moved = false;
    let origin = g.units[&gun].pos;
    let detour = g
        .wring(origin, 2)
        .into_iter()
        .find(|pos| {
            if g.city_at(*pos).is_some()
                || !g.unit_ids_at(*pos).is_empty()
                || g.map
                    .get(*pos)
                    .is_none_or(|tile| !g.rules.is_passable(tile) || g.rules.is_water(tile))
                || g.wdist(*pos, g.cities[&cid].pos) <= g.wdist(origin, g.cities[&cid].pos)
            {
                return false;
            }
            let mut probe = g.clone();
            probe.units.get_mut(&builder).unwrap().pos = *pos;
            ai.base
                .pursue_capturable_civilian(&mut probe, 0, gun, false)
        })
        .expect("a civilian can lure the siege shooter away from the city");
    g.units.get_mut(&builder).unwrap().pos = detour;

    let city_hp = g.cities[&cid].hp;
    assert!(ai.advanced_military_step(&mut g, 0, gun, &plan));
    assert_eq!(
        g.units[&gun].pos, origin,
        "the siege shooter stays on its post"
    );
    assert!(g.cities[&cid].hp < city_hp, "the shooter reduces the city");
    assert_eq!(g.units[&builder].owner, 1);
}

#[test]
fn obsolete_siege_reservation_still_allows_a_civilian_pickup() {
    let (mut g, mut ai, plan, cid, warrior, builder) = taker_with_builder(1);
    g.cities.get_mut(&cid).unwrap().owner = 0;
    assert!(ai.advanced_military_step(&mut g, 0, warrior, &plan));
    assert_eq!(g.units[&builder].owner, 0);
}

#[test]
fn an_unassigned_soldier_can_still_pick_up_a_civilian() {
    let (mut g, mut ai, plan, cid, warrior, builder) = taker_with_builder(1);
    ai.reserved_units.clear();
    ai.force_groups
        .retain(|group| !group.units.contains(&warrior));
    assert!(ai.advanced_military_step(&mut g, 0, warrior, &plan));
    assert_eq!(g.units[&builder].owner, 0);
    assert_eq!(g.cities[&cid].owner, 1);
}

/// Live King civvis-20261004T033533Z (game 46): full-health Archers reached
/// Mari's range-2 posts during Reduce and the envelope evacuation in
/// `healing_step` walked them back to 3-4 tiles every turn (4 shots in 9
/// turns). A healthy member of an active siege keeps the turn for the siege;
/// the same unit outside an active siege still evacuates.
#[test]
fn a_healthy_siege_shooter_is_not_evacuated_off_its_post() {
    let turn = |active: bool| {
        let (mut g, cid) = walled_city();
        // A campaign board: an arena has no recovery to evacuate into.
        g.map_script = crate::setup::MapScript::LandOnly;
        g.at_war.insert((0, 1));
        g.at_war.insert((1, 0));
        let target = g.cities[&cid].pos;
        let post = at_distance(&g, cid, 2)
            .into_iter()
            .find(|pos| g.line_of_sight_from(*pos, target))
            .expect("a firing tile");
        let archer = g.spawn_unit("archer", 0, post);
        // Enemy shooters out of the Archer's reach whose summed envelope
        // reads lethal on the post, as Mari's city strike and Trebuchet did.
        let mut placed = 0;
        for pos in g.wring(post, 3) {
            if placed == 4 {
                break;
            }
            if g.wdist(pos, target) >= 2
                && g.city_at(pos).is_none()
                && g.unit_ids_at(pos).is_empty()
                && g.map
                    .get(pos)
                    .is_some_and(|t| g.rules.is_passable(t) && !g.rules.is_water(t))
            {
                g.spawn_unit("crossbowman", 1, pos);
                placed += 1;
            }
        }
        assert_eq!(placed, 4);
        let mut ai = AdvancedAi::targeting(crate::ai::VictoryTarget::Domination);
        ai.enable_siege_train();
        ai.force_groups = vec![group(&g, archer, cid)];
        if active {
            ai.sieges.insert(
                cid,
                Siege {
                    stage: SiegeStage::Reduce,
                    taker: None,
                    entered: g.turn,
                    assessed: g.turn,
                    posts: BTreeMap::from([(archer, post)]),
                    short_since: None,
                },
            );
        }
        assert_eq!(ai.active_siege_member(&g, 0, archer), active);
        let plan = plan_against(&g, cid);
        let _ = ai.advanced_military_step(&mut g, 0, archer, &plan);
        (
            ai.base.recovering_units.contains(&archer),
            g.units[&archer].pos == post,
            g.cities[&cid].wall_hp,
        )
    };
    let (recovering, held, _) = turn(false);
    assert!(
        recovering && !held,
        "the control: the summed envelope walks the archer off the post"
    );
    let (recovering, held, walls) = turn(true);
    assert!(!recovering && held, "an active siege keeps its healthy shooter on the post");
    assert!(walls < 100, "and the shooter fires at the walls");
}

/// See `assess_siege`: a Hold over a city still the enemy's is a capture the
/// host never carried out (live King civvis-20261004T070716Z, Kyoto at turn
/// 213). The assault resumes and the taker walks in.
#[test]
fn a_held_siege_over_an_enemy_city_resumes_the_capture() {
    let (mut g, city) = walled_city();
    let ring = ring_of(&g, city)
        .into_iter()
        .find(|p| g.city_at(*p).is_none() && !g.rules.is_water(g.map.get(*p).unwrap()))
        .unwrap();
    let warrior = g.spawn_unit("warrior", 0, ring);
    {
        let target = g.cities.get_mut(&city).unwrap();
        target.hp = 1;
        target.wall_hp = 0;
    }
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.force_groups = vec![group(&g, warrior, city)];
    // The board's capture of an earlier frame that the host never made.
    ai.sieges.insert(
        city,
        Siege {
            stage: SiegeStage::Hold,
            taker: None,
            entered: g.turn.saturating_sub(1),
            assessed: g.turn.saturating_sub(1),
            posts: Default::default(),
            short_since: None,
        },
    );
    let plan = plan_against(&g, city);
    let _ = ai.siege_doctrine_step(&mut g, 0, warrior, &plan);
    assert_eq!(g.cities[&city].owner, 0, "the taker walks into the city");
}
