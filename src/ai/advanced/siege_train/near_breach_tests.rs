//! `guns-post-for-a-near-breach`: the fit guns of a walled siege whose shots
//! open the walls within `NEAR_BREACH_TURNS` enter their firing posts on a
//! survivable reply. Over the 10-04/05 control runs, 537 walled Invest/Reduce
//! siege-turns had such guns within seven tiles and 58% saw no shot on the
//! city: the guns held one tile outside their own range.

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

/// A firing post two tiles out and a start three tiles out beside it.
fn lane(g: &Game, cid: u32) -> (Pos, Pos) {
    let city = g.cities[&cid].pos;
    at_distance(g, cid, 2)
        .into_iter()
        .find_map(|post| {
            g.nbrs(post)
                .into_iter()
                .find(|start| g.wdist(*start, city) == 3)
                .map(|start| (start, post))
        })
        .expect("a start beside a post")
}

/// The live seat's siege defaults, with or without the gene.
fn train(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_shared_danger();
    ai.enable_guns_enter_together();
    if gene {
        ai.enable_guns_post_for_a_near_breach();
    }
    ai
}

fn siege(ai: &mut AdvancedAi, cid: u32, stage: SiegeStage, posts: &[(u32, Pos)]) {
    ai.sieges.insert(
        cid,
        Siege {
            stage,
            taker: None,
            entered: 25,
            assessed: 29,
            posts: posts.iter().copied().collect(),
            short_since: None,
        },
    );
}

/// A catapult at the fitness floor: the city's strike on its post (~37)
/// leaves no 20-point margin, but the reply's upper roll (~44) still leaves
/// it standing.
const GUN_HP: i32 = 50;

fn lone_gun(g: &mut Game, cid: u32) -> (u32, Pos, Pos) {
    let (start, post) = lane(g, cid);
    let gun = g.spawn_unit("catapult", 0, start);
    g.units.get_mut(&gun).unwrap().hp = GUN_HP;
    (gun, start, post)
}

/// Walls low enough for the lone gun to open them within the window.
fn wear_walls_to(g: &mut Game, cid: u32, gun: u32, turns: f64) {
    let per_shot = wall_damage_per_shot(g, gun, cid);
    assert!(per_shot > 0.0, "fixture: the gun hurts the walls");
    g.cities.get_mut(&cid).unwrap().wall_hp = ((per_shot * turns).floor() as i32).max(1);
}

#[test]
fn a_lone_gun_enters_for_a_near_breach_only_under_the_gene() {
    for gene in [false, true] {
        let (mut g, cid) = open_walled_city();
        let (gun, start, post) = lone_gun(&mut g, cid);
        wear_walls_to(&mut g, cid, gun, 3.0);
        let mut ai = train(gene);
        siege(&mut ai, cid, SiegeStage::Invest, &[(gun, post)]);
        let reply = ai.approach_danger(&g, 0, post, gun);
        assert!(
            f64::from(GUN_HP) <= reply + ENTRY_HP_MARGIN
                && f64::from(GUN_HP) > reply * NEAR_BREACH_REPLY_ROLL,
            "fixture: the reply on the post reads {reply:.1}"
        );
        let city = CityView::of(&g, cid).unwrap();
        let moved = ai.post_step(&mut g, 0, gun, &city);
        if gene {
            assert_eq!(moved, Some(true), "the gun takes its post");
            assert_eq!(g.units[&gun].pos, post);
        } else {
            assert_eq!(moved, None, "the margin keeps it out");
            assert_eq!(g.units[&gun].pos, start);
        }
    }
}

#[test]
fn a_far_breach_keeps_the_margin() {
    let (mut g, cid) = open_walled_city();
    let (gun, start, post) = lone_gun(&mut g, cid);
    wear_walls_to(&mut g, cid, gun, NEAR_BREACH_TURNS + 2.0);
    let mut ai = train(true);
    siege(&mut ai, cid, SiegeStage::Invest, &[(gun, post)]);
    let city = CityView::of(&g, cid).unwrap();
    assert_eq!(ai.near_breach(&g, 0, &city), None);
    assert_eq!(ai.post_step(&mut g, 0, gun, &city), None);
    assert_eq!(g.units[&gun].pos, start);
}

#[test]
fn a_reply_whose_upper_roll_would_finish_the_gun_keeps_it_out() {
    let (mut g, cid) = open_walled_city();
    let (gun, start, post) = lone_gun(&mut g, cid);
    wear_walls_to(&mut g, cid, gun, 2.0);
    let mut ai = train(true);
    siege(&mut ai, cid, SiegeStage::Invest, &[(gun, post)]);
    // A stronger city strike: its centre under the gun's health, its upper
    // roll not.
    let base = g.city_ranged_strength(cid);
    let strength = (0..40)
        .map(|step| base + f64::from(step) * 0.5)
        .find(|strength| {
            std::sync::Arc::make_mut(&mut g.observed_city_ranged_strength).insert(cid, *strength);
            let reply = ai.approach_danger(&g, 0, post, gun);
            f64::from(GUN_HP) > reply && f64::from(GUN_HP) <= reply * NEAR_BREACH_REPLY_ROLL
        })
        .expect("fixture: a strike between the centre and the upper roll");
    std::sync::Arc::make_mut(&mut g.observed_city_ranged_strength).insert(cid, strength);
    let reply = ai.approach_danger(&g, 0, post, gun);
    assert!(
        f64::from(GUN_HP) > reply && f64::from(GUN_HP) <= reply * NEAR_BREACH_REPLY_ROLL,
        "fixture: the reply on the post reads {reply:.1}"
    );
    let city = CityView::of(&g, cid).unwrap();
    assert!(ai.near_breach(&g, 0, &city).is_some());
    assert_eq!(ai.post_step(&mut g, 0, gun, &city), None);
    assert_eq!(g.units[&gun].pos, start);
}

#[test]
fn the_near_breach_needs_invest_or_reduce_walls_and_a_fit_posted_gun() {
    let (mut g, cid) = open_walled_city();
    let (gun, _, post) = lone_gun(&mut g, cid);
    wear_walls_to(&mut g, cid, gun, 3.0);
    let mut ai = train(true);
    for stage in [SiegeStage::Invest, SiegeStage::Reduce] {
        siege(&mut ai, cid, stage, &[(gun, post)]);
        let city = CityView::of(&g, cid).unwrap();
        let (group, turns) = ai.near_breach(&g, 0, &city).expect("a near breach");
        assert_eq!(group, vec![gun]);
        assert!(turns <= NEAR_BREACH_TURNS);
    }
    siege(&mut ai, cid, SiegeStage::Stage, &[(gun, post)]);
    let city = CityView::of(&g, cid).unwrap();
    assert_eq!(ai.near_breach(&g, 0, &city), None, "Stage holds the ring");
    siege(&mut ai, cid, SiegeStage::Invest, &[(gun, post)]);
    g.units.get_mut(&gun).unwrap().hp = GUN_HP - 1;
    let city = CityView::of(&g, cid).unwrap();
    assert_eq!(ai.near_breach(&g, 0, &city), None, "a wounded gun heals first");
    g.units.get_mut(&gun).unwrap().hp = GUN_HP;
    g.cities.get_mut(&cid).unwrap().wall_hp = 0;
    let city = CityView::of(&g, cid).unwrap();
    assert_eq!(ai.near_breach(&g, 0, &city), None, "no walls, no breach");
    let mut off = train(false);
    g.cities.get_mut(&cid).unwrap().wall_hp = 10;
    siege(&mut off, cid, SiegeStage::Invest, &[(gun, post)]);
    let city = CityView::of(&g, cid).unwrap();
    assert_eq!(off.near_breach(&g, 0, &city), None, "gene off");
}

/// Two posts two tiles out, each with a start three tiles out beside it, the
/// posts apart.
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

#[test]
fn a_lethal_single_blow_bars_the_post_even_shared_across_the_group() {
    let (mut g, cid) = open_walled_city();
    let lanes = two_lanes(&g, cid);
    let guns: Vec<u32> = lanes
        .iter()
        .map(|(start, _)| {
            let gun = g.spawn_unit("catapult", 0, *start);
            g.units.get_mut(&gun).unwrap().hp = GUN_HP;
            gun
        })
        .collect();
    wear_walls_to(&mut g, cid, guns[0], 2.0);
    let mut ai = train(true);
    siege(
        &mut ai,
        cid,
        SiegeStage::Invest,
        &[(guns[0], lanes[0].1), (guns[1], lanes[1].1)],
    );
    // A city whose one blow may finish a gun although, shared across the
    // pair, the reading would admit both.
    let base = g.city_ranged_strength(cid);
    let strength = (0..80)
        .map(|step| base + f64::from(step) * 0.5)
        .find(|strength| {
            std::sync::Arc::make_mut(&mut g.observed_city_ranged_strength).insert(cid, *strength);
            let shared = ai.entry_danger(&g, 0, lanes[0].1, guns[0], 2);
            let single = strongest_blow(&g, 0, lanes[0].1, guns[0]);
            f64::from(GUN_HP) > shared * NEAR_BREACH_REPLY_ROLL
                && f64::from(GUN_HP) <= single * NEAR_BREACH_REPLY_ROLL
        })
        .expect("fixture: a strike lethal alone, survivable shared");
    std::sync::Arc::make_mut(&mut g.observed_city_ranged_strength).insert(cid, strength);
    let city = CityView::of(&g, cid).unwrap();
    assert_eq!(
        ai.near_breach(&g, 0, &city).map(|(group, _)| group.len()),
        Some(2)
    );
    for (gun, (start, _)) in guns.iter().zip(&lanes) {
        assert_eq!(ai.post_step(&mut g, 0, *gun, &city), None);
        assert_eq!(g.units[gun].pos, *start);
    }
}
