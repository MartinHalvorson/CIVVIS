//! `guns-enter-together`: fit siege guns of a walled siege step into the
//! city's strike ring together, sharing its once-a-turn blow. Live King
//! civvis-20261005T051413Z (game 102) held four fit Bombards three tiles
//! from walled Toronto from turn 137 to 145; not one fired in twenty turns.

use super::tests::{at_distance, walled_city};
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
