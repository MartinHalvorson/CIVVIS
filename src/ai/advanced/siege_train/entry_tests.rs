//! `guns-enter-together`: fit siege guns of a walled siege step into the
//! city's strike ring together, sharing its once-a-turn blow. Live King
//! civvis-20261005T051413Z (game 102) held four fit Bombards three tiles
//! from walled Toronto from turn 137 to 145; not one fired in twenty turns.

use super::tests::{at_distance, plan_against, walled_city};
use super::*;

/// `walled_city` on open grassland, at war, its walls at full strength.
fn open_walled_city() -> (Game, u32) {
    let (mut g, cid) = walled_city();
    let city = g.cities[&cid].pos;
    for tile in g.map.tiles.values_mut() {
        if tile.pos != city {
            tile.terrain = crate::name!("grassland");
            tile.feature = None;
            tile.hills = false;
        }
    }
    g.map_script = crate::setup::MapScript::Pangaea;
    g.turn = 30;
    g.at_war.insert((0, 1));
    assert!(g.cities[&cid].wall_hp > 0, "fixture: walls stand");
    (g, cid)
}

/// Two firing posts two tiles out, each with a start three tiles out beside
/// it, all four tiles distinct and the posts not adjacent.
fn two_lanes(g: &Game, cid: u32) -> [(Pos, Pos); 2] {
    let city = g.cities[&cid].pos;
    let posts = at_distance(g, cid, 2);
    let lane = |post: Pos| {
        g.nbrs(post)
            .into_iter()
            .find(|start| g.wdist(*start, city) == 3)
            .map(|start| (start, post))
    };
    let first = lane(posts[0]).expect("a start beside the first post");
    let second = posts
        .iter()
        .copied()
        .filter(|post| g.wdist(*post, first.1) >= 3)
        .find_map(|post| lane(post).filter(|(start, _)| g.wdist(*start, first.0) >= 2))
        .expect("a second lane apart from the first");
    [first, second]
}

/// A siege train AI on the live seat's defaults for the danger reading.
fn train(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_shared_danger();
    if gene {
        ai.enable_guns_enter_together();
    }
    ai
}

/// The siege in Invest with each gun posted on its lane's post.
fn investing(ai: &mut AdvancedAi, cid: u32, posts: &[(u32, Pos)]) {
    ai.sieges.insert(
        cid,
        Siege {
            stage: SiegeStage::Invest,
            taker: None,
            entered: 25,
            assessed: 29,
            posts: posts.iter().copied().collect(),
            short_since: None,
        },
    );
}

/// Catapults at 50 hp: alone, the city's strike (~37 on a post) leaves no
/// 20-point margin; shared between two (~18), it does.
const GUN_HP: i32 = 50;

fn guns_on(g: &mut Game, lanes: &[(Pos, Pos)]) -> Vec<u32> {
    lanes
        .iter()
        .map(|(start, _)| {
            let gun = g.spawn_unit("catapult", 0, *start);
            g.units.get_mut(&gun).unwrap().hp = GUN_HP;
            gun
        })
        .collect()
}

#[test]
fn two_fit_guns_step_into_the_strike_ring_together_only_under_the_gene() {
    for gene in [false, true] {
        let (mut g, cid) = open_walled_city();
        let lanes = two_lanes(&g, cid);
        let guns = guns_on(&mut g, &lanes);
        let mut ai = train(gene);
        investing(
            &mut ai,
            cid,
            &[(guns[0], lanes[0].1), (guns[1], lanes[1].1)],
        );
        let city = CityView::of(&g, cid).unwrap();
        let alone = ai.approach_danger(&g, 0, lanes[0].1, guns[0]);
        assert!(
            f64::from(GUN_HP) <= alone + 20.0,
            "fixture: one gun alone reads {alone:.1} on its post"
        );
        for (gun, (_, post)) in guns.iter().zip(&lanes) {
            let moved = ai.post_step(&mut g, 0, *gun, &city);
            if gene {
                assert_eq!(moved, Some(true), "the gun steps in with its partner");
                assert_eq!(g.units[gun].pos, *post);
            } else {
                assert_eq!(moved, None, "alone each gun reads the whole strike");
                assert_ne!(g.units[gun].pos, *post);
            }
        }
    }
}

#[test]
fn a_lone_gun_keeps_the_veto_with_the_gene_on() {
    let (mut g, cid) = open_walled_city();
    let lanes = two_lanes(&g, cid);
    let gun = guns_on(&mut g, &lanes[..1])[0];
    let mut ai = train(true);
    investing(&mut ai, cid, &[(gun, lanes[0].1)]);
    let city = CityView::of(&g, cid).unwrap();
    assert_eq!(ai.entry_group(&g, 0, &city), vec![gun]);
    assert_eq!(ai.post_step(&mut g, 0, gun, &city), None);
    assert_eq!(g.units[&gun].pos, lanes[0].0);
}

#[test]
fn the_group_needs_walls_fitness_and_a_post_inside_the_ring() {
    let (mut g, cid) = open_walled_city();
    let lanes = two_lanes(&g, cid);
    let guns = guns_on(&mut g, &lanes);
    let mut ai = train(true);
    investing(
        &mut ai,
        cid,
        &[(guns[0], lanes[0].1), (guns[1], lanes[1].1)],
    );
    let city = CityView::of(&g, cid).unwrap();
    assert_eq!(ai.entry_group(&g, 0, &city).len(), 2);
    // Under ROTATE_HP the battle planner holds a gun out: it enters with no one.
    g.units.get_mut(&guns[1]).unwrap().hp = super::super::battle_planner::ROTATE_HP - 1;
    assert_eq!(ai.entry_group(&g, 0, &city), vec![guns[0]]);
    g.units.get_mut(&guns[1]).unwrap().hp = GUN_HP;
    // Bare walls: the city does not strike, so there is nothing to share.
    g.cities.get_mut(&cid).unwrap().wall_hp = 0;
    let bare = CityView::of(&g, cid).unwrap();
    assert!(ai.entry_group(&g, 0, &bare).is_empty());
    // The gene off: no group.
    g.cities.get_mut(&cid).unwrap().wall_hp = city.wall_hp;
    let mut off = train(false);
    investing(
        &mut off,
        cid,
        &[(guns[0], lanes[0].1), (guns[1], lanes[1].1)],
    );
    assert!(off.entry_group(&g, 0, &city).is_empty());
}

#[test]
fn a_gun_kept_off_its_post_is_no_breaker_under_the_gene() {
    for gene in [false, true] {
        let (mut g, cid) = open_walled_city();
        let lanes = two_lanes(&g, cid);
        let gun = guns_on(&mut g, &lanes[..1])[0];
        let mut ai = train(gene);
        investing(&mut ai, cid, &[(gun, lanes[0].1)]);
        let city = CityView::of(&g, cid).unwrap();
        let reading = ai.breach_reading(&g, 0, &city, &[gun]);
        if gene {
            assert_eq!((reading.guns, reading.barred_guns), (0, 1));
        } else {
            assert_eq!((reading.guns, reading.barred_guns), (1, 0));
        }
    }
    // Two guns that enter together are both breakers.
    let (mut g, cid) = open_walled_city();
    let lanes = two_lanes(&g, cid);
    let guns = guns_on(&mut g, &lanes);
    let mut ai = train(true);
    investing(
        &mut ai,
        cid,
        &[(guns[0], lanes[0].1), (guns[1], lanes[1].1)],
    );
    let city = CityView::of(&g, cid).unwrap();
    let reading = ai.breach_reading(&g, 0, &city, &guns);
    assert_eq!((reading.guns, reading.barred_guns), (2, 0));
}

#[test]
fn a_gun_with_no_post_or_already_in_range_is_not_barred() {
    let (mut g, cid) = open_walled_city();
    let lanes = two_lanes(&g, cid);
    let gun = guns_on(&mut g, &lanes[..1])[0];
    let mut ai = train(true);
    // Stage: no posts, nothing judged.
    ai.sieges.insert(
        cid,
        Siege {
            stage: SiegeStage::Stage,
            taker: None,
            entered: 25,
            assessed: 29,
            posts: BTreeMap::new(),
            short_since: None,
        },
    );
    let city = CityView::of(&g, cid).unwrap();
    assert_eq!(ai.breach_reading(&g, 0, &city, &[gun]).guns, 1);
    // On its post, within range: it fires from where it stands.
    investing(&mut ai, cid, &[(gun, lanes[0].1)]);
    g.relocate(gun, lanes[0].1);
    assert!(!ai.gun_barred_from_its_post(&g, 0, gun, &city, &[]));
}

/// `march_past_a_remembered_cuirassier`'s board: a lone Trebuchet nine tiles
/// from the walled city, a Cuirassier seen last turn five tiles past the
/// march's next step and in the fog now.
fn march_past_a_remembered_cuirassier() -> (Game, u32, u32, Pos, Pos, AdvancedAi) {
    let (mut g, cid) = walled_city();
    for tile in g.map.tiles.values_mut() {
        tile.terrain = crate::name!("grassland");
        tile.feature = None;
        tile.hills = false;
    }
    g.at_war.insert((0, 1));
    g.turn = 50;
    let target = g.cities[&cid].pos;
    let start = (target.0 - 9, target.1);
    let gun = g.spawn_unit("trebuchet", 0, start);
    let next = march_step(&g, gun, target, STAGING_FAR).expect("an open march");
    let seen_at = (next.0, next.1 + 5);
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_staging_gun_remembers_hostiles();
    ai.hostile_last_seen.insert(
        424_242,
        super::super::RememberedHostile {
            pos: seen_at,
            when: g.turn - 1,
            owner: 1,
            kind: crate::name!("cuirassier"),
        },
    );
    (g, cid, gun, start, next, ai)
}

/// Fit Trebuchets marching in the band past the staging ring (six to eight
/// tiles out), away from the lone gun's lane.
fn marching_guns(g: &mut Game, cid: u32, count: usize) -> Vec<u32> {
    let target = g.cities[&cid].pos;
    let mut tiles: Vec<Pos> = (STAGING_FAR + 1..=STAGING_FAR + 3)
        .flat_map(|distance| at_distance(g, cid, distance))
        .filter(|pos| pos.0 > target.0)
        .collect();
    tiles.sort_unstable();
    tiles
        .into_iter()
        .take(count)
        .map(|pos| g.spawn_unit("trebuchet", 0, pos))
        .collect()
}

#[test]
fn guns_marching_together_share_a_remembered_hostiles_one_strike() {
    let (g, _, gun, _, _, ai) = march_past_a_remembered_cuirassier();
    let blow = ai.remembered_strikers(&g, 0, gun)[0].blow;
    let limit = (100.0 - STAGING_GUN_HP_RESERVE) / STAGING_GUN_REPLY_TURNS;
    assert!(blow > limit, "fixture: one gun alone holds short");
    let together = (blow / limit).floor() as usize + 1;
    for gene in [false, true] {
        let (mut g, cid, gun, start, _, mut ai) = march_past_a_remembered_cuirassier();
        marching_guns(&mut g, cid, together);
        if gene {
            ai.enable_guns_enter_together();
        }
        let city = CityView::of(&g, cid).unwrap();
        let plan = plan_against(&g, cid);
        assert_eq!(
            ai.staging_fog_terms(&g, 0, &city, gun).0,
            if gene { together as f64 } else { 1.0 },
            "the lone gun nine out is past the band; the others share"
        );
        ai.siege_stage_step(&mut g, 0, gun, &city, &plan);
        let at = g.units[&gun].pos;
        if gene {
            // The gun nine tiles out is not in the band itself, so the share
            // is the others'; with them it reads under the limit and walks.
            assert_ne!(at, start, "the shared strike lets it march");
        } else {
            assert_eq!(at, start, "alone it holds short");
        }
    }
}

#[test]
fn two_guns_on_the_ring_drop_a_hostile_seen_only_last_turn() {
    let (mut g, cid, gun, start, _, mut ai) = march_past_a_remembered_cuirassier();
    let ring: Vec<Pos> = at_distance(&g, cid, STAGING_FAR)
        .into_iter()
        .filter(|pos| pos.0 > g.cities[&cid].pos.0)
        .take(2)
        .collect();
    for pos in ring {
        g.spawn_unit("trebuchet", 0, pos);
    }
    let city = CityView::of(&g, cid).unwrap();
    let plan = plan_against(&g, cid);
    let mut off = ai.clone();
    ai.enable_guns_enter_together();
    assert_eq!(ai.staging_fog_terms(&g, 0, &city, gun), (2.0, true));
    assert_eq!(off.staging_fog_terms(&g, 0, &city, gun), (1.0, false));
    let mut g_off = g.clone();
    off.siege_stage_step(&mut g_off, 0, gun, &city, &plan);
    assert_eq!(g_off.units[&gun].pos, start, "off: still held short");
    ai.siege_stage_step(&mut g, 0, gun, &city, &plan);
    assert_ne!(
        g.units[&gun].pos, start,
        "on: the stale sighting no longer holds it"
    );
}

#[test]
fn a_reserve_gun_at_a_walled_siege_joins_it_under_the_gene() {
    let (mut g, cid) = open_walled_city();
    let target = g.cities[&cid].pos;
    let melee: Vec<u32> = at_distance(&g, cid, 3)
        .into_iter()
        .take(3)
        .map(|pos| g.spawn_unit("swordsman", 0, pos))
        .collect();
    let gun = g.spawn_unit("catapult", 0, at_distance(&g, cid, 4)[0]);
    let siege = ForceGroup {
        id: melee[0],
        domain: ForceDomain::Land,
        units: melee.clone(),
        anchor: target,
        objective: target,
        focus_target: Some(target),
        posture: super::super::ForcePosture::Advance,
        readiness: 1.0,
        local_strength_ratio: 2.0,
    };
    // The Reserve holds well away from any city it could besiege.
    let rally = (target.0 - 20, target.1);
    let reserve = ForceGroup {
        id: gun,
        domain: ForceDomain::Land,
        units: vec![gun],
        anchor: rally,
        objective: rally,
        focus_target: None,
        posture: super::super::ForcePosture::Muster,
        readiness: 1.0,
        local_strength_ratio: 1.0,
    };
    let plan = plan_against(&g, cid);
    let view = CityView::of(&g, cid).unwrap();
    for gene in [false, true] {
        let mut ai = train(gene);
        ai.force_groups = vec![siege.clone(), reserve.clone()];
        assert_eq!(ai.siege_city_of(&g, 0, &plan, &reserve), None, "fixture");
        let force = ai.siege_force(&g, 0, &view, &plan, &siege);
        assert_eq!(force.contains(&gun), gene);
    }
}

#[test]
fn a_gun_kept_off_its_post_still_counts_against_falling_back_to_stage() {
    // A lone gun posted inside the strike ring and kept off it: under the
    // gene it is no breaker for the reading, but the siege is not told the
    // walls have nothing to open them, so it does not oscillate to Stage.
    let (mut g, cid) = open_walled_city();
    let lanes = two_lanes(&g, cid);
    let gun = guns_on(&mut g, &lanes[..1])[0];
    let city = CityView::of(&g, cid).unwrap();
    for gene in [false, true] {
        let mut ai = train(gene);
        investing(&mut ai, cid, &[(gun, lanes[0].1)]);
        let reading = ai.breach_reading(&g, 0, &city, &[gun]);
        assert_eq!(reading.barred_guns, usize::from(gene));
        assert!(
            !reading.leaves_walls_shut(&city),
            "a waiting gun is a breaker (gene {gene})"
        );
    }
    // No gun at all: the walls are shut, as before.
    let ai = train(true);
    let empty = ai.breach_reading(&g, 0, &city, &[]);
    assert!(empty.leaves_walls_shut(&city));
}

/// `gun-queues-behind-the-column`: a one-tile defile through mountains
/// from six tiles out to a firing post two out, one of ours standing in it
/// two tiles ahead of a Catapult. The train's router treats the friend as a
/// wall and the crossing looks only beside the gun, so with the gene off
/// the gun stands; on, it steps up behind the column.
#[test]
fn a_gun_boxed_behind_the_column_steps_up_only_under_the_gene() {
    for gene in [false, true] {
        let (mut g, cid) = open_walled_city();
        let city = g.cities[&cid].pos;
        let post = at_distance(&g, cid, 2)[0];
        let mut lane = vec![post];
        while lane.len() < 5 {
            let last = *lane.last().unwrap();
            let next = g
                .nbrs(last)
                .into_iter()
                .filter(|pos| g.wdist(*pos, city) == g.wdist(last, city) + 1)
                .min()
                .expect("a tile one further out");
            lane.push(next);
        }
        for tile in g.map.tiles.values_mut() {
            if tile.pos != city && !lane.contains(&tile.pos) && g_wdist_gt(tile.pos, city) {
                tile.terrain = crate::name!("mountain");
            }
        }
        let friend = g.spawn_unit("warrior", 0, lane[2]);
        let gun = g.spawn_unit("catapult", 0, lane[4]);
        let mut ai = train(false);
        if gene {
            ai.enable_gun_queues_behind_the_column();
        }
        investing(&mut ai, cid, &[(gun, post)]);
        let view = CityView::of(&g, cid).unwrap();
        let moved = ai.post_step(&mut g, 0, gun, &view);
        assert_eq!(g.units[&friend].pos, lane[2], "the friend holds the defile");
        if gene {
            assert_eq!(moved, Some(true), "the gun steps up");
            assert_eq!(g.units[&gun].pos, lane[3], "behind the friend");
        } else {
            assert_eq!(moved, None, "boxed in, the gun stands");
            assert_eq!(g.units[&gun].pos, lane[4]);
        }
    }

    /// Keep the city's own ring passable, so the post's ring stays legal.
    fn g_wdist_gt(pos: Pos, city: Pos) -> bool {
        let (dq, dr) = (pos.0 - city.0, pos.1 - city.1);
        (dq.abs() + dr.abs() + (dq + dr).abs()) / 2 > 1
    }
}
