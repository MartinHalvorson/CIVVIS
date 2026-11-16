//! `siege-guns-take-a-reachable-post`: a gun boxed behind our own soldier
//! on the near side of the band, its kept post across the city, takes the
//! near post; the soldier steps off it; a Stage escort stops waiting at the
//! muster line once a gun stands in range.

use super::reachable_post::{stalled_guns, HeldPost, GUN_POST_PATIENCE_TURNS};
use super::tests::{plan_against, walled_city};
use super::*;

const TAG: &str = "siege-guns-take-a-reachable-post";

/// `walled_city` with mountains everywhere but the city's first ring, the
/// three-tile ring round it, a near firing tile two west of the city and a
/// far one two east. Our Catapult stands four west, its one way out the
/// ring-3 tile beside it; our Spearman stands on the near firing tile. The
/// far tile is reached only round the ring-3 road, half the city round.
fn boxed_gun() -> (Game, u32, u32, u32, Pos, Pos) {
    let (mut g, cid) = walled_city();
    let city = g.cities[&cid].pos;
    let at = |x, y| (city.0 + x, city.1 + y);
    let start = at(-4, 0);
    let near = at(-2, 0);
    let far = at(2, 0);
    let ring_three: BTreeSet<Pos> = g.wring(city, 3).into_iter().collect();
    for tile in g.map.tiles.values_mut() {
        let d = {
            let (dq, dr) = (tile.pos.0 - city.0, tile.pos.1 - city.1);
            dq.abs().max(dr.abs()).max((dq + dr).abs())
        };
        tile.terrain = if tile.pos == city
            || d == 1
            || ring_three.contains(&tile.pos)
            || [start, near, far].contains(&tile.pos)
        {
            crate::name!("grassland")
        } else {
            crate::name!("mountain")
        };
        tile.feature = None;
        tile.hills = false;
    }
    g.map_script = crate::setup::MapScript::Pangaea;
    g.turn = 30;
    g.at_war.insert((0, 1));
    let gun = g.spawn_unit("catapult", 0, start);
    let spear = g.spawn_unit("spearman", 0, near);
    assert_eq!(g.unit_attack_range(gun), 2);
    assert!(g.unit_has_line_of_sight_from(gun, near, city));
    assert!(g.unit_has_line_of_sight_from(gun, far, city));
    let far_walk = g
        .route_preview(gun, far)
        .expect("fixture: a road round the city");
    assert!(
        far_walk.last().unwrap().1 > 1,
        "fixture: the far post is more than a turn away, {far_walk:?}"
    );
    (g, cid, gun, spear, near, far)
}

fn train(gene: bool) -> AdvancedAi {
    let mut ai = AdvancedAi::new();
    ai.enable_siege_train();
    ai.enable_shared_danger();
    ai.enable_recorded_tactical_step();
    if gene {
        ai.enable_siege_guns_take_a_reachable_post();
    }
    ai
}

/// The siege in Invest with `gun` posted on `post`.
fn investing(ai: &mut AdvancedAi, cid: u32, gun: u32, post: Pos) {
    ai.sieges.insert(
        cid,
        Siege {
            stage: SiegeStage::Invest,
            taker: None,
            entered: 25,
            assessed: 29,
            posts: [(gun, post)].into_iter().collect(),
            short_since: None,
        },
    );
}

#[test]
fn the_gene_ships_off_and_is_registered() {
    let ai = AdvancedAi::new();
    assert!(!ai.siege_guns_take_a_reachable_post);
    assert!(ai.siege_gun_posts_held.is_empty());
    assert!(super::super::GENES.iter().any(|gene| gene.opt_in()
        && gene.tag == TAG
        && gene.field == "siege_guns_take_a_reachable_post"));
}

#[test]
fn off_the_gun_keeps_its_post_across_the_city() {
    let (g, cid, gun, _, _, far) = boxed_gun();
    let city = CityView::of(&g, cid).unwrap();
    assert_eq!(
        siege_posts(&g, 0, &city, &[gun], None).get(&gun),
        Some(&far),
        "today our Spearman on the near tile sends the gun across the city"
    );
    let previous: BTreeMap<u32, Pos> = [(gun, far)].into_iter().collect();
    let kept = siege_posts_keeping(&g, 0, &city, &[gun], None, &previous);
    assert_eq!(kept.get(&gun), Some(&far), "and keeps it there");
    assert_eq!(
        siege_posts_reaching(&g, 0, &city, &[gun], None, &previous, &BTreeSet::new()),
        (kept, BTreeSet::new()),
        "no stalled gun is the old search, and nothing is drawn again"
    );
    let held: BTreeMap<u32, HeldPost> = BTreeMap::new();
    assert!(
        stalled_guns(&g, &previous, &held, 40).is_empty(),
        "a gun with no clock (the gene off keeps none) is never stalled"
    );
}

#[test]
fn a_gun_blocked_by_our_own_unit_gets_a_reachable_post() {
    let (g, cid, gun, _, near, far) = boxed_gun();
    let city = CityView::of(&g, cid).unwrap();
    let previous: BTreeMap<u32, Pos> = [(gun, far)].into_iter().collect();
    let since = 30 - GUN_POST_PATIENCE_TURNS;
    let held: BTreeMap<u32, HeldPost> = [(
        gun,
        HeldPost {
            post: far,
            since,
            repicked: false,
        },
    )]
    .into_iter()
    .collect();
    assert!(
        stalled_guns(&g, &previous, &held, 30 - 1).is_empty(),
        "within its patience the gun keeps walking"
    );
    let stalled = stalled_guns(&g, &previous, &held, 30);
    assert_eq!(stalled, [gun].into_iter().collect());
    let (posts, drawn) = siege_posts_reaching(&g, 0, &city, &[gun], None, &previous, &stalled);
    assert_eq!(
        posts.get(&gun),
        Some(&near),
        "the near tile our Spearman holds, a step from the gun's next stand"
    );
    assert_eq!(drawn, [gun].into_iter().collect());
}

#[test]
fn a_shooter_on_the_band_is_never_asked_to_step_off() {
    let (mut g, cid, gun, spear, near, far) = boxed_gun();
    g.remove_unit(spear);
    let crossbow = g.spawn_unit("crossbowman", 0, near);
    assert_eq!(arm_of(&g, crossbow), Arm::Shooter);
    let city = CityView::of(&g, cid).unwrap();
    let previous: BTreeMap<u32, Pos> = [(gun, far)].into_iter().collect();
    let stalled: BTreeSet<u32> = [gun].into_iter().collect();
    let (posts, drawn) =
        siege_posts_reaching(&g, 0, &city, &[gun, crossbow], None, &previous, &stalled);
    assert_ne!(posts.get(&gun), Some(&near), "the shooter keeps its tile");
    assert_eq!(posts.get(&crossbow), Some(&near), "and fires from it");
    assert!(
        drawn.is_empty(),
        "no other band tile off the first ring: the old search's post stands"
    );
}

#[test]
fn the_blocking_unit_steps_aside_and_the_gun_takes_the_post() {
    for gene in [false, true] {
        let (mut g, cid, gun, spear, near, _) = boxed_gun();
        let mut ai = train(gene);
        investing(&mut ai, cid, gun, near);
        ai.siege_gun_posts_held.insert(
            gun,
            HeldPost {
                post: near,
                since: 30,
                repicked: true,
            },
        );
        let journal = crate::reasoning::Journal::recording();
        crate::ai::Ai::attach_journal(&mut ai, journal.handle());
        let view = CityView::of(&g, cid).unwrap();
        let stepped = ai.clear_gun_post(&mut g, 0, spear, &view);
        if !gene {
            assert_eq!(stepped, None);
            assert_eq!(g.units[&spear].pos, near, "off, the Spearman stays");
            continue;
        }
        assert_eq!(stepped, Some(true));
        let off = g.units[&spear].pos;
        assert_ne!(off, near, "the Spearman leaves the gun's post");
        assert!(g.wdist(off, view.pos) > 1, "off the city's first ring");
        assert!(
            journal
                .since(0)
                .thoughts
                .iter()
                .any(|t| t.headline.starts_with(TAG) && t.headline.contains("steps off")),
            "the step is journaled under the tag"
        );
        for _ in 0..3 {
            if g.units[&gun].pos == near {
                break;
            }
            ai.post_step(&mut g, 0, gun, &view);
            g.apply(0, &Action::EndTurn).unwrap();
            g.apply(1, &Action::EndTurn).unwrap();
        }
        assert_eq!(g.units[&gun].pos, near, "the gun stands on its post");
    }
}

#[test]
fn the_gun_s_own_step_clears_its_post_first() {
    let (mut g, cid, gun, spear, near, _) = boxed_gun();
    let mut ai = train(true);
    investing(&mut ai, cid, gun, near);
    ai.siege_gun_posts_held.insert(
        gun,
        HeldPost {
            post: near,
            since: 30,
            repicked: true,
        },
    );
    let view = CityView::of(&g, cid).unwrap();
    ai.post_step(&mut g, 0, gun, &view);
    assert_ne!(g.units[&spear].pos, near, "the Spearman stepped off");
    assert!(
        g.wdist(g.units[&gun].pos, near) < 2,
        "the gun closed on its post: {:?}",
        g.units[&gun].pos
    );
}

#[test]
fn a_post_the_old_search_drew_is_not_cleared() {
    let (mut g, cid, gun, spear, near, _) = boxed_gun();
    let mut ai = train(true);
    investing(&mut ai, cid, gun, near);
    let view = CityView::of(&g, cid).unwrap();
    assert_eq!(ai.clear_gun_post(&mut g, 0, spear, &view), None);
    assert_eq!(g.units[&spear].pos, near);
}

/// `muster_tests`' board: a hostile Swordsman two west of the city, our
/// Warrior four west on the staging ring inside its reach.
fn ring_under_a_raider() -> (Game, u32, u32, Pos) {
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
    g.spawn_unit("swordsman", 1, (city.0 - 2, city.1));
    let ring = (city.0 - 4, city.1);
    let warrior = g.spawn_unit("warrior", 0, ring);
    (g, cid, warrior, ring)
}

#[test]
fn escorts_stop_waiting_at_the_muster_once_a_gun_stands_in_range() {
    for (gene, gun_in_range) in [(false, true), (true, false), (true, true)] {
        let (mut g, cid, warrior, ring) = ring_under_a_raider();
        let city = CityView::of(&g, cid).unwrap();
        if gun_in_range {
            let gun = g.spawn_unit("catapult", 0, (city.pos.0 + 2, city.pos.1));
            assert!(reachable_post::gun_stands_in_range(&g, 0, city.pos));
            assert!(g.wdist(g.units[&gun].pos, city.pos) <= g.unit_attack_range(gun));
        }
        let plan = plan_against(&g, cid);
        let mut ai = train(gene);
        ai.enable_stage_musters_out_of_reach();
        ai.stage_muster_ready.insert(cid, false);
        ai.siege_stage_step(&mut g, 0, warrior, &city, &plan);
        let at = g.units[&warrior].pos;
        if gene && gun_in_range {
            assert_eq!(at, ring, "the escort keeps the staging ring by its gun");
        } else {
            assert_ne!(
                at, ring,
                "gene {gene}, gun {gun_in_range}: the train cannot close, so it falls back"
            );
        }
    }
}
